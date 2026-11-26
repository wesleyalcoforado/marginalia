//! Resultado dos trabalhos, rodar a fila e o flush de saida.

use super::*;

impl QuickNoteSession {
    /// Entra com o resultado de um [`Work`] (na thread da interface).
    /// Resultado velho, ja usado ou cancelado: nada muda.
    pub fn conclude(&mut self, st: &mut AppState, f: Done, current_time: Instant) -> Effect {
        let mut e = Effect::default();
        let flight = match self.in_flight.take() {
            Some((seq, flight)) if seq == f.seq && !f.cancelled => flight,
            other => {
                self.in_flight = other;
                return e;
            }
        };
        {
            let mut g = lock(&self.queue);
            if g.done.as_ref().is_some_and(|x| x.seq == f.seq) {
                g.done = None;
            }
        }
        let same = st.vault.as_ref().is_some_and(|v| v.root == f.root_dir);
        for (a, n) in &f.renames {
            if same {
                st.switch_id(a, n);
                e.renamed.push((a.clone(), n.clone()));
            }
            // A carga pedida pelo id antigo (linha ainda velha na gaveta) segue o novo.
            if let Some(c) = self.load.as_mut().filter(|c| c.vault.root == f.root_dir && &c.id == a) {
                c.id = n.clone();
            }
        }
        if f.created.is_some() || !f.renames.is_empty() || f.erased {
            e.reload = true;
        }
        match flight {
            InFlight::Item(serial) => self.conclude_item(st, serial, f, current_time, &mut e),
            InFlight::Resumed(r) => {
                if f.lost {
                    // Ja terminada por outro caminho: sem aviso, sem nova tentativa.
                    e.reload = true;
                } else if let Some(m) = &f.error {
                    e.warning = Some(t!("warning.quick_note.finish_renaming", r.old, m));
                    self.resume_at = Some(current_time + RETRY_RESUMED);
                    self.resumed.push_back(r);
                }
            }
            InFlight::Load(c) => {
                if c.generation == self.generation && self.item.is_none() {
                    match (f.error, f.loaded, f.id) {
                        (None, Some((note_body, draft)), Some(id)) => {
                            self.serial += 1;
                            self.view_image = st.quick_note_image().cloned();
                            self.root_view = st.vault.as_ref().map(|v| v.root.clone());
                            let mut it = Item::new_item(self.serial, c.vault, c.kind, None, note_body.clone(), current_time);
                            it.new = false;
                            it.id = Some(id);
                            it.written = Some(note_body.clone());
                            it.since_dirty = None;
                            it.pinned = !draft;
                            self.item = Some(it);
                            e.loaded = Some(note_body);
                        }
                        (Some(m), _, _) => e.warning = Some(t!("warning.quick_note.open", c.id, m)),
                        _ => {}
                    }
                }
            }
        }
        e.join(self.next(st, current_time));
        e
    }

    pub(super) fn where_(&self, serial: u64) -> Option<Where> {
        if self.item.as_ref().is_some_and(|it| it.serial == serial) {
            return Some(Where::Current);
        }
        self.leaving.iter().position(|it| it.serial == serial).map(Where::Leaving)
    }

    pub(super) fn item_at(&mut self, where_: Where) -> &mut Item {
        match where_ {
            Where::Current => self.item.as_mut().expect("atual"),
            Where::Leaving(i) => &mut self.leaving[i],
        }
    }

    pub(super) fn remove(&mut self, where_: Where) -> Item {
        match where_ {
            Where::Current => self.item.take().expect("atual"),
            Where::Leaving(i) => self.leaving.remove(i).expect("saindo"),
        }
    }

    pub(super) fn conclude_item(&mut self, st: &mut AppState, serial: u64, f: Done, current_time: Instant, e: &mut Effect) {
        let Some(where_) = self.where_(serial) else { return };
        if f.erased {
            // Rascunho apagado: o grifo dele volta a ser o pendente.
            let it = self.remove(where_);
            if let Some(c) = &it.ctx {
                if let Some(g) = &c.highlight {
                    self.used_highlights.remove(g);
                    st.restore_highlight_pending(&c.image, g);
                }
            }
            if it.key.is_some() {
                self.rewrite_recovery(e);
            }
            return;
        }
        if let Some(Created::Note(p)) = &f.created {
            if let Some(g) = &p.highlight {
                st.linked_highlight(g);
            }
        }
        if f.no_highlight {
            let it = self.item_at(where_);
            if let Some(g) = it.ctx.as_mut().and_then(|c| c.highlight.take()) {
                self.used_highlights.remove(&g);
            }
        }
        let it = self.item_at(where_);
        let (id_before, written_before) = (it.id.clone(), it.written.clone());
        // O trabalho voltou: o que ele levava ja esta (ou nao estara) no disco.
        it.in_flight_bodies.clear();
        if let Some(c) = f.created {
            it.created = Some(c);
        }
        if let Some(id) = f.id {
            it.id = Some(id);
            it.search = false;
        }
        if f.conflict {
            it.conflict = true;
        }
        if f.written.is_some() && f.written != written_before {
            // O texto mudou no disco: as gavetas (rotulo da linha, etiquetas) o releem.
            e.reload = true;
        }
        if let Some(g) = f.written {
            // Adotado: a gravacao seguinte so vai por cima deste corpo (mudou por fora: conflito).
            it.expected = f.adopted.then(|| vec![g.clone()]);
            it.written = Some(g);
            it.urgent = false;
            if f.emptied {
                // Citado (ou ja nao e rascunho): ficou so com o corpo vazio.
                it.erase = false;
            }
            // Digitado enquanto gravava: a espera maxima recomeca agora.
            it.since_dirty = it.dirty().then_some(current_time);
        }
        if f.pinned {
            it.pinned = true;
            it.pin = false;
        }
        if let (Some(c), Some(id)) = (it.created.as_mut(), it.id.as_deref()) {
            with_id(c, id, it.pinned);
        }
        // O registro acompanha o id e o corpo em disco de cada nota dele (um
        // restaurar depois nao duplica nem compara com um corpo velho).
        let mut rewrite = it.key.is_some() && (it.id != id_before || it.written != written_before);
        let mut warning = None;
        match f.error {
            Some(m) => {
                it.lost |= f.lost;
                it.failures += 1;
                it.attempt_at = Some(current_time + wait(it.failures));
                warning = Some(if it.lost {
                    t!("warning.quick_note.vanished", it.id.as_deref().unwrap_or(""))
                } else {
                    t!("error.quick_note.write", m)
                });
                it.error = Some(m);
            }
            None if f.conflict => {
                warning = Some(t!("warning.quick_note.changed_in_the_disk", it.id.as_deref().unwrap_or("")));
            }
            None => {
                it.error = None;
                it.failures = 0;
                it.urgent = false;
            }
        }
        let key_needs = stopped(it) && it.key.is_none();
        if it.key.is_some() && it.safe() && !stopped(it) {
            // O texto esta no vault: sai do registro de recuperacao.
            it.key = None;
            rewrite = true;
        }
        let error = it.error.clone();
        if key_needs {
            let c = self.new_key();
            self.item_at(where_).key = Some(c);
            rewrite = true;
        }
        if rewrite {
            self.rewrite_recovery(e);
        }
        self.last_error = error;
        match warning {
            Some(a) if self.last_warning.as_deref() != Some(a.as_str()) => {
                self.last_warning = Some(a.clone());
                e.warning = Some(a);
            }
            Some(_) => {}
            None => self.last_warning = None,
        }
    }

    /// Roda os trabalhos de `e` (e os que vierem deles) aqui mesmo. BLOQUEIA:
    /// so para `submit_quick_note` e testes.
    pub fn run(&mut self, st: &mut AppState, mut e: Effect, current_time: Instant) -> Effect {
        let mut total = Effect::default();
        for _ in 0..64 {
            let Some(t) = e.work.take() else { break };
            total.join(e);
            e = self.conclude(st, t.execute(), current_time);
        }
        total.join(e);
        total
    }

    /// Sair do app / fechar a janela, em [`FLUSH_DEADLINE`] (a view passa o prazo).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn flush(&mut self, st: &mut AppState) -> Vec<String> {
        self.flush_with_deadline(st, FLUSH_DEADLINE)
    }

    /// Primeiro grava no registro de recuperacao (local, atomico) todo texto
    /// que ainda nao esta seguro no vault; depois, com renomeacao de fonte
    /// em andamento, para (nao disputa os arquivos com ela). Senao espera o
    /// trabalho que esta rodando (ou cancela o que nao comecou) e grava/fixa
    /// cada item, tudo dentro de `deadline` (cada trabalho numa thread). Prazo
    /// estourado: o registro fica (o app tenta de novo ao abrir) e a sessao
    /// esquece tudo. Devolve os avisos.
    pub fn flush_with_deadline(&mut self, st: &mut AppState, deadline: Duration) -> Vec<String> {
        if self.finished {
            // Um flush anterior estourou o prazo: o registro em disco e a unica copia.
            return Vec::new();
        }
        let end = Instant::now() + deadline;
        self.base = Some(st.app_data.base().to_path_buf());
        let mut warnings = Vec::new();
        if let Some(mut it) = self.item.take() {
            if !(it.new && it.id.is_none() && new_kind(&it.textbox).is_none()) {
                it.end = true;
                self.leaving.push_back(it);
            }
        }
        for i in 0..self.leaving.len() {
            let it = &mut self.leaving[i];
            it.end = true;
            it.attempt_at = None;
            if it.key.is_none() && !it.safe() {
                let c = self.new_key();
                self.leaving[i].key = Some(c);
            }
        }
        let mut e = Effect::default();
        self.rewrite_recovery(&mut e);
        warnings.extend(e.warning.take());
        if self.paused {
            warnings.push(t!("warning.quick_note.renaming_in_progress"));
            self.forget();
            return warnings;
        }
        self.so_item = true;
        let current_time = Instant::now();
        let mut e = Effect::default();
        if let Some(seq) = self.in_flight.as_ref().map(|(s, _)| *s) {
            let done = loop {
                let r = match self.queue.try_lock() {
                    Ok(g) => Some(g),
                    Err(TryLockError::Poisoned(p)) => Some(p.into_inner()),
                    Err(TryLockError::WouldBlock) => None,
                };
                if let Some(mut g) = r {
                    let f = g.done.take().filter(|f| f.seq == seq);
                    if f.is_none() {
                        // Nao comecou (e agora nao comeca mais): refeito abaixo.
                        g.valid = 0;
                    }
                    break Some(f);
                }
                if Instant::now() >= end {
                    break None;
                }
                std::thread::sleep(Duration::from_millis(10));
            };
            match done {
                Some(Some(f)) => e = self.conclude(st, f, current_time),
                Some(None) => self.in_flight = None,
                None => return self.overflowed(warnings, deadline),
            }
        }
        if e.work.is_none() {
            e = self.next(st, current_time);
        }
        while let Some(t) = e.work.take() {
            let seq = t.seq;
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(t.execute());
            });
            match rx.recv_timeout(end.saturating_duration_since(Instant::now())) {
                Ok(f) => e = self.conclude(st, f, Instant::now()),
                Err(_) => {
                    if let Ok(mut g) = self.queue.try_lock() {
                        if g.valid == seq {
                            g.valid = 0;
                        }
                    }
                    return self.overflowed(warnings, deadline);
                }
            }
        }
        self.so_item = false;
        let mut e = Effect::default();
        self.rewrite_recovery(&mut e);
        warnings.extend(e.warning);
        let n = self.records().len();
        if n > 0 {
            warnings.push(t!("warning.quick_note.not_written", n));
        }
        warnings
    }

    pub(super) fn overflowed(&mut self, mut warnings: Vec<String>, deadline: Duration) -> Vec<String> {
        warnings.push(t!("warning.quick_note.vault_not_respondeu", format!("{:.1}", deadline.as_secs_f32())));
        // O registro leva tambem o corpo do trabalho que ficou rodando (pode chegar depois).
        let mut e = Effect::default();
        self.rewrite_recovery(&mut e);
        warnings.extend(e.warning);
        self.forget();
        warnings
    }

    /// Fim da sessao (depois do registro de recuperacao gravado): nada mais
    /// reescreve o registro (outro `flush` o apagaria com a lista vazia).
    pub(super) fn forget(&mut self) {
        self.finished = true;
        self.item = None;
        self.leaving.clear();
        self.records.clear();
        self.in_flight = None;
        self.so_item = false;
    }
}
