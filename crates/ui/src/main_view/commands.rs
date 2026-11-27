//! Comandos (menu, atalho, botao), teclado, desfazer e confirmacoes.

use super::*;

impl MainView {
    /// Mostra na barra (por um tempo) a confirmacao que o estado deixou.
    pub(super) fn confirm_state(&mut self, cx: &mut Context<Self>) {
        if let Some(c) = self.state.confirmation.clone() {
            self.confirm(c, cx);
        }
    }

    /// Gravar no vault durante a renomeacao: recusa, com o "aguarde" na barra.
    pub(super) fn allows_write(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        let r = self.exclusive.allows_write();
        if let Err(e) = &r {
            self.state.warning = Some(e.clone());
            cx.notify();
        }
        r
    }

    /// Mostra `msg` na barra por [`CONFIRMATION`].
    pub(super) fn confirm(&mut self, msg: String, cx: &mut Context<Self>) {
        self.state.confirmation = Some(msg);
        self.generation_confirmation += 1;
        let g = self.generation_confirmation;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(CONFIRMATION).await;
            let _ = this.update(cx, |v, cx| {
                if v.generation_confirmation == g {
                    v.state.confirmation = None;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    /// O botao de cores da barra de ferramentas (e o item do menu Imagem): abre/fecha o menuzinho.
    pub fn toggle_colors(&mut self, cx: &mut Context<Self>) {
        self.open_colors = !self.open_colors;
        cx.notify();
    }

    pub fn open_colors(&self) -> bool {
        self.open_colors
    }

    /// `?`, o icone da barra de atividades e o menu Ajuda: abre/fecha a tela de ajuda.
    /// Ao abrir, o foco vai para a imagem (nada que se digite vai parar na nota por baixo).
    pub fn toggle_help(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_help = !self.open_help;
        if self.open_help {
            self.config_open = false;
            self.focus_image(window, cx);
        }
        cx.notify();
    }

    /// A tela de ajuda esta aberta (o smoke confere).
    pub fn open_help(&self) -> bool {
        self.open_help
    }

    /// A barra de ferramentas esta a mostra (dados do app; padrao: sim).
    pub fn visible_tools(&self) -> bool {
        !self.state.app_data.hidden_tools()
    }

    /// Menu Exibir > Barra de ferramentas: mostra/oculta e guarda nos dados do app. Ao
    /// ocultar, a ferramenta de grifo desliga (sem barra o cursor em cruz ficaria preso).
    pub fn toggle_tools(&mut self, cx: &mut Context<Self>) {
        let hides = self.state.app_data.hidden_tools();
        self.state.app_data.set_tools_hidden(!hides);
        if !hides {
            self.viewer.update(cx, |v, cx| v.disable_tool(cx));
        }
        self.write_adjustments(cx);
        cx.notify();
    }

    /// O que um item de menu, um botao ou uma tecla faz (`menus::MenuAction`): o
    /// mesmo metodo para os tres. As teclas continuam decidindo antes se valem
    /// (foco, tecla segurada); o menu passa por [`MainView::menu_command`].
    pub fn command(&mut self, a: menus::MenuAction, window: &mut Window, cx: &mut Context<Self>) {
        use menus::MenuAction as A;
        match a {
            A::About => self.about(window, cx),
            // A janela do aviso e a checagem chegam na proxima tarefa do plano.
            A::CheckUpdates => {}
            // Sair e do app (`main.rs`): o `on_app_quit` grava a nota e apaga o cache.
            A::Exit => cx.quit(),
            A::OpenSource => self.dialog_open(window, cx),
            A::OpenVault => self.toggle_window_picker(window, cx),
            A::CloseSource => self.close_source(window, cx),
            A::Undo => self.undo(window, cx),
            // Da caixa de texto em foco (`QuickNoteBox::edit`), nao daqui.
            A::Cut | A::Copy | A::Paste | A::SelectAll => {}
            A::DeleteHighlight => self.delete_highlight_selected(cx),
            A::DeleteItem => {
                if let Some(g) = self.drawers.open() {
                    self.drawer_action(g, DrawerAction::Delete, window, cx);
                }
            }
            A::Notes | A::Sources | A::Tasks => {
                if let Some(g) = a.drawer() {
                    self.toggle_drawer(g, window, cx);
                }
            }
            A::AdjustmentsPanel => self.toggle_adjustments(cx),
            A::ZoomUp | A::ZoomDown | A::Adjust => self.viewer.update(cx, |v, cx| v.zoom_by(a, cx)),
            A::Previous => {
                self.go_prev(window, cx);
            }
            A::Next => {
                self.go_next(window, cx);
            }
            A::FocusQuickNote => self.focus_command(Command::FocusQuickNote, window, cx),
            A::FocusImage => self.focus_command(Command::FocusImage, window, cx),
            A::BrightnessUp => self.relocate(Control::Brightness, 1, window, cx),
            A::BrightnessDown => self.relocate(Control::Brightness, -1, window, cx),
            A::ContrastUp => self.relocate(Control::Contrast, 1, window, cx),
            A::ContrastDown => self.relocate(Control::Contrast, -1, window, cx),
            A::SharpnessUp => self.relocate(Control::Sharpness, 1, window, cx),
            A::SharpnessDown => self.relocate(Control::Sharpness, -1, window, cx),
            A::HighpassUp => self.relocate(Control::Highpass, 1, window, cx),
            A::HighpassDown => self.relocate(Control::Highpass, -1, window, cx),
            A::ThresholdUp => self.relocate(Control::Threshold, 1, window, cx),
            A::ThresholdDown => self.relocate(Control::Threshold, -1, window, cx),
            A::Invert => self.toggle(|a| a.invert = !a.invert, window, cx),
            A::Gray => self.toggle(|a| a.grayscale = !a.grayscale, window, cx),
            A::Threshold => self.toggle(|a| a.threshold = if a.threshold.is_some() { None } else { Some(128) }, window, cx),
            A::Restore => self.set_adjustments(Adjustments::default(), window, cx),
            A::RotateLeft => self.rotate(false, window, cx),
            A::RotateRight => self.rotate(true, window, cx),
            A::HighlightBox => self.viewer.update(cx, |v, cx| v.toggle_tool(annotate::Tool::Textbox, cx)),
            A::ArrowHighlight => self.viewer.update(cx, |v, cx| v.toggle_tool(annotate::Tool::Arrow, cx)),
            A::HighlightColors => self.toggle_colors(cx),
            A::Tools => self.toggle_tools(cx),
            A::Help => self.toggle_help(window, cx),
            A::Settings => self.toggle_config(window, cx),
        }
    }

    /// Um item de menu escolhido: so roda se ainda se aplica (o menu foi
    /// validado no quadro anterior); senao a barra diz por que.
    pub fn menu_command(&mut self, a: menus::MenuAction, window: &mut Window, cx: &mut Context<Self>) {
        if menus::available(a, &self.state_menu(window, cx)) {
            self.command(a, window, cx);
        } else {
            self.state.warning = Some(t!("warning.not_se_aplica", menus::hint(a)));
            cx.notify();
        }
    }

    /// O que liga/desliga os itens de menu da view.
    pub fn state_menu(&self, window: &Window, cx: &App) -> menus::State {
        menus::State {
            source_has: self.state.source_has(),
            selected: self.state.annot.selected.is_some(),
            image_in_focus: self.current_focus(window, cx) == Focus::Image,
            undo: self.target_undo(window, cx).is_some(),
            delete_item: self.can_delete_item(window, cx),
        }
    }

    /// Backspace / "Excluir nota/tarefa…": so com a gaveta de notas ou de
    /// tarefas em foco, uma linha escolhida e nenhuma confirmacao aberta (como a tecla).
    pub(super) fn can_delete_item(&self, window: &Window, cx: &App) -> bool {
        self.drawers.can_delete_item(self.current_focus(window, cx) == Focus::Drawer)
    }

    pub(super) fn target_undo(&self, window: &Window, cx: &App) -> Option<focus_state::TargetUndo> {
        let in_text = self.quick_note.read(cx).focus_handle().is_focused(window)
            || self.rename.read(cx).focus_handle().is_focused(window)
            || self.record_name.read(cx).focus_handle().is_focused(window)
            || self.ir_to.read(cx).focus_handle().is_focused(window)
            || self.search.read(cx).focus_handle().is_focused(window);
        let focus_state = self.current_focus(window, cx);
        let modal = self.drawers.deletion_confirming().is_some();
        focus_state::target_undo(focus_state, in_text, self.state.can_undo_highlight(), modal)
    }

    /// Cmd+Z / "Editar > Desfazer": o grifo apagado (imagem em foco, `focus_state::target_undo`);
    /// num campo de texto, nada.
    pub fn undo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.target_undo(window, cx) {
            Some(focus_state::TargetUndo::Highlight) => self.undo_highlight(cx),
            None => {
                self.state.warning = Some(t!("warning.nothing_the_undo"));
                cx.notify();
            }
        }
    }

    /// A barra de menus (fora do macOS) no teclado, na fase de captura (antes da caixa de nota):
    /// `Alt+letra` abre o menu da letra; com a barra em uso, setas, Enter, Esc e letras valem
    /// so para ela (o resto e engolido, como num menu do Windows); um atalho com Ctrl fecha a
    /// barra e segue o caminho de sempre.
    pub(super) fn on_key_menu(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if debug_keys() {
            let (k, m) = (&ev.keystroke, ev.keystroke.modifiers);
            eprintln!(
                "teclas: key={:?} key_char={:?} ctrl={} alt={} shift={} win={} repetida={}",
                k.key, k.key_char, m.control, m.alt, m.shift, m.platform, ev.is_held
            );
        }
        if cfg!(target_os = "macos") {
            return;
        }
        let k = &ev.keystroke;
        let m = crate::normalize_modifiers(k.modifiers);
        let font = {
            let mut cs = k.key.chars();
            match (cs.next(), cs.next()) {
                (Some(c), None) if c.is_alphabetic() => Some(c),
                _ => None,
            }
        };
        let state = self.state_menu(window, cx);
        let view = crate::menu_bar::view(&state);
        if !self.menu.active() {
            if let (Some(c), true) = (font, m.alt && !m.platform && !m.control && !m.shift) {
                if self.menu.alt_font(c, &view) {
                    cx.stop_propagation();
                    cx.notify();
                }
            }
            return;
        }
        if m.platform {
            // Ctrl+algo: um atalho do app; a barra sai da frente.
            self.menu.close();
            cx.notify();
            return;
        }
        cx.stop_propagation();
        use crate::menu_bar::{Output, Key};
        let key = match (k.key.as_str(), font) {
            ("escape", _) => Some(Key::Esc),
            ("left", _) => Some(Key::Left),
            ("right", _) => Some(Key::Right),
            ("up", _) => Some(Key::Up),
            ("down", _) => Some(Key::Down),
            ("enter", _) => Some(Key::Enter),
            (_, Some(c)) if !m.control => Some(Key::Font(c)),
            _ => None,
        };
        if let Some(t) = key {
            if let Output::MenuAction(a) = self.menu.key(t, &view) {
                cx.notify();
                self.command(a, window, cx);
                return;
            }
        }
        cx.notify();
    }

    /// O `Alt` apertado mostra as letras de acesso da barra de menus sublinhadas (fora do macOS),
    /// como no Windows; so repinta quando ele muda.
    pub(crate) fn changed_modifiers(&mut self, ev: &ModifiersChangedEvent, _: &mut Window, cx: &mut Context<Self>) {
        if debug_keys() {
            let m = ev.modifiers;
            eprintln!("teclas: modificadores ctrl={} alt={} shift={} win={}", m.control, m.alt, m.shift, m.platform);
        }
        if cfg!(target_os = "macos") {
            return;
        }
        if self.alt_held != ev.modifiers.alt {
            self.alt_held = ev.modifiers.alt;
            cx.notify();
        }
    }

    /// A barra de menus, para o smoke.
    pub fn menu_bar(&self) -> &crate::menu_bar::MenuBar {
        &self.menu
    }

    /// Onde o menu aberto foi desenhado, na ultima pintura (coordenadas da janela).
    pub fn limits_menu(&self) -> Option<Bounds<Pixels>> {
        self.limits_menu.get()
    }

    /// Clique do mouse no titulo `i` (o que o handler do titulo faz), para o smoke.
    pub fn menu_click_title(&mut self, i: usize, cx: &mut Context<Self>) {
        self.menu.click(i);
        cx.notify();
    }

    pub(super) fn on_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // Cmd-Q, Cmd-O e Cmd-Shift-V sao acoes do app (`main.rs`: teclas e menu),
        // que valem de qualquer foco e mesmo sem foco; nao chegam aqui.
        let k = &ev.keystroke;
        let mods = crate::normalize_modifiers(k.modifiers);
        let no_mod = !(mods.platform || mods.control || mods.alt);
        if self.open_help && no_mod {
            // Com a ajuda aberta so Esc e `?` valem (fecham); o resto e engolido.
            cx.stop_propagation();
            let ch = k.key_char.as_deref().unwrap_or(k.key.as_str());
            if !ev.is_held && (k.key == "escape" || ch == "?") {
                self.open_help = false;
                cx.notify();
            }
            return;
        }
        if self.config_open && no_mod {
            // Com as Configuracoes abertas so Esc vale (fecha); o resto e engolido.
            cx.stop_propagation();
            if !ev.is_held && k.key == "escape" {
                self.config_open = false;
                cx.notify();
            }
            return;
        }
        if self.open_colors && no_mod && k.key == "escape" {
            cx.stop_propagation();
            self.open_colors = false;
            cx.notify();
            return;
        }
        if self.picker.is_open() && no_mod {
            if let Some(a) = vault_picker::picker_key(&k.key) {
                cx.stop_propagation();
                self.picker_action(a, window, cx);
                return;
            }
        }
        if mods.platform && !mods.shift && !mods.control && !mods.alt {
            if let Some(g) = Drawer::key_by_the(&k.key) {
                // Cmd-1/2/3 de qualquer foco, como Cmd-Q (segurada nao pisca a gaveta).
                cx.stop_propagation();
                if ev.is_held {
                    return;
                }
                self.toggle_drawer(g, window, cx);
                return;
            }
        }
        let focus_state = self.current_focus(window, cx);
        let in_textbox = self.rename.read(cx).focus_handle().is_focused(window)
            || self.record_name.read(cx).focus_handle().is_focused(window)
            || self.ir_to.read(cx).focus_handle().is_focused(window)
            || self.search.read(cx).focus_handle().is_focused(window);
        if focus_state::text_field(focus_state, in_textbox) {
            // Digitando na nota, no nome novo, na busca ou na caixa numerica de uma barra:
            // as letras sao texto, nao atalhos.
            return;
        }
        let m = &mods;
        // A gaveta (lista, menu "Promover como", confirmacao de excluir) so ouve
        // o teclado com o foco NELA (fix round 1: com a imagem em foco as teclas
        // sao da imagem; W/A/S/D nunca sao da gaveta, `focus_state::drawer_key`).
        let in_drawer = focus_state == Focus::Drawer;
        if in_drawer && no_mod && self.drawers.deletion_confirming().is_some() {
            // A confirmacao de excluir fica com Enter e Esc; o resto (e a tecla
            // segurada: confirmar e sempre um toque novo) e engolido.
            cx.stop_propagation();
            if ev.is_held {
                return;
            }
            if let DeletionAction::Confirm(id, kind) = self.drawers.deletion_key(&k.key) {
                let _ = self.delete(&id, kind, cx);
            }
            cx.notify();
            return;
        }
        let modal = self.drawers.deletion_confirming().is_some();
        // Teclas da lista (setas, Enter, x) e Cmd+Backspace (excluir de vez; nao com uma
        // confirmacao aberta).
        if !(modal && m.platform) {
            if let Some(a) = focus_state::drawer_key(&k.key, focus_state, self.drawers.open(), m.shift, m.platform, m.control, m.alt) {
                if let Some(g) = self.drawers.open() {
                    cx.stop_propagation();
                    if ev.is_held && !focus_state::repetition_accepts(a) {
                        // Tecla segurada: so andar pela lista repete (Backspace
                        // segurado nao descarta varias notas).
                        return;
                    }
                    self.drawer_action(g, a, window, cx);
                    return;
                }
            }
        }
        // Grifos (so com a imagem em foco, `focus_state::highlight_key`): Delete/Backspace
        // apagam o selecionado, Cmd+Z desfaz, Esc tira a selecao antes de tudo.
        // Esc com uma ferramenta de grifo ligada so a desliga (a selecao fica; o proximo Esc solta).
        let is_on = self.viewer.read(cx).tool() != annotate::Tool::No;
        if focus_state::esc_disables_tool(&k.key, focus_state, m.shift, m.platform, m.control, m.alt, is_on) {
            cx.stop_propagation();
            self.viewer.update(cx, |v, cx| v.disable_tool(cx));
            cx.notify();
            return;
        }
        let selected = self.state.annot.selected.is_some();
        let can_undo = self.state.can_undo_highlight();
        if let Some(a) = focus_state::highlight_key(&k.key, focus_state, m.shift, m.platform, m.control, m.alt, selected, can_undo) {
            cx.stop_propagation();
            if ev.is_held && a != focus_state::HighlightAction::Deselect {
                return; // segurar Delete/Cmd+Z nao repete
            }
            match a {
                focus_state::HighlightAction::Delete => self.command(menus::MenuAction::DeleteHighlight, window, cx),
                // `target_undo` da o grifo aqui (imagem em foco, com o que desfazer).
                focus_state::HighlightAction::Undo => self.command(menus::MenuAction::Undo, window, cx),
                focus_state::HighlightAction::Deselect => {
                    self.state.select(None);
                    self.sync_screen(cx);
                    cx.notify();
                }
            }
            return;
        }
        // Cmd+Z: so desfaz grifo apagado, como "Editar > Desfazer"
        // (`focus_state::target_undo`); segurada nao repete.
        if m.platform && !m.shift && !m.control && !m.alt && k.key.eq_ignore_ascii_case("z") {
            if self.target_undo(window, cx).is_some() {
                cx.stop_propagation();
                if !ev.is_held {
                    self.command(menus::MenuAction::Undo, window, cx);
                }
            }
            return;
        }
        // W/A/S/D movem a imagem, so com ELA em foco (nunca num campo de texto nem
        // na gaveta) e sem modificador.
        if let Some(dir) = focus_state::pan_key(&k.key, focus_state, m.shift, m.platform, m.control, m.alt) {
            cx.stop_propagation();
            self.viewer.update(cx, |v, cx| v.steps_pan(dir, cx));
            return;
        }
        if k.modifiers.platform || k.modifiers.control || k.modifiers.alt {
            return;
        }
        // Setas/PageUp/PageDown pelo nome da tecla; o resto pelo caractere.
        let ch = match k.key.as_str() {
            n @ ("left" | "right" | "pageup" | "pagedown") => n,
            _ => k.key_char.as_deref().unwrap_or(k.key.as_str()),
        };
        if ch == "/" {
            cx.stop_propagation();
            self.focus_quick_note(window, cx);
            return;
        }
        // Teclas simples (`menus::KEYS`, as mesmas dos itens de menu); o zoom
        // (`+`, `-`, `0`) e do visualizador, so com ele em foco.
        if let Some(a) = menus::key_action(ch).filter(|a| !a.e_zoom()) {
            self.command(a, window, cx);
        }
    }
}

/// `MARGINALIA_DEBUG_KEYS=1` escreve no terminal cada tecla e cada mudanca de modificadores como
/// o gpui as entrega (para descobrir por que um atalho nao responde no teclado de verdade).
fn debug_keys() -> bool {
    static LINKED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *LINKED.get_or_init(|| std::env::var("MARGINALIA_DEBUG_KEYS").is_ok_and(|v| v == "1"))
}
