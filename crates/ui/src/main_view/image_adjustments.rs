//! Imagem e ajustes: filtro, barras do painel, girar, e o desenho do painel.

use super::*;

impl MainView {
    /// Icone de ajustes da barra da direita (ou Cmd-F): abre/fecha o painel encaixado.
    pub fn toggle_adjustments(&mut self, cx: &mut Context<Self>) {
        let mut l = self.layout();
        l.toggle_adjustments();
        self.open_adjustments = l.open_adjustments;
        cx.notify();
    }

    /// A regiao visivel do quadro atual passou a ser `j` (o visualizador informa
    /// em pan, zoom, roda, redimensionar e imagem nova: `ViewerEvent::ViewWindow`;
    /// a agenda cuida da rajada). O estado a guarda ja (um tile
    /// de outra janela que chegar cai); a agenda interrompe o tile em voo da
    /// janela velha e pede o da nova depois de a janela parar.
    pub fn window_changed(&mut self, j: ViewWindow, window: &mut Window, cx: &mut Context<Self>) {
        let r = self.state.schedule_window_changed(&mut self.schedule, j, Instant::now());
        self.schedule_after(r, window, cx);
    }

    /// Um passo inteiro (tecla, menu, botao): mudou e soltou de uma vez, a
    /// tile, a base (logo depois do tile) e a gravacao saem pela agenda sem esperar os prazos.
    pub fn set_adjustments(&mut self, a: Adjustments, window: &mut Window, cx: &mut Context<Self>) {
        self.adjustment_changed(a, window, cx);
        self.adjustment_dropped(window, cx);
    }

    /// O usuario escolheu `a` (arrastando a barra): vale ja na memoria (painel,
    /// menus); o tile sai pela agenda, a base e a gravacao depois de parar.
    pub fn adjustment_changed(&mut self, a: Adjustments, window: &mut Window, cx: &mut Context<Self>) {
        let r = self.state.schedule_adjustment_changed(&mut self.schedule, a, Instant::now());
        self.schedule_after(r, window, cx);
        cx.notify();
    }

    /// O usuario soltou a barra: a base (depois do tile) e a gravacao saem ja.
    pub fn adjustment_dropped(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.schedule.dropped(Instant::now());
        self.tick_adjustments_in(Instant::now(), window, cx);
    }

    /// A agenda segue o quadro atual (`AppState::schedule_follow_image`): um
    /// arrasto de barra nao atravessa a troca de imagem e a gravacao pendente sai.
    pub(super) fn schedule_follow_image(&mut self, cx: &mut Context<Self>) {
        let s = self.state.schedule_follow_image(&mut self.schedule);
        self.follow_after(s, cx);
    }

    /// A agenda depois de uma resposta (`AppState::schedule_after_receive`).
    pub(super) fn schedule_on_receive(&mut self, r: &Received, work: Option<u64>, cx: &mut Context<Self>) {
        let s = self.state.schedule_after_receive(&mut self.schedule, r, work, Instant::now());
        self.follow_after(s, cx);
    }

    /// O que a view faz com o resultado de uma chamada do estado que mexeu na agenda.
    fn schedule_after(&mut self, r: crate::app_state::ScheduleAfter, window: &mut Window, cx: &mut Context<Self>) {
        self.follow_after(r.followed, cx);
        if r.tick {
            self.tick_adjustments_in(Instant::now(), window, cx);
        }
    }

    fn follow_after(&mut self, s: crate::app_state::Followed, cx: &mut Context<Self>) {
        if s.switched {
            self.bar_drag = None;
        }
        if s.write {
            self.write_adjustments(cx);
        }
    }

    /// Tick da agenda em `current_time`: executa o que ela pede e agenda o proximo.
    pub(super) fn tick_adjustments_in(&mut self, current_time: Instant, window: &mut Window, cx: &mut Context<Self>) {
        self.schedule_follow_image(cx);
        // Um pedido que nem sai (sem quadro, imagem com erro) libera a agenda na hora.
        loop {
            let mut released = false;
            for step in self.state.schedule_tick(&mut self.schedule, current_time) {
                match step {
                    ScheduleStep::Filter { work, image_adjustments, quality } => {
                        released |= !self.filter(work, image_adjustments, quality, window, cx)
                    }
                    ScheduleStep::Write => self.write_adjustments(cx),
                }
            }
            if !released {
                break;
            }
        }
        self.schedule_tick = self.schedule.next_tick(Instant::now()).map(|d| {
            cx.spawn_in(window, async move |this, cx| {
                cx.background_executor().timer(d).await;
                let _ = this.update_in(cx, |v, window, cx| v.tick_adjustments_in(Instant::now(), window, cx));
            })
        });
        cx.notify();
    }

    /// Roda o filtro `work` da agenda no fundo; o fim (qualquer resultado)
    /// volta a agenda por `finished`. `false`: nada a filtrar (ja liberado).
    pub(super) fn filter(
        &mut self,
        work: u64,
        image_adjustments: Adjustments,
        q: Quality,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(request) = self.state.request_schedule(&mut self.schedule, work, image_adjustments, q) else {
            return false;
        };
        if request.page_loads() {
            // So ler a pagina deixa o visualizador carregando (ele recusa grifos
            // enquanto isso); refiltrar no pan/zoom nao pisca o carregando.
            self.viewer.update(cx, |v, cx| v.set_loading(true, cx));
        }
        self.filter = Some(cx.spawn_in(window, async move |this, cx| {
            let resp = cx.background_executor().spawn(async move { load::execute(request) }).await;
            let _ = this.update_in(cx, |v, window, cx| {
                v.receive(resp, Some(work), window, cx);
                v.schedule.finished(work);
                v.tick_adjustments_in(Instant::now(), window, cx);
            });
        }));
        true
    }

    /// Grava os dados do app no fundo (instantaneo numerado, gravacao
    /// atomica); a interface nunca espera o disco. A memoria ja tem cada valor
    /// na chave da imagem em que foi escolhido (`apply_adjustments`): nada e
    /// reescrito aqui (depois de uma renomeacao, a chave da acao e a antiga).
    pub(super) fn write_adjustments(&mut self, cx: &mut Context<Self>) {
        let g = self.state.app_data_write();
        self.writes += 1;
        cx.spawn(async move |this, cx| {
            let r = cx.background_executor().spawn(async move { g.execute().map_err(|e| e.to_string()) }).await;
            let _ = this.update(cx, |v, cx| {
                v.writes -= 1;
                v.state.wrote_adjustments(r);
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn relocate(&mut self, c: Control, steps: i32, window: &mut Window, cx: &mut Context<Self>) {
        let a = step(&self.state.adjustments, c, steps);
        self.set_adjustments(a, window, cx);
    }

    pub(super) fn toggle(&mut self, f: impl FnOnce(&mut Adjustments), window: &mut Window, cx: &mut Context<Self>) {
        let mut a = self.state.adjustments.clone();
        f(&mut a);
        self.set_adjustments(a, window, cx);
    }

    /// A barra `b` em `v` (na escala da tela): o que cada movimento do arrasto
    /// faz (o smoke chama o mesmo). O tile sai pela agenda; a base e a
    /// gravacao, ao soltar ([`MainView::adjustment_dropped`]) ou depois de parar.
    pub fn set_bar(&mut self, b: Bar, v: i32, window: &mut Window, cx: &mut Context<Self>) {
        let a = b.apply(&self.state.adjustments, v);
        self.adjustment_changed(a, window, cx);
    }

    /// Duplo clique na trilha ou no rotulo: a barra volta ao neutro (um passo inteiro).
    pub fn restore_bar(&mut self, b: Bar, window: &mut Window, cx: &mut Context<Self>) {
        let a = b.restore(&self.state.adjustments);
        self.set_adjustments(a, window, cx);
        self.write_textbox(b, cx);
    }

    /// ↑/↓ (Shift: 10) na caixa numerica de `b`.
    pub(super) fn bar_step(&mut self, b: Bar, delta: i32, window: &mut Window, cx: &mut Context<Self>) {
        let a = b.step(&self.state.adjustments, delta);
        self.set_adjustments(a, window, cx);
        self.write_textbox(b, cx);
    }

    /// Enter na caixa numerica de `b` (o smoke chama o mesmo): o numero vale
    /// (saneado a faixa); texto invalido nao muda nada (aviso na barra). A
    /// caixa volta a mostrar o valor atual e fica com o foco. `true`: valeu.
    pub fn confirm_number(&mut self, b: Bar, text: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let thanks = match slider::interpret_text(text, b) {
            Some(v) => {
                let a = b.apply(&self.state.adjustments, v);
                self.set_adjustments(a, window, cx);
                true
            }
            None => {
                if !text.trim().is_empty() {
                    self.state.warning = Some(t!("warning.adjustment.not_and_number", label = b.label(), text = format!("{:?}", text.trim())));
                }
                false
            }
        };
        self.write_textbox(b, cx);
        cx.notify();
        thanks
    }

    /// Esc na caixa numerica: devolve o valor atual e o foco vai a imagem.
    pub(super) fn cancel_number(&mut self, b: Bar, window: &mut Window, cx: &mut Context<Self>) {
        self.write_textbox(b, cx);
        self.focus_command(Command::Esc, window, cx);
    }

    /// Onde a trilha de `b` foi pintada no ultimo quadro `(esquerda, largura)` (smoke).
    pub fn screen_track(&self, b: Bar) -> Option<(f32, f32)> {
        self.tracks.borrow()[b.index()]
    }

    /// A caixa numerica de `b` (para o smoke digitar nela).
    pub fn numeric_textbox(&self, b: Bar) -> &Entity<QuickNoteBox> {
        &self.textboxes[b.index()]
    }

    /// A caixa de `b` mostra o valor atual (todo selecionado: digitar substitui)
    /// e deixa de estar suja.
    pub(super) fn write_textbox(&mut self, b: Bar, cx: &mut Context<Self>) {
        self.dirty_all.clear(b);
        let t = b.text(&self.state.adjustments);
        self.textboxes[b.index()].update(cx, |n, cx| n.set_text(&t, cx));
    }

    /// Texto e ligado/desligado (sem fonte nada a ajustar) das caixas
    /// numericas: toda caixa que nao esta suja (`slider::DirtyTextboxes`: texto
    /// digitado ainda sem Enter/Esc) mostra o valor atual, focada ou nao.
    pub(super) fn sync_textboxes(&mut self, cx: &mut Context<Self>) {
        let has = self.state.source_has();
        // "Ir para": caixa vazia (o total aparece na dica ao passar o mouse).
        // So cabem tantos algarismos quanto o total de imagens (ou paginas): 99 -> 2, 500 -> 3.
        let max = self.state.unit_go_to().map_or(MAX_DIGITS_GO_TO, |(_, _, n)| state::quick_note::digits(n));
        // A caixa mostra onde estamos e acompanha a navegacao, salvo enquanto se digita nela.
        let where_ = self.state.unit_go_to().map(|(_, a, _)| a.to_string()).unwrap_or_default();
        let dirty = self.go_to_dirty;
        self.ir_to.update(cx, |n, cx| {
            n.configure(has, "", cx);
            n.limit_digits(max, cx);
            if !dirty && n.text() != where_ {
                // Cursor no fim, sem selecionar nada (nao e uma selecao do usuario).
                n.load_text(&where_, cx);
            }
        });
        let dirty_all = self.dirty_all;
        for b in slider::ALL {
            let t = b.text(&self.state.adjustments);
            let hint = if b == Bar::Threshold { t!("adjustment.threshold.hint_textbox") } else { String::new() };
            self.textboxes[b.index()].update(cx, |n, cx| {
                n.configure(has, hint, cx);
                if dirty_all.rewrite(b, n.text(), &t) {
                    n.set_text(&t, cx);
                }
            });
        }
    }

    /// Clique na trilha de `b` em `x` (janela): pula para o ponto e comeca o
    /// arrasto; duplo clique na trilha volta ao neutro (o 1o clique ja levou o
    /// marcador ao ponteiro, entao e o duplo clique no marcador). O listener da trilha
    /// e o smoke chamam isto (e [`MainView::drag_bar`], [`MainView::drop_bar`]).
    pub fn press_track(&mut self, b: Bar, x: f32, clicks: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.current_focus(window, cx) == Focus::Number {
            // O que se digitava numa caixa nao fica pendurado: o foco vai a imagem.
            self.focus_image(window, cx);
        }
        let Some((left, width)) = self.tracks.borrow()[b.index()] else { return };
        if clicks >= 2 {
            self.drop_bar(window, cx);
            self.restore_bar(b, window, cx);
            return;
        }
        let Some(v) = b.point_value(x, left, width) else { return };
        self.bar_drag = Some(b);
        self.set_bar(b, v, window, cx);
    }

    /// O ponteiro andou durante o arrasto de uma barra (de qualquer lugar da janela).
    pub fn drag_bar(&mut self, x: f32, pressed: bool, window: &mut Window, cx: &mut Context<Self>) {
        // A imagem mudou desde o aperto? O arrasto acabou (`schedule_follow_image`).
        self.schedule_follow_image(cx);
        let Some(b) = self.bar_drag else { return };
        if !pressed {
            // O botao foi solto sem o evento chegar (fora da janela): solta agora.
            self.drop_bar(window, cx);
            return;
        }
        let Some((left, width)) = self.tracks.borrow()[b.index()] else { return };
        if let Some(v) = b.point_value(x, left, width) {
            self.set_bar(b, v, window, cx);
        }
    }

    /// Soltou o mouse: a base (depois do tile) e a gravacao saem ja.
    pub fn drop_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.bar_drag.take().is_some() {
            self.adjustment_dropped(window, cx);
        }
    }

    /// Gira a imagem do quadro atual em 90 graus (`clockwise`: para a direita). A
    /// rotacao vale ja na memoria e nos dados do app (gravados no fundo, por imagem);
    /// o visualizador a adota na hora (o ponto no centro fica no centro e os grifos
    /// giram junto) e as texturas, que eram da orientacao velha, saem: com ajuste na
    /// tela a agenda refiltra (a janela visivel nova primeiro, como no pan), sem ajuste
    /// pede-se a textura cheia girada. Grifos, notas e enquadramentos guardados
    /// continuam nas coordenadas da imagem, entao nada no vault muda.
    pub fn rotate(&mut self, clockwise: bool, window: &mut Window, cx: &mut Context<Self>) {
        let (followed, fresh) = self.state.rotate_frame(&mut self.schedule, clockwise);
        self.follow_after(followed, cx);
        let Some(fresh) = fresh else { return };
        // A janela visivel nova ja vale: o estado e a agenda a conhecem antes do pedido, e o
        // visualizador nao a avisa de novo.
        let view_window = self.viewer.update(cx, |v, cx| v.set_rotation(fresh, window, cx));
        match self.state.schedule_after_rotate(&mut self.schedule, view_window, Instant::now()) {
            RotationAfter::Refilter { tick } => {
                if tick {
                    self.tick_adjustments_in(Instant::now(), window, cx);
                }
            }
            RotationAfter::Full(request) => {
                // Sem filtro na tela a agenda nao tem o que pedir: a textura cheia girada sai
                // direto, na tarefa da carga do quadro (nao na `filter`, que e a da agenda).
                if request.page_loads() {
                    self.viewer.update(cx, |v, cx| v.set_loading(true, cx));
                }
                self.task = Some(cx.spawn_in(window, async move |this, cx| {
                    let resp = cx.background_executor().spawn(async move { load::execute(request) }).await;
                    let _ = this.update_in(cx, |v, window, cx| v.receive(resp, None, window, cx));
                }));
            }
            RotationAfter::Nothing => {}
        }
        self.write_adjustments(cx);
        cx.notify();
    }

    /// O olho do cabecalho "Ajustes": liga/desliga a previa dos ajustes (a
    /// imagem sem eles, para comparar). Nada e apagado nem gravado; o quadro
    /// atual e refiltrado pela agenda (mesmas regras: um filtro em voo, a
    /// geracao nova derruba o que estava em voo). Vale para a sessao.
    pub fn toggle_preview_adjustments(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let r = self.state.schedule_toggle_preview(&mut self.schedule, Instant::now());
        self.schedule_after(r, window, cx);
        cx.notify();
    }

    /// Painel de ajustes encaixado a direita (`width` de `layout::widths`;
    /// o conteudo tem a largura cheia e e cortado se a janela for estreita).
    ///
    /// No modelo do painel Basic do Camera Raw: cabecalho "Ajustes" recolhivel
    /// com o olho (antes/depois), uma barra por ajuste (rotulo a esquerda e a
    /// caixa numerica editavel a direita, a trilha fina com o marcador
    /// triangular embaixo; ver `slider`), Inverter e Tons de cinza como
    /// interruptores, "Restaurar ajustes" e a paleta dos grifos.
    pub(super) fn panel(&self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let a = &self.state.adjustments;
        let mut content = div()
            .w(px(layout::ADJUSTMENTS_PANEL))
            .flex_none()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .text_sm()
            .child(self.adjustments_header(cx));
        if !self.collapsed_adjustments {
            let group = || div().flex().flex_col().gap_2();
            content = content
                .child(group().child(self.bar_line(Bar::Brightness, cx)).child(self.bar_line(Bar::Contrast, cx)))
                .child(group().pt_2().child(self.bar_line(Bar::Sharpness, cx)).child(self.bar_line(Bar::Highpass, cx)))
                .child(group().pt_2().child(self.bar_line(Bar::Threshold, cx)))
                .child(
                    div()
                        .pt_2()
                        .flex()
                        .flex_row()
                        .gap_4()
                        .child(switch("interruptor-inverter", t!("adjustment.invert"), menus::MenuAction::Invert, a.invert, cx, |a| {
                            a.invert = !a.invert
                        }))
                        .child(switch("interruptor-cinza", t!("adjustment.gray_tones"), menus::MenuAction::Gray, a.grayscale, cx, |a| {
                            a.grayscale = !a.grayscale
                        })),
                )
                .child(
                    div().flex().flex_row().justify_end().child(
                        div()
                            .id("restaurar-ajustes")
                            .tooltip(hint_of(menus::hint(menus::MenuAction::Restore)))
                            .child(btn(t!("btn.restore_adjustments"), cx, |v, w, cx| v.command(menus::MenuAction::Restore, w, cx))),
                    ),
                );
        }
        div()
            .w(px(width))
            .flex_none()
            .h_full()
            .overflow_hidden()
            .bg(rgb(crate::theme::current().bar))
            .border_l_1()
            .border_color(rgb(crate::theme::current().border))
            .occlude()
            .child(content)
    }

    /// "▾ Ajustes" (clique recolhe/mostra as barras) e o olho, que liga/desliga
    /// a previa dos ajustes (a imagem sem eles; nada e apagado).
    pub(super) fn adjustments_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (arrow, hint_arrow) = if self.collapsed_adjustments { ("▸", t!("adjustment.show_barras")) } else { ("▾", t!("adjustment.recolher_barras")) };
        let (eye, hint_eye) = if self.state.show_adjustments() {
            (Icon::Eye, t!("adjustment.linked_preview"))
        } else {
            (Icon::ClosedEye, t!("adjustment.disabled_preview"))
        };
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(
                div()
                    .id("ajustes-recolher")
                    .flex_1()
                    .flex()
                    .flex_row()
                    .gap_1()
                    .cursor_pointer()
                    .font_weight(FontWeight::BOLD)
                    .tooltip(hint_of(t!("adjustment.hint_header", hint_arrow)))
                    .child(arrow)
                    .child(t!("adjustment.title"))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|v, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            v.collapsed_adjustments = !v.collapsed_adjustments;
                            cx.notify();
                        }),
                    ),
            )
            .child(icon_button("ajustes-olho", eye, hint_eye, cx, |v, w, cx| v.toggle_preview_adjustments(w, cx)))
    }

    /// Uma barra: rotulo a esquerda (duplo clique: neutro) e a caixa numerica a
    /// direita na mesma linha; a trilha fina embaixo, na largura toda. O Limiar
    /// tem a caixa de liga/desliga antes do rotulo.
    pub(super) fn bar_line(&self, b: Bar, cx: &mut Context<Self>) -> impl IntoElement {
        let a = &self.state.adjustments;
        let active = b.active(a);
        let i = b.index();
        let shortcut = match b {
            Bar::Brightness => format!(" {}", t!("adjustment.shortcut.brightness")),
            Bar::Contrast => format!(" {}", t!("adjustment.shortcut.contrast")),
            Bar::Threshold => format!(" {}", t!("adjustment.shortcut.threshold")),
            Bar::Sharpness | Bar::Highpass => String::new(),
        };
        let (min, max) = b.strip();
        let label = div()
            .id(("barra-rotulo", i))
            .flex_1()
            .min_w(px(0.0))
            .truncate()
            .cursor_pointer()
            .text_color(rgb(if active { crate::theme::current().text } else { crate::theme::current().soft_text }))
            .tooltip(hint_of(t!("adjustment.hint_bar", b.label(), shortcut, min, max)))
            .child(b.label())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |v, ev: &MouseDownEvent, w, cx| {
                    if ev.click_count >= 2 {
                        cx.stop_propagation();
                        v.restore_bar(b, w, cx);
                    }
                }),
            );
        let textbox = div()
            .w(px(64.0))
            .flex_none()
            // Antes do editor (fase de captura): ↑/↓ mudam o valor, nao o cursor.
            .capture_key_down(cx.listener(move |v, ev: &KeyDownEvent, w, cx| {
                let m = &ev.keystroke.modifiers;
                if m.platform || m.control || m.alt {
                    return;
                }
                if let Some(d) = slider::key_step(&ev.keystroke.key, m.shift) {
                    cx.stop_propagation();
                    v.bar_step(b, d, w, cx);
                }
            }))
            .child(self.textboxes[i].clone());
        let mut top = div().flex().flex_row().items_center().gap_2();
        if b == Bar::Threshold {
            top = top.child(
                div()
                    .id("limiar-liga")
                    .flex_none()
                    .cursor_pointer()
                    .tooltip(hint_of(menus::hint(menus::MenuAction::Threshold)))
                    .child(checkbox(active))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|v, _: &MouseDownEvent, w, cx| {
                            cx.stop_propagation();
                            v.command(menus::MenuAction::Threshold, w, cx)
                        }),
                    ),
            );
        }
        top = top.child(label).child(textbox);
        div().flex().flex_col().gap_1().child(top).child(self.track(b, b.in_screen(a), active, cx))
    }

    /// A trilha fina de `b` com o marcador triangular em `v` (apagado com a
    /// barra desligada). Apertar pula para o ponto e comeca o arrasto; a
    /// posicao pintada fica em `tracks` para o ponteiro -> valor.
    pub(super) fn track(&self, b: Bar, v: i32, active: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let i = b.index();
        let fraction = b.fraction(v);
        let bipolar = b.bipolar();
        let tracks = self.tracks.clone();
        let color: Hsla = rgb(if active { crate::theme::current().strong_text } else { crate::theme::current().control }).into();
        let drawing = canvas(
            move |bounds, _, _| {
                tracks.borrow_mut()[i] = Some((f32::from(bounds.left()), f32::from(bounds.size.width)));
            },
            move |bounds, _, window, _| paint_track(window, bounds, fraction, bipolar, color),
        )
        .size_full();
        div()
            .id(("barra-trilha", i))
            .w_full()
            .h(px(TRACK_HEIGHT))
            .cursor_pointer()
            .child(drawing)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |v, ev: &MouseDownEvent, w, cx| {
                    cx.stop_propagation();
                    v.press_track(b, f32::from(ev.position.x), ev.click_count, w, cx);
                }),
            )
    }

    /// Durante o arrasto de uma barra: o movimento e a soltura do mouse em
    /// qualquer lugar da janela (o ponteiro sai da trilha fina o tempo todo).
    pub(super) fn drag_thread(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().downgrade();
        canvas(
            |_, _, _| {},
            move |_, _, window, _| {
                let v1 = view.clone();
                window.on_mouse_event(move |ev: &MouseMoveEvent, phase, window, cx| {
                    if phase != DispatchPhase::Capture {
                        return;
                    }
                    let (x, pressed) = (f32::from(ev.position.x), ev.pressed_button == Some(MouseButton::Left));
                    let _ = v1.update(cx, |v, cx| v.drag_bar(x, pressed, window, cx));
                });
                let v2 = view.clone();
                window.on_mouse_event(move |ev: &MouseUpEvent, phase, window, cx| {
                    if phase != DispatchPhase::Capture || ev.button != MouseButton::Left {
                        return;
                    }
                    let _ = v2.update(cx, |v, cx| v.drop_bar(window, cx));
                });
                // A soltura se perdeu (Cmd-Tab no meio do arrasto): o proximo aperto
                // fecha o arrasto antes de tudo; se for na trilha, o listener dela
                // (fase de bolha, depois deste) comeca um arrasto novo.
                let v3 = view.clone();
                window.on_mouse_event(move |_: &MouseDownEvent, phase, window, cx| {
                    if phase != DispatchPhase::Capture {
                        return;
                    }
                    let _ = v3.update(cx, |v, cx| v.drop_bar(window, cx));
                });
            },
        )
        .absolute()
        .size_0()
    }
}

/// Altura da area da trilha (a linha fina, o marcador embaixo e a folga de clique).
pub(super) const TRACK_HEIGHT: f32 = 14.0;

/// A trilha: linha fina cinza na largura toda, o tique do centro nos
/// bipolares e o marcador (triangulo pequeno apontando para a linha).
pub(super) fn paint_track(window: &mut Window, b: Bounds<Pixels>, fraction: f32, bipolar: bool, color: Hsla) {
    let (x0, y0, w) = (f32::from(b.left()), f32::from(b.top()), f32::from(b.size.width));
    let line_y = y0 + 3.0;
    window.paint_quad(fill(Bounds::new(point(px(x0), px(line_y)), size(px(w), px(2.0))), rgb(crate::theme::current().track)));
    if bipolar {
        window.paint_quad(fill(Bounds::new(point(px(x0 + w / 2.0 - 0.5), px(y0)), size(px(1.0), px(8.0))), rgb(crate::theme::current().control_hover)));
    }
    let x = x0 + fraction.clamp(0.0, 1.0) * w;
    let top = line_y + 3.0;
    let mut p = PathBuilder::fill();
    p.add_polygon(&[point(px(x), px(top)), point(px(x - 5.0), px(top + 7.0)), point(px(x + 5.0), px(top + 7.0))], true);
    if let Ok(path) = p.build() {
        window.paint_path(path, color);
    }
}

/// Caixa de marcar (14 px): azul com ✓ ligada, vazia desligada.
pub(super) fn checkbox(is_on: bool) -> gpui::Div {
    div()
        .flex_none()
        .size(px(14.0))
        .rounded_sm()
        .border_1()
        .border_color(rgb(if is_on { crate::theme::current().accent } else { crate::theme::current().control }))
        .bg(rgb(if is_on { crate::theme::current().accent } else { crate::theme::current().field }))
        .flex()
        .items_center()
        .justify_center()
        .text_xs()
        .text_color(rgb(crate::theme::current().accent_about))
        .child(if is_on { "✓" } else { "" })
}

/// Interruptor (Inverter, Tons de cinza): a caixa de marcar e o rotulo; o
/// clique faz o mesmo que a tecla e o item de menu `a` (a dica mostra o atalho).
pub(super) fn switch(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    a: menus::MenuAction,
    enabled: bool,
    cx: &mut Context<MainView>,
    f: fn(&mut Adjustments),
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .cursor_pointer()
        .tooltip(hint_of(menus::hint(a)))
        .child(checkbox(enabled))
        .child(label.into())
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |v, _: &MouseDownEvent, w, cx| {
                cx.stop_propagation();
                v.toggle(f, w, cx)
            }),
        )
}
