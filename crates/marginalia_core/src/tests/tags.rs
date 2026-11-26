use super::*;

fn e(t: &str) -> Vec<String> {
    tags(t)
}

#[test]
fn text_tags() {
    assert_eq!(e("Batismo #achado #batismo"), ["achado", "batismo"]);
    assert!(e("# Título").is_empty());
    assert!(e("## x").is_empty());
    assert!(e("a#b").is_empty());
    assert!(e("#2024").is_empty());
    assert_eq!(e("#2024a"), ["2024a"]);
    assert!(e("veja http://x/#frag").is_empty());
    assert_eq!(e("#certidão e #A-b_c."), ["certidão", "a-b_c"]);
    assert_eq!(e("#Achado #achado"), ["achado"]);
    assert_eq!(e("#a,b"), ["a"]);
    assert!(e("##x").is_empty());
    assert_eq!(e("linha1\n#um\n#dois"), ["um", "dois"]);
    assert!(e("").is_empty());
}

#[test]
fn and_tag_valid_the_name() {
    assert!(is_tag("achado"));
    assert!(is_tag("a-b_c"));
    assert!(is_tag("certidão"));
    assert!(!is_tag(""));
    assert!(!is_tag("2024"));
    assert!(!is_tag("a b"));
    assert!(!is_tag("a#b"));
}

#[test]
fn color_pastel_and_stable_light_and_distinguishes_tags() {
    assert_eq!(color_pastel("batismo"), color_pastel("Batismo"), "maiusculas nao mudam a cor");
    assert_eq!(color_pastel("batismo"), color_pastel("batismo"));
    for name in ["batismo", "obito", "achado", "pendente", "x", "familia-silva"] {
        let c = color_pastel(name);
        assert!(c <= 0xffffff);
        // pastel: nenhum canal escuro demais, e o brilho geral e alto
        let channels = [(c >> 16) & 0xff, (c >> 8) & 0xff, c & 0xff];
        assert!(channels.iter().all(|&v| v >= 100), "{name}: {c:06x}");
        assert!(channels.iter().max().unwrap() >= &200, "{name}: {c:06x}");
    }
    let colors: std::collections::HashSet<u32> = ["a", "b", "c", "d", "e", "f"].iter().map(|n| color_pastel(n)).collect();
    assert!(colors.len() >= 5, "etiquetas diferentes quase sempre tem cores diferentes");
}

#[test]
fn colors_pastel_not_repeat_ate_exhaust_as_places() {
    let names: Vec<String> = (0..48).map(|i| format!("etiqueta{i}")).collect();
    let colors = colors_pastel(&names);
    assert_eq!(colors.len(), 48);
    let distinct: std::collections::HashSet<u32> = colors.values().copied().collect();
    assert_eq!(distinct.len(), 48, "48 etiquetas, 48 cores");
    // poucas etiquetas tambem nunca repetem (era o defeito do hash puro)
    for n in 2..=24 {
        let c = colors_pastel(&names[..n]);
        let d: std::collections::HashSet<u32> = c.values().copied().collect();
        assert_eq!(d.len(), n, "{n} etiquetas");
    }
}

#[test]
fn colors_pastel_stable_and_alone_uses_the_matches_of_the_hash() {
    let a = vec!["batismo".to_string(), "obito".to_string(), "achado".to_string()];
    let mut b = a.clone();
    b.reverse();
    assert_eq!(colors_pastel(&a), colors_pastel(&b), "ordem de chegada nao importa");
    assert_eq!(colors_pastel(&["batismo".to_string()])["batismo"], color_pastel("batismo"));
    assert!(colors_pastel(&[]).is_empty());
}
