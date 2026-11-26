//! Seletor de vault, trocar/criar vault e renomear fonte.

use super::*;

impl MainView {
    pub(super) fn vault_items(&self) -> Vec<vault_picker::ItemVault> {
        vault_picker::items(self.state.app_data.known_vaults(), self.state.vault.as_ref().map(|v| v.root.as_path()))
    }

    pub fn picker(&self) -> &Picker {
        &self.picker
    }

    pub fn open_picker(&mut self, cx: &mut Context<Self>) {
        let items = self.vault_items();
        self.picker.open(&items);
        cx.notify();
    }

    /// Cmd-Shift-V: abre (com o foco na imagem, para as setas chegarem) ou fecha.
    pub fn toggle_window_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.picker.is_open() {
            self.picker.close();
        } else {
            self.open_picker(cx);
            self.focus_image(window, cx);
        }
        cx.notify();
    }

    pub(super) fn picker_action(&mut self, a: PickerAction, window: &mut Window, cx: &mut Context<Self>) {
        let items = self.vault_items();
        match a {
            PickerAction::Relocate(d) => self.picker.relocate(d, items.len()),
            PickerAction::Close => self.picker.close(),
            PickerAction::Choose => {
                if let Some(i) = self.picker.selection(items.len()).and_then(|i| items.get(i)) {
                    self.switch_vault(i.file_path.clone(), false, window, cx);
                }
            }
            PickerAction::OpenFolder => self.dialog_folder_vault(false, window, cx),
            PickerAction::CreateHere => self.dialog_folder_vault(true, window, cx),
        }
        cx.notify();
    }

    /// "Abrir pasta..." / "Criar vault aqui": painel de pastas do sistema (pode criar pasta nova).
    pub(super) fn dialog_folder_vault(&mut self, create: bool, window: &mut Window, cx: &mut Context<Self>) {
        let choice = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(if create { t!("btn.create_vault_here").into() } else { t!("dialog.vault.open").into() }),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(file_paths))) = choice.await else { return };
            let Some(folder) = file_paths.into_iter().next() else { return };
            let _ = this.update_in(cx, |v, window, cx| v.switch_vault(folder, create, window, cx));
        })
        .detach();
    }

    /// Troca para o vault em `root`: valida (e com `create` prepara as subpastas)
    /// e le os grifos da fonte aberta no fundo. Falha: o vault atual continua e o
    /// aviso diz por que; uma pasta que nao e vault pergunta se deve criar.
    pub fn switch_vault(&mut self, root: PathBuf, create: bool, window: &mut Window, cx: &mut Context<Self>) {
        // Recusada (outro trabalho em andamento): o seletor fica aberto.
        if !self.start_exclusive(t!("status.vault_opening"), cx) {
            return;
        }
        self.picker.close();
        let source = self.state.source_has().then(|| self.state.current.image.source.clone());
        self.work = Some(cx.spawn_in(window, async move |this, cx| {
            let r2 = root.clone();
            let r = cx
                .background_executor()
                .spawn(async move { vault_picker::open_with_highlights(&r2, create, source.as_deref()) })
                .await;
            let _ = this.update_in(cx, |v, window, cx| {
                v.work = None;
                v.exclusive.finish();
                match r {
                    Ok(a) => v.open_vault(a, window, cx),
                    Err(VaultError::NotAVault(p)) => v.ask_create_vault(p, window, cx),
                    Err(e) => v.state.warning = Some(e.to_string()),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn ask_create_vault(&mut self, folder: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let question = t!("dialog.vault.not_and_vault", folder.display());
        let r = window.prompt(
            PromptLevel::Info,
            &question,
            Some(&t!("dialog.vault.create_folders")),
            &[t!("btn.create_vault_here").as_str(), t!("btn.cancel").as_str()],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            if r.await == Ok(0) {
                let _ = this.update_in(cx, |v, window, cx| v.switch_vault(folder, true, window, cx));
            }
        })
        .detach();
    }

    pub(super) fn open_vault(&mut self, a: vault_picker::Open, window: &mut Window, cx: &mut Context<Self>) {
        let name = layout::folder_name(&a.vault.root);
        // Uma abertura do vault anterior ainda no fundo: o resultado e descartado
        // (`AppState::enter_source`), e o viewer nao fica preso em "carregando".
        self.opening = None;
        self.state.drop_load_postponed(&t!("error.vault_changed"));
        self.viewer.update(cx, |vw, cx| vw.set_loading(false, cx));
        let swap = self.state.adopt_vault(a);
        if swap.closed_source.is_some() {
            self.viewer.update(cx, |vw, cx| vw.clear(window, cx));
        }
        self.drawers.clear_other_vault_filters();
        self.search.update(cx, |b, cx| b.load_text("", cx));
        self.renaming = None;
        if let Some(warning) = swap.source_warning() {
            self.state.warning = Some(warning);
        }
        // A nota em edicao termina no vault em que comecou (`sync` abaixo).
        let msg = swap.message(&name, self.session.has_item(), !self.quick_note.read(cx).text().is_empty());
        self.confirm(msg, cx);
        self.sync(cx);
        self.refresh(cx);
    }

    /// "Renomear" na gaveta de fontes: a caixa aparece na linha com o nome atual selecionado.
    pub fn start_rename(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.renaming = Some(name.to_string());
        self.rename.update(cx, |n, cx| {
            n.configure(true, t!("drawer.sources.new_name"), cx);
            n.set_text(name, cx);
        });
        self.rename.read(cx).focus_handle().focus(window);
        cx.notify();
    }

    pub fn cancel_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.renaming = None;
        self.focus_image(window, cx);
    }

    pub fn renaming(&self) -> Option<&str> {
        self.renaming.as_deref()
    }

    /// Texto da caixa de renomear (para o smoke).
    pub fn rename_textbox_text(&self, cx: &App) -> String {
        self.rename.read(cx).text().to_string()
    }

    /// Enter na caixa de renomear: confere o nome (aqui, sem disco), reescreve
    /// o vault no fundo (`Vault::rename_source`) e depois atualiza dados do app,
    /// fonte aberta e gavetas. Nome recusado: o aviso diz por que e a caixa fica.
    pub fn confirm_rename(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(old) = self.renaming.clone() else { return };
        let existing = self.drawers.sources().iter().map(|f| f.name.clone()).collect();
        let existing = self.state.busy_names(&old, existing);
        let new = match relink::validate_name(text, &old, &existing) {
            Ok(n) => n,
            Err(e) => {
                self.state.warning = Some(e);
                cx.notify();
                return;
            }
        };
        let Some(vault) = self.state.vault.clone() else { return };
        if self.session.in_flight() {
            // A renomeacao reescreve as notas: nunca junto com uma gravacao da nota.
            self.state.warning = Some(t!("warning.quick_note.sendo_salva"));
            cx.notify();
            return;
        }
        if !self.start_exclusive(relink::renaming(), cx) {
            return;
        }
        self.pause_quick_note();
        // O vault em que a renomeacao foi pedida (o ativo pode mudar no meio).
        let root_dir = vault.root.clone();
        self.work = Some(cx.spawn_in(window, async move |this, cx| {
            let (a, n) = (old.clone(), new.clone());
            let r = cx
                .background_executor()
                .spawn(async move {
                    let r = vault.rename_source(&a, &n);
                    // Parou no meio? `sources/<new>.md` ja existe: o mapa vale para os dois nomes.
                    let failure = r.as_ref().err().map(|e| sort_failure_rename(&vault, &a, &n, e));
                    (r, failure)
                })
                .await;
            let _ = this.update_in(cx, |v, window, cx| {
                v.work = None;
                v.exclusive.finish();
                let same = v.state.vault.as_ref().is_some_and(|x| x.root == root_dir);
                let outcome = match &r {
                    (Ok(()), _) => Ok(()),
                    (Err(_), failure) => Err(failure.unwrap_or(FailureRename::Common)),
                };
                match (&r.0, outcome) {
                    (Err(e), Err(failure)) => {
                        let _ = v.state.rename_failed(&root_dir, &old, &new, &e.to_string(), failure);
                    }
                    _ => v.state.conclude_rename(&root_dir, &old, &new),
                }
                let follows = renaming_follower(outcome, same);
                if follows.session {
                    // Antes de qualquer `observe`: a troca de nome nao e troca de imagem.
                    v.session.renamed_source(&root_dir, &old, &new);
                }
                if follows.screen {
                    // Nem para os ajustes: a base pendente e o filtro em voo seguem.
                    v.schedule.renamed_source(&old, &new);
                    v.drawers.rename_source(&old, &new);
                    v.renaming = None;
                }
                if follows.confirms {
                    v.confirm_state(cx);
                    v.focus_image(window, cx);
                }
                // Sempre: mesmo uma falha pode ter mudado o vault (parou no meio).
                v.refresh(cx);
                v.sync(cx);
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn vault_picker(&self, view_window: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let (left, width) = layout::picker_textbox(view_window);
        let items = self.vault_items();
        let sel = self.picker.selection(items.len());
        let mut list = div().id("lista-vaults").flex().flex_col().gap_1().max_h(px(360.0)).overflow_y_scroll();
        for (i, it) in items.iter().enumerate() {
            let c = it.file_path.clone();
            let label = if it.active { t!("picker.active", it.label) } else { it.label.clone() };
            list = list.child(
                drawer_line(Some(i) == sel, it.active)
                    .child(div().font_weight(FontWeight::BOLD).child(label))
                    .child(div().text_xs().text_color(rgb(crate::theme::current().soft_text)).truncate().child(it.file_path.display().to_string()))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |v, _: &MouseDownEvent, w, cx| v.switch_vault(c.clone(), false, w, cx)),
                    ),
            );
        }
        if items.is_empty() {
            list = list.child(hint(t!("picker.empty")));
        }
        div()
            .absolute()
            .top(px(40.0))
            .left(px(left))
            .w(px(width))
            .overflow_hidden()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .rounded_md()
            .bg(rgb(crate::theme::current().card))
            .border_1()
            .border_color(rgb(crate::theme::current().linked_border))
            .text_sm()
            .occlude()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(div().font_weight(FontWeight::BOLD).child(t!("picker.title")))
                    .child(div().text_xs().text_color(rgb(crate::theme::current().soft_text)).child(vault_picker::shortcut()))
                    .child(div().flex_1())
                    .child(btn("x", cx, |v, _, cx| {
                        v.picker.close();
                        cx.notify();
                    })),
            )
            .child(list)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(btn(t!("picker.button_open"), cx, |v, w, cx| v.dialog_folder_vault(false, w, cx)))
                    .child(btn(t!("picker.button_create"), cx, |v, w, cx| v.dialog_folder_vault(true, w, cx))),
            )
            .child(hint(t!("picker.hint")))
    }
}
