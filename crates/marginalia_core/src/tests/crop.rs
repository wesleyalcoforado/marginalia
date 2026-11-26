use super::*;
use crate::filters::{apply_cancellable_in_scale, Adjustments};
use image::Rgba;

/// Imagem pseudoaleatoria deterministica (LCG), com alfa variado.
fn noisy(w: u32, h: u32) -> RgbaImage {
    let mut s: u32 = 0x9e37_79b9 ^ (w * 17 + h);
    let mut next = move || {
        s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (s >> 24) as u8
    };
    RgbaImage::from_fn(w, h, |_, _| Rgba([next(), next(), next(), next()]))
}

fn record(x0: u32, y0: u32, x1: u32, y1: u32) -> RegionPx {
    RegionPx { x0, y0, x1, y1 }
}

fn rg(x0: u32, y0: u32, x1: u32, y1: u32) -> GridRegion {
    GridRegion { x0, y0, x1, y1 }
}

fn crop(img: &RgbaImage, x0: u32, y0: u32, x1: u32, y1: u32) -> RgbaImage {
    imageops::crop_imm(img, x0, y0, x1 - x0, y1 - y0).to_image()
}

fn filters() -> Vec<(&'static str, Adjustments)> {
    let d = Adjustments::default();
    vec![
        ("nitidez", Adjustments { sharpen: 0.8, ..d.clone() }),
        ("passa-alta", Adjustments { highpass: 0.7, ..d.clone() }),
        ("ambos", Adjustments { sharpen: 0.6, highpass: 0.5, contrast: 1.3, ..d.clone() }),
    ]
}

const MARGIN: u32 = 8;

/// Pagina de lados impares, para a grade nunca cair alinhada por acaso.
const PAGE: (u32, u32) = (203, 157);
const SCALES: [f32; 4] = [1.0, 0.9, 0.5, 0.3];

fn grid_of(e: f32) -> (u32, u32) {
    ((PAGE.0 as f32 * e).round() as u32, (PAGE.1 as f32 * e).round() as u32)
}

/// A pagina inteira reamostrada na grade (a referencia de todos os testes).
fn resampled(page: &RgbaImage, grid: (u32, u32)) -> RgbaImage {
    imageops::resize(page, grid.0, grid.1, imageops::FilterType::Triangle)
}

/// Retangulos da grade: alinhados e desalinhados, colados em cada borda, finos.
fn rectangles((gw, gh): (u32, u32)) -> Vec<GridRegion> {
    vec![
        rg(gw / 4, gh / 4, gw * 3 / 4, gh * 3 / 4),       // meio
        rg(gw / 4 + 1, gh / 5 + 1, gw * 2 / 3 + 1, gh / 2), // meio desalinhado (impar)
        rg(0, gh / 3, gw / 2 + 1, gh * 2 / 3),             // esquerda
        rg(gw / 3, gh / 3, gw, gh * 2 / 3 + 1),            // direita
        rg(gw / 3 + 1, 0, gw * 2 / 3, gh / 2 + 1),         // topo
        rg(gw / 3, gh / 2 + 1, gw * 2 / 3 + 1, gh),        // baixo
        rg(0, 0, gw, gh),                                  // grade inteira
        rg(gw / 2 + 1, 3, gw / 2 + 2, gh - 3),             // 1 px de largura
        rg(5, gh / 2 + 1, gw - 7, gh / 2 + 2),             // 1 px de altura
        rg(gw - 1, gh - 1, gw, gh),                        // 1x1 no canto
        rg(7, 9, 8, 10),                                   // 1x1
    ]
}

#[test]
fn crop_and_the_rectangle_of_the_page_whole_resampled() {
    let page = noisy(PAGE.0, PAGE.1);
    for e in SCALES {
        let grid = grid_of(e);
        let full = resampled(&page, grid);
        for r in rectangles(grid) {
            let rec = grid_crop(&page, grid, r, MARGIN);
            assert_eq!(rec.region, r, "{e} {r:?}");
            assert_eq!(rec.grid, grid);
            let (me, mt, md, mb) = rec.margin;
            assert!(rec.img == crop(&full, r.x0 - me, r.y0 - mt, r.x1 + md, r.y1 + mb), "img {e} {r:?}");
            let core = trim_margin(rec.img, rec.margin);
            assert_eq!(core.dimensions(), (r.x1 - r.x0, r.y1 - r.y0), "{e} {r:?}");
            assert!(core == crop(&full, r.x0, r.y0, r.x1, r.y1), "miolo {e} {r:?}");
        }
    }
}

#[test]
fn margin_and_the_requested_more_the_support_and_clamped_in_the_grid() {
    let page = noisy(PAGE.0, PAGE.1);
    // Sem reamostrar: so a margem pedida.
    let rec = grid_crop(&page, PAGE, rg(3, 20, 200, 60), MARGIN);
    assert_eq!(rec.margin, (3, 8, 3, 8));
    assert!(rec.img == crop(&page, 0, 12, 203, 68));
    // Reamostrando: margem + 1 de suporte, grampeada.
    let grid = grid_of(0.5);
    let rec = grid_crop(&page, grid, rg(20, 20, 60, 50), MARGIN);
    assert_eq!(rec.margin, (9, 9, 9, 9));
    assert_eq!(rec.img.dimensions(), (40 + 18, 30 + 18));
    let rec = grid_crop(&page, grid, rg(0, 4, grid.0, 50), MARGIN);
    assert_eq!(rec.margin, (0, 4, 0, 9));
}

#[test]
fn tile_filtered_has_core_identical_the_page_resampled_filtered() {
    // Diferenca documentada: zero. A amostragem segue a grade global e a margem
    // (8 + 1) cobre os dois desfoques (2 · raio <= 8 nas escalas testadas).
    let page = noisy(PAGE.0, PAGE.1);
    for e in [1.0f32, 0.5, 0.3] {
        let grid = grid_of(e);
        let scale = grid_scale(PAGE, grid);
        assert!(2 * crate::filters::marginalia_core_radius(scale) as u32 <= MARGIN);
        let full = resampled(&page, grid);
        for (name, a) in filters() {
            let filtered = apply_cancellable_in_scale(full.clone(), &a, scale, &|| false).unwrap();
            for r in rectangles(grid) {
                let rec = grid_crop(&page, grid, r, MARGIN);
                let tile = apply_cancellable_in_scale(rec.img, &a, scale, &|| false).unwrap();
                let core = trim_margin(tile, rec.margin);
                assert!(core == crop(&filtered, r.x0, r.y0, r.x1, r.y1), "{name} {e} {r:?}");
            }
        }
    }
}

#[test]
fn scale_of_the_grid_and_the_media_of_the_axes_sanitized() {
    assert_eq!(grid_scale((200, 100), (100, 50)), 0.5);
    assert_eq!(grid_scale((200, 100), (100, 25)), 0.375);
    assert_eq!(grid_scale((200, 100), (200, 100)), 1.0);
    assert_eq!(grid_scale((200, 100), (400, 300)), 1.0, "nunca amplia");
    assert_eq!(grid_scale((200, 100), (0, 50)), 1.0);
    assert_eq!(grid_scale((0, 0), (10, 10)), 1.0);
}

#[test]
fn rectangles_degenerate_not_enter_in_panic() {
    let page = noisy(50, 40);
    for grid in [(50, 40), (25, 20), (1, 1)] {
        for r in [rg(10, 10, 10, 20), rg(10, 10, 20, 10), rg(30, 30, 10, 10), rg(60, 0, 80, 10), rg(0, 45, 10, 90)] {
            let rec = grid_crop(&page, grid, r, MARGIN);
            assert_eq!(rec.img.dimensions(), (0, 0), "{r:?} {grid:?}");
            assert_eq!(rec.margin, (0, 0, 0, 0));
            assert_eq!(rec.region, GridRegion::default());
        }
    }
    // Grade maior que a pagina e grampeada; grade vazia nao tem pixels.
    let rec = grid_crop(&page, (500, 400), rg(40, 30, 900, 900), MARGIN);
    assert_eq!((rec.grid, rec.region), ((50, 40), rg(40, 30, 50, 40)));
    assert!(trim_margin(rec.img, rec.margin) == crop(&page, 40, 30, 50, 40));
    assert_eq!(grid_crop(&page, (0, 40), rg(0, 0, 5, 5), MARGIN).img.dimensions(), (0, 0));
    assert_eq!(grid_crop(&RgbaImage::new(0, 0), (5, 5), rg(0, 0, 5, 5), MARGIN).img.dimensions(), (0, 0));
    // Margem enorme nao estoura.
    let rec = grid_crop(&page, (25, 20), rg(5, 5, 6, 6), u32::MAX);
    assert_eq!(rec.margin, (5, 5, 19, 14));
}

#[test]
fn crop_in_the_scale_uses_the_grid_derived() {
    let page = noisy(PAGE.0, PAGE.1);
    let r = record(41, 30, 150, 101);
    for e in SCALES {
        let grid = grid_of(e);
        let rec = scale_crop(&page, r, e, MARGIN);
        assert_eq!(rec.grid, grid, "{e}");
        let in_grid = |v: u32, side: u32, g: u32| ((v as f64 * g as f64 / side as f64).round()) as u32;
        let expected = rg(
            in_grid(r.x0, PAGE.0, grid.0),
            in_grid(r.y0, PAGE.1, grid.1),
            in_grid(r.x1, PAGE.0, grid.0),
            in_grid(r.y1, PAGE.1, grid.1),
        );
        assert_eq!(rec.region, expected, "{e}");
        assert!(rec == grid_crop(&page, grid, expected, MARGIN), "{e}");
    }
    // Escala 1: sem reamostrar, margem grampeada.
    let rec = scale_crop(&page, record(3, 10, 200, 30), 1.0, MARGIN);
    assert_eq!(rec.margin, (3, 8, 3, 8));
    assert!(rec.img == crop(&page, 0, 2, 203, 38));
}

#[test]
fn scale_small_not_enters_in_panic_and_region_not_some() {
    let page = noisy(300, 200);
    let a = Adjustments { sharpen: 1.0, highpass: 1.0, ..Default::default() };
    for (r, dims) in [(record(0, 0, 300, 200), (15, 10)), (record(100, 50, 140, 90), (2, 2)), (record(10, 10, 11, 11), (1, 1)), (record(299, 199, 300, 200), (1, 1))] {
        let rec = scale_crop(&page, r, 0.05, MARGIN);
        assert_eq!(rec.grid, (15, 10));
        let tile = apply_cancellable_in_scale(rec.img, &a, 0.05, &|| false).unwrap();
        assert_eq!(trim_margin(tile, rec.margin).dimensions(), dims, "{r:?}");
    }
}

#[test]
fn regions_degenerate_in_the_scale_not_enter_in_panic() {
    let page = noisy(50, 40);
    for scale in [1.0f32, 0.5, 0.05] {
        for r in [record(10, 10, 10, 20), record(10, 10, 20, 10), record(30, 30, 10, 10), record(60, 0, 80, 10), record(0, 45, 10, 90)] {
            let rec = scale_crop(&page, r, scale, MARGIN);
            assert_eq!(rec.img.dimensions(), (0, 0), "{r:?} {scale}");
            assert_eq!(rec.margin, (0, 0, 0, 0));
        }
        assert_eq!(trim_margin_of(scale_crop(&page, record(7, 7, 8, 8), scale, MARGIN)), (1, 1));
    }
    for scale in [0.0f32, -1.0, f32::NAN, f32::INFINITY, 3.0] {
        let rec = scale_crop(&page, record(10, 10, 20, 20), scale, MARGIN);
        assert_eq!(rec.grid, (50, 40), "{scale} vale 1");
        assert_eq!(trim_margin_of(rec), (10, 10), "{scale} vale 1");
    }
    assert_eq!(scale_crop(&RgbaImage::new(0, 0), record(0, 0, 5, 5), 0.5, MARGIN).img.dimensions(), (0, 0));
}

fn trim_margin_of(rec: Crop) -> (u32, u32) {
    trim_margin(rec.img, rec.margin).dimensions()
}

#[test]
fn resampling_parallel_and_identical_to_the_resize_of_the_image() {
    let page = noisy(230, 310);
    for grid in [(230, 310), (115, 155), (77, 89), (230, 100), (60, 310), (1, 1), (229, 309)] {
        let full = resampled(&page, grid);
        let (gw, gh) = grid;
        for (xs, ys) in [(0..gw, 0..gh), (gw / 3..gw / 2 + 1, gh / 4..gh - 1), (gw - 1..gw, 0..1)] {
            let got = resample_triangle(&page, grid, xs.clone(), ys.clone());
            assert!(got == crop(&full, xs.start, ys.start, xs.end, ys.end), "{grid:?} {xs:?} {ys:?}");
        }
    }
}

#[test]
fn trim_margin_larger_that_the_image_returns_empty() {
    let img = noisy(10, 6);
    assert_eq!(trim_margin(img.clone(), (5, 0, 5, 0)).dimensions(), (0, 6));
    assert_eq!(trim_margin(img.clone(), (9, 9, 9, 9)).dimensions(), (0, 0));
    assert_eq!(trim_margin(img.clone(), (0, 0, 0, 0)), img);
    assert!(trim_margin(img.clone(), (2, 1, 3, 1)) == imageops::crop_imm(&img, 2, 1, 5, 4).to_image());
}

#[test]
fn adjustment_neutral_returns_the_crop_no_work() {
    let page = noisy(80, 60);
    let rec = scale_crop(&page, record(10, 10, 50, 40), 0.5, MARGIN);
    let calls = std::cell::Cell::new(0u32);
    let stop = || {
        calls.set(calls.get() + 1);
        true
    };
    let got = apply_cancellable_in_scale(rec.img.clone(), &Adjustments::default(), 0.5, &stop);
    assert_eq!(got, Some(rec.img));
    assert_eq!(calls.get(), 0);
}

#[test]
fn filter_with_scale_cancelled_returns_none() {
    let page = noisy(200, 300);
    let rec = scale_crop(&page, record(0, 0, 200, 300), 0.5, MARGIN);
    let a = Adjustments { sharpen: 1.0, ..Default::default() };
    assert!(apply_cancellable_in_scale(rec.img, &a, 0.5, &|| true).is_none());
}

/// Medicao (nao roda por padrao): `cargo test --release -p nucleo -- --ignored --nocapture medir_passo_do_arrasto`.
///
/// Um passo do arrasto de uma barra = um tile, pelas mesmas chamadas do app
/// (`interface/src/load.rs`, `cancellable_tile`): margem `2 · raio_do_nucleo`,
/// recorte na grade (sem cache: `grid_crop`; com cache: a copia do recorte
/// guardado), `apply_cancellable_in_scale` com `grid_scale`, `trim_margin` e a
/// troca RGBA -> BGRA. Janela de 2560x1600 pixels fisicos, em paginas de 6000x4000
/// e 12000x8000: a pagina inteira ajustada a janela e uma janela de 2560x1600 no
/// meio da pagina em escala 1 (zoom alto: o tile mais caro). Comparado com o filtro
/// antigo da pagina inteira em resolucao cheia (`apply_cancellable_in`). Mediana de 5.
#[test]
#[ignore]
fn measure_step_of_the_drag() {
    use crate::filters::{apply_cancellable_in, marginalia_core_radius};
    use std::time::{Duration, Instant};
    fn median(n: usize, mut f: impl FnMut()) -> Duration {
        let mut t: Vec<Duration> = (0..n)
            .map(|_| {
                let t0 = Instant::now();
                f();
                t0.elapsed()
            })
            .collect();
        t.sort();
        t[n / 2]
    }
    fn bgra(img: &mut RgbaImage) {
        for px in img.as_chunks_mut::<4>().0 {
            px.swap(0, 2);
        }
    }
    let ms = |d: Duration| d.as_secs_f64() * 1e3;
    let (screen_w, screen_h) = (2560u32, 1600u32);
    let d = Adjustments::default();
    let image_adjustments = [
        ("nitidez", Adjustments { sharpen: 0.6, ..d.clone() }),
        ("passa-alta", Adjustments { highpass: 0.7, ..d.clone() }),
        ("ambos", Adjustments { sharpen: 0.6, highpass: 0.7, ..d.clone() }),
    ];
    println!("medicao: threads rayon {}", rayon::current_num_threads());
    for (w, h) in [(6000u32, 4000u32), (12000, 8000)] {
        let page = noisy(w, h);
        // Ajustada a janela: a grade e a pagina reduzida para caber nela, a janela e a grade toda.
        let s = (screen_w as f64 / w as f64).min(screen_h as f64 / h as f64).min(1.0);
        let grid_aj = ((w as f64 * s).round() as u32, (h as f64 * s).round() as u32);
        let cases = [
            ("ajustada", grid_aj, rg(0, 0, grid_aj.0, grid_aj.1)),
            ("escala 1", (w, h), rg((w - screen_w) / 2, (h - screen_h) / 2, (w + screen_w) / 2, (h + screen_h) / 2)),
        ];
        for (name, a) in &image_adjustments {
            let old = median(3, || {
                apply_cancellable_in(page.clone(), a, &|| false).unwrap();
            });
            for (case, grid, region) in cases {
                let scale = grid_scale((w, h), grid);
                let margin = 2 * marginalia_core_radius(scale) as u32;
                let step = |rec: &Crop| {
                    let filtered = apply_cancellable_in_scale(rec.img.clone(), a, scale, &|| false).unwrap();
                    let mut out = trim_margin(filtered, rec.margin);
                    bgra(&mut out);
                    out
                };
                let stored = grid_crop(&page, grid, region, margin);
                let sz = step(&stored).dimensions();
                assert_eq!(sz, (region.x1 - region.x0, region.y1 - region.y0));
                let no_cache = median(5, || {
                    step(&grid_crop(&page, grid, region, margin));
                });
                let so_crop = median(5, || {
                    grid_crop(&page, grid, region, margin);
                });
                let with_cache = median(5, || {
                    step(&stored);
                });
                println!(
                    "medicao: {w}x{h} {name:<10} {case:<8} tile {sz:?} escala {scale:.3} | passo com cache {:>6.1} ms | sem cache {:>6.1} ms (recorte {:>5.1}) | antigo (pagina cheia) {:>7.1} ms",
                    ms(with_cache),
                    ms(no_cache),
                    ms(so_crop),
                    ms(old)
                );
            }
        }
    }
}

/// Medicao (nao roda por padrao): `cargo test --release -p nucleo -- --ignored --nocapture medir_recorte`.
/// Tempo do recorte reamostrado (sem filtro) e do filtro do tile, numa pagina
/// 6000x4000, para janelas tipicas de uma tela 1800x1200.
#[test]
#[ignore]
fn measure_crop() {
    use std::time::{Duration, Instant};
    fn best<T>(n: usize, mut f: impl FnMut() -> T) -> (Duration, T) {
        let (mut t, mut r) = (Duration::MAX, None);
        for _ in 0..n {
            let t0 = Instant::now();
            let v = f();
            t = t.min(t0.elapsed());
            r = Some(v);
        }
        (t, r.unwrap())
    }
    let ms = |d: Duration| d.as_secs_f64() * 1e3;
    let page = noisy(6000, 4000);
    let a = Adjustments { sharpen: 0.6, highpass: 0.7, ..Default::default() };
    let cases = [
        ("pagina inteira, escala 0.3", record(0, 0, 6000, 4000), 0.3f32),
        ("metade, escala 0.6", record(1500, 1000, 4500, 3000), 0.6),
        ("1800x1200, escala 1.0", record(2000, 1000, 3800, 2200), 1.0),
    ];
    for (name, r, e) in cases {
        let (t_rec, rec) = best(5, || scale_crop(&page, r, e, MARGIN));
        let dims = rec.img.dimensions();
        let (t_filter, _) = best(5, || apply_cancellable_in_scale(rec.img.clone(), &a, e, &|| false).unwrap());
        println!("medicao: {name:<28} recorte {:>6.1} ms -> {dims:?} | filtro {:>6.1} ms", ms(t_rec), ms(t_filter));
    }
}
