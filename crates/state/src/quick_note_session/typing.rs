//! Entradas do usuario na caixa: digitar, sair, finalizar, carregar, excluir, retomar.

use super::*;

impl QuickNoteSession {
    /// O texto da caixa agora e `text` (cada mudanca, fora de uma composicao).
    pub fn typed(&mut self, st: &mut AppState, text: &str, current_time: Instant) -> Effect {
        let mut e = Effect::default();
        if self.notice(st) {
            // A caixa ainda tinha o texto da nota que terminou: esta tecla nao e de
            // uma nota nova (a caixa limpa agora; a proxima tecla comeca a nova).
            e.clear_textbox = true;
            e.join(self.next(st, current_time));
            return e;
        }
        self.generation += 1;
        self.load = None;
        if text.trim().is_empty() && self.item.as_ref().is_some_and(stopped) {
            // Caixa apagada (Cmd+A, Delete) com uma nota perdida: o texto dela fica no
            // registro, solto da caixa; so o "Descartar" explicito abre mao dele.
            let it = self.item.take().expect("existe");
            self.leaving.push_back(it);
            e.join(self.next(st, current_time));
            return e;
        }
        let in_flight = self.in_flight_item();
        match self.item.as_mut() {
            Some(it) => {
                let clean = !it.dirty();
                it.textbox = text.to_string();
                it.last_key = current_time;
                if clean && it.dirty() {
                    it.since_dirty = Some(current_time);
                }
                let empty = it.new && new_kind(text).is_none();
                if stopped(it) {
                    if it.key.is_some() {
                        self.dirty_recovery = Some(current_time);
                    }
                } else if empty && !it.pinned {
                    if it.id.is_none() && !in_flight {
                        // Nunca chegou ao disco: esquece e devolve o grifo reservado.
                        let it = self.item.take().expect("existe");
                        if let Some(g) = it.ctx.and_then(|c| c.highlight) {
                            self.used_highlights.remove(&g);
                        }
                    } else {
                        // Rascunho no disco (ou sendo criado): apaga depois do que estiver rodando.
                        let mut it = self.item.take().expect("existe");
                        it.erase = true;
                        it.end = true;
                        self.leaving.push_back(it);
                    }
                }
            }
            None => {
                let Some(kind) = new_kind(text) else { return e };
                match st.quick_note_context() {
                    Ok(ctx) => {
                        self.create_item(kind, ctx, text.to_string(), current_time);
                    }
                    Err(err) => {
                        e.warning = Some(err.to_string());
                        return e;
                    }
                }
            }
        }
        e.join(self.next(st, current_time));
        e
    }

    pub(super) fn create_item(&mut self, kind: DocKind, mut ctx: Context, textbox: String, current_time: Instant) -> u64 {
        if kind == DocKind::Task || ctx.highlight.as_ref().is_some_and(|g| self.used_highlights.contains(g)) {
            ctx.highlight = None;
        }
        if kind == DocKind::Note && ctx.highlight.is_none() {
            // Apagou e redigitou antes de o rascunho ser apagado: o grifo reservado
            // por ele (desta mesma imagem) vai para a nota nova, nao volta a pendente.
            let donor = self.leaving.iter_mut().find(|it| {
                it.erase && it.ctx.as_ref().is_some_and(|c| c.image == ctx.image && c.highlight.is_some())
            });
            if let Some(c) = donor.and_then(|it| it.ctx.as_mut()) {
                ctx.highlight = c.highlight.take();
            }
        }
        if let Some(g) = &ctx.highlight {
            self.used_highlights.insert(g.clone());
        }
        self.serial += 1;
        self.view_image = Some(ctx.image.clone());
        self.root_view = Some(ctx.vault.root.clone());
        let vault = ctx.vault.clone();
        self.item = Some(Item::new_item(self.serial, vault, kind, Some(ctx), textbox, current_time));
        self.serial
    }

    /// Relogio da view: grava o que o debounce segurou, fixa o id depois de
    /// [`PIN_AFTER`] sem digitar e tenta de novo o que falhou.
    pub fn tick(&mut self, st: &mut AppState, current_time: Instant) -> Effect {
        self.observe(st, current_time)
    }

    /// Confere se a imagem, a fonte ou o vault mudaram (o item da caixa
    /// termina e a caixa limpa) e anda a fila.
    pub fn observe(&mut self, st: &mut AppState, current_time: Instant) -> Effect {
        let mut e = Effect { clear_textbox: self.notice(st), ..Effect::default() };
        e.join(self.next(st, current_time));
        e
    }

    /// A caixa perdeu o foco: grava e fixa o id agora (o item continua na caixa).
    pub fn textbox_exit(&mut self, st: &mut AppState, current_time: Instant) -> Effect {
        let mut e = Effect { clear_textbox: self.notice(st), ..Effect::default() };
        if let Some(it) = self.item.as_mut() {
            it.pin = true;
            it.urgent = true;
        }
        e.join(self.next(st, current_time));
        e
    }

    /// A imagem/pagina/fonte mudou: o mesmo que `observe` (a view chama ao
    /// pedir outro quadro; `observe`, em toda sincronizacao, tambem percebe).
    pub fn image_changed(&mut self, st: &mut AppState, current_time: Instant) -> Effect {
        self.observe(st, current_time)
    }

    /// Cmd+Enter: grava, fixa o id e, quando o disco tiver o texto da caixa,
    /// limpa a caixa ([`Effect::clear_textbox`], com [`Effect::created`] e a confirmacao).
    pub fn finish(&mut self, st: &mut AppState, current_time: Instant) -> Effect {
        let mut e = Effect { clear_textbox: self.notice(st), ..Effect::default() };
        if let Some(it) = self.item.as_mut() {
            if it.lost {
                e.warning = self.marker();
            } else {
                it.end = true;
            }
        }
        e.join(self.next(st, current_time));
        e
    }

    /// Pede o texto de `id` (nota, nota promovida ou tarefa) para editar na
    /// caixa. O item atual termina antes (gravado e fixado); o texto volta em
    /// [`Effect::loaded`], a menos que se digite algo antes de ele chegar.
    /// Editar um documento carregado nunca o renomeia (a nao ser um rascunho
    /// que ficou para tras, que e fixado uma vez como qualquer rascunho).
    /// A view chama pela acao Editar (tecla `e`) e pelo clique na linha da gaveta
    /// (depois de abrir a imagem dela: `MainView::click_line_note`).
    pub fn load(&mut self, st: &mut AppState, kind: DocKind, id: &str, current_time: Instant) -> Effect {
        let mut e = Effect { clear_textbox: self.notice(st), ..Effect::default() };
        if let Some(it) = self.item.as_ref().filter(|it| it.kind == kind && it.id.as_deref() == Some(id)) {
            e.loaded = Some(it.textbox.clone());
            return e;
        }
        let Some(vault) = st.vault.clone() else {
            e.warning = Some(crate::app_state::QuickNoteError::NoVault.to_string());
            return e;
        };
        if let Some(mut it) = self.item.take() {
            it.end = true;
            self.leaving.push_back(it);
            // O texto dela sai da caixa ja (nada se perde: ela termina gravada e fixada).
            e.clear_textbox = true;
        }
        self.generation += 1;
        self.load = Some(Load { vault, kind, id: id.to_string(), generation: self.generation });
        e.join(self.next(st, current_time));
        e
    }

    /// Exclui de vez o documento `id` (depois de a gaveta pedir e o usuario
    /// confirmar) e, se a sessao o edita (na caixa ou terminando), a sessao o
    /// larga SEM gravar nada: o texto nao volta ao disco, a caixa limpa e um
    /// aviso diz que foi excluido. Recusa (com aviso, nada muda) enquanto um
    /// trabalho da sessao mexe nele no fundo ("aguarde"), e quando o vault
    /// recusa (renomeacao do id em curso). Outros documentos nao tocam a sessao.
    pub fn delete(&mut self, st: &mut AppState, kind: DocKind, id: &str, current_time: Instant) -> Result<Effect, String> {
        let dropped = self.notice(st);
        if self.touching_at(id) {
            let e = t!("warning.quick_note.sendo_salva");
            st.warning = Some(e.clone());
            return Err(e);
        }
        st.delete_item_in(kind, id)?;
        let mut e = Effect { reload: true, clear_textbox: dropped, ..Effect::default() };
        let its = |it: &Item| it.kind == kind && it.id.as_deref() == Some(id);
        let key_had = self.item.iter().chain(self.leaving.iter()).any(|it| its(it) && it.key.is_some());
        // O grifo que o item reservou fica livre para a proxima nota.
        let loose: Vec<String> = self
            .item
            .iter()
            .chain(self.leaving.iter())
            .filter(|it| its(it))
            .filter_map(|it| it.ctx.as_ref().and_then(|c| c.highlight.clone()))
            .collect();
        for g in &loose {
            self.used_highlights.remove(g);
        }
        if self.item.as_ref().is_some_and(its) {
            self.item = None;
            e.clear_textbox = true;
            e.warning = Some(t!("warning.quick_note.deleted_in_editing", id));
        }
        self.leaving.retain(|it| !its(it));
        self.resumed.retain(|r| !(r.kind == kind && r.old == id));
        if self.load.as_ref().is_some_and(|c| c.kind == kind && c.id == id) {
            self.load = None;
        }
        if key_had {
            self.rewrite_recovery(&mut e);
        }
        e.join(self.next(st, current_time));
        Ok(e)
    }

    /// Um trabalho em voo da sessao toca `id` (gravacao do item, carga ou fixacao).
    pub(super) fn touching_at(&self, id: &str) -> bool {
        match &self.in_flight {
            None => false,
            Some((_, InFlight::Item(serial))) => {
                self.item.iter().chain(self.leaving.iter()).any(|it| it.serial == *serial && it.id.as_deref() == Some(id))
            }
            Some((_, InFlight::Resumed(r))) => r.old == id,
            Some((_, InFlight::Load(c))) => c.id == id,
        }
    }

    /// Termina a fixacao parada de `old` (achada na recarga das gavetas:
    /// `Lists::resume`) na fila da sessao, antes de qualquer edicao do documento novo.
    pub fn resume(&mut self, st: &mut AppState, kind: DocKind, old: &str, current_time: Instant) -> Effect {
        if let Some(vault) = st.vault.clone() {
            let same = |r: &Resumed| r.vault.root == vault.root && r.kind == kind && r.old == old;
            let ja = self.resumed.iter().any(same) || matches!(&self.in_flight, Some((_, InFlight::Resumed(r))) if same(r));
            // A propria sessao cuida de `old` (o item na caixa ou terminando,
            // ou um trabalho em voo nele): a fixacao dela termina a renomeacao.
            let from_session = self.touching_at(old)
                || self
                    .item
                    .iter()
                    .chain(self.leaving.iter())
                    .any(|it| it.kind == kind && it.vault.root == vault.root && it.id.as_deref() == Some(old));
            if !ja && !from_session {
                self.resumed.push_back(Resumed { vault, kind, old: old.to_string() });
            }
        }
        self.next(st, current_time)
    }
}
