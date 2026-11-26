//! Geometria do visualizador, sem gpui (regra R4: tudo que se testa sem janela
//! fica aqui).
//!
//! Convencoes:
//! - `area` e `img` sao tamanhos `(largura, altura)` em pixels: `area` e a
//!   regiao de desenho do visualizador, `img` a imagem decodificada.
//! - Coordenadas de tela sao pixels relativos ao canto superior esquerdo da area.
//! - Coordenadas de imagem sao relativas, `0..=1` em cada eixo (`(0, 0)` e o
//!   canto superior esquerdo da imagem, `(1, 1)` o inferior direito).
//! - Escala: `fit_scale(area, img) = min(area_w / img_w, area_h / img_h)`
//!   ("contain": a imagem inteira cabe na area) e a escala total, em pixels de
//!   tela por pixel de imagem, e `fit_scale * zoom`. Zoom 1.0 = ajustar.
//! - O ponto de imagem `(cx, cy)` do `Framing` e desenhado no CENTRO da
//!   area.

use marginalia_core::model::Framing;
use marginalia_core::rotation::Rotation;

use crate::load::{Layer, ViewWindow};

pub const MIN_ZOOM: f32 = 1.0;
pub const MAX_ZOOM: f32 = 32.0;

/// Tamanho `(largura, altura)` em pixels.
pub type Size = (f32, f32);

/// Enquadramento padrao: ajustado a janela, centrado.
pub fn default_framing() -> Framing {
    Framing { zoom: 1.0, cx: 0.5, cy: 0.5 }
}

/// Escala "contain": `min(area_w / img_w, area_h / img_h)`. Devolve 1.0 se
/// algum tamanho for zero, negativo ou nao finito (nunca NaN nem infinito).
pub fn fit_scale(area: Size, img: Size) -> f32 {
    match (valid(area), valid(img)) {
        (true, true) => {
            let s = (area.0 / img.0).min(area.1 / img.1);
            if s.is_finite() && s > 0.0 {
                s
            } else {
                1.0
            }
        }
        _ => 1.0,
    }
}

/// Limita o zoom a `MIN_ZOOM..=MAX_ZOOM` e o centro a `0..=1`. NaN ou infinito
/// viram os padroes (zoom 1.0, centro 0.5/0.5).
pub fn clamp(fr: Framing) -> Framing {
    let default = default_framing();
    let zoom = if fr.zoom.is_finite() { fr.zoom.clamp(MIN_ZOOM, MAX_ZOOM) } else { default.zoom };
    let cx = if fr.cx.is_finite() { fr.cx.clamp(0.0, 1.0) } else { default.cx };
    let cy = if fr.cy.is_finite() { fr.cy.clamp(0.0, 1.0) } else { default.cy };
    Framing { zoom, cx, cy }
}

/// Ponto de imagem relativo `p_img01` -> pixels de tela relativos a area.
/// `tela = centro_da_area + (p - (cx, cy)) * img * fit_scale * zoom`.
pub fn image_to_screen(fr: &Framing, area: Size, img: Size, p_img01: (f32, f32)) -> (f32, f32) {
    let middle = area_middle(area);
    let Some((px_w, px_h)) = pixels_by_unit(fr, area, img) else {
        return middle;
    };
    let e = clamp(fr.clone());
    let s = (middle.0 + (p_img01.0 - e.cx) * px_w, middle.1 + (p_img01.1 - e.cy) * px_h);
    if s.0.is_finite() && s.1.is_finite() {
        s
    } else {
        middle
    }
}

/// Inverso de [`image_to_screen`]: pixels de tela -> ponto de imagem relativo
/// (pode cair fora de `0..=1` se a tela mostrar fundo fora da imagem). Com
/// tamanhos degenerados devolve o centro `(cx, cy)`.
pub fn screen_to_image(fr: &Framing, area: Size, img: Size, p_screen: (f32, f32)) -> (f32, f32) {
    let e = clamp(fr.clone());
    let Some((px_w, px_h)) = pixels_by_unit(&e, area, img) else {
        return (e.cx, e.cy);
    };
    let middle = area_middle(area);
    let p = (e.cx + (p_screen.0 - middle.0) / px_w, e.cy + (p_screen.1 - middle.1) / px_h);
    if p.0.is_finite() && p.1.is_finite() {
        p
    } else {
        (e.cx, e.cy)
    }
}

/// Multiplica o zoom por `factor` mantendo fixo, na tela, o ponto de imagem que
/// esta sob `anchor_screen`. Depois limita o zoom a `MIN_ZOOM..=MAX_ZOOM` e o
/// centro a `0..=1` (os limites podem deslocar o ponto ancorado).
pub fn zoom_at(fr: &Framing, area: Size, img: Size, factor: f32, anchor_screen: (f32, f32)) -> Framing {
    let e = clamp(fr.clone());
    let factor = if factor.is_finite() && factor > 0.0 { factor } else { 1.0 };
    let zoom = (e.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
    let anchor = if anchor_screen.0.is_finite() && anchor_screen.1.is_finite() {
        anchor_screen
    } else {
        area_middle(area)
    };
    let p = screen_to_image(&e, area, img, anchor);
    let new = Framing { zoom, ..e.clone() };
    let Some((px_w, px_h)) = pixels_by_unit(&new, area, img) else {
        return clamp(new);
    };
    let middle = area_middle(area);
    // resolve image_to_screen(novo, p) == ancora para o centro
    clamp(Framing {
        zoom,
        cx: p.0 - (anchor.0 - middle.0) / px_w,
        cy: p.1 - (anchor.1 - middle.1) / px_h,
    })
}

/// Arrasto de `(dx, dy)` pixels de tela: a imagem acompanha o ponteiro, logo
/// o centro anda no sentido oposto, `dx / (img_w * escala_total)`. Centro
/// limitado a `0..=1`.
pub fn pan_by(fr: &Framing, area: Size, img: Size, dx_screen: f32, dy_screen: f32) -> Framing {
    let e = clamp(fr.clone());
    let Some((px_w, px_h)) = pixels_by_unit(&e, area, img) else {
        return e;
    };
    let dx = if dx_screen.is_finite() { dx_screen } else { 0.0 };
    let dy = if dy_screen.is_finite() { dy_screen } else { 0.0 };
    clamp(Framing { cx: e.cx - dx / px_w, cy: e.cy - dy / px_h, ..e })
}

/// Fator de zoom de um evento de roda: `exp(delta_y * por_pixel)`, limitado a
/// `FATOR_RODA_MIN..=FATOR_RODA_MAX` (uma roda que salta muitas linhas num so
/// evento nao pode multiplicar o zoom sem limite). Delta nao finito = 1.0.
pub fn runs_factor(delta_y: f32, by_pixel: f32) -> f32 {
    let f = (delta_y * by_pixel).exp();
    if f.is_finite() && !f.is_nan() && delta_y.is_finite() {
        f.clamp(FACTOR_RUNS_MIN, FACTOR_RUNS_MAX)
    } else {
        1.0
    }
}

/// Fracao do menor lado da area que um toque de W/A/S/D anda.
pub const STEP_PAN_FRACTION: f32 = 0.1;
/// Passo (pixels) quando a area ainda nao foi medida ou e degenerada.
pub const STEP_PAN_DEFAULT: f32 = 40.0;
const STEP_PAN_MIN: f32 = 4.0;
const STEP_PAN_MAX: f32 = 400.0;

/// Pixels de TELA por toque de W/A/S/D (para [`pan_by`]): 10% do menor lado da
/// area, entre 4 e 400 px. Nao depende do zoom: cada toque move a imagem a
/// mesma distancia na tela (no zoom alto, uma fracao menor da pagina), e o
/// passo nunca zera nem explode. Area degenerada (zero, negativa, NaN):
/// [`STEP_PAN_DEFAULT`]. Segurar a tecla repete pela repeticao do sistema.
pub fn pan_step(_zoom: f32, area: Size) -> f32 {
    if !valid(area) {
        return STEP_PAN_DEFAULT;
    }
    let p = area.0.min(area.1) * STEP_PAN_FRACTION;
    if p.is_finite() {
        p.clamp(STEP_PAN_MIN, STEP_PAN_MAX)
    } else {
        STEP_PAN_DEFAULT
    }
}

pub const FACTOR_RUNS_MIN: f32 = 0.5;
pub const FACTOR_RUNS_MAX: f32 = 2.0;

/// Fator de escala da tela saneado (NaN, infinito, zero ou negativo: 1).
fn sanitized_factor(factor: f32) -> f32 {
    if factor.is_finite() && factor > 0.0 { factor } else { 1.0 }
}

/// A janela visivel da pagina para o tile: o retangulo que a area mostra (os
/// cantos da area por [`screen_to_image`], grampeados em 0..1), na grade da
/// pagina inteira com o tamanho FISICO que ela tem na tela (pixels logicos da
/// pagina × `factor_scale_screen`, o `window.scale_factor()`; nunca maior que a
/// pagina), arredondado para fora ([`ViewWindow::in_screen`]). `img` e o tamanho
/// ORIGINAL da pagina (nunca o da textura). Area ou pagina sem tamanho, ou uma
/// janela vazia: `None` (nada a pedir).
///
/// A pagina pode estar exibida girada em `rot`: `fr` esta no espaco de EXIBICAO
/// (a pagina ja girada; `img` continua sendo o tamanho ORIGINAL dela), mas a
/// janela sai na grade da pagina SEM girar, que e onde o tile e recortado: o
/// retangulo visivel volta para a imagem por `Rotation::display_rectangle` e o
/// tamanho fisico por eixo troca de lugar em 90 e 270 graus.
pub fn window_visible_rotated(
    fr: &Framing,
    area: Size,
    img: Size,
    rot: Rotation,
    factor_scale_screen: f32,
) -> Option<ViewWindow> {
    let display = display_sz(img, rot);
    let (pw, ph) = pixels_by_unit(fr, area, display)?;
    let (pw, ph) = if rot.sides_swap() { (ph, pw) } else { (pw, ph) };
    let factor = sanitized_factor(factor_scale_screen);
    let a = screen_to_image(fr, area, display, (0.0, 0.0));
    let b = screen_to_image(fr, area, display, area);
    let c = |v: f32| v.clamp(0.0, 1.0);
    let in_display = [c(a.0).min(c(b.0)), c(a.1).min(c(b.1)), c(a.0).max(c(b.0)), c(a.1).max(c(b.1))];
    if !(in_display[2] > in_display[0] && in_display[3] > in_display[1]) {
        return None;
    }
    let visible = rot.display_rectangle(in_display);
    let page = (img.0.round() as u32, img.1.round() as u32);
    let j = ViewWindow::in_screen(page, (pw * factor, ph * factor), visible);
    (!j.empty()).then_some(j)
}

/// Tamanho da pagina na exibicao: os lados trocam em 90 e 270 graus.
pub fn display_sz(img: Size, rot: Rotation) -> Size {
    if rot.sides_swap() {
        (img.1, img.0)
    } else {
        img
    }
}

/// Caixa `(x0, y0, x1, y1)` da pagina inteira na tela (pixels relativos a area).
pub fn page_textbox(fr: &Framing, area: Size, img: Size) -> (f32, f32, f32, f32) {
    let a = image_to_screen(fr, area, img, (0.0, 0.0));
    let b = image_to_screen(fr, area, img, (1.0, 1.0));
    (a.0, a.1, b.0, b.1)
}


/// Caixa `(x0, y0, x1, y1)` do tile da janela `j` na tela: a imagem-para-tela
/// dos cantos da regiao na grade (`x0 / grade_w`, ...). So depende do tamanho da
/// pagina, nunca do tamanho da textura.
///
/// Com a pagina exibida girada em `rot`: a regiao da janela esta na grade da
/// pagina SEM girar (`img` e o tamanho original), `fr` no espaco de exibicao. A
/// regiao vai para a exibicao (`Rotation::rectangle_to_display`) antes de ir
/// para a tela.
pub fn rotated_window_textbox(fr: &Framing, area: Size, img: Size, rot: Rotation, j: &ViewWindow) -> (f32, f32, f32, f32) {
    let f = |v: u32, g: u32| if g == 0 { 0.0 } else { v as f32 / g as f32 };
    let r = j.region;
    let in_image = [f(r.x0, j.grid.0), f(r.y0, j.grid.1), f(r.x1, j.grid.0), f(r.y1, j.grid.1)];
    let [x0, y0, x1, y1] = rot.rectangle_to_display(in_image);
    let display = display_sz(img, rot);
    let a = image_to_screen(fr, area, display, (x0, y0));
    let b = image_to_screen(fr, area, display, (x1, y1));
    (a.0, a.1, b.0, b.1)
}

/// Origem e tamanho (pixels logicos) para `paint_image` cobrir EXATAMENTE os
/// pixels fisicos `[floor(x0·f), ceil(x1·f))` (e o mesmo em y) da caixa
/// `(x0, y0, x1, y1)`: o gpui escala pelo fator e arredonda a origem para baixo
/// e o tamanho para cima, cada um por si, o que deixaria a borda direita de duas
/// caixas com a mesma borda a 1 px uma da outra. Arredondando para fora aqui,
/// base e tile que acabam na mesma borda acabam no mesmo pixel, e o tile nunca
/// deixa uma fresta de 1 px. Caixa invertida: tamanho zero.
pub fn textbox_to_outside(c: (f32, f32, f32, f32), factor_scale_screen: f32) -> ((f32, f32), (f32, f32)) {
    let f = sanitized_factor(factor_scale_screen);
    let (x0, y0) = ((c.0 * f).floor(), (c.1 * f).floor());
    let (x1, y1) = ((c.2 * f).ceil(), (c.3 * f).ceil());
    let side = |a: f32, b: f32| if b > a { (b - a - 0.5) / f } else { 0.0 };
    // +0,25 / -0,5 px fisico: a margem contra o erro do f32 na multiplicacao do gpui.
    (((x0 + 0.25) / f, (y0 + 0.25) / f), (side(x0, x1), side(y0, y1)))
}

/// As texturas na tela, uma por camada: a cheia (sem ajuste, sozinha) ou a base
/// (pagina inteira, por baixo) e o tile (a janela, por cima). Espelha as camadas
/// do estado (`AppState::base_display` / `tile_display`): base e tile so ficam
/// juntos quando sao dos mesmos ajustes, entao o fundo nunca mostra a pagina sem
/// filtro nem o filtro de outro valor. Generico na textura (testes sem gpui).
#[derive(Debug)]
pub struct Layers<T> {
    pub full: Option<T>,
    pub base: Option<T>,
    pub tile: Option<(T, ViewWindow)>,
}

impl<T> Default for Layers<T> {
    fn default() -> Self {
        Layers { full: None, base: None, tile: None }
    }
}

impl<T> Layers<T> {
    pub fn has(&self) -> bool {
        self.full.is_some() || self.base.is_some() || self.tile.is_some()
    }

    /// Poe `t` na `layer`. A cheia tira as outras; base e tile tiram a cheia;
    /// `keep_other`: o estado guardou a outra camada de ajuste (a base por baixo
    /// do tile novo, o tile por cima da base nova), senao ela sai tambem. Devolve
    /// as texturas que sairam (para `drop_image`), cada uma uma vez.
    pub fn receive(&mut self, layer: Layer, t: T, keep_other: bool) -> Vec<T> {
        let mut left = Vec::new();
        match layer {
            Layer::Full => {
                left.extend(self.clear());
                self.full = Some(t);
            }
            Layer::Base => {
                left.extend(self.full.take());
                left.extend(self.base.replace(t));
                if !keep_other {
                    left.extend(self.tile.take().map(|(t, _)| t));
                }
            }
            Layer::Tile(j) => {
                left.extend(self.full.take());
                left.extend(self.tile.replace((t, j)).map(|(t, _)| t));
                if !keep_other {
                    left.extend(self.base.take());
                }
            }
        }
        left
    }

    /// Tira todas (a fonte fechou, erro): as texturas que sairam.
    pub fn clear(&mut self) -> Vec<T> {
        let mut left: Vec<T> = Vec::new();
        left.extend(self.full.take());
        left.extend(self.base.take());
        left.extend(self.tile.take().map(|(t, _)| t));
        left
    }
}

fn valid(s: Size) -> bool {
    s.0.is_finite() && s.1.is_finite() && s.0 > 0.0 && s.1 > 0.0
}

/// Centro da area; `(0, 0)` se a area for degenerada.
fn area_middle(area: Size) -> (f32, f32) {
    if valid(area) {
        (area.0 / 2.0, area.1 / 2.0)
    } else {
        (0.0, 0.0)
    }
}

/// Pixels de tela por unidade relativa de imagem em cada eixo:
/// `img * fit_scale * zoom`. `None` se algum tamanho for degenerado.
pub fn pixels_by_unit(fr: &Framing, area: Size, img: Size) -> Option<(f32, f32)> {
    if !valid(area) || !valid(img) {
        return None;
    }
    let scale = fit_scale(area, img) * clamp(fr.clone()).zoom;
    let (w, h) = (img.0 * scale, img.1 * scale);
    (w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0).then_some((w, h))
}

#[cfg(test)]
#[path = "tests/viewer_math.rs"]
mod tests;
