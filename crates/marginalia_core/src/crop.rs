//! Recorte de uma regiao da pagina, reamostrado para a resolucao da tela, com margem.
//!
//! A pagina reamostrada e pensada como uma *grade* global `Wout × Hout` (o tamanho que
//! a pagina inteira teria na escala da tela). Um recorte e um retangulo dessa grade,
//! amostrado com os mesmos centros da reamostragem da pagina inteira
//! (`(o_global + 0,5) · lado_pagina / lado_grade`, por eixo): o recorte sai identico byte
//! a byte a "reamostrar a pagina inteira e recortar", em qualquer posicao. Deslocar a
//! janela (pan) nunca muda a fase da amostragem, e o tile casa com uma base reamostrada
//! na mesma grade.
//!
//! O filtro com desfoque (nitidez, passa-alta) olha vizinhos, entao o recorte leva uma
//! margem de pixels da grade em volta do retangulo (grampeada na borda da grade, onde o
//! filtro da pagina inteira tambem grampeia); depois do filtro a margem sai com
//! [`trim_margin`] e o miolo tem exatamente o tamanho do retangulo. Com margem que
//! cobre o alcance do filtro (`2 · raio_do_nucleo` com nitidez e passa-alta juntas), o
//! miolo filtrado e identico ao mesmo retangulo da pagina reamostrada filtrada inteira.

use crate::filters::sanitized_scale;
use image::imageops;
use image::RgbaImage;
use std::ops::Range;

/// Regiao retangular da pagina em pixels da fonte, semiaberta: `[x0, x1) × [y0, y1)`.
/// Regiao vazia ou invertida (`x1 <= x0` ou `y1 <= y0`) nao tem pixels; a parte fora
/// da pagina e ignorada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RegionPx {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}

/// Retangulo semiaberto `[x0, x1) × [y0, y1)` em pixels da grade (pagina reamostrada).
/// Vazio ou invertido nao tem pixels; a parte fora da grade e ignorada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GridRegion {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}

/// Recorte reamostrado: o retangulo mais a margem, em pixels da grade.
#[derive(Debug, Clone, PartialEq)]
pub struct Crop {
    pub img: RgbaImage,
    /// Margem realmente aplicada, em pixels de `img`: (esquerda, topo, direita, baixo).
    /// Menor que a pedida quando o retangulo encosta na borda da grade.
    pub margin: (u32, u32, u32, u32),
    /// Retangulo da grade que o miolo cobre (o pedido, grampeado na grade); vazio
    /// (tudo zero) quando nao sobrou pixel.
    pub region: GridRegion,
    /// Tamanho da grade usada (o pedido, grampeado para nunca ampliar a pagina).
    pub grid: (u32, u32),
}

/// Pixels da grade somados a margem pedida quando ha reamostragem: o suporte do
/// Triangle em pixels de saida (`support 1,0 · sratio` px da fonte = 1 px da grade ao
/// reduzir; ao manter o lado, 1 px tambem). Com a amostragem na grade global o recorte
/// ja e exato sem ele; ele so garante que a margem pedida inteira seja de pixels cuja
/// janela de amostragem nao toca a borda do retangulo lido.
const OUTPUT_SUPPORT: u32 = 1;

/// Escala de sigma do desfoque para um tile na grade `grid` de uma pagina `page`:
/// a media das razoes `lado_grade / lado_pagina` dos dois eixos (iguais a menos de
/// arredondamento quando a grade vem de uma escala unica), passada por
/// [`sanitized_scale`]. Pagina ou grade vazia vale 1.
pub fn grid_scale(page: (u32, u32), grid: (u32, u32)) -> f32 {
    let ((w, h), (gw, gh)) = (page, grid);
    if w == 0 || h == 0 || gw == 0 || gh == 0 {
        return 1.0;
    }
    let media = (gw as f64 / w as f64 + gh as f64 / h as f64) / 2.0;
    sanitized_scale(media as f32)
}

impl Crop {
    fn empty(grid: (u32, u32)) -> Crop {
        Crop { img: RgbaImage::new(0, 0), margin: (0, 0, 0, 0), region: GridRegion::default(), grid }
    }
}

/// Recorta o retangulo `r` da grade `grid` da pagina (a pagina reamostrada para
/// `grid`; `grade == pagina.dimensions()` nao reamostra) com `margin_px` pixels de
/// margem da grade, mais [`OUTPUT_SUPPORT`] quando reamostra, grampeada na borda da
/// grade.
///
/// - `img` e identico byte a byte ao retangulo `[x0-esq, x1+dir) × [y0-topo, y1+baixo)`
///   de `imageops::resize(pagina, grade.0, grade.1, FilterType::Triangle)`; o miolo
///   depois de [`trim_margin`] tem exatamente `(x1-x0) × (y1-y0)` pixels.
/// - Lado da grade maior que o da pagina e grampeado (nunca amplia); lado 0 nao tem
///   pixels.
/// - Retangulo vazio, invertido ou fora da grade devolve imagem 0×0, sem margem e com
///   `region` vazia.
pub fn grid_crop(page: &RgbaImage, grid: (u32, u32), r: GridRegion, margin_px: u32) -> Crop {
    let (w, h) = page.dimensions();
    let grid = (grid.0.min(w), grid.1.min(h));
    let (gw, gh) = grid;
    let (x0, x1) = (r.x0.min(gw), r.x1.min(gw));
    let (y0, y1) = (r.y0.min(gh), r.y1.min(gh));
    if x1 <= x0 || y1 <= y0 {
        return Crop::empty(grid);
    }
    let resamples = grid != (w, h);
    let m = if resamples { margin_px.saturating_add(OUTPUT_SUPPORT) } else { margin_px };
    let margin = (m.min(x0), m.min(y0), m.min(gw - x1), m.min(gh - y1));
    let (left, top, dir, down) = margin;
    let (xs, ys) = (x0 - left..x1 + dir, y0 - top..y1 + down);
    let img = if resamples {
        resample_triangle(page, grid, xs, ys)
    } else {
        imageops::crop_imm(page, xs.start, ys.start, xs.len() as u32, ys.len() as u32).to_image()
    };
    Crop { img, margin, region: GridRegion { x0, y0, x1, y1 }, grid }
}

/// Leva o intervalo `[v0, v1)` da pagina (lado `side`) para a grade (lado `g`),
/// arredondando cada borda; um intervalo nao vazio continua com pelo menos 1 px.
fn grid_axis(v0: u32, v1: u32, side: u32, g: u32) -> (u32, u32) {
    let (v0, v1) = (v0.min(side), v1.min(side));
    if v1 <= v0 || g == 0 {
        return (0, 0);
    }
    let conv = |v: u32| ((v as f64 * g as f64 / side as f64).round() as u32).min(g);
    let (a, b) = (conv(v0), conv(v1));
    if b > a {
        (a, b)
    } else if a < g {
        (a, a + 1)
    } else {
        (g - 1, g)
    }
}

/// Invólucro de [`grid_crop`] para uma regiao em pixels da pagina e uma escala
/// unica: a grade e `(round(W·e), round(H·e))` (no minimo 1; `e` por
/// [`sanitized_scale`]) e cada borda da regiao vira `round(borda · lado_grade /
/// lado_pagina)`; uma regiao nao vazia nunca vira vazia (ganha 1 px da grade).
pub fn scale_crop(page: &RgbaImage, r: RegionPx, scale: f32, margin_px: u32) -> Crop {
    let (w, h) = page.dimensions();
    let e = sanitized_scale(scale) as f64;
    let side = |l: u32| if e == 1.0 { l } else { ((l as f64 * e).round() as u32).max(1).min(l) };
    let grid = (side(w), side(h));
    let (x0, x1) = grid_axis(r.x0, r.x1, w, grid.0);
    let (y0, y1) = grid_axis(r.y0, r.y1, h, grid.1);
    grid_crop(page, grid, GridRegion { x0, y0, x1, y1 }, margin_px)
}

/// Tira a margem (esquerda, topo, direita, baixo) de `img`. Margem maior que a imagem
/// devolve imagem vazia no eixo correspondente; margem zero devolve `img` sem copia.
pub fn trim_margin(img: RgbaImage, margin: (u32, u32, u32, u32)) -> RgbaImage {
    if margin == (0, 0, 0, 0) {
        return img;
    }
    let (w, h) = img.dimensions();
    let (left, top, dir, down) = margin;
    let x0 = left.min(w);
    let x1 = w.saturating_sub(dir).max(x0);
    let y0 = top.min(h);
    let y1 = h.saturating_sub(down).max(y0);
    imageops::crop_imm(&img, x0, y0, x1 - x0, y1 - y0).to_image()
}

/// Pesos de uma passada do Triangle do `image` 0.25.5 (`imageops/sample.rs`), na mesma
/// aritmetica f32, para os indices de saida `strip` de uma reamostragem `fonte ->
/// destino`: por indice, o primeiro indice da fonte e os pesos normalizados. A razao e a
/// da pagina inteira (`fonte / destino`), entao os centros seguem a grade global.
fn weights_triangle(source: u32, destination: u32, strip: Range<u32>) -> Vec<(usize, Vec<f32>)> {
    let ratio = source as f32 / destination as f32;
    let sratio = if ratio < 1.0 { 1.0 } else { ratio };
    let support = sratio; // support 1.0 do Triangle
    strip
        .map(|o| {
            let center = (o as f32 + 0.5) * ratio;
            let left = ((center - support).floor() as i64).clamp(0, source as i64 - 1);
            let dir = ((center + support).ceil() as i64).clamp(left + 1, source as i64);
            let center = center - 0.5;
            let mut ws: Vec<f32> = (left..dir)
                .map(|i| {
                    let x = (i as f32 - center) / sratio;
                    if x.abs() < 1.0 {
                        1.0 - x.abs()
                    } else {
                        0.0
                    }
                })
                .collect();
            let sum: f32 = ws.iter().fold(0.0, |s, w| s + w);
            ws.iter_mut().for_each(|w| *w /= sum);
            (left as usize, ws)
        })
        .collect()
}

/// O retangulo `xs × ys` de `imageops::resize(fonte, grade.0, grade.1,
/// FilterType::Triangle)`, calculado so para esse retangulo e em paralelo por linha de
/// saida (rayon).
///
/// Mesmo algoritmo, mesma razao global e mesma ordem das somas em f32 que o `image`
/// (passada vertical sem arredondar, depois horizontal com arredondamento para u8),
/// entao o resultado e identico byte a byte (garantido por teste). **Acoplamento:** o
/// algoritmo foi copiado do `image` 0.25.5 (fixado no Cargo.lock); se o `image` mudar o
/// `resize`, o teste `resampling_parallel_and_identical_to_the_resize_of_the_image` acusa e a
/// copia aqui continua valendo por si (so deixa de bater com o `image`).
///
/// Memoria: cada linha de saida faz a passada vertical so das colunas da fonte que as
/// colunas de `xs` leem, num buffer f32 reaproveitado por thread
/// (`4 · 4 · colunas_lidas` bytes, ~96 KB para 6000 colunas), e grava direto na saida;
/// nao ha imagem intermediaria inteira (o `resize` do `image` aloca `16 · W · Hout`
/// bytes, ~115 MB para 6000 colunas e 1200 linhas). O pico e a saida mais um buffer
/// desses por thread. Medido (`measure_crop`, pagina 6000x4000 inteira para
/// 1800x1200, 14 threads): ~80 ms com o `resize` serial do `image` + copia do recorte,
/// ~6 ms aqui (o filtro do tile resultante leva ~20 ms).
fn resample_triangle(source: &RgbaImage, grid: (u32, u32), xs: Range<u32>, ys: Range<u32>) -> RgbaImage {
    use rayon::prelude::*;

    let (w, h) = source.dimensions();
    let (tw, th) = (xs.len() as u32, ys.len() as u32);
    if w == 0 || h == 0 || grid.0 == 0 || grid.1 == 0 || tw == 0 || th == 0 {
        return RgbaImage::new(tw, th);
    }
    let ph = weights_triangle(w, grid.0, xs);
    let pv = weights_triangle(h, grid.1, ys);
    // Colunas da fonte lidas pela passada horizontal (os pesos sao monotonos).
    let c0 = ph.iter().map(|(e, _)| *e).min().unwrap_or(0);
    let c1 = ph.iter().map(|(e, ws)| e + ws.len()).max().unwrap_or(c0);
    let step = w as usize * 4;
    let src: &[u8] = source;

    let mut out = vec![0u8; tw as usize * th as usize * 4];
    out.par_chunks_mut(tw as usize * 4).zip(pv.par_iter()).for_each_init(
        || vec![0f32; (c1 - c0) * 4],
        |middle, (destination, (left, ws))| {
            // Passada vertical desta linha de saida: fonte (u8) -> `middle` (f32).
            for x in c0..c1 {
                let mut t = [0f32; 4];
                for (i, weight) in ws.iter().enumerate() {
                    let p = &src[(left + i) * step + x * 4..][..4];
                    for c in 0..4 {
                        t[c] += p[c] as f32 * weight;
                    }
                }
                middle[(x - c0) * 4..][..4].copy_from_slice(&t);
            }
            // Passada horizontal: `middle` -> linha de saida (u8, arredondado como o `image`).
            for (ox, (e, ws)) in ph.iter().enumerate() {
                let mut t = [0f32; 4];
                for (i, weight) in ws.iter().enumerate() {
                    let p = &middle[(e + i - c0) * 4..][..4];
                    for c in 0..4 {
                        t[c] += p[c] * weight;
                    }
                }
                for c in 0..4 {
                    destination[ox * 4 + c] = t[c].clamp(0.0, 255.0).round() as u8;
                }
            }
        },
    );
    RgbaImage::from_raw(tw, th, out).expect("tamanho exato")
}

#[cfg(test)]
#[path = "tests/crop.rs"]
mod tests;
