//! Conteudo da janela "Sobre o Marginalia", sem gpui (regra R4): os textos prontos, o ano e o
//! endereco do site. A view (`main_view/about_screen.rs`) so desenha o que `content` devolve.

use texts::t;

/// Onde o app e distribuido ao publico (a janela Sobre abre este endereco no navegador).
pub const SITE: &str = "https://www.genealogiadoceara.net";

/// Ano dos direitos reservados. Fixo (o do lancamento): atualize a mao quando quiser.
pub const YEAR: i32 = 2026;

/// Tudo o que a janela mostra, ja com os textos do catalogo.
pub struct Content {
    pub title: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub developer: String,
    pub link_label: String,
    pub right: String,
}

pub fn content() -> Content {
    let author = t!("about.name_developer");
    Content {
        title: t!("about.title"),
        name: t!("app.name"),
        version: t!("about.version", env!("CARGO_PKG_VERSION")),
        description: t!("about.description"),
        developer: t!("about.developer", author),
        link_label: t!("about.link"),
        right: t!("about.right", YEAR, author),
    }
}

#[cfg(test)]
#[path = "tests/about.rs"]
mod tests;
