//! Preenchimento de um texto do catalogo: valores, chaves literais e atalhos.

use crate::resolver_shortcut;

/// Preenche `model` numa passada so: `{{` e `}}` sao chaves literais,
/// `{shortcut:ID}` vira a tecla do resolvedor (ou `[ID]` se ele nao a conhece) e
/// `{nome}` vira o valor de mesmo nome (nome desconhecido fica como escrito). O
/// texto de um valor e copiado como esta, nunca relido.
pub fn expand(model: &str, values: &[(&str, String)]) -> String {
    let mut output = String::with_capacity(model.len());
    let mut rest = model;
    while let Some(i) = rest.find(['{', '}']) {
        output.push_str(&rest[..i]);
        rest = &rest[i..];
        if let Some(r) = rest.strip_prefix("{{") {
            output.push('{');
            rest = r;
        } else if let Some(r) = rest.strip_prefix("}}") {
            output.push('}');
            rest = r;
        } else if rest.starts_with('{') {
            match rest.find('}') {
                Some(end) => {
                    let name = &rest[1..end];
                    match name.strip_prefix("shortcut:") {
                        Some(id) => output.push_str(&resolver_shortcut(id).unwrap_or_else(|| format!("[{id}]"))),
                        None => match values.iter().find(|(n, _)| *n == name) {
                            Some((_, v)) => output.push_str(v),
                            None => output.push_str(&rest[..=end]),
                        },
                    }
                    rest = &rest[end + 1..];
                }
                None => {
                    output.push_str(rest);
                    rest = "";
                }
            }
        } else {
            output.push('}');
            rest = &rest[1..];
        }
    }
    output.push_str(rest);
    output
}

#[cfg(test)]
#[path = "tests/model.rs"]
mod tests;
