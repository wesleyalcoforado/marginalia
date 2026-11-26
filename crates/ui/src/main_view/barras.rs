//! As barras: titulo, atividades, ferramentas e status.

use super::*;
use gpui::{anchored, deferred, point, Corner, WindowControlArea};

impl MainView {
    /// O que a barra de status mostra (vault, fonte, pagina, imagem, arquivo).
    pub fn status_info(&self) -> StatusInfo {
        self.state.status_info()
    }

    /// Barra de atividades (esquerda): Notas, Fontes, Tarefas abrem/fecham o
    /// painel esquerdo (um por vez); embaixo ficam a ajuda e a engrenagem (Configuracoes).
    pub(super) fn activities_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let open = self.drawers.open();
        let item = |g: Drawer, i: Icon, cx: &mut Context<Self>| {
            let id = match g {
                Drawer::Notes => "atividade-notas",
                Drawer::Sources => "atividade-fontes",
                Drawer::Tasks => "atividade-tarefas",
            };
            activity_button(id, i, g.hint(), open == Some(g), cx, move |v, w, cx| v.toggle_drawer(g, w, cx))
        };
        div()
            .w(px(layout::ACTIVITIES))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(crate::theme::current().lateral))
            .border_r_1()
            .border_color(rgb(crate::theme::current().border))
            .child(item(Drawer::Notes, Icon::FileSearch, cx))
            .child(item(Drawer::Sources, Icon::Files, cx))
            .child(item(Drawer::Tasks, Icon::Marked, cx))
            .child(div().flex_1())
            .child(activity_button("atividade-ajuda", Icon::Help, menus::hint(menus::MenuAction::Help), self.open_help, cx, |v, w, cx| {
                v.command(menus::MenuAction::Help, w, cx)
            }))
            .child(activity_button(
                "atividade-config",
                Icon::AdjustmentsPanel,
                menus::hint(menus::MenuAction::Settings),
                self.config_open,
                cx,
                |v, w, cx| v.command(menus::MenuAction::Settings, w, cx),
            ))
    }

    /// Barra fina da direita: o icone de ajustes, sempre visivel, abre/fecha o painel.
    pub(super) fn right_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(layout::RIGHT_BAR))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .bg(rgb(crate::theme::current().lateral))
            .border_l_1()
            .border_color(rgb(crate::theme::current().border))
            .child(activity_button("ajustes", Icon::Brush, menus::hint(menus::MenuAction::AdjustmentsPanel), self.open_adjustments, cx, |v, w, cx| {
                v.command(menus::MenuAction::AdjustmentsPanel, w, cx)
            }))
    }

    /// Barra de status (embaixo, largura toda): abrir fonte, quadro anterior /
    /// seguinte, o texto de `layout::text_status` (clicar abre o seletor de
    /// vault) e, a direita, o que e passageiro: abrindo/carregando, trabalho
    /// exclusivo, confirmacao, aviso e "nota descartada · Desfazer (u)".
    ///
    /// As mensagens ficam com a largura natural (ate
    /// `layout::width_max_messages`); quem encolhe com `…` e o texto vault ·
    /// fonte · .... Aviso ou confirmacao mais longos que isso sao cortados, e o
    /// texto inteiro aparece na dica ao passar o mouse.
    /// Barra de titulo (em cima, largura toda): a janela tem o titulo nativo transparente
    /// (`main.rs`), entao o app pinta a faixa, na cor da barra de status. No macOS os botoes
    /// ficam por cima dela e a faixa nativa por baixo segue arrastando a janela e dando zoom no
    /// duplo clique; em tela cheia o macOS esconde a barra e a faixa tambem. No Windows o sistema
    /// nao desenha nada: o app marca a area de arrasto (o titulo) e desenha minimizar, maximizar
    /// e fechar (`window_buttons`).
    pub(super) fn title_bar(&self, window: &Window) -> impl IntoElement {
        let t = crate::theme::current();
        let bar = div()
            .w_full()
            .h(px(layout::TITLE_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .bg(rgb(t.bar))
            .border_b_1()
            .border_color(rgb(t.border))
            .text_xs()
            .text_color(rgb(t.soft_text));
        if !cfg!(target_os = "windows") {
            return bar.justify_center().child(t!("app.name"));
        }
        // Espaco do mesmo tamanho dos botoes, a esquerda, para o titulo ficar no centro. A area de
        // arrasto e irma dos botoes (nunca a pai): o gpui devolve a primeira area desenhada sob o
        // mouse, e uma area pai engoliria os botoes.
        let buttons_width = 3.0 * WIDTH_BUTTON_WINDOW;
        bar
            .child(div().flex_none().w(px(buttons_width)).h_full())
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .window_control_area(WindowControlArea::Drag)
                    .child(t!("app.name")),
            )
            .child(window_buttons(window.is_maximized()))
    }

    /// Barra de menus fora do macOS (la o sistema desenha a do app; aqui nao): os titulos de
    /// `menu_bar::view` (a `menus::tree_pc()` com os itens ligados ou desligados) em fila, e o
    /// menu aberto colado embaixo do titulo. Os itens chamam o mesmo `command` do menu do macOS.
    /// O mouse e o teclado (`on_key_menu`) mexem no mesmo estado, `MenuBar`: titulo em
    /// destaque, menu aberto e item marcado. As letras de acesso (mnemonicos) so aparecem
    /// sublinhadas com o `Alt` apertado ou com o menu aberto pelo teclado, como no Windows. O visual segue o dos menus do
    /// Windows: titulos e itens retos e de largura toda, coluna dos icones a esquerda e o atalho
    /// a direita.
    pub(super) fn render_menu_bar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = crate::theme::current();
        let state = self.state_menu(window, cx);
        let underline = self.menu.keyboard() || self.alt_held;
        if self.menu.open_at().is_none() {
            self.limits_menu.set(None);
        }
        let mut bar = div()
            .w_full()
            .h(px(layout::MENU_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .bg(rgb(t.bar))
            .border_b_1()
            .border_color(rgb(t.border))
            .text_sm()
            .text_color(rgb(t.text));
        for (i, menu) in crate::menu_bar::view(&state).into_iter().enumerate() {
            let in_emphasis = self.menu.title() == Some(i);
            let is_open = self.menu.open_at() == Some(i);
            let mut title = div()
                .id(("menu-titulo", i))
                .relative()
                .h_full()
                .px(px(10.0))
                .flex()
                .items_center()
                .hover(|s| s.bg(rgb(crate::theme::current().hover_soft)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |v, _: &MouseDownEvent, _, cx| {
                        v.menu.click(i);
                        cx.notify();
                    }),
                )
                .on_hover(cx.listener(move |v, passed: &bool, _, cx| {
                    if *passed {
                        let before = v.menu.clone();
                        v.menu.pass(i);
                        if v.menu != before {
                            cx.notify();
                        }
                    }
                }))
                .child(text_with_mnemonic(&menu.title, menu.mnemonic, underline));
            if in_emphasis {
                title = title.bg(rgb(t.hover_soft));
            }
            if is_open {
                // O `anchored` parte de onde o elemento cairia no fluxo: no titulo (`items_center`) o
                // menu, mais alto que a barra, seria centrado para cima do topo e o gpui o encaixaria
                // em y=0, por cima da barra. O involucro absoluto no canto do titulo fixa a origem.
                title = title.child(
                    div().absolute().top_0().left_0().child(
                        deferred(
                            anchored()
                                .anchor(Corner::TopLeft)
                                .offset(point(px(0.0), px(layout::MENU_HEIGHT - 1.0)))
                                .child(self.dropdown_menu(i, menu.items, underline, cx)),
                        )
                        .with_priority(1),
                    ),
                );
            }
            bar = bar.child(title);
        }
        bar
    }

    /// O menu `i` aberto: coluna dos icones (vazia) e rotulo a esquerda, atalho (so os com Ctrl) a
    /// direita; os que nao se aplicam agora ficam apagados e nao respondem ao clique. O item
    /// marcado pelo teclado tem o mesmo realce do mouse em cima.
    fn dropdown_menu(&self, i: usize, items: Vec<crate::menu_bar::ItemView>, underline: bool, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::menu_bar::ItemView;
        let t = crate::theme::current();
        let limits = self.limits_menu.clone();
        let marked = self.menu.item();
        let mut list = div()
            .id(("menu-suspenso", i))
            .relative()
            .min_w(px(240.0))
            .py(px(3.0))
            .flex()
            .flex_col()
            .bg(rgb(t.raised))
            .border_1()
            .border_color(rgb(t.strong_border))
            .shadow_sm()
            .text_sm()
            .text_color(rgb(t.text))
            .occlude()
            // Onde o menu foi desenhado de fato (o smoke confere a posicao; nao pinta nada).
            .child(canvas(move |b, _, _| limits.set(Some(b)), |_, _, _, _| {}).absolute().top_0().left_0().size_full());
        for (j, item) in items.into_iter().enumerate() {
            match item {
                ItemView::Separator => {
                    list = list.child(div().h(px(1.0)).my(px(4.0)).ml(px(28.0)).bg(rgb(t.border)));
                }
                ItemView::MenuAction { label, menu_action, key, enabled, mnemonic } => {
                    let mut line = div()
                        .id(("menu-item", i * 100 + j))
                        .h(px(26.0))
                        .pl(px(28.0))
                        .pr(px(14.0))
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .gap(px(32.0))
                        .child(text_with_mnemonic(&label, mnemonic, underline))
                        .child(div().text_color(rgb(if enabled { t.soft_text } else { t.weak_text })).child(key));
                    if enabled {
                        if marked == Some(j) {
                            line = line.bg(rgb(t.hover_soft));
                        }
                        line = line.hover(|s| s.bg(rgb(crate::theme::current().hover_soft))).on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |v, _: &MouseDownEvent, w, cx| {
                                cx.stop_propagation();
                                v.menu.close();
                                cx.notify();
                                v.command(menu_action, w, cx);
                            }),
                        );
                    } else {
                        line = line.text_color(rgb(t.weak_text));
                    }
                    list = list.child(line);
                }
            }
        }
        list
    }

    /// Por baixo do menu aberto, da barra de menus para baixo: clicar fora fecha o menu (e o clique
    /// nao passa adiante). A barra de titulo e a de menus ficam de fora, para trocar de menu pelo mouse.
    pub(super) fn menu_background(&self, full_screen: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let top = if full_screen { layout::MENU_HEIGHT } else { layout::TITLE_HEIGHT + layout::MENU_HEIGHT };
        deferred(
            div()
                .id("menu-fundo")
                .absolute()
                .top(px(top))
                .left_0()
                .right_0()
                .bottom_0()
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|v, _: &MouseDownEvent, _, cx| {
                        v.menu.close();
                        cx.notify();
                    }),
                ),
        )
    }

    /// Botao de cores da barra de ferramentas: o icone da paleta com uma bolinha da cor
    /// atual; ligado enquanto o menuzinho (ancorado embaixo, por cima de tudo) esta aberto.
    fn colors_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let a = menus::MenuAction::HighlightColors;
        let is_open = self.open_colors;
        let base = icon_button("ferr-cores", Icon::Palette, menus::hint(a), cx, move |v, w, cx| v.command(a, w, cx));
        let mut textbox = div().flex_none().relative().rounded_sm().border_1().border_color(rgb(crate::theme::current().bar));
        if is_open {
            textbox = textbox.bg(rgb(crate::theme::current().linked_background)).border_color(rgb(crate::theme::current().linked_border));
        }
        let dot = div()
            .absolute()
            .right(px(2.0))
            .bottom(px(2.0))
            .size(px(8.0))
            .rounded_full()
            .border_1()
            .border_color(rgb(0x1a1a1a))
            .bg(rgb(annotate::color_rgb(&self.state.current_color())));
        let textbox = textbox.child(base).child(dot);
        if is_open {
            let menu = self.colors_menu(cx);
            textbox.child(deferred(anchored().anchor(Corner::TopLeft).offset(point(px(0.0), px(layout::TOOLS_HEIGHT - 2.0))).child(menu)))
        } else {
            textbox
        }
    }

    /// Barra de ferramentas opcional (menu Exibir), no topo da area da imagem: grifo
    /// caixa e seta (a ferramenta ligada fica azul) | girar | zoom. Cada botao chama o
    /// mesmo `command` do item de menu e usa a dica dele.
    pub(super) fn tools_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        use menus::MenuAction as A;
        let is_on = self.viewer.read(cx).tool();
        let btn = |id: &'static str, i: Icon, a: A, enabled: bool, cx: &mut Context<Self>| {
            let base = icon_button(id, i, menus::hint(a), cx, move |v, w, cx| v.command(a, w, cx));
            let mut textbox = div().flex_none().rounded_sm().border_1().border_color(rgb(crate::theme::current().bar));
            if enabled {
                textbox = textbox.bg(rgb(crate::theme::current().linked_background)).border_color(rgb(crate::theme::current().linked_border));
            }
            textbox.child(base)
        };
        // Dica do "Ir para": quantas imagens (ou paginas) a fonte tem.
        let hint_go_to = match self.state.unit_go_to() {
            Some((crate::app_state::UnitIr::Page, _, n)) => t!("tools.go_to_total_pages", n),
            Some((crate::app_state::UnitIr::Image, _, n)) => t!("tools.go_to_total_images", n),
            None => t!("tools.go_to_hint"),
        };
        let separator = || div().flex_none().w(px(1.0)).h(px(16.0)).mx_1().bg(rgb(crate::theme::current().border));
        div()
            .w_full()
            .h(px(layout::TOOLS_HEIGHT))
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .px_2()
            .overflow_hidden()
            .bg(rgb(crate::theme::current().bar))
            .border_b_1()
            .border_color(rgb(crate::theme::current().border))
            .child(btn("ferr-caixa", Icon::Textbox, A::HighlightBox, is_on == annotate::Tool::Textbox, cx))
            .child(btn("ferr-seta", Icon::Arrow, A::ArrowHighlight, is_on == annotate::Tool::Arrow, cx))
            .child(self.colors_button(cx))
            .child(separator())
            .child(btn("ferr-girar-esq", Icon::RotateLeft, A::RotateLeft, false, cx))
            .child(btn("ferr-girar-dir", Icon::RotateRight, A::RotateRight, false, cx))
            .child(separator())
            .child(btn("ferr-zoom-menos", Icon::ZoomDown, A::ZoomDown, false, cx))
            .child(btn("ferr-zoom-mais", Icon::ZoomUp, A::ZoomUp, false, cx))
            .child(btn("ferr-ajustar", Icon::Adjust, A::Adjust, false, cx))
            .child(separator())
            .child(div().flex_none().text_xs().px_1().child(t!("tools.ir_to")))
            .child(
                div()
                    .flex_none()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(
                        div()
                            .id("ir-para")
                            .flex_none()
                            .w(px(56.0))
                            .tooltip(hint_of(hint_go_to))
                            // Antes do editor (fase de captura): ↑/↓ mudam o numero, nao o cursor.
                            .capture_key_down(cx.listener(|v, ev: &KeyDownEvent, w, cx| {
                                let m = &ev.keystroke.modifiers;
                                if m.platform || m.control || m.alt {
                                    return;
                                }
                                if let Some(d) = slider::key_step(&ev.keystroke.key, m.shift) {
                                    cx.stop_propagation();
                                    v.step_go_to(d, w, cx);
                                }
                            }))
                            .child(self.ir_to.clone()),
                    )
                    // As setinhas de uma caixa numerica (como as dos navegadores).
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .flex_col()
                            .child(arrow_go_to("ir-para-mais", Icon::Above, 1, cx))
                            .child(arrow_go_to("ir-para-menos", Icon::Below, -1, cx)),
                    ),
            )
    }

    /// ↑/↓ (Shift: 10) ou as setinhas do "Ir para": muda o numero da caixa (a partir do que
    /// esta digitado, ou de onde estamos), sempre entre 1 e o total. Quem
    /// leva a imagem ate la e o Enter.
    pub fn step_go_to(&mut self, delta: i32, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_, current, total)) = self.state.unit_go_to() else { return };
        // Digitando (ou ja passeando com as setas): parte do numero da caixa; senao, de onde estamos.
        let typed = if self.go_to_dirty { crate::app_state::interpret_go_to(self.ir_to.read(cx).text()) } else { None };
        let new = (typed.unwrap_or(current as i64).saturating_add(delta as i64)).clamp(1, total as i64);
        // O numero escolhido fica na caixa ate Enter/Esc (nao volta para a imagem atual).
        self.go_to_dirty = true;
        self.ir_to.update(cx, |n, cx| n.load_text(&new.to_string(), cx));
        self.ir_to.read(cx).focus_handle().focus(window);
        cx.notify();
    }

    /// A caixa do "Ir para" (o smoke digita nela).
    pub fn textbox_go_to(&self) -> &Entity<QuickNoteBox> {
        &self.ir_to
    }

    /// Enter no "Ir para": vai a imagem (ou pagina) digitada, limitada ao primeiro e ao
    /// ultimo; o que nao e numero avisa e a caixa fica como esta.
    pub fn confirm_go_to(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        if text.trim().is_empty() {
            self.cancel_go_to(window, cx);
            return;
        }
        let Some(n) = crate::app_state::interpret_go_to(text) else {
            self.state.warning = Some(t!("warning.ir_to.not_and_number", text.trim()));
            cx.notify();
            return;
        };
        match self.state.go_to_number(n) {
            Ok(request) => {
                self.execute(request, window, cx);
                self.cancel_go_to(window, cx);
            }
            Err(e) => {
                self.state.warning = Some(e);
                cx.notify();
            }
        }
    }

    /// Esc (ou depois de ir): o que se digitava cai e o foco volta a imagem.
    pub fn cancel_go_to(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // A caixa volta a mostrar a imagem atual no proximo quadro.
        self.go_to_dirty = false;
        self.focus_image(window, cx);
    }

    pub(super) fn status_bar(&self, view_window: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let text = layout::text_status(&self.status_info());
        let mut msgs = div()
            .flex()
            .flex_row()
            .gap_3()
            .items_center()
            .justify_end()
            .flex_none()
            .max_w(px(layout::width_max_messages(view_window)))
            .overflow_hidden();
        let messages = self.state.status_messages(self.opening.is_some(), self.quick_note_loading(), self.exclusive.busy());
        for m in messages {
            msgs = msgs.child(match m {
                StatusMessage::Progress(t) => div().flex_none().text_color(rgb(crate::theme::current().medium_text)).child(t).into_any_element(),
                StatusMessage::Confirmation(t) => message("msg-confirmacao", &t, crate::theme::current().ok).into_any_element(),
                StatusMessage::Warning(t) => message("msg-aviso", &t, crate::theme::current().warning).into_any_element(),
            });
        }
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .h(px(layout::STATUS_HEIGHT))
            .flex_none()
            .px_1()
            .bg(rgb(crate::theme::current().bar))
            .border_t_1()
            .border_color(rgb(crate::theme::current().border))
            .text_xs()
            // Os botoes chamam o mesmo `command` dos itens de menu, com a dica do menu.
            .child(icon_button("abrir-fonte", Icon::OpenFolder, menus::hint(menus::MenuAction::OpenSource), cx, |v, w, cx| {
                v.command(menus::MenuAction::OpenSource, w, cx)
            }))
            .child(icon_button("trocar-vault", Icon::Vault, menus::hint(menus::MenuAction::OpenVault), cx, |v, w, cx| {
                v.command(menus::MenuAction::OpenVault, w, cx)
            }))
            .child(
                div()
                    .id("status-texto")
                    .flex_1()
                    .min_w(px(0.0))
                    .truncate()
                    .px_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(crate::theme::current().hover_soft)))
                    .tooltip(hint_of(t!("status.hint_click", text = text, hint = menus::hint(menus::MenuAction::OpenVault))))
                    .child(text)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|v, _: &MouseDownEvent, w, cx| {
                            cx.stop_propagation();
                            // O mesmo caminho do menu e de Cmd-Shift-V (o foco vai a imagem).
                            v.command(menus::MenuAction::OpenVault, w, cx);
                        }),
                    ),
            )
            .child(msgs)
    }

    pub(super) fn footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Altura fixa: abrir uma gaveta nao muda o rodape (nem a altura da imagem).
        let mut textbox = div().flex().flex_row().gap_2().items_center().child(self.quick_note.clone());
        // Marcador persistente: nota que sumiu do vault ou guardada so na recuperacao.
        if let Some(m) = self.session.marker() {
            textbox = textbox.child(div().text_xs().text_color(rgb(crate::theme::current().error)).child(m));
            if self.session.can_recreate() {
                textbox = textbox
                    .child(btn(t!("btn.recreate_quick_note"), cx, |v, _, cx| v.recreate_quick_note(cx)))
                    .child(btn(t!("btn.discard"), cx, |v, _, cx| v.discard_quick_note_lost(cx)));
            }
        }
        div()
            .flex()
            .flex_col()
            .gap_1()
            .px_2()
            .py_1()
            .bg(rgb(crate::theme::current().lateral))
            .border_t_1()
            .border_color(rgb(crate::theme::current().border))
            .child(textbox)
    }
}

/// Uma das setinhas da caixa "Ir para" (`delta`: +1 sobe, -1 desce).
fn arrow_go_to(id: &'static str, i: Icon, delta: i32, cx: &mut Context<MainView>) -> impl IntoElement {
    div()
        .id(id)
        .w(px(14.0))
        .h(px(11.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_sm()
        .cursor_pointer()
        .hover(|s| s.bg(rgb(crate::theme::current().button_hover)))
        .child(icon(i, 10.0, rgb(crate::theme::current().medium_text)))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |v, _: &MouseDownEvent, w, cx| {
                cx.stop_propagation();
                v.step_go_to(delta, w, cx)
            }),
        )
}

/// Largura de cada botao da janela no Windows (o tamanho padrao do sistema).
const WIDTH_BUTTON_WINDOW: f32 = 46.0;

/// Minimizar, maximizar/restaurar e fechar, no estilo do Windows. Nao tem `on_click`: cada um
/// marca sua area (`WindowControlArea`) e o proprio Windows trata o clique (inclusive o menu
/// de encaixe do botao de maximizar, no Windows 11). Os icones sao glifos da fonte de icones do
/// sistema.
fn window_buttons(maximized: bool) -> impl IntoElement {
    let t = crate::theme::current();
    let btn = move |id: &'static str, glyph: &'static str, area: WindowControlArea| {
        let close = area == WindowControlArea::Close;
        div()
            .id(id)
            .flex_none()
            .w(px(WIDTH_BUTTON_WINDOW))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .font_family("Segoe MDL2 Assets")
            .text_size(px(10.0))
            .text_color(rgb(t.text))
            .window_control_area(area)
            .hover(move |s| if close { s.bg(rgb(COLOR_CLOSE_WINDOW)).text_color(rgb(0xffffff)) } else { s.bg(rgb(t.hover_soft)) })
            .child(glyph)
    };
    div()
        .flex_none()
        .h_full()
        .flex()
        .child(btn("janela-min", "\u{E921}", WindowControlArea::Min))
        .child(btn("janela-max", if maximized { "\u{E923}" } else { "\u{E922}" }, WindowControlArea::Max))
        .child(btn("janela-fechar", "\u{E8BB}", WindowControlArea::Close))
}

/// Vermelho do botao de fechar do Windows ao passar o mouse: cor do sistema, nao do tema.
const COLOR_CLOSE_WINDOW: u32 = 0xe81123;

/// `text`, com a letra de acesso (`mnemonic`: posicao em caracteres e a letra) sublinhada quando
/// `underline` (so no modo teclado, como no Windows).
fn text_with_mnemonic(text: &str, mnemonic: Option<(usize, char)>, underline: bool) -> gpui::AnyElement {
    match mnemonic.filter(|_| underline) {
        None => div().child(text.to_string()).into_any_element(),
        Some((i, _)) => {
            let cs: Vec<char> = text.chars().collect();
            let (before, font, after): (String, String, String) =
                (cs[..i].iter().collect(), cs[i..=i].iter().collect(), cs[i + 1..].iter().collect());
            div().flex().flex_row().child(before).child(div().underline().child(font)).child(after).into_any_element()
        }
    }
}
