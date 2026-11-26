//! Nota rapida, notas e tarefas ligadas ao vault.

use super::*;

/// O que uma nota nova leva do quadro atual ([`AppState::quick_note_context`]).
#[derive(Clone, Debug)]
pub struct Context {
    pub vault: Vault,
    pub image: ImageRef,
    pub framing: Framing,
    pub highlight: Option<String>,
}

impl AppState {
    /// O que uma nota nova da nota rapida leva do quadro atual: o vault, a
    /// imagem (com a pagina), o enquadramento e o grifo pendente desta imagem.
    pub fn quick_note_context(&self) -> Result<Context, QuickNoteError> {
        let vault = self.vault.clone().ok_or(QuickNoteError::NoVault)?;
        let image = self.current_image().ok_or(QuickNoteError::NoImage)?.clone();
        Ok(Context { vault, image, framing: self.fr.clone(), highlight: self.pending_highlight().map(|g| g.id().to_string()) })
    }

    /// Imagem atual (para a nota rapida perceber a troca de imagem).
    pub fn quick_note_image(&self) -> Option<&ImageRef> {
        self.current_image()
    }

    /// Abre a nota `id`: a imagem e a pagina dela, no enquadramento guardado,
    /// com o grifo destacado se ele ainda existir (sem grifo, ou com o grifo
    /// apagado, abre igual, sem destaque e sem erro). Nota de outra fonte
    /// resolve a fonte pelos dados do app; sem caminho ou caminho inexistente
    /// devolve o erro (nada muda).
    pub fn open_note(&mut self, id: &str) -> Result<OpenNote, SourceError> {
        let vault = self.vault.as_ref().ok_or_else(|| QuickNoteError::NoVault.to_string())?;
        let p = match vault.find_by_id(id) {
            Ok(Some(Found::Note(p, _))) => p,
            Ok(Some(_)) => return Err(t!("error.quick_note.not_and_quick_note", id = format!("{id:?}")).into()),
            Ok(None) => return Err(t!("error.quick_note.not_found", id = format!("{id:?}")).into()),
            Err(e) => return Err(t!("error.quick_note.read_file", id = format!("{id:?}"), e = e).into()),
        };
        self.open_point(p.image, Some(p.framing), p.highlight, Some(p.id))
    }

    /// Abre um ponto de imagem (de uma nota ou tarefa):
    /// `image` no enquadramento dado (sem: ajustada a janela), com `highlight`
    /// destacado. `note`: a nota que fica marcada como aberta.
    pub fn open_point(
        &mut self,
        image: ImageRef,
        framing: Option<Framing>,
        highlight: Option<String>,
        note: Option<String>,
    ) -> Result<OpenNote, SourceError> {
        let image_frame = Frame { image: image.clone(), framing };
        if self.source_has() && self.current.image.source == image.source {
            let request = self.ir_to(image_frame);
            self.annot.emphasis = highlight;
            self.note_open = note.map(|id| (id, image));
            self.warning = None;
            return Ok(OpenNote::ImageFrame(request));
        }
        let opening = match self.open_source(&image.source, ZipMode::Direct) {
            Ok(a) => a,
            Err(e) => {
                // Depois da nova localizacao, abre direto neste ponto.
                if let Some(p) = self.pending.as_mut().filter(|p| p.name == image.source) {
                    p.image_frame = Some(image_frame);
                    p.highlight = highlight;
                    p.note = note;
                }
                return Err(e);
            }
        };
        // Fica valendo quando `adopt_source_in_the` chegar a esta imagem.
        self.note_open = note.map(|id| (id, image));
        Ok(OpenNote::Source { opening, image_frame, highlight })
    }

    /// Nota aberta (pela gaveta ou `open_note`) se ainda e a imagem atual.
    pub fn note_open(&self) -> Option<&str> {
        let (id, img) = self.note_open.as_ref()?;
        (Some(img) == self.current_image()).then_some(id.as_str())
    }

    /// Clique numa linha da gaveta de notas: abre por `open_note` (relida do vault).
    pub fn open_line_note(&mut self, l: &NoteLine) -> Result<OpenNote, SourceError> {
        self.open_note(&l.id)
    }

    /// Clique numa tarefa: abre a imagem ligada a ela (ajustada a janela).
    pub fn open_line_task(&mut self, l: &LineTask) -> Result<OpenNote, SourceError> {
        match &l.image {
            Some(img) => self.open_point(img.clone(), None, None, None),
            None => Err(SourceError::Other(t!("error.task.no_image"))),
        }
    }

    pub(super) fn vault_or_error(&mut self) -> Result<Vault, String> {
        match &self.vault {
            Some(v) => Ok(v.clone()),
            None => {
                let e = QuickNoteError::NoVault.to_string();
                self.warning = Some(e.clone());
                Err(e)
            }
        }
    }

    /// Falha visivel: vai para a barra e volta como erro.
    pub(super) fn fail<T>(&mut self, e: String) -> Result<T, String> {
        self.warning = Some(e.clone());
        Err(e)
    }

    /// Como `delete_item` (so nos testes), com o tipo ja conhecido (a linha da
    /// gaveta sabe; um vault antigo pode ter o mesmo id em duas pastas).
    /// Depois de excluir limpa a nota aberta, o destaque, o pedido de nova
    /// localizacao que se referiam a ele. A recusa do
    /// vault (renomeacao do id em curso) vira aviso e o item fica.
    pub fn delete_item_in(&mut self, kind: DocKind, id: &str) -> Result<(), String> {
        let vault = self.vault_or_error()?;
        let highlight = match vault.find_by_id(id) {
            Ok(Some(Found::Note(p, _))) if kind == DocKind::Note => p.highlight,
            _ => None,
        };
        let r = match kind {
            DocKind::Note => vault.delete_note(id),
            DocKind::Task => vault.delete_task(id),
        };
        if let Err(e) = r {
            return self.fail(t!("error.quick_note.delete", id = format!("{id:?}"), e = e));
        }
        let was_open = self.note_open.as_ref().is_some_and(|(p, _)| p == id);
        if was_open {
            self.note_open = None;
        }
        if was_open || (highlight.is_some() && self.annot.emphasis == highlight) {
            self.annot.emphasis = None;
        }
        if let Some(p) = self.pending.as_mut() {
            if p.note.as_deref() == Some(id) {
                p.note = None;
            }
        }
        self.warning = None;
        self.confirmation = Some(match kind {
            DocKind::Note => t!("confirmation.deleted_quick_note", id),
            DocKind::Task => t!("confirmation.deleted_task", id),
        });
        Ok(())
    }

    /// Marca a tarefa `id` como feita (`feita = false` reabre).
    pub fn mark_task_done(&mut self, id: &str, done: bool) -> Result<Task, String> {
        let vault = self.vault_or_error()?;
        let state = if done { StateTask::Done } else { StateTask::Open };
        match vault.set_state_task(id, state) {
            Ok(t) => {
                self.warning = None;
                self.confirmation = Some(if done { t!("confirmation.done_task") } else { t!("confirmation.task_reaberta") });
                Ok(t)
            }
            Err(e) => self.fail(t!("error.task.mark", id, e)),
        }
    }
}

impl AppState {
    /// Clique numa linha de outra fonte: o texto de `id` vai para a caixa
    /// quando a fonte entrar (`resume_load_postponed`).
    pub fn postpone_load(&mut self, kind: DocKind, id: &str) {
        self.postponed_load = Some((kind, id.to_string()));
    }

    /// Ha um texto esperando a fonte entrar.
    pub fn has_load_postponed(&self) -> bool {
        self.postponed_load.is_some()
    }

    /// A fonte entrou: o texto que esperava (sai do estado).
    pub fn resume_load_postponed(&mut self) -> Option<(DocKind, String)> {
        self.postponed_load.take()
    }

    /// O item `old` do pedido adiado passou a se chamar `new`.
    pub fn load_postponed_follow_renaming(&mut self, old: &str, new: &str) {
        if let Some((_, id)) = self.postponed_load.as_mut().filter(|(_, id)| id == old) {
            *id = new.to_string();
        }
    }

    /// O texto pedido para depois da abertura nao vem mais: o aviso diz por
    /// que (nunca some calado), depois do aviso que ja estava na barra.
    pub fn drop_load_postponed(&mut self, by: &str) {
        if let Some((_, id)) = self.postponed_load.take() {
            let m = t!("warning.text_not_loaded", id, by);
            self.warning = Some(match self.warning.take() {
                Some(a) => t!("warning.join", a, m),
                None => m,
            });
        }
    }
}
