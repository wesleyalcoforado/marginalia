//! A tela de ajuda (atalhos).

use super::*;

impl MainView {
    /// O seletor de vault, por cima da janela (posicao e largura de
    /// `layout::picker_textbox`: nunca passa da borda numa janela estreita).
    /// Tela de ajuda: painel sobreposto com todos os comandos (`help::groups`), em tres
    /// colunas de dois assuntos. Clicar fora ou Esc/`?` fecha.
    pub(super) fn help_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let column = |groups: &[help::Group]| {
            let mut c = div().flex_1().min_w(px(0.0)).flex().flex_col().gap_4();
            for g in groups {
                let mut block = div().flex().flex_col().child(
                    div().pb_1().text_xs().font_weight(FontWeight::BOLD).text_color(rgb(crate::theme::current().emphasis)).child(g.title.to_uppercase()),
                );
                for l in &g.lines {
                    block = block.child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .gap_2()
                            .py(px(2.0))
                            .border_b_1()
                            .border_color(rgb(crate::theme::current().soft_border))
                            .text_color(rgb(crate::theme::current().medium_text))
                            .child(div().min_w(px(0.0)).child(l.description.clone()))
                            .child(div().flex_none().text_color(rgb(crate::theme::current().text)).child(l.keys.clone())),
                    );
                }
                c = c.child(block);
            }
            c
        };
        let groups = help::groups();
        div()
            .id("ajuda-fundo")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_start()
            .justify_center()
            .pt(px(48.0))
            .bg(gpui::rgba(0x00000066))
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|v, _: &MouseDownEvent, _, cx| {
                    v.open_help = false;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .w_full()
                    .max_w(px(900.0))
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
                            .flex()
                            .flex_row()
                            .items_center()
                            .px_4()
                            .py_3()
                            .border_b_1()
                            .border_color(rgb(crate::theme::current().border))
                            .child(div().flex_1().font_weight(FontWeight::BOLD).child(t!("help.title")))
                            .child(div().text_xs().text_color(rgb(crate::theme::current().weak_text)).child(t!("help.close"))),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_6()
                            .p_4()
                            .child(column(&groups[0..2]))
                            .child(column(&groups[2..4]))
                            .child(column(&groups[4..6])),
                    ),
            )
    }
}
