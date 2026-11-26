//! Abrir e fechar fonte, navegar, soltar arquivos, relink e o carregamento em segundo plano.

use super::*;

/// Quantas fontes recentes o convite lista.
const MAX_RECENT_INVITATION: usize = 5;

impl MainView {
    /// Sem fonte aberta (partida sem caminho ou fonte fechada): um quadro no meio da area da
    /// imagem convida a abrir uma (como a tela de boas-vindas do VS Code).
    pub(super) fn invitation_open_source(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let recent: Vec<LineSource> = match &self.state.vault {
            Some(v) => self.drawers.recent_sources(self.state.app_data.recent(&v.root)).into_iter().take(MAX_RECENT_INVITATION).cloned().collect(),
            None => Vec::new(),
        };
        let mut list = div().flex().flex_col().gap_1().w_full();
        for (i, f) in recent.iter().enumerate() {
            let ok = f.status == drawers::DrawerSourceStatus::Ok;
            let l = f.clone();
            let file_path = f.file_path.as_ref().map(|c| c.display().to_string()).unwrap_or_else(|| f.status.label());
            list = list.child(
                div()
                    .id(("recente", i))
                    .flex()
                    .flex_col()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(crate::theme::current().hover_soft)))
                    .child(div().font_weight(FontWeight::BOLD).text_color(rgb(if ok { crate::theme::current().text } else { crate::theme::current().weak_text })).truncate().child(f.name.clone()))
                    .child(div().text_xs().text_color(rgb(if ok { crate::theme::current().soft_text } else { crate::theme::current().warning })).truncate().child(file_path))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |v, _: &MouseDownEvent, w, cx| v.open_line_source(&l, w, cx)),
                    ),
            );
        }
        div().absolute().top_0().left_0().size_full().flex().items_center().justify_center().child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_3()
                .px_6()
                .pt_5()
                .pb(px(44.0))
                .w(px(420.0))
                .max_w_full()
                .rounded_lg()
                .bg(rgb(crate::theme::current().card))
                .border_1()
                .border_color(rgb(crate::theme::current().border))
                .child(div().font_weight(FontWeight::BOLD).child(t!("empty.title")))
                .child(div().text_xs().text_color(rgb(crate::theme::current().soft_text)).text_center().child(t!("empty.text")))
                .child(
                    div()
                        .id("convite-abrir-fonte")
                        .tooltip(hint_of(menus::hint(menus::MenuAction::OpenSource)))
                        .child(button_with_icon(Icon::OpenFolder, t!("empty.btn"), cx, |v, w, cx| {
                            v.command(menus::MenuAction::OpenSource, w, cx)
                        })),
                )
                .when(!recent.is_empty(), |d| {
                    d.child(div().w_full().pt_2().text_xs().text_color(rgb(crate::theme::current().soft_text)).child(t!("empty.recent"))).child(list)
                }),
        )
    }

    /// Ao sair do app: guarda a ultima posicao aberta (dados do app).
    /// Sair / fechar a janela: os ajustes ainda por gravar vao ao disco agora
    /// (sem esperar o fundo) e a posicao, se mudou.
    pub fn save_position(&mut self) {
        // A memoria ja tem o valor (`apply_adjustments`): a gravacao pendente da
        // agenda so cai, e uma gravacao do fundo em voo (ou que falhou) e
        // refeita aqui; a trava compartilhada espera a que estiver rodando.
        let in_schedule = self.schedule.unload().is_some();
        if in_schedule || self.state.pending_app_data() {
            if let Err(e) = self.state.save_app_data() {
                eprintln!("marginalia: nao foi possivel salvar os ajustes: {e}");
            }
        }
        self.state.save_position();
    }

    pub fn go_next(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let p = self.state.go_next();
        let moved = self.state.moved(&p);
        self.execute(p, window, cx);
        moved
    }

    pub fn go_prev(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let p = self.state.go_prev();
        let moved = self.state.moved(&p);
        self.execute(p, window, cx);
        moved
    }

    pub fn open_frame(&mut self, f: Frame, window: &mut Window, cx: &mut Context<Self>) {
        let p = self.state.open_frame(f);
        self.execute(p, window, cx);
    }

    /// Roda `request` (carga de quadro) no executor de fundo e entrega a resposta ao estado.
    pub(super) fn execute(&mut self, request: Option<Request>, window: &mut Window, cx: &mut Context<Self>) {
        if request.is_some() {
            // Outro quadro (ou o mesmo de novo): a nota da caixa termina se a imagem
            // mudou (R18: grava, fixa o id, a caixa limpa com "nota salva: <id>").
            self.pause_quick_note();
            let e = self.session.image_changed(&mut self.state, Instant::now());
            self.apply_quick_note(e, cx);
            // O que se digitava numa caixa numerica nao vale para o quadro novo:
            // todas voltam a mostrar os valores dele (`sync_textboxes`).
            self.dirty_all.clear_all();
        }
        if let Some(request) = request {
            self.viewer.update(cx, |v, cx| v.set_loading(true, cx));
            self.task = Some(cx.spawn_in(window, async move |this, cx| {
                let resp = cx.background_executor().spawn(async move { load::execute(request) }).await;
                let _ = this.update_in(cx, |v, window, cx| v.receive(resp, None, window, cx));
            }));
        }
        self.sync(cx);
        cx.notify();
    }

    /// Entrega `r` ao estado. `work`: o filtro da agenda que a trouxe (`None`:
    /// a carga do quadro, que a agenda nao pediu).
    pub(super) fn receive(&mut self, r: Response, work: Option<u64>, window: &mut Window, cx: &mut Context<Self>) {
        let received = self.state.receive(r);
        self.schedule_on_receive(&received, work, cx);
        match received {
            Received::Image(ex, fr, layer) => {
                let image = self.state.current.image.clone();
                // O visualizador espelha as camadas do estado (`keep_other_layer`).
                let keep = self.state.keep_other_layer(layer);
                self.viewer.update(cx, |v, cx| v.set_image(ex, fr, image, layer, keep, window, cx))
            }
            Received::Error(m) | Received::Unavailable(m) => self.viewer.update(cx, |v, cx| v.set_error(m, window, cx)),
            // So a base de fundo mudou: o tile e o enquadramento ficam.
            Received::Layer(ex) => self.viewer.update(cx, |v, cx| v.set_background(ex, window, cx)),
            // O tile caiu porque a janela mudou: a agenda pede o da janela vigente
            // (mesmos ajustes) assim que o lugar do filtro em voo vagar. So o tile
            // da propria agenda (`work`) conta como derrubado nela.
            Received::WindowChanged => {
                self.tick_adjustments_in(Instant::now(), window, cx);
            }
            Received::Ignored => {}
        }
        // Seta apertada enquanto o quadro carregava: anda agora que o total de paginas e conhecido.
        let pending = self.state.resume_navigation();
        self.execute(pending, window, cx);
    }

    /// Abre `file_path` como fonte chamada `name` (ja decidido). Tudo o que toca
    /// o vault ou a fonte roda no fundo (`open_source::open_in_background`: recusa um
    /// nome que e de outra fonte, percorre/copia, le os grifos, cria
    /// `sources/<name>.md`); aqui so entra o resultado e o caminho vai para os
    /// dados do app (pasta local) e para os caminhos desta sessao.
    pub fn open_path(
        &mut self,
        file_path: PathBuf,
        name: String,
        mode: ZipMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.allows_open(cx) {
            return;
        }
        // Outra abertura substitui a do clique numa linha: o texto dela nao vem mais.
        self.state.drop_load_postponed(&t!("warning.other_source_before"));
        self.sync_screen(cx);
        let opening = self.state.path_opening(&name, file_path, mode);
        let app_data = self.state.app_data.clone();
        self.viewer.update(cx, |v, cx| v.set_loading(true, cx));
        self.opening = Some(cx.spawn_in(window, async move |this, cx| {
            let a = opening.clone();
            let r = cx.background_executor().spawn(async move { open_source::open_in_background(&a, &app_data) }).await;
            let _ = this.update_in(cx, |v, window, cx| {
                v.opening = None;
                v.enter(&opening, r, None, None, window, cx);
            });
        }));
        cx.notify();
    }

    /// Uma abertura voltou do fundo: [`AppState::enter_source`] (que descarta o
    /// resultado se o vault mudou nesse meio tempo) e depois a carga do quadro.
    pub(super) fn enter(
        &mut self,
        a: &Opening,
        r: Result<open_source::Open, OpeningFailure>,
        start: Option<Frame>,
        highlight: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.state.enter_source(a, r, start, highlight) {
            Input::Open { request, fresh } => {
                if fresh {
                    self.refresh(cx);
                }
                if request.is_none() {
                    let msg = self.state.error.clone().unwrap_or_default();
                    self.viewer.update(cx, |vw, cx| vw.set_error(msg, window, cx));
                }
                self.execute(request, window, cx);
                // Clique numa linha de outra fonte: o quadro novo ja e o atual (a
                // nota antiga terminou no `execute`); agora o texto vai para a caixa.
                if let Some((kind, id)) = self.state.resume_load_postponed() {
                    self.textbox_load(kind, &id, cx);
                }
            }
            Input::Stale | Input::Failed(_) => {
                self.state.drop_load_postponed(&t!("warning.source_not_abriu"));
                self.sync_screen(cx);
                self.viewer.update(cx, |vw, cx| vw.set_loading(false, cx));
            }
        }
        cx.notify();
    }

    /// Cmd-O: escolhe pasta ou zip, pergunta o modo (zip) e o nome (se o
    /// padrao ja for de outra fonte do vault), e abre.
    pub fn dialog_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let choice = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: true,
            multiple: false,
            prompt: Some(t!("dialog.source.open").into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(file_paths))) = choice.await else { return };
            let Some(file_path) = file_paths.into_iter().next() else { return };
            let _ = this.update_in(cx, |v, window, cx| v.open_chosen(file_path, window, cx));
        })
        .detach();
    }

    /// Arrastar e soltar na janela: [`relink::release_drop`] (vault aberto,
    /// sem renomeacao) e [`open_source::sort_drop`] (um item so: pasta,
    /// zip, imagem ou PDF); aceito, segue o mesmo caminho do Cmd-O depois do
    /// painel ([`MainView::open_chosen`]). Recusado: o aviso da barra diz por
    /// que, e a mensagem volta (`None`: aceito).
    pub fn drop_item(&mut self, paths: &[PathBuf], window: &mut Window, cx: &mut Context<Self>) -> Option<String> {
        let menu_action = match relink::release_drop(self.state.vault.is_some(), &self.exclusive) {
            Err(e) => open_source::DropAction::Refuse(e),
            Ok(()) => open_source::sort_drop(paths),
        };
        cx.notify();
        match menu_action {
            open_source::DropAction::Open(file_path) => {
                self.open_chosen(file_path, window, cx);
                None
            }
            open_source::DropAction::Refuse(m) => {
                self.state.warning = Some(m.clone());
                Some(m)
            }
        }
    }

    /// O resto do Cmd-O depois de escolher `file_path` (o painel ou um drop):
    /// pergunta o modo (zip) e o nome (se o padrao ja for de outra fonte do
    /// vault), e abre ([`MainView::open_path`]).
    pub(super) fn open_chosen(&mut self, file_path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            let mut mode = ZipMode::Direct;
            if open_source::e_zip(&file_path) {
                let Some(m) = ask_mode_zip(cx).await else { return };
                mode = m;
            }

            let base = open_source::default_name(&file_path);
            let Ok((vault, app_data)) = this.update(cx, |v, _| (v.state.vault.clone(), v.state.app_data.clone())) else {
                return;
            };
            // `sources/` e lida no fundo (o vault pode estar no NAS).
            let (b, c) = (base.clone(), file_path.clone());
            let name = cx
                .background_executor()
                .spawn(async move {
                    match vault.map(|v| (v.list_sources(), v)) {
                        Some((Ok(fs), v)) => {
                            open_source::free_name(&b, |n| open_source::conflicts_at(&fs, &app_data, &v.root, n, &c))
                        }
                        _ => b,
                    }
                })
                .await;
            if name != base {
                let question = t!("dialog.source.ja_exists", base);
                let btn = t!("dialog.source.open_as", name);
                let Ok(r) = cx.update(|window, cx| {
                    window.prompt(PromptLevel::Warning, &question, None, &[btn.as_str(), t!("btn.cancel").as_str()], cx)
                }) else {
                    return;
                };
                if r.await != Ok(0) {
                    return;
                }
            }
            let _ = this.update_in(cx, |v, window, cx| v.open_path(file_path, name, mode, window, cx));
        })
        .detach();
    }

    /// "Arquivo > Fechar fonte": a nota em edicao termina (gravada e fixada), a
    /// posicao fica guardada, a imagem sai da tela e a caixa desliga. Uma
    /// abertura ou carga em andamento e cancelada. Recusa durante uma
    /// renomeacao/nova localizacao/troca de vault ("aguarde").
    pub fn close_source(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.allows_open(cx) {
            return;
        }
        self.opening = None;
        self.task = None;
        self.state.drop_load_postponed(&t!("warning.closed_source"));
        let textbox_in_focus = self.quick_note.read(cx).focus_handle().is_focused(window);
        if let Some(msg) = self.state.close_source_requested() {
            self.viewer.update(cx, |vw, cx| {
                vw.set_loading(false, cx);
                vw.clear(window, cx);
            });
            self.confirm(msg, cx);
        }
        self.sync(cx);
        if textbox_in_focus {
            // A caixa desliga sem fonte: o foco nao fica num campo morto.
            self.focus_image(window, cx);
        }
        cx.notify();
    }

    /// O aviso "Fonte indisponivel" / "Fonte nao encontrada" com as acoes.
    pub(super) fn source_warning(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.state.pending().expect("so com pendencia");
        let (name, kind) = (p.name.clone(), self.drawers.sources().iter().find(|f| f.name == p.name).map(|f| f.kind));
        let color = if p.reason == Reason::Unavailable { crate::theme::current().background_error } else { crate::theme::current().background_warning };
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .bg(rgb(color))
            .border_b_1()
            .border_color(rgb(crate::theme::current().border_warning))
            .text_sm()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(0.0))
                    .child(div().font_weight(FontWeight::BOLD).child(p.title()))
                    .child(div().text_xs().truncate().child(p.explanation())),
            )
            .child(btn(t!("btn.new_attempt"), cx, |v, w, cx| v.new_attempt(w, cx)))
            .child(btn(t!("btn.point_location"), cx, move |v, w, cx| v.point_location(&name, kind, w, cx)))
            .child(btn("x", cx, |v, _, cx| {
                v.state.dismiss_pending();
                cx.notify();
            }))
    }

    /// "Tentar de novo": reabre o quadro que falhou ou resolve a fonte de novo.
    pub fn new_attempt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let r = self.state.new_attempt();
        let _ = self.handle_opening(r, window, cx);
    }

    /// "Apontar nova localizacao...": painel do sistema (pasta para fonte-pasta,
    /// arquivo para zip) e depois [`MainView::relink`].
    pub fn point_location(
        &mut self,
        name: &str,
        kind: Option<marginalia_core::model::SourceKind>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use marginalia_core::model::SourceKind;
        let choice = cx.prompt_for_paths(PathPromptOptions {
            files: kind != Some(SourceKind::Folder),
            directories: kind != Some(SourceKind::Zip),
            multiple: false,
            prompt: Some(t!("dialog.source.new_location", name).into()),
        });
        let name = name.to_string();
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(file_paths))) = choice.await else { return };
            let Some(c) = file_paths.into_iter().next() else { return };
            let _ = this.update_in(cx, |v, window, cx| v.relink(&name, c, window, cx));
        })
        .detach();
    }

    /// Aponta `name` para `file_path`: confere no fundo que a pasta/zip tem as
    /// imagens da fonte (as citadas nas gavetas e a que vai reabrir); se tiver,
    /// grava o mapa nos dados do app e reabre o mesmo quadro no mesmo
    /// enquadramento. Se nao, o aviso explica e nada muda.
    pub fn relink(&mut self, name: &str, file_path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let l = self.drawers.lists();
        let kind = l.sources.iter().find(|f| f.name == name).map(|f| f.kind);
        let files = l
            .notes
            .iter()
            .map(|p| &p.image)
            .chain(l.tasks.iter().filter_map(|t| t.image.as_ref()))
            .filter(|i| i.source == name)
            .map(|i| i.file.as_str());
        let expected = self.state.expected_relink(name, kind, relink::cited(files));
        if !self.start_exclusive(t!("status.conferindo_location"), cx) {
            return;
        }
        let r = self.state.prepare_relink(name, file_path, expected);
        self.viewer.update(cx, |v, cx| v.set_loading(true, cx));
        self.work = Some(cx.spawn_in(window, async move |this, cx| {
            let r2 = r.clone();
            let res = cx.background_executor().spawn(async move { r2.execute() }).await;
            let _ = this.update_in(cx, |v, window, cx| {
                v.work = None;
                v.exclusive.finish();
                v.viewer.update(cx, |vw, cx| vw.set_loading(false, cx));
                match res {
                    Ok(val) => match val.confirm {
                        None => v.conclude_relink(&r, val.ready, window, cx),
                        Some(n) => v.ask_relink(r, val.ready, n, window, cx),
                    },
                    Err(e) => v.state.relink_failed(e),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn conclude_relink(&mut self, r: &crate::app_state::Relink, ready: crate::app_state::Ready, window: &mut Window, cx: &mut Context<Self>) {
        let p = self.state.conclude_relink(r, ready);
        self.confirm_state(cx);
        self.execute(p, window, cx);
        self.refresh(cx);
    }

    /// Nada no vault cita imagens da fonte: a nova localizacao nao pode ser
    /// conferida, entao pergunta (com o caminho e quantas imagens tem) antes de usar.
    pub(super) fn ask_relink(
        &mut self,
        r: crate::app_state::Relink,
        ready: crate::app_state::Ready,
        images: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let question = relink::confirmation_question(&r.name, &r.file_path, images);
        let response = window.prompt(PromptLevel::Info, &question, None, &[t!("btn.adopt_location").as_str(), t!("btn.cancel").as_str()], cx);
        cx.spawn_in(window, async move |this, cx| {
            let yes = response.await == Ok(0);
            let _ = this.update_in(cx, |v, window, cx| {
                if yes {
                    v.conclude_relink(&r, ready, window, cx);
                } else {
                    std::thread::spawn(move || drop(ready));
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Abrir uma fonte durante uma renomeacao: o aviso diz "aguarde" e nada abre.
    pub(super) fn allows_open(&mut self, cx: &mut Context<Self>) -> bool {
        match self.exclusive.allows_open() {
            Ok(()) => true,
            Err(e) => {
                self.state.warning = Some(e);
                cx.notify();
                false
            }
        }
    }

    /// Comeca um dos trabalhos exclusivos (troca de vault, nova localizacao,
    /// renomeacao). Outro em andamento: o aviso diz qual, e nada comeca.
    pub(super) fn start_exclusive(&mut self, label: String, cx: &mut Context<Self>) -> bool {
        match self.exclusive.start(label) {
            Ok(()) => true,
            Err(e) => {
                self.state.warning = Some(e);
                cx.notify();
                false
            }
        }
    }
}

/// Realce por cima da janela enquanto algo e arrastado sobre ela: borda e
/// "Solte para abrir a fonte". Sem ouvintes de mouse (nao rouba o drop da
/// raiz). O gpui tira o arrasto quando ele sai da janela sem redesenhar: o
/// canvas pede o redesenho nesse `FileDropEvent::Exited`.
pub(super) fn drop_highlight() -> impl IntoElement {
    let on_exit = gpui::canvas(
        |_, _, _| (),
        |_, _, window, _| {
            window.on_mouse_event(|e: &gpui::FileDropEvent, _, window, _| {
                if matches!(e, gpui::FileDropEvent::Exited) {
                    window.refresh();
                }
            })
        },
    )
    .absolute()
    .size_full();
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .border_2()
        .border_color(rgb(crate::theme::current().drop_border))
        .flex()
        .items_center()
        .justify_center()
        .child(on_exit)
        .child(div().px_3().py_1().rounded_md().bg(rgb(crate::theme::current().drop_background)).child(t!("source.solte_to_open")))
}

/// "Como abrir este zip?" (Task 12): `None` se cancelou.
pub(super) async fn ask_mode_zip(cx: &mut gpui::AsyncWindowContext) -> Option<ZipMode> {
    let r = cx
        .update(|window, cx| {
            window.prompt(
                PromptLevel::Info,
                &t!("dialog.zip.question"),
                Some(&t!("dialog.zip.detail")),
                &[t!("btn.open_direct").as_str(), t!("btn.copy_to_cache").as_str(), t!("btn.cancel").as_str()],
                cx,
            )
        })
        .ok()?;
    match r.await {
        Ok(0) => Some(ZipMode::Direct),
        Ok(1) => Some(ZipMode::Cache),
        _ => None,
    }
}
