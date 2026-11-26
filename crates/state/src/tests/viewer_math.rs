use super::*;
use crate::load::GridRegion;

const EPS: f32 = 1e-4;

fn fr(zoom: f32, cx: f32, cy: f32) -> Framing {
    Framing { zoom, cx, cy }
}

fn near(a: (f32, f32), b: (f32, f32), eps: f32) -> bool {
    (a.0 - b.0).abs() <= eps && (a.1 - b.1).abs() <= eps
}

/// Imagem larga numa janela alta e imagem alta numa janela larga.
const CASES: [(Size, Size); 2] = [((600.0, 1000.0), (3000.0, 1000.0)), ((1200.0, 800.0), (900.0, 2400.0))];

#[test]
fn center_of_the_area_maps_to_center_of_the_image() {
    for (area, img) in CASES {
        let p = screen_to_image(&default_framing(), area, img, (area.0 / 2.0, area.1 / 2.0));
        assert!(near(p, (0.5, 0.5), EPS), "{p:?}");
        let s = image_to_screen(&default_framing(), area, img, (0.5, 0.5));
        assert!(near(s, (area.0 / 2.0, area.1 / 2.0), EPS), "{s:?}");
    }
}

#[test]
fn zoom_1_adjusts_the_image_whole() {
    // imagem larga 3000x1000 em area 600x1000: escala 0.2, imagem ocupa 600x200
    let (area, img) = CASES[0];
    assert!((fit_scale(area, img) - 0.2).abs() < 1e-6);
    let a = image_to_screen(&default_framing(), area, img, (0.0, 0.0));
    let b = image_to_screen(&default_framing(), area, img, (1.0, 1.0));
    assert!(near(a, (0.0, 400.0), 1e-3), "{a:?}");
    assert!(near(b, (600.0, 600.0), 1e-3), "{b:?}");
}

#[test]
fn forward_and_back_between_screen_and_image() {
    let centers = [(0.5, 0.5), (0.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.25, 0.8)];
    let points = [(0.0, 0.0), (1.0, 1.0), (0.3, 0.7), (0.5, 0.5), (0.9, 0.1)];
    for (area, img) in CASES {
        for zoom in [1.0, 2.5, 8.0] {
            for (cx, cy) in centers {
                let e = fr(zoom, cx, cy);
                for p in points {
                    let s = image_to_screen(&e, area, img, p);
                    let q = screen_to_image(&e, area, img, s);
                    assert!(near(p, q, EPS), "zoom {zoom} centro {cx},{cy}: {p:?} -> {s:?} -> {q:?}");
                }
            }
        }
    }
}

#[test]
fn zoom_limited_between_1_and_32() {
    let (area, img) = CASES[1];
    let middle = (area.0 / 2.0, area.1 / 2.0);
    let z = zoom_at(&fr(20.0, 0.5, 0.5), area, img, 100.0, middle);
    assert_eq!(z.zoom, MAX_ZOOM);
    let z = zoom_at(&fr(20.0, 0.5, 0.5), area, img, 0.01, middle);
    assert_eq!(z.zoom, MIN_ZOOM);
    assert_eq!(clamp(fr(0.2, 0.5, 0.5)).zoom, MIN_ZOOM);
    assert_eq!(clamp(fr(99.0, 0.5, 0.5)).zoom, MAX_ZOOM);
}

#[test]
fn zoom_keeps_the_point_anchored_fixed() {
    for (area, img) in CASES {
        let e = fr(2.0, 0.4, 0.6);
        let anchor = (area.0 * 0.45, area.1 * 0.55);
        let p = screen_to_image(&e, area, img, anchor);
        for factor in [1.5, 0.75, 3.0] {
            let z = zoom_at(&e, area, img, factor, anchor);
            assert!((z.zoom - 2.0 * factor).abs() < 1e-5);
            let s = image_to_screen(&z, area, img, p);
            assert!(near(s, anchor, 1e-3), "factor {factor}: {s:?} != {anchor:?}");
        }
    }
}

#[test]
fn drag_move_the_image_with_the_pointer() {
    let (area, img) = CASES[1];
    let e = fr(4.0, 0.5, 0.5);
    let p = (0.5, 0.5);
    let before = image_to_screen(&e, area, img, p);
    let after = pan_by(&e, area, img, 30.0, -20.0);
    let s = image_to_screen(&after, area, img, p);
    assert!(near(s, (before.0 + 30.0, before.1 - 20.0), 1e-3), "{s:?}");
}

#[test]
fn drag_not_removes_the_center_of_0_the_1() {
    for (area, img) in CASES {
        for (dx, dy) in [(1e6, 1e6), (-1e6, -1e6), (1e6, -1e6), (-5e4, 3e5)] {
            for zoom in [1.0, 8.0, 32.0] {
                let r = pan_by(&fr(zoom, 0.5, 0.5), area, img, dx, dy);
                assert!((0.0..=1.0).contains(&r.cx) && (0.0..=1.0).contains(&r.cy), "{r:?}");
            }
        }
    }
}

#[test]
fn nan_and_infinite_become_patterns() {
    let r = clamp(fr(f32::NAN, f32::INFINITY, f32::NEG_INFINITY));
    assert_eq!(r, default_framing());
    let r = clamp(fr(2.0, -0.5, 1.5));
    assert_eq!(r, fr(2.0, 0.0, 1.0));
    let (area, img) = CASES[0];
    let r = zoom_at(&default_framing(), area, img, f32::NAN, (f32::NAN, 1.0));
    assert!(r.zoom.is_finite() && r.cx.is_finite() && r.cy.is_finite(), "{r:?}");
    let r = pan_by(&default_framing(), area, img, f32::NAN, f32::INFINITY);
    assert!(r.cx.is_finite() && r.cy.is_finite(), "{r:?}");
}

#[test]
fn sizes_degenerate_not_enter_in_panic() {
    let e = fr(3.0, 0.2, 0.7);
    let bad_ones: [Size; 4] = [(0.0, 0.0), (f32::NAN, 100.0), (100.0, -5.0), (f32::INFINITY, 10.0)];
    for bad in bad_ones {
        for (area, img) in [(bad, (100.0, 100.0)), ((100.0, 100.0), bad), (bad, bad)] {
            assert!(fit_scale(area, img).is_finite());
            let s = image_to_screen(&e, area, img, (0.5, 0.5));
            assert!(s.0.is_finite() && s.1.is_finite(), "{s:?}");
            let p = screen_to_image(&e, area, img, (10.0, 10.0));
            assert!(p.0.is_finite() && p.1.is_finite(), "{p:?}");
            for r in [zoom_at(&e, area, img, 2.0, (10.0, 10.0)), pan_by(&e, area, img, 5.0, 5.0)] {
                assert!(r.zoom.is_finite() && r.cx.is_finite() && r.cy.is_finite(), "{r:?}");
            }
        }
    }
}

#[test]
fn step_pan_proportional_the_area_and_safe() {
    let p = pan_step(1.0, (1000.0, 800.0));
    assert!(p > 0.0 && p.is_finite(), "{p}");
    // cresce com a area
    assert!(pan_step(1.0, (2000.0, 1600.0)) > p);
    assert!(pan_step(1.0, (500.0, 400.0)) < p);
    // o mesmo passo na tela em qualquer zoom (nada de passo zero ou gigante no zoom maximo)
    for z in [MIN_ZOOM, 2.0, 8.0, MAX_ZOOM, 0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(pan_step(z, (1000.0, 800.0)), p, "zoom {z}");
    }
    // area degenerada: passo padrao, positivo e finito
    for a in [(0.0, 0.0), (f32::NAN, 100.0), (100.0, -5.0), (f32::INFINITY, f32::INFINITY)] {
        let q = pan_step(1.0, a);
        assert!(q > 0.0 && q.is_finite(), "{a:?} -> {q}");
    }
    // janela minuscula ainda anda; janela enorme nao salta a imagem inteira
    assert!(pan_step(1.0, (10.0, 10.0)) >= 1.0);
    assert!(pan_step(1.0, (1e6, 1e6)) <= 1000.0);
}

#[test]
fn step_pan_with_pan_by_move_the_frame_in_the_direction_right() {
    // W: a regiao visivel sobe (cy diminui), a imagem desce na tela (pan_by com dy > 0).
    let (area, img) = CASES[1];
    let e = fr(4.0, 0.5, 0.5);
    let step = pan_step(e.zoom, area);
    let w = pan_by(&e, area, img, 0.0, step);
    let s = pan_by(&e, area, img, 0.0, -step);
    let a = pan_by(&e, area, img, step, 0.0);
    let d = pan_by(&e, area, img, -step, 0.0);
    assert!(w.cy < e.cy && s.cy > e.cy && a.cx < e.cx && d.cx > e.cx, "{w:?} {s:?} {a:?} {d:?}");
    // a imagem anda exatamente `step` pixels na tela
    let before = image_to_screen(&e, area, img, (0.5, 0.5));
    let after = image_to_screen(&w, area, img, (0.5, 0.5));
    assert!((after.1 - before.1 - step).abs() < 1e-3);
}

// --- Janela visivel e camadas (Task 4) ---

const PAGE: Size = (4000.0, 3000.0);
/// Mesma proporcao da pagina: no zoom 1 a pagina ocupa a area inteira.
const AREA: Size = (1000.0, 750.0);

/// Fracoes `[x0, y0, x1, y1]` que a janela cobre na pagina.
fn fractions(j: &ViewWindow) -> [f32; 4] {
    let (gw, gh) = (j.grid.0 as f32, j.grid.1 as f32);
    [j.region.x0 as f32 / gw, j.region.y0 as f32 / gh, j.region.x1 as f32 / gw, j.region.y1 as f32 / gh]
}

/// O retangulo visivel exato (fracoes da pagina grampeadas em 0..1).
fn visible(e: &Framing, area: Size, img: Size) -> [f32; 4] {
    let a = screen_to_image(e, area, img, (0.0, 0.0));
    let b = screen_to_image(e, area, img, area);
    let c = |v: f32| v.clamp(0.0, 1.0);
    [c(a.0), c(a.1), c(b.0), c(b.1)]
}

/// A janela cobre o retangulo visivel arredondado para fora na grade: no
/// maximo 1 pixel da grade a mais de cada lado, nunca a menos, sempre dentro.
fn exact_covers(j: &ViewWindow, v: [f32; 4], ctx: &str) {
    let (gw, gh) = (j.grid.0 as f32, j.grid.1 as f32);
    let f = fractions(j);
    let tol = 1e-4;
    assert!(j.region.x1 <= j.grid.0 && j.region.y1 <= j.grid.1, "{ctx}: fora da grade {j:?}");
    assert!(f[0] <= v[0] + tol && v[0] - f[0] < 1.0 / gw + tol, "{ctx}: x0 {f:?} vs {v:?}");
    assert!(f[1] <= v[1] + tol && v[1] - f[1] < 1.0 / gh + tol, "{ctx}: y0 {f:?} vs {v:?}");
    assert!(f[2] + tol >= v[2] && f[2] - v[2] < 1.0 / gw + tol, "{ctx}: x1 {f:?} vs {v:?}");
    assert!(f[3] + tol >= v[3] && f[3] - v[3] < 1.0 / gh + tol, "{ctx}: y1 {f:?} vs {v:?}");
    assert!(f.iter().all(|x| (0.0..=1.0).contains(x)), "{ctx}: fracoes fora de 0..1 {f:?}");
    assert!(!j.empty(), "{ctx}: janela vazia");
}

#[test]
fn window_visible_in_the_zoom_1_and_the_page_whole_in_the_grid_of_the_screen() {
    let j = visible_window(&default_framing(), AREA, PAGE, 1.0).unwrap();
    assert_eq!(j, ViewWindow { grid: (1000, 750), region: GridRegion { x0: 0, y0: 0, x1: 1000, y1: 750 } });
    // Retina: o dobro de pixels fisicos.
    let j = visible_window(&default_framing(), AREA, PAGE, 2.0).unwrap();
    assert_eq!(j, ViewWindow { grid: (2000, 1500), region: GridRegion { x0: 0, y0: 0, x1: 2000, y1: 1500 } });
    assert_eq!(j.size_px(), (2000, 1500), "o tile tem o tamanho fisico da area");
}

#[test]
fn window_visible_in_the_zoom_2_and_the_quarter_of_the_middle() {
    let e = fr(2.0, 0.5, 0.5);
    let j = visible_window(&e, AREA, PAGE, 1.0).unwrap();
    assert_eq!(j, ViewWindow { grid: (2000, 1500), region: GridRegion { x0: 500, y0: 375, x1: 1500, y1: 1125 } });
    let j = visible_window(&e, AREA, PAGE, 2.0).unwrap();
    assert_eq!(j, ViewWindow { grid: (4000, 3000), region: GridRegion { x0: 1000, y0: 750, x1: 3000, y1: 2250 } });
}

#[test]
fn window_visible_in_zooms_centers_and_factors() {
    let centers = [(0.5, 0.5), (0.0, 0.0), (1.0, 1.0), (0.0, 1.0), (1.0, 0.0), (0.3, 0.8)];
    let cases: [(Size, Size); 4] = [(AREA, PAGE), CASES[0], CASES[1], ((1280.0, 700.0), (1234.0, 5678.0))];
    for (area, img) in cases {
        for zoom in [1.0, 2.0, 8.0, 32.0] {
            for (cx, cy) in centers {
                for factor in [1.0, 2.0] {
                    let e = fr(zoom, cx, cy);
                    let ctx = format!("area {area:?} img {img:?} zoom {zoom} centro {cx},{cy} fator {factor}");
                    let j = visible_window(&e, area, img, factor).unwrap_or_else(|| panic!("{ctx}: sem janela"));
                    exact_covers(&j, visible(&e, area, img), &ctx);
                    // A grade e a pagina inteira no tamanho fisico da tela, nunca maior que a pagina.
                    let (pw, ph) = pixels_by_unit(&e, area, img).unwrap();
                    let expected = ViewWindow::screen_grid((img.0 as u32, img.1 as u32), (pw * factor, ph * factor));
                    assert_eq!(j.grid, expected, "{ctx}");
                    assert!(j.grid.0 <= img.0 as u32 && j.grid.1 <= img.1 as u32, "{ctx}");
                }
            }
        }
    }
}

const ROTATIONS: [Rotation; 4] = [Rotation::R0, Rotation::R90, Rotation::R180, Rotation::R270];

/// O retangulo visivel na IMAGEM por outro caminho: os cantos da area na
/// exibicao, cada um levado a imagem ponto a ponto.
fn image_visible(e: &Framing, area: Size, img: Size, rot: Rotation) -> [f32; 4] {
    let display = display_sz(img, rot);
    let c = |v: f32| v.clamp(0.0, 1.0);
    let a = rot.from_display((c(screen_to_image(e, area, display, (0.0, 0.0)).0), c(screen_to_image(e, area, display, (0.0, 0.0)).1)));
    let b = rot.from_display((c(screen_to_image(e, area, display, area).0), c(screen_to_image(e, area, display, area).1)));
    [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)]
}

#[test]
fn window_rotated_covers_the_rectangle_visible_in_the_image() {
    let centers = [(0.5, 0.5), (0.0, 0.0), (1.0, 1.0), (0.0, 1.0), (1.0, 0.0), (0.3, 0.8)];
    let cases: [(Size, Size); 4] = [(AREA, PAGE), CASES[0], CASES[1], ((1280.0, 700.0), (1234.0, 5678.0))];
    for rot in ROTATIONS {
        for (area, img) in cases {
            for zoom in [1.0, 2.0, 8.0, 32.0] {
                for (cx, cy) in centers {
                    for factor in [1.0, 2.0] {
                        // O centro esta na EXIBICAO: e o que o visualizador guarda.
                        let e = fr(zoom, cx, cy);
                        let ctx = format!("{rot:?} area {area:?} img {img:?} zoom {zoom} centro {cx},{cy} fator {factor}");
                        let j = window_visible_rotated(&e, area, img, rot, factor).unwrap_or_else(|| panic!("{ctx}: sem janela"));
                        exact_covers(&j, image_visible(&e, area, img, rot), &ctx);
                        // A grade e da pagina SEM girar: nunca maior que ela, e o tamanho
                        // fisico de cada eixo da imagem e o do eixo certo da exibicao.
                        let display = display_sz(img, rot);
                        let (pw, ph) = pixels_by_unit(&e, area, display).unwrap();
                        let (pw, ph) = if rot.sides_swap() { (ph, pw) } else { (pw, ph) };
                        let expected = ViewWindow::screen_grid((img.0 as u32, img.1 as u32), (pw * factor, ph * factor));
                        assert_eq!(j.grid, expected, "{ctx}");
                    }
                }
            }
        }
    }
}

#[test]
fn window_rotated_in_zero_and_the_of_always() {
    for (area, img) in [(AREA, PAGE), CASES[0], CASES[1]] {
        for zoom in [1.0, 3.0, 32.0] {
            let e = fr(zoom, 0.3, 0.8);
            assert_eq!(window_visible_rotated(&e, area, img, Rotation::R0, 2.0), visible_window(&e, area, img, 2.0));
        }
    }
}

#[test]
fn window_rotated_in_the_zoom_2_in_90_degrees() {
    // Pagina 4000x3000 girada 90 graus horarios vira 3000x4000 na exibicao. Com a
    // area 750x1000 (a mesma proporcao) o zoom 1 mostra a pagina inteira.
    let area = (750.0, 1000.0);
    let j = window_visible_rotated(&default_framing(), area, PAGE, Rotation::R90, 1.0).unwrap();
    assert_eq!(j.region, GridRegion { x0: 0, y0: 0, x1: j.grid.0, y1: j.grid.1 });
    assert_eq!(j.grid, (1000, 750), "a grade e a da pagina sem girar, no tamanho que ela tem na tela");
    // Zoom 2 no centro: o quarto do meio, tambem na imagem.
    let j = window_visible_rotated(&fr(2.0, 0.5, 0.5), area, PAGE, Rotation::R90, 1.0).unwrap();
    assert_eq!(j, ViewWindow { grid: (2000, 1500), region: GridRegion { x0: 500, y0: 375, x1: 1500, y1: 1125 } });
    // Zoom 2 com o centro na borda de cima da EXIBICAO (cy = 0): em 90 graus horarios o
    // alto da exibicao e a ESQUERDA da imagem, entao a janela encosta em x0 = 0.
    let j = window_visible_rotated(&fr(2.0, 0.5, 0.0), area, PAGE, Rotation::R90, 1.0).unwrap();
    assert_eq!(j.region.x0, 0);
    assert!(j.region.x1 < j.grid.0 && j.region.y0 > 0 && j.region.y1 < j.grid.1, "{j:?}");
}

#[test]
fn textbox_of_the_tile_rotated_and_the_part_visible_of_the_page() {
    // O tile da janela visivel cobre a pagina que cabe na area: a caixa da pagina
    // (exibicao) cortada pela area, com folga de no maximo 1 pixel da grade por lado.
    for rot in ROTATIONS {
        for (area, img) in [(AREA, PAGE), CASES[0], CASES[1]] {
            for zoom in [1.0, 2.0, 8.0] {
                for (cx, cy) in [(0.5, 0.5), (0.2, 0.9)] {
                    let e = fr(zoom, cx, cy);
                    let j = window_visible_rotated(&e, area, img, rot, 1.0).unwrap();
                    let c = rotated_window_textbox(&e, area, img, rot, &j);
                    let p = page_textbox(&e, area, display_sz(img, rot));
                    let expected = (p.0.max(0.0), p.1.max(0.0), p.2.min(area.0), p.3.min(area.1));
                    let ctx = format!("{rot:?} {area:?} {img:?} zoom {zoom} {cx},{cy}: {c:?} vs {expected:?} / {j:?}");
                    // Um pixel da grade na tela: a pagina na exibicao dividida pela grade.
                    let (lw, lh) = (p.2 - p.0, p.3 - p.1);
                    let (gw, gh) = if rot.sides_swap() { (j.grid.1, j.grid.0) } else { (j.grid.0, j.grid.1) };
                    let tol = 1.5 * (lw / gw.max(1) as f32).max(lh / gh.max(1) as f32).max(1.0);
                    assert!((c.0 - expected.0).abs() <= tol && (c.1 - expected.1).abs() <= tol, "{ctx}");
                    assert!((c.2 - expected.2).abs() <= tol && (c.3 - expected.3).abs() <= tol, "{ctx}");
                }
            }
        }
    }
    // R0 e o `window_textbox` de sempre.
    let e = fr(2.0, 0.3, 0.6);
    let j = visible_window(&e, AREA, PAGE, 1.0).unwrap();
    assert_eq!(rotated_window_textbox(&e, AREA, PAGE, Rotation::R0, &j), window_textbox(&e, AREA, PAGE, &j));
}

#[test]
fn window_visible_in_the_zoom_max_uses_the_grid_native() {
    let e = fr(MAX_ZOOM, 0.5, 0.5);
    let j = visible_window(&e, AREA, PAGE, 2.0).unwrap();
    assert_eq!(j.grid, (4000, 3000), "nunca amplia a pagina");
    // 1/32 da pagina em cada eixo: 125 x 93,75 pixels, arredondados para fora.
    assert_eq!(j.region, GridRegion { x0: 1937, y0: 1453, x1: 2063, y1: 1547 });
}

#[test]
fn window_visible_in_areas_small_and_page_small() {
    for area in [(1.0, 1.0), (3.0, 2.0), (0.5, 0.5), (7.0, 400.0)] {
        for zoom in [1.0, 32.0] {
            for factor in [1.0, 2.0] {
                let e = fr(zoom, 0.1, 0.9);
                let j = visible_window(&e, area, PAGE, factor).unwrap_or_else(|| panic!("area {area:?} zoom {zoom}"));
                exact_covers(&j, visible(&e, area, PAGE), &format!("area {area:?} zoom {zoom} fator {factor}"));
            }
        }
    }
    // Pagina menor que a area: a grade e a propria pagina e a janela, ela inteira.
    let j = visible_window(&default_framing(), (1000.0, 800.0), (200.0, 100.0), 2.0).unwrap();
    assert_eq!(j, ViewWindow { grid: (200, 100), region: GridRegion { x0: 0, y0: 0, x1: 200, y1: 100 } });
    let j = visible_window(&default_framing(), (1000.0, 800.0), (1.0, 1.0), 1.0).unwrap();
    assert_eq!(j.size_px(), (1, 1));
}

#[test]
fn area_or_page_no_size_not_has_window() {
    let e = fr(4.0, 0.3, 0.7);
    for bad in [(0.0, 0.0), (0.0, 100.0), (100.0, 0.0), (f32::NAN, 100.0), (-5.0, 10.0), (f32::INFINITY, 10.0)] {
        assert_eq!(visible_window(&e, bad, PAGE, 2.0), None, "area {bad:?}");
        assert_eq!(visible_window(&e, AREA, bad, 2.0), None, "pagina {bad:?}");
    }
    // Fator de escala absurdo vira 1 (nunca uma grade 0 ou infinita).
    for factor in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(visible_window(&default_framing(), AREA, PAGE, factor), visible_window(&default_framing(), AREA, PAGE, 1.0));
    }
    // Uma pagina de menos de meio pixel nao tem grade: nada a pedir.
    assert_eq!(visible_window(&default_framing(), AREA, (0.4, 0.4), 1.0), None);
}

#[test]
fn textbox_of_the_tile_and_the_image_to_screen_of_the_corners_of_the_window() {
    // Zoom 1 (mesma proporcao): a caixa do tile e a area visivel inteira.
    let j = visible_window(&default_framing(), AREA, PAGE, 2.0).unwrap();
    let c = window_textbox(&default_framing(), AREA, PAGE, &j);
    assert!(near((c.0, c.1), (0.0, 0.0), 1e-3) && near((c.2, c.3), AREA, 1e-3), "{c:?}");
    assert_eq!(c, page_textbox(&default_framing(), AREA, PAGE));
    // Em qualquer enquadramento: os cantos da regiao na grade, levados a tela,
    // e a caixa cobre a area visivel (no maximo 1 pixel da grade a mais).
    for (zoom, cx, cy) in [(2.0, 0.5, 0.5), (8.0, 0.0, 1.0), (32.0, 0.7, 0.2)] {
        let e = fr(zoom, cx, cy);
        let j = visible_window(&e, AREA, PAGE, 2.0).unwrap();
        let f = fractions(&j);
        let c = window_textbox(&e, AREA, PAGE, &j);
        assert_eq!((c.0, c.1), image_to_screen(&e, AREA, PAGE, (f[0], f[1])));
        assert_eq!((c.2, c.3), image_to_screen(&e, AREA, PAGE, (f[2], f[3])));
        let page = page_textbox(&e, AREA, PAGE);
        // Um pixel da grade na tela, em pixels logicos.
        let (pw, ph) = pixels_by_unit(&e, AREA, PAGE).unwrap();
        let (gx, gy) = (pw / j.grid.0 as f32, ph / j.grid.1 as f32);
        let vis = (page.0.max(0.0), page.1.max(0.0), page.2.min(AREA.0), page.3.min(AREA.1));
        assert!(c.0 <= vis.0 + 1e-2 && vis.0 - c.0 <= gx + 1e-2, "{c:?} {vis:?}");
        assert!(c.1 <= vis.1 + 1e-2 && vis.1 - c.1 <= gy + 1e-2, "{c:?} {vis:?}");
        assert!(c.2 + 1e-2 >= vis.2 && c.2 - vis.2 <= gx + 1e-2, "{c:?} {vis:?}");
        assert!(c.3 + 1e-2 >= vis.3 && c.3 - vis.3 <= gy + 1e-2, "{c:?} {vis:?}");
    }
}

#[test]
fn layers_use_the_size_of_the_page_never_the_of_the_texture() {
    // A base (2048x1536), o tile da pagina inteira na grade da tela e o da
    // grade nativa caem todos na MESMA caixa: a geometria (e com ela grifos e
    // setas, que usam `image_to_screen` com o tamanho da pagina) nao depende
    // do tamanho em pixels da textura de nenhuma camada.
    for e in [default_framing(), fr(3.0, 0.2, 0.6), fr(32.0, 1.0, 0.0)] {
        let page = page_textbox(&e, AREA, PAGE);
        for grid in [(2048, 1536), (1000, 750), (4000, 3000), (37, 5)] {
            let whole = ViewWindow { grid, region: GridRegion { x0: 0, y0: 0, x1: grid.0, y1: grid.1 } };
            assert_eq!(window_textbox(&e, AREA, PAGE, &whole), page, "grade {grid:?}");
        }
        let a = image_to_screen(&e, AREA, PAGE, (0.0, 0.0));
        let b = image_to_screen(&e, AREA, PAGE, (1.0, 1.0));
        assert_eq!(page, (a.0, a.1, b.0, b.1));
    }
}

/// O que o gpui faz com a caixa de `paint_image`: escala pelo fator e
/// arredonda a origem para baixo e o tamanho para cima, cada um por si.
fn like_gpui(origin: (f32, f32), size: (f32, f32), factor: f32) -> (f32, f32, f32, f32) {
    let (x, y) = ((origin.0 * factor).floor(), (origin.1 * factor).floor());
    (x, y, x + (size.0 * factor).ceil(), y + (size.1 * factor).ceil())
}

#[test]
fn textbox_to_outside_covers_os_pixels_physical_rounded_to_outside() {
    let textboxes = [(0.0, 0.0, 1000.0, 750.0), (10.3, 7.7, 20.1, 9.2), (-33.33, -0.49, 1033.4, 760.01), (5.0, 5.0, 5.4, 5.4), (0.1, 0.1, 0.2, 0.2)];
    for c in textboxes {
        for factor in [1.0, 2.0, 1.5, 3.0] {
            let (o, t) = textbox_to_outside(c, factor);
            let d = like_gpui(o, t, factor);
            let expected = ((c.0 * factor).floor(), (c.1 * factor).floor(), (c.2 * factor).ceil(), (c.3 * factor).ceil());
            assert_eq!(d, expected, "caixa {c:?} fator {factor}");
        }
    }
    // Base e tile que terminam na mesma borda (a da pagina) terminam no mesmo
    // pixel fisico: nada de degrau de 1 px entre as camadas na borda.
    let page = (12.25, 3.75, 812.6, 603.4);
    let tile = (400.125, 3.75, 812.6, 300.0);
    for factor in [1.0, 2.0] {
        let (op, tp) = textbox_to_outside(page, factor);
        let (ot, tt) = textbox_to_outside(tile, factor);
        let (p, t) = (like_gpui(op, tp, factor), like_gpui(ot, tt, factor));
        assert_eq!((p.1, p.2), (t.1, t.2), "fator {factor}");
    }
    // Caixa degenerada: tamanho zero, nunca negativo.
    let (_, t) = textbox_to_outside((10.0, 10.0, 5.0, 5.0), 2.0);
    assert_eq!(t, (0.0, 0.0));
}

#[test]
fn layers_swap_and_return_each_texture_a_time() {
    let j1 = ViewWindow { grid: (100, 100), region: GridRegion { x0: 0, y0: 0, x1: 50, y1: 50 } };
    let j2 = ViewWindow { grid: (100, 100), region: GridRegion { x0: 10, y0: 0, x1: 60, y1: 50 } };
    let mut c: Layers<u32> = Layers::default();
    assert!(!c.has());
    // Sem ajuste: so a cheia.
    assert_eq!(c.receive(Layer::Full, 1, false), Vec::<u32>::new());
    assert_eq!((c.full, c.base, c.tile), (Some(1), None, None));
    // Com ajuste: a base tira a cheia (a pagina sem filtro nunca fica por baixo).
    assert_eq!(c.receive(Layer::Base, 2, false), vec![1]);
    // O tile dos mesmos valores fica por cima da base.
    assert_eq!(c.receive(Layer::Tile(j1), 3, true), Vec::<u32>::new());
    assert_eq!((c.full, c.base, c.tile), (None, Some(2), Some((3, j1))));
    // Base de fundo nova (mesmos valores): so a base troca.
    assert_eq!(c.receive(Layer::Base, 4, true), vec![2]);
    // Tile de outra janela, mesmos valores: troca o tile, a base fica.
    assert_eq!(c.receive(Layer::Tile(j2), 5, true), vec![3]);
    assert_eq!((c.base, c.tile), (Some(4), Some((5, j2))));
    // Tile de OUTRO valor (o estado tirou a base): a base velha sai junto, o
    // fundo fica vazio, nunca com o filtro do valor anterior.
    let mut left = c.receive(Layer::Tile(j1), 6, false);
    left.sort();
    assert_eq!(left, vec![4, 5]);
    assert_eq!((c.full, c.base, c.tile), (None, None, Some((6, j1))));
    // Base de outro valor: tira o tile.
    assert_eq!(c.receive(Layer::Base, 7, false), vec![6]);
    // Voltou ao neutro: a cheia tira tudo.
    assert_eq!(c.receive(Layer::Tile(j1), 8, true), Vec::<u32>::new());
    let mut left = c.receive(Layer::Full, 9, false);
    left.sort();
    assert_eq!(left, vec![7, 8]);
    assert_eq!(c.clear(), vec![9]);
    assert!(!c.has() && c.clear().is_empty());
}

#[test]
fn factor_of_the_runs_limited_by_event() {
    assert!((runs_factor(0.0, 0.002) - 1.0).abs() < EPS);
    assert!((runs_factor(100.0, 0.002) - (0.2f32).exp()).abs() < EPS);
    assert_eq!(runs_factor(10_000.0, 0.002), FACTOR_RUNS_MAX);
    assert_eq!(runs_factor(-10_000.0, 0.002), FACTOR_RUNS_MIN);
    assert_eq!(runs_factor(f32::NAN, 0.002), 1.0);
    assert_eq!(runs_factor(f32::INFINITY, 0.002), 1.0);
}

/// Janela visivel da pagina sem rotacao (so os testes pedem sem girar).
pub fn visible_window(fr: &Framing, area: Size, img: Size, factor_scale_screen: f32) -> Option<ViewWindow> {
    window_visible_rotated(fr, area, img, Rotation::R0, factor_scale_screen)
}

/// Caixa do tile da janela `j` na tela, sem rotacao (so os testes pedem sem girar).
pub fn window_textbox(fr: &Framing, area: Size, img: Size, j: &ViewWindow) -> (f32, f32, f32, f32) {
    rotated_window_textbox(fr, area, img, Rotation::R0, j)
}
