use super::*;
use std::collections::BTreeMap;
use std::path::Path;

/// Luminancia relativa (WCAG 2) de `0xRRGGBB`.
fn luminance(c: u32) -> f64 {
    let channel = |s: u32| {
        let v = ((c >> s) & 0xff) as f64 / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
}

/// Razao de contraste (WCAG 2) entre duas cores: de 1 (iguais) a 21.
fn contrast(a: u32, b: u32) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (light, dark) = if la > lb { (la, lb) } else { (lb, la) };
    (light + 0.05) / (dark + 0.05)
}

const THEMES: [Theme; 3] = Theme::ALL;

#[test]
fn text_principal_and_strong_read_well_about_all_surface() {
    for t in THEMES {
        let p = palette(t);
        for (name, background) in [
            ("fundo", p.background),
            ("barra", p.bar),
            ("lateral", p.lateral),
            ("cartao", p.card),
            ("elevado", p.raised),
            ("campo", p.field),
            ("hover_suave", p.hover_soft),
            ("botao", p.btn),
            ("acento_fundo", p.background_accent),
            ("ligado_fundo", p.linked_background),
        ] {
            for (label, text) in [("texto", p.text), ("texto_forte", p.strong_text)] {
                let c = contrast(text, background);
                assert!(c >= 4.5, "{t:?}: {label} sobre {name} = {c:.2}");
            }
        }
    }
}

#[test]
fn text_secondary_le_about_as_surfaces_of_content() {
    for t in THEMES {
        let p = palette(t);
        for (name, background) in [("fundo", p.background), ("barra", p.bar), ("cartao", p.card), ("campo", p.field), ("lateral", p.lateral)] {
            for (label, text, min) in
                [("texto_medio", p.medium_text, 4.5), ("texto_suave", p.soft_text, 3.5), ("texto_fraco", p.weak_text, 3.0)]
            {
                let c = contrast(text, background);
                assert!(c >= min, "{t:?}: {label} sobre {name} = {c:.2} (minimo {min})");
            }
        }
        // Do mais forte ao mais fraco, o contraste so cai.
        let c = |text| contrast(text, p.bar);
        assert!(c(p.text) >= c(p.medium_text) && c(p.medium_text) >= c(p.soft_text) && c(p.soft_text) >= c(p.weak_text), "{t:?}");
    }
}

#[test]
fn accent_states_and_emphasis_read_well() {
    for t in THEMES {
        let p = palette(t);
        // Controles (caixa marcada, botao ligado), nao texto corrido: o escuro de sempre fica em 3,2.
        let c = contrast(p.accent_about, p.accent);
        assert!(c >= 3.0, "{t:?}: sobre_acento sobre acento = {c:.2}");
        // Os estados (texto colorido) sobre o fundo e as barras.
        for (label, color) in [("aviso", p.warning), ("ok", p.ok), ("erro", p.error), ("destaque", p.emphasis)] {
            for (name, background) in [("fundo", p.background), ("barra", p.bar), ("campo", p.field)] {
                let c = contrast(color, background);
                assert!(c >= 3.5, "{t:?}: {label} sobre {name} = {c:.2}");
            }
        }
        for (label, color, background) in [
            ("erro_texto", p.text_error, p.background_error),
            ("aviso", p.warning, p.background_warning),
            ("texto", p.text, p.background_error),
            ("texto", p.text, p.background_warning),
        ] {
            let c = contrast(color, background);
            assert!(c >= 3.5, "{t:?}: {label} sobre o fundo do aviso/erro = {c:.2}");
        }
    }
}

#[test]
fn dark_and_light_se_distinguish_and_sepia_and_hot() {
    let (e, c, s) = (palette(Theme::Dark), palette(Theme::Light), palette(Theme::Sepia));
    assert!(luminance(e.background) < 0.05 && luminance(c.background) > 0.6 && luminance(s.background) > 0.55);
    assert!(luminance(e.text) > luminance(e.background) && luminance(c.text) < luminance(c.background) && luminance(s.text) < luminance(s.background));
    // Sepia: papel amarelado (vermelho e verde acima do azul).
    let (r, g, b) = ((s.background >> 16) & 0xff, (s.background >> 8) & 0xff, s.background & 0xff);
    assert!(r > b + 20 && g > b + 10, "{:06x}", s.background);
    assert_ne!(e, c);
    assert_ne!(c, s);
}

#[test]
fn the_dark_and_the_visual_of_today() {
    // Os valores que o app tinha antes dos temas (as telas e os dados antigos nao mudam).
    let e = palette(Theme::Dark);
    assert_eq!((e.background, e.bar, e.lateral, e.border, e.text), (0x202020, 0x252525, 0x2c2c2c, 0x3c3c3c, 0xe0e0e0));
    assert_eq!((e.accent, e.accent_hover, e.background_accent), (0x5b8def, 0x7aa2f7, 0x34405a));
    assert_eq!((e.btn, e.button_hover, e.hover_soft), (0x3d3d3d, 0x505050, 0x3a3a3a));
}

#[test]
fn set_swap_the_palette_current() {
    // Unico teste que mexe no tema global: volta ao escuro no fim.
    let before = current_theme();
    for t in THEMES {
        set(t);
        assert_eq!(current_theme(), t);
        assert_eq!(current(), palette(t));
    }
    set(before);
}

/// Cores que nao mudam com o tema (por arquivo): a selecao e as sombras do visualizador, o
/// fundo escurecido das telas sobrepostas, o vermelho dos botoes de perigo, o texto escuro dos
/// chips de etiqueta e o aro da bolinha de cor. Todo o resto vem de `theme::current()`.
fn fixed() -> BTreeMap<&'static str, Vec<&'static str>> {
    BTreeMap::from([
        ("viewer.rs", vec!["00e5ff", "ffffff", "1e88e5", "0000008c", "00000000", "ffffff1a", "000000"]),
        ("main_view/help_screen.rs", vec!["00000066"]),
        ("main_view/screen_config.rs", vec!["00000066"]),
        ("main_view/widgets.rs", vec!["8a3030", "a04040", "1e1e1e"]),
        ("main_view/barras.rs", vec!["1a1a1a", "ffffff", "e81123"]),
    ])
}

fn files(dir: &Path, output: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            if name != "tests" && name != "smoke" {
                files(&p, output);
            }
        } else if p.extension().is_some_and(|x| x == "rs") {
            output.push(p);
        }
    }
}

/// Os literais `0xRRGGBB` / `0xRRGGBBAA` de um texto, com a linha.
fn literals(text: &str) -> Vec<(usize, String)> {
    let mut v = Vec::new();
    for (i, l) in text.lines().enumerate() {
        // Comentario (de linha ou no fim dela) nao conta.
        let l = l.split("//").next().unwrap_or("");
        let b = l.as_bytes();
        let mut k = 0;
        while k + 1 < b.len() {
            if b[k] == b'0' && b[k + 1] == b'x' && (k == 0 || !b[k - 1].is_ascii_alphanumeric()) {
                let hex: String = l[k + 2..].chars().take_while(|c| c.is_ascii_hexdigit() || *c == '_').collect();
                if hex.len() == 6 || hex.len() == 8 {
                    v.push((i + 1, hex.to_lowercase()));
                }
                k += 2 + hex.len();
            } else {
                k += 1;
            }
        }
    }
    v
}

#[test]
fn no_color_drops_outside_of_the_theme() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let fixed = fixed();
    let mut found = Vec::new();
    let mut all = Vec::new();
    files(&src, &mut all);
    for p in all {
        let rel = p.strip_prefix(&src).unwrap().to_string_lossy().replace('\\', "/");
        if rel == "theme.rs" {
            continue;
        }
        let allowed = fixed.get(rel.as_str()).cloned().unwrap_or_default();
        for (line, hex) in literals(&std::fs::read_to_string(&p).unwrap()) {
            if !allowed.contains(&hex.as_str()) {
                found.push(format!("{rel}:{line}: 0x{hex}"));
            }
        }
    }
    assert!(found.is_empty(), "cores fora do tema (use `tema::atual().<papel>`):\n{}", found.join("\n"));
}

#[test]
fn the_detector_of_literals_finds_hex_and_ignores_the_rest() {
    let found = literals("rgb(0xAABBCC) // 0x112233\nlet n = 0x26; let x = 0x5b8def55; // 0x445566\nlet y = a0xabcdef;");
    assert_eq!(found, vec![(1, "aabbcc".to_string()), (2, "5b8def55".to_string())]);
}
