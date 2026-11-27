//! Tela de Configuracoes, sem gpui (regra R4): as secoes, as linhas e as opcoes de cada
//! linha, e o que escolher uma opcao faz nos dados do app. A view (`main_view/screen_config.rs`)
//! so desenha o que `sections` devolve e chama `choose`; os testes conferem tudo aqui, sem janela.
//!
//! Toda linha e uma lista curta de opcoes prontas (controle segmentado): nada de campo de
//! texto, entao a tela cabe no modelo de foco da tela de ajuda. Para uma configuracao nova:
//! uma variante em [`Id`], uma linha em [`sections`], o ramo em [`choose`] e o campo nos
//! dados do app (com default e em `AppData::restore_settings`).

use crate::drawers;
use marginalia_core::app_data::{AppData, QuickNoteRhythm, FontSize, Theme};
use texts::t;

/// Cada configuracao da tela.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Id {
    Theme,
    QuickNoteRhythm,
    QuickNoteFont,
    PanelWidth,
    ReopenLastSource,
    UpdateCheck,
}

/// As larguras prontas do painel lateral: estreito, medio (o padrao) e largo.
pub const PANEL_WIDTHS: [f32; 3] = [260.0, drawers::WIDTH, 460.0];

/// A largura do painel hoje (a que o usuario arrastou, ou a padrao).
fn panel_width(app_data: &AppData) -> f32 {
    app_data.drawers_width().unwrap_or(drawers::WIDTH)
}

/// Qual das larguras prontas vale hoje (`None`: uma que o usuario arrastou e nao bate com nenhuma).
fn ready_width(app_data: &AppData) -> Option<usize> {
    let current = panel_width(app_data);
    PANEL_WIDTHS.iter().position(|l| (l - current).abs() < 0.5)
}

/// Uma opcao de uma linha; `active` diz qual vale hoje.
pub struct Choice {
    pub label: String,
    pub active: bool,
}

/// Uma configuracao: titulo, dica opcional e as opcoes.
pub struct Line {
    pub id: Id,
    pub title: String,
    pub hint: Option<String>,
    pub options: Vec<Choice>,
}

/// Um assunto da tela, com as linhas dele.
pub struct Section {
    pub title: String,
    pub lines: Vec<Line>,
}

fn choice(label: String, active: bool) -> Choice {
    Choice { label, active }
}

/// As secoes da tela, com a opcao ativa de cada linha lida de `app_data`.
pub fn sections(app_data: &AppData) -> Vec<Section> {
    let reopen = app_data.reopen_last_source();
    let update = app_data.auto_update_check();
    let theme = app_data.theme();
    let rhythm = app_data.quick_note_rhythm();
    let font = app_data.quick_note_font();
    let ready = ready_width(app_data);
    vec![
        Section {
            title: t!("config.section.aparencia"),
            lines: vec![Line {
                id: Id::Theme,
                title: t!("config.theme.title"),
                hint: Some(t!("config.theme.hint")),
                options: Theme::ALL.iter().map(|x| choice(theme_label(*x), *x == theme)).collect(),
            }],
        },
        Section {
            title: t!("config.section.quick_note"),
            lines: vec![Line {
                id: Id::QuickNoteRhythm,
                title: t!("config.rhythm.title"),
                hint: Some(t!("config.rhythm.hint")),
                options: QuickNoteRhythm::ALL.iter().map(|r| choice(rhythm_label(*r), *r == rhythm)).collect(),
            }, Line {
                id: Id::QuickNoteFont,
                title: t!("config.font.title"),
                hint: Some(t!("config.font.hint")),
                options: FontSize::ALL.iter().map(|l| choice(font_label(*l), *l == font)).collect(),
            }],
        },
        Section {
            title: t!("config.section.panel"),
            lines: vec![Line {
                id: Id::PanelWidth,
                title: t!("config.panel.title"),
                hint: Some(if ready.is_some() { t!("config.panel.hint") } else { t!("config.panel.hint_personalizada") }),
                options: (0..PANEL_WIDTHS.len()).map(|i| choice(width_label(i), ready == Some(i))).collect(),
            }],
        },
        Section {
            title: t!("config.section.start"),
            lines: vec![
                Line {
                    id: Id::ReopenLastSource,
                    title: t!("config.reopen.title"),
                    hint: Some(t!("config.reopen.hint")),
                    options: vec![choice(t!("config.choice.yes"), reopen), choice(t!("config.choice.not"), !reopen)],
                },
                Line {
                    id: Id::UpdateCheck,
                    title: t!("config.update.title"),
                    hint: Some(t!("config.update.hint")),
                    options: vec![choice(t!("config.choice.yes"), update), choice(t!("config.choice.not"), !update)],
                },
            ],
        },
    ]
}

fn theme_label(x: Theme) -> String {
    match x {
        Theme::Dark => t!("config.theme.dark"),
        Theme::Light => t!("config.theme.light"),
        Theme::Sepia => t!("config.theme.sepia"),
    }
}

fn font_label(l: FontSize) -> String {
    match l {
        FontSize::Small => t!("config.font.small"),
        FontSize::Media => t!("config.font.media"),
        FontSize::Large => t!("config.font.large"),
    }
}

fn width_label(i: usize) -> String {
    match i {
        0 => t!("config.panel.estreito"),
        1 => t!("config.panel.medium"),
        _ => t!("config.panel.largo"),
    }
}

fn rhythm_label(r: QuickNoteRhythm) -> String {
    match r {
        QuickNoteRhythm::Fast => t!("config.rhythm.fast"),
        QuickNoteRhythm::Normal => t!("config.rhythm.normal"),
        QuickNoteRhythm::Calm => t!("config.rhythm.calm"),
    }
}

/// Escolhe a opcao `index` da linha `id` (a ordem de `Line::options`). Muda so a memoria: quem
/// chama grava os dados do app. Devolve se algo mudou (indice fora da lista ou valor igual: nao).
pub fn choose(app_data: &mut AppData, id: Id, index: usize) -> bool {
    match id {
        Id::Theme => {
            let Some(new) = Theme::ALL.get(index).copied() else { return false };
            let changed = app_data.theme() != new;
            app_data.set_theme(new);
            changed
        }
        Id::QuickNoteRhythm => {
            let Some(new) = QuickNoteRhythm::ALL.get(index).copied() else { return false };
            let changed = app_data.quick_note_rhythm() != new;
            app_data.set_rhythm_quick_note(new);
            changed
        }
        Id::QuickNoteFont => {
            let Some(new) = FontSize::ALL.get(index).copied() else { return false };
            let changed = app_data.quick_note_font() != new;
            app_data.set_font_quick_note(new);
            changed
        }
        Id::PanelWidth => {
            let Some(fresh) = PANEL_WIDTHS.get(index).copied() else { return false };
            let changed = ready_width(app_data) != Some(index);
            // A media e a padrao: nao fica gravada.
            app_data.set_width_drawers((fresh != drawers::WIDTH).then_some(fresh));
            changed
        }
        Id::ReopenLastSource => {
            let new = match index {
                0 => true,
                1 => false,
                _ => return false,
            };
            let changed = app_data.reopen_last_source() != new;
            app_data.set_reopen_last_source(new);
            changed
        }
        Id::UpdateCheck => {
            let new = match index {
                0 => true,
                1 => false,
                _ => return false,
            };
            let changed = app_data.auto_update_check() != new;
            app_data.set_auto_update_check(new);
            changed
        }
    }
}

#[cfg(test)]
#[path = "tests/config.rs"]
mod tests;
