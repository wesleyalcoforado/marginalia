//! Gavetas (notas, tarefas, fontes): a logica dos cliques e o desenho delas.

use super::*;
use crate::drawers::MAX_LINES;
use state::tree::{guides, DestinationRelocate, Guide, TreeLine};

impl MainView {
    /// Abre (ou fecha) a gaveta `g` no painel esquerdo (icone da barra de
    /// atividades ou Cmd-1/2/3). Abrir rele as listas e da o foco a gaveta (as
    /// teclas dela so valem com ela em foco); fechar a gaveta em foco devolve o
    /// foco a imagem.
    pub fn toggle_drawer(&mut self, g: Drawer, window: &mut Window, cx: &mut Context<Self>) {
        if self.drawers.toggle(g) {
            self.refresh(cx);
        }
        if self.drawers.open().is_some() {
            self.focus_drawer(window, cx);
        } else {
            self.drawer_closed(window, cx);
        }
        cx.notify();
    }

    /// Foco na gaveta aberta (abrir, clicar dentro dela; o smoke).
    /// Foco na busca da gaveta de notas (para o smoke; o clique na caixa faz o mesmo).
    pub fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.read(cx).focus_handle().focus(window);
    }

    /// Texto da caixa de busca das notas (para o smoke).
    pub fn text_search(&self, cx: &App) -> String {
        self.search.read(cx).text().to_string()
    }

    pub fn focus_drawer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.drawers.open().is_some() {
            self.drawer_focus.focus(window);
            cx.notify();
        }
    }

    /// A gaveta fechou: se o foco era dela, volta a imagem (nao fica em lugar nenhum).
    pub(super) fn drawer_closed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.drawer_focus.is_focused(window) {
            self.focus_image(window, cx);
        }
    }

    pub fn mark_task(&mut self, id: &str, done: bool, cx: &mut Context<Self>) -> Result<(), String> {
        self.allows_write(cx)?;
        self.free_of_quick_note(id, cx)?;
        let r =self.state.mark_task_done(id, done).map(|_| ());
        if r.is_ok() {
            self.drawers.mark_task(id, done);
            self.confirm_state(cx);
            self.refresh(cx);
        }
        cx.notify();
        r
    }

    /// Abre a confirmacao de excluir de vez `id` (nada e excluido ainda). Id que
    /// as gavetas nao conhecem vira aviso.
    pub fn request_deletion(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Err(e) = self.drawers.request_deletion(id) {
            self.state.warning = Some(e);
        }
        cx.notify();
    }

    /// Confirmado: exclui `id` de vez (ver `QuickNoteSession::delete`). Recusa, com o
    /// aviso na barra, durante a renomeacao de fonte, com gravacao do item em
    /// voo, ou se o vault recusa; a linha sai na hora e a recarga confirma.
    /// `kind` e o do pedido confirmado (o mesmo id pode existir em duas pastas).
    /// Uma recusa deixa a confirmacao aberta; so o sucesso (ou uma linha que
    /// sumiu) a fecha.
    pub fn delete(&mut self, id: &str, kind: marginalia_core::vault::DocKind, cx: &mut Context<Self>) -> Result<(), String> {
        self.allows_write(cx)?;
        if let Err(e) = self.drawers.check_deletion(id, kind) {
            self.state.warning = Some(e.clone());
            cx.notify();
            return Err(e);
        }
        self.pause_quick_note();
        let r = self.session.delete(&mut self.state, kind, id, Instant::now());
        let r = match r {
            Ok(e) => {
                self.drawers.remove(id, kind);
                self.confirm_state(cx);
                // `reload` vem no efeito: a recarga das gavetas roda ali.
                self.apply_quick_note(e, cx);
                Ok(())
            }
            Err(m) => Err(m),
        };
        self.sync(cx);
        cx.notify();
        r
    }

    /// Acao Editar: leva o texto da linha `id` para a caixa de nota (o item
    /// atual termina antes) e da o foco a caixa.
    pub fn edit(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.allows_write(cx).is_err() {
            return;
        }
        let Some(kind) = self.drawers.line_kind(id) else {
            self.state.warning = Some(t!("warning.not_encontrei_in_the_drawers", id = format!("{id:?}")));
            cx.notify();
            return;
        };
        self.textbox_load(kind, id, cx);
        self.focus_quick_note(window, cx);
    }

    /// Pede o texto de `id` para a caixa (`QuickNoteSession::load`): a nota que
    /// estava na caixa termina antes (gravada e fixada, R18: nada se perde).
    pub(super) fn textbox_load(&mut self, kind: marginalia_core::vault::DocKind, id: &str, cx: &mut Context<Self>) {
        if self.allows_write(cx).is_err() {
            return;
        }
        self.pause_quick_note();
        let e = self.session.load(&mut self.state, kind, id, Instant::now());
        self.apply_quick_note(e, cx);
    }

    /// Clique ou Enter numa linha de nota (R25c, R26): abre a imagem dela, leva
    /// o texto para a caixa e o foco a caixa (editar). A ordem
    /// importa: primeiro o quadro (a nota da caixa, de outra imagem, termina
    /// ali), depois `load` (que termina a nota da mesma imagem). Nota de
    /// outra fonte: o texto vem quando a fonte entrar (`enter`), senao a troca
    /// de imagem que chega depois terminaria a nota carregada e limparia a caixa.
    pub fn click_line_note(&mut self, l: &NoteLine, window: &mut Window, cx: &mut Context<Self>) -> Result<(), String> {
        let kind = self.drawers.line_kind(&l.id).unwrap_or(marginalia_core::vault::DocKind::Note);
        self.open_and_load(|e| e.open_line_note(l), kind, &l.id, window, cx)
    }

    /// Clique numa linha de tarefa: o mesmo que [`MainView::click_line_note`].
    pub fn click_line_task(&mut self, l: &LineTask, window: &mut Window, cx: &mut Context<Self>) -> Result<(), String> {
        self.open_and_load(|e| e.open_line_task(l), marginalia_core::vault::DocKind::Task, &l.id, window, cx)
    }

    /// A ordem (imagem, depois texto) e a de [`drawers::click_line`].
    pub(super) fn open_and_load(
        &mut self,
        open: impl FnOnce(&mut AppState) -> Result<OpenNote, crate::app_state::SourceError>,
        kind: marginalia_core::vault::DocKind,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let mut c = ClickInTheView { v: self, open: Some(open), kind, id, window, cx };
        drawers::click_line(&mut c)?;
        // Caixa limpa e desligada ate o texto chegar; sem carga nenhuma, o pedido cai.
        self.sync_screen(cx);
        Ok(())
    }

    /// Clique numa fonte: o mesmo fluxo de Cmd-O (Task 12) com o caminho que os
    /// dados do app guardam; zip pergunta o modo. Sem caminho valido, avisa.
    pub fn open_line_source(&mut self, l: &LineSource, window: &mut Window, cx: &mut Context<Self>) {
        let Some(file_path) = self.state.source_line_path(l) else {
            cx.notify();
            return;
        };
        let name = l.name.clone();
        if !open_source::e_zip(&file_path) {
            self.open_path(file_path, name, ZipMode::Direct, window, cx);
            return;
        }
        cx.spawn_in(window, async move |this, cx| {
            let Some(mode) = ask_mode_zip(cx).await else { return };
            let _ = this.update_in(cx, |v, window, cx| v.open_path(file_path, name, mode, window, cx));
        })
        .detach();
    }

    pub(super) fn drawer_action(&mut self, g: Drawer, a: DrawerAction, window: &mut Window, cx: &mut Context<Self>) {
        match a {
            DrawerAction::Relocate(d) => self.drawers.relocate_selection(d),
            DrawerAction::Close => {
                self.drawers.close();
                self.drawer_closed(window, cx);
            }
            // Enter numa linha de nota/tarefa = o clique (R26): abre a imagem, leva
            // o texto e o foco a caixa.
            DrawerAction::Open => match g {
                Drawer::Notes => {
                    if let Some(l) = self.drawers.note_selected().cloned() {
                        let _ = self.click_line_note(&l, window, cx);
                    }
                }
                Drawer::Tasks => {
                    if let Some(l) = self.drawers.selected_task().cloned() {
                        let _ = self.click_line_task(&l, window, cx);
                    }
                }
                Drawer::Sources => {
                    if let Some(l) = self.drawers.selected_source().cloned() {
                        self.open_line_source(&l, window, cx);
                    }
                }
            },
            DrawerAction::ToggleDone => {
                if let Some(l) = self.drawers.selected_task() {
                    let (id, done) = (l.id.clone(), l.done);
                    let _ = self.mark_task(&id, !done, cx);
                }
            }
            DrawerAction::Delete => {
                let id = match g {
                    Drawer::Notes => self.drawers.note_selected().map(|l| l.id.clone()),
                    Drawer::Tasks => self.drawers.selected_task().map(|l| l.id.clone()),
                    Drawer::Sources => None,
                };
                if let Some(id) = id {
                    self.request_deletion(&id, cx);
                }
            }
        }
        cx.notify();
    }

    /// A gaveta lateral aberta (notas, tarefas ou fontes).
    pub(super) fn drawer(&self, g: Drawer, width: f32, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let gv = &self.drawers;
        let mut head = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(div().font_weight(FontWeight::BOLD).child(g.title()))
            .child(div().text_xs().text_color(rgb(crate::theme::current().soft_text)).child(g.shortcut()));
        if g == Drawer::Notes && self.state.vault.is_some() {
            head = head.child(div().text_xs().text_color(rgb(crate::theme::current().medium_text)).child(gv.counter_notes()));
        }
        if gv.loading() {
            head = head.child(div().text_xs().text_color(rgb(crate::theme::current().medium_text)).child(t!("drawer.atualizando")));
        }
        head = head.child(div().flex_1()).child(icon_button("fechar-gaveta", Icon::Close, t!("drawer.close"), cx, move |v, w, cx| {
            v.toggle_drawer(g, w, cx)
        }));
        let mut note_body = div().flex().flex_col().gap_2().p_2().size_full().text_sm().child(head);
        if self.state.vault.is_none() {
            note_body = note_body.child(hint(t!("drawer.no_vault")));
        } else {
            note_body = match g {
                Drawer::Notes => self.drawer_notes(note_body, cx),
                Drawer::Tasks => self.tasks_drawer(note_body, cx),
                Drawer::Sources => self.sources_drawer(note_body, cx),
            };
            if let Some(c) = gv.deletion_confirming() {
                // Dialogo de excluir: o foco abre em Cancelar; Enter aciona o
                // botao em foco (so exclui com o foco em Excluir); Tab/setas trocam.
                let (id, kind) = (c.id.clone(), c.kind);
                let focus_state = gv.deletion_focus();
                note_body = note_body.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .p_2()
                        .rounded_md()
                        .bg(rgb(crate::theme::current().background_error))
                        .border_1()
                        .border_color(rgb(crate::theme::current().border_error))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_1()
                                .font_weight(FontWeight::BOLD)
                                .child(icon(Icon::Trash, 14.0, rgb(crate::theme::current().text_error)))
                                .child(c.title()),
                        )
                        .child(div().text_xs().font_weight(FontWeight::BOLD).child(c.stretch()))
                        .child(div().text_xs().child(c.warning()))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_2()
                                .child(div().flex_1())
                                .child(dialog_button(
                                    "excluir-cancelar",
                                    None,
                                    t!("btn.cancel"),
                                    focus_state == drawers::DeletionButton::Cancel,
                                    false,
                                    cx,
                                    |v, _, cx| {
                                        v.drawers.cancel_deletion();
                                        cx.notify();
                                    },
                                ))
                                .child(dialog_button(
                                    "excluir-confirmar",
                                    Some(Icon::Trash),
                                    t!("btn.delete"),
                                    focus_state == drawers::DeletionButton::Delete,
                                    true,
                                    cx,
                                    move |v, _, cx| {
                                        v.drawers.focus_button_deletion(drawers::DeletionButton::Delete);
                                        let _ = v.delete(&id, kind, cx);
                                    },
                                )),
                        )
                        .child(hint(t!("drawer.delete.hint_keys"))),
                );
            }
            if let Some(h) = gv.empty() {
                note_body = note_body.child(hint(h));
            }
            let n = gv.lists().unreadable.len();
            if n > 0 {
                note_body = note_body.child(
                    div()
                        .text_xs()
                        .text_color(rgb(crate::theme::current().warning))
                        .child(t!("drawer.unreadable", n)),
                );
            }
        }
        // Largura de `layout::widths` (cede numa janela estreita); o conteudo
        // fica com a largura cheia e e cortado. Foco proprio (`Focus::Drawer`):
        // qualquer clique dentro dela o pega antes (fase de captura); o clique numa
        // linha depois o leva a caixa (R26). Com foco, borda azul por cima.
        let focused = self.drawer_focus.is_focused(window);
        let mut g = div()
            .w(px(width))
            .flex_none()
            .h_full()
            .relative()
            .overflow_hidden()
            .bg(rgb(crate::theme::current().bar))
            .border_r_1()
            .border_color(rgb(crate::theme::current().border))
            .occlude()
            .track_focus(&self.drawer_focus)
            .capture_any_mouse_down(cx.listener(|v, _: &MouseDownEvent, w, cx| v.focus_drawer(w, cx)))
            .child(div().w(px(self.drawers.width())).h_full().child(note_body))
            // Alca para redimensionar: a borda direita do painel.
            .child(
                div()
                    .id("alca-gaveta")
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right_0()
                    .w(px(5.0))
                    .cursor_col_resize()
                    .hover(|s| s.bg(rgb(crate::theme::current().accent)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|v, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            v.start_drag_drawer(cx);
                        }),
                    ),
            );
        if focused {
            g = g.child(div().absolute().inset_0().border_1().border_color(rgb(crate::theme::current().accent)));
        }
        g
    }

    pub(super) fn drawer_notes(&self, note_body: gpui::Div, cx: &mut Context<Self>) -> gpui::Div {
        let gv = &self.drawers;
        let search = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .child(div().flex_1().min_w(px(0.0)).child(self.search.clone()))
            .when(!gv.notes_filters().text.is_empty(), |d| {
                d.child(icon_button("limpar-busca", Icon::Close, t!("drawer.search.clear"), cx, |v, _, cx| v.clear_search(cx)))
            });
        let marked = &gv.notes_filters().sources;
        let label = match marked.len() {
            0 => t!("drawer.filter_sources.all_menu"),
            1 => t!("drawer.filter_sources.uma", marked.iter().next().unwrap()),
            n => t!("drawer.filter_sources.several", n),
        };
        let mut sources = div().flex().flex_col().gap_1().child(
            div().flex().flex_row().child(chip(label, !marked.is_empty() || self.sources_menu, cx, |v, _, cx| {
                v.sources_menu = !v.sources_menu;
                cx.notify();
            })),
        );
        if self.sources_menu {
            let mut list = div().flex().flex_col().p_1().rounded_md().bg(rgb(crate::theme::current().raised)).border_1().border_color(rgb(crate::theme::current().strong_border));
            list = list.child(markable_option(t!("drawer.filter_sources.all"), marked.is_empty(), "fonte-todas".into(), cx, |v, _, cx| {
                v.drawers.clear_sources();
                cx.notify();
            }));
            for f in gv.note_sources() {
                let name = f.clone();
                let id = format!("fonte-{f}");
                list = list.child(markable_option(f.clone(), marked.contains(&f), id.into(), cx, move |v, _, cx| {
                    v.drawers.toggle_source(&name);
                    cx.notify();
                }));
            }
            sources = sources.child(list);
        }
        let colors = gv.tags_colors();
        let mut tags = div().flex().flex_row().flex_wrap().gap_1();
        for e in gv.vault_tags() {
            let active = gv.notes_filters().tags.contains(&e);
            let name = e.clone();
            tags = tags.child(chip_tag(&e, colors.get(&e).copied().unwrap_or(crate::theme::current().medium_text), active, cx, move |v, _, cx| {
                v.drawers.toggle_tag(&name);
                cx.notify();
            }));
        }
        let mut note_body = note_body.child(search).child(sources);
        if !gv.vault_tags().is_empty() {
            note_body = note_body.child(tags);
        }
        let visible = gv.notes_visible();
        let sel = gv.selection();
        let open = self.state.note_open();
        let mut list = div().id("lista-notas").flex().flex_col().gap_1().size_full().overflow_y_scroll().track_scroll(&self.scroll_notes);
        let mut drawn = 0;
        let lines = gv.tree_notes();
        let lines_guides = guides(&lines);
        for (n, tree_line) in lines.into_iter().enumerate() {
            let i = match tree_line {
                TreeLine::Item(k) | TreeLine::Drops(k) => k,
                cab => {
                    list = list.child(self.tree_header(&cab, &lines_guides[n], cx));
                    continue;
                }
            };
            if drawn >= MAX_LINES {
                break;
            }
            drawn += 1;
            let p = visible[i];
            let l = p.clone();
            let ex = p.id.clone();
            // So o texto da nota (sem titulo, arquivo ou pagina: isso vai para a barra de
            // status ao reabrir). Excluir e mover so aparecem com o mouse em cima.
            let group = SharedString::from(format!("nota-{i}"));
            let mut actions = hover_actions(group.clone());
            if let Some(d) = gv.relocate_destination(&p.image, p.record.as_deref()) {
                actions = actions.child(self.button_relocate(marginalia_core::vault::DocKind::Note, &p.id, d, cx));
            }
            actions = actions.child(icon_button("excluir-nota", Icon::Trash, t!("btn.delete"), cx, move |v, _, cx| v.request_deletion(&ex, cx)));
            let card = drawer_line(Some(i) == sel, open == Some(p.id.as_str()))
                .group(group)
                .relative()
                .flex_1()
                .min_w(px(0.0))
                .child(
                    div()
                        .text_xs()
                        .overflow_hidden()
                        .line_clamp(MAX_LINES_CARD)
                        .text_ellipsis()
                        .child(drawers::lines_first(&p.note_body, MAX_LINES_CARD)),
                )
                .when(!p.tags.is_empty(), |d| {
                    // Uma linha extra com as etiquetas da nota, nas cores do filtro.
                    d.child(div().flex().flex_row().flex_wrap().gap_x_2().text_xs().children(p.tags.iter().map(|e| {
                        div().text_color(rgb(colors.get(e).copied().unwrap_or(crate::theme::current().medium_text))).child(format!("#{e}"))
                    })))
                })
                .child(actions)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |v, _: &MouseDownEvent, w, cx| {
                        v.drawers.select(i);
                        // abre a imagem, leva o texto e o foco a caixa (R26)
                        let _ = v.click_line_note(&l, w, cx);
                    }),
                );
            list = list.child(with_guides(&lines_guides[n], CARD_MIDDLE, card));
        }
        if let Some(a) = gv.cut_warning() {
            list = list.child(cut_warning(a));
        }
        note_body.child(with_bar(list, &self.scroll_notes, cx))
    }

    pub(super) fn tasks_drawer(&self, note_body: gpui::Div, cx: &mut Context<Self>) -> gpui::Div {
        let gv = &self.drawers;
        let filters = div().flex().flex_row().gap_1().children(TasksFilter::ALL.map(|f| {
            let done_all = f == TasksFilter::Done;
            let n = gv.lists().tasks.iter().filter(|t| t.done == done_all).count();
            chip(format!("{} {n}", f.label()), gv.tasks_filter() == f, cx, move |v, _, cx| {
                v.drawers.set_filter_tasks(f);
                cx.notify();
            })
        }));
        let sel = gv.selection();
        let visible = gv.visible_tasks();
        let mut list = div().id("lista-tarefas").flex().flex_col().gap_1().size_full().overflow_y_scroll().track_scroll(&self.tasks_scroll);
        let mut drawn = 0;
        let lines = gv.tasks_tree();
        let lines_guides = guides(&lines);
        for (n, tree_line) in lines.into_iter().enumerate() {
            let i = match tree_line {
                TreeLine::Item(k) | TreeLine::Drops(k) => k,
                cab => {
                    list = list.child(self.tree_header(&cab, &lines_guides[n], cx));
                    continue;
                }
            };
            if drawn >= MAX_LINES {
                break;
            }
            drawn += 1;
            let t = visible[i];
            let l = t.clone();
            let (id, done) = (t.id.clone(), t.done);
            let textbox = button_check("check-tarefa", done, cx, move |v, _, cx| {
                let _ = v.mark_task(&id, !done, cx);
            });
            let ex = t.id.clone();
            let group = SharedString::from(format!("tarefa-{i}"));
            let mut actions = hover_actions(group.clone());
            if let Some(d) = t.image.as_ref().and_then(|m| gv.relocate_destination(m, t.record.as_deref())) {
                actions = actions.child(self.button_relocate(marginalia_core::vault::DocKind::Task, &t.id, d, cx));
            }
            actions = actions.child(icon_button("excluir-tarefa", Icon::Trash, t!("btn.delete"), cx, move |v, _, cx| v.request_deletion(&ex, cx)));
            let card = drawer_line(Some(i) == sel, false)
                    .group(group)
                    .relative()
                    .flex_1()
                    .min_w(px(0.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            // Sem esticar o botao na altura do texto: so o quadradinho realca.
                            .items_start()
                            .gap_2()
                            .child(textbox)
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .text_xs()
                                    .overflow_hidden()
                                    .line_clamp(MAX_LINES_CARD)
                                    .text_ellipsis()
                                    .child(drawers::lines_first(&t.note_body, MAX_LINES_CARD)),
                            ),
                    )
                    .when(t.note_deleted, |d| d.child(div().text_xs().text_color(rgb(crate::theme::current().warning)).child(t!("drawer.task.deleted_quick_note"))))
                    .child(actions)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |v, _: &MouseDownEvent, w, cx| {
                            v.drawers.select(i);
                            // abre a imagem, leva o texto e o foco a caixa (R26)
                            let _ = v.click_line_task(&l, w, cx);
                        }),
                    );
            list = list.child(with_guides(&lines_guides[n], CARD_MIDDLE, card));
        }
        if let Some(a) = gv.cut_warning() {
            list = list.child(cut_warning(a));
        }
        note_body.child(filters).child(with_bar(list, &self.tasks_scroll, cx))
    }

    pub(super) fn sources_drawer(&self, note_body: gpui::Div, cx: &mut Context<Self>) -> gpui::Div {
        let gv = &self.drawers;
        let sel = gv.selection();
        let mut list = div().id("lista-fontes").flex().flex_col().gap_1().flex_1().overflow_y_scroll();
        for (i, f) in gv.sources().iter().enumerate() {
            let l = f.clone();
            let color = match f.status {
                DrawerSourceStatus::Ok => crate::theme::current().ok,
                DrawerSourceStatus::NoPath => crate::theme::current().medium_text,
                DrawerSourceStatus::NotFound => crate::theme::current().error,
            };
            let (kind_icon, hint_kind) = match f.kind() {
                DrawerSourceKind::Image => (Icon::Image, t!("drawer.kind_source.image")),
                DrawerSourceKind::Folder => (Icon::Folder, t!("drawer.kind_source.folder")),
                DrawerSourceKind::Pdf => (Icon::Pdf, t!("drawer.kind_source.pdf")),
                DrawerSourceKind::Zip => (Icon::Zip, t!("drawer.kind_source.zip")),
            };
            let file_path = f.file_path.as_ref().map(|c| c.display().to_string()).unwrap_or_default();
            let editing = self.renaming.as_deref() == Some(f.name.as_str());
            let title = if editing {
                div().flex_1().min_w(px(0.0)).flex().flex_row().child(self.rename.clone())
            } else {
                div().flex_1().min_w(px(0.0)).truncate().text_xs().font_weight(FontWeight::BOLD).child(f.name.clone())
            };
            // Renomear e trocar a localizacao: so icones (clicar no card ja abre a fonte).
            let mut actions = div().flex().flex_row().flex_none().items_center();
            if !editing {
                let (n, n2, t) = (f.name.clone(), f.name.clone(), f.kind);
                actions = actions
                    .child(icon_button("renomear-fonte", Icon::Edit, t!("btn.rename"), cx, move |v, w, cx| v.start_rename(&n, w, cx)))
                    .child(icon_button("relocalizar-fonte", Icon::RotateRight, t!("btn.point_location"), cx, move |v, w, cx| {
                        v.point_location(&n2, Some(t), w, cx)
                    }));
            }
            // 1a linha: icone do tipo + bolinha de status, e os botoes a direita; 2a: nome; 3a: caminho.
            let mut line = drawer_line(Some(i) == sel, self.state.current.image.source == f.name)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(div().id(("tipo-fonte", i)).flex_none().tooltip(hint_of(hint_kind)).child(icon(kind_icon, 14.0, rgb(crate::theme::current().medium_text))))
                        .child(
                            div()
                                .id(("status-fonte", i))
                                .flex_none()
                                .size(px(8.0))
                                .rounded_full()
                                .bg(rgb(color))
                                .tooltip(hint_of(f.status.label())),
                        )
                        .child(div().flex_1())
                        .child(actions),
                )
                .child(title)
                .child(div().text_xs().text_color(rgb(crate::theme::current().soft_text)).truncate().child(file_path))
                .when(editing, |d| d.child(hint(t!("drawer.sources.hint_rename"))));
            if !editing {
                // Clicar na caixa de renomear nao abre a fonte.
                line = line.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |v, _: &MouseDownEvent, w, cx| {
                        v.drawers.select(i);
                        v.open_line_source(&l, w, cx);
                    }),
                );
            }
            list = list.child(line);
        }
        note_body.child(list)
    }

    /// O "x" da busca das notas: limpa o texto e o filtro.
    pub fn clear_search(&mut self, cx: &mut Context<Self>) {
        self.search.update(cx, |b, cx| b.load_text("", cx));
        self.drawers.set_search("");
        cx.notify();
    }
}

/// Os passos de [`drawers::click_line`] com o estado real da view.
pub(super) struct ClickInTheView<'a, 'b, F> {
    v: &'a mut MainView,
    open: Option<F>,
    kind: marginalia_core::vault::DocKind,
    id: &'a str,
    window: &'a mut Window,
    cx: &'a mut Context<'b, MainView>,
}

impl<F> drawers::LineClick for ClickInTheView<'_, '_, F>
where
    F: FnOnce(&mut AppState) -> Result<OpenNote, crate::app_state::SourceError>,
{
    fn open_image(&mut self) -> Result<bool, String> {
        let Some(open) = self.open.take() else {
            return Err(t!("error.image_already_open"));
        };
        // `open_note` (ou `open_point`, nota promovida / tarefa)
        let r = open(&mut self.v.state);
        let other_source = matches!(r, Ok(OpenNote::Source { .. }));
        self.v.state.drop_load_postponed(&t!("warning.other_click"));
        self.v.handle_opening(r, self.window, self.cx)?;
        // R26: o clique (ou Enter) numa linha leva o foco a caixa, mas so quando o
        // texto chegar: ate la a caixa fica desligada e engoliria o que se digitasse
        // (`focus_state::FocusToTheLoad`); o foco fica na gaveta, e se a carga falhar fica la.
        self.v.focus_on_load.request();
        Ok(other_source)
    }

    fn load(&mut self) {
        self.v.textbox_load(self.kind, self.id, self.cx);
    }

    fn load_on_enter(&mut self) {
        if self.v.opening.is_some() {
            self.v.state.postpone_load(self.kind, self.id);
        }
    }
}

impl MainView {
    /// Apertou a alca da borda do painel: o arrasto comeca.
    pub fn start_drag_drawer(&mut self, cx: &mut Context<Self>) {
        self.drawer_drag = true;
        cx.notify();
    }

    /// A borda do painel acompanha o ponteiro enquanto o botao esta apertado.
    pub fn drag_drawer(&mut self, x: f32, pressed: bool, cx: &mut Context<Self>) {
        if !self.drawer_drag {
            return;
        }
        if !pressed {
            self.drop_drawer(cx);
            return;
        }
        self.drawers.set_width(layout::width_to_point(x));
        cx.notify();
    }

    /// Soltou: guarda a largura nos dados do app (por maquina, fora do vault).
    pub fn drop_drawer(&mut self, cx: &mut Context<Self>) {
        if !std::mem::take(&mut self.drawer_drag) {
            return;
        }
        self.state.app_data.set_width_drawers(Some(self.drawers.width()));
        self.write_adjustments(cx);
        cx.notify();
    }

    /// Escuta o mouse na janela inteira durante o arrasto (o ponteiro sai da alca).
    pub(super) fn drawer_drag_thread(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().downgrade();
        canvas(
            |_, _, _| {},
            move |_, _, window, _| {
                let v1 = view.clone();
                window.on_mouse_event(move |ev: &MouseMoveEvent, phase, _, cx| {
                    if phase != DispatchPhase::Capture {
                        return;
                    }
                    let (x, pressed) = (f32::from(ev.position.x), ev.pressed_button == Some(MouseButton::Left));
                    let _ = v1.update(cx, |v, cx| v.drag_drawer(x, pressed, cx));
                });
                let v2 = view.clone();
                window.on_mouse_event(move |ev: &MouseUpEvent, phase, _, cx| {
                    if phase != DispatchPhase::Capture || ev.button != MouseButton::Left {
                        return;
                    }
                    let _ = v2.update(cx, |v, cx| v.drop_drawer(cx));
                });
            },
        )
        .absolute()
        .size_0()
    }

    /// Escuta o mouse na janela inteira durante o arrasto da barra de rolagem: o polegar
    /// segue o ponteiro mesmo fora da trilha, enquanto o botao esta apertado.
    pub(super) fn scroll_drag_thread(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().downgrade();
        canvas(
            |_, _, _| {},
            move |_, _, window, _| {
                let v1 = view.clone();
                window.on_mouse_event(move |ev: &MouseMoveEvent, phase, _, cx| {
                    if phase != DispatchPhase::Capture {
                        return;
                    }
                    let (y, pressed) = (ev.position.y, ev.pressed_button == Some(MouseButton::Left));
                    let _ = v1.update(cx, |v, cx| match v.scroll_drag.clone() {
                        Some(r) if pressed => {
                            scroll_to(&r, y);
                            cx.notify();
                        }
                        Some(_) => {
                            // Soltou fora da janela: o evento de soltar nao chegou.
                            v.scroll_drag = None;
                            cx.notify();
                        }
                        None => {}
                    });
                });
                let v2 = view.clone();
                window.on_mouse_event(move |ev: &MouseUpEvent, phase, _, cx| {
                    if phase != DispatchPhase::Capture || ev.button != MouseButton::Left {
                        return;
                    }
                    let _ = v2.update(cx, |v, cx| {
                        if v.scroll_drag.take().is_some() {
                            cx.notify();
                        }
                    });
                });
            },
        )
        .absolute()
        .size_0()
    }

    /// Cabecalho de fonte ou de registro da arvore (os itens sao desenhados por quem chama).
    pub(super) fn tree_header(&self, l: &TreeLine, g: &[Guide], cx: &mut Context<Self>) -> gpui::AnyElement {
        match l {
            TreeLine::Source { name, total, collapsed } => {
                let title = if name.is_empty() { t!("record.drawer.no_source") } else { name.clone() };
                let arrow = if *collapsed { "▸" } else { "▾" };
                let key = name.clone();
                let mut cab = div()
                    .id(SharedString::from(format!("fonte-{name}")))
                    .pt_1()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .text_sm()
                    .child(arrow)
                    .child(div().font_weight(FontWeight::BOLD).truncate().child(title));
                if *collapsed {
                    cab = cab.child(div().text_xs().text_color(rgb(crate::theme::current().soft_text)).child(t!("record.drawer.count", total)));
                }
                if !name.is_empty() {
                    cab = cab.cursor_pointer().on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |v, _: &MouseDownEvent, _, cx| {
                            v.drawers.toggle_tree_source(&key);
                            cx.notify();
                        }),
                    );
                }
                cab.into_any_element()
            }
            TreeLine::Record { record, valid, range, total, collapsed } => {
                let id = record.id.clone();
                let arrow = if *collapsed { "▸" } else { "▾" };
                let mut detail = range.clone();
                if *collapsed {
                    detail = t!("warning.join", detail, t!("record.drawer.count", total));
                }
                if !*valid {
                    detail = t!("warning.join", detail, t!("record.drawer.invalid"));
                }
                let cab = div()
                    .id(SharedString::from(format!("reg-{id}")))
                    .flex_1()
                    .min_w(px(0.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .text_xs()
                    .cursor_pointer()
                    .child(arrow)
                    .child(div().font_weight(FontWeight::BOLD).truncate().child(record.name.clone()))
                    .child(div().flex_1().min_w(px(0.0)).truncate().text_color(rgb(if *valid { crate::theme::current().soft_text } else { crate::theme::current().warning })).child(detail))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |v, _: &MouseDownEvent, _, cx| {
                            v.drawers.toggle_record(&id);
                            cx.notify();
                        }),
                    );
                with_guides(g, HEADER_MIDDLE, cab).into_any_element()
            }
            TreeLine::Item(_) | TreeLine::Drops(_) => div().into_any_element(),
        }
    }

    /// "Mover para <registro>" / "Voltar para <registro>" de um item numa imagem de fronteira.
    pub(super) fn button_relocate(&self, kind: marginalia_core::vault::DocKind, id: &str, d: DestinationRelocate, cx: &mut Context<Self>) -> impl IntoElement {
        let label = if d.back { t!("record.relocate.of_back", d.name) } else { t!("record.relocate.to", d.name) };
        let id = id.to_string();
        btn(label, cx, move |v, _, cx| v.relocate_record_item(kind, &id, d.destination.as_deref(), cx))
    }
}

/// Linhas que o card de nota mostra, no maximo (o resto some com `…`).
const MAX_LINES_CARD: usize = 4;
/// Onde o traco da guia encosta: meio da 1a linha do card (padding 4 + meia linha).
const CARD_MIDDLE: f32 = 12.0;
/// Idem para o cabecalho de registro (sem padding).
const HEADER_MIDDLE: f32 = 8.0;
/// Largura de cada coluna de guia (px).
const GUIDE_WIDTH: f32 = 12.0;

/// `content` precedido das guias pontilhadas da arvore (uma coluna por nivel de
/// recuo), sem nenhum caractere: so linhas tracejadas, como no explorador de
/// arquivos. `middle`: altura (px) do traco ate o centro da 1a linha do conteudo.
fn with_guides(g: &[Guide], middle: f32, content: impl IntoElement) -> gpui::Div {
    // A lista tem 4px entre linhas: a vertical avanca 2px para cada lado e emenda.
    const SLACK: f32 = 2.0;
    let mut line = div().flex().flex_row().w_full();
    for guide in g {
        let mut col = div().relative().flex_none().w(px(GUIDE_WIDTH));
        if matches!(guide, Guide::Continues | Guide::Branch | Guide::Last) {
            let vertical = div().absolute().left(px(5.0)).top(px(-SLACK)).w(px(1.0)).border_l_1().border_dashed().border_color(rgb(crate::theme::current().control_hover));
            col = col.child(match guide {
                Guide::Last => vertical.h(px(middle + SLACK)),
                _ => vertical.bottom(px(-SLACK)),
            });
        }
        if matches!(guide, Guide::Branch | Guide::Last) {
            col = col.child(div().absolute().left(px(5.0)).top(px(middle)).w(px(7.0)).h(px(1.0)).border_t_1().border_dashed().border_color(rgb(crate::theme::current().control_hover)));
        }
        line = line.child(col);
    }
    line.child(content)
}

/// Bloco de acoes de um card (excluir, mover): no canto superior direito, so aparece com o
/// mouse sobre o card do grupo `group`.
fn hover_actions(group: SharedString) -> gpui::Div {
    div()
        .absolute()
        .top_0()
        .right_0()
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .p_1()
        .rounded_sm()
        .bg(rgb(crate::theme::current().raised))
        .invisible()
        .group_hover(group, |s| s.visible())
}

/// Uma linha de gaveta: realcada se selecionada; borda viva se e a aberta.
pub(super) fn drawer_line(selected: bool, open: bool) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .p_1()
        .rounded_sm()
        .cursor_pointer()
        .bg(rgb(if selected { crate::theme::current().background_accent } else { crate::theme::current().lateral }))
        .hover(|s| s.bg(rgb(crate::theme::current().hover_soft)))
        .border_l_2()
        .border_color(rgb(if open { crate::theme::current().open_mark } else { crate::theme::current().lateral }))
}

pub(super) fn cut_warning(t: String) -> impl IntoElement {
    div().text_xs().text_color(rgb(crate::theme::current().warning)).child(t)
}

