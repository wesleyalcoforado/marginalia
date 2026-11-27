use super::*;
use std::path::PathBuf;

fn data() -> AppData {
    AppData::new(PathBuf::from("/x"))
}

fn active(app_data: &AppData, id: Id) -> Vec<bool> {
    let line = sections(app_data).into_iter().flat_map(|s| s.lines).find(|l| l.id == id).unwrap();
    line.options.iter().map(|o| o.active).collect()
}

#[test]
fn all_line_has_exactly_a_option_active() {
    let d = data();
    for s in sections(&d) {
        assert!(!s.title.is_empty());
        for l in s.lines {
            assert!(!l.title.is_empty());
            assert!(l.options.len() >= 2, "{:?}", l.id);
            assert_eq!(l.options.iter().filter(|o| o.active).count(), 1, "{:?}", l.id);
        }
    }
}

#[test]
fn reopen_the_last_source_starts_in_not_and_changes_by_the_screen() {
    let mut d = data();
    assert_eq!(active(&d, Id::ReopenLastSource), vec![false, true]);
    assert!(choose(&mut d, Id::ReopenLastSource, 0));
    assert!(d.reopen_last_source());
    assert_eq!(active(&d, Id::ReopenLastSource), vec![true, false]);
    // Escolher a que ja vale ou uma opcao que nao existe nao muda nada.
    assert!(!choose(&mut d, Id::ReopenLastSource, 0));
    assert!(!choose(&mut d, Id::ReopenLastSource, 7));
    assert!(d.reopen_last_source());
    assert!(choose(&mut d, Id::ReopenLastSource, 1));
    assert!(!d.reopen_last_source());
}

#[test]
fn the_screen_not_uses_terms_technical() {
    let d = data();
    for s in sections(&d) {
        let mut texts = vec![s.title.clone()];
        for l in &s.lines {
            texts.push(l.title.clone());
            texts.extend(l.hint.clone());
            texts.extend(l.options.iter().map(|o| o.label.clone()));
        }
        for text in texts {
            let lowercase = text.to_lowercase();
            for forbidden in ["debounce", "ms", "milissegundo", "appdata", "json"] {
                assert!(!lowercase.split_whitespace().any(|p| p == forbidden), "{forbidden:?} em {text:?}");
            }
        }
    }
}

#[test]
fn rhythm_of_the_quick_note_starts_normal_and_as_three_options_change_the_data() {
    use marginalia_core::app_data::QuickNoteRhythm;
    let mut d = data();
    assert_eq!(active(&d, Id::QuickNoteRhythm), vec![false, true, false]);
    assert!(choose(&mut d, Id::QuickNoteRhythm, 0));
    assert_eq!(d.quick_note_rhythm(), QuickNoteRhythm::Fast);
    assert_eq!(active(&d, Id::QuickNoteRhythm), vec![true, false, false]);
    assert!(choose(&mut d, Id::QuickNoteRhythm, 2));
    assert_eq!(d.quick_note_rhythm(), QuickNoteRhythm::Calm);
    assert!(!choose(&mut d, Id::QuickNoteRhythm, 2));
    assert!(!choose(&mut d, Id::QuickNoteRhythm, 3));
    assert_eq!(d.quick_note_rhythm(), QuickNoteRhythm::Calm);
}

#[test]
fn font_of_the_quick_note_starts_media_and_as_three_options_change_the_data() {
    use marginalia_core::app_data::FontSize;
    let mut d = data();
    assert_eq!(active(&d, Id::QuickNoteFont), vec![false, true, false]);
    assert!(choose(&mut d, Id::QuickNoteFont, 2));
    assert_eq!(d.quick_note_font(), FontSize::Large);
    assert!(choose(&mut d, Id::QuickNoteFont, 0));
    assert_eq!(d.quick_note_font(), FontSize::Small);
    assert!(!choose(&mut d, Id::QuickNoteFont, 0));
    assert!(!choose(&mut d, Id::QuickNoteFont, 9));
}

#[test]
fn width_of_the_panel_ready_default_and_dragged() {
    let mut d = data();
    // Padrao: "Medio" ativo e nada gravado.
    assert_eq!(active(&d, Id::PanelWidth), vec![false, true, false]);
    assert!(choose(&mut d, Id::PanelWidth, 2));
    assert_eq!(d.drawers_width(), Some(PANEL_WIDTHS[2]));
    assert_eq!(active(&d, Id::PanelWidth), vec![false, false, true]);
    assert!(!choose(&mut d, Id::PanelWidth, 2));
    assert!(choose(&mut d, Id::PanelWidth, 0));
    assert_eq!(d.drawers_width(), Some(PANEL_WIDTHS[0]));
    // Voltar ao medio limpa o dado (a media e a padrao, nao fica gravada).
    assert!(choose(&mut d, Id::PanelWidth, 1));
    assert_eq!(d.drawers_width(), None);
    assert!(!choose(&mut d, Id::PanelWidth, 1));
    assert!(!choose(&mut d, Id::PanelWidth, 5));
}

#[test]
fn width_dragged_the_hand_not_mark_no_and_the_hint_warns() {
    let mut d = data();
    let hint_default = sections(&d).into_iter().flat_map(|s| s.lines).find(|l| l.id == Id::PanelWidth).unwrap().hint;
    d.set_width_drawers(Some(333.0));
    assert_eq!(active(&d, Id::PanelWidth), vec![false, false, false]);
    let hint_dragged = sections(&d).into_iter().flat_map(|s| s.lines).find(|l| l.id == Id::PanelWidth).unwrap().hint;
    assert_ne!(hint_default, hint_dragged);
    // Escolher uma pronta volta a marcar uma.
    assert!(choose(&mut d, Id::PanelWidth, 1));
    assert_eq!(active(&d, Id::PanelWidth), vec![false, true, false]);
}

#[test]
fn restore_patterns_back_each_line_the_option_initial() {
    let mut d = data();
    for (id, i) in [(Id::Theme, 1), (Id::QuickNoteRhythm, 2), (Id::QuickNoteFont, 2), (Id::PanelWidth, 2), (Id::ReopenLastSource, 0)] {
        assert!(choose(&mut d, id, i), "{id:?}");
    }
    d.restore_settings();
    let default = data();
    for s in sections(&default) {
        for l in s.lines {
            let expected: Vec<bool> = l.options.iter().map(|o| o.active).collect();
            assert_eq!(active(&d, l.id), expected, "{:?}", l.id);
        }
    }
}

#[test]
fn theme_starts_dark_and_as_three_options_change_the_data() {
    use marginalia_core::app_data::Theme;
    let mut d = data();
    assert_eq!(active(&d, Id::Theme), vec![true, false, false]);
    assert!(choose(&mut d, Id::Theme, 1));
    assert_eq!(d.theme(), Theme::Light);
    assert_eq!(active(&d, Id::Theme), vec![false, true, false]);
    assert!(choose(&mut d, Id::Theme, 2));
    assert_eq!(d.theme(), Theme::Sepia);
    assert!(!choose(&mut d, Id::Theme, 2));
    assert!(!choose(&mut d, Id::Theme, 3));
    assert!(choose(&mut d, Id::Theme, 0));
    assert_eq!(d.theme(), Theme::Dark);
}

#[test]
fn update_check_line_reflects_and_changes_data() {
    let mut d = data();
    assert_eq!(active(&d, Id::UpdateCheck), vec![true, false]);
    assert!(choose(&mut d, Id::UpdateCheck, 1));
    assert!(!d.auto_update_check());
    assert_eq!(active(&d, Id::UpdateCheck), vec![false, true]);
    assert!(!choose(&mut d, Id::UpdateCheck, 1), "nada mudou");
    assert!(!choose(&mut d, Id::UpdateCheck, 7), "indice invalido");
    d.restore_settings();
    assert!(d.auto_update_check());
}
