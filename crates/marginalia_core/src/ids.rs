use chrono::NaiveDateTime;
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

const MAX_WORDS: usize = 6;
const MAX_CHARS: usize = 40;

/// Slug seguro para nome de arquivo: ASCII, minúsculo, até 6 palavras e 40 caracteres.
pub fn slug(text: &str) -> String {
    // NFKD: decompõe acentos e também formas de compatibilidade como "ª" -> "a".
    let folded: String = text
        .nfkd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(|c| c.to_lowercase())
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let words: Vec<&str> = folded.split('-').filter(|w| !w.is_empty()).take(MAX_WORDS).collect();
    let mut out = words.join("-");
    if out.len() > MAX_CHARS {
        out.truncate(MAX_CHARS);
        while out.ends_with('-') {
            out.pop();
        }
    }
    if out.is_empty() {
        NO_TEXT.to_string()
    } else {
        out
    }
}

/// `YYYYMMDD-HHMM-<slug>`; em colisão acrescenta `-2`, `-3`, ...
pub fn make_id(now: NaiveDateTime, text: &str, taken: &dyn Fn(&str) -> bool) -> String {
    let base = format!("{}-{}", now.format("%Y%m%d-%H%M"), slug(text));
    if !taken(&base) {
        return base;
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}-{n}");
        if !taken(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// Slug de um texto sem nada aproveitavel (vazio, so pontuacao).
pub const NO_TEXT: &str = "sem-texto";

/// `AAAAMMDD-HHMM-` do id e o minuto que ele representa, se o id comecar assim.
pub fn created_at(id: &str) -> Option<NaiveDateTime> {
    let p = id.get(..13)?;
    let ok = p.char_indices().all(|(i, c)| if i == 8 { c == '-' } else { c.is_ascii_digit() });
    if !ok || id.as_bytes().get(13) != Some(&b'-') {
        return None;
    }
    NaiveDateTime::parse_from_str(&format!("{p}00"), "%Y%m%d-%H%M%S").ok()
}

/// Forma de id provisorio: `AAAAMMDD-HHMM-no-text` ou com sufixo de
/// colisao (`-2`, `-3`, ...). So a forma: quem decide se um documento ainda e
/// rascunho e a marca `rascunho: true` no frontmatter (ver
/// `Vault::finish_id_in`), porque uma nota do usuario criada sem texto tem
/// esta mesma forma e nunca deve ser renomeada.
pub fn eh_provisional(id: &str) -> bool {
    if created_at(id).is_none() {
        return false;
    }
    match id[14..].strip_prefix(NO_TEXT) {
        Some("") => true,
        Some(r) => r.strip_prefix('-').is_some_and(|n| !n.is_empty() && !n.starts_with('0') && n.bytes().all(|b| b.is_ascii_digit())),
        None => false,
    }
}

#[cfg(test)]
#[path = "tests/ids.rs"]
mod tests;
