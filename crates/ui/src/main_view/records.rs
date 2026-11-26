//! Faixa de registro no topo do leitor: diz em que registro a imagem atual
//! esta e oferece marcar o inicio e o fim, renomear, cancelar e excluir. A regra
//! e as gravacoes ficam em `AppState` (`app_state/records.rs`); aqui so se
//! desenha e se traduz clique em chamada. Os botoes chamam metodos da view
//! direto: nao ha `MenuAction` nem atalho para registros.

use super::*;
use crate::app_state::RecordStrip;
use marginalia_core::model::Position;

/// O que a caixa de nome da faixa esta fazendo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecordEditing {
    /// Pedindo o nome de um registro que comeca na imagem atual.
    New,
    /// Renomeando o registro `id`.
    Rename(String),
}

/// O que cada botao da faixa faz agora (`None`/`false`: apagado).
#[derive(Default)]
struct Buttons {
    start: bool,
    /// Registro cujo fim se marca/ajusta (id) e se ele ainda esta sem fim.
    end: Option<(String, bool)>,
    extend: Option<marginalia_core::model::Record>,
    /// Registro a renomear (id, nome).
    rename: Option<(String, String)>,
    /// Registro a excluir (id) e se pede confirmacao.
    delete: Option<(String, bool)>,
}

impl MainView {
    /// A faixa; `None` sem vault ou sem fonte aberta.
    pub(super) fn record_strip(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let strip = self.state.record_strip()?;
        let base = div()
            .w_full()
            .h(px(layout::TOOLS_HEIGHT))
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_2()
            .overflow_hidden()
            .text_xs()
            .bg(rgb(crate::theme::current().bar))
            .border_b_1()
            .border_color(rgb(crate::theme::current().border));
        if self.record_editing.is_some() {
            return Some(
                base.child(t!("record.strip.name"))
                    .child(div().flex_1().min_w(px(0.0)).flex().flex_row().child(self.record_name.clone()))
                    .child(btn(t!("btn.cancel"), cx, |v, w, cx| v.cancel_name_record(w, cx)))
                    .into_any_element(),
            );
        }
        if let Some(id) = self.record_deleting.clone() {
            let name = self.state.records().iter().find(|r| r.id == id).map(|r| r.name.clone()).unwrap_or_default();
            let target = id.clone();
            return Some(
                base.child(div().flex_1().min_w(px(0.0)).overflow_hidden().child(t!("record.strip.confirm_delete", name)))
                    .child(btn(t!("btn.delete"), cx, move |v, w, cx| v.delete_record(&target, w, cx)))
                    .child(btn(t!("btn.cancel"), cx, |v, _, cx| {
                        v.record_deleting = None;
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        let here = self.state.current.image.clone();
        Some(match strip {
            RecordStrip::Outside => {
                // O registro que terminou antes pode ser levado ate aqui.
                let extend = self.state.record_to_extend();
                let title = div().flex_1().min_w(px(0.0)).opacity(0.6).child(t!("record.strip.outside"));
                let buttons = self.strip_buttons(
                    Buttons { start: true, extend, ..Buttons::default() },
                    cx,
                );
                base.child(title).child(buttons).into_any_element()
            }
            RecordStrip::Invalid { record } => {
                let title = div().flex_1().min_w(px(0.0)).overflow_hidden().child(t!("record.strip.invalid", record.name));
                let buttons = self.strip_buttons(Buttons { delete: Some((record.id.clone(), true)), ..Buttons::default() }, cx);
                base.child(title).child(buttons).into_any_element()
            }
            RecordStrip::Em { record, end, shared } => {
                let is_open = record.end.is_none();
                let title = if is_open {
                    t!("record.strip.no_end", record.name, self.state.position_label(&record.start))
                } else {
                    t!("record.strip.em", record.name, self.state.record_range(&record))
                };
                let title = if shared { format!("{title} {}", t!("record.strip.boundary")) } else { title };
                // Na imagem em que o registro termina da para comecar o seguinte.
                let here_ends = end.as_ref() == Some(&Position::de(&here)) && !is_open;
                // "Cancelar" so para o que acabou de ser criado; qualquer outro registro pede confirmacao.
                let new = is_open && self.created_just.as_deref() == Some(record.id.as_str());
                let buttons = self.strip_buttons(
                    Buttons {
                        start: here_ends,
                        end: Some((record.id.clone(), is_open)),
                        rename: Some((record.id.clone(), record.name.clone())),
                        delete: Some((record.id.clone(), !new)),
                        ..Buttons::default()
                    },
                    cx,
                );
                base.child(div().flex_1().min_w(px(0.0)).overflow_hidden().child(title)).child(buttons).into_any_element()
            }
        })
    }

    /// Os cinco botoes de icone da faixa, sempre nos mesmos lugares; o que nao vale para a
    /// imagem atual fica apagado e sem clique.
    fn strip_buttons(&self, b: Buttons, cx: &mut Context<Self>) -> gpui::Div {
        let mut line = div().flex_none().flex().flex_row().items_center();
        // Inicio: fora de registro, ou na imagem em que um termina (o seguinte comeca ali).
        line = line.child(button_icon_se(
            "registro-inicio",
            Icon::RecordStart,
            t!("record.btn.mark_start"),
            b.start,
            cx,
            |v, w, cx| v.start_mark_start(w, cx),
        ));
        // Fim: dentro de um registro (marcar o fim, ou ajustar um fim ja marcado).
        let (end_id, hint_end) = match &b.end {
            Some((id, is_open)) => (Some(id.clone()), if *is_open { t!("record.btn.mark_end") } else { t!("record.btn.adjust_end") }),
            None => (None, t!("record.btn.mark_end")),
        };
        line = line.child(button_icon_se("registro-fim", Icon::RecordEnd, hint_end, end_id.is_some(), cx, move |v, w, cx| {
            if let Some(id) = &end_id {
                v.record_end_marker(id, w, cx)
            }
        }));
        // Estender: fora de registro, leva ate aqui o que terminou antes (e o mesmo
        // `record_end_marker`, mas num registro que nao e o desta imagem).
        let (est_id, est_hint) = match &b.extend {
            Some(r) => (Some(r.id.clone()), t!("record.btn.extend", r.name)),
            None => (None, t!("record.btn.extend_no_target")),
        };
        line = line.child(button_icon_se("registro-estender", Icon::ExtendRecord, est_hint, est_id.is_some(), cx, move |v, w, cx| {
            if let Some(id) = &est_id {
                v.record_end_marker(id, w, cx)
            }
        }));
        let ren = b.rename.clone();
        line = line.child(button_icon_se("registro-renomear", Icon::Edit, t!("btn.rename"), ren.is_some(), cx, move |v, w, cx| {
            if let Some((id, name)) = &ren {
                v.start_rename_record(id, name, w, cx)
            }
        }));
        let exc = b.delete.clone();
        let hint = match &exc {
            Some((_, false)) => t!("btn.cancel"),
            _ => t!("btn.delete"),
        };
        line.child(button_icon_se("registro-excluir", Icon::Trash, hint, exc.is_some(), cx, move |v, w, cx| {
            if let Some((id, confirm)) = &exc {
                v.request_delete_record(id, *confirm, w, cx)
            }
        }))
    }

    /// "Marcar inicio de registro aqui": a caixa de nome aparece na faixa.
    pub fn start_mark_start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.record_editing = Some(RecordEditing::New);
        self.record_deleting = None;
        self.record_name.update(cx, |n, cx| {
            n.configure(true, t!("record.strip.name_placeholder"), cx);
            n.set_text("", cx);
        });
        self.record_name.read(cx).focus_handle().focus(window);
        cx.notify();
    }

    pub fn start_rename_record(&mut self, id: &str, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.record_editing = Some(RecordEditing::Rename(id.to_string()));
        self.record_deleting = None;
        self.record_name.update(cx, |n, cx| {
            n.configure(true, t!("record.strip.name_placeholder"), cx);
            n.set_text(name, cx);
        });
        self.record_name.read(cx).focus_handle().focus(window);
        cx.notify();
    }

    pub fn cancel_name_record(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.record_editing = None;
        self.focus_image(window, cx);
    }

    /// Texto da caixa de nome (para o smoke).
    pub fn name_record_text(&self, cx: &App) -> String {
        self.record_name.read(cx).text().to_string()
    }

    pub fn record_editing(&self) -> Option<&RecordEditing> {
        self.record_editing.as_ref()
    }

    /// Enter na caixa de nome: cria o registro ou renomeia. Nome recusado: o
    /// aviso diz por que e a caixa fica.
    pub fn confirm_name_record(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editing) = self.record_editing.clone() else { return };
        if self.allows_write(cx).is_err() {
            return;
        }
        let r = match &editing {
            RecordEditing::New => self.state.mark_start_record(text, chrono::Local::now().naive_local()).map(|r| {
                self.created_just = Some(r.id);
            }),
            RecordEditing::Rename(id) => self.state.rename_record(id, text),
        };
        if r.is_ok() {
            self.record_editing = None;
            self.written_record(cx);
            self.focus_image(window, cx);
        } else {
            cx.notify();
        }
    }

    /// "Marcar fim aqui" / "Ajustar fim aqui".
    pub fn record_end_marker(&mut self, id: &str, _window: &mut Window, cx: &mut Context<Self>) {
        if self.allows_write(cx).is_err() {
            return;
        }
        if self.state.mark_end_record(id).is_ok() {
            self.written_record(cx);
        } else {
            cx.notify();
        }
    }

    /// "Cancelar" (sem fim: apaga na hora) e "Excluir" (com fim: pede confirmacao na faixa).
    pub fn request_delete_record(&mut self, id: &str, confirm: bool, window: &mut Window, cx: &mut Context<Self>) {
        if confirm {
            self.record_deleting = Some(id.to_string());
            cx.notify();
        } else {
            self.delete_record(id, window, cx);
        }
    }

    pub fn delete_record(&mut self, id: &str, _window: &mut Window, cx: &mut Context<Self>) {
        self.record_deleting = None;
        if self.allows_write(cx).is_err() {
            return;
        }
        if self.state.discard_record(id).is_ok() {
            self.written_record(cx);
        } else {
            cx.notify();
        }
    }

    /// "Mover para <registro>" de uma nota ou tarefa numa imagem de fronteira.
    pub fn relocate_record_item(&mut self, kind: marginalia_core::vault::DocKind, id: &str, destination: Option<&str>, cx: &mut Context<Self>) {
        if self.allows_write(cx).is_err() {
            return;
        }
        if self.state.relocate_to_record(kind, id, destination).is_ok() {
            self.written_record(cx);
        } else {
            cx.notify();
        }
    }

    /// Uma gravacao de registro deu certo: mostra a confirmacao e relê as gavetas.
    fn written_record(&mut self, cx: &mut Context<Self>) {
        if let Some(m) = self.state.confirmation.take() {
            self.confirm(m, cx);
        }
        self.refresh(cx);
    }
}
