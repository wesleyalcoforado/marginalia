//! A janela do aviso de versao: uma janela propria, pequena e de tamanho fixo, no estilo da janela
//! Sobre. O que ela mostra vem de `update_notice` (sem gpui); aqui so se desenha, se abre (uma so
//! por vez), se trata os botoes e o Esc, e se dispara a checagem de fundo.

use super::*;
use crate::update_notice::{self, Button, Content};
use gpui::{Bounds, Render, TitlebarOptions, WeakEntity, WindowBounds, WindowOptions};
use marginalia_core::update::{self, Check, Notice, Release, Version};

/// O conteudo da janela do aviso (uma entidade na janela propria dela).
pub(super) struct ViewUpdate {
    content: Content,
    release: Option<Release>,
    main: WeakEntity<MainView>,
    focus_state: FocusHandle,
}

impl ViewUpdate {
    /// O que cada botao faz.
    fn press(&mut self, b: Button, window: &mut Window, cx: &mut Context<Self>) {
        match b {
            Button::Download => {
                if let Some(r) = &self.release {
                    cx.open_url(&r.url);
                }
            }
            Button::Skip => {
                if let Some(r) = &self.release {
                    let version = r.version;
                    let _ = self.main.update(cx, |v, cx| v.skip_update(version, cx));
                }
            }
            Button::Later | Button::Ok => {}
        }
        window.remove_window();
    }
}

impl Render for ViewUpdate {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = crate::theme::current();
        let buttons: Vec<_> = self
            .content
            .buttons
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let b = *b;
                div()
                    .id(("aviso-botao", i))
                    .flex_none()
                    .px_3()
                    .py(px(3.0))
                    .rounded_sm()
                    .bg(rgb(if b == Button::Download { t.accent } else { t.btn }))
                    .hover(move |s| s.bg(rgb(if b == Button::Download { t.accent_hover } else { t.button_hover })))
                    .cursor_pointer()
                    .child(update_notice::label(b))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            this.press(b, window, cx);
                        }),
                    )
            })
            .collect();
        div()
            .id("aviso-versao")
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
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                if ev.keystroke.key == "escape" {
                    this.press(Button::Later, window, cx);
                }
            }))
            .child(crate::icons::app_icon(window, 64.0))
            .child(div().pt_2().text_center().text_lg().font_weight(FontWeight::BOLD).child(self.content.heading.clone()))
            .child(div().text_center().text_color(rgb(t.medium_text)).child(self.content.detail.clone()))
            .child(div().pt_1().text_xs().text_color(rgb(t.soft_text)).child(self.content.notes.clone()))
            .child(div().flex_1())
            .child(div().flex().flex_row().gap_2().children(buttons))
    }
}

impl MainView {
    /// Checa se ha versao nova (Ajuda > Buscar atualizacoes e, ao abrir, a checagem automatica):
    /// a rede roda no executor de fundo e o resultado volta para a janela. `manual` ignora o
    /// intervalo e a versao pulada e sempre responde; a automatica fica calada se nada ha a dizer.
    pub fn check_updates(&mut self, manual: bool, cx: &mut Context<Self>) {
        let current = update::parse_own(env!("CARGO_PKG_VERSION"));
        let skipped = self.state.app_data.skipped_version();
        cx.spawn(async move |this, cx| {
            let (check, notice) = cx
                .background_executor()
                .spawn(async move { state::update_check::run(current, skipped, manual) })
                .await;
            let _ = this.update(cx, |v, cx| v.finish_update_check(check, notice, manual, current, cx));
        })
        .detach();
    }

    /// A checagem automatica da partida: so se ligada, com o intervalo vencido e (no smoke) com
    /// endereco proprio.
    pub(super) fn start_auto_update_check(&mut self, cx: &mut Context<Self>) {
        let now = chrono::Utc::now().timestamp();
        let smoke = std::env::var_os("MARGINALIA_SMOKE").is_some();
        if state::update_check::auto_enabled(&self.state.app_data, now, smoke) {
            self.check_updates(false, cx);
        }
    }

    fn finish_update_check(&mut self, check: Check, notice: Notice, manual: bool, current: Version, cx: &mut Context<Self>) {
        if check != Check::Failed {
            self.state.app_data.set_last_update_check(chrono::Utc::now().timestamp());
            self.write_adjustments(cx);
        }
        if !manual && self.update_shown {
            return;
        }
        if self.show_update(&notice, current, cx) && !manual {
            self.update_shown = true;
        }
    }

    /// Abre (ou troca) a janela do aviso; `false` quando nao ha nada a mostrar.
    fn show_update(&mut self, notice: &Notice, current: Version, cx: &mut Context<Self>) -> bool {
        let Some(content) = update_notice::content(notice, current) else { return false };
        if let Some(h) = self.update_window.take() {
            let _ = h.update(cx, |_, w, _| w.remove_window());
        }
        let release = match notice {
            Notice::Show(r) => Some(r.clone()),
            _ => None,
        };
        let main = cx.entity().downgrade();
        let bounds = Bounds::centered(None, size(px(380.0), px(320.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions { title: Some(content.title.clone().into()), ..Default::default() }),
            is_resizable: false,
            is_minimizable: false,
            focus: true,
            ..Default::default()
        };
        let open = cx.open_window(options, |window, cx| {
            let focus_state = cx.focus_handle();
            window.focus(&focus_state);
            cx.new(|_| ViewUpdate { content, release, main, focus_state })
        });
        self.update_window = open.ok();
        if let Some(h) = self.update_window {
            // Fechar a janela principal fecha o aviso junto.
            cx.on_release(move |_, cx| {
                let _ = h.update(cx, |_, w, _| w.remove_window());
            })
            .detach();
        }
        true
    }

    /// "Pular esta versao": guarda nos dados do app (so volta a avisar de uma versao maior).
    pub fn skip_update(&mut self, version: Version, cx: &mut Context<Self>) {
        self.state.app_data.set_skipped_version(Some(version));
        self.write_adjustments(cx);
    }

    /// A janela do aviso esta aberta (o smoke confere).
    pub fn open_update(&self, cx: &App) -> bool {
        self.update_window.is_some_and(|h| cx.windows().iter().any(|w| w.window_id() == h.window_id()))
    }

    /// Fecha a janela do aviso, se aberta (o smoke usa).
    pub fn close_update(&mut self, cx: &mut Context<Self>) {
        if let Some(h) = self.update_window.take() {
            let _ = h.update(cx, |_, w, _| w.remove_window());
        }
    }
}
