use super::*;

fn state() -> menus::State {
    menus::State { source_has: true, ..Default::default() }
}

fn menu_of(v: &[ViewMenu], font: char) -> usize {
    font_title(font, v).unwrap_or_else(|| panic!("sem menu com a letra {font}"))
}

/// So o titulo em destaque, sem menu aberto (o que sobra de `Alt+letra` seguido de `Esc`).
fn so_highlighted(v: &[ViewMenu], font: char) -> MenuBar {
    let mut b = MenuBar::default();
    assert!(b.alt_font(font, v));
    b.key(Key::Esc, v);
    assert_eq!((b.open_at(), b.title().is_some()), (None, true));
    b
}

fn item_label(v: &[ViewMenu], menu: usize, item: usize) -> String {
    match &v[menu].items[item] {
        ItemView::MenuAction { label, .. } => label.clone(),
        ItemView::Separator => "---".into(),
    }
}

#[test]
fn mnemonics_catch_the_first_font_free_and_ignore_accent() {
    let m = mnemonics(&["Arquivo", "Editar", "Exibir", "Navegar", "Imagem", "Ajuda"]);
    let letters: Vec<_> = m.iter().map(|x| x.map(|(_, c)| c)).collect();
    assert_eq!(letters, [Some('a'), Some('e'), Some('x'), Some('n'), Some('i'), Some('j')]);
    // "Exibir": o E ja e do Editar, entao e o X (posicao 1).
    assert_eq!(m[2], Some((1, 'x')));
    let m = mnemonics(&["Configurações", "Sobre"]);
    assert_eq!(m[0], Some((0, 'c')));
    assert_eq!(m[1], Some((0, 's')));
    // Sem letra livre: nenhum mnemonico.
    assert_eq!(mnemonics(&["Aa", "A", "a"]), vec![Some((0, 'a')), None, None]);
    assert_eq!(mnemonics(&["Ççã"])[0], Some((0, 'c')));
}

#[test]
fn view_has_os_same_items_and_none_mnemonic_repeated_in_the_menu() {
    let v = view(&state());
    assert_eq!(v.len(), menus::tree_pc().len());
    for m in &v {
        let mut letters: Vec<char> = m.items.iter().filter_map(|i| match i {
            ItemView::MenuAction { mnemonic: Some((_, c)), .. } => Some(*c),
            _ => None,
        }).collect();
        let before = letters.len();
        letters.sort();
        letters.dedup();
        assert_eq!(before, letters.len(), "mnemonico repetido em {}", m.title);
    }
    let titles: Vec<char> = v.iter().filter_map(|m| m.mnemonic.map(|(_, c)| c)).collect();
    let mut u = titles.clone();
    u.sort();
    u.dedup();
    assert_eq!(titles.len(), v.len(), "todo titulo tem mnemonico");
    assert_eq!(u.len(), titles.len(), "mnemonicos dos titulos repetidos");
}

#[test]
fn click_opens_and_click_of_new_closes_and_pass_so_swap_with_menu_open() {
    let mut b = MenuBar::default();
    b.pass(2);
    assert!(!b.active(), "sem menu aberto o mouse so passa");
    b.click(1);
    assert_eq!((b.open_at(), b.keyboard()), (Some(1), false));
    b.pass(3);
    assert_eq!(b.open_at(), Some(3));
    b.click(3);
    assert!(!b.active());
}

#[test]
fn esc_of_one_menu_open_by_the_mouse_closes_all() {
    let v = view(&state());
    let mut b = MenuBar::default();
    b.click(0);
    assert_eq!(b.key(Key::Esc, &v), Output::Nothing);
    assert!(!b.active());
}

#[test]
fn alt_font_opens_the_menu_of_the_font_and_ignores_font_no_menu() {
    let v = view(&state());
    let mut b = MenuBar::default();
    assert!(!b.alt_font('z', &v));
    assert!(!b.active());
    assert!(b.alt_font('E', &v), "caixa alta tambem");
    assert_eq!(b.open_at(), Some(menu_of(&v, 'e')));
    assert!(b.keyboard());
}

#[test]
fn arrows_swap_of_menu_giving_the_back() {
    let v = view(&state());
    let mut b = so_highlighted(&v, 'a');
    b.key(Key::Left, &v);
    assert_eq!(b.title(), Some(v.len() - 1));
    b.key(Key::Right, &v);
    assert_eq!(b.title(), Some(0));
    b.key(Key::Right, &v);
    assert_eq!(b.title(), Some(1));
    assert_eq!(b.open_at(), None, "so a barra: nada abre sozinho");
}

#[test]
fn down_opens_and_mark_the_first_item_linked_and_up_the_last() {
    let v = view(&state());
    let mut b = so_highlighted(&v, 'a');
    b.key(Key::Down, &v);
    assert_eq!(b.open_at(), Some(0));
    assert_eq!(item_label(&v, 0, b.item().unwrap()), v_first_label(&v, 0));
    // Cima a partir do primeiro da a volta para o ultimo ligado.
    b.key(Key::Up, &v);
    let last = last_linked(&v[0]);
    assert_eq!(b.item(), Some(last));
    // Do ultimo, Baixo da a volta para o primeiro.
    b.key(Key::Down, &v);
    assert_eq!(item_label(&v, 0, b.item().unwrap()), v_first_label(&v, 0));
}

fn v_first_label(v: &[ViewMenu], menu: usize) -> String {
    v[menu].items.iter().find_map(|i| match i {
        ItemView::MenuAction { label, enabled: true, .. } => Some(label.clone()),
        _ => None,
    }).unwrap()
}

fn last_linked(m: &ViewMenu) -> usize {
    m.items.iter().rposition(|i| matches!(i, ItemView::MenuAction { enabled: true, .. })).unwrap()
}

#[test]
fn item_disabled_and_skipped_and_not_triggers() {
    // Sem fonte, "Fechar fonte" (Arquivo) fica desligado.
    let v = view(&menus::State::default());
    let file = menu_of(&v, 'a');
    let close = v[file].items.iter().position(|i| matches!(i, ItemView::MenuAction { menu_action: MenuAction::CloseSource, .. })).unwrap();
    assert!(matches!(v[file].items[close], ItemView::MenuAction { enabled: false, .. }));
    let mut b = MenuBar::default();
    b.alt_font('a', &v);
    for _ in 0..v[file].items.len() * 2 {
        b.key(Key::Down, &v);
        assert_ne!(b.item(), Some(close), "o item desligado nao pode ser marcado");
    }
    // Nem a letra dele aciona.
    assert_eq!(b.key(Key::Font('f'), &v), Output::Nothing);
    assert!(b.open_at().is_some());
}

#[test]
fn enter_triggers_the_item_marked_and_closes() {
    let v = view(&state());
    let mut b = MenuBar::default();
    b.alt_font('a', &v);
    b.key(Key::Down, &v);
    let expected = match v[0].items[b.item().unwrap()] {
        ItemView::MenuAction { menu_action, .. } => menu_action,
        _ => panic!(),
    };
    assert_eq!(b.key(Key::Enter, &v), Output::MenuAction(expected));
    assert!(!b.active());
}

#[test]
fn enter_in_the_bar_opens_the_menu_and_mark_the_first() {
    let v = view(&state());
    let mut b = so_highlighted(&v, 'a');
    assert_eq!(b.key(Key::Enter, &v), Output::Nothing);
    assert_eq!(b.open_at(), Some(0));
    assert!(b.item().is_some());
}

#[test]
fn font_triggers_the_item_of_same_font_and_in_the_bar_opens_the_menu() {
    let v = view(&state());
    let mut b = so_highlighted(&v, 'e');
    // So a barra em foco: uma letra abre o menu.
    b.key(Key::Font('a'), &v);
    assert_eq!(b.open_at(), Some(menu_of(&v, 'a')));
    // Com o menu aberto: "c" e Configuracoes (a primeira letra livre do rotulo).
    assert_eq!(b.key(Key::Font('C'), &v), Output::MenuAction(MenuAction::Settings));
    assert!(!b.active());
}

#[test]
fn esc_of_the_keyboard_back_to_the_bar_and_the_next_leaves() {
    let v = view(&state());
    let mut b = MenuBar::default();
    b.alt_font('e', &v);
    b.key(Key::Down, &v);
    b.key(Key::Esc, &v);
    assert_eq!((b.open_at(), b.item(), b.title().is_some()), (None, None, true));
    b.key(Key::Esc, &v);
    assert!(!b.active());
}

#[test]
fn switch_of_menu_with_as_arrows_being_open_mark_the_first_item() {
    let v = view(&state());
    let mut b = MenuBar::default();
    b.alt_font('a', &v);
    b.key(Key::Right, &v);
    assert_eq!(b.open_at(), Some(1));
    assert!(b.item().is_some());
}

#[test]
fn keys_no_bar_active_not_make_nothing() {
    let v = view(&state());
    let mut b = MenuBar::default();
    for t in [Key::Down, Key::Enter, Key::Esc, Key::Font('a'), Key::Left] {
        assert_eq!(b.key(t, &v), Output::Nothing);
        assert!(!b.active());
    }
}
