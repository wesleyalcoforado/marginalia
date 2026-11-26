use super::*;
use crate::drawers::Drawer;
use crate::focus_state::{self, Focus};
use crate::menus::{self, SHORTCUTS, KEYS};

/// As pecas (teclas) de um grupo, em minusculas.
fn pieces(id: &str) -> Vec<String> {
    crate::menus::register_app_shortcuts();
    let groups = groups();
    let g = groups.iter().find(|g| g.id == id).unwrap_or_else(|| panic!("grupo {id}"));
    g.lines.iter().flat_map(|l| l.keys.split_whitespace().map(str::to_lowercase).collect::<Vec<_>>()).collect()
}

fn all() -> Vec<String> {
    groups().iter().flat_map(|g| pieces(g.id)).collect()
}

/// Como a ajuda escreve uma tecla (`key` do gpui), com ou sem Cmd.
fn piece(key: &str, cmd: bool) -> String {
    let t = match key {
        "space" => "espaço".to_string(),
        "delete" => "⌦".to_string(),
        k if cmd => menus::symbol(&format!("cmd-{k}")),
        k => menus::symbol(k),
    };
    let t = if cmd && key == "delete" { format!("⌘{t}") } else { t };
    t.to_lowercase()
}

#[test]
fn groups_no_line_empty_and_with_title_single() {
    let mut titles = std::collections::HashSet::new();
    crate::menus::register_app_shortcuts();
    for g in &groups() {
        assert!(titles.insert(g.title.clone()), "{} repetido", g.title);
        assert!(!g.lines.is_empty(), "{} sem linhas", g.title);
        for l in &g.lines {
            assert!(!l.description.trim().is_empty() && !l.keys.trim().is_empty(), "{}: linha vazia", g.title);
        }
    }
}

#[test]
fn help_list_whole_shortcut_of_the_menus() {
    let all = all();
    for (t, a, _) in SHORTCUTS {
        assert!(all.contains(&menus::symbol(t).to_lowercase()), "ajuda sem {t} ({a:?})");
    }
    // `=` e `_` sao apelidos de `+` e `-` na mesma linha.
    for (t, a) in KEYS.iter().filter(|(t, _)| !matches!(*t, "=" | "_")) {
        assert!(all.contains(&menus::symbol(t).to_lowercase()), "ajuda sem a tecla {t} ({a:?})");
    }
}

#[test]
fn help_list_as_keys_of_the_drawer_of_the_quick_note_and_of_the_picker() {
    let (drawers, quick_note, navigate, image) = (pieces(DRAWERS), pieces(QUICK_NOTE), pieces(NAVIGATE), pieces(IMAGE));
    let highlights = pieces(HIGHLIGHTS);
    let mut candidates: Vec<String> = ('a'..='z').chain('0'..='9').chain("[]{}+-=_/".chars()).map(String::from).collect();
    candidates.extend(
        ["up", "down", "left", "right", "enter", "escape", "backspace", "delete", "space", "home", "end"].map(String::from),
    );
    let mut checked = 0;
    for k in &candidates {
        for cmd in [false, true] {
            let p = piece(k, cmd);
            for g in Drawer::ALL {
                if focus_state::drawer_key(k, Focus::Drawer, Some(g), false, cmd, false, false).is_some() {
                    assert!(drawers.contains(&p), "ajuda (Gavetas) sem {p} ({g:?})");
                    checked += 1;
                }
            }
            let arrow = matches!(k.as_str(), "up" | "down" | "left" | "right" | "home" | "end");
            let nt = |c| crate::quick_note::key(k, false, c, false, false);
            if !(cmd && (arrow || nt(true) == nt(false))) {
                if let Some(t) = nt(cmd) {
                    if t != crate::quick_note::Key::Ignore {
                        assert!(quick_note.contains(&p), "ajuda (Nota rápida) sem {p}");
                        checked += 1;
                    }
                }
            }
            if !cmd {
                if crate::vault_picker::picker_key(k).is_some() {
                    assert!(navigate.contains(&p), "ajuda (Navegar) sem {p} (seletor)");
                    checked += 1;
                }
                if crate::slider::key_step(k, false).is_some() {
                    assert!(image.contains(&p), "ajuda (Imagem) sem {p} (caixa numérica)");
                    checked += 1;
                }
                if focus_state::pan_key(k, Focus::Image, false, false, false, false).is_some() {
                    assert!(image.contains(&p), "ajuda (Imagem) sem {p} (mover)");
                    checked += 1;
                }
                if focus_state::highlight_key(k, Focus::Image, false, false, false, false, true, true).is_some() {
                    assert!(highlights.contains(&p), "ajuda (Grifos) sem {p}");
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 30, "poucas teclas conferidas: {checked}");
    // Teclas tratadas direto na view: `/` (nota) e Cmd-Z (grifo).
    for (group, p) in [(&quick_note, "/"), (&highlights, "⌘z")] {
        assert!(group.contains(&p.to_string()), "ajuda sem {p}");
    }
}

/// A tela de ajuda como era antes de os textos irem para o catalogo: nada visivel muda.
#[test]
fn help_has_as_lines_of_today() {
    crate::menus::register_app_shortcuts();
    let mut s = String::new();
    for g in &groups() {
        s.push_str(&format!("## {}\n", g.title));
        for l in &g.lines {
            s.push_str(&format!("{}|{}\n", l.description, l.keys));
        }
    }
    assert_eq!(s, include_str!("golden_help.txt"));
}
