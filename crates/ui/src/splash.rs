//! Tela de escolha do vault na partida (janelinha propria, antes da principal).
//! So desenha e traduz eventos: as linhas, a selecao e a validacao do vault
//! vem de `splash_vault` e `vault_picker` (sem gpui, regra R4).

use std::path::PathBuf;

use gpui::{div, prelude::*, px, rgb, App, Context, FontWeight, MouseButton, MouseDownEvent, PathPromptOptions, Window, WindowControlArea};
use marginalia_core::vault::Vault;
use texts::t;

use crate::icons::{icon, Icon};
use crate::splash_vault::{self, ItemSplash};
use crate::vault_picker;

/// Chamada quando um vault foi aberto: a janela principal toma o lugar da tela.
pub type OnChoose = Box<dyn Fn(Vault, &mut Window, &mut App)>;

pub struct SplashView {
    items: Vec<ItemSplash>,
    warning: Option<String>,
    /// Abrindo um vault no fundo: os cliques esperam.
    busy: bool,
    on_choose: OnChoose,
}

impl SplashView {
    pub fn new(items: Vec<ItemSplash>, warning: Option<String>, on_choose: OnChoose) -> Self {
        Self { items, warning, busy: false, on_choose }
    }

    /// Valida e abre `root` no fundo; com `create`, prepara as subpastas.
    fn open(&mut self, root: PathBuf, create: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.warning = None;
        cx.spawn_in(window, async move |this, cx| {
            let r = cx.background_executor().spawn(async move { vault_picker::open(&root, create) }).await;
            let _ = this.update_in(cx, |v, window, cx| {
                v.busy = false;
                match r {
                    Ok(vault) => (v.on_choose)(vault, window, cx),
                    Err(e) => v.warning = Some(e.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn folder_dialog(&mut self, create: bool, window: &mut Window, cx: &mut Context<Self>) {
        let choice = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(if create { t!("btn.create_vault_here").into() } else { t!("dialog.vault.open").into() }),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(file_paths))) = choice.await else { return };
            let Some(folder) = file_paths.into_iter().next() else { return };
            let _ = this.update_in(cx, |v, window, cx| v.open(folder, create, window, cx));
        })
        .detach();
    }

    fn choose_line(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        match splash_vault::choose(&self.items, i) {
            Some(root) => self.open(root, false, window, cx),
            None => {
                if let Some(it) = self.items.get(i) {
                    self.warning = Some(t!("splash.not_found_warning", it.file_path.display()));
                    cx.notify();
                }
            }
        }
    }
}

fn btn(label: String, cx: &mut Context<SplashView>, f: impl Fn(&mut SplashView, &mut Window, &mut Context<SplashView>) + 'static) -> impl IntoElement {
    div()
        .px_3()
        .py_1()
        .rounded_md()
        .bg(rgb(crate::theme::current().btn))
        .hover(|s| s.bg(rgb(crate::theme::current().button_hover)))
        .cursor_pointer()
        .child(label)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |v, _: &MouseDownEvent, w, cx| {
                cx.stop_propagation();
                f(v, w, cx)
            }),
        )
}

impl Render for SplashView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = div().id("splash-lista").flex().flex_col().gap_1().p_2().w_full().flex_1().min_h(px(0.0)).overflow_y_scroll().rounded_lg().bg(rgb(crate::theme::current().field)).border_1().border_color(rgb(crate::theme::current().border));
        for (i, it) in self.items.iter().enumerate() {
            let color = if it.exists { crate::theme::current().text } else { crate::theme::current().weak_text };
            let mut name = div().font_weight(FontWeight::BOLD).text_color(rgb(color)).child(it.label.clone());
            if !it.exists {
                name = name.child(div().text_xs().text_color(rgb(crate::theme::current().warning)).child(t!("splash.not_found")));
            }
            list = list.child(
                div()
                    .id(("splash-vault", i))
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(crate::theme::current().hover_soft)))
                    .child(name)
                    .child(div().text_xs().text_color(rgb(crate::theme::current().soft_text)).truncate().child(it.file_path.display().to_string()))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |v, _: &MouseDownEvent, w, cx| v.choose_line(i, w, cx)),
                    ),
            );
        }
        div()
            .id("splash")
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .gap_4()
            .px_6()
            .pt(px(48.0))
            .pb_4()
            .bg(rgb(crate::theme::current().bar))
            .text_color(rgb(crate::theme::current().text))
            .text_sm()
            .child(icon(Icon::Vault, 72.0, rgb(crate::theme::current().soft_text)))
            .child(div().text_2xl().font_weight(FontWeight::BOLD).child(t!("app.name")))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(btn(t!("splash.open_vault"), cx, |v, w, cx| v.folder_dialog(false, w, cx)))
                    .child(btn(t!("splash.create_vault"), cx, |v, w, cx| v.folder_dialog(true, w, cx))),
            )
            .when_some(self.warning.clone(), |d, a| d.child(div().text_xs().text_color(rgb(crate::theme::current().warning)).child(a)))
            .child(list)
            .child(close(window, cx))
            // No Windows o sistema nao desenha barra de titulo: a faixa do topo, a direita do "x",
            // arrasta a janela (no macOS a faixa nativa ja faz isso). Nao cobre o "x": uma area
            // de arrasto sob o clique o engoliria.
            .when(cfg!(target_os = "windows"), |d| {
                d.child(div().absolute().top(px(0.0)).left(px(44.0)).right(px(0.0)).h(px(48.0)).window_control_area(WindowControlArea::Drag))
            })
    }
}

/// Fechar a janela (encerra o app: o vault e obrigatorio). A janela nao tem
/// barra de titulo, entao o "x" fica no canto.
fn close(_: &mut Window, cx: &mut Context<SplashView>) -> impl IntoElement {
    div()
        .id("splash-fechar")
        .absolute()
        .top(px(10.0))
        .left(px(10.0))
        .size(px(28.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(rgb(crate::theme::current().btn))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(crate::theme::current().button_hover)))
        .child(icon(Icon::Close, 16.0, rgb(crate::theme::current().medium_text)))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|_, _: &MouseDownEvent, w, _| w.remove_window()),
        )
}
