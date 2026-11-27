use super::*;
use std::collections::HashSet;

fn items() -> Vec<(String, String, &'static str, Option<&'static str>)> {
    let mut v = Vec::new();
    for m in tree() {
        for i in m.items {
            if let ItemDesc::MenuAction { label, action_id, shortcut } = i {
                v.push((m.title.clone(), label, action_id, shortcut));
            }
        }
    }
    v
}

#[test]
fn whole_item_has_action_resolvable_and_all_action_has_item() {
    let ids: Vec<&str> = items().iter().map(|i| i.2).collect();
    for id in &ids {
        assert!(MenuAction::from_id(id).is_some(), "acao_id {id:?} sem Acao");
    }
    for a in MenuAction::ALL {
        assert!(ids.contains(&a.id()), "{a:?} nao esta em nenhum menu");
        assert_eq!(MenuAction::from_id(a.id()), Some(a));
    }
}

#[test]
fn tools_in_the_menu() {
    for (a, label) in [(MenuAction::HighlightBox, "Grifo caixa"), (MenuAction::ArrowHighlight, "Grifo seta"), (MenuAction::HighlightColors, "Cor dos grifos"), (MenuAction::Tools, "Barra de ferramentas")] {
        assert_eq!(item_of(a), Some((label.to_string(), None)), "{a:?}");
    }
    let no_source = State::default();
    let with_source = State { source_has: true, ..Default::default() };
    for a in [MenuAction::HighlightBox, MenuAction::ArrowHighlight, MenuAction::HighlightColors] {
        assert!(!available(a, &no_source), "{a:?} sem fonte");
        assert!(available(a, &with_source), "{a:?} com fonte");
    }
    assert!(available(MenuAction::Tools, &no_source));
    assert!(MenuAction::ALL.len() >= 42);
}

#[test]
fn help_has_item_and_key() {
    assert_eq!(item_of(MenuAction::Help), Some(("Ajuda".to_string(), Some("?"))));
    assert_eq!(key_action("?"), Some(MenuAction::Help));
    assert!(available(MenuAction::Help, &State::default()));
    assert_eq!(MenuAction::ALL.len(), 46);
}

#[test]
fn none_action_id_repeated() {
    let mut seen = HashSet::new();
    for (menu, label, id, _) in items() {
        assert!(seen.insert(id), "{id:?} repetido ({menu} > {label})");
    }
    let unique: HashSet<&str> = MenuAction::ALL.iter().map(|a| a.id()).collect();
    assert_eq!(unique.len(), MenuAction::ALL.len());
}

#[test]
fn none_shortcut_repeated() {
    let mut seen = HashSet::new();
    for (menu, label, _, shortcut) in items() {
        if let Some(a) = shortcut {
            assert!(seen.insert(a.to_lowercase()), "atalho {a:?} repetido ({menu} > {label})");
        }
    }
    let mut b = HashSet::new();
    for (t, a, _) in SHORTCUTS {
        assert!(b.insert(*t), "KeyBinding {t:?} repetida ({a:?})");
    }
}

/// Todo atalho com Cmd da arvore e uma `KeyBinding` da mesma acao, e toda
/// `KeyBinding` aparece num item com o mesmo atalho (README, dicas,
/// bindings e menus nao se separam).
#[test]
fn shortcuts_with_cmd_of_the_tree_are_os_bindings() {
    for (menu, label, id, shortcut) in items() {
        let Some(t) = shortcut.filter(|t| with_cmd(t)) else { continue };
        let a = MenuAction::from_id(id).unwrap();
        assert!(
            SHORTCUTS.iter().any(|(bt, ba, _)| *bt == t && *ba == a),
            "{menu} > {label}: {t} sem KeyBinding para {a:?}"
        );
    }
    for (t, a, _) in SHORTCUTS {
        assert!(with_cmd(t), "{t:?}: so atalhos com Cmd viram KeyBinding");
        assert_eq!(item_of(*a).and_then(|(_, at)| at), Some(*t), "{a:?}: KeyBinding {t} fora do menu");
    }
}

/// Todo atalho simples da arvore e tratado pela tecla da view (a mesma acao).
#[test]
fn keys_simple_of_the_tree_are_keys_of_the_views() {
    use crate::focus_state::{highlight_key, HighlightAction, Focus};
    for (menu, label, id, shortcut) in items() {
        let Some(t) = shortcut.filter(|t| !with_cmd(t)) else { continue };
        let a = MenuAction::from_id(id).unwrap();
        if a == MenuAction::DeleteHighlight {
            // Delete/Backspace com a imagem em foco e um grifo selecionado.
            assert_eq!(highlight_key(t, Focus::Image, false, false, false, false, true, false), Some(HighlightAction::Delete));
            continue;
        }
        assert_eq!(key_action(t), Some(a), "{menu} > {label}: tecla {t:?}");
    }
    // e toda tecla da tabela e a do item ou um apelido documentado dela
    // (README: `F`/`f`, `+` (ou `=`), `-` (ou `_`), setas e PageUp/PageDown)
    let aliases = [("=", "+"), ("_", "-"), ("pageup", "left"), ("pagedown", "right")];
    for (t, a) in KEYS {
        let (_, at) = item_of(*a).unwrap();
        let shown = aliases.iter().find(|(ap, _)| ap == t).map_or(*t, |(_, p)| *p);
        assert_eq!(at, Some(shown), "{t:?} -> {a:?}: o item mostra outra tecla");
    }
}

#[test]
fn drawers_cmd_1_2_3_match_with_by_the_key_and_hints() {
    register_app_shortcuts();
    for a in [MenuAction::Notes, MenuAction::Sources, MenuAction::Tasks] {
        let g = a.drawer().unwrap();
        let (label, at) = item_of(a).unwrap();
        let t = at.unwrap();
        assert_eq!(Drawer::key_by_the(t.trim_start_matches("cmd-")), Some(g));
        assert_eq!(label, g.title());
        // a dica do icone da barra de atividades e a do menu
        assert_eq!(g.hint(), hint(a));
        assert_eq!(symbol(t), g.shortcut());
    }
}

#[test]
fn hints_of_the_bar_show_os_shortcuts_of_the_menu() {
    assert_eq!(hint(MenuAction::OpenSource), "Abrir fonte (⌘O)");
    assert_eq!(hint(MenuAction::OpenVault), "Abrir vault (⇧⌘V)");
    assert_eq!(hint(MenuAction::AdjustmentsPanel), "Ajustes de imagem (⌘F)");
    assert_eq!(hint(MenuAction::Previous), "Imagem anterior (←)");
}

#[test]
fn symbols_and_labels() {
    assert_eq!(symbol("cmd-shift-v"), "⇧⌘V");
    assert_eq!(symbol("cmd-backspace"), "⌘⌫");
    assert_eq!(symbol("cmd-1"), "⌘1");
    assert_eq!(symbol("-"), "−");
    assert_eq!(symbol("left"), "←");
    assert_eq!(symbol("F"), "F");
    assert_eq!(symbol("i"), "i");
    assert_eq!(menu_label("Inverter", Some("i")), "Inverter (i)");
    assert_eq!(menu_label("Abrir fonte…", Some("cmd-o")), "Abrir fonte…");
    assert_eq!(menu_label("Fechar fonte", None), "Fechar fonte");
    assert_eq!(notation_readme("cmd-shift-v"), "Cmd-Shift-V");
    assert_eq!(notation_readme("cmd-backspace"), "Cmd-Backspace");
    assert_eq!(notation_readme("F"), "`F`");
}

/// Titulos e rotulos em portugues, com maiuscula so na primeira palavra
/// (salvo "Marginalia") e reticencias "…" (nao "...") nos que abrem dialogo.
#[test]
fn titles_and_labels_in_portuguese_and_capitalization_consistent() {
    let titles: Vec<String> = tree().iter().map(|m| m.title.clone()).collect();
    assert_eq!(titles, ["Marginalia", "Arquivo", "Editar", "Exibir", "Navegar", "Imagem", "Ajuda"]);
    let english = ["open", "file", "edit", "view", "undo", "cut", "copy", "paste", "select", "delete", "quit", "about", "close", "next", "previous", "image", "brightness", "invert"];
    for (menu, label, _, _) in items() {
        let mut words = label.split([' ', '/']);
        let first = words.next().unwrap();
        assert!(first.chars().next().unwrap().is_uppercase(), "{menu} > {label}: comeca com maiuscula");
        for p in words {
            if p == "Marginalia" {
                continue;
            }
            assert!(!p.chars().next().is_some_and(char::is_uppercase), "{menu} > {label}: {p:?} em maiuscula");
        }
        assert!(!label.contains("..."), "{menu} > {label}: use …");
        for p in label.to_lowercase().split(|c: char| !c.is_alphanumeric()) {
            assert!(!english.contains(&p), "{menu} > {label}: {p:?} em ingles");
        }
    }
}

/// A primeira coluna das linhas da tabela de teclas do README.
fn readme_keys() -> Vec<&'static str> {
    include_str!("../../../../README.md")
        .lines()
        .filter(|l| l.starts_with('|'))
        .filter_map(|l| l.split('|').nth(1))
        .collect()
}

/// A tabela de teclas do README lista todo atalho dos menus.
#[test]
fn readme_list_whole_shortcut_of_the_menus() {
    let first = readme_keys();
    for (menu, label, _, shortcut) in items() {
        let Some(t) = shortcut else { continue };
        let n = notation_readme(t);
        assert!(first.iter().any(|c| c.contains(&n)), "README sem {n} ({menu} > {label})");
    }
}

/// Como uma tecla aparece na coluna de teclas do README, sem diferenca de
/// caixa: `` `p` ``, `Esc`, `Cmd-V`; as setas, por `Arrow`/`Setas`.
fn readme_name(key: &str, cmd: bool) -> String {
    let name = match key {
        "up" | "down" | "left" | "right" => "Seta".to_string(),
        "enter" => "Enter".into(),
        "escape" => "Esc".into(),
        "backspace" => "Backspace".into(),
        "delete" => "Delete".into(),
        "space" => "espaço".into(),
        "tab" => "Tab".into(),
        "home" => "Home".into(),
        "end" => "End".into(),
        "pageup" => "PageUp".into(),
        "pagedown" => "PageDown".into(),
        k if cmd => k.to_uppercase(),
        k => format!("`{k}`"),
    };
    if cmd { format!("Cmd-{name}") } else { name }
}

/// Toda tecla que o app trata esta na tabela do README: os `KeyBinding`
/// ([`SHORTCUTS`]), as teclas simples das views ([`KEYS`]) e, por contexto
/// (o marcador na coluna de teclas), as teclas das funcoes que as views
/// chamam: a gaveta em foco, a nota rapida, o seletor de vault, a imagem
/// (W/A/S/D, grifos) e o dialogo de excluir de vez.
#[test]
fn readme_list_all_key_registered() {
    use crate::focus_state::{self, Focus};
    let first: Vec<String> = readme_keys().iter().map(|c| c.to_lowercase()).collect();
    let has = |marker: &str, n: &str| {
        let (m, n) = (marker.to_lowercase(), n.to_lowercase());
        first.iter().any(|c| c.contains(&m) && c.contains(&n))
    };
    for (t, a, _) in SHORTCUTS {
        assert!(has("", &notation_readme(t)), "README sem o KeyBinding {t} ({a:?})");
    }
    let aliases = [("pageup", "PageUp"), ("pagedown", "PageDown")];
    for (t, a) in KEYS {
        let n = aliases.iter().find(|(ap, _)| ap == t).map_or_else(|| notation_readme(t), |(_, p)| p.to_string());
        assert!(has("", &n), "README sem a tecla {n} ({a:?})");
    }
    let mut candidates: Vec<String> = ('a'..='z').chain('0'..='9').chain("[]{}+-=_/".chars()).map(String::from).collect();
    candidates.extend(
        ["up", "down", "left", "right", "enter", "escape", "backspace", "delete", "space", "tab", "home", "end", "pageup", "pagedown"]
            .map(String::from),
    );
    let mut checked = 0;
    for k in &candidates {
        for cmd in [false, true] {
            let n = readme_name(k, cmd);
            for g in Drawer::ALL {
                if focus_state::drawer_key(k, Focus::Drawer, Some(g), false, cmd, false, false).is_some() {
                    assert!(has("gaveta em foco", &n), "README sem {n} (gaveta {g:?} em foco)");
                    checked += 1;
                }
            }
            let arrow = matches!(k.as_str(), "up" | "down" | "left" | "right" | "home" | "end");
            // Cmd/Option com setas andam pelo texto: estao na descricao da linha "Setas".
            // Com Cmd so conta o que muda com ele (Cmd-Backspace apaga como Backspace).
            let quick_note = |cmd| crate::quick_note::key(k, false, cmd, false, false);
            if !(cmd && (arrow || quick_note(true) == quick_note(false))) {
                if let Some(t) = quick_note(cmd) {
                    if t != crate::quick_note::Key::Ignore {
                        assert!(has("na nota", &n), "README sem {n} (na nota)");
                        checked += 1;
                    }
                }
            }
            if !cmd {
                // A caixa numerica de uma barra do painel de ajustes: ↑/↓ (Shift: de 10 em 10).
                for shift in [false, true] {
                    if crate::slider::key_step(k, shift).is_some() {
                        assert!(has("caixa numérica", &n), "README sem {n} (caixa numérica)");
                        if shift {
                            assert!(has("caixa numérica", "Shift"), "README sem Shift (caixa numérica)");
                        }
                        checked += 1;
                    }
                }
                if crate::vault_picker::picker_key(k).is_some() {
                    assert!(has("seletor de vault", &n), "README sem {n} (seletor de vault)");
                    checked += 1;
                }
                let pan = focus_state::pan_key(k, Focus::Image, false, false, false, false).is_some();
                let highlight = focus_state::highlight_key(k, Focus::Image, false, false, false, false, true, true).is_some();
                if pan || highlight {
                    assert!(has("imagem em foco", &n), "README sem {n} (imagem em foco)");
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 30, "poucas teclas conferidas: {checked}");
    // Teclas tratadas direto na view (main_view: `/`; a caixa "Renomear")
    // e as do dialogo de excluir (`Drawers::deletion_key`, testadas em drawers).
    for (marker, n) in [
        ("imagem em foco", "`/`"),
        ("renomear", "Enter"),
        ("renomear", "Esc"),
        ("caixa numérica", "Enter"),
        ("caixa numérica", "Esc"),
        ("barra", "duplo clique"),
        ("confirmação de excluir", "Tab"),
        ("confirmação de excluir", "Seta"),
        ("confirmação de excluir", "Enter"),
        ("confirmação de excluir", "espaço"),
        ("confirmação de excluir", "Esc"),
    ] {
        assert!(has(marker, n), "README sem {n} ({marker})");
    }
}

#[test]
fn items_disabled_when_not_se_apply() {
    let nothing = State::default();
    for a in [MenuAction::CloseSource, MenuAction::Previous, MenuAction::ZoomUp, MenuAction::Invert, MenuAction::Restore, MenuAction::DeleteHighlight, MenuAction::Undo, MenuAction::DeleteItem] {
        assert!(!available(a, &nothing), "{a:?} sem fonte");
    }
    for a in [MenuAction::About, MenuAction::Notes, MenuAction::AdjustmentsPanel, MenuAction::OpenSource, MenuAction::Exit] {
        assert!(available(a, &nothing), "{a:?} vale sempre");
    }
    let with_source = State { source_has: true, ..State::default() };
    assert!(available(MenuAction::CloseSource, &with_source) && available(MenuAction::Invert, &with_source));
    assert!(!available(MenuAction::DeleteHighlight, &with_source), "sem selecao");
    // Excluir grifo so com a imagem em foco (o "⌘Z desfaz" da barra so vale la)
    assert!(!available(MenuAction::DeleteHighlight, &State { selected: true, ..with_source }), "imagem sem foco");
    assert!(available(MenuAction::DeleteHighlight, &State { selected: true, image_in_focus: true, ..with_source }));
    assert!(available(MenuAction::Undo, &State { undo: true, ..nothing }));
    assert!(available(MenuAction::DeleteItem, &State { delete_item: true, ..nothing }));
}

#[test]
fn actions_reach() {
    assert_eq!(MenuAction::Exit.reach(), Reach::App);
    assert_eq!(MenuAction::Copy.reach(), Reach::Text);
    assert_eq!(MenuAction::Invert.reach(), Reach::View);
    // os de texto e os globais tem KeyBinding no contexto certo
    for (_, a, c) in SHORTCUTS {
        match a.reach() {
            Reach::App => assert_eq!(*c, Context::Global, "{a:?}"),
            Reach::Text => assert_eq!(*c, Context::Text, "{a:?}"),
            Reach::View => assert_eq!(*c, Context::SoMenu, "{a:?}"),
        }
    }
}

/// Como o README escreve o atalho na tabela de teclas: `Cmd-Shift-V`, `` `F` ``, `Seta esquerda`.
pub fn notation_readme(shortcut: &str) -> String {
    if with_cmd(shortcut) {
        return shortcut
            .split('-')
            .map(|p| match p {
                "cmd" => "Cmd".to_string(),
                "shift" => "Shift".to_string(),
                "backspace" => "Backspace".to_string(),
                t => t.to_uppercase(),
            })
            .collect::<Vec<_>>()
            .join("-");
    }
    match shortcut {
        "left" => "Seta esquerda".into(),
        "right" => "Seta direita".into(),
        "backspace" => "Backspace".into(),
        t => format!("`{t}`"),
    }
}

#[test]
fn shortcut_by_id_resolve_actions_with_the_symbol_of_the_mac() {
    assert_eq!(shortcut_by_id("open_source").as_deref(), Some("⌘O"));
    assert_eq!(shortcut_by_id("open_vault").as_deref(), Some("⇧⌘V"));
    assert_eq!(shortcut_by_id("previous").as_deref(), Some("←"));
    assert!(shortcut_by_id("help").is_some());
    assert_eq!(shortcut_by_id("nao_existe"), None);
}

#[test]
fn whole_id_of_action_with_shortcut_resolve() {
    for a in MenuAction::ALL {
        let has = item_of(a).is_some_and(|(_, t)| t.is_some()) || KEYS.iter().any(|(_, x)| *x == a);
        if has {
            assert!(shortcut_by_id(a.id()).is_some(), "{} tem atalho mas nao resolve", a.id());
        }
    }
}

#[test]
fn shortcuts_no_menu_not_repeat_ids_of_action_and_resolve() {
    for (id, shortcut) in SHORTCUTS_NO_MENU {
        assert!(MenuAction::from_id(id).is_none(), "{id} ja e uma acao");
        assert_eq!(shortcut_by_id(id), Some(symbol(shortcut)));
    }
}

#[test]
fn register_shortcuts_of_the_app_makes_the_marker_resolver() {
    register_app_shortcuts();
    assert_eq!(texts::expand("abre {shortcut:open_source}", &[]), "abre ⌘O");
}

/// A arvore de menus e as dicas como eram antes de os textos irem para o
/// catalogo (`tests/menus_dourado.txt`): nada visivel pode mudar.
#[test]
fn tree_and_hints_has_os_texts_of_today() {
    let mut s = String::new();
    for m in tree() {
        for i in m.items {
            match i {
                ItemDesc::MenuAction { label, action_id, shortcut } => {
                    s.push_str(&format!("{}|{}|{}|{}\n", m.title, action_id, label, shortcut.unwrap_or("")))
                }
                ItemDesc::Separator => s.push_str(&format!("{}|---\n", m.title)),
            }
        }
    }
    for a in MenuAction::ALL {
        s.push_str(&format!("dica|{}|{}\n", a.id(), hint(a)));
    }
    assert_eq!(s, include_str!("menus_golden.txt"));
}

#[test]
fn modifiers_alone_and_shortcuts_no_menu_become_symbols() {
    assert_eq!(symbol("shift"), "⇧");
    assert_eq!(symbol("alt"), "⌥");
    assert_eq!(symbol("cmd-enter"), "⌘↵");
    for (id, symbol) in [
        ("finish_quick_note", "⌘↵"),
        ("key_enter", "↵"),
        ("key_backspace", "⌫"),
        ("key_delete", "⌦"),
        ("key_shift", "⇧"),
        ("key_alt", "⌥"),
    ] {
        assert_eq!(shortcut_by_id(id).as_deref(), Some(symbol), "{id}");
    }
}

#[test]
fn to_gpui_swap_so_the_cmd_of_the_start() {
    assert_eq!(gpui_to("cmd-o"), "secondary-o");
    assert_eq!(gpui_to("cmd-shift-v"), "secondary-shift-v");
    assert_eq!(gpui_to("cmd-,"), "secondary-,");
    assert_eq!(gpui_to("backspace"), "backspace");
    assert_eq!(gpui_to("?"), "?");
}

#[test]
fn command_and_control_in_the_mac_not_changes_and_outside_its_the_ctrl_becomes_command() {
    assert_eq!(command_and_control(true, false, true), (true, false));
    assert_eq!(command_and_control(false, true, true), (false, true), "no mac o Ctrl continua Ctrl");
    assert_eq!(command_and_control(false, true, false), (true, false));
    assert_eq!(command_and_control(false, false, false), (false, false));
}

#[test]
fn symbol_in_the_windows_writes_as_keys_by_long() {
    let pc = |a: &str| to_symbol(a, false);
    assert_eq!(pc("cmd-o"), "Ctrl+O");
    assert_eq!(pc("cmd-shift-v"), "Ctrl+Shift+V");
    assert_eq!(pc("cmd-]"), "Ctrl+]");
    assert_eq!(pc("cmd-enter"), "Ctrl+Enter");
    assert_eq!(pc("backspace"), "Backspace");
    assert_eq!(pc("delete"), "Del");
    assert_eq!(pc("left"), "←");
    assert_eq!(pc("alt"), "Alt");
    assert_eq!(pc("shift"), "Shift");
    assert_eq!(pc("-"), "−");
    assert_eq!(pc("?"), "?");
    // E o estilo do macOS segue o de sempre.
    assert_eq!(to_symbol("cmd-shift-v", true), "⇧⌘V");
}

#[test]
fn whole_shortcut_with_cmd_becomes_one_shortcut_valid_of_the_gpui() {
    for (t, a, _) in SHORTCUTS {
        let g = gpui_to(t);
        assert!(g.starts_with("secondary-"), "{t} ({a:?}) -> {g}");
        assert!(!g.contains("cmd"), "{g}");
    }
}

fn tree_ids(tree: &[MenuDesc]) -> Vec<&'static str> {
    tree.iter().flat_map(|m| m.items.iter()).filter_map(|i| match i {
        ItemDesc::MenuAction { action_id, .. } => Some(*action_id),
        ItemDesc::Separator => None,
    }).collect()
}

fn menu_ids(m: &MenuDesc) -> Vec<&'static str> {
    m.items.iter().filter_map(|i| match i { ItemDesc::MenuAction { action_id, .. } => Some(*action_id), ItemDesc::Separator => None }).collect()
}

#[test]
fn tree_pc_not_has_menu_of_the_app_and_not_loses_none_item() {
    let mac = tree();
    let pc = tree_pc();
    assert_eq!(pc.len(), mac.len() - 1, "so o menu do app some");
    let (mut a, mut b) = (tree_ids(&mac), tree_ids(&pc));
    a.sort();
    b.sort();
    assert_eq!(a, b, "os mesmos itens, so mudam de lugar");
}

#[test]
fn tree_pc_puts_exit_and_settings_in_file_and_about_in_help() {
    let pc = tree_pc();
    let file = menu_ids(&pc[0]);
    let help = menu_ids(pc.last().unwrap());
    assert_eq!(&file[file.len() - 2..], [MenuAction::Settings.id(), MenuAction::Exit.id()], "Sair por ultimo");
    assert_eq!(help.last(), Some(&MenuAction::About.id()));
    assert!(help.contains(&MenuAction::Help.id()));
    // Nenhum separador no fim, no comeco nem dois seguidos.
    for m in &pc {
        assert!(!matches!(m.items.first(), Some(ItemDesc::Separator)) && !matches!(m.items.last(), Some(ItemDesc::Separator)), "{}", m.title);
        assert!(!m.items.windows(2).any(|w| matches!(w, [ItemDesc::Separator, ItemDesc::Separator])), "{}", m.title);
    }
}

#[test]
fn check_updates_is_in_help_menu_always_available_and_about_stays_last_on_pc() {
    assert_eq!(MenuAction::CheckUpdates.id(), "check_updates");
    assert!(MenuAction::ALL.contains(&MenuAction::CheckUpdates));
    assert!(available(MenuAction::CheckUpdates, &State::default()));
    let help = menu_ids(tree().last().unwrap());
    assert!(help.contains(&MenuAction::CheckUpdates.id()));
    let pc = menu_ids(tree_pc().last().unwrap());
    assert!(pc.contains(&MenuAction::CheckUpdates.id()));
    assert_eq!(pc.last(), Some(&MenuAction::About.id()));
}
