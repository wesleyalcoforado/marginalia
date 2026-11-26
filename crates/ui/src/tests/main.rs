use super::*;
use std::collections::HashSet;

/// Todo item de menu tem uma acao do gpui, e ida e volta da a mesma acao
/// (uma acao do gpui por item: nenhuma repetida).
#[test]
fn whole_action_id_of_the_tree_becomes_a_action_of_the_gpui_own() {
    let mut names = HashSet::new();
    for m in menus::tree() {
        for i in m.items {
            let ItemDesc::MenuAction { action_id, .. } = i else { continue };
            let a = MenuAction::from_id(action_id).unwrap_or_else(|| panic!("{action_id} sem Acao"));
            let g = action_to_gpui(a);
            assert_eq!(action_from_gpui(g.as_ref()), Some(a), "{action_id}");
            assert!(names.insert(g.name()), "{} repetida", g.name());
        }
    }
    assert_eq!(names.len(), MenuAction::ALL.len());
}

/// Toda `KeyBinding` sai de `menus::SHORTCUTS`: tecla valida, a acao certa e o contexto certo.
#[test]
fn bindings_of_the_shortcuts_match_the_action_and_the_context() {
    for (t, a, c) in menus::SHORTCUTS {
        let b = binding(&menus::gpui_to(t), *a, c.name());
        assert_eq!(action_from_gpui(b.action()), Some(*a), "{t}");
        assert_eq!(b.keystrokes().len(), 1, "{t}: uma tecla so");
        assert_eq!(b.predicate().is_some(), c.name().is_some(), "{t}");
    }
}

/// Fora do macOS as teclas `SoMenu` que a view nao trata viram `KeyBinding` sem contexto: sem isso
/// nada as entrega (Ctrl-F, Ctrl-I, brilho, girar, Configuracoes...).
#[test]
fn shortcuts_no_menu_native_become_bindings_no_context() {
    let list = menus::shortcuts_no_menu_native();
    for a in [MenuAction::AdjustmentsPanel, MenuAction::Settings, MenuAction::RotateLeft, MenuAction::RotateRight, MenuAction::BrightnessUp, MenuAction::BrightnessDown,
              MenuAction::ContrastUp, MenuAction::ContrastDown, MenuAction::Invert, MenuAction::Gray, MenuAction::Threshold, MenuAction::Restore] {
        assert!(list.iter().any(|(_, x)| *x == a), "{a:?} sem binding fora do macOS");
    }
    for a in [MenuAction::Undo, MenuAction::Notes, MenuAction::Sources, MenuAction::Tasks] {
        assert!(!list.iter().any(|(_, x)| *x == a), "{a:?} ja e tratada pela view: nao pode disparar duas vezes");
    }
    for (t, a) in list {
        let b = binding(&menus::gpui_to(t), a, None);
        assert_eq!(action_from_gpui(b.action()), Some(a), "{t}");
        assert!(b.predicate().is_none(), "{t}: sem contexto");
    }
}

/// A barra do gpui espelha a arvore: mesmos menus e itens, rotulos com a
/// tecla simples entre parenteses e os itens de edicao com o `OsAction`.
#[test]
fn menus_of_the_gpui_mirror_the_tree() {
    let tree = menus::tree();
    let bar = menus_gpui();
    assert_eq!(bar.len(), tree.len());
    for (m, d) in bar.iter().zip(&tree) {
        assert_eq!(m.name.as_ref(), d.title);
        assert_eq!(m.items.len(), d.items.len(), "{}", d.title);
        for (i, di) in m.items.iter().zip(&d.items) {
            match (i, di) {
                (MenuItem::Separator, ItemDesc::Separator) => {}
                (MenuItem::Action { name, action, os_action }, ItemDesc::MenuAction { label, action_id, shortcut }) => {
                    assert_eq!(name.as_ref(), menus::menu_label(label, *shortcut));
                    let a = action_from_gpui(action.as_ref()).unwrap();
                    assert_eq!(a.id(), *action_id);
                    let expected = match a {
                        MenuAction::Undo => Some(OsAction::Undo),
                        MenuAction::Cut => Some(OsAction::Cut),
                        MenuAction::Copy => Some(OsAction::Copy),
                        MenuAction::Paste => Some(OsAction::Paste),
                        MenuAction::SelectAll => Some(OsAction::SelectAll),
                        _ => None,
                    };
                    assert!(*os_action == expected, "{action_id}: OsAction errado");
                }
                _ => panic!("{}: item diferente da arvore", d.title),
            }
        }
    }
    assert_eq!(menus::menu_label("Inverter", Some(menus::shortcut::INVERT)), "Inverter");
}
