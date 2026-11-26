//! Nota rapida, grifos, cor e abrir nota.

use super::*;

impl MainView {
    /// Ao abrir: o que o registro de recuperacao guarda (de uma saida em que o
    /// vault nao respondeu) volta para o vault; o que nao puder, fica no registro
    /// com o marcador no rodape.
    pub fn restore_quick_note(&mut self, cx: &mut Context<Self>) {
        self.pause_quick_note();
        let e = self.session.restore(&mut self.state, Instant::now());
        self.apply_quick_note(e, cx);
    }

    /// Botao "Recriar como nova nota" (nota que sumiu do vault): acao do usuario.
    pub fn recreate_quick_note(&mut self, cx: &mut Context<Self>) {
        if self.allows_write(cx).is_err() {
            return;
        }
        let e = self.session.recreate_as_new_note(&mut self.state, Instant::now());
        self.apply_quick_note(e, cx);
    }

    /// Gravacoes da nota esperam enquanto uma renomeacao de fonte reescreve o vault.
    pub(super) fn pause_quick_note(&mut self) {
        self.session.set_paused(self.exclusive.allows_write().is_err());
    }

    /// O texto da caixa mudou: a sessao cria o rascunho na 1a letra e grava com debounce.
    pub fn quick_note_changed(&mut self, text: &str, cx: &mut Context<Self>) {
        self.pause_quick_note();
        // So vale se o visualizador ja mostra a imagem atual; durante a carga fica o
        // alvo do quadro novo (ver `AppState::mirror_framing`).
        let fr = self.viewer.read(cx).framing().clone();
        self.state.mirror_framing(fr);
        let e = self.session.typed(&mut self.state, text, Instant::now());
        self.apply_quick_note(e, cx);
    }

    /// A caixa perdeu o foco: grava e fixa o id.
    pub(super) fn left_quick_note(&mut self, cx: &mut Context<Self>) {
        self.pause_quick_note();
        let e = self.session.textbox_exit(&mut self.state, Instant::now());
        self.apply_quick_note(e, cx);
    }

    /// Tick da sessao em `current_time` (o relogio da view; o smoke adianta o relogio).
    pub fn tick_quick_note_in(&mut self, current_time: Instant, cx: &mut Context<Self>) {
        self.pause_quick_note();
        let e = self.session.tick(&mut self.state, current_time);
        self.apply_quick_note(e, cx);
    }

    /// Sair do app / fechar a janela: grava o registro de recuperacao e depois a
    /// nota em edicao, esperando o vault no maximo `FLUSH_DEADLINE`. Devolve os avisos.
    pub fn flush_quick_note(&mut self) -> Vec<String> {
        self.flush_quick_note_with_deadline(crate::quick_note_session::FLUSH_DEADLINE)
    }

    /// [`MainView::flush_quick_note`] com outro prazo (smoke).
    pub fn flush_quick_note_with_deadline(&mut self, deadline: Duration) -> Vec<String> {
        self.pause_quick_note();
        let warnings = self.session.flush_with_deadline(&mut self.state, deadline);
        for a in &warnings {
            eprintln!("marginalia: {a}");
        }
        // O trabalho que rodava no executor volta cancelado: nao ha o que esperar.
        self.work_quick_note = None;
        self.quick_note_tick = None;
        warnings
    }

    /// Botao "Descartar" do marcador: abre mao do texto das notas perdidas (acao explicita).
    pub fn discard_quick_note_lost(&mut self, cx: &mut Context<Self>) {
        let e = self.session.discard_lost();
        self.apply_quick_note(e, cx);
    }

    /// Smoke: cada gravacao da nota espera `d` antes de tocar o vault (vault lento).
    pub fn delay_writes_quick_note(&mut self, d: Option<Duration>) {
        self.session.set_delay(d.map(|d| std::sync::Arc::new(move || std::thread::sleep(d)) as crate::quick_note_session::Delay));
    }

    /// Recusa mexer em `id` enquanto a nota rapida o grava (um arquivo, uma gravacao por vez).
    pub(super) fn free_of_quick_note(&mut self, id: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let r = self.session.free(id);
        if let Err(e) = &r {
            self.state.warning = Some(e.clone());
            cx.notify();
        }
        r
    }

    /// Executa o que a sessao pediu: ids nas gavetas, caixa, avisos, recarga, o
    /// trabalho no executor de fundo (o resultado volta por `conclude`) e o tick.
    pub(super) fn apply_quick_note(&mut self, e: Effect, cx: &mut Context<Self>) {
        let Effect { work, tick, warning, confirmation, clear_textbox, created: _, loaded, renamed, reload } = e;
        for (a, n) in &renamed {
            self.drawers.switch_id(a, n);
            self.state.load_postponed_follow_renaming(a, n);
        }
        if clear_textbox {
            self.quick_note.update(cx, |n, cx| n.clear(cx));
        }
        if let Some(t) = loaded {
            self.quick_note.update(cx, |n, cx| n.load_text(&t, cx));
            if self.focus_on_load.after(true, self.quick_note_loading()) {
                self.focus_textbox_now = true;
            }
        }
        if let Some(a) = warning {
            self.state.warning = Some(a);
        }
        if let Some(c) = confirmation {
            self.confirm(c, cx);
        }
        if reload {
            self.refresh(cx);
        }
        if let Some(t) = work {
            self.work_quick_note = Some(cx.spawn(async move |this, cx| {
                let f = cx.background_executor().spawn(async move { t.execute() }).await;
                let _ = this.update(cx, |v, cx| {
                    let e = v.session.conclude(&mut v.state, f, Instant::now());
                    v.apply_quick_note(e, cx);
                });
            }));
        }
        if let Some(d) = tick {
            self.quick_note_tick = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(d).await;
                let _ = this.update(cx, |v, cx| v.tick_quick_note_in(Instant::now(), cx));
            }));
        }
        self.sync_screen(cx);
        cx.notify();
    }

    /// O texto pedido por um clique/Enter numa linha ainda nao chegou (a sessao o
    /// le, ou espera a fonte dele abrir).
    pub fn quick_note_loading(&self) -> bool {
        self.session.loading() || self.state.has_load_postponed()
    }

    pub fn focus_image(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.viewer.read(cx).focus_handle().focus(window);
        cx.notify();
    }

    /// `/` (com a imagem em foco) e Cmd-K: foco na caixa de nota (se houver vault e imagem).
    pub fn focus_quick_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_command(Command::FocusQuickNote, window, cx);
    }

    /// Quem tem o foco do teclado agora (`focus_state::Focus`).
    pub fn current_focus(&self, window: &Window, cx: &App) -> Focus {
        if self.viewer.read(cx).focus_handle().is_focused(window) {
            Focus::Image
        } else if self.quick_note.read(cx).focus_handle().is_focused(window) {
            Focus::QuickNote
        } else if self.textboxes.iter().any(|c| c.read(cx).focus_handle().is_focused(window)) {
            Focus::Number
        } else if self.drawer_focus.is_focused(window) && self.drawers.open().is_some() {
            Focus::Drawer
        } else {
            Focus::Other
        }
    }

    /// Cmd-K, Cmd-Shift-K e Esc na nota (`focus_state::transition`). Caixa desligada:
    /// o foco fica e a barra diz por que (sem vault, o seletor abre).
    pub fn focus_command(&mut self, c: Command, window: &mut Window, cx: &mut Context<Self>) {
        let enabled = self.quick_note.read(cx).enabled();
        match focus_state::transition(self.current_focus(window, cx), c, enabled) {
            Transition::To(Focus::QuickNote) => self.quick_note.read(cx).focus_handle().focus(window),
            Transition::To(Focus::Image) => self.viewer.read(cx).focus_handle().focus(window),
            Transition::To(Focus::Drawer) => self.focus_drawer(window, cx),
            Transition::To(Focus::Other | Focus::Number) | Transition::Stays => {}
            Transition::Refused => {
                if self.state.vault.is_none() {
                    self.state.warning = Some(QuickNoteError::NoVault.to_string());
                    self.open_picker(cx);
                } else {
                    self.state.warning = Some(t!("warning.abra_source_to_quick_note"));
                }
            }
        }
        cx.notify();
    }

    /// Grifo desenhado no visualizador: grava e guarda para a proxima nota.
    pub fn add_highlight(&mut self, image: &ImageRef, g: Highlight, cx: &mut Context<Self>) {
        if self.allows_write(cx).is_err() {
            return;
        }
        match self.state.commit_highlight_in(image, g) {
            Ok(()) => self.confirm(t!("confirmation.stored_highlight"), cx),
            Err(e) => self.state.warning = Some(e),
        }
        self.sync(cx);
        cx.notify();
    }

    /// Clique simples na imagem: seleciona o grifo `id` (ou, no vazio, tira a
    /// selecao). Um clique feito sobre outra imagem (o quadro ja mudou) nao conta.
    pub fn select_highlight(&mut self, image: &ImageRef, id: Option<String>, cx: &mut Context<Self>) {
        if self.state.source_has() && &self.state.current.image == image {
            self.state.select(id);
            self.sync_screen(cx);
            cx.notify();
        }
    }

    /// Delete/Backspace com a imagem em foco: apaga o grifo selecionado (Cmd+Z desfaz).
    pub fn delete_highlight_selected(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.state.annot.selected.clone() else { return };
        if self.allows_write(cx).is_err() {
            return;
        }
        match self.state.delete_highlight(&id) {
            Ok(()) => self.confirm(t!("confirmation.erased_highlight"), cx),
            Err(e) => self.state.warning = Some(e),
        }
        self.sync_screen(cx);
        cx.notify();
    }

    /// Cmd+Z com a imagem em foco: devolve o ultimo grifo apagado (mesmo id e cor), selecionado.
    pub fn undo_highlight(&mut self, cx: &mut Context<Self>) {
        if self.allows_write(cx).is_err() {
            return;
        }
        match self.state.undo_highlight() {
            Ok(true) => self.confirm(t!("confirmation.returned_highlight"), cx),
            Ok(false) => {}
            Err(e) => self.state.warning = Some(e),
        }
        self.sync_screen(cx);
        cx.notify();
    }

    /// O que o clique na amostra `i` do seletor faz (o listener so para a
    /// propagacao e chama isto; o smoke chama o mesmo). A amostra nao tem
    /// `track_focus`, entao o clique nao tira o foco da imagem: Delete e Cmd+Z
    /// continuam valendo logo depois.
    pub fn click_sample(&mut self, i: usize, cx: &mut Context<Self>) {
        if let Some(color) = PALETTE.get(i) {
            self.set_color(color, cx);
            self.open_colors = false;
        }
    }

    /// Amostra da paleta (menu de cores da barra de ferramentas): cor dos novos grifos e, com um
    /// grifo selecionado, a dele. Falha: aviso na barra, nada muda.
    pub fn set_color(&mut self, color: &str, cx: &mut Context<Self>) {
        if self.state.annot.selected.is_some() && self.allows_write(cx).is_err() {
            return;
        }
        match self.state.set_color(color) {
            Ok(()) => {}
            Err(e) => self.state.warning = Some(t!("warning.switch_color", e)),
        }
        self.sync_screen(cx);
        cx.notify();
    }

    /// Cmd+Enter na caixa: grava, fixa o id e devolve o foco a imagem (pronta
    /// para a proxima nota). A caixa limpa (com "nota salva: <id>" na barra)
    /// quando o disco tiver o texto; se gravar falhar, o texto fica na caixa, o
    /// erro vai para a barra e a sessao tenta de novo.
    pub fn finish_quick_note(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.allows_write(cx).is_err() {
            // Renomeacao de fonte em andamento: o texto fica e e gravado depois.
            return;
        }
        self.quick_note_changed(text, cx);
        let e = self.session.finish(&mut self.state, Instant::now());
        let finished = e.clear_textbox;
        self.apply_quick_note(e, cx);
        // Sem item (sem vault/imagem, texto vazio) o foco fica na caixa, com o aviso.
        if finished || (self.session.has_item() && self.session.text_to_restore().is_none()) {
            self.focus_image(window, cx);
        }
        cx.notify();
    }

    /// Abre a nota `id` na imagem e no enquadramento dela (abrindo a fonte, se
    /// for outra). Erro (nota ou fonte nao encontrada) vai para a barra.
    pub fn open_note(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) -> Result<(), String> {
        let r = self.state.open_note(id);
        self.handle_opening(r, window, cx)
    }

    /// Executa uma abertura de nota/ponto: na fonte aberta pede o quadro; em
    /// outra fonte abre a fonte no fundo e vai ao quadro. Erro vai para a barra.
    pub(super) fn handle_opening(
        &mut self,
        r: Result<OpenNote, crate::app_state::SourceError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        match r {
            Ok(OpenNote::ImageFrame(p)) => {
                self.execute(p, window, cx);
                Ok(())
            }
            Ok(OpenNote::Source { opening, image_frame, highlight }) => {
                if let Err(e) = self.exclusive.allows_open() {
                    self.state.warning = Some(e.clone());
                    cx.notify();
                    return Err(e);
                }
                self.viewer.update(cx, |v, cx| v.set_loading(true, cx));
                self.opening = Some(cx.spawn_in(window, async move |this, cx| {
                    let a = opening.clone();
                    // Confere o caminho, percorre/copia a fonte e le os grifos dela no fundo.
                    let r = cx
                        .background_executor()
                        .spawn(async move { a.execute_ready().map(|ready| open_source::Open { ready, record: None }) })
                        .await;
                    let _ = this.update_in(cx, |v, window, cx| {
                        v.opening = None;
                        v.enter(&opening, r, Some(image_frame), highlight, window, cx);
                    });
                }));
                cx.notify();
                Ok(())
            }
            Err(e) => {
                let e = e.to_string();
                self.state.warning = Some(e.clone());
                cx.notify();
                Err(e)
            }
        }
    }

    /// Menuzinho de cor dos grifos (botao da barra de ferramentas): as 6 cores
    /// de `PALETTE`, a atual com contorno. Clicar define a cor dos novos grifos
    /// e, com um grifo selecionado, recolore ele (`AppState::set_color`).
    pub(super) fn colors_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.state.current_color();
        let title = if self.state.annot.selected.is_some() { t!("adjustment.color.selected_title") } else { t!("adjustment.color.new_title") };
        let mut samples = div().flex().flex_row().gap_2();
        for (i, color) in PALETTE.iter().copied().enumerate() {
            let chosen = color == current;
            samples = samples.child(
                div()
                    .id(("cor-grifo", i))
                    .tooltip(hint_of(t!("adjustment.color.hint", color_name(i), color)))
                    .size(px(28.0))
                    .flex_none()
                    .rounded_sm()
                    .cursor_pointer()
                    .border_2()
                    .border_color(rgb(if chosen { crate::theme::current().strong_text } else { crate::theme::current().bar }))
                    .p(px(2.0))
                    .child(div().size_full().rounded_sm().bg(rgb(annotate::color_rgb(color))))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |v, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            v.click_sample(i, cx)
                        }),
                    ),
            );
        }
        div()
            .id("menu-cores")
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .rounded_md()
            .bg(rgb(crate::theme::current().bar))
            .border_1()
            .border_color(rgb(crate::theme::current().strong_border))
            .shadow_md()
            .text_sm()
            .occlude()
            .child(div().font_weight(FontWeight::BOLD).child(title))
            .child(samples)
            .child(div().text_xs().text_color(rgb(crate::theme::current().soft_text)).child(t!("adjustment.color.help")))
    }

    /// Fundo transparente sobre a area da imagem enquanto o menu de cores esta
    /// aberto: clicar fora fecha (e o clique nao vira um grifo novo).
    pub(super) fn colors_background(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div().id("cores-fundo").absolute().top_0().left_0().size_full().occlude().on_mouse_down(
            MouseButton::Left,
            cx.listener(|v, _: &MouseDownEvent, _, cx| {
                v.open_colors = false;
                cx.notify();
            }),
        )
    }
}

/// Nome da cor `i` de `PALETTE` (mesma ordem; dicas do seletor).
pub(super) fn color_name(i: usize) -> String {
    match i {
        0 => t!("adjustment.color.yellow"),
        1 => t!("adjustment.color.vermelho"),
        2 => t!("adjustment.color.green"),
        3 => t!("adjustment.color.blue"),
        4 => t!("adjustment.color.rosa"),
        _ => t!("adjustment.color.white"),
    }
}
