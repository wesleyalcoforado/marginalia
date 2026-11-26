//! Carga de um quadro para exibicao, sem gpui (regra R4).
//!
//! Tudo aqui bloqueia (leitura do NAS, decodificacao, filtros) e por isso roda
//! FORA da thread da interface: a view chama [`execute`] no executor de fundo
//! do gpui e entrega a [`Response`] ao `AppState::receive`.

use std::borrow::Cow;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use image::imageops::{self, FilterType};
use image::RgbaImage;
use marginalia_core::decode::{DecodeError, DecodedPage};
use marginalia_core::filters::{self, Adjustments};
pub use marginalia_core::crop::GridRegion;
use marginalia_core::crop::{trim_margin, grid_scale, grid_crop, Crop};
use marginalia_core::model::Framing;
use marginalia_core::prefetch::{self, Prefetcher};
use marginalia_core::rotation::Rotation;
use texts::t;

/// Maior lado de textura que o atlas Metal do gpui aceita (`metal_atlas.rs`).
pub const SIDE_MAX_TEXTURE: u32 = 16384;

/// Como posicionar a imagem quando ela chegar.
#[derive(Clone, Debug, PartialEq)]
pub enum Reframe {
    /// Mesmo quadro (so mudou o ajuste): manter zoom e centro.
    Keep,
    /// Quadro novo: voltar a ajustar a janela (zoom 1.0, centro).
    Adjust,
    /// Quadro aberto com um enquadramento guardado (de uma nota).
    Adopt(Framing),
}

/// Imagem pronta para virar textura: pixels em **BGRA** (o que o gpui espera),
/// no maximo `SIDE_MAX_TEXTURE` por lado, e o tamanho ORIGINAL da pagina (a
/// geometria do visualizador e dos grifos usa a proporcao original). O `Arc`
/// chega ao visualizador como a unica referencia (o estado guarda so o tamanho,
/// `AppState::tile_display`): virar textura nao copia os pixels.
#[derive(Debug)]
pub struct Display {
    pub bgra: Arc<RgbaImage>,
    /// Tamanho da pagina SEM girar (`bgra` tem os lados trocados em 90/270).
    pub sz_original: (u32, u32),
    /// Rotacao com que os pixels de `bgra` foram girados.
    pub rot: Rotation,
}

impl Display {
    /// Gira os pixels (o filtro e isotropico, entao girar depois dele da o mesmo
    /// resultado). `R0` nao copia nada.
    pub fn rotated(self, rot: Rotation) -> Display {
        if rot == Rotation::R0 {
            return self;
        }
        Display { bgra: Arc::new(rot.rotate_rgba(&self.bgra)), sz_original: self.sz_original, rot }
    }
}

/// `Arc<Prefetcher>` cuja ULTIMA copia e solta numa thread propria: o `Drop`
/// do `Prefetcher` junta as threads de fundo, que podem estar presas numa
/// leitura do NAS, e isso nunca pode acontecer na thread da interface (pedido
/// cancelado, troca de fonte, fim do app). `Arc::into_inner` garante que so
/// uma das copias soltas ao mesmo tempo fica com o valor.
pub struct PrefetcherShared(Option<Arc<Prefetcher>>);

impl PrefetcherShared {
    pub fn new(p: Prefetcher) -> Self {
        PrefetcherShared(Some(Arc::new(p)))
    }
}

impl Clone for PrefetcherShared {
    fn clone(&self) -> Self {
        PrefetcherShared(self.0.clone())
    }
}

impl std::ops::Deref for PrefetcherShared {
    type Target = Prefetcher;
    fn deref(&self) -> &Prefetcher {
        self.0.as_ref().expect("presente ate o drop")
    }
}

impl Drop for PrefetcherShared {
    fn drop(&mut self) {
        if let Some(last) = self.0.take().and_then(Arc::into_inner) {
            std::thread::spawn(move || drop(last));
        }
    }
}

/// Maior lado da copia reduzida da pagina inteira usada na base dos ajustes.
pub const SIDE_BASE: u32 = 2048;

/// O que o pedido produz.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quality {
    /// Com ajuste: a pagina inteira filtrada sobre a copia reduzida (lado maior <=
    /// [`SIDE_BASE`]). So camada de fundo: cobre o que o tile ainda nao cobre.
    Base,
    /// Com ajuste: so a janela visivel ([`ViewWindow`]), na resolucao da tela, filtrada.
    Tile,
    /// Sem ajuste: a pagina inteira sem filtro (limitada so pela textura).
    Full,
}

/// Onde uma imagem aceita para a tela e desenhada (o visualizador guarda uma
/// textura por camada).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// Sem ajuste: a textura cheia, sozinha.
    Full,
    /// A base: esticada sobre a pagina inteira, por baixo do tile.
    Base,
    /// O tile: na sub-caixa da sua janela, por cima da base.
    Tile(ViewWindow),
}

/// Regiao visivel da pagina em pixels da *grade*: a pagina inteira reamostrada
/// para o tamanho fisico que ela tem na tela, `grade = (round(W·sx), round(H·sy))`
/// com `s = min(1, fisico / lado_da_pagina)` por eixo (pagina afastada usa uma grade
/// menor, aproximada usa a nativa; nunca amplia). O tile tem exatamente
/// `(x1-x0) × (y1-y0)` pixels (ver `marginalia_core::crop::grid_crop`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewWindow {
    pub grid: (u32, u32),
    /// Retangulo visivel na grade, arredondado para fora e grampeado.
    pub region: GridRegion,
}

impl ViewWindow {
    /// Grade da pagina `page` quando ela inteira ocupa `physical` pixels fisicos da
    /// tela: por eixo `clamp(round(min(lado, fisico)), 1, lado)` (NaN/infinito: nativa).
    pub fn screen_grid(page: (u32, u32), physical: (f32, f32)) -> (u32, u32) {
        let axis = |side: u32, f: f32| {
            if side == 0 {
                0
            } else if f.is_finite() {
                (f.min(side as f32).round().max(1.0) as u32).min(side)
            } else {
                side
            }
        };
        (axis(page.0, physical.0), axis(page.1, physical.1))
    }

    /// A janela visivel: `visivel = [x0, y0, x1, y1]` em fracoes da pagina (0..1),
    /// levadas para a grade da tela arredondando para fora e grampeadas.
    pub fn in_screen(page: (u32, u32), physical: (f32, f32), visible: [f32; 4]) -> ViewWindow {
        let grid = ViewWindow::screen_grid(page, physical);
        let frac = |v: f32| if v.is_finite() { v.clamp(0.0, 1.0) as f64 } else { 0.0 };
        let down = |v: f32, g: u32| ((frac(v) * g as f64).floor() as u32).min(g);
        let high = |v: f32, g: u32| ((frac(v) * g as f64).ceil() as u32).min(g);
        let [x0, y0, x1, y1] = visible;
        let region = GridRegion { x0: down(x0, grid.0), y0: down(y0, grid.1), x1: high(x1, grid.0), y1: high(y1, grid.1) };
        ViewWindow { grid, region }.normalized(page)
    }

    /// A pagina inteira numa grade que cabe na textura (o padrao enquanto o
    /// visualizador nao informa a janela visivel): na escala 1 e o filtro antigo.
    pub fn whole_page(page: (u32, u32)) -> ViewWindow {
        let grid = reduced_size(page.0, page.1, SIDE_MAX_TEXTURE);
        ViewWindow { grid, region: GridRegion { x0: 0, y0: 0, x1: grid.0, y1: grid.1 } }.normalized(page)
    }

    /// Grade grampeada na pagina (nunca amplia) e retangulo grampeado na grade; sem
    /// pixel sobrando, o retangulo vira o vazio padrao (o mesmo de `grid_crop`).
    pub fn normalized(self, page: (u32, u32)) -> ViewWindow {
        let grid = (self.grid.0.min(page.0), self.grid.1.min(page.1));
        let r = self.region;
        let (x0, x1) = (r.x0.min(grid.0), r.x1.min(grid.0));
        let (y0, y1) = (r.y0.min(grid.1), r.y1.min(grid.1));
        let region = if x1 > x0 && y1 > y0 { GridRegion { x0, y0, x1, y1 } } else { GridRegion::default() };
        ViewWindow { grid, region }
    }

    /// Tamanho do tile em pixels: `(x1-x0, y1-y0)`.
    pub fn size_px(&self) -> (u32, u32) {
        (self.region.x1.saturating_sub(self.region.x0), self.region.y1.saturating_sub(self.region.y0))
    }

    pub fn empty(&self) -> bool {
        let (w, h) = self.size_px();
        w == 0 || h == 0
    }

    /// Escala do sigma do filtro nesta grade (`grid_scale`).
    pub fn scale(&self, page: (u32, u32)) -> f32 {
        grid_scale(page, self.grid)
    }
}

/// Recorte reamostrado SEM filtro guardado para os tiles: so depende da janela e da
/// margem, entao serve a todos os valores do ajuste.
type TileSource = (ViewWindow, u32, Arc<Crop>);

/// Pagina decodificada (compartilhada com o cache do `Prefetcher`, nunca copiada),
/// a copia reduzida dela para a base (criada na primeira base, no fundo, dentro de
/// [`execute`]) e o recorte reamostrado da ultima janela pedida (a parte cara do
/// tile), reusado enquanto so os valores do ajuste mudam.
pub struct Page {
    pub decoded: Arc<DecodedPage>,
    /// `None` dentro: a pagina ja cabe em `SIDE_BASE` e a base usa a propria.
    reduced: OnceLock<Option<RgbaImage>>,
    tile_source: Mutex<Option<TileSource>>,
    tile_sources: AtomicU64,
}

impl Page {
    pub fn new(decoded: Arc<DecodedPage>) -> Page {
        Page { decoded, reduced: OnceLock::new(), tile_source: Mutex::new(None), tile_sources: AtomicU64::new(0) }
    }

    pub fn page_count(&self) -> u32 {
        self.decoded.page_count
    }

    /// Fonte da base: lado maior <= `SIDE_BASE`. Bloqueia na primeira chamada
    /// (reducao); so chamar fora da thread da interface.
    pub fn base(&self) -> &RgbaImage {
        let full = &self.decoded.image;
        self.reduced
            .get_or_init(|| {
                let (w, h) = full.dimensions();
                let (rw, rh) = reduced_size(w, h, SIDE_BASE);
                // A grade inteira pela reamostragem paralela do recorte: identica byte a
                // byte ao `imageops::resize` Triangle (serial; ~80 ms numa 6000x4000).
                let whole = GridRegion { x0: 0, y0: 0, x1: rw, y1: rh };
                ((rw, rh) != (w, h)).then(|| grid_crop(full, (rw, rh), whole, 0).img)
            })
            .as_ref()
            .unwrap_or(full)
    }

    /// Recorte reamostrado (sem filtro) de `view_window` com `margin` pixels da grade:
    /// o guardado se a janela e a margem sao as mesmas, senao um novo, que passa a
    /// ser o guardado. Bloqueia (reamostragem): so fora da thread da interface.
    pub fn tile_source(&self, view_window: ViewWindow, margin: u32) -> Arc<Crop> {
        let lock = || self.tile_source.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((j, m, r)) = lock().as_ref() {
            if (*j, *m) == (view_window, margin) {
                return Arc::clone(r);
            }
        }
        // Fora da trava: outro tile (de outra janela) nao espera esta reamostragem.
        let r = Arc::new(grid_crop(&self.decoded.image, view_window.grid, view_window.region, margin));
        self.tile_sources.fetch_add(1, Ordering::SeqCst);
        *lock() = Some((view_window, margin, Arc::clone(&r)));
        r
    }

}

/// Trabalho de carga. `decoded` presente = so reaplicar filtros.
pub struct Request {
    pub generation: u64,
    pub target: prefetch::Frame,
    pub decoded: Option<Arc<Page>>,
    pub image_adjustments: Adjustments,
    /// Base e tile so fazem sentido com ajuste; sem ajuste o estado pede a cheia.
    pub quality: Quality,
    /// Janela do tile; `None` (ou fora do tile): a pagina inteira
    /// ([`ViewWindow::whole_page`]), calculada quando a pagina estiver decodificada.
    pub view_window: Option<ViewWindow>,
    pub frame: Reframe,
    /// Rotacao de exibicao da imagem: os pixels saem girados (a janela e a grade
    /// continuam na pagina sem girar).
    pub rotation: Rotation,
    pub prefetcher: PrefetcherShared,
    /// Geracao mais recente pedida pelo estado; um pedido com geracao menor ja
    /// nao interessa e e abandonado antes do trabalho pesado.
    pub effective: Arc<AtomicU64>,
}

pub struct Response {
    pub generation: u64,
    pub quality: Quality,
    /// Tile: a janela realmente usada (normalizada na pagina). Base e cheia: `None`.
    pub view_window: Option<ViewWindow>,
    pub frame: Reframe,
    /// Os ajustes que o pedido levou (os da imagem que vai para a tela).
    pub image_adjustments: Adjustments,
    pub outcome: Outcome,
}

pub enum Outcome {
    Ready { page: Arc<Page>, display: Display },
    /// A imagem nao decodifica (corrompida, formato nao suportado): marcador de erro.
    Error(String),
    /// A FONTE nao pode ser lida (erro de E/S: NAS desmontado). Nao fica em cache:
    /// tentar de novo ou apontar a nova localizacao resolve.
    Unavailable(String),
    /// Abandonado porque um pedido mais novo chegou.
    Stale,
}

impl Request {
    /// O pedido le e decodifica a pagina (abrir o quadro, ou um filtro pedido antes
    /// de ela chegar); `false`: so refiltra a pagina ja decodificada (tile e base do
    /// pan/zoom), o que nao deixa o visualizador "carregando".
    pub fn page_loads(&self) -> bool {
        self.decoded.is_none()
    }

    fn stale(&self) -> bool {
        self.effective.load(Ordering::SeqCst) != self.generation
    }
}

/// Bloqueia: le/decodifica (via `Prefetcher::get`), recorta e reamostra a janela
/// (tile) ou reduz a pagina (base), aplica os ajustes e converte para BGRA. Um
/// pedido que fica obsoleto no meio do filtro para e devolve `Stale`.
pub fn execute(p: Request) -> Response {
    let (outcome, view_window) = load(&p);
    Response { generation: p.generation, quality: p.quality, view_window, frame: p.frame, image_adjustments: p.image_adjustments, outcome }
}

fn load(p: &Request) -> (Outcome, Option<ViewWindow>) {
    let mut view_window = None;
    let r = load_at(p, &mut view_window);
    (r, view_window)
}

fn load_at(p: &Request, used_window: &mut Option<ViewWindow>) -> Outcome {
    if p.stale() {
        return Outcome::Stale;
    }
    let page = match &p.decoded {
        Some(d) => Arc::clone(d),
        None => match p.prefetcher.get(&p.target) {
            Ok(d) => Arc::new(Page::new(d)),
            Err(e @ DecodeError::Reading(_)) => {
                return Outcome::Unavailable(t!("error.load.page", p.target.path, p.target.page, e))
            }
            Err(e) => return Outcome::Error(t!("error.load.page", p.target.path, p.target.page, e)),
        },
    };
    // A leitura pode ter demorado (NAS): nao filtrar o que ja nao sera mostrado.
    if p.stale() {
        return Outcome::Stale;
    }
    let sz_original = page.decoded.image.dimensions();
    let stop = || p.stale();
    let display = match p.quality {
        Quality::Base => cancellable_base(&page, &p.image_adjustments, &stop),
        Quality::Tile => {
            let view_window = p.view_window.unwrap_or_else(|| ViewWindow::whole_page(sz_original)).normalized(sz_original);
            *used_window = Some(view_window);
            cancellable_tile(&page, view_window, &p.image_adjustments, &stop)
        }
        Quality::Full => {
            cancellable_display(&page.decoded.image, sz_original, &p.image_adjustments, SIDE_MAX_TEXTURE, &stop)
        }
    };
    match display {
        Some(display) => Outcome::Ready { page, display: display.rotated(p.rotation) },
        None => Outcome::Stale,
    }
}

/// A base: a copia reduzida da pagina inteira ([`Page::base`]) filtrada com o
/// sigma da escala dela (o realce tem o mesmo alcance relativo que no tile).
pub fn cancellable_base(page: &Page, a: &Adjustments, stop: &dyn Fn() -> bool) -> Option<Display> {
    let sz_original = page.decoded.image.dimensions();
    cancellable_display(page.base(), sz_original, a, SIDE_MAX_TEXTURE, stop)
}

/// O tile da `view_window` (ja normalizada na pagina): o recorte reamostrado na grade
/// com margem `2 · raio_do_nucleo` (o alcance de nitidez e passa-alta juntas; o
/// guardado em [`Page::tile_source`] quando a janela e a mesma), filtrado com o
/// sigma da escala da grade, sem a margem (exatamente `view_window.size_px()`) e em
/// BGRA. `stop` verdadeiro: `None`.
pub fn cancellable_tile(page: &Page, view_window: ViewWindow, a: &Adjustments, stop: &dyn Fn() -> bool) -> Option<Display> {
    let sz_original = page.decoded.image.dimensions();
    let scale = view_window.scale(sz_original);
    let margin = 2 * filters::marginalia_core_radius(scale) as u32;
    let source = page.tile_source(view_window, margin);
    if stop() {
        return None;
    }
    let filtered = filters::apply_cancellable_in_scale(source.img.clone(), a, scale, stop)?;
    let mut bgra = trim_margin(filtered, source.margin);
    to_bgra(&mut bgra);
    Some(Display { bgra: Arc::new(bgra), sz_original, rot: Rotation::R0 })
}

fn to_bgra(img: &mut RgbaImage) {
    for px in img.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
    }
}

/// Reduz `img` para caber em `side_max`, aplica os ajustes (interrompivel por
/// `stop`: `None`, nunca uma imagem filtrada pela metade) e converte para BGRA.
/// `sz_original` e o tamanho da pagina cheia (`img` pode ser a copia reduzida); o
/// sigma do filtro segue a escala de `sz_original` para o tamanho filtrado.
pub fn cancellable_display(
    img: &RgbaImage,
    sz_original: (u32, u32),
    a: &Adjustments,
    side_max: u32,
    stop: &dyn Fn() -> bool,
) -> Option<Display> {
    let (w0, h0) = img.dimensions();
    let (w, h) = reduced_size(w0, h0, side_max);
    // Reduz ANTES dos filtros: mais barato, e o resultado so e usado na tela.
    let base: Cow<'_, RgbaImage> = if (w, h) == (w0, h0) {
        Cow::Borrowed(img)
    } else {
        Cow::Owned(imageops::resize(img, w, h, FilterType::Triangle))
    };
    let scale = grid_scale(sz_original, (w, h));
    // Uma copia so (o buffer que vira textura); o filtro trabalha sobre ela.
    let mut bgra = filters::apply_cancellable_in_scale(base.into_owned(), a, scale, stop)?;
    to_bgra(&mut bgra);
    Some(Display { bgra: Arc::new(bgra), sz_original, rot: Rotation::R0 })
}

/// Tamanho que cabe em `side_max` mantendo a proporcao (cada lado >= 1).
pub fn reduced_size(w: u32, h: u32, side_max: u32) -> (u32, u32) {
    let larger = w.max(h);
    if larger <= side_max || side_max == 0 {
        return (w, h);
    }
    let scale = |v: u32| ((v as u64 * side_max as u64 + larger as u64 / 2) / larger as u64).max(1) as u32;
    (scale(w), scale(h))
}

#[cfg(test)]
#[path = "tests/load.rs"]
mod tests;
