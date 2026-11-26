//! Filtros de visualizacao: ajustam apenas a vista da imagem, nunca o arquivo.

use image::RgbaImage;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

/// Ajustes de visualizacao. Veja [`apply`] para a semantica e a ordem.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Adjustments {
    pub brightness: f32,
    pub contrast: f32,
    pub invert: bool,
    pub grayscale: bool,
    pub sharpen: f32,
    pub highpass: f32,
    pub threshold: Option<u8>,
}

impl Default for Adjustments {
    fn default() -> Self {
        Adjustments {
            brightness: 0.0,
            contrast: 1.0,
            invert: false,
            grayscale: false,
            sharpen: 0.0,
            highpass: 0.0,
            threshold: None,
        }
    }
}

/// Desvio padrao (em pixels) do desfoque gaussiano usado por `sharpen` e `highpass`.
const BLUR_SIGMA: f32 = 2.0;
/// Maior multiplicador de contraste aceito.
const MAX_CONTRAST: f32 = 4.0;

/// Valores ja saneados: NaN vira o valor neutro e o resto e limitado ao intervalo valido.
struct Clean {
    brightness: f32,
    contrast: f32,
    sharpen: f32,
    highpass: f32,
}

fn clean(v: f32, neutral: f32, min: f32, max: f32) -> f32 {
    if v.is_nan() {
        neutral
    } else {
        v.clamp(min, max)
    }
}

impl Adjustments {
    fn clean(&self) -> Clean {
        Clean {
            brightness: clean(self.brightness, 0.0, -1.0, 1.0),
            contrast: clean(self.contrast, 1.0, 0.0, MAX_CONTRAST),
            sharpen: clean(self.sharpen, 0.0, 0.0, 1.0),
            highpass: clean(self.highpass, 0.0, 0.0, 1.0),
        }
    }

    /// `true` quando `apply` devolveria a imagem inalterada (NaN conta como neutro).
    pub fn is_neutral(&self) -> bool {
        let c = self.clean();
        c.brightness == 0.0
            && c.contrast == 1.0
            && c.sharpen == 0.0
            && c.highpass == 0.0
            && !self.invert
            && !self.grayscale
            && self.threshold.is_none()
    }
}

fn to_u8(v: f32) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

/// Luma Rec.601.
fn luma(r: u8, g: u8, b: u8) -> f32 {
    0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32
}

/// Linhas por bloco: `stop` e consultado uma vez antes de cada bloco de cada etapa.
/// Num 6000x4000 um bloco de blur leva ~2-3 ms, o que limita a latencia da parada.
const BLOCK: usize = 128;
/// Linhas por tarefa paralela dentro de um bloco (8 faixas por bloco). Medido
/// (`measure_times_of_the_filters`, 14 threads) 128/16 foi ~20% mais rapido que 64/8, e
/// 128/32 ou 64/16 (so 4 faixas por bloco) bem mais lentos; com o desfoque que le as
/// linhas vizinhas direto do buffer, 128/8 ficou dentro do ruido de 128/16
/// (`measure_step_of_the_drag`).
const STRIP: usize = 16;

/// Percorre `buf` (linhas de `line` bytes) em blocos de [`BLOCK`] linhas, consultando
/// `stop` antes de cada bloco (sempre na thread chamadora: `stop` nao precisa ser
/// `Sync`). Dentro do bloco, faixas de [`STRIP`] linhas rodam em paralelo com rayon;
/// `f(y0, faixa)` recebe a primeira linha da faixa e seus bytes.
fn blocks_by(
    buf: &mut [u8],
    line: usize,
    stop: &dyn Fn() -> bool,
    f: impl Fn(usize, &mut [u8]) + Sync + Send,
) -> Option<()> {
    for (b, block) in buf.chunks_mut(line * BLOCK).enumerate() {
        if stop() {
            return None;
        }
        block
            .par_chunks_mut(line * STRIP)
            .enumerate()
            .for_each(|(i, strip)| f(b * BLOCK + i * STRIP, strip));
    }
    Some(())
}

/// Aplica `f` a cada canal de cor (R, G, B) de cada pixel, sem tocar no alfa.
fn map_rgb(img: &mut RgbaImage, stop: &dyn Fn() -> bool, f: impl Fn(f32) -> f32 + Sync + Send) -> Option<()> {
    let line = img.width() as usize * 4;
    blocks_by(img, line, stop, |_, strip| {
        for p in strip.chunks_exact_mut(4) {
            for c in &mut p[..3] {
                *c = to_u8(f(*c as f32));
            }
        }
    })
}

/// Aplica `f(r, g, b)` a cada pixel, gravando o valor devolvido em R=G=B.
fn map_luma(img: &mut RgbaImage, stop: &dyn Fn() -> bool, f: impl Fn(u8, u8, u8) -> u8 + Sync + Send) -> Option<()> {
    let line = img.width() as usize * 4;
    blocks_by(img, line, stop, |_, strip| {
        for p in strip.chunks_exact_mut(4) {
            let v = f(p[0], p[1], p[2]);
            p[..3].fill(v);
        }
    })
}

/// Mesmo nucleo que `imageproc::filter::gaussian_blur_f32` usa (a funcao dele e privada).
/// A igualdade com o blur do imageproc e garantida pelos testes contra `apply_reference`.
/// Com raio `ceil(2·sigma)`, no minimo 1 (o imageproc nao aceita sigma <= 0; aqui
/// [`scale_sigma`] garante sigma positivo, e o minimo so protege o nucleo).
fn gaussian_kernel_f32(sigma: f32) -> Vec<f32> {
    fn gaussian(x: f32, r: f32) -> f32 {
        ((2.0 * std::f32::consts::PI).sqrt() * r).recip() * (-x.powi(2) / (2.0 * r.powi(2))).exp()
    }
    let radius = ((2.0 * sigma).ceil() as usize).max(1);
    let mut k = vec![0.0; 2 * radius + 1];
    for i in 0..radius + 1 {
        let v = gaussian(i as f32, sigma);
        k[radius + i] = v;
        k[radius - i] = v;
    }
    let sum: f32 = k.iter().sum();
    k.iter_mut().for_each(|x| *x /= sum);
    k
}

/// Combina `img` com seu desfoque gaussiano: `f(original, borrado)` por canal de cor.
///
/// Granularidade do cancelamento: o `gaussian_blur_f32` do imageproc nao e
/// interrompivel (e nem usa rayon, mesmo com a feature ligada). Por isso o blur e
/// refeito aqui por faixas horizontais, com a mesma aritmetica f32 e o mesmo
/// truncamento intermediario para u8 que o `horizontal_filter`/`vertical_filter` do
/// imageproc (cada pixel soma `acc + p·k` do primeiro peso ao ultimo, nessa ordem):
/// - passada horizontal: cada linha so depende dela mesma ([`filter_line`], com o
///   peso no laco de fora: o miolo da linha e um laco continuo que o compilador
///   vetoriza);
/// - passada vertical: cada linha de saida soma as `2·raio + 1` linhas da passada
///   horizontal em volta dela, grampeadas na imagem inteira ("clamp to edge" do
///   imageproc), lidas direto do buffer (sem copiar faixas com linhas extras).
///
/// Medido (`measure_step_of_the_drag`, tile 2560x1600 com nitidez e passa-alta, 14
/// threads): ~60 ms com as funcoes do imageproc por faixa, ~22 ms assim; a pagina
/// 6000x4000 inteira, ~260 ms -> ~100 ms.
///
/// O resultado e identico byte a byte ao blur original (garantido pelos testes), e
/// `stop` e consultado a cada [`BLOCK`] linhas em cada passada. A mistura com a
/// imagem acontece na propria passada vertical, sem uma terceira imagem inteira.
///
/// `sigma` e o desvio do desfoque (ja escalado); o raio das linhas extras segue o
/// nucleo (`nucleo.len() / 2`), entao vale para qualquer escala.
fn blend_with_blur(
    img: &mut RgbaImage,
    sigma: f32,
    stop: &dyn Fn() -> bool,
    f: impl Fn(f32, f32) -> f32 + Sync + Send,
) -> Option<()> {
    let (w, h) = img.dimensions();
    let (width, height) = (w as usize, h as usize);
    let line = width * 4;
    let marginalia_core = gaussian_kernel_f32(sigma);
    let radius = marginalia_core.len() / 2;

    // Passada horizontal: `img` -> `horiz` (u8, como o `horizontal_filter`).
    let mut horiz = vec![0u8; line * height];
    {
        let source: &[u8] = img;
        blocks_by(&mut horiz, line, stop, |y0, destination| {
            let (mut row, mut acc) = (vec![0f32; line], vec![0f32; line]);
            for (dy, output) in destination.chunks_exact_mut(line).enumerate() {
                let y = y0 + dy;
                row.iter_mut().zip(&source[y * line..(y + 1) * line]).for_each(|(l, &p)| *l = p as f32);
                filter_line(&row, &marginalia_core, radius, &mut acc);
                output.iter_mut().zip(&acc).for_each(|(s, &a)| *s = truncate_u_8(a));
            }
        })?;
    }

    // Passada vertical sobre `horiz` (u8, como o `vertical_filter`, com as linhas
    // grampeadas na imagem inteira), misturando direto em `img`.
    blocks_by(img, line, stop, |y0, destination| {
        let mut acc = vec![0f32; line];
        for (dy, output) in destination.chunks_exact_mut(line).enumerate() {
            let y = (y0 + dy) as isize;
            acc.fill(0.0);
            for (i, &k) in marginalia_core.iter().enumerate() {
                let yy = (y + i as isize - radius as isize).clamp(0, height as isize - 1) as usize;
                let source = &horiz[yy * line..(yy + 1) * line];
                acc.iter_mut().zip(source).for_each(|(a, &p)| *a = *a + p as f32 * k);
            }
            for (p, b) in output.chunks_exact_mut(4).zip(acc.chunks_exact(4)) {
                for i in 0..3 {
                    p[i] = to_u8(f(p[i] as f32, truncate_u_8(b[i]) as f32));
                }
            }
        }
    })
}

/// O `Clamp<f32> for u8` do imageproc: trunca (nao arredonda), grampeado em 0..=255.
fn truncate_u_8(x: f32) -> u8 {
    if x < 255.0 {
        if x > 0.0 {
            x as u8
        } else {
            0
        }
    } else {
        255
    }
}

/// Correlacao horizontal de uma linha RGBA em f32 (`row`) com `marginalia_core` (raio
/// `radius`), grampeada na borda da linha, em `acc`: a mesma soma do
/// `horizontal_filter` do imageproc, termo a termo e na mesma ordem (`acc + p·k`,
/// do primeiro peso ao ultimo), so que com o peso por fora (cada termo cai no
/// mesmo acumulador, na mesma ordem; o miolo vira um laco continuo).
fn filter_line(row: &[f32], marginalia_core: &[f32], radius: usize, acc: &mut [f32]) {
    let width = row.len() / 4;
    acc.fill(0.0);
    if width == 0 {
        return;
    }
    let add = |acc: &mut [f32], source: &[f32], k: f32| acc.iter_mut().zip(source).for_each(|(a, &p)| *a = *a + p * k);
    let (first, last) = (&row[..4], &row[(width - 1) * 4..]);
    for (i, &k) in marginalia_core.iter().enumerate() {
        let d = i as isize - radius as isize;
        // x + d dentro da linha para x em [lo, hi); antes, a 1a coluna; depois, a ultima.
        let lo = ((-d).max(0) as usize).min(width);
        let hi = ((width as isize - d).clamp(lo as isize, width as isize)) as usize;
        for x in 0..lo {
            add(&mut acc[x * 4..x * 4 + 4], first, k);
        }
        if hi > lo {
            let ini = (lo as isize + d) as usize;
            add(&mut acc[lo * 4..hi * 4], &row[ini * 4..(ini + hi - lo) * 4], k);
        }
        for x in hi..width {
            add(&mut acc[x * 4..x * 4 + 4], last, k);
        }
    }
}

/// Devolve uma copia da imagem com os ajustes aplicados; a entrada nunca e alterada
/// e o canal alfa nunca muda. Com ajustes neutros o resultado e identico a entrada.
/// Invólucro de [`apply_cancellable`] que nunca para.
///
/// Ordem fixa: grayscale, brightness/contrast, sharpen, highpass, invert, threshold.
///
/// - `grayscale`: luma Rec.601 em R=G=B.
/// - `brightness` (-1.0..=1.0): somado a cada canal como fracao de 255, com saturacao.
/// - `contrast` (0.0..=4.0): multiplicador em torno do cinza medio 128 (1.0 = neutro).
/// - `sharpen` (0.0..=1.0): quantidade de mascara de nitidez (`c + k * (c - borrado)`).
/// - `highpass` (0.0..=1.0): mistura do passa-alta (`c - borrado + 128`) sobre a imagem,
///   para realcar tinta fraca; 1.0 mostra apenas o passa-alta.
/// - `invert`: `255 - c`.
/// - `threshold: Some(t)`: luma >= t vira branco (255), senao preto (0), com R=G=B.
///
/// Parametros fora do intervalo sao limitados e NaN vale o valor neutro; nunca ha panico.
pub fn apply(img: &RgbaImage, a: &Adjustments) -> RgbaImage {
    apply_cancellable(img, a, &|| false).expect("parar nunca e verdadeiro")
}

/// Como [`apply`], mas interrompivel: `stop` e consultado na thread chamadora antes
/// de cada bloco de linhas de cada etapa (inclusive dentro dos desfoques). Quando
/// `stop` devolve `true` o trabalho e abandonado e o resultado e `None` (nunca uma
/// imagem filtrada pela metade). Com ajustes neutros ou imagem vazia devolve a copia
/// sem consultar `stop`. Sem parada, o resultado e identico byte a byte a [`apply`].
pub fn apply_cancellable(img: &RgbaImage, a: &Adjustments, stop: &dyn Fn() -> bool) -> Option<RgbaImage> {
    apply_cancellable_in(img.clone(), a, stop)
}

/// Como [`apply_cancellable`], mas filtra sobre o buffer recebido (sem copia inicial).
/// Em caso de parada o buffer e descartado. Invólucro de [`apply_cancellable_in_scale`]
/// com escala 1,0.
pub fn apply_cancellable_in(out: RgbaImage, a: &Adjustments, stop: &dyn Fn() -> bool) -> Option<RgbaImage> {
    apply_cancellable_in_scale(out, a, 1.0, stop)
}

/// Escala saneada, a mesma para o filtro e para o recorte ([`crate::crop`]): a
/// reamostragem nunca amplia a fonte, entao acima de 1 vale 1; NaN, infinito ou `<= 0`
/// valem 1 (sem escala).
pub fn sanitized_scale(scale: f32) -> f32 {
    if scale.is_finite() && scale > 0.0 {
        scale.min(1.0)
    } else {
        1.0
    }
}

/// Menor sigma usado: abaixo disso `sigma²` chega perto do subfluxo de f32 e o nucleo
/// viraria NaN; com 0,01 o nucleo ja e praticamente `[0, 1, 0]`.
const SIGMA_MIN: f32 = 0.01;

/// Sigma do desfoque para uma imagem reamostrada por `scale` (sempre >= [`SIGMA_MIN`]).
fn scale_sigma(scale: f32) -> f32 {
    (BLUR_SIGMA * sanitized_scale(scale)).max(SIGMA_MIN)
}

/// Raio (em pixels) do nucleo do desfoque na escala dada: `ceil(2·sigma)`, no minimo 1.
pub fn marginalia_core_radius(scale: f32) -> usize {
    gaussian_kernel_f32(scale_sigma(scale)).len() / 2
}

/// Como [`apply_cancellable_in`], para uma imagem reamostrada por `scale` em relacao a
/// fonte (p.ex. o recorte da regiao visivel na resolucao da tela): o desfoque de
/// `sharpen`/`highpass` usa sigma `BLUR_SIGMA * escala`, para o realce ter o mesmo
/// alcance relativo a pagina em qualquer zoom. O raio do nucleo e `ceil(2·sigma)`, no
/// minimo 1. A escala passa por [`sanitized_scale`] (acima de 1, NaN, infinita ou `<= 0`
/// valem 1,0); com escala 1,0 o resultado e identico byte a byte a [`apply`]. Para um
/// tile de [`crate::crop::grid_crop`], use [`crate::crop::grid_scale`].
pub fn apply_cancellable_in_scale(
    mut out: RgbaImage,
    a: &Adjustments,
    scale: f32,
    stop: &dyn Fn() -> bool,
) -> Option<RgbaImage> {
    if out.width() == 0 || out.height() == 0 || a.is_neutral() {
        return Some(out);
    }
    let c = a.clean();
    let sigma = scale_sigma(scale);

    if a.grayscale {
        map_luma(&mut out, stop, |r, g, b| to_u8(luma(r, g, b)))?;
    }
    if c.brightness != 0.0 || c.contrast != 1.0 {
        let shift = c.brightness * 255.0;
        map_rgb(&mut out, stop, |v| (v - 128.0) * c.contrast + 128.0 + shift)?;
    }
    if c.sharpen != 0.0 {
        blend_with_blur(&mut out, sigma, stop, |v, b| v + c.sharpen * (v - b))?;
    }
    if c.highpass != 0.0 {
        blend_with_blur(&mut out, sigma, stop, |v, b| {
            let high = (v - b + 128.0).clamp(0.0, 255.0);
            v * (1.0 - c.highpass) + high * c.highpass
        })?;
    }
    if a.invert {
        map_rgb(&mut out, stop, |v| 255.0 - v)?;
    }
    if let Some(t) = a.threshold {
        map_luma(&mut out, stop, |r, g, b| if luma(r, g, b) >= t as f32 { 255 } else { 0 })?;
    }
    Some(out)
}

#[cfg(test)]
#[path = "tests/filters.rs"]
mod tests;
