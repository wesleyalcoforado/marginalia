use super::*;
use marginalia_core::highlights::HighlightsFile;
use marginalia_core::model::ImageRef;
use marginalia_core::rotation::Rotation;

const AREA: Size = (1000.0, 800.0);
/// Imagem 1000x800 na area 1000x800: zoom 1 ocupa a area toda (1 px = 1/1000).
const IMG: Size = (1000.0, 800.0);

fn framing_1() -> Framing {
    viewer_math::default_framing()
}

fn img() -> ImageRef {
    ImageRef { source: "Inv".into(), file: "p1.jpg".into(), page: 1 }
}

#[test]
fn gestures_mapping() {
    assert_eq!(gesture(false, false, Tool::No), Gesture::Relocate);
    assert_eq!(gesture(true, false, Tool::No), Gesture::Rectangle);
    assert_eq!(gesture(false, true, Tool::No), Gesture::Arrow);
    assert_eq!(gesture(true, true, Tool::No), Gesture::Rectangle);
}

#[test]
fn tool_linked_draws_no_modifier() {
    assert_eq!(gesture(false, false, Tool::Textbox), Gesture::LinkedRectangle);
    assert_eq!(gesture(false, false, Tool::Arrow), Gesture::LinkedArrow);
}

#[test]
fn modifier_prevails_about_the_tool() {
    assert_eq!(gesture(true, false, Tool::Arrow), Gesture::Rectangle);
    assert_eq!(gesture(false, true, Tool::Textbox), Gesture::Arrow);
}

#[test]
fn click_with_tool_linked_selects() {
    for g in [Gesture::LinkedRectangle, Gesture::LinkedArrow] {
        assert_eq!(sort(g, (0.0, 0.0), (1.0, 1.0)), MenuAction::Click, "{g:?}");
    }
    assert_eq!(sort(Gesture::LinkedRectangle, (0.0, 0.0), (20.0, 20.0)), MenuAction::Rectangle);
    assert_eq!(sort(Gesture::LinkedArrow, (0.0, 0.0), (20.0, 20.0)), MenuAction::Arrow);
}

#[test]
fn toggle_links_swap_and_disables() {
    assert_eq!(Tool::No.toggle(Tool::Textbox), Tool::Textbox);
    assert_eq!(Tool::Textbox.toggle(Tool::Arrow), Tool::Arrow);
    assert_eq!(Tool::Arrow.toggle(Tool::Arrow), Tool::No);
}

#[test]
fn drag_of_down_to_up_normalizes() {
    let g = rectangle(&framing_1(), AREA, IMG, (600.0, 400.0), (100.0, 80.0), "g1".into()).unwrap();
    match g {
        Highlight::Rectangle { x, y, w, h, .. } => {
            assert!((x - 0.1).abs() < 1e-4 && (y - 0.1).abs() < 1e-4, "{x},{y}");
            assert!((w - 0.5).abs() < 1e-4 && (h - 0.4).abs() < 1e-4, "{w},{h}");
        }
        o => panic!("{o:?}"),
    }
}

#[test]
fn drag_that_leaves_of_the_image_and_limited() {
    // area larga: imagem 1000x800 ajustada em 2000x800 ocupa x de 500 a 1500
    let area = (2000.0, 800.0);
    let g = rectangle(&framing_1(), area, IMG, (0.0, -50.0), (1000.0, 2000.0), "g".into()).unwrap();
    match g {
        Highlight::Rectangle { x, y, w, h, .. } => {
            assert_eq!((x, y), (0.0, 0.0));
            assert!((w - 0.5).abs() < 1e-4, "{w}");
            assert_eq!(h, 1.0);
        }
        o => panic!("{o:?}"),
    }
    // todo no fundo, a esquerda da imagem
    assert!(rectangle(&framing_1(), area, IMG, (10.0, 10.0), (400.0, 300.0), "g".into()).is_none());
}

#[test]
fn click_careless_not_becomes_highlight() {
    assert!(rectangle(&framing_1(), AREA, IMG, (100.0, 100.0), (102.0, 103.0), "g".into()).is_none());
    assert!(rectangle(&framing_1(), AREA, IMG, (100.0, 100.0), (100.0, 100.0), "g".into()).is_none());
    assert!(is_click((0.0, 0.0), (3.9, -3.9)));
    assert!(!is_click((0.0, 0.0), (4.0, 0.0)));
    assert!(is_click((0.0, 0.0), (f32::NAN, 10.0)));
    // arrasto fino (so num eixo) da largura zero no outro: nao e retangulo
    assert!(rectangle(&framing_1(), AREA, IMG, (100.0, 100.0), (300.0, 100.0), "g".into()).is_none());
}

#[test]
fn whole_rectangle_produced_and_accepted_by_the_highlights_file() {
    let mut f = HighlightsFile::default();
    let points = [-1e6, -3.0, 0.0, 0.1, 0.3333333, 0.5, 0.7, 0.9999999, 1.0, 1.0000001, 3.0, 1e6, f32::NAN];
    let mut n = 0;
    for &ax in &points {
        for &bx in &points {
            for &(ay, by) in &[(0.0, 1.0), (0.2, 0.9999999), (-0.5, 0.3)] {
                if let Some((x, y, w, h)) = relative_rectangle((ax, ay), (bx, by)) {
                    assert!(x + w <= 1.0 && y + h <= 1.0, "{x}+{w} {y}+{h}");
                    let g = Highlight::Rectangle { id: format!("g{n}"), x, y, w, h, color: None };
                    f.add(&img(), g).unwrap_or_else(|e| panic!("{ax},{bx}: {e}"));
                    n += 1;
                }
            }
        }
    }
    assert!(n > 50, "{n}");
    // e via tela, com zoom e centro quaisquer
    for zoom in [1.0, 3.0, 32.0] {
        for (cx, cy) in [(0.0, 0.0), (0.5, 0.5), (1.0, 1.0), (0.97, 0.03)] {
            let e = Framing { zoom, cx, cy };
            if let Some(g) = rectangle(&e, AREA, IMG, (-500.0, -500.0), (5000.0, 5000.0), "z".into()) {
                f.add(&img(), g).unwrap();
            }
        }
    }
}

#[test]
fn pointer_inside_and_outside_of_the_image() {
    match pointer(&framing_1(), AREA, IMG, (250.0, 200.0), "p".into()).unwrap() {
        Highlight::Pointer { x, y, .. } => assert!((x - 0.25).abs() < 1e-4 && (y - 0.25).abs() < 1e-4),
        o => panic!("{o:?}"),
    }
    let area = (2000.0, 800.0);
    assert!(pointer(&framing_1(), area, IMG, (100.0, 400.0), "p".into()).is_none());
    let mut f = HighlightsFile::default();
    f.add(&img(), pointer(&framing_1(), AREA, IMG, (1000.0, 800.0), "p".into()).unwrap()).unwrap();
}

#[test]
fn highlight_saved_back_the_same_screen_and_follows_the_zoom() {
    let g = rectangle(&framing_1(), AREA, IMG, (100.0, 80.0), (600.0, 400.0), "g".into()).unwrap();
    match in_screen(&framing_1(), AREA, IMG, &g) {
        Shape::Rectangle { x, y, w, h } => {
            assert!((x - 100.0).abs() < 0.1 && (y - 80.0).abs() < 0.1, "{x},{y}");
            assert!((w - 500.0).abs() < 0.1 && (h - 320.0).abs() < 0.1, "{w},{h}");
        }
        o => panic!("{o:?}"),
    }
    let z = Framing { zoom: 2.0, cx: 0.5, cy: 0.5 };
    match in_screen(&z, AREA, IMG, &g) {
        Shape::Rectangle { w, h, .. } => assert!((w - 1000.0).abs() < 0.1 && (h - 640.0).abs() < 0.1),
        o => panic!("{o:?}"),
    }
    let p = Highlight::Pointer { id: "p".into(), x: 0.5, y: 0.5, tail: None, color: None };
    assert_eq!(in_screen(&z, AREA, IMG, &p), Shape::ImagePoint { x: 500.0, y: 400.0 });
}

fn ac(gs: &[Highlight], p: (f32, f32), t: f32) -> Option<String> {
    fix(gs, p, (t, t))
}
fn r(id: &str, x: f32, y: f32, w: f32, h: f32) -> Highlight {
    Highlight::Rectangle { id: id.into(), x, y, w, h, color: None }
}
fn s(id: &str, tail: (f32, f32), tip: (f32, f32)) -> Highlight {
    Highlight::Pointer { id: id.into(), x: tip.0, y: tip.1, tail: Some(tail), color: None }
}

#[test]
fn fix_rectangle_inside_border_and_outside() {
    let gs = [r("r", 0.2, 0.2, 0.4, 0.4)];
    assert_eq!(ac(&gs, (0.4, 0.4), 0.01).as_deref(), Some("r"), "dentro");
    assert_eq!(ac(&gs, (0.605, 0.4), 0.01).as_deref(), Some("r"), "perto da borda, por fora");
    assert_eq!(ac(&gs, (0.2, 0.2), 0.0).as_deref(), Some("r"), "exatamente no canto");
    assert_eq!(ac(&gs, (0.7, 0.4), 0.01), None, "longe");
    assert_eq!(ac(&gs, (0.4, 0.1), 0.01), None, "longe, acima");
    assert_eq!(ac(&[], (0.4, 0.4), 0.5), None);
    assert_eq!(ac(&gs, (f32::NAN, 0.4), 0.01), None);
}

#[test]
fn fix_arrow_long_near_of_the_segment_far_and_tips() {
    let gs = [s("s", (0.1, 0.1), (0.9, 0.9))];
    assert_eq!(ac(&gs, (0.5, 0.505), 0.01).as_deref(), Some("s"), "perto do meio");
    assert_eq!(ac(&gs, (0.5, 0.6), 0.01), None, "longe do segmento");
    assert_eq!(ac(&gs, (0.1, 0.1), 0.01).as_deref(), Some("s"), "cauda");
    assert_eq!(ac(&gs, (0.9, 0.9), 0.01).as_deref(), Some("s"), "ponta");
    assert_eq!(ac(&gs, (0.95, 0.95), 0.01), None, "alem da ponta");
    assert_eq!(ac(&gs, (0.05, 0.05), 0.01), None, "antes da cauda");
    assert_eq!(ac(&gs, (0.95, 0.95), 0.1).as_deref(), Some("s"), "tolerancia maior");
}

#[test]
fn fix_pointer_old_no_tail_and_one_point_with_tail_short() {
    let gs = [Highlight::Pointer { id: "p".into(), x: 0.5, y: 0.5, tail: None, color: None }];
    assert_eq!(ac(&gs, (0.5, 0.5), 0.01).as_deref(), Some("p"));
    assert_eq!(ac(&gs, (0.505, 0.505), 0.01).as_deref(), Some("p"));
    let (dx, dy) = DEFAULT_TAIL;
    assert_eq!(ac(&gs, (0.5 + dx / 2.0, 0.5 + dy / 2.0), 0.005).as_deref(), Some("p"), "no meio da cauda padrao");
    assert_eq!(ac(&gs, (0.6, 0.6), 0.01), None);
    assert_eq!(ac(&gs, (0.5, 0.6), 0.01), None);
}

#[test]
fn fix_overlap_wins_the_last_of_the_list() {
    let gs = [r("baixo", 0.1, 0.1, 0.5, 0.5), r("cima", 0.3, 0.3, 0.5, 0.5), s("seta", (0.0, 0.5), (1.0, 0.5))];
    assert_eq!(ac(&gs, (0.4, 0.4), 0.01).as_deref(), Some("cima"));
    assert_eq!(ac(&gs, (0.15, 0.15), 0.01).as_deref(), Some("baixo"));
    assert_eq!(ac(&gs, (0.4, 0.5), 0.01).as_deref(), Some("seta"), "a seta e a ultima");
    assert_eq!(ac(&gs[..2], (0.4, 0.5), 0.01).as_deref(), Some("cima"));
}

#[test]
fn fix_rectangles_of_the_same_size_wins_the_last_same_with_noise_of_rounding() {
    // Dois retangulos gravados no mesmo lugar da imagem por rotacoes diferentes (smoke de rotacao):
    // as larguras diferem so no ultimo bit de um f32, o que nao faz um deles "menor". O de cima e o
    // ultimo da lista, nao o que a conta de area acaba deixando um pouquinho menor.
    let gs = [r("antigo", 0.19999999, 0.29999995, 0.3, 0.30000007), r("novo", 0.19999993, 0.29999995, 0.30000007, 0.30000007)];
    assert_eq!(ac(&gs, (0.2, 0.45), 0.01).as_deref(), Some("novo"));
    // Tamanho realmente menor continua ganhando, mesmo desenhado antes.
    let gs = [r("pequeno", 0.2, 0.3, 0.2, 0.2), r("grande", 0.1, 0.2, 0.5, 0.5)];
    assert_eq!(ac(&gs, (0.3, 0.4), 0.01).as_deref(), Some("pequeno"));
}

#[test]
fn arrow_rejects_degenerate_and_limits_the_0_1() {
    let id = || "s".to_string();
    assert!(arrow((0.5, 0.5), (0.5, 0.5), None, id()).is_none());
    assert!(arrow((0.5, 0.5), (0.500001, 0.5), None, id()).is_none(), "curta demais (abaixo de SETA_MINIMA)");
    assert!(arrow((0.5, 0.5), (0.5005, 0.5), None, id()).is_some(), "curta mas valida: o limiar de tela e do gesto");
    assert!(arrow((f32::NAN, 0.5), (0.9, 0.5), None, id()).is_none());
    assert!(arrow((2.0, 2.0), (1.5, 1.5), None, id()).is_none(), "as duas caem no mesmo canto depois de limitar");
    match arrow((-0.5, 0.2), (1.7, 0.2), Some("#e53935"), "s9".into()).unwrap() {
        Highlight::Pointer { id, x, y, tail, color } => {
            assert_eq!(id, "s9");
            assert_eq!((x, y), (1.0, 0.2));
            assert_eq!(tail, Some((0.0, 0.2)));
            assert_eq!(color.as_deref(), Some("#e53935"));
        }
        o => panic!("{o:?}"),
    }
    // sem cor: fica sem `color` (quem grava decide pela cor atual)
    let g = arrow((0.1, 0.1), (0.4, 0.5), None, "s1".into()).unwrap();
    assert!(matches!(&g, Highlight::Pointer { color: None, .. }));
    HighlightsFile::default().add(&img(), g).unwrap();
}

#[test]
fn fix_arrow_and_point_win_rectangle_same_drawn_after() {
    let gs = [s("seta", (0.1, 0.5), (0.9, 0.5)), r("grande", 0.0, 0.0, 1.0, 1.0)];
    assert_eq!(ac(&gs, (0.5, 0.5), 0.01).as_deref(), Some("seta"));
    assert_eq!(ac(&gs, (0.5, 0.2), 0.01).as_deref(), Some("grande"), "fora da seta sobra o retangulo");
    let pt = Highlight::Pointer { id: "pt".into(), x: 0.3, y: 0.3, tail: None, color: None };
    let gs = [pt, r("grande", 0.0, 0.0, 1.0, 1.0)];
    assert_eq!(ac(&gs, (0.3, 0.3), 0.01).as_deref(), Some("pt"));
}

#[test]
fn fix_rectangle_smaller_inside_of_the_larger_and_selectable() {
    let gs = [r("pequeno", 0.4, 0.4, 0.1, 0.1), r("grande", 0.1, 0.1, 0.8, 0.8)];
    assert_eq!(ac(&gs, (0.45, 0.45), 0.01).as_deref(), Some("pequeno"), "o grande vem depois mas e maior");
    assert_eq!(ac(&gs, (0.2, 0.2), 0.01).as_deref(), Some("grande"));
    let gs = [r("grande", 0.1, 0.1, 0.8, 0.8), r("pequeno", 0.4, 0.4, 0.1, 0.1)];
    assert_eq!(ac(&gs, (0.45, 0.45), 0.01).as_deref(), Some("pequeno"));
    // mesma area: o ultimo desenhado vence
    let gs = [r("a", 0.1, 0.1, 0.3, 0.3), r("b", 0.2, 0.2, 0.3, 0.3)];
    assert_eq!(ac(&gs, (0.3, 0.3), 0.01).as_deref(), Some("b"));
    // duas setas: a ultima
    let gs = [s("s1", (0.1, 0.5), (0.9, 0.5)), s("s2", (0.2, 0.5), (0.8, 0.5))];
    assert_eq!(ac(&gs, (0.5, 0.5), 0.01).as_deref(), Some("s2"));
}

#[test]
fn fix_tolerance_by_axis() {
    let gs = [r("r", 0.4, 0.4, 0.2, 0.2)];
    // 0.03 acima da borda: so a tolerancia de y alcanca
    assert_eq!(fix(&gs, (0.5, 0.37), (0.001, 0.05)).as_deref(), Some("r"));
    assert_eq!(fix(&gs, (0.5, 0.37), (0.05, 0.001)), None);
    // 0.03 a esquerda: o inverso
    assert_eq!(fix(&gs, (0.37, 0.5), (0.05, 0.001)).as_deref(), Some("r"));
    assert_eq!(fix(&gs, (0.37, 0.5), (0.001, 0.05)), None);
    let arrow_h = [s("s", (0.1, 0.5), (0.9, 0.5))];
    assert_eq!(fix(&arrow_h, (0.5, 0.53), (0.001, 0.05)).as_deref(), Some("s"));
    assert_eq!(fix(&arrow_h, (0.5, 0.53), (0.05, 0.001)), None);
    assert_eq!(fix(&gs, (0.5, 0.5), (f32::NAN, 0.01)), None);
    assert_eq!(fix(&gs, (0.5, 0.5), (0.01, f32::NAN)), None);
    assert_eq!(fix(&gs, (0.9, 0.9), (f32::INFINITY, f32::INFINITY)).as_deref(), Some("r"));
    assert_eq!(fix(&gs, (0.5, 0.5), (0.0, 0.0)).as_deref(), Some("r"), "dentro vale com tolerancia zero");
}

#[test]
fn elastic_normalizes_no_limit() {
    assert_eq!(elastic((50.0, 60.0), (10.0, 100.0)), Shape::Rectangle { x: 10.0, y: 60.0, w: 40.0, h: 40.0 });
}

// ---- Task 9: seta, clique, selecao, cor ----

#[test]
fn sort_modifier_and_distance() {
    let (a, far, near) = ((100.0, 100.0), (140.0, 130.0), (103.0, 96.5));
    assert_eq!(sort(Gesture::Rectangle, a, far), MenuAction::Rectangle);
    assert_eq!(sort(Gesture::Rectangle, a, near), MenuAction::Nothing, "Shift+clique nao e grifo");
    assert_eq!(sort(Gesture::Arrow, a, far), MenuAction::Arrow);
    assert_eq!(sort(Gesture::Arrow, a, near), MenuAction::ShortArrow, "Option+clique: seta curta padrao");
    assert_eq!(sort(Gesture::Relocate, a, far), MenuAction::Pan);
    assert_eq!(sort(Gesture::Relocate, a, near), MenuAction::Click, "clique simples seleciona");
    // limiar de 4 px em qualquer eixo
    assert_eq!(sort(Gesture::Relocate, a, (103.9, 103.9)), MenuAction::Click);
    assert_eq!(sort(Gesture::Relocate, a, (104.0, 100.0)), MenuAction::Pan);
    assert_eq!(sort(Gesture::Arrow, a, (100.0, 96.0)), MenuAction::Arrow);
    assert_eq!(sort(Gesture::Relocate, a, (f32::NAN, 100.0)), MenuAction::Click, "NaN nunca vira arrasto");
}

#[test]
fn option_drag_becomes_arrow_of_the_tail_ate_the_tip() {
    // pressiona em (100, 80) e solta em (600, 400): cauda (0.1, 0.1), ponta (0.6, 0.5)
    match gesture_arrow(&framing_1(), AREA, IMG, (100.0, 80.0), (600.0, 400.0), "s".into()).unwrap() {
        Highlight::Pointer { id, x, y, tail: Some((cx, cy)), color } => {
            assert_eq!(id, "s");
            assert!((x - 0.6).abs() < 1e-4 && (y - 0.5).abs() < 1e-4, "ponta {x},{y}");
            assert!((cx - 0.1).abs() < 1e-4 && (cy - 0.1).abs() < 1e-4, "cauda {cx},{cy}");
            assert_eq!(color, None, "a cor fica para commit_grifo");
        }
        o => panic!("{o:?}"),
    }
    // seta longa e exagerada, de um canto ao outro
    let g = gesture_arrow(&framing_1(), AREA, IMG, (1.0, 1.0), (999.0, 799.0), "s".into()).unwrap();
    HighlightsFile::default().add(&img(), g).unwrap();
}

#[test]
fn arrow_drops_outside_of_the_image_and_limited_and_starts_inside() {
    // area larga: a imagem ocupa x de 500 a 1500
    let area = (2000.0, 800.0);
    match gesture_arrow(&framing_1(), area, IMG, (1000.0, 400.0), (1900.0, -300.0), "s".into()).unwrap() {
        Highlight::Pointer { x, y, tail: Some((cx, cy)), .. } => {
            assert_eq!((x, y), (1.0, 0.0), "ponta limitada ao canto");
            assert!((cx - 0.5).abs() < 1e-4 && (cy - 0.5).abs() < 1e-4);
        }
        o => panic!("{o:?}"),
    }
    // apertar no fundo (fora da imagem) nao cria nada, nem com clique
    assert!(gesture_arrow(&framing_1(), area, IMG, (100.0, 400.0), (900.0, 400.0), "s".into()).is_none());
    assert!(gesture_arrow(&framing_1(), area, IMG, (100.0, 400.0), (100.0, 400.0), "s".into()).is_none());
    // tudo o que sai e aceito pelo GrifosFile
    let mut f = HighlightsFile::default();
    let pts = [-5000.0, -1.0, 0.0, 499.0, 500.0, 900.0, 1500.0, 1501.0, 5000.0, f32::NAN];
    let mut n = 0;
    for &bx in &pts {
        for &by in &[-900.0, 0.0, 400.0, 800.0, 900.0] {
            for start in [(500.0, 0.0), (1500.0, 800.0), (700.0, 300.0)] {
                if let Some(g) = gesture_arrow(&framing_1(), area, IMG, start, (bx, by), format!("g{n}")) {
                    f.add(&img(), g).unwrap_or_else(|e| panic!("{start:?} -> {bx},{by}: {e}"));
                    n += 1;
                }
            }
        }
    }
    assert!(n > 100, "{n}");
}

#[test]
fn option_click_makes_arrow_short_default_with_the_tip_in_the_click() {
    let p = (500.0, 400.0); // (0.5, 0.5)
    match gesture_arrow(&framing_1(), AREA, IMG, p, (501.0, 402.0), "s".into()).unwrap() {
        Highlight::Pointer { x, y, tail: Some((cx, cy)), .. } => {
            assert!((x - 0.5).abs() < 1e-4 && (y - 0.5).abs() < 1e-4, "ponta no ponto apertado");
            // cauda SETA_CURTA px acima e a esquerda (na tela)
            assert!((cx - (500.0 + SHORT_ARROW.0) / 1000.0).abs() < 1e-4, "{cx}");
            assert!((cy - (400.0 + SHORT_ARROW.1) / 800.0).abs() < 1e-4, "{cy}");
        }
        o => panic!("{o:?}"),
    }
    // no canto superior esquerdo a cauda padrao cairia em cima da ponta: vira para o outro lado
    match gesture_arrow(&framing_1(), AREA, IMG, (0.0, 0.0), (0.0, 0.0), "s".into()).unwrap() {
        Highlight::Pointer { x, y, tail: Some((cx, cy)), .. } => {
            assert_eq!((x, y), (0.0, 0.0));
            assert!(cx > 0.0 && cy > 0.0, "cauda abaixo e a direita: {cx},{cy}");
        }
        o => panic!("{o:?}"),
    }
}

/// Comprimento da seta na tela (pixels), com o enquadramento dado.
fn screen_px(fr: &Framing, g: &Highlight) -> f32 {
    match in_screen(fr, AREA, IMG, g) {
        Shape::Arrow { tail, tip } => ((tip.0 - tail.0).powi(2) + (tip.1 - tail.1).powi(2)).sqrt(),
        o => panic!("esperava seta, veio {o:?}"),
    }
}

#[test]
fn with_zoom_32_arrow_short_and_drag_of_6_px_continue_arrows() {
    let z = Framing { zoom: 32.0, cx: 0.5, cy: 0.5 };
    let p = (500.0, 400.0);
    // Option+arrasto de 6 px (acima do limiar de 4): seta, nao ponto
    let g = gesture_arrow(&z, AREA, IMG, p, (506.0, 400.0), "s".into()).unwrap();
    assert!(matches!(g, Highlight::Pointer { tail: Some(_), .. }), "{g:?}");
    assert!((screen_px(&z, &g) - 6.0).abs() < 0.1, "{}", screen_px(&z, &g));
    HighlightsFile::default().add(&img(), g).unwrap();
    // Option+clique: a seta curta padrao tem o mesmo tamanho NA TELA em qualquer zoom
    let expected = (SHORT_ARROW.0.powi(2) + SHORT_ARROW.1.powi(2)).sqrt();
    for zoom in [1.0, 4.0, 32.0] {
        let e = Framing { zoom, cx: 0.5, cy: 0.5 };
        let g = gesture_arrow(&e, AREA, IMG, p, p, "s".into()).unwrap();
        assert!((screen_px(&e, &g) - expected).abs() < 0.1, "zoom {zoom}: {}", screen_px(&e, &g));
        HighlightsFile::default().add(&img(), g).unwrap();
    }
}

#[test]
fn option_drag_of_2_px_and_click_not_arrow_of_the_drag() {
    // zoom 1, 2 px para a direita: e clique -> a seta curta padrao (cauda acima e a
    // esquerda), nunca uma seta de 2 px na direcao do arrasto
    let p = (500.0, 400.0);
    match gesture_arrow(&framing_1(), AREA, IMG, p, (502.0, 400.0), "s".into()).unwrap() {
        Highlight::Pointer { x, y, tail: Some((cx, cy)), .. } => {
            assert!((x - 0.5).abs() < 1e-4 && (y - 0.5).abs() < 1e-4);
            assert!(cx < x && cy < y, "cauda padrao, nao o arrasto: {cx},{cy}");
        }
        o => panic!("{o:?}"),
    }
}

#[test]
fn drag_that_the_limit_flattens_becomes_point() {
    // area larga (imagem de x 500 a 1500): arrasto de dentro, na borda esquerda, para
    // fora a esquerda: depois de limitar a ponta cai a menos de 4 px da cauda
    let area = (2000.0, 800.0);
    match gesture_arrow(&framing_1(), area, IMG, (501.0, 400.0), (100.0, 400.0), "s".into()).unwrap() {
        Highlight::Pointer { x, tail, .. } => {
            assert_eq!(tail, None, "ponto, nao seta de 1 px");
            assert_eq!(x, 0.0);
        }
        o => panic!("{o:?}"),
    }
}

#[test]
fn shape_outside_of_the_area_not_and_drawn() {
    let a = (1000.0, 800.0);
    assert!(!outside_area(Shape::Rectangle { x: 10.0, y: 10.0, w: 5.0, h: 5.0 }, a, 0.0));
    assert!(outside_area(Shape::Rectangle { x: -50.0, y: 10.0, w: 20.0, h: 5.0 }, a, 0.0));
    assert!(!outside_area(Shape::Rectangle { x: -50.0, y: 10.0, w: 20.0, h: 5.0 }, a, 40.0), "margem do halo");
    assert!(!outside_area(Shape::Rectangle { x: -50.0, y: -50.0, w: 2000.0, h: 2000.0 }, a, 0.0), "cobre a area");
    assert!(outside_area(Shape::Arrow { tail: (1100.0, 10.0), tip: (1200.0, 50.0) }, a, 20.0));
    assert!(!outside_area(Shape::Arrow { tail: (-100.0, 400.0), tip: (1100.0, 400.0) }, a, 0.0), "atravessa");
    assert!(outside_area(Shape::ImagePoint { x: 500.0, y: 900.0 }, a, 20.0));
    assert!(!outside_area(Shape::ImagePoint { x: 500.0, y: 810.0 }, a, 20.0));
    assert!(outside_area(Shape::ImagePoint { x: f32::NAN, y: 10.0 }, a, 20.0), "NaN nao desenha");
}

#[test]
fn preview_of_the_arrow_so_after_of_the_threshold_and_limited() {
    assert_eq!(arrow_preview(&framing_1(), AREA, IMG, (100.0, 100.0), (102.0, 101.0)), None, "ainda e clique");
    assert_eq!(
        arrow_preview(&framing_1(), AREA, IMG, (100.0, 80.0), (600.0, 400.0)),
        Some(Shape::Arrow { tail: (100.0, 80.0), tip: (600.0, 400.0) })
    );
    // a ponta segue o ponteiro mas para na borda da imagem
    match arrow_preview(&framing_1(), AREA, IMG, (100.0, 80.0), (1300.0, 900.0)) {
        Some(Shape::Arrow { tip, .. }) => assert!((tip.0 - 1000.0).abs() < 0.1 && (tip.1 - 800.0).abs() < 0.1, "{tip:?}"),
        o => panic!("{o:?}"),
    }
    assert_eq!(arrow_preview(&framing_1(), (2000.0, 800.0), IMG, (100.0, 400.0), (900.0, 400.0)), None, "comecou fora");
}

#[test]
fn tolerance_in_pixels_becomes_relative_by_axis() {
    // imagem 1000x800 na tela inteira: 8 px = 0.008 em x, 0.01 em y
    let (tx, ty) = tolerance(&framing_1(), AREA, IMG, 8.0).unwrap();
    assert!((tx - 0.008).abs() < 1e-6 && (ty - 0.01).abs() < 1e-6, "{tx},{ty}");
    // com zoom 4 a mesma tolerancia em pixels e 4x menor na imagem
    let z = Framing { zoom: 4.0, cx: 0.5, cy: 0.5 };
    let (zx, zy) = tolerance(&z, AREA, IMG, 8.0).unwrap();
    assert!((zx - 0.002).abs() < 1e-6 && (zy - 0.0025).abs() < 1e-6, "{zx},{zy}");
    // imagem nao quadrada numa area larga: eixos diferentes
    let (wx, wy) = tolerance(&framing_1(), (2000.0, 800.0), (400.0, 800.0), 8.0).unwrap();
    assert!((wx - 0.02).abs() < 1e-6 && (wy - 0.01).abs() < 1e-6, "{wx},{wy}");
    assert_eq!(tolerance(&framing_1(), (0.0, 0.0), IMG, 8.0), None, "area vazia");
    assert_eq!(tolerance(&framing_1(), AREA, (0.0, 0.0), 8.0), None, "sem imagem");
    // e o clique usa a conversao: 6 px ao lado de uma seta horizontal acerta, 12 nao
    let gs = [s("s", (0.1, 0.5), (0.9, 0.5))];
    let t = tolerance(&framing_1(), AREA, IMG, 8.0).unwrap();
    assert_eq!(fix(&gs, viewer_math::screen_to_image(&framing_1(), AREA, IMG, (500.0, 406.0)), t).as_deref(), Some("s"));
    assert_eq!(fix(&gs, viewer_math::screen_to_image(&framing_1(), AREA, IMG, (500.0, 412.0)), t), None);
}

#[test]
fn highlight_in_the_point_of_the_screen_selects_with_the_priority_right() {
    let gs = [r("grande", 0.0, 0.0, 1.0, 1.0), s("seta", (0.1, 0.5), (0.9, 0.5)), r("pequeno", 0.6, 0.1, 0.1, 0.1)];
    let no = |p| screen_highlight(&gs, &framing_1(), AREA, IMG, p);
    assert_eq!(no((500.0, 402.0)).as_deref(), Some("seta"));
    assert_eq!(no((650.0, 120.0)).as_deref(), Some("pequeno"));
    assert_eq!(no((200.0, 700.0)).as_deref(), Some("grande"));
    assert_eq!(screen_highlight(&gs[1..2], &framing_1(), AREA, IMG, (200.0, 700.0)), None, "vazio: tira a selecao");
}

#[test]
fn in_the_screen_arrow_point_old_and_handles() {
    let sg = s("s", (0.1, 0.25), (0.5, 0.5));
    assert_eq!(in_screen(&framing_1(), AREA, IMG, &sg), Shape::Arrow { tail: (100.0, 200.0), tip: (500.0, 400.0) });
    let p = Highlight::Pointer { id: "p".into(), x: 0.5, y: 0.5, tail: None, color: None };
    assert_eq!(in_screen(&framing_1(), AREA, IMG, &p), Shape::ImagePoint { x: 500.0, y: 400.0 }, "antigo continua ponto");
    assert_eq!(handles(Shape::Arrow { tail: (1.0, 2.0), tip: (3.0, 4.0) }), vec![(1.0, 2.0), (3.0, 4.0)]);
    assert_eq!(handles(Shape::Rectangle { x: 1.0, y: 2.0, w: 3.0, h: 4.0 }), vec![(1.0, 2.0), (4.0, 2.0), (1.0, 6.0), (4.0, 6.0)]);
    assert_eq!(handles(Shape::ImagePoint { x: 1.0, y: 2.0 }), vec![(1.0, 2.0)]);
}

fn near(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
}

#[test]
fn head_of_the_arrow_three_points_and_shaft_ate_the_base() {
    // seta horizontal para a direita, cabeca de 20 px
    let g = arrow_geometry((0.0, 0.0), (100.0, 0.0), 20.0).unwrap();
    assert!(near(g.head[0], (100.0, 0.0)), "a ponta e a ponta");
    assert!(near(g.end_shaft, (80.0, 0.0)), "a haste para na base: {:?}", g.end_shaft);
    let (e, d) = (g.head[1], g.head[2]);
    assert!(near((e.0, d.0), (80.0, 80.0)), "{e:?} {d:?}");
    assert!(e.1 < 0.0 && d.1 > 0.0 && (e.1 + d.1).abs() < 1e-3, "simetrica: {e:?} {d:?}");
    // vertical para cima
    let g = arrow_geometry((0.0, 100.0), (0.0, 0.0), 20.0).unwrap();
    assert!(near(g.end_shaft, (0.0, 20.0)));
    assert!(near(((g.head[1].1), g.head[2].1), (20.0, 20.0)));
    // seta curta: a cabeca nao passa da cauda (no maximo 60% do comprimento)
    let g = arrow_geometry((0.0, 0.0), (10.0, 0.0), 20.0).unwrap();
    assert!(near(g.end_shaft, (4.0, 0.0)), "{:?}", g.end_shaft);
    // degenerada ou NaN: nada (quem desenha cai no ponto)
    assert!(arrow_geometry((5.0, 5.0), (5.0, 5.0), 20.0).is_none());
    assert!(arrow_geometry((f32::NAN, 5.0), (5.0, 5.0), 20.0).is_none());
    assert!(arrow_geometry((0.0, 0.0), (5.0, 5.0), f32::NAN).is_none());
    assert!(arrow_geometry((0.0, 0.0), (5.0, 5.0), 0.0).is_none());
}

#[test]
fn color_of_the_highlight_in_rgb() {
    assert_eq!(color_rgb("#ffc400"), 0xffc400);
    assert_eq!(color_rgb("#E53935"), 0xe53935);
    for c in marginalia_core::highlights::PALETTE {
        assert_eq!(format!("#{:06x}", color_rgb(c)), c);
    }
    // cor estranha no JSON (editado a mao): a padrao, nunca um panico
    for bad in ["", "#fff", "amarelo", "#gg0000", "ffc400", "#ffc4001"] {
        assert_eq!(color_rgb(bad), 0xffc400, "{bad}");
    }
    // a cor vem so do grifo: ponteiro antigo sem cor e amarelo
    let p = Highlight::Pointer { id: "p".into(), x: 0.5, y: 0.5, tail: None, color: None };
    assert_eq!(color_rgb(p.color()), 0xffc400);
    assert_eq!(color_rgb(r("r", 0.1, 0.1, 0.1, 0.1).with_color("#1e88e5").color()), 0x1e88e5);
}

// --- Pagina exibida girada: o gesto e feito na EXIBICAO, o grifo vale na IMAGEM ---

const ROTATIONS: [Rotation; 4] = [Rotation::R0, Rotation::R90, Rotation::R180, Rotation::R270];

/// Ponto da imagem -> ponto da tela com a pagina `img` (original) girada em `rot`.
fn screen_rotated(rot: Rotation, area: Size, img: Size, p: (f32, f32)) -> (f32, f32) {
    viewer_math::image_to_screen(&framing_1(), area, viewer_math::display_sz(img, rot), rot.display_to(p))
}

#[test]
fn rectangle_drawn_in_the_page_rotated_falls_in_the_points_certain_of_the_image() {
    let img = (800.0, 400.0);
    let area = (1000.0, 1000.0);
    let (p, q) = ((0.2, 0.3), (0.7, 0.8));
    let (x, y, w, h) = relative_rectangle(p, q).unwrap();
    for rot in ROTATIONS {
        let display = viewer_math::display_sz(img, rot);
        let (sp, sq) = (screen_rotated(rot, area, img, p), screen_rotated(rot, area, img, q));
        let g = rectangle(&framing_1(), area, display, sp, sq, "g".into()).unwrap().from_display(rot);
        let Highlight::Rectangle { x: gx, y: gy, w: gw, h: gh, .. } = g else { panic!("{g:?}") };
        let ok = |a: f32, b: f32| (a - b).abs() < 1e-4;
        assert!(ok(gx, x) && ok(gy, y) && ok(gw, w) && ok(gh, h), "{rot:?}: {gx},{gy},{gw},{gh}");
    }
}

#[test]
fn arrow_drawn_in_the_page_rotated_keeps_tail_and_tip() {
    let img = (800.0, 400.0);
    let area = (1000.0, 1000.0);
    let (tail, tip) = ((0.15, 0.25), (0.75, 0.6));
    for rot in ROTATIONS {
        let display = viewer_math::display_sz(img, rot);
        let (sc, sp) = (screen_rotated(rot, area, img, tail), screen_rotated(rot, area, img, tip));
        let g = gesture_arrow(&framing_1(), area, display, sc, sp, "s".into()).unwrap().from_display(rot);
        let Highlight::Pointer { x, y, tail: Some(c), .. } = g else { panic!("{g:?}") };
        let ok = |a: f32, b: f32| (a - b).abs() < 1e-4;
        assert!(ok(x, tip.0) && ok(y, tip.1) && ok(c.0, tail.0) && ok(c.1, tail.1), "{rot:?}: {x},{y} {c:?}");
    }
}

#[test]
fn click_in_the_page_rotated_finds_the_highlight_of_the_image() {
    let img = (800.0, 400.0);
    let area = (1000.0, 1000.0);
    let stored = Highlight::Rectangle { id: "g".into(), x: 0.2, y: 0.3, w: 0.4, h: 0.3, color: None };
    for rot in ROTATIONS {
        let display = viewer_math::display_sz(img, rot);
        let in_display = vec![stored.display_to(rot)];
        // O meio de uma borda do retangulo, e um ponto bem longe dele.
        let border = screen_rotated(rot, area, img, (0.2, 0.45));
        let far = screen_rotated(rot, area, img, (0.95, 0.95));
        assert_eq!(screen_highlight(&in_display, &framing_1(), area, display, border).as_deref(), Some("g"), "{rot:?}");
        assert_eq!(screen_highlight(&in_display, &framing_1(), area, display, far), None, "{rot:?}");
    }
}
