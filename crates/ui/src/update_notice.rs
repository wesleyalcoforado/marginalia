//! O que a janela do aviso de versao mostra, sem gpui (regra R4): textos, botoes e o trecho
//! limpo das notas da release. A view (`main_view/update_screen.rs`) so desenha e trata os cliques.

use marginalia_core::update::{Notice, Version};
use texts::t;

/// Quantas linhas das notas aparecem e quantos caracteres, no maximo.
const NOTES_LINES: usize = 4;
const NOTES_CHARS: usize = 300;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Download,
    Later,
    Skip,
    Ok,
}

/// Tudo o que a janela mostra, ja com os textos do catalogo.
pub struct Content {
    pub title: String,
    pub heading: String,
    pub detail: String,
    pub notes: String,
    pub buttons: Vec<Button>,
}

pub fn label(b: Button) -> String {
    match b {
        Button::Download => t!("update.button.download"),
        Button::Later => t!("update.button.later"),
        Button::Skip => t!("update.button.skip"),
        Button::Ok => t!("update.button.ok"),
    }
}

/// As primeiras linhas das notas, sem marcas de titulo/lista do markdown, cortadas em 300 caracteres.
fn clean_notes(body: &str) -> String {
    let lines: Vec<&str> = body
        .lines()
        .map(|l| l.trim().trim_start_matches(['#', '*', '-']).trim())
        .filter(|l| !l.is_empty())
        .take(NOTES_LINES)
        .collect();
    let text = lines.join("\n");
    if text.chars().count() > NOTES_CHARS {
        let cut: String = text.chars().take(NOTES_CHARS).collect();
        format!("{cut}…")
    } else {
        text
    }
}

/// O conteudo para o aviso; `None` quando nao ha nada a mostrar.
pub fn content(notice: &Notice, current: Version) -> Option<Content> {
    let title = t!("update.title");
    match notice {
        Notice::Nothing => None,
        Notice::Show(r) => Some(Content {
            title,
            heading: t!("update.new.heading"),
            detail: t!("update.new.versions", current, r.version),
            notes: clean_notes(&r.notes),
            buttons: vec![Button::Download, Button::Later, Button::Skip],
        }),
        Notice::UpToDate => Some(Content {
            title,
            heading: t!("update.none.heading"),
            detail: t!("update.none.detail", current),
            notes: String::new(),
            buttons: vec![Button::Ok],
        }),
        Notice::Failed => Some(Content {
            title,
            heading: t!("update.failed.heading"),
            detail: t!("update.failed.detail"),
            notes: String::new(),
            buttons: vec![Button::Ok],
        }),
    }
}

#[cfg(test)]
#[path = "tests/update_notice.rs"]
mod tests;
