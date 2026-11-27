//! Menus do macOS, sem gpui (regra R4): a arvore dos menus (titulos, itens,
//! atalhos), os atalhos com Cmd que viram `KeyBinding` e as teclas simples que
//! as views tratam. `main.rs` monta os `gpui::Menu` e as `KeyBinding` daqui e
//! liga cada [`MenuAction`] a uma acao do gpui; os botoes, as teclas e os itens de
//! menu chamam o mesmo metodo da view ([`MenuAction`] -> `MainView::command`), entao
//! nada de logica duplicada.
//!
//! Teclas simples (setas, `+`, `-`, `0`, `?`, `W/A/S/D`...) nao viram `KeyBinding`: elas so
//! valem com o foco certo (nunca num campo de texto) e continuam nos listeners
//! das views; no menu aparecem no rotulo, "Mais zoom (+)". Os ajustes de imagem (painel,
//! brilho, contraste, inverter, cinza, limiar, restaurar) sao atalhos com Cmd. Os atalhos
//! com Cmd sao de tres tipos ([`Context`]): globais (valem de qualquer foco),
//! de texto (so com a caixa de nota / "Renomear" em foco) e os que dependem do
//! foco ou da tecla segurada (Cmd-1/2/3, Cmd-Z, Cmd-Backspace), que continuam
//! no listener da view raiz e so tem a `KeyBinding` para o menu mostrar o atalho.

use crate::drawers::Drawer;

/// Cada item de menu (e o metodo da view que ele chama).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MenuAction {
    About,
    CheckUpdates,
    Exit,
    OpenSource,
    OpenVault,
    CloseSource,
    Undo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    DeleteHighlight,
    DeleteItem,
    Notes,
    Sources,
    Tasks,
    AdjustmentsPanel,
    ZoomUp,
    ZoomDown,
    Adjust,
    Previous,
    Next,
    FocusQuickNote,
    FocusImage,
    BrightnessUp,
    BrightnessDown,
    ContrastUp,
    ContrastDown,
    SharpnessUp,
    SharpnessDown,
    HighpassUp,
    HighpassDown,
    ThresholdUp,
    ThresholdDown,
    Invert,
    Gray,
    Threshold,
    Restore,
    RotateLeft,
    RotateRight,
    HighlightBox,
    ArrowHighlight,
    HighlightColors,
    Tools,
    Help,
    Settings,
}

/// Quem trata a acao.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reach {
    /// O app (`cx.on_action` no `main.rs`): vale de qualquer foco, mesmo sem foco.
    App,
    /// A view raiz (`MainView::command`), registrada so quando se aplica.
    View,
    /// A caixa de texto em foco (nota rapida ou "Renomear").
    Text,
}

impl MenuAction {
    pub const ALL: [MenuAction; 46] = [
        MenuAction::About,
        MenuAction::CheckUpdates,
        MenuAction::Exit,
        MenuAction::OpenSource,
        MenuAction::OpenVault,
        MenuAction::CloseSource,
        MenuAction::Undo,
        MenuAction::Cut,
        MenuAction::Copy,
        MenuAction::Paste,
        MenuAction::SelectAll,
        MenuAction::DeleteHighlight,
        MenuAction::DeleteItem,
        MenuAction::Notes,
        MenuAction::Sources,
        MenuAction::Tasks,
        MenuAction::AdjustmentsPanel,
        MenuAction::ZoomUp,
        MenuAction::ZoomDown,
        MenuAction::Adjust,
        MenuAction::Previous,
        MenuAction::Next,
        MenuAction::FocusQuickNote,
        MenuAction::FocusImage,
        MenuAction::BrightnessUp,
        MenuAction::BrightnessDown,
        MenuAction::ContrastUp,
        MenuAction::ContrastDown,
        MenuAction::SharpnessUp,
        MenuAction::SharpnessDown,
        MenuAction::HighpassUp,
        MenuAction::HighpassDown,
        MenuAction::ThresholdUp,
        MenuAction::ThresholdDown,
        MenuAction::Invert,
        MenuAction::Gray,
        MenuAction::Threshold,
        MenuAction::Restore,
        MenuAction::RotateLeft,
        MenuAction::RotateRight,
        MenuAction::HighlightBox,
        MenuAction::ArrowHighlight,
        MenuAction::HighlightColors,
        MenuAction::Tools,
        MenuAction::Help,
        MenuAction::Settings,
    ];

    /// Identificador estavel (o `action_id` dos itens de menu).
    pub fn id(self) -> &'static str {
        match self {
            MenuAction::About => "about",
            MenuAction::CheckUpdates => "check_updates",
            MenuAction::Exit => "exit",
            MenuAction::OpenSource => "open_source",
            MenuAction::OpenVault => "open_vault",
            MenuAction::CloseSource => "close_source",
            MenuAction::Undo => "undo",
            MenuAction::Cut => "cut",
            MenuAction::Copy => "copy",
            MenuAction::Paste => "paste",
            MenuAction::SelectAll => "select_all",
            MenuAction::DeleteHighlight => "delete_highlight",
            MenuAction::DeleteItem => "delete_item",
            MenuAction::Notes => "notes",
            MenuAction::Sources => "sources",
            MenuAction::Tasks => "tasks",
            MenuAction::AdjustmentsPanel => "image_adjustments",
            MenuAction::ZoomUp => "zoom_up",
            MenuAction::ZoomDown => "zoom_down",
            MenuAction::Adjust => "adjust",
            MenuAction::Previous => "previous",
            MenuAction::Next => "next",
            MenuAction::FocusQuickNote => "focus_quick_note",
            MenuAction::FocusImage => "focus_image",
            MenuAction::BrightnessUp => "brightness_up",
            MenuAction::BrightnessDown => "brightness_down",
            MenuAction::ContrastUp => "contrast_up",
            MenuAction::ContrastDown => "contrast_down",
            MenuAction::SharpnessUp => "sharpness_up",
            MenuAction::SharpnessDown => "sharpness_down",
            MenuAction::HighpassUp => "highpass_up",
            MenuAction::HighpassDown => "highpass_down",
            MenuAction::ThresholdUp => "threshold_up",
            MenuAction::ThresholdDown => "threshold_down",
            MenuAction::Invert => "invert",
            MenuAction::Gray => "gray",
            MenuAction::Threshold => "threshold",
            MenuAction::Restore => "restore",
            MenuAction::RotateLeft => "rotate_left",
            MenuAction::RotateRight => "rotate_right",
            MenuAction::HighlightBox => "highlight_box",
            MenuAction::ArrowHighlight => "arrow_highlight",
            MenuAction::HighlightColors => "highlight_colors",
            MenuAction::Tools => "tools",
            MenuAction::Help => "help",
            MenuAction::Settings => "settings",
        }
    }

    pub fn from_id(id: &str) -> Option<MenuAction> {
        MenuAction::ALL.iter().copied().find(|a| a.id() == id)
    }

    pub fn reach(self) -> Reach {
        match self {
            MenuAction::Exit | MenuAction::OpenSource | MenuAction::OpenVault | MenuAction::FocusQuickNote | MenuAction::FocusImage => Reach::App,
            MenuAction::Cut | MenuAction::Copy | MenuAction::Paste | MenuAction::SelectAll => Reach::Text,
            _ => Reach::View,
        }
    }

    /// A gaveta que a acao abre/fecha (Cmd-1/2/3).
    pub fn drawer(self) -> Option<Drawer> {
        match self {
            MenuAction::Notes => Some(Drawer::Notes),
            MenuAction::Sources => Some(Drawer::Sources),
            MenuAction::Tasks => Some(Drawer::Tasks),
            _ => None,
        }
    }

    /// Zoom: tratado pelo visualizador (as teclas so com ele em foco).
    pub fn e_zoom(self) -> bool {
        matches!(self, MenuAction::ZoomUp | MenuAction::ZoomDown | MenuAction::Adjust)
    }
}

/// Atalhos (sintaxe de `gpui::Keystroke::parse`). Os mesmos valores montam a
/// arvore, as `KeyBinding` ([`SHORTCUTS`]), a tabela das teclas simples
/// ([`KEYS`]) e as dicas dos botoes ([`hint`]).
pub mod shortcut {
    pub const EXIT: &str = "cmd-q";
    pub const OPEN_SOURCE: &str = "cmd-o";
    pub const OPEN_VAULT: &str = "cmd-shift-v";
    pub const UNDO: &str = "cmd-z";
    pub const CUT: &str = "cmd-x";
    pub const COPY: &str = "cmd-c";
    pub const PASTE: &str = "cmd-v";
    pub const SELECT_ALL: &str = "cmd-a";
    pub const DELETE_HIGHLIGHT: &str = "backspace";
    pub const NOTES: &str = "cmd-1";
    pub const SOURCES: &str = "cmd-2";
    pub const TASKS: &str = "cmd-3";
    pub const ADJUSTMENTS: &str = "cmd-f";
    pub const ZOOM_UP: &str = "+";
    pub const ZOOM_DOWN: &str = "-";
    pub const ADJUST: &str = "0";
    pub const PREVIOUS: &str = "left";
    pub const NEXT: &str = "right";
    pub const FOCUS_QUICK_NOTE: &str = "cmd-k";
    pub const FOCUS_IMAGE: &str = "cmd-shift-k";
    pub const BRIGHTNESS_UP: &str = "cmd-]";
    pub const BRIGHTNESS_DOWN: &str = "cmd-[";
    pub const CONTRAST_UP: &str = "cmd-shift-]";
    pub const CONTRAST_DOWN: &str = "cmd-shift-[";
    pub const INVERT: &str = "cmd-i";
    pub const GRAY: &str = "cmd-g";
    pub const THRESHOLD: &str = "cmd-t";
    pub const RESTORE: &str = "cmd-shift-r";
    pub const ROTATE_LEFT: &str = "cmd-l";
    pub const ROTATE_RIGHT: &str = "cmd-r";
    pub const HELP: &str = "?";
    pub const SETTINGS: &str = "cmd-,";
}

/// Onde a `KeyBinding` de um atalho com Cmd casa.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Context {
    /// Qualquer foco (e sem foco): Cmd-Q, Cmd-O, Cmd-Shift-V, Cmd-K, Cmd-Shift-K.
    Global,
    /// So com uma caixa de texto em foco ([`TEXT_CONTEXT`]): recortar, copiar, colar, selecionar tudo.
    Text,
    /// A tecla e do listener da view raiz (depende do foco ou nao repete
    /// segurada); a `KeyBinding` usa [`CONTEXT_SO_MENU`], que nenhum elemento
    /// declara, so para o menu mostrar o atalho. Tecla que a view nao trata
    /// (ex.: Cmd-Z na caixa de nota) chega ao menu pelo macOS, e o item so esta
    /// ligado quando se aplica.
    SoMenu,
}

/// `key_context` das caixas de texto (`QuickNoteBox`).
pub const TEXT_CONTEXT: &str = "NotaRapida";
/// Contexto que nenhum elemento declara (ver [`Context::SoMenu`]).
pub const CONTEXT_SO_MENU: &str = "MarginaliaSoMenu";

impl Context {
    pub fn name(self) -> Option<&'static str> {
        match self {
            Context::Global => None,
            Context::Text => Some(TEXT_CONTEXT),
            Context::SoMenu => Some(CONTEXT_SO_MENU),
        }
    }
}

/// Todos os atalhos com Cmd: viram `KeyBinding` em `main.rs`.
pub const SHORTCUTS: &[(&str, MenuAction, Context)] = &[
    (shortcut::EXIT, MenuAction::Exit, Context::Global),
    (shortcut::OPEN_SOURCE, MenuAction::OpenSource, Context::Global),
    (shortcut::OPEN_VAULT, MenuAction::OpenVault, Context::Global),
    (shortcut::FOCUS_QUICK_NOTE, MenuAction::FocusQuickNote, Context::Global),
    (shortcut::FOCUS_IMAGE, MenuAction::FocusImage, Context::Global),
    (shortcut::CUT, MenuAction::Cut, Context::Text),
    (shortcut::COPY, MenuAction::Copy, Context::Text),
    (shortcut::PASTE, MenuAction::Paste, Context::Text),
    (shortcut::SELECT_ALL, MenuAction::SelectAll, Context::Text),
    (shortcut::UNDO, MenuAction::Undo, Context::SoMenu),
    (shortcut::NOTES, MenuAction::Notes, Context::SoMenu),
    (shortcut::SOURCES, MenuAction::Sources, Context::SoMenu),
    (shortcut::TASKS, MenuAction::Tasks, Context::SoMenu),
    (shortcut::ROTATE_LEFT, MenuAction::RotateLeft, Context::SoMenu),
    (shortcut::ROTATE_RIGHT, MenuAction::RotateRight, Context::SoMenu),
    (shortcut::ADJUSTMENTS, MenuAction::AdjustmentsPanel, Context::SoMenu),
    (shortcut::SETTINGS, MenuAction::Settings, Context::SoMenu),
    (shortcut::BRIGHTNESS_DOWN, MenuAction::BrightnessDown, Context::SoMenu),
    (shortcut::BRIGHTNESS_UP, MenuAction::BrightnessUp, Context::SoMenu),
    (shortcut::CONTRAST_DOWN, MenuAction::ContrastDown, Context::SoMenu),
    (shortcut::CONTRAST_UP, MenuAction::ContrastUp, Context::SoMenu),
    (shortcut::INVERT, MenuAction::Invert, Context::SoMenu),
    (shortcut::GRAY, MenuAction::Gray, Context::SoMenu),
    (shortcut::THRESHOLD, MenuAction::Threshold, Context::SoMenu),
    (shortcut::RESTORE, MenuAction::Restore, Context::SoMenu),
];

/// A view raiz trata sozinha estas teclas em `on_key` (Cmd-1/2/3 de qualquer foco, Cmd-Z): ali ela
/// sabe o foco e nao repete a tecla segurada. As outras de [`Context::SoMenu`] so chegam pelo menu.
pub fn view_key(a: MenuAction) -> bool {
    matches!(a, MenuAction::Undo | MenuAction::Notes | MenuAction::Sources | MenuAction::Tasks)
}

/// Os atalhos `SoMenu` que no macOS o menu nativo entrega (o NSMenu leva a tecla ao item quando a view
/// nao a tratou) e que, fora dele, nao tem quem entregue: viram `KeyBinding` sem contexto. A acao
/// segue para o mesmo handler do menu, registrado na view so quando se aplica (`available`), entao
/// o item desligado deixa a tecla passar.
pub fn shortcuts_no_menu_native() -> Vec<(&'static str, MenuAction)> {
    SHORTCUTS.iter().filter(|(_, a, c)| *c == Context::SoMenu && !view_key(*a)).map(|(t, a, _)| (*t, *a)).collect()
}

/// Teclas simples (sem modificador) das views -> acao. A chave e o caractere
/// (`key_char`, ou o nome da tecla: `"left"`). Zoom e do visualizador.
pub const KEYS: &[(&str, MenuAction)] = &[
    (shortcut::PREVIOUS, MenuAction::Previous),
    ("pageup", MenuAction::Previous),
    (shortcut::NEXT, MenuAction::Next),
    ("pagedown", MenuAction::Next),
    (shortcut::ZOOM_UP, MenuAction::ZoomUp),
    ("=", MenuAction::ZoomUp),
    (shortcut::ZOOM_DOWN, MenuAction::ZoomDown),
    ("_", MenuAction::ZoomDown),
    (shortcut::ADJUST, MenuAction::Adjust),
    (shortcut::HELP, MenuAction::Help),
];

/// A acao de uma tecla simples (o caractere ou o nome da tecla).
pub fn key_action(key: &str) -> Option<MenuAction> {
    KEYS.iter().find(|(t, _)| *t == key).map(|(_, a)| *a)
}

/// O atalho tem Cmd (vira `KeyBinding`); senao e uma tecla simples de uma view.
pub fn with_cmd(shortcut: &str) -> bool {
    shortcut.starts_with("cmd-")
}

pub struct MenuDesc {
    pub title: String,
    pub items: Vec<ItemDesc>,
}

pub enum ItemDesc {
    MenuAction { label: String, action_id: &'static str, shortcut: Option<&'static str> },
    Separator,
}

fn item(label: String, a: MenuAction, shortcut: Option<&'static str>) -> ItemDesc {
    ItemDesc::MenuAction { label, action_id: a.id(), shortcut }
}

/// A barra de menus. O primeiro e o menu do app no macOS.
pub fn tree() -> Vec<MenuDesc> {
    use shortcut as t;
    use ItemDesc::Separator;
    use texts::t;
    vec![
        MenuDesc {
            title: t!("menu.title.marginalia"),
            items: vec![
                item(t!("menu.about"), MenuAction::About, None),
                Separator,
                item(t!("menu.settings"), MenuAction::Settings, Some(t::SETTINGS)),
                Separator,
                item(t!("menu.exit"), MenuAction::Exit, Some(t::EXIT)),
            ],
        },
        MenuDesc {
            title: t!("menu.title.file"),
            items: vec![
                item(t!("menu.open_source"), MenuAction::OpenSource, Some(t::OPEN_SOURCE)),
                item(t!("menu.open_vault"), MenuAction::OpenVault, Some(t::OPEN_VAULT)),
                Separator,
                item(t!("menu.close_source"), MenuAction::CloseSource, None),
            ],
        },
        MenuDesc {
            title: t!("menu.title.edit"),
            items: vec![
                item(t!("menu.undo"), MenuAction::Undo, Some(t::UNDO)),
                Separator,
                item(t!("menu.cut"), MenuAction::Cut, Some(t::CUT)),
                item(t!("menu.copy"), MenuAction::Copy, Some(t::COPY)),
                item(t!("menu.paste"), MenuAction::Paste, Some(t::PASTE)),
                item(t!("menu.select_all"), MenuAction::SelectAll, Some(t::SELECT_ALL)),
                Separator,
                item(t!("menu.delete_highlight"), MenuAction::DeleteHighlight, Some(t::DELETE_HIGHLIGHT)),
                item(t!("menu.delete_item"), MenuAction::DeleteItem, None),
            ],
        },
        MenuDesc {
            title: t!("menu.title.exibir"),
            items: vec![
                item(t!("menu.notes"), MenuAction::Notes, Some(t::NOTES)),
                item(t!("menu.sources"), MenuAction::Sources, Some(t::SOURCES)),
                item(t!("menu.tasks"), MenuAction::Tasks, Some(t::TASKS)),
                Separator,
                item(t!("menu.image_adjustments"), MenuAction::AdjustmentsPanel, Some(t::ADJUSTMENTS)),
                item(t!("menu.tools"), MenuAction::Tools, None),
                Separator,
                item(t!("menu.zoom_up"), MenuAction::ZoomUp, Some(t::ZOOM_UP)),
                item(t!("menu.zoom_down"), MenuAction::ZoomDown, Some(t::ZOOM_DOWN)),
                item(t!("menu.adjust"), MenuAction::Adjust, Some(t::ADJUST)),
            ],
        },
        MenuDesc {
            title: t!("menu.title.navigate"),
            items: vec![
                item(t!("menu.previous"), MenuAction::Previous, Some(t::PREVIOUS)),
                item(t!("menu.next"), MenuAction::Next, Some(t::NEXT)),
                Separator,
                item(t!("menu.focus_quick_note"), MenuAction::FocusQuickNote, Some(t::FOCUS_QUICK_NOTE)),
                item(t!("menu.focus_image"), MenuAction::FocusImage, Some(t::FOCUS_IMAGE)),
            ],
        },
        MenuDesc {
            title: t!("menu.title.image"),
            items: vec![
                item(t!("menu.rotate_left"), MenuAction::RotateLeft, Some(t::ROTATE_LEFT)),
                item(t!("menu.rotate_right"), MenuAction::RotateRight, Some(t::ROTATE_RIGHT)),
                Separator,
                item(t!("menu.highlight_box"), MenuAction::HighlightBox, None),
                item(t!("menu.arrow_highlight"), MenuAction::ArrowHighlight, None),
                item(t!("menu.highlight_colors"), MenuAction::HighlightColors, None),
                Separator,
                item(t!("menu.brightness_up"), MenuAction::BrightnessUp, Some(t::BRIGHTNESS_UP)),
                item(t!("menu.brightness_down"), MenuAction::BrightnessDown, Some(t::BRIGHTNESS_DOWN)),
                item(t!("menu.contrast_up"), MenuAction::ContrastUp, Some(t::CONTRAST_UP)),
                item(t!("menu.contrast_down"), MenuAction::ContrastDown, Some(t::CONTRAST_DOWN)),
                item(t!("menu.sharpness_up"), MenuAction::SharpnessUp, None),
                item(t!("menu.sharpness_down"), MenuAction::SharpnessDown, None),
                item(t!("menu.highpass_up"), MenuAction::HighpassUp, None),
                item(t!("menu.highpass_down"), MenuAction::HighpassDown, None),
                item(t!("menu.threshold_up"), MenuAction::ThresholdUp, None),
                item(t!("menu.threshold_down"), MenuAction::ThresholdDown, None),
                Separator,
                item(t!("menu.invert"), MenuAction::Invert, Some(t::INVERT)),
                item(t!("menu.gray"), MenuAction::Gray, Some(t::GRAY)),
                item(t!("menu.threshold"), MenuAction::Threshold, Some(t::THRESHOLD)),
                Separator,
                item(t!("menu.restore"), MenuAction::Restore, Some(t::RESTORE)),
            ],
        },
        MenuDesc {
            title: t!("menu.title.help"),
            items: vec![
                item(t!("menu.help"), MenuAction::Help, Some(t::HELP)),
                Separator,
                item(t!("menu.check_updates"), MenuAction::CheckUpdates, None),
            ],
        },
    ]
}

/// A barra de menus do Windows e do Linux: a mesma arvore, sem o menu do app (o do macOS, com o
/// nome dele) que la nao existe por convencao. Configuracoes e Sair vao para o fim de Arquivo e
/// Sobre para o fim de Ajuda. Nenhum item se perde: so muda de lugar.
pub fn tree_pc() -> Vec<MenuDesc> {
    let mut menus = tree();
    let app = menus.remove(0);
    let (mut about, mut config, mut exit) = (None, None, None);
    for i in app.items {
        match &i {
            ItemDesc::MenuAction { action_id, .. } if *action_id == MenuAction::About.id() => about = Some(i),
            ItemDesc::MenuAction { action_id, .. } if *action_id == MenuAction::Settings.id() => config = Some(i),
            ItemDesc::MenuAction { action_id, .. } if *action_id == MenuAction::Exit.id() => exit = Some(i),
            _ => {}
        }
    }
    let (file, help) = (0, menus.len() - 1);
    menus[file].items.push(ItemDesc::Separator);
    menus[file].items.extend(config);
    menus[file].items.push(ItemDesc::Separator);
    menus[file].items.extend(exit);
    menus[help].items.push(ItemDesc::Separator);
    menus[help].items.extend(about);
    menus
}

/// O item da acao `a` na arvore: (rotulo, atalho).
pub fn item_of(a: MenuAction) -> Option<(String, Option<&'static str>)> {
    tree().into_iter().flat_map(|m| m.items).find_map(|i| match i {
        ItemDesc::MenuAction { label, action_id, shortcut } if action_id == a.id() => Some((label, shortcut)),
        _ => None,
    })
}

/// O atalho como o gpui o entende: `cmd-` (a tecla de comando) vira `secondary-`, que e o Cmd
/// no macOS e o Ctrl no Windows e no Linux. So as `KeyBinding` usam esta forma; o resto do app
/// (tabelas, README, testes) continua falando `cmd-`.
pub fn gpui_to(shortcut: &str) -> String {
    match shortcut.strip_prefix("cmd-") {
        Some(rest) => format!("secondary-{rest}"),
        None => shortcut.to_string(),
    }
}

/// `(comando, controle)` de uma tecla apertada, na regra do app: o "comando" e o Cmd no macOS e
/// o Ctrl nos outros sistemas (la o `control` some, para nao valer duas vezes). Quem le os modificadores de um
/// evento passa por aqui, entao a logica das teclas fala so em "comando".
pub fn command_and_control(platform: bool, control: bool, mac: bool) -> (bool, bool) {
    if mac {
        (platform, control)
    } else {
        (platform || control, false)
    }
}

/// Simbolo do atalho na plataforma: `⇧⌘V` no macOS e no que os testes gravam, `Ctrl+Shift+V` nos outros.
pub fn symbol(shortcut: &str) -> String {
    to_symbol(shortcut, cfg!(target_os = "macos") || cfg!(test))
}

/// `symbol` escolhendo o estilo: `mac` como o macOS mostra, senao como o Windows e o Linux.
pub fn to_symbol(shortcut: &str, mac: bool) -> String {
    if mac {
        symbol_mac(shortcut)
    } else {
        symbol_pc(shortcut)
    }
}

/// `ctrl+shift+v`: separa o atalho em (modificadores, tecla). "-" sozinho (zoom menos) e a tecla "-".
fn shortcut_parts(shortcut: &str) -> (Vec<&str>, &str) {
    let mut parts: Vec<&str> = shortcut.split('-').collect();
    let key = match parts.pop() {
        Some("") => "-",
        Some(t) => t,
        None => "",
    };
    (parts, key)
}

/// Como o Windows e o Linux mostram: `cmd-shift-v` -> `Ctrl+Shift+V`, `left` -> `←`.
fn symbol_pc(shortcut: &str) -> String {
    let (parts, key) = shortcut_parts(shortcut);
    let mut v: Vec<String> = Vec::new();
    if parts.contains(&"cmd") || parts.contains(&"ctrl") {
        v.push("Ctrl".into());
    }
    if parts.contains(&"alt") {
        v.push("Alt".into());
    }
    if parts.contains(&"shift") {
        v.push("Shift".into());
    }
    v.push(match key {
        "left" => "←".to_string(),
        "right" => "→".to_string(),
        "up" => "↑".to_string(),
        "down" => "↓".to_string(),
        "backspace" => "Backspace".to_string(),
        "delete" => "Del".to_string(),
        "enter" => "Enter".to_string(),
        "escape" => "Esc".to_string(),
        "shift" => "Shift".to_string(),
        "alt" => "Alt".to_string(),
        "ctrl" | "cmd" => "Ctrl".to_string(),
        "-" => "−".to_string(),
        t if with_cmd(shortcut) => t.to_uppercase(),
        t => t.to_string(),
    });
    v.join("+")
}

/// Simbolo do atalho como o macOS mostra: `cmd-shift-v` -> `⇧⌘V`, `left` -> `←`.
fn symbol_mac(shortcut: &str) -> String {
    let (parts, key) = shortcut_parts(shortcut);
    let mut s = String::new();
    if parts.contains(&"ctrl") {
        s.push('⌃');
    }
    if parts.contains(&"alt") {
        s.push('⌥');
    }
    if parts.contains(&"shift") {
        s.push('⇧');
    }
    if parts.contains(&"cmd") {
        s.push('⌘');
    }
    let t = match key {
        "left" => "←".to_string(),
        "right" => "→".to_string(),
        "up" => "↑".to_string(),
        "down" => "↓".to_string(),
        "backspace" => "⌫".to_string(),
        "delete" => "⌦".to_string(),
        "enter" => "↵".to_string(),
        "escape" => "Esc".to_string(),
        "shift" => "⇧".to_string(),
        "alt" => "⌥".to_string(),
        "ctrl" => "⌃".to_string(),
        "cmd" => "⌘".to_string(),
        "-" => "−".to_string(),
        t if with_cmd(shortcut) => t.to_uppercase(),
        t => t.to_string(),
    };
    s.push_str(&t);
    s
}

/// Atalhos que os textos citam e que nao sao `MenuAction` de menu: `(id, atalho)`, no
/// mesmo formato de [`shortcut`] (ex.: `("finalizar_nota", "cmd-enter")`). O id e o
/// do marcador `{shortcut:id}` nos textos.
pub const SHORTCUTS_NO_MENU: &[(&str, &str)] = &[
    ("finish_quick_note", "cmd-enter"),
    ("key_enter", "enter"),
    ("key_backspace", "backspace"),
    ("key_delete", "delete"),
    ("key_shift", "shift"),
    ("key_alt", "alt"),
];

/// A tecla, como se mostra, da acao (ou do atalho sem menu) `id`: o que o
/// marcador `{shortcut:id}` vira nos textos. `None`: id desconhecido ou sem tecla.
pub fn shortcut_by_id(id: &str) -> Option<String> {
    if let Some(a) = MenuAction::from_id(id) {
        let of_menu = item_of(a).and_then(|(_, t)| t);
        let simple = KEYS.iter().find(|(_, x)| *x == a).map(|(t, _)| *t);
        return of_menu.or(simple).map(symbol);
    }
    SHORTCUTS_NO_MENU.iter().find(|(i, _)| *i == id).map(|(_, t)| symbol(t))
}

/// Liga os marcadores `{shortcut:id}` dos textos a esta tabela (na partida do app).
pub fn register_app_shortcuts() {
    texts::register_shortcuts(shortcut_by_id);
}

/// Rotulo no menu: atalho com Cmd vai como atalho do item (o macOS mostra a
/// direita); tecla simples vai no proprio rotulo, entre parenteses.
pub fn menu_label(label: &str, shortcut: Option<&str>) -> String {
    match shortcut {
        Some(a) if !with_cmd(a) => format!("{label} ({})", symbol(a)),
        _ => label.to_string(),
    }
}

/// Dica (tooltip) de um botao que faz o mesmo que o item `a`: o rotulo do
/// menu (sem as reticencias) e o atalho, `Abrir fonte (⌘O)`.
pub fn hint(a: MenuAction) -> String {
    let (label, shortcut) = item_of(a).unwrap_or((a.id().to_string(), None));
    let label = label.trim_end_matches('…');
    match shortcut {
        Some(t) => format!("{label} ({})", symbol(t)),
        None => label.to_string(),
    }
}

/// O que a view sabe agora para ligar/desligar os itens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub source_has: bool,
    /// Um grifo selecionado na imagem.
    pub selected: bool,
    /// A imagem tem o foco (Excluir grifo, como Delete: o Cmd+Z que o devolve so vale la).
    pub image_in_focus: bool,
    /// `focus_state::target_undo` tem o que desfazer.
    pub undo: bool,
    /// Gaveta de notas/tarefas em foco com uma linha escolhida e nada modal aberto.
    pub delete_item: bool,
}

/// O item `a` se aplica agora? Os que nao se aplicam ficam desligados no menu
/// (a acao da view so e registrada quando se aplica). Os do app e os de texto
/// sao decididos por quem os trata.
pub fn available(a: MenuAction, e: &State) -> bool {
    match a {
        MenuAction::CloseSource
        | MenuAction::Previous
        | MenuAction::Next
        | MenuAction::ZoomUp
        | MenuAction::ZoomDown
        | MenuAction::Adjust
        | MenuAction::BrightnessUp
        | MenuAction::BrightnessDown
        | MenuAction::ContrastUp
        | MenuAction::ContrastDown
        | MenuAction::SharpnessUp
        | MenuAction::SharpnessDown
        | MenuAction::HighpassUp
        | MenuAction::HighpassDown
        | MenuAction::ThresholdUp
        | MenuAction::ThresholdDown
        | MenuAction::Invert
        | MenuAction::Gray
        | MenuAction::Threshold
        | MenuAction::Restore
        | MenuAction::RotateLeft
        | MenuAction::RotateRight
        | MenuAction::HighlightBox
        | MenuAction::ArrowHighlight
        | MenuAction::HighlightColors => e.source_has,
        MenuAction::DeleteHighlight => e.selected && e.image_in_focus,
        MenuAction::Undo => e.undo,
        MenuAction::DeleteItem => e.delete_item,
        _ => true,
    }
}

#[cfg(test)]
#[path = "tests/menus.rs"]
mod tests;
