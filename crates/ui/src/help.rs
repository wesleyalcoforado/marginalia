//! Tela de ajuda, sem gpui (regra R4): a tabela de todos os comandos, por
//! assunto, que a view desenha em tres colunas. A coluna `keys` de cada linha
//! e uma lista de "pecas" separadas por espaco (`⌘O`, `←`, `W`, `espaço`); o
//! teste `ajuda_lista_*` cruza essas pecas com as teclas que o app realmente
//! trata (`menus::SHORTCUTS`/`KEYS`, a gaveta, a nota, o seletor, a imagem),
//! entao um atalho novo sem linha aqui derruba o teste. Palavras como "ou" e
//! "arrastar" nao sao teclas: o teste so procura as teclas.

/// Uma linha da ajuda: o que faz e as teclas (ou gestos) que o fazem.
pub struct Line {
    pub description: String,
    pub keys: String,
}

/// Um assunto: id estavel (para os testes), titulo e linhas.
pub struct Group {
    #[cfg_attr(not(test), allow(dead_code))]
    pub id: &'static str,
    pub title: String,
    pub lines: Vec<Line>,
}

fn l(description: String, keys: String) -> Line {
    Line { description, keys }
}

pub const NAVIGATE: &str = "navegar";
pub const IMAGE: &str = "imagem";
pub const HIGHLIGHTS: &str = "grifos";
pub const QUICK_NOTE: &str = "nota";
pub const DRAWERS: &str = "gavetas";
pub const WINDOW: &str = "janela";

/// Todos os grupos, na ordem em que a tela os mostra (colunas da esquerda para a
/// direita). Descricoes e teclas vem do catalogo; as teclas de acoes entram por
/// marcador `{shortcut:id}` (a tabela de `menus.rs`).
pub fn groups() -> Vec<Group> {
    use texts::t;
    vec![
        Group {
            id: NAVIGATE,
            title: t!("help.group.navigate"),
            lines: vec![
            l(t!("help.navigate.image_previous_next"), t!("help.navigate.image_previous_next.keys")),
            l(t!("help.navigate.page_previous_next"), t!("help.navigate.page_previous_next.keys")),
            l(t!("help.navigate.open_source"), t!("help.navigate.open_source.keys")),
            l(t!("help.navigate.open_vault"), t!("help.navigate.open_vault.keys")),
            l(t!("help.navigate.picker_of_vault_choose_open_close"), t!("help.navigate.picker_of_vault_choose_open_close.keys")),
            l(t!("help.navigate.picker_open_folder_create_vault_here"), t!("help.navigate.picker_open_folder_create_vault_here.keys")),
            ],
        },
        Group {
            id: IMAGE,
            title: t!("help.group.image"),
            lines: vec![
            l(t!("help.image.relocate"), t!("help.image.relocate.keys")),
            l(t!("help.image.zoom_adjust"), t!("help.image.zoom_adjust.keys")),
            l(t!("help.image.rotate_the_left_right"), t!("help.image.rotate_the_left_right.keys")),
            l(t!("help.image.adjustments_panel"), t!("help.image.adjustments_panel.keys")),
            l(t!("help.image.brightness"), t!("help.image.brightness.keys")),
            l(t!("help.image.contrast"), t!("help.image.contrast.keys")),
            l(t!("help.image.invert_gray_threshold"), t!("help.image.invert_gray_threshold.keys")),
            l(t!("help.image.restore_adjustments"), t!("help.image.restore_adjustments.keys")),
            l(t!("help.image.textbox_numeric_of_a_bar"), t!("help.image.textbox_numeric_of_a_bar.keys")),
            ],
        },
        Group {
            id: HIGHLIGHTS,
            title: t!("help.group.highlights"),
            lines: vec![
            l(t!("help.highlights.textbox"), t!("help.highlights.textbox.keys")),
            l(t!("help.highlights.arrow"), t!("help.highlights.arrow.keys")),
            l(t!("help.highlights.short_arrow"), t!("help.highlights.short_arrow.keys")),
            l(t!("help.highlights.select"), t!("help.highlights.select.keys")),
            l(t!("help.highlights.erase_highlight"), t!("help.highlights.erase_highlight.keys")),
            l(t!("help.highlights.undo_erase"), t!("help.highlights.undo_erase.keys")),
            l(t!("help.highlights.disable_the_tool_drop_the_highlight"), t!("help.highlights.disable_the_tool_drop_the_highlight.keys")),
            ],
        },
        Group {
            id: QUICK_NOTE,
            title: t!("help.group.quick_note"),
            lines: vec![
            l(t!("help.quick_note.go_to_the_quick_note"), t!("help.quick_note.go_to_the_quick_note.keys")),
            l(t!("help.quick_note.back_the_image"), t!("help.quick_note.back_the_image.keys")),
            l(t!("help.quick_note.new_line"), t!("help.quick_note.new_line.keys")),
            l(t!("help.quick_note.finish"), t!("help.quick_note.finish.keys")),
            l(t!("help.quick_note.walk_by_the_text"), t!("help.quick_note.walk_by_the_text.keys")),
            l(t!("help.quick_note.erase"), t!("help.quick_note.erase.keys")),
            l(t!("help.quick_note.select_all_cut_copy_paste"), t!("help.quick_note.select_all_cut_copy_paste.keys")),
            l(t!("help.quick_note.start_with_creates_task"), t!("help.quick_note.start_with_creates_task.keys")),
            l(t!("help.quick_note.etiquetar_word_in_the_text"), t!("help.quick_note.etiquetar_word_in_the_text.keys")),
            ],
        },
        Group {
            id: DRAWERS,
            title: t!("help.group.drawers"),
            lines: vec![
            l(t!("help.drawers.notes_sources_tasks"), t!("help.drawers.notes_sources_tasks.keys")),
            l(t!("help.drawers.search_in_the_text_of_the_notes"), t!("help.drawers.search_in_the_text_of_the_notes.keys")),
            l(t!("help.drawers.choose_line"), t!("help.drawers.choose_line.keys")),
            l(t!("help.drawers.open_quick_note_task_source"), t!("help.drawers.open_quick_note_task_source.keys")),
            l(t!("help.drawers.delete_quick_note_or_task_asks_confirmation"), t!("help.drawers.delete_quick_note_or_task_asks_confirmation.keys")),
            l(t!("help.drawers.mark_task_done"), t!("help.drawers.mark_task_done.keys")),
            l(t!("help.drawers.close_drawer"), t!("help.drawers.close_drawer.keys")),
            ],
        },
        Group {
            id: WINDOW,
            title: t!("help.group.view_window"),
            lines: vec![
            l(t!("help.view_window.tools_bar"), t!("help.view_window.tools_bar.keys")),
            l(t!("help.view_window.settings"), t!("help.view_window.settings.keys")),
            l(t!("help.view_window.help_is"), t!("help.view_window.help_is.keys")),
            l(t!("help.view_window.exit"), t!("help.view_window.exit.keys")),
            ],
        },
    ]
}

#[cfg(test)]
#[path = "tests/help.rs"]
mod tests;
