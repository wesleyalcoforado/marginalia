//! Estado e teclado da barra de menus fora do macOS (sem gpui, regra R4). Nao tem desenho: a view
//! (`MainView::menu_bar`) le daqui qual titulo esta em destaque, se o menu esta aberto e qual
//! item esta marcado, e repassa as teclas. As letras sublinhadas (mnemonicos) vem de
//! [`mnemonics`]: a primeira letra do rotulo que nenhum irmao usou antes.
//!
//! Como nos menus do Windows: segurando `Alt`, a letra abre o menu daquela letra (as letras de
//! acesso aparecem sublinhadas enquanto o `Alt` esta apertado); com o menu aberto as setas trocam
//! de menu (`<-` `->`) e de item (`cima` `down`); `Enter` aciona; uma letra aciona o item de mesma
//! letra; `Esc` volta um nivel (do menu para o titulo em destaque, e dai para fora). Aberto pelo
//! mouse, `Esc` fecha tudo.

use unicode_normalization::UnicodeNormalization;

use crate::menus::{self, MenuAction, ItemDesc};

/// Um menu da barra, como a view o desenha e o teclado o percorre.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewMenu {
    pub title: String,
    /// (posicao em caracteres, letra minuscula) do mnemonico do titulo.
    pub mnemonic: Option<(usize, char)>,
    pub items: Vec<ItemView>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemView {
    Separator,
    MenuAction {
        /// O rotulo como aparece (com a tecla simples entre parenteses, se houver).
        label: String,
        menu_action: MenuAction,
        /// O atalho com Ctrl, a direita ("" se nao tem).
        key: String,
        enabled: bool,
        mnemonic: Option<(usize, char)>,
    },
}

/// A letra sem acento e em minuscula, se for uma letra do alfabeto latino ("Ç" -> 'c').
fn base_font(c: char) -> Option<char> {
    c.nfd().next().filter(|b| b.is_ascii_alphabetic()).map(|b| b.to_ascii_lowercase())
}

/// O mnemonico de cada rotulo: a primeira letra que os anteriores da lista nao usaram (`None` se
/// todas as letras ja foram). Devolve (posicao em caracteres, letra minuscula).
pub fn mnemonics(labels: &[&str]) -> Vec<Option<(usize, char)>> {
    let mut used: Vec<char> = Vec::new();
    labels
        .iter()
        .map(|r| {
            let found = r.chars().enumerate().find_map(|(i, c)| base_font(c).filter(|b| !used.contains(b)).map(|b| (i, b)));
            if let Some((_, b)) = found {
                used.push(b);
            }
            found
        })
        .collect()
}

/// A barra de menus agora: `menus::tree_pc()` com os itens ligados ou desligados pelo `state`.
pub fn view(state: &menus::State) -> Vec<ViewMenu> {
    let tree = menus::tree_pc();
    let titles: Vec<&str> = tree.iter().map(|m| m.title.as_str()).collect();
    let mnem_titles = mnemonics(&titles);
    tree
        .iter()
        .zip(mnem_titles)
        .map(|(m, mnemonic)| {
            let labels: Vec<&str> = m
                .items
                .iter()
                .filter_map(|i| match i {
                    ItemDesc::MenuAction { label, .. } => Some(label.as_str()),
                    ItemDesc::Separator => None,
                })
                .collect();
            let mut mnem_items = mnemonics(&labels).into_iter();
            let items = m
                .items
                .iter()
                .filter_map(|i| match i {
                    ItemDesc::Separator => Some(ItemView::Separator),
                    ItemDesc::MenuAction { label, action_id, shortcut } => {
                        let mnemonic = mnem_items.next().flatten();
                        let menu_action = MenuAction::from_id(action_id)?;
                        Some(ItemView::MenuAction {
                            label: menus::menu_label(label, *shortcut),
                            menu_action,
                            key: shortcut.filter(|a| menus::with_cmd(a)).map(menus::symbol).unwrap_or_default(),
                            enabled: menus::available(menu_action, state),
                            mnemonic,
                        })
                    }
                })
                .collect();
            ViewMenu { title: m.title.clone(), mnemonic, items }
        })
        .collect()
}

/// Uma tecla que a barra entende.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Left,
    Right,
    Up,
    Down,
    Enter,
    Esc,
    /// Uma letra (qualquer caixa), sem modificador.
    Font(char),
}

/// O que a tecla pede a quem chamou.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output {
    Nothing,
    /// Rodar esta acao (a barra ja fechou).
    MenuAction(MenuAction),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuBar {
    /// O titulo em destaque (a barra esta ativa quando ha um).
    title: Option<usize>,
    /// O menu do `title` esta aberto.
    is_open: bool,
    /// O item marcado no menu aberto (posicao em `ViewMenu::items`, separadores contados).
    item: Option<usize>,
    /// Veio do teclado: mostra os mnemonicos sublinhados e `Esc` volta so um nivel.
    keyboard: bool,
}

impl MenuBar {
    /// A barra esta em uso (titulo em destaque, com o menu aberto ou so o destaque).
    pub fn active(&self) -> bool {
        self.title.is_some()
    }

    pub fn title(&self) -> Option<usize> {
        self.title
    }

    /// O indice do menu aberto, se algum.
    pub fn open_at(&self) -> Option<usize> {
        self.title.filter(|_| self.is_open)
    }

    pub fn item(&self) -> Option<usize> {
        self.item
    }

    /// Mostrar os mnemonicos (sublinhados).
    pub fn keyboard(&self) -> bool {
        self.keyboard
    }

    pub fn close(&mut self) {
        *self = MenuBar::default();
    }

    /// Clique no titulo: abre o menu, ou fecha se ja era o aberto.
    pub fn click(&mut self, i: usize) {
        if self.open_at() == Some(i) {
            self.close();
        } else {
            *self = MenuBar { title: Some(i), is_open: true, item: None, keyboard: false };
        }
    }

    /// O mouse passou no titulo `i`: com um menu aberto, troca para ele.
    pub fn pass(&mut self, i: usize) {
        if self.is_open && self.title != Some(i) {
            self.title = Some(i);
            self.item = None;
        }
    }

    /// `Alt+letra`: abre o menu cujo titulo tem essa letra. Devolve se achou.
    pub fn alt_font(&mut self, c: char, menus: &[ViewMenu]) -> bool {
        match font_title(c, menus) {
            Some(i) => {
                *self = MenuBar { title: Some(i), is_open: true, item: None, keyboard: true };
                true
            }
            None => false,
        }
    }

    pub fn key(&mut self, t: Key, menus: &[ViewMenu]) -> Output {
        let (Some(title), n) = (self.title, menus.len()) else { return Output::Nothing };
        if n == 0 {
            return Output::Nothing;
        }
        match t {
            Key::Esc => {
                if self.is_open && self.keyboard {
                    // Volta para a barra (so o titulo em destaque); outro Esc sai.
                    self.is_open = false;
                    self.item = None;
                } else {
                    self.close();
                }
            }
            Key::Left | Key::Right => {
                let new = if t == Key::Left { (title + n - 1) % n } else { (title + 1) % n };
                self.title = Some(new);
                self.keyboard = true;
                self.item = if self.is_open { step(&menus[new], None, 1) } else { None };
            }
            Key::Down | Key::Up => {
                let dir = if t == Key::Down { 1 } else { -1 };
                self.keyboard = true;
                if self.is_open {
                    self.item = step(&menus[title], self.item, dir);
                } else {
                    self.is_open = true;
                    self.item = step(&menus[title], None, dir);
                }
            }
            Key::Enter => {
                if !self.is_open {
                    self.is_open = true;
                    self.keyboard = true;
                    self.item = step(&menus[title], None, 1);
                } else if let Some(ItemView::MenuAction { menu_action, enabled: true, .. }) = self.item.and_then(|j| menus[title].items.get(j)) {
                    let a = *menu_action;
                    self.close();
                    return Output::MenuAction(a);
                }
            }
            Key::Font(c) => {
                let c = c.to_ascii_lowercase();
                if self.is_open {
                    let found = menus[title].items.iter().enumerate().find_map(|(j, i)| match i {
                        ItemView::MenuAction { menu_action, enabled: true, mnemonic: Some((_, m)), .. } if *m == c => Some((j, *menu_action)),
                        _ => None,
                    });
                    if let Some((j, a)) = found {
                        self.item = Some(j);
                        self.close();
                        return Output::MenuAction(a);
                    }
                } else if let Some(i) = font_title(c, menus) {
                    *self = MenuBar { title: Some(i), is_open: true, item: None, keyboard: true };
                }
            }
        }
        Output::Nothing
    }
}

fn font_title(c: char, menus: &[ViewMenu]) -> Option<usize> {
    let c = c.to_ascii_lowercase();
    menus.iter().position(|m| matches!(m.mnemonic, Some((_, l)) if l == c))
}

/// O proximo item ligado a partir de `de` (exclusive) na direcao `dir` (+1 ou -1), dando a volta;
/// sem `de`, o primeiro (ou o ultimo, se `dir` e -1). `None` se nenhum item esta ligado.
fn step(menu: &ViewMenu, de: Option<usize>, dir: isize) -> Option<usize> {
    let n = menu.items.len() as isize;
    if n == 0 {
        return None;
    }
    let start = match de {
        Some(j) => j as isize,
        None if dir > 0 => -1,
        None => n,
    };
    (1..=n)
        .map(|k| (start + dir * k).rem_euclid(n) as usize)
        .find(|&j| matches!(menu.items[j], ItemView::MenuAction { enabled: true, .. }))
}

#[cfg(test)]
#[path = "tests/menu_bar.rs"]
mod tests;
