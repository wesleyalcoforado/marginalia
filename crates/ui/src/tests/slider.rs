use super::*;
use crate::app_state::step;

fn neutral() -> Adjustments {
    Adjustments::default()
}

#[test]
fn strips_and_bipolar() {
    assert_eq!(Bar::Brightness.strip(), (-100, 100));
    assert_eq!(Bar::Contrast.strip(), (-100, 100));
    assert_eq!(Bar::Sharpness.strip(), (0, 100));
    assert_eq!(Bar::Highpass.strip(), (0, 100));
    assert_eq!(Bar::Threshold.strip(), (0, 255));
    assert!(Bar::Brightness.bipolar() && Bar::Contrast.bipolar());
    assert!(!Bar::Sharpness.bipolar() && !Bar::Highpass.bipolar() && !Bar::Threshold.bipolar());
    for (i, b) in ALL.iter().enumerate() {
        assert_eq!(b.index(), i);
    }
}

#[test]
fn neutral_in_the_screen_and_zero_and_the_bipolar_stays_in_the_center() {
    for b in [Bar::Brightness, Bar::Contrast, Bar::Sharpness, Bar::Highpass] {
        assert_eq!(b.in_screen(&neutral()), 0, "{b:?}");
    }
    assert_eq!(Bar::Threshold.in_screen(&neutral()), THRESHOLD_DEFAULT, "desligado: marcador em 128");
    assert!(!Bar::Threshold.active(&neutral()));
    assert!(Bar::Brightness.active(&neutral()));
    assert_eq!(Bar::Brightness.fraction(0), 0.5);
    assert_eq!(Bar::Contrast.fraction(0), 0.5);
    assert_eq!(Bar::Sharpness.fraction(0), 0.0);
    assert_eq!(Bar::Brightness.fraction_value(0.5), 0);
}

#[test]
fn contrast_screen_and_internal() {
    let c = |v| Bar::Contrast.apply(&neutral(), v).contrast;
    assert_eq!(c(0), 1.0, "tela 0 = 1,0 por dentro");
    assert_eq!(c(-100), 0.0);
    assert_eq!(c(100), 4.0);
    assert!((c(-50) - 0.5).abs() < 1e-6);
    assert!((c(50) - 2.5).abs() < 1e-6);
    let t = |x: f32| Bar::Contrast.in_screen(&Adjustments { contrast: x, ..neutral() });
    assert_eq!((t(0.0), t(1.0), t(4.0), t(2.5), t(0.5)), (-100, 0, 100, 50, -50));
    assert_eq!(t(9.0), 100, "fora da faixa: o limite");
    assert_eq!(t(f32::NAN), 0, "NaN: o neutro");
}

#[test]
fn brightness_sharpness_pass_high_and_threshold_screen_and_internal() {
    let a = Bar::Brightness.apply(&neutral(), -37);
    assert!((a.brightness + 0.37).abs() < 1e-6);
    assert_eq!(Bar::Brightness.in_screen(&Adjustments { brightness: 0.25, ..neutral() }), 25);
    assert_eq!(Bar::Brightness.in_screen(&Adjustments { brightness: f32::NAN, ..neutral() }), 0);
    assert_eq!(Bar::Brightness.in_screen(&Adjustments { brightness: 3.0, ..neutral() }), 100);
    assert!((Bar::Sharpness.apply(&neutral(), 40).sharpen - 0.4).abs() < 1e-6);
    assert!((Bar::Highpass.apply(&neutral(), 100).highpass - 1.0).abs() < 1e-6);
    assert_eq!(Bar::Sharpness.in_screen(&Adjustments { sharpen: -1.0, ..neutral() }), 0);
    let l = Bar::Threshold.apply(&neutral(), 200);
    assert_eq!(l.threshold, Some(200), "mexer na barra liga o limiar");
    assert!(Bar::Threshold.active(&l));
    assert_eq!(Bar::Threshold.in_screen(&l), 200);
    // so a barra muda
    let mut all = Adjustments { brightness: 0.2, invert: true, grayscale: true, threshold: Some(9), ..neutral() };
    all = Bar::Contrast.apply(&all, 50);
    assert_eq!((all.brightness, all.invert, all.grayscale, all.threshold), (0.2, true, true, Some(9)));
}

#[test]
fn apply_sanitizes_the_strip() {
    assert_eq!(Bar::Brightness.apply(&neutral(), 500).brightness, 1.0);
    assert_eq!(Bar::Brightness.apply(&neutral(), -500).brightness, -1.0);
    assert_eq!(Bar::Threshold.apply(&neutral(), 999).threshold, Some(255));
    assert_eq!(Bar::Threshold.apply(&neutral(), -3).threshold, Some(0));
    assert_eq!(Bar::Sharpness.apply(&neutral(), -3).sharpen, 0.0);
}

#[test]
fn forward_and_back_screen_internal_screen() {
    for b in ALL {
        let (min, max) = b.strip();
        for v in min..=max {
            assert_eq!(b.in_screen(&b.apply(&neutral(), v)), v, "{b:?} {v}");
        }
    }
}

#[test]
fn position_and_value() {
    assert_eq!(Bar::Brightness.fraction(-100), 0.0);
    assert_eq!(Bar::Brightness.fraction(100), 1.0);
    assert_eq!(Bar::Threshold.fraction(255), 1.0);
    assert_eq!(Bar::Brightness.fraction(900), 1.0, "fora: o limite");
    assert_eq!(Bar::Brightness.fraction_value(0.0), -100);
    assert_eq!(Bar::Brightness.fraction_value(1.0), 100);
    assert_eq!(Bar::Brightness.fraction_value(-3.0), -100);
    assert_eq!(Bar::Brightness.fraction_value(7.0), 100);
    assert_eq!(Bar::Sharpness.fraction_value(0.25), 25);
    assert_eq!(Bar::Brightness.fraction_value(f32::NAN), 0, "NaN: o neutro");
    assert_eq!(Bar::Threshold.fraction_value(f32::NAN), THRESHOLD_DEFAULT);
    for b in ALL {
        let (min, max) = b.strip();
        for v in min..=max {
            assert_eq!(b.fraction_value(b.fraction(v)), v, "{b:?} {v}");
        }
    }
    // trilha de 200 px a partir de x = 50
    assert_eq!(Bar::Brightness.point_value(150.0, 50.0, 200.0), Some(0), "o meio");
    assert_eq!(Bar::Brightness.point_value(50.0, 50.0, 200.0), Some(-100));
    assert_eq!(Bar::Brightness.point_value(0.0, 50.0, 200.0), Some(-100), "a esquerda da trilha: o minimo");
    assert_eq!(Bar::Brightness.point_value(900.0, 50.0, 200.0), Some(100), "a direita: o maximo");
    assert_eq!(Bar::Sharpness.point_value(100.0, 50.0, 200.0), Some(25));
    assert_eq!(Bar::Brightness.point_value(100.0, 50.0, 0.0), None, "trilha sem largura (ainda nao pintada)");
    assert_eq!(Bar::Brightness.point_value(f32::NAN, 50.0, 200.0), None);
}

#[test]
fn number_huge_saturates_in_the_limit_of_the_sign() {
    let huge = "9".repeat(400); // passa do f64: vira inf
    assert_eq!(interpret_text(&huge, Bar::Brightness), Some(100));
    assert_eq!(interpret_text(&format!("-{huge}"), Bar::Brightness), Some(-100));
    assert_eq!(interpret_text(&format!("−{huge}.5"), Bar::Threshold), Some(0));
    assert_eq!(interpret_text(&huge, Bar::Threshold), Some(255));
}

/// A caixa so guarda o que o usuario digitou (suja) ate Enter/Esc; fora
/// disso mostra sempre o valor atual, focada ou nao (Restaurar, interruptor,
/// menu ou outra imagem com a caixa ainda em foco).
#[test]
fn textbox_dirty_so_while_se_types() {
    let mut s = DirtyTextboxes::default();
    for b in ALL {
        assert!(s.rewrite(b, "+50", "0"), "{b:?}: limpa e diferente: reescreve");
        assert!(!s.rewrite(b, "0", "0"), "{b:?}: igual: nada");
    }
    s.changed(Bar::Contrast);
    assert!(!s.rewrite(Bar::Contrast, "4", "+50"), "digitando: o texto fica");
    assert!(s.rewrite(Bar::Brightness, "+50", "0"), "as outras seguem o valor");
    s.clear(Bar::Contrast); // Enter / Esc
    assert!(s.rewrite(Bar::Contrast, "4", "+50"));
    s.changed(Bar::Brightness);
    s.changed(Bar::Threshold);
    s.clear_all(); // outra imagem
    assert!(s.rewrite(Bar::Brightness, "7", "0") && s.rewrite(Bar::Threshold, "7", ""));
}

#[test]
fn double_click_back_to_the_neutral() {
    let a = Adjustments { brightness: 0.4, contrast: 2.0, sharpen: 0.3, highpass: 0.5, threshold: Some(90), invert: true, ..neutral() };
    assert_eq!(Bar::Brightness.restore(&a).brightness, 0.0);
    assert_eq!(Bar::Contrast.restore(&a).contrast, 1.0);
    assert_eq!(Bar::Sharpness.restore(&a).sharpen, 0.0);
    assert_eq!(Bar::Highpass.restore(&a).highpass, 0.0);
    assert_eq!(Bar::Threshold.restore(&a).threshold, None, "o neutro do limiar e desligado");
    // so a barra volta; o resto fica
    let r = Bar::Brightness.restore(&a);
    assert_eq!((r.contrast, r.sharpen, r.highpass, r.threshold, r.invert), (2.0, 0.3, 0.5, Some(90), true));
    for b in ALL {
        assert!(b.restore(&neutral()).is_neutral(), "{b:?}");
    }
}

#[test]
fn textbox_text() {
    assert_eq!(Bar::Brightness.text(&neutral()), "0");
    assert_eq!(Bar::Brightness.text(&Adjustments { brightness: 0.12, ..neutral() }), "+12");
    assert_eq!(Bar::Brightness.text(&Adjustments { brightness: -0.12, ..neutral() }), "-12");
    assert_eq!(Bar::Contrast.text(&Adjustments { contrast: 4.0, ..neutral() }), "+100");
    assert_eq!(Bar::Sharpness.text(&Adjustments { sharpen: 0.4, ..neutral() }), "40");
    assert_eq!(Bar::Threshold.text(&neutral()), "", "desligado: a caixa mostra a dica");
    assert_eq!(Bar::Threshold.text(&Adjustments { threshold: Some(7), ..neutral() }), "7");
    // o texto mostrado e lido de volta como o mesmo valor
    for b in ALL {
        let (min, max) = b.strip();
        for v in min..=max {
            let a = b.apply(&neutral(), v);
            assert_eq!(interpret_text(&b.text(&a), b), Some(v), "{b:?} {v}");
        }
    }
}

#[test]
fn interpret_the_text_typed() {
    let b = Bar::Brightness;
    assert_eq!(interpret_text("25", b), Some(25));
    assert_eq!(interpret_text("  +25 ", b), Some(25));
    assert_eq!(interpret_text("-25", b), Some(-25));
    assert_eq!(interpret_text("−25", b), Some(-25), "sinal de menos tipografico");
    assert_eq!(interpret_text("12.6", b), Some(13), "decimal arredonda");
    assert_eq!(interpret_text("12,4", b), Some(12), "virgula decimal");
    assert_eq!(interpret_text("250", b), Some(100), "fora da faixa: saneado ao limite");
    assert_eq!(interpret_text("-250", b), Some(-100));
    assert_eq!(interpret_text("-5", Bar::Sharpness), Some(0));
    assert_eq!(interpret_text("300", Bar::Threshold), Some(255));
    for invalid in ["", "   ", "abc", "1a", "+", "-", "12..3", "nan", "NaN", "inf", "-infinity", "1 2", "0x10"] {
        assert_eq!(interpret_text(invalid, b), None, "{invalid:?}: o valor anterior fica");
    }
}

#[test]
fn arrows_of_the_textbox_with_and_no_shift() {
    assert_eq!(key_step("up", false), Some(1));
    assert_eq!(key_step("down", false), Some(-1));
    assert_eq!(key_step("up", true), Some(10));
    assert_eq!(key_step("down", true), Some(-10));
    for k in ["left", "right", "enter", "escape", "w", "]", "pageup"] {
        assert_eq!(key_step(k, false), None, "{k}");
    }
    let a = Bar::Brightness.step(&neutral(), 10);
    assert_eq!(Bar::Brightness.in_screen(&a), 10);
    let a = Bar::Brightness.step(&a, -1);
    assert_eq!(Bar::Brightness.in_screen(&a), 9);
    assert_eq!(Bar::Brightness.in_screen(&Bar::Brightness.step(&Bar::Brightness.apply(&neutral(), 95), 10)), 100, "para no limite");
    assert_eq!(Bar::Sharpness.in_screen(&Bar::Sharpness.step(&neutral(), -1)), 0);
    // Limiar desligado: a seta liga a partir de 128
    assert_eq!(Bar::Threshold.step(&neutral(), 1).threshold, Some(129));
}

/// `[ ] { }` e os itens de menu (`app_state::step`) movem a barra pelos passos de sempre.
#[test]
fn keys_and_menus_move_the_bar_by_the_same_steps() {
    let n = neutral();
    assert_eq!(Bar::Brightness.in_screen(&step(&n, Bar::Brightness.control(), 1)), 5, "] = 0,05 = 5 na barra");
    assert_eq!(Bar::Brightness.in_screen(&step(&n, Bar::Brightness.control(), -2)), -10);
    assert_eq!(Bar::Contrast.in_screen(&step(&n, Bar::Contrast.control(), 1)), 3, "}} = 1,1 por dentro");
    assert_eq!(Bar::Contrast.in_screen(&step(&n, Bar::Contrast.control(), -1)), -10, "{{ = 0,9 por dentro");
    assert_eq!(Bar::Sharpness.in_screen(&step(&n, Bar::Sharpness.control(), 1)), 10);
    assert_eq!(Bar::Highpass.in_screen(&step(&n, Bar::Highpass.control(), 1)), 10);
    let l = step(&n, Bar::Threshold.control(), 1);
    assert!(Bar::Threshold.active(&l));
    assert_eq!(Bar::Threshold.in_screen(&l), 128);
    assert_eq!(Bar::Threshold.in_screen(&step(&l, Bar::Threshold.control(), 1)), 136, "8 em 8");
    // e a partir de um valor posto pela barra, o passo continua no mesmo controle
    let a = Bar::Brightness.apply(&n, 12);
    assert_eq!(Bar::Brightness.in_screen(&step(&a, Bar::Brightness.control(), 1)), 15);
}
