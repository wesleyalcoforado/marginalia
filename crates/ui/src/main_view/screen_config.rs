//! A tela de Configuracoes (sobreposta, como a ajuda). O que ela mostra e o que cada
//! opcao faz esta em `config` (sem gpui); aqui so se desenha e se liga o clique.

use super::*;
use crate::config;

impl MainView {
    /// A tela de Configuracoes esta aberta (o smoke confere).
    pub fn config_open(&self) -> bool {
        self.config_open
    }

    /// Cmd-, , a engrenagem da barra de atividades e o menu Marginalia: abre/fecha. Ao abrir, o foco
    /// vai para a imagem (nada que se digite vai parar na nota por baixo) e as outras telas fecham.
    pub fn toggle_config(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.config_open = !self.config_open;
        if self.config_open {
            self.open_help = false;
            self.open_colors = false;
            self.focus_image(window, cx);
        }
        cx.notify();
    }

    /// O ritmo de salvamento que a sessao da nota usa agora (o smoke confere).
    pub fn quick_note_rhythm(&self) -> marginalia_core::app_data::QuickNoteRhythm {
        self.session.rhythm()
    }

    /// O tamanho de letra que a caixa da nota usa agora (o smoke confere).
    pub fn quick_note_size_font(&self, cx: &App) -> Option<f32> {
        self.quick_note.read(cx).font_size()
    }

    /// A largura do painel das gavetas agora (o smoke confere).
    pub fn panel_width(&self) -> f32 {
        self.drawers.width()
    }

    /// Na partida, sem fonte na linha de comando: a ultima fonte do vault, se a configuracao
    /// "reabrir a ultima fonte" estiver ligada e ela ainda existir (nome e caminho).
    pub fn last_source_to_reopen(&self) -> Option<(String, PathBuf)> {
        let vault = self.state.vault.as_ref()?;
        let a = &self.state.app_data;
        if !a.reopen_last_source() {
            return None;
        }
        a.last_source_existing(&vault.root)
    }

    /// Escolhe a opcao `i` da linha `id`: muda nos dados do app, aplica na hora e grava no fundo.
    pub fn choose_config(&mut self, id: config::Id, i: usize, cx: &mut Context<Self>) {
        if config::choose(&mut self.state.app_data, id, i) {
            self.apply_config(cx);
            self.write_adjustments(cx);
        }
        cx.notify();
    }

    /// Leva as configuracoes dos dados do app para o que ja esta rodando (a partida e cada troca).
    pub(super) fn apply_config(&mut self, cx: &mut Context<Self>) {
        // O tema vale para todas as janelas do app: troca a paleta e manda repintar tudo.
        crate::theme::set(self.state.app_data.theme());
        cx.refresh_windows();
        self.session.set_rhythm(self.state.app_data.quick_note_rhythm());
        let px = self.state.app_data.quick_note_font().pixels();
        self.quick_note.update(cx, |n, cx| n.set_size_font(Some(px), cx));
        self.drawers.set_width(self.state.app_data.drawers_width().unwrap_or(crate::drawers::WIDTH));
    }

    /// "Restaurar padroes": volta todas as configuracoes da tela ao que eram de fabrica.
    pub fn restore_config(&mut self, cx: &mut Context<Self>) {
        self.state.app_data.restore_settings();
        self.apply_config(cx);
        self.write_adjustments(cx);
        cx.notify();
    }

    pub(super) fn screen_config(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut note_body = div().id("config-corpo").flex_1().min_h(px(0.0)).overflow_y_scroll().flex().flex_col().gap_4().p_4();
        let mut n = 0usize;
        for section in config::sections(&self.state.app_data) {
            let mut block = div().flex().flex_col().gap_3().child(
                div().text_xs().font_weight(FontWeight::BOLD).text_color(rgb(crate::theme::current().soft_text)).child(section.title.to_uppercase()),
            );
            for line in section.lines {
                let id = line.id;
                let mut options = div().flex_none().flex().flex_row().rounded_sm().border_1().border_color(rgb(crate::theme::current().strong_border)).overflow_hidden();
                for (i, o) in line.options.into_iter().enumerate() {
                    n += 1;
                    let mut b = div()
                        .id(("config-opcao", n))
                        .px_3()
                        .py_1()
                        .text_sm()
                        .cursor_pointer()
                        .child(o.label)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |v, _: &MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                v.choose_config(id, i, cx)
                            }),
                        );
                    b = if o.active {
                        b.bg(rgb(crate::theme::current().background_accent)).text_color(rgb(crate::theme::current().strong_text))
                    } else {
                        b.hover(|s| s.bg(rgb(crate::theme::current().hover_soft))).text_color(rgb(crate::theme::current().medium_text))
                    };
                    options = options.child(b);
                }
                let mut text = div().flex_1().min_w(px(0.0)).flex().flex_col().gap_0p5().child(div().child(line.title));
                if let Some(d) = line.hint {
                    text = text.child(div().text_xs().text_color(rgb(crate::theme::current().soft_text)).child(d));
                }
                block = block.child(div().flex().flex_row().items_center().gap_4().child(text).child(options));
            }
            note_body = note_body.child(block);
        }
        div()
            .id("config-fundo")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_start()
            .justify_center()
            .pt(px(48.0))
            .pb(px(16.0))
            .bg(gpui::rgba(0x00000066))
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|v, _: &MouseDownEvent, _, cx| {
                    v.config_open = false;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .w_full()
                    .max_w(px(640.0))
                    .mx_4()
                    .max_h_full()
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .rounded_md()
                    .bg(rgb(crate::theme::current().bar))
                    .border_1()
                    .border_color(rgb(crate::theme::current().strong_border))
                    .text_sm()
                    // O painel nao fecha quando se clica nele.
                    .on_mouse_down(MouseButton::Left, |_: &MouseDownEvent, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .flex_row()
                            .items_center()
                            .px_4()
                            .py_3()
                            .border_b_1()
                            .border_color(rgb(crate::theme::current().border))
                            .child(div().flex_1().font_weight(FontWeight::BOLD).child(t!("config.title")))
                            .child(div().text_xs().text_color(rgb(crate::theme::current().weak_text)).child(t!("config.close"))),
                    )
                    .child(note_body)
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .flex_row()
                            .justify_end()
                            .px_4()
                            .py_3()
                            .border_t_1()
                            .border_color(rgb(crate::theme::current().border))
                            .child(btn(t!("config.restore"), cx, |v, _, cx| v.restore_config(cx))),
                    ),
            )
    }
}
