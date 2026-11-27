#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Marginalia: casca do app. Janela com a imagem (zoom, arrasto), barra minima,
//! navegacao pelas setas, painel de ajustes (Cmd-F) e "Abrir fonte" (Cmd-O).
//!
//! Uso: `cargo run -p interface [-- caminho [--cache] [--vault pasta]]`,
//! onde `file_path` e uma pasta, um zip (`--cache` copia para o cache local em
//! vez de ler direto) ou uma imagem/PDF avulso. Sem caminho a janela abre vazia, com o convite para abrir uma fonte.
//! Vault: `--vault pasta`, senao `MARGINALIA_VAULT`, senao o ultimo vault dos
//! dados do app, se ainda existir; sem nenhum, o seletor de vault abre sozinho
//! (Cmd-Shift-V o abre depois: vaults conhecidos, "Abrir pasta...", "Criar vault
//! aqui"). Cmd-Q, Cmd-O e Cmd-Shift-V sao acoes do app, com teclas e menu, e
//! valem de qualquer foco (mesmo sem foco). A barra de menus do macOS (Task 10)
//! sai de `menus::tree` e espelha todos os botoes e atalhos (`menu_actions!`).
//!

mod help;
mod config;
mod focus_state;
mod icons;
mod main_view;
mod menu_bar;
mod menus;
mod quick_note_view;
mod slider;
mod about;
mod splash;
mod theme;
mod smoke;
mod viewer;

// Os modulos de estado moram no crate `state`; aqui na raiz para os caminhos curtos (`drawers::...`).
use state::{
    schedule_adjustment, annotate, app_state, load, drawers, layout, quick_note_session, open_source, quick_note, relink, splash_vault, vault_picker,
    viewer_math,
};

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use gpui::{
    actions, point, prelude::*, px, size, Action, App, Application, Bounds, KeyBinding, Menu, MenuItem, OsAction, Point,
    TitlebarOptions, WindowBounds, WindowHandle, WindowOptions,
};
use crate::menus::{MenuAction, ItemDesc};
use marginalia_core::app_data::AppData;
use marginalia_core::source::cleanup_cache;
use marginalia_core::vault::Vault;
use texts::t;

use crate::app_state::{choose_vault, AppState, ZipMode};
use crate::main_view::MainView;
use crate::smoke::run_smoke;

/// Imagem de amostra que o smoke copia para montar as fontes dele (a janela nao a abre mais sozinha).
const SAMPLE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../marginalia_core/integration/fixtures/amostra.jpg");
/// Posicao dos botoes do macOS (a partir do canto de cima, a esquerda): o centro deles cai no
/// meio da barra de titulo (`layout::TITLE_HEIGHT`).
const TRAFFIC_LIGHT: Point<gpui::Pixels> = point(px(12.0), px(9.0));

/// Uma acao do gpui por item de menu (`menus::MenuAction`), e as funcoes que ligam
/// as duas: a acao de cada item, o inverso, a `KeyBinding` de um atalho e o
/// registro na view raiz das acoes dela que se aplicam agora. Tudo sai da
/// mesma lista, entao um item sem acao (ou uma acao sem item) nao compila.
macro_rules! menu_actions {
    ($($kind:ident => $menu_action:ident),* $(,)?) => {
        actions!(marginalia, [$($kind),*]);

        /// A acao do gpui do item `a`.
        fn action_to_gpui(a: MenuAction) -> Box<dyn Action> {
            match a {
                $(MenuAction::$menu_action => Box::new($kind),)*
            }
        }

        /// O item de uma acao do gpui (o inverso de `action_to_gpui`).
        #[cfg_attr(not(test), allow(dead_code))]
        fn action_from_gpui(a: &dyn Action) -> Option<MenuAction> {
            $(
                if a.as_any().is::<$kind>() {
                    return Some(MenuAction::$menu_action);
                }
            )*
            None
        }

        /// `KeyBinding` de `keys` para o item `a`, no `context` (`menus::Context::name`).
        fn binding(keys: &str, a: MenuAction, context: Option<&str>) -> KeyBinding {
            match a {
                $(MenuAction::$menu_action => KeyBinding::new(keys, $kind, context),)*
            }
        }

        /// Registra na view raiz as acoes dela (`menus::Reach::View`) que se
        /// aplicam agora (`menus::available`): as outras nao estao no caminho
        /// do foco, e o macOS mostra o item desligado (`is_action_available`).
        pub fn register_in_view(mut d: gpui::Div, e: &menus::State, cx: &mut gpui::Context<MainView>) -> gpui::Div {
            $(
                if MenuAction::$menu_action.reach() == menus::Reach::View && menus::available(MenuAction::$menu_action, e) {
                    d = d.on_action(cx.listener(|v: &mut MainView, _: &$kind, w, cx| v.menu_command(MenuAction::$menu_action, w, cx)));
                }
            )*
            d
        }
    };
}

menu_actions! {
    About => About,
    Exit => Exit,
    OpenSource => OpenSource,
    VaultPicker => OpenVault,
    CloseSource => CloseSource,
    Undo => Undo,
    Cut => Cut,
    Copy => Copy,
    Paste => Paste,
    SelectAll => SelectAll,
    DeleteHighlight => DeleteHighlight,
    DeleteItem => DeleteItem,
    ShowNotes => Notes,
    ShowSources => Sources,
    ShowTasks => Tasks,
    ShowAdjustments => AdjustmentsPanel,
    IncreaseZoom => ZoomUp,
    DecreaseZoom => ZoomDown,
    AdjustWindow => Adjust,
    PreviousImage => Previous,
    NextImage => Next,
    FocusQuickNote => FocusQuickNote,
    FocusImage => FocusImage,
    IncreaseBrightness => BrightnessUp,
    DecreaseBrightness => BrightnessDown,
    IncreaseContrast => ContrastUp,
    DecreaseContrast => ContrastDown,
    IncreaseSharpness => SharpnessUp,
    DecreaseSharpness => SharpnessDown,
    IncreaseHighpass => HighpassUp,
    DecreaseHighpass => HighpassDown,
    IncreaseThreshold => ThresholdUp,
    DecreaseThreshold => ThresholdDown,
    Invert => Invert,
    GrayTones => Gray,
    Threshold => Threshold,
    RestoreAdjustments => Restore,
    RotateToLeft => RotateLeft,
    RotateToRight => RotateRight,
    ToggleHighlightBox => HighlightBox,
    ToggleHighlightArrow => ArrowHighlight,
    ShowColorsHighlight => HighlightColors,
    ToggleTools => Tools,
    ShowHelp => Help,
    ShowSettings => Settings,
}

/// O `OsAction` dos itens de edicao (o macOS manda `cut:`/`copy:`/`paste:`/
/// `selectAll:` pela cadeia de respondedores: tambem valem nos paineis do sistema).
fn os_action(a: MenuAction) -> Option<OsAction> {
    match a {
        MenuAction::Undo => Some(OsAction::Undo),
        MenuAction::Cut => Some(OsAction::Cut),
        MenuAction::Copy => Some(OsAction::Copy),
        MenuAction::Paste => Some(OsAction::Paste),
        MenuAction::SelectAll => Some(OsAction::SelectAll),
        _ => None,
    }
}

/// A barra de menus do macOS montada da arvore de `menus` (testada sem gpui).
fn menus_gpui() -> Vec<Menu> {
    menus::tree()
        .into_iter()
        .map(|m| Menu {
            name: m.title.into(),
            items: m
                .items
                .into_iter()
                .map(|i| match i {
                    ItemDesc::Separator => MenuItem::separator(),
                    ItemDesc::MenuAction { label, action_id, shortcut } => {
                        // `menus::tests` garante que todo acao_id resolve.
                        let a = MenuAction::from_id(action_id).expect("acao_id desconhecido");
                        MenuItem::Action {
                            name: menus::menu_label(&label, shortcut).into(),
                            action: action_to_gpui(a),
                            os_action: os_action(a),
                        }
                    }
                })
                .collect(),
        })
        .collect()
}

/// Pastas temporarias do smoke, apagadas na saida (sucesso ou falha).
static TEMPORARY: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

fn temporary(p: &Path) {
    TEMPORARY.lock().unwrap_or_else(|e| e.into_inner()).push(p.to_path_buf());
}

fn erase_temporary() {
    for p in TEMPORARY.lock().unwrap_or_else(|e| e.into_inner()).drain(..) {
        let _ = std::fs::remove_dir_all(p);
    }
}

struct Args {
    file_path: Option<PathBuf>,
    mode: ZipMode,
    vault: Option<PathBuf>,
}

fn args() -> Args {
    let mut file_path = None;
    let mut mode = ZipMode::Direct;
    let mut vault = None;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        if a == "--cache" {
            mode = ZipMode::Cache;
        } else if a == "--vault" {
            vault = it.next().map(PathBuf::from);
        } else if file_path.is_none() {
            file_path = Some(PathBuf::from(a));
        }
    }
    Args { file_path, mode, vault }
}

/// Abre o vault escolhido (R12) e o lembra como ultimo vault.
fn open_vault(cli: Option<PathBuf>, app_data: &mut AppData) -> (Option<Vault>, Option<String>) {
    let env = std::env::var_os("MARGINALIA_VAULT").map(PathBuf::from);
    let Some(folder) = choose_vault(cli, env, app_data.last_vault()) else {
        return (None, None);
    };
    match Vault::open_or_create(&folder) {
        Ok(v) => {
            app_data.set_last_vault(folder);
            let warning = app_data.save().err().map(|e| t!("warning.remember_vault", e));
            (Some(v), warning)
        }
        Err(e) => (None, Some(t!("error.open_vault", folder.display(), e))),
    }
}

fn app_data_base() -> PathBuf {
    static BASE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    BASE.get_or_init(|| {
        let smoke = std::env::var("MARGINALIA_SMOKE").is_ok_and(|v| v == "1");
        match std::env::var_os("MARGINALIA_APPDATA").filter(|v| !v.is_empty()) {
            Some(p) => PathBuf::from(p),
            // O smoke nunca toca os dados do app do usuario.
            None if smoke => {
                let p = std::env::temp_dir().join(format!("marginalia-smoke-appdata-{}", std::process::id()));
                println!("smoke: MARGINALIA_APPDATA nao definido; dados do app em {} (temporario, apagado na saida)", p.display());
                temporary(&p);
                p
            }
            None => AppData::default_base(),
        }
    })
    .clone()
}

/// Dados do app. Arquivo ilegivel nao e sobrescrito: a sessao usa uma pasta
/// temporaria e avisa.
fn load_app_data() -> (AppData, Option<String>) {
    let base = app_data_base();
    match AppData::load(base.clone()) {
        Ok(d) => (d, None),
        Err(e) => {
            let tmp = std::env::temp_dir().join(format!("interfacedata-{}", std::process::id()));
            let warning = t!("warning.unreadable_app_data", base.display(), e);
            eprintln!("{warning}");
            (AppData::new(tmp), Some(warning))
        }
    }
}

/// O que a janela principal precisa para abrir (o vault vem separado: pode ser
/// escolhido na tela da partida).
struct Start {
    app_data: AppData,
    warning: Option<String>,
    file_path: Option<PathBuf>,
    mode: ZipMode,
    cache_dir: PathBuf,
}

/// Abre a janela principal com `vault` e a fonte da linha de comando, e liga o
/// que e dela: acoes, menus e a gravacao da nota ao sair.
fn open_principal(cx: &mut App, vault: Option<Vault>, p: Start) -> WindowHandle<MainView> {
    let Start { app_data, warning, file_path, mode, cache_dir } = p;
    theme::set(app_data.theme());
    let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        // Titulo nativo transparente: a barra de titulo e pintada pelo app (`MainView::title_bar`),
        // na cor da barra de status, e os botoes do macOS ficam por cima dela.
        titlebar: Some(TitlebarOptions {
            title: Some(t!("app.name").into()),
            appears_transparent: true,
            traffic_light_position: Some(TRAFFIC_LIGHT),
        }),
        focus: true,
        ..Default::default()
    };
    let mut state = AppState::new(vault, app_data, cache_dir);
    state.warning = warning;
    let view_window = cx
        .open_window(options, |window, cx| {
            cx.new(|cx| {
                let mut v = MainView::new(state, window, cx);
                // Sem fonte na linha de comando, a configuracao "reabrir a ultima fonte" abre a
                // mais recente do vault que ainda existe (sumiu: a janela abre vazia, sem aviso).
                let open = match file_path {
                    Some(file_path) => {
                        let name = open_source::default_name(&file_path);
                        Some((name, file_path))
                    }
                    None => v.last_source_to_reopen(),
                };
                if let Some((name, file_path)) = open {
                    v.open_path(file_path, name, mode, window, cx);
                }
                v.viewer().read(cx).focus_handle().focus(window);
                v
            })
        })
        .expect("abrir a janela");

    app_actions(view_window, cx);
    cx.on_app_quit(move |cx| {
        // A nota em edicao vai para o disco (texto e id fixado) antes de sair;
        // a ultima posicao aberta so vai para o disco em trocas e na saida.
        let _ = view_window.update(cx, |v, _, _| {
            v.flush_quick_note();
            v.save_position()
        });
        async {}
    })
    .detach();
    view_window
}

/// A janelinha de escolha do vault (antes da principal). Escolhido um vault, a
/// principal abre e a janelinha fecha; fechar a janelinha encerra o app.
fn vault_open_screen(cx: &mut App, start: Start) {
    theme::set(start.app_data.theme());
    let items = splash_vault::items(start.app_data.known_vaults(), |p| p.is_dir());
    let warning = start.warning.clone();
    let start = std::rc::Rc::new(std::cell::RefCell::new(Some(start)));
    let bounds = Bounds::centered(None, size(px(480.0), px(560.0)), cx);
    // Sem barra de titulo nem botoes de janela (os do macOS ficam fora da tela; a view desenha o "x"
    // do canto); a faixa nativa de cima continua arrastando a janela.
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some(t!("app.name").into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(-100.0), px(-100.0))),
        }),
        is_resizable: false,
        is_minimizable: false,
        focus: true,
        ..Default::default()
    };
    let view_window = cx
        .open_window(options, |window, cx| {
            let on_choose: splash::OnChoose = Box::new(move |vault, window, cx| {
                let Some(mut p) = start.borrow_mut().take() else { return };
                p.app_data.set_last_vault(vault.root.clone());
                p.warning = p.app_data.save().err().map(|e| t!("warning.remember_vault", e));
                // A principal abre antes de a janelinha sair: nunca ha um instante sem janela (o app sairia).
                open_principal(cx, Some(vault), p);
                window.remove_window();
            });
            let _ = window;
            cx.new(|_| splash::SplashView::new(items, warning, on_choose))
        })
        .expect("abrir a janela de escolha do vault");
    let _ = view_window;
}

fn main() {
    menus::register_app_shortcuts();
    let Args { file_path, mode, vault } = args();
    let smoke = std::env::var("MARGINALIA_SMOKE").is_ok_and(|v| v == "1");
    let (mut app_data, warning) = load_app_data();
    // Sem `--vault` nem `MARGINALIA_VAULT`, a tela de escolha aparece (o smoke sempre diz o vault).
    let env = std::env::var_os("MARGINALIA_VAULT").map(PathBuf::from);
    let with_screen = !smoke && splash_vault::screen_needs(vault.as_deref(), env.as_deref());
    let (vault, warning) = if with_screen {
        (None, warning)
    } else {
        let (v, vault_warning) = open_vault(vault, &mut app_data);
        (v, warning.or(vault_warning))
    };
    // Um cache por processo: duas instancias nao apagam o cache uma da outra.
    let cache_dir = app_data_base().join("cache").join(std::process::id().to_string());

    Application::new().with_assets(icons::Assets).run(move |cx: &mut App| {
        let start = Start { app_data, warning, file_path, mode, cache_dir: cache_dir.clone() };
        let cache = cache_dir.clone();
        cx.on_app_quit(move |_| {
            cleanup_cache(&cache);
            erase_temporary();
            async {}
        })
        .detach();
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);

        if with_screen {
            vault_open_screen(cx, start);
            return;
        }
        let view_window = open_principal(cx, vault, start);

        if smoke {
            let expected = std::env::var("MARGINALIA_SMOKE_ERROS").ok().and_then(|v| v.parse::<usize>().ok());
            cx.spawn(async move |cx| match run_smoke(view_window, cx, expected).await {
                Ok(()) => {
                    let _ = cx.update(|cx| cx.quit());
                }
                Err(msg) => {
                    eprintln!("smoke: FALHOU: {msg}");
                    cleanup_cache(&cache_dir);
                    erase_temporary();
                    std::process::exit(1);
                }
            })
            .detach();
        }
    });
}

/// Os modificadores de uma tecla na regra do app: fora do macOS, o Ctrl vale como o "comando"
/// (`platform`) que o resto do codigo espera (`menus::command_and_control`).
pub(crate) fn normalize_modifiers(m: gpui::Modifiers) -> gpui::Modifiers {
    let (platform, control) = menus::command_and_control(m.platform, m.control, cfg!(target_os = "macos"));
    gpui::Modifiers { platform, control, ..m }
}

/// Teclas e menus do app. `menus::SHORTCUTS` vira `KeyBinding` (Cmd-Q, Cmd-O,
/// Cmd-Shift-V, Cmd-K, Cmd-Shift-K globais: casam antes dos listeners das
/// views, entao valem com a caixa de nota em foco e mesmo sem nenhum foco;
/// recortar/copiar/colar/selecionar tudo so na caixa de texto; Cmd-1/2/3, Cmd-Z
/// e Cmd-Backspace so para o menu mostrar o atalho, a tecla fica com a view).
/// As acoes globais rodam aqui; as da view raiz, em `register_in_view`.
fn app_actions(view_window: WindowHandle<MainView>, cx: &mut App) {
    cx.bind_keys(menus::SHORTCUTS.iter().map(|(t, a, c)| binding(&menus::gpui_to(t), *a, c.name())));
    // Fora do macOS nao ha menu nativo para levar as teclas `SoMenu` ate a view (Ctrl-F, Ctrl-I,
    // brilho, contraste, girar, Configuracoes...): elas valem de qualquer foco, como o item do menu.
    if !cfg!(target_os = "macos") {
        cx.bind_keys(menus::shortcuts_no_menu_native().into_iter().map(|(t, a)| binding(&menus::gpui_to(t), a, None)));
    }
    // A janela esta ocupada durante o despacho da tecla: a acao roda logo depois,
    // no mesmo metodo que o botao chama (`MainView::command`).
    let in_window = move |a: MenuAction| {
        move |cx: &mut App| {
            cx.defer(move |cx| {
                let _ = view_window.update(cx, |v, w, cx| v.command(a, w, cx));
            })
        }
    };
    let (quick_note, image, open, vault) =
        (in_window(MenuAction::FocusQuickNote), in_window(MenuAction::FocusImage), in_window(MenuAction::OpenSource), in_window(MenuAction::OpenVault));
    cx.on_action(move |_: &FocusQuickNote, cx| quick_note(cx));
    cx.on_action(move |_: &FocusImage, cx| image(cx));
    cx.on_action(move |_: &OpenSource, cx| open(cx));
    cx.on_action(move |_: &VaultPicker, cx| vault(cx));
    // Sair: o `on_app_quit` grava a nota em edicao e apaga o cache.
    cx.on_action(|_: &Exit, cx| cx.quit());
    // Depois das teclas: o menu le o atalho de cada item no keymap.
    cx.set_menus(menus_gpui());
}

#[cfg(test)]
#[path = "tests/main.rs"]
mod tests;
