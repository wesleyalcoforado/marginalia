//! A janela "Sobre o Marginalia": uma janela do macOS propria, pequena e de tamanho fixo. O que ela
//! mostra vem de `about` (sem gpui); aqui so se desenha, se abre (uma so por vez) e se trata o clique
//! no link e o Esc.

use super::*;
use crate::about;
use gpui::{Bounds, Render, TitlebarOptions, WindowBounds, WindowOptions};

/// O conteudo da janela Sobre (uma entidade na janela propria dela).
pub(super) struct ViewAbout {
    focus_state: FocusHandle,
}

impl Render for ViewAbout {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = about::content();
        let t = crate::theme::current();
        let app_icon = crate::icons::app_icon(window, 96.0);
        let link = div()
            .id("sobre-link")
            .cursor_pointer()
            .underline()
            .text_color(rgb(t.accent))
            .hover(|s| s.text_color(rgb(t.accent_hover)))
            .tooltip(hint_of(about::SITE))
            .child(c.link_label)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    cx.open_url(about::SITE);
                }),
            );
        div()
            .id("sobre")
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .gap_2()
            .px_6()
            .pt_6()
            .pb_4()
            .bg(rgb(t.bar))
            .text_color(rgb(t.text))
            .text_sm()
            .track_focus(&self.focus_state)
            .on_key_down(cx.listener(|_, ev: &KeyDownEvent, window, _| {
                if ev.keystroke.key == "escape" {
                    window.remove_window();
                }
            }))
            .child(app_icon)
            .child(div().pt_2().text_2xl().font_weight(FontWeight::BOLD).child(c.name))
            .child(div().text_color(rgb(t.soft_text)).child(c.version))
            .child(div().pt_2().text_center().text_color(rgb(t.medium_text)).child(c.description))
            .child(div().flex_1())
            .child(div().child(c.developer))
            .child(link)
            .child(div().pt_4().text_xs().text_center().text_color(rgb(t.soft_text)).child(c.right))
    }
}

impl MainView {
    /// "Marginalia > Sobre o Marginalia": abre a janela Sobre. Com ela ja aberta, so a traz para a frente.
    pub(super) fn about(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(h) = self.about_window {
            if h.update(cx, |_, w, _| w.activate_window()).is_ok() {
                return;
            }
        }
        let bounds = Bounds::centered(None, size(px(340.0), px(480.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions { title: Some(about::content().title.into()), ..Default::default() }),
            is_resizable: false,
            is_minimizable: false,
            focus: true,
            ..Default::default()
        };
        let open = cx.open_window(options, |window, cx| {
            let focus_state = cx.focus_handle();
            window.focus(&focus_state);
            cx.new(|_| ViewAbout { focus_state })
        });
        self.about_window = open.ok();
        if let Some(h) = self.about_window {
            // Fechar a janela principal fecha o Sobre junto (o app sai quando nao sobra janela).
            cx.on_release(move |_, cx| {
                let _ = h.update(cx, |_, w, _| w.remove_window());
            })
            .detach();
        }
    }

    /// A janela Sobre esta aberta (o smoke confere).
    pub fn open_about(&self, cx: &App) -> bool {
        self.about_window.is_some_and(|h| cx.windows().iter().any(|w| w.window_id() == h.window_id()))
    }

    /// Fecha a janela Sobre, se aberta (o smoke usa).
    pub fn close_about(&mut self, cx: &mut Context<Self>) {
        if let Some(h) = self.about_window.take() {
            let _ = h.update(cx, |_, w, _| w.remove_window());
        }
    }
}
