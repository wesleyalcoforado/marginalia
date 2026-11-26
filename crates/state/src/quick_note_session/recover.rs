//! Restaurar ao abrir e recriar uma nota perdida, a partir do registro de recuperacao.

use super::*;

impl QuickNoteSession {
    /// Ao abrir o app: tenta de novo o que o registro de recuperacao guarda. Um
    /// vault que nao esta acessivel fica no registro (o [`QuickNoteSession::marker`]
    /// avisa); cada nota sai do registro so quando o texto esta no vault.
    pub fn restore(&mut self, st: &mut AppState, current_time: Instant) -> Effect {
        let mut e = Effect::default();
        if self.finished {
            // Uma sessao terminada (flush estourado) recomeca do zero, como ao abrir o app.
            let delay = self.delay.take();
            *self = QuickNoteSession::with_clock(Arc::clone(&self.clock));
            self.delay = delay;
        }
        self.base = Some(st.app_data.base().to_path_buf());
        let Some(records) = self.read_record_in_disk(&mut e) else { return e };
        for r in records {
            let known = self.records.iter().any(|x| x.key == r.key)
                || self.leaving.iter().chain(self.item.iter()).any(|it| it.key.as_deref() == Some(r.key.as_str()));
            if known {
                continue;
            }
            self.my.insert(r.key.clone());
            let vault = Vault { root: r.vault.clone() };
            let mut ctx = r.image.clone().map(|image| Context {
                vault: vault.clone(),
                image,
                framing: r.framing.clone().unwrap_or_else(crate::viewer_math::default_framing),
                highlight: r.highlight.clone(),
            });
            // A pasta do vault (talvez no NAS) e conferida no trabalho, nao aqui.
            let kind = match kind_of(&r.kind) {
                Some(t) if r.id.is_some() || ctx.is_some() => t,
                _ => {
                    self.records.push(r);
                    continue;
                }
            };
            if let Some(c) = ctx.as_mut().filter(|_| r.id.is_none()) {
                // Cada grifo vai para uma nota so (o trabalho confere tambem o disco).
                match c.highlight.clone() {
                    Some(g) if self.used_highlights.contains(&g) || kind == DocKind::Task => c.highlight = None,
                    Some(g) => {
                        self.used_highlights.insert(g);
                    }
                    None => {}
                }
            }
            self.serial += 1;
            let mut it = Item::new_item(self.serial, vault, kind, ctx, r.text.clone(), current_time);
            it.new = false;
            it.id = r.id.clone();
            it.search = r.id.is_none();
            // Sempre comparar antes de gravar (sem `no_disk`: so se o disco ja tem o texto).
            it.expected = Some(r.no_disk.clone());
            it.end = true;
            it.key = Some(r.key.clone());
            self.leaving.push_back(it);
        }
        e.join(self.next(st, current_time));
        e
    }

    /// Acao do usuario para uma nota que sumiu do vault: cria uma nota nova
    /// (rascunho no quadro atual) com o texto dela. Nunca e automatico (regra 3).
    pub fn recreate_as_new_note(&mut self, st: &mut AppState, current_time: Instant) -> Effect {
        let mut e = Effect::default();
        let where_ = if self.item.as_ref().is_some_and(stopped) {
            Some(Where::Current)
        } else {
            // (uma nota com a tentativa rodando agora fica: o resultado ainda vai voltar)
            let in_flight = self.serial_in_flight();
            self.leaving
                .iter()
                .position(stopped)
                .or_else(|| self.leaving.iter().position(|it| vault_waiting(it) && Some(it.serial) != in_flight))
                .map(Where::Leaving)
        };
        if where_.is_none() && self.records.is_empty() {
            return e;
        }
        let ctx = match st.quick_note_context() {
            Ok(c) => c,
            Err(err) => {
                e.warning = Some(err.to_string());
                return e;
            }
        };
        // O texto e a chave do registro (o registro so sai quando o texto estiver na nota nova).
        let (text, key) = match where_ {
            Some(o) => {
                let old = match o {
                    Where::Current => self.item.take(),
                    Where::Leaving(i) => self.leaving.remove(i),
                }
                .expect("achado");
                if let Some(g) = old.ctx.as_ref().and_then(|c| c.highlight.as_ref()) {
                    self.used_highlights.remove(g);
                }
                (old.textbox, old.key)
            }
            None => {
                let r = self.records.remove(0);
                (r.text, Some(r.key))
            }
        };
        let kind = new_kind(&text).unwrap_or(DocKind::Note);
        let current = self.item.take();
        self.create_item(kind, ctx, text, current_time);
        if let Some(it) = self.item.as_mut() {
            it.key = key;
        }
        if let Some(a) = current {
            // A nota perdida veio do registro: a da caixa continua na caixa.
            let fresh = self.item.take().map(|mut n| {
                n.end = true;
                n
            });
            self.item = Some(a);
            self.leaving.extend(fresh);
        }
        e.join(self.next(st, current_time));
        e
    }
}
