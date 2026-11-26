use super::*;

#[test]
fn esc_disables_so_with_the_image_in_focus_and_no_modifier() {
    assert!(esc_disables_tool("escape", Focus::Image, false, false, false, false, true));
    assert!(!esc_disables_tool("escape", Focus::Image, false, false, false, false, false), "nada ligado");
    for f in [Focus::QuickNote, Focus::Drawer, Focus::Other, Focus::Number] {
        assert!(!esc_disables_tool("escape", f, false, false, false, false, true), "{f:?}");
    }
    for (shift, cmd, ctrl, alt) in [(true, false, false, false), (false, true, false, false), (false, false, true, false), (false, false, false, true)] {
        assert!(!esc_disables_tool("escape", Focus::Image, shift, cmd, ctrl, alt, true));
    }
    assert!(!esc_disables_tool("a", Focus::Image, false, false, false, false, true));
}

#[test]
fn cmd_k_focuses_the_quick_note_of_any_focus() {
    for f in [Focus::Image, Focus::QuickNote, Focus::Other] {
        assert_eq!(transition(f, Command::FocusQuickNote, true), Transition::To(Focus::QuickNote), "{f:?}");
        // sem vault / sem fonte a caixa esta desligada: nada de foco num campo morto
        assert_eq!(transition(f, Command::FocusQuickNote, false), Transition::Refused, "{f:?}");
    }
}

#[test]
fn cmd_shift_k_back_the_image_of_any_focus() {
    for f in [Focus::Image, Focus::QuickNote, Focus::Other] {
        for enabled in [true, false] {
            assert_eq!(transition(f, Command::FocusImage, enabled), Transition::To(Focus::Image), "{f:?} {enabled}");
        }
    }
}

#[test]
fn esc_so_removes_the_focus_of_the_quick_note() {
    assert_eq!(transition(Focus::QuickNote, Command::Esc, true), Transition::To(Focus::Image));
    assert_eq!(transition(Focus::QuickNote, Command::Esc, false), Transition::To(Focus::Image));
    assert_eq!(transition(Focus::Image, Command::Esc, true), Transition::Stays, "Esc na imagem nao sai do app");
    assert_eq!(transition(Focus::Other, Command::Esc, true), Transition::Stays, "a caixa Renomear trata o proprio Esc");
}

#[test]
fn forward_and_back_cmd_k_esc() {
    let mut f = Focus::Image;
    for (c, expected) in [
        (Command::FocusQuickNote, Focus::QuickNote),
        (Command::Esc, Focus::Image),
        (Command::FocusQuickNote, Focus::QuickNote),
        (Command::FocusImage, Focus::Image),
        (Command::Esc, Focus::Image),
    ] {
        if let Transition::To(n) = transition(f, c, true) {
            f = n;
        }
        assert_eq!(f, expected, "{c:?}");
    }
}

#[test]
fn wasd_move_the_region_visible() {
    // (dx, dy) em passos para `viewer_math::pan_by`, que move a IMAGEM com o
    // ponteiro: W mostra o que esta acima (a imagem desce, dy > 0), S o que esta
    // abaixo, A a esquerda (a imagem vai para a direita, dx > 0), D a direita.
    let no = |k| pan_key(k, Focus::Image, false, false, false, false);
    assert_eq!(no("w"), Some((0.0, 1.0)));
    assert_eq!(no("s"), Some((0.0, -1.0)));
    assert_eq!(no("a"), Some((1.0, 0.0)));
    assert_eq!(no("d"), Some((-1.0, 0.0)));
    assert_eq!(no("W"), Some((0.0, 1.0)), "Caps Lock ligado");
    for k in ["q", "e", "up", "left", "f", "space", "+", "0"] {
        assert_eq!(no(k), None, "{k}");
    }
}

#[test]
fn wasd_never_with_field_of_text_in_focus() {
    for k in ["w", "a", "s", "d"] {
        assert_eq!(pan_key(k, Focus::QuickNote, false, false, false, false), None, "nota: {k} e texto");
        assert_eq!(pan_key(k, Focus::Other, false, false, false, false), None, "renomear/gaveta: {k} e texto");
    }
}

const ALL: [Focus; 4] = [Focus::Image, Focus::QuickNote, Focus::Other, Focus::Drawer];
const MODS: [(bool, bool, bool, bool); 5] =
    [(false, false, false, false), (true, false, false, false), (false, true, false, false), (false, false, true, false), (false, false, false, true)];

#[test]
fn wasd_never_and_key_of_the_drawer_in_none_focus() {
    // I1: W/A/S/D nunca descartam, promovem nem mexem numa linha, com qualquer foco e gaveta.
    for f in ALL {
        for g in Drawer::ALL {
            for k in ["w", "a", "s", "d", "W", "A", "S", "D"] {
                for (shift, cmd, ctrl, alt) in MODS {
                    assert_eq!(drawer_key(k, f, Some(g), shift, cmd, ctrl, alt), None, "{k} {f:?} {g:?}");
                }
            }
        }
    }
    // e com a gaveta em foco tambem nao movem a imagem
    for k in ["w", "a", "s", "d"] {
        assert_eq!(pan_key(k, Focus::Drawer, false, false, false, false), None);
    }
}

#[test]
fn backspace_in_the_drawer_of_notes_and_of_tasks_asks_deletion() {
    let k = |g| drawer_key("backspace", Focus::Drawer, Some(g), false, false, false, false);
    assert_eq!(k(Drawer::Notes), Some(DrawerAction::Delete));
    assert_eq!(k(Drawer::Tasks), Some(DrawerAction::Delete));
    assert_eq!(k(Drawer::Sources), None, "fontes nao se excluem");
    // com a imagem ou um campo de texto em foco, Backspace nao e da gaveta
    for f in [Focus::Image, Focus::QuickNote, Focus::Other] {
        assert_eq!(drawer_key("backspace", f, Some(Drawer::Notes), false, false, false, false), None, "{f:?}");
    }
    // segurada nao repete: um Backspace so abre uma confirmacao
    assert!(!repetition_accepts(DrawerAction::Delete));
}

#[test]
fn cmd_backspace_not_makes_more_nothing() {
    for g in Drawer::ALL {
        assert_eq!(drawer_key("backspace", Focus::Drawer, Some(g), false, true, false, false), None, "{g:?}");
    }
}

#[test]
fn keys_of_the_drawer_so_with_ela_in_focus() {
    let keys = ["up", "down", "enter", "escape", "p", "e", "x", "space", "backspace"];
    for g in Drawer::ALL {
        for f in [Focus::Image, Focus::QuickNote, Focus::Other] {
            for k in keys {
                assert_eq!(drawer_key(k, f, Some(g), false, false, false, false), None, "{k} {f:?} {g:?}");
            }
            assert_eq!(drawer_key("backspace", f, Some(g), false, true, false, false), None, "Cmd+Backspace {f:?}");
        }
        // gaveta fechada: nada, mesmo com o foco nela (o foco volta a imagem ao fechar)
        assert_eq!(drawer_key("enter", Focus::Drawer, None, false, false, false, false), None);
    }
    let g = |k, cmd| drawer_key(k, Focus::Drawer, Some(Drawer::Notes), false, cmd, false, false);
    assert_eq!(g("enter", false), Some(DrawerAction::Open));
    assert_eq!(g("down", false), Some(DrawerAction::Relocate(1)));
    assert_eq!(g("escape", false), Some(DrawerAction::Close));
    assert_eq!((g("p", false), g("e", false)), (None, None), "promover e editar acabaram");
    let t = |k| drawer_key(k, Focus::Drawer, Some(Drawer::Tasks), false, false, false, false);
    assert_eq!((t("x"), t("space")), (Some(DrawerAction::ToggleDone), Some(DrawerAction::ToggleDone)));
    // com Shift/Ctrl/Alt nada
    assert_eq!(drawer_key("enter", Focus::Drawer, Some(Drawer::Notes), true, false, false, false), None);
    assert_eq!(drawer_key("backspace", Focus::Drawer, Some(Drawer::Notes), false, false, false, true), None);
}

#[test]
fn focus_in_the_drawer_leaves_with_cmd_shift_k_or_cmd_k() {
    assert_eq!(transition(Focus::Drawer, Command::FocusImage, true), Transition::To(Focus::Image));
    assert_eq!(transition(Focus::Drawer, Command::FocusQuickNote, true), Transition::To(Focus::QuickNote));
    assert_eq!(transition(Focus::Drawer, Command::Esc, true), Transition::Stays, "Esc na gaveta e dela (fecha o menu ou a gaveta)");
}

#[test]
fn key_held_so_walks_by_the_list() {
    assert!(repetition_accepts(DrawerAction::Relocate(1)));
    assert!(repetition_accepts(DrawerAction::Relocate(-1)));
    for a in [
        DrawerAction::Delete,
        DrawerAction::Open,
        DrawerAction::ToggleDone,
        DrawerAction::Close,
    ] {
        assert!(!repetition_accepts(a), "{a:?} segurada nao repete");
    }
}

#[test]
fn focus_goes_the_textbox_so_when_the_text_arrives() {
    let mut f = FocusToTheLoad::default();
    // sem pedido: nada
    assert!(!f.after(true, false));
    f.request();
    assert!(f.pending());
    // carregando (sessao ou abertura de fonte adiada): o foco espera
    assert!(!f.after(false, true));
    assert!(!f.after(false, true));
    assert!(f.pending());
    // o texto chegou: foca uma vez so
    assert!(f.after(true, false));
    assert!(!f.pending());
    assert!(!f.after(true, false), "uma vez so");
    // a carga falhou ou foi cancelada (nada carregando, nada chegou): o pedido cai
    f.request();
    assert!(!f.after(false, false));
    assert!(!f.pending(), "falhou: o foco fica onde estava");
    assert!(!f.after(true, false), "um texto que chegue depois nao rouba o foco");
    // pedir de novo (outro clique) substitui o anterior
    f.request();
    f.request();
    assert!(f.after(true, true), "chegou (mesmo com outra coisa carregando)");
}

#[test]
fn delete_backspace_and_cmd_z_so_with_the_image_in_focus() {
    let t = |k, f, cmd, sel, undo| highlight_key(k, f, false, cmd, false, false, sel, undo);
    assert_eq!(t("delete", Focus::Image, false, true, false), Some(HighlightAction::Delete));
    assert_eq!(t("backspace", Focus::Image, false, true, false), Some(HighlightAction::Delete));
    assert_eq!(t("z", Focus::Image, true, false, true), Some(HighlightAction::Undo));
    assert_eq!(t("escape", Focus::Image, false, true, false), Some(HighlightAction::Deselect));
    // sem selecao / sem nada para desfazer: a tecla nao e de grifo
    assert_eq!(t("delete", Focus::Image, false, false, true), None);
    assert_eq!(t("backspace", Focus::Image, false, false, true), None);
    assert_eq!(t("z", Focus::Image, true, true, false), None);
    assert_eq!(t("escape", Focus::Image, false, false, true), None, "Esc sem selecao: o significado de sempre");
    // nunca com um campo de texto (nota, renomear) nem com a gaveta em foco
    for f in [Focus::QuickNote, Focus::Other, Focus::Drawer] {
        for (k, cmd) in [("delete", false), ("backspace", false), ("z", true), ("escape", false)] {
            assert_eq!(t(k, f, cmd, true, true), None, "{k} {f:?}");
        }
    }
}

#[test]
fn keys_of_highlight_with_modifiers_wrong_not_count() {
    let t = |k, shift, cmd, ctrl, alt| highlight_key(k, Focus::Image, shift, cmd, ctrl, alt, true, true);
    assert_eq!(t("z", false, false, false, false), None, "z sozinho nao desfaz");
    assert_eq!(t("z", true, true, false, false), None, "Cmd-Shift-Z nao e desfazer");
    assert_eq!(t("z", false, true, true, false), None);
    assert_eq!(t("z", false, true, false, true), None);
    assert_eq!(t("Z", false, true, false, false), Some(HighlightAction::Undo), "Caps Lock");
    for (shift, cmd, ctrl, alt) in [(true, false, false, false), (false, true, false, false), (false, false, true, false), (false, false, false, true)] {
        assert_eq!(t("delete", shift, cmd, ctrl, alt), None, "{shift} {cmd} {ctrl} {alt}");
        assert_eq!(t("backspace", shift, cmd, ctrl, alt), None, "Cmd+Backspace e da gaveta");
        assert_eq!(t("escape", shift, cmd, ctrl, alt), None);
    }
    for k in ["x", "d", "u", "enter"] {
        assert_eq!(t(k, false, false, false, false), None, "{k}");
    }
}

#[test]
fn undo_never_touches_in_highlight_typing_and_prefers_the_highlight_in_the_image() {
    use TargetUndo::*;
    let target_undo = |f, t, g| target_undo(f, t, g, false);
    // num campo de texto: nada (a caixa nao tem desfazer; o grifo fica)
    for f in [Focus::QuickNote, Focus::Other] {
        assert_eq!(target_undo(f, true, true), None, "{f:?}");
    }
    assert_eq!(target_undo(Focus::Image, false, true), Some(Highlight));
    // so o grifo, e so com a imagem em foco
    assert_eq!(target_undo(Focus::Drawer, false, true), None);
    assert_eq!(target_undo(Focus::Other, false, true), None, "sem foco");
    assert_eq!(target_undo(Focus::Image, false, false), None);
}

/// Com a confirmacao de excluir aberta na gaveta em foco, Cmd+Z nao desfaz nada.
#[test]
fn undo_not_is_worth_with_confirmation_open_in_the_drawer() {
    assert_eq!(target_undo(Focus::Drawer, false, true, true), None);
    // fora da gaveta o modal dela nao muda nada
    assert_eq!(target_undo(Focus::Image, false, true, true), Some(TargetUndo::Highlight));
}

/// A caixa numerica de uma barra do painel de ajustes e um campo de texto:
/// W/A/S/D, as teclas da gaveta, as de grifo, os atalhos de uma letra e o
/// Cmd+Z nao disparam enquanto se digita nela.
#[test]
fn textbox_numeric_and_field_of_text() {
    assert!(text_field(Focus::Number, false));
    assert!(text_field(Focus::QuickNote, false));
    assert!(text_field(Focus::Other, true), "a caixa Renomear (Outro) e texto");
    assert!(!text_field(Focus::Image, false));
    assert!(!text_field(Focus::Drawer, false));
    assert!(!text_field(Focus::Other, false), "sem foco: as teclas simples valem");
    for k in ["w", "a", "s", "d"] {
        assert_eq!(pan_key(k, Focus::Number, false, false, false, false), None, "{k}");
    }
    for g in Drawer::ALL {
        for k in ["up", "down", "enter", "escape", "p", "e", "x", "backspace"] {
            assert_eq!(drawer_key(k, Focus::Number, Some(g), false, false, false, false), None, "{k} {g:?}");
        }
    }
    for (k, cmd) in [("delete", false), ("backspace", false), ("z", true), ("escape", false)] {
        assert_eq!(highlight_key(k, Focus::Number, false, cmd, false, false, true, true), None, "{k}");
    }
    assert_eq!(target_undo(Focus::Number, false, true, false), None, "Cmd+Z digitando nao desfaz nada");
    // Esc na caixa: volta a imagem (a caixa devolve o valor antes); Cmd-K vai a nota
    assert_eq!(transition(Focus::Number, Command::Esc, true), Transition::To(Focus::Image));
    assert_eq!(transition(Focus::Number, Command::FocusQuickNote, true), Transition::To(Focus::QuickNote));
    assert_eq!(transition(Focus::Number, Command::FocusImage, true), Transition::To(Focus::Image));
}

#[test]
fn wasd_ignores_keys_with_modifier() {
    for k in ["w", "a", "s", "d", "W"] {
        for (shift, cmd, ctrl, alt) in
            [(true, false, false, false), (false, true, false, false), (false, false, true, false), (false, false, false, true)]
        {
            assert_eq!(pan_key(k, Focus::Image, shift, cmd, ctrl, alt), None, "{k} {shift} {cmd} {ctrl} {alt}");
        }
    }
}
