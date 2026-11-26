//! Passos internos: registros, envio de trabalho, termino de item e o proximo da fila.

use super::*;

impl QuickNoteSession {
    /// Imagem, fonte ou vault mudaram desde a ultima chamada: o item da caixa
    /// termina (vai para `leaving`, gravado e fixado no vault em que comecou).
    /// Devolve se soltou um item (a caixa deve limpar). Nota perdida fica na caixa.
    pub(super) fn notice(&mut self, st: &AppState) -> bool {
        self.base = Some(st.app_data.base().to_path_buf());
        let img = st.quick_note_image().cloned();
        let mut dropped = false;
        if let Some(it) = self.item.as_ref() {
            let other_vault = !st.vault.as_ref().is_some_and(|v| v.root == it.vault.root);
            if (other_vault || img != self.view_image) && !it.lost {
                let mut it = self.item.take().expect("existe");
                if it.new && !it.pinned && new_kind(&it.textbox).is_none() {
                    it.erase = true;
                }
                it.end = true;
                self.leaving.push_back(it);
                dropped = true;
            }
        }
        self.view_image = img;
        self.root_view = st.vault.as_ref().map(|v| v.root.clone());
        dropped
    }

    /// Registros de recuperacao: os nao tentados e os itens com chave.
    pub(super) fn records(&self) -> Vec<Record> {
        let mut v = self.records.clone();
        for it in self.leaving.iter().chain(self.item.iter()) {
            if let Some(c) = it.key.as_deref().filter(|_| !it.erase) {
                v.push(it.record(c));
            }
        }
        v
    }

    /// O registro em disco. Ilegivel: e isolado (`recovery.json.corrupt-*`,
    /// nunca sobrescrito) e o marcador avisa; se nem isso der, `None` e o
    /// registro nao e mais escrito nesta sessao.
    pub(super) fn read_record_in_disk(&mut self, e: &mut Effect) -> Option<Vec<Record>> {
        let base = self.base.clone()?;
        let fname = recovery_file(&base);
        match recovery_read(&fname) {
            Ok(r) => Some(r),
            Err(m) => match isolate_recovery(&fname, (self.clock)()) {
                Ok(p) => {
                    self.corrupt = Some(t!("warning.recovery.isolated", p.display()));
                    e.warning = Some(m);
                    Some(Vec::new())
                }
                Err(m2) => {
                    self.blocked = true;
                    self.corrupt = Some(t!("warning.recovery.blocked", m, m2));
                    e.warning = self.corrupt.clone();
                    None
                }
            },
        }
    }

    /// Reescreve o registro: as notas desta sessao como estao agora, mais as
    /// entradas de outras sessoes como estavam (so as chaves desta sessao saem,
    /// e so depois de o texto estar no vault ou de o usuario descartar).
    pub(super) fn rewrite_recovery(&mut self, e: &mut Effect) {
        if self.finished || self.blocked {
            return;
        }
        let Some(base) = self.base.clone() else { return };
        let Some(disk) = self.read_record_in_disk(e) else { return };
        let mut records = self.records();
        for r in disk {
            if !self.my.contains(&r.key) && !records.iter().any(|x| x.key == r.key) {
                records.push(r);
            }
        }
        if let Err(m) = write_recovery(&recovery_file(&base), &records) {
            e.warning = Some(m);
        }
    }

    pub(super) fn send(&mut self, vault: Vault, kind: DocKind, step: Step, flight: InFlight) -> Work {
        self.seq += 1;
        {
            let mut g = lock(&self.queue);
            g.valid = self.seq;
            g.done = None;
        }
        self.in_flight = Some((self.seq, flight));
        Work {
            seq: self.seq,
            vault,
            kind,
            step,
            now: (self.clock)(),
            queue: Arc::clone(&self.queue),
            delay: self.delay.clone(),
        }
    }

    /// Mensagem de quando um item termina.
    pub(super) fn finished(&self, st: &AppState, it: Item, from_textbox: bool, e: &mut Effect) {
        if it.erase {
            return;
        }
        let id = it.id.clone().unwrap_or_default();
        let other_vault = !st.vault.as_ref().is_some_and(|v| v.root == it.vault.root);
        let created = from_textbox && it.created.is_some();
        e.confirmation = Some(match (other_vault, created, it.kind) {
            (true, _, _) => t!("confirmation.quick_note.salva_in_the_vault_previous", id),
            (false, true, DocKind::Note) => t!("confirmation.quick_note.created_quick_note", id),
            (false, true, DocKind::Task) => t!("confirmation.quick_note.created_task", id),
            (false, false, DocKind::Note) => t!("confirmation.quick_note.quick_note_salva", id),
            (false, false, DocKind::Task) => t!("confirmation.quick_note.task_salva", id),
        });
        if from_textbox {
            e.clear_textbox = true;
            e.created = it.created;
        }
    }

    /// O proximo passo (no maximo um trabalho em voo), ou quando olhar de novo.
    pub(super) fn next(&mut self, st: &mut AppState, current_time: Instant) -> Effect {
        let mut e = Effect::default();
        if let Some(t) = self.dirty_recovery {
            let debounce = rhythm_wait(self.rhythm);
            if current_time >= t + debounce {
                self.dirty_recovery = None;
                self.rewrite_recovery(&mut e);
            } else {
                e.tick_at(t + debounce - current_time);
            }
        }
        if self.in_flight.is_some() {
            return e;
        }
        if self.paused {
            if self.item.is_some() || !self.leaving.is_empty() || !self.resumed.is_empty() || self.load.is_some() {
                e.tick_at(RETRY);
            }
            return e;
        }
        let mut i = 0;
        while i < self.leaving.len() {
            match menu_action(&mut self.leaving[i], current_time, self.rhythm) {
                MenuAction::Work(p) => {
                    let it = &mut self.leaving[i];
                    annotate_in_flight(it, &p);
                    let (v, t, s) = (it.vault.clone(), it.kind, it.serial);
                    e.work = Some(self.send(v, t, p, InFlight::Item(s)));
                    return e;
                }
                MenuAction::Finished => {
                    let it = self.leaving.remove(i).expect("existe");
                    let key = it.key.is_some();
                    self.finished(st, it, false, &mut e);
                    if key {
                        self.rewrite_recovery(&mut e);
                    }
                }
                MenuAction::Wait(d) => {
                    e.tick_at(d);
                    i += 1;
                }
                MenuAction::Nothing => i += 1,
            }
        }
        let rhythm = self.rhythm;
        if let Some(it) = self.item.as_mut() {
            match menu_action(it, current_time, rhythm) {
                MenuAction::Work(p) => {
                    annotate_in_flight(it, &p);
                    let (v, t, s) = (it.vault.clone(), it.kind, it.serial);
                    e.work = Some(self.send(v, t, p, InFlight::Item(s)));
                    return e;
                }
                MenuAction::Finished => {
                    let it = self.item.take().expect("existe");
                    let key = it.key.is_some();
                    self.finished(st, it, true, &mut e);
                    if key {
                        self.rewrite_recovery(&mut e);
                    }
                }
                MenuAction::Wait(d) => e.tick_at(d),
                MenuAction::Nothing => {}
            }
        }
        if self.so_item {
            return e;
        }
        if !self.resumed.is_empty() {
            match self.resume_at.filter(|t| current_time < *t) {
                Some(t) => e.tick_at(t - current_time),
                None => {
                    self.resume_at = None;
                    let r = self.resumed.pop_front().expect("nao vazia");
                    let step = Step::Resume { old: r.old.clone() };
                    e.work = Some(self.send(r.vault.clone(), r.kind, step, InFlight::Resumed(r)));
                    return e;
                }
            }
        }
        if self.item.is_none() {
            if let Some(c) = self.load.take() {
                let step = Step::Load { id: c.id.clone() };
                e.work = Some(self.send(c.vault.clone(), c.kind, step, InFlight::Load(c)));
            }
        }
        e
    }
}
