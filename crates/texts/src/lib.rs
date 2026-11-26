//! Textos de usuario fora do codigo: catalogo em portugues, busca por chave e
//! marcadores. Sem dependencias do projeto e sem gpui.
//!
//! - [`t!`]: `t!("key")`, `t!("chave", nome = valor, ...)` ou `t!("chave", v0, v1, ...)`
//!   (os posicionais enchem `{0}`, `{1}`...). Chave ausente devolve a propria chave.
//! - `{shortcut:ID}` num texto vira a tecla da acao `ID`, pelo resolvedor que a
//!   interface registra ([`register_shortcuts`]); sem resolvedor aparece `[ID]`.
//! - `{{` e `}}` dao chaves literais; um valor nunca e relido como marcador.

mod catalog;
mod model;

use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

pub use catalog::parse;
pub use model::expand;

static CATALOG: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    catalog::parse(include_str!("../languages/pt.txt")).unwrap_or_else(|e| panic!("{e}"))
});

type Resolver = fn(&str) -> Option<String>;

static SHORTCUTS: RwLock<Option<Resolver>> = RwLock::new(None);

/// Registra a funcao que transforma o id de uma acao na tecla dela (`abrir_fonte`
/// -> `⌘O`). A ultima chamada vale; chamar de novo com a mesma funcao e inofensivo.
pub fn register_shortcuts(f: Resolver) {
    *SHORTCUTS.write().unwrap_or_else(|e| e.into_inner()) = Some(f);
}

pub(crate) fn resolver_shortcut(id: &str) -> Option<String> {
    let f = (*SHORTCUTS.read().unwrap_or_else(|e| e.into_inner()))?;
    f(id)
}

/// O texto bruto de `key` (sem preencher nada), ou a propria chave se nao existe.
pub fn search(key: &str) -> String {
    CATALOG.get(key).cloned().unwrap_or_else(|| key.to_string())
}

/// O texto de `key` com os valores e os atalhos preenchidos.
pub fn fill_in(key: &str, values: &[(&str, String)]) -> String {
    match CATALOG.get(key) {
        Some(model) => expand(model, values),
        None => key.to_string(),
    }
}

/// `t!("key")`, `t!("chave", nome = valor, ...)` ou `t!("chave", v0, v1, ...)`.
/// A chave tem de ser um literal: assim os testes acham todas no codigo.
#[macro_export]
macro_rules! t {
    ($key:literal) => {
        $crate::fill_in($key, &[])
    };
    ($key:literal, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::fill_in($key, &[$((stringify!($name), ($value).to_string())),+])
    };
    ($key:literal, $($value:expr),+ $(,)?) => {{
        let mut _i = 0usize;
        let mut _v: Vec<(String, String)> = Vec::new();
        $( _v.push((_i.to_string(), ($value).to_string())); _i += 1; )+
        let _r: Vec<(&str, String)> = _v.iter().map(|(n, v)| (n.as_str(), v.clone())).collect();
        $crate::fill_in($key, &_r)
    }};
}

#[cfg(test)]
#[path = "tests/lib.rs"]
mod tests;
