//! Pecas de desenho reutilizadas: botoes, dica, chip, caixas.

use super::*;

/// Dica (tooltip): uma caixa com o texto, uma linha por `\n`, que quebra linhas longas.
pub(super) struct Hint(SharedString);

impl Render for Hint {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .max_w(px(520.0))
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(rgb(crate::theme::current().hover_soft))
            .border_1()
            .border_color(rgb(crate::theme::current().track))
            .text_xs()
            .text_color(rgb(crate::theme::current().text))
            .children(self.0.split('\n').map(|l| div().child(l.to_string())))
    }
}

/// Construtor de dica para `.tooltip(...)`.
pub(super) fn hint_of(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    move |_, cx| cx.new(|_| Hint(text.clone())).into()
}

/// Mensagem passageira da barra de status: cortada com `…` se nao couber, inteira na dica.
pub(super) fn message(id: &'static str, text: &str, color: u32) -> impl IntoElement {
    div()
        .id(id)
        .min_w(px(0.0))
        .truncate()
        .text_color(rgb(color))
        .tooltip(hint_of(text.to_string()))
        .child(text.to_string())
}

/// Icone da barra de atividades / da barra da direita: realcado quando o painel
/// esta aberto; a dica diz o nome e o atalho.
pub(super) fn activity_button(
    id: &'static str,
    i: Icon,
    hint: impl Into<SharedString>,
    active: bool,
    cx: &mut Context<MainView>,
    f: impl Fn(&mut MainView, &mut Window, &mut Context<MainView>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .tooltip(hint_of(hint))
        .w_full()
        .h(px(44.0))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .border_l_2()
        .border_color(rgb(if active { crate::theme::current().accent } else { crate::theme::current().lateral }))
        .hover(|s| s.bg(rgb(crate::theme::current().hover_soft)))
        .child(icon(i, 22.0, rgb(if active { crate::theme::current().strong_text } else { crate::theme::current().soft_text })))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |v, _: &MouseDownEvent, w, cx| {
                cx.stop_propagation();
                f(v, w, cx)
            }),
        )
}

/// Botao so com icone (barra de status, fechar gaveta): alvo de 28 px (icone de
/// 16 px + 6 px de cada lado) e dica com o nome e o atalho.
pub(super) fn icon_button(
    id: &'static str,
    i: Icon,
    hint: impl Into<SharedString>,
    cx: &mut Context<MainView>,
    f: impl Fn(&mut MainView, &mut Window, &mut Context<MainView>) + 'static,
) -> impl IntoElement {
    button_icon_se(id, i, hint, true, cx, f)
}

/// Botao so de icone que pode estar desabilitado: apagado, sem realce nem clique (a dica
/// continua, para dizer o que ele faria).
pub(super) fn button_icon_se(
    id: &'static str,
    i: Icon,
    hint: impl Into<SharedString>,
    enabled: bool,
    cx: &mut Context<MainView>,
    f: impl Fn(&mut MainView, &mut Window, &mut Context<MainView>) + 'static,
) -> impl IntoElement {
    let textbox = div().id(id).tooltip(hint_of(hint)).flex_none().p(px(6.0)).rounded_sm();
    if !enabled {
        return textbox.opacity(0.3).child(icon(i, 16.0, rgb(crate::theme::current().medium_text))).into_any_element();
    }
    textbox
        .cursor_pointer()
        .hover(|s| s.bg(rgb(crate::theme::current().button_hover)))
        .child(icon(i, 16.0, rgb(crate::theme::current().medium_text)))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |v, _: &MouseDownEvent, w, cx| {
                cx.stop_propagation();
                f(v, w, cx)
            }),
        )
        .into_any_element()
}

/// A caixa de marcar de uma tarefa: quadrado vazio (pendente) ou quadrado com visto (feita).
pub(super) fn button_check(
    id: &'static str,
    done: bool,
    cx: &mut Context<MainView>,
    f: impl Fn(&mut MainView, &mut Window, &mut Context<MainView>) + 'static,
) -> impl IntoElement {
    let (i, color, hint) = if done {
        (Icon::Marked, crate::theme::current().ok, t!("drawer.task.hint_reopen"))
    } else {
        (Icon::Square, crate::theme::current().medium_text, t!("drawer.task.hint_conclude"))
    };
    div()
        .id(id)
        .tooltip(hint_of(hint))
        .flex_none()
        .mt(px(1.0))
        .p(px(1.0))
        .rounded_sm()
        .cursor_pointer()
        .hover(|s| s.bg(rgb(crate::theme::current().button_hover)))
        .child(icon(i, 14.0, rgb(color)))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |v, _: &MouseDownEvent, w, cx| {
                cx.stop_propagation();
                f(v, w, cx)
            }),
        )
}

/// Botao com icone e rotulo.
pub(super) fn button_with_icon(
    i: Icon,
    label: impl Into<SharedString>,
    cx: &mut Context<MainView>,
    f: impl Fn(&mut MainView, &mut Window, &mut Context<MainView>) + 'static,
) -> impl IntoElement {
    div()
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .px_1()
        .rounded_sm()
        .bg(rgb(crate::theme::current().btn))
        .hover(|s| s.bg(rgb(crate::theme::current().button_hover)))
        .cursor_pointer()
        .child(icon(i, 12.0, rgb(crate::theme::current().text)))
        .child(label.into())
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |v, _: &MouseDownEvent, w, cx| {
                cx.stop_propagation();
                f(v, w, cx)
            }),
        )
}

/// Botao de dialogo: o que tem o foco ganha a borda azul (Enter o aciona);
/// `danger` pinta de vermelho (Excluir).
pub(super) fn dialog_button(
    id: &'static str,
    i: Option<Icon>,
    label: impl Into<SharedString>,
    focused: bool,
    danger: bool,
    cx: &mut Context<MainView>,
    f: impl Fn(&mut MainView, &mut Window, &mut Context<MainView>) + 'static,
) -> impl IntoElement {
    let (background, highlight) = if danger { (0x8a3030, 0xa04040) } else { (crate::theme::current().btn, crate::theme::current().button_hover) };
    let mut b = div()
        .id(id)
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .px_2()
        .py(px(2.0))
        .rounded_sm()
        .bg(rgb(background))
        .hover(move |s| s.bg(rgb(highlight)))
        .cursor_pointer()
        .border_1()
        .border_color(rgb(if focused { crate::theme::current().accent } else { background }));
    if let Some(i) = i {
        b = b.child(icon(i, 12.0, rgb(crate::theme::current().text)));
    }
    b.child(label.into()).on_mouse_down(
        MouseButton::Left,
        cx.listener(move |v, _: &MouseDownEvent, w, cx| {
            cx.stop_propagation();
            f(v, w, cx)
        }),
    )
}

pub(super) fn btn(
    label: impl Into<SharedString>,
    cx: &mut Context<MainView>,
    f: impl Fn(&mut MainView, &mut Window, &mut Context<MainView>) + 'static,
) -> impl IntoElement {
    div()
        .px_2()
        .rounded_sm()
        .bg(rgb(crate::theme::current().btn))
        .hover(|s| s.bg(rgb(crate::theme::current().button_hover)))
        .cursor_pointer()
        .child(label.into())
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |v, _: &MouseDownEvent, w, cx| {
                cx.stop_propagation();
                f(v, w, cx)
            }),
        )
}

/// Botao de filtro (realcado quando ativo).
pub(super) fn chip(
    label: impl Into<SharedString>,
    active: bool,
    cx: &mut Context<MainView>,
    f: impl Fn(&mut MainView, &mut Window, &mut Context<MainView>) + 'static,
) -> impl IntoElement {
    div()
        .px_2()
        .rounded_sm()
        .text_xs()
        .bg(rgb(if active { crate::theme::current().accent } else { crate::theme::current().btn }))
        .hover(move |s| s.bg(rgb(if active { crate::theme::current().accent_hover } else { crate::theme::current().button_hover })))
        .cursor_pointer()
        .child(label.into())
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |v, _: &MouseDownEvent, w, cx| {
                cx.stop_propagation();
                f(v, w, cx)
            }),
        )
}

/// Chip de etiqueta na cor pastel dela: ativo = fundo pastel com texto escuro; inativo =
/// texto pastel sobre o fundo neutro.
pub(super) fn chip_tag(
    name: &str,
    color: u32,
    active: bool,
    cx: &mut Context<MainView>,
    f: impl Fn(&mut MainView, &mut Window, &mut Context<MainView>) + 'static,
) -> impl IntoElement {
    div()
        .px_2()
        .rounded_sm()
        .text_xs()
        .bg(rgb(if active { color } else { crate::theme::current().btn }))
        .text_color(rgb(if active { 0x1e1e1e } else { color }))
        .hover(move |s| s.bg(rgb(if active { color } else { crate::theme::current().button_hover })).opacity(0.9))
        .cursor_pointer()
        .child(format!("#{name}"))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |v, _: &MouseDownEvent, w, cx| {
                cx.stop_propagation();
                f(v, w, cx)
            }),
        )
}

/// Uma linha de lista suspensa com caixa de marcar (`☑` / `☐`).
pub(super) fn markable_option(
    label: impl Into<SharedString>,
    marked: bool,
    id: SharedString,
    cx: &mut Context<MainView>,
    f: impl Fn(&mut MainView, &mut Window, &mut Context<MainView>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .px_1()
        .rounded_sm()
        .text_xs()
        .hover(|s| s.bg(rgb(crate::theme::current().hover_soft)))
        .cursor_pointer()
        .child(div().text_color(rgb(if marked { crate::theme::current().accent } else { crate::theme::current().soft_text })).child(if marked { "☑" } else { "☐" }))
        .child(div().flex_1().min_w(px(0.0)).truncate().child(label.into()))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |v, _: &MouseDownEvent, w, cx| {
                cx.stop_propagation();
                f(v, w, cx)
            }),
        )
}

/// A lista rolavel com um cursor de rolagem a direita (so aparece quando a
/// lista passa da altura; a roda e o trackpad rolam, o cursor so mostra onde se esta).
pub(super) fn with_bar(list: gpui::Stateful<gpui::Div>, scroll: &gpui::ScrollHandle, cx: &mut Context<MainView>) -> gpui::Div {
    let visible: f32 = scroll.bounds().size.height.into();
    let max: f32 = scroll.max_offset().height.into();
    let offset: f32 = scroll.offset().y.into();
    // Respiro a direita: os cards nao ficam sob a trilha.
    let mut textbox = div().relative().flex_1().min_h(px(0.0)).child(list.pr(px(12.0)));
    if let Some((top, alt)) = drawers::scroll_bar(visible, visible + max, offset) {
        // Apertar na trilha leva o polegar ao ponteiro e comeca o arrasto (que a janela
        // inteira acompanha: `scroll_drag_thread`); a trilha engole o clique, senao
        // cairia no card de baixo e abriria a nota.
        let r = scroll.clone();
        textbox = textbox.child(
            div()
                .id("barra-rolagem")
                .group("barra-rolagem")
                .occlude()
                .absolute()
                .top_0()
                .right_0()
                .bottom_0()
                .w(px(10.0))
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |v, e: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        scroll_to(&r, e.position.y);
                        v.scroll_drag = Some(r.clone());
                        cx.notify();
                    }),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(3.0))
                        .w(px(4.0))
                        .top(px(top * visible))
                        .h(px(alt * visible))
                        .rounded_full()
                        .bg(rgb(crate::theme::current().control))
                        .group_hover("barra-rolagem", |s| s.bg(rgb(crate::theme::current().control_hover))),
                ),
        );
    }
    textbox
}

/// Leva o polegar da barra de `scroll` ao ponteiro (`y`, em coordenadas da janela).
pub(super) fn scroll_to(r: &gpui::ScrollHandle, y: gpui::Pixels) {
    let v: f32 = r.bounds().size.height.into();
    let m: f32 = r.max_offset().height.into();
    let y_rel: f32 = (y - r.bounds().origin.y).into();
    r.set_offset(point(r.offset().x, px(drawers::bar_offset(y_rel, v, v + m))));
}

pub(super) fn hint(t: impl Into<SharedString>) -> impl IntoElement {
    let t: SharedString = t.into();
    div().text_xs().text_color(rgb(crate::theme::current().soft_text)).child(t)
}
