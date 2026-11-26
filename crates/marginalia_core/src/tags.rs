//! Etiquetas (`#hashtags`) dentro do texto de uma nota, com a regra do Obsidian para
//! elas tambem aparecerem la: `#` seguido de letras (com acento), digitos, `_` ou `-`,
//! com ao menos um nao-digito, que nao esteja colado a uma palavra antes dele.

use std::collections::BTreeMap;

fn name_character(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

/// `name` (sem o `#`) e uma etiqueta valida: so caracteres de nome e ao menos um nao-digito.
pub fn is_tag(name: &str) -> bool {
    !name.is_empty() && name.chars().all(name_character) && name.chars().any(|c| !c.is_ascii_digit())
}

/// As etiquetas de `text`, sem o `#`, em minusculas, na ordem em que aparecem e sem repetir.
/// Um `#` so abre etiqueta se o caractere antes dele nao e de nome, `/` nem `#` (isso deixa de
/// fora `a#b`, `http://x/#frag` e `## titulo`) e o nome que vem depois e valido.
pub fn tags(text: &str) -> Vec<String> {
    let mut output: Vec<String> = Vec::new();
    let mut previous: Option<char> = None;
    for (i, c) in text.char_indices() {
        if c == '#' && !previous.is_some_and(|a| name_character(a) || a == '/' || a == '#') {
            let rest = &text[i + 1..];
            let name: String = rest.chars().take_while(|&c| name_character(c)).collect();
            if is_tag(&name) {
                let name = name.to_lowercase();
                if !output.contains(&name) {
                    output.push(name);
                }
            }
        }
        previous = Some(c);
    }
    output
}

/// Quantos matizes distintos a paleta tem (15 graus entre um e outro).
const HUES: usize = 24;
/// Com todos os matizes ocupados, uma segunda volta mais escura (ate `2 * MATIZES` cores).
const LUMINOSITIES: [f32; 2] = [0.78, 0.68];

fn tag_hash(name: &str) -> u32 {
    // FNV-1a, depois uma mistura final para nomes parecidos nao cairem em casas vizinhas.
    let mut h: u32 = 0x811c_9dc5;
    for b in name.to_lowercase().bytes() {
        h = (h ^ b as u32).wrapping_mul(0x0100_0193);
    }
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h
}

fn matches_color(matches: usize) -> u32 {
    let hue = (matches % HUES) as f32 * 360.0 / HUES as f32;
    hsl_to_rgb(hue, 0.55, LUMINOSITIES[(matches / HUES) % LUMINOSITIES.len()])
}

/// Cor pastel (`0xRRGGBB`) de cada etiqueta de `names`, **sem repetir cor** enquanto houver
/// casa livre (ate 48 etiquetas). Cada nome parte de uma casa que sai do hash dele (a mesma
/// etiqueta tende a ter sempre a mesma cor); se a casa ja foi tomada por outra etiqueta
/// (nomes em ordem alfabetica, para o resultado nao depender da ordem de chegada), anda
/// de 7 em 7 casas ate achar uma livre. Passando todas as etiquetas do vault, o mesmo
/// conjunto sempre da as mesmas cores; so mudam se o conjunto muda.
pub fn colors_pastel(names: &[String]) -> BTreeMap<String, u32> {
    const TOTAL: usize = HUES * LUMINOSITIES.len();
    // 7 e coprimo com 24 e com 48: o passo visita todas as casas.
    const STEP: usize = 7;
    let mut sorted: Vec<&String> = names.iter().collect();
    sorted.sort();
    sorted.dedup();
    let mut busy = [false; TOTAL];
    let mut colors = BTreeMap::new();
    for name in sorted {
        let preferred = tag_hash(name) as usize % HUES;
        let matches = (0..TOTAL).map(|k| (preferred + k * STEP) % TOTAL).find(|&c| !busy[c]).unwrap_or(preferred);
        busy[matches] = true;
        colors.insert(name.clone(), matches_color(matches));
    }
    colors
}

/// Cor de uma etiqueta sozinha (a casa que o hash do nome indica).
pub fn color_pastel(name: &str) -> u32 {
    matches_color(tag_hash(name) as usize % HUES)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> u32 {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let byte = |v: f32| ((v + m) * 255.0).round() as u32;
    (byte(r) << 16) | (byte(g) << 8) | byte(b)
}

#[cfg(test)]
#[path = "tests/tags.rs"]
mod tests;
