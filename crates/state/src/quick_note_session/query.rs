//! Perguntas sobre o estado da sessao, que a view faz sem mudar nada, e ajustes pequenos.

use super::*;

impl QuickNoteSession {
    /// A sessao nao esta gravando `id` agora; senao a mensagem da recusa (um
    /// arquivo, uma gravacao por vez).
    pub fn free(&self, id: &str) -> Result<(), String> {
        if self.safe(id) {
            Err(t!("warning.quick_note.sendo_salva"))
        } else {
            Ok(())
        }
    }

    /// Ha algo a gravar ou rodando (um item, uma retomada, uma carga). Um item
    /// gravado esperando so os 10 s para fixar o id nao conta; nota perdida
    /// (esperando o usuario) tambem nao.
    pub fn busy(&self) -> bool {
        // Uma nota do registro esperando o vault voltar (NAS) tambem nao: ja esta no registro.
        let pending = |it: &Item| {
            !stopped(it)
                && !(it.key.is_some() && it.error.is_some())
                && (it.id.is_none() || it.dirty() || it.end || it.urgent || it.error.is_some())
        };
        self.in_flight.is_some()
            || !self.resumed.is_empty()
            || self.load.is_some()
            || self.item.as_ref().is_some_and(pending)
            || self.leaving.iter().any(pending)
    }

    /// Um texto pedido por `load` ainda nao chegou (a view desliga a caixa:
    /// uma tecla agora comecaria outra nota no lugar do texto pedido).
    pub fn loading(&self) -> bool {
        self.load.is_some() || matches!(&self.in_flight, Some((_, InFlight::Load(_))))
    }

    /// Ha um trabalho rodando no fundo.
    pub fn in_flight(&self) -> bool {
        self.in_flight.is_some()
    }

    /// Ha um documento na caixa (criado ou por criar).
    pub fn has_item(&self) -> bool {
        self.item.is_some()
    }

    /// Id do documento da caixa.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn id_current(&self) -> Option<&str> {
        self.item.as_ref().and_then(|it| it.id.as_deref())
    }

    /// A sessao segura `id`: um item dele com trabalho rodando, texto ainda nao
    /// gravado, rascunho a apagar, id ainda por fixar ou terminando; a carga do
    /// texto dele pedida; ou uma fixacao parada dele por retomar.
    /// Promover/descartar/marcar agora disputaria o mesmo arquivo (ou o
    /// renomearia por baixo da sessao).
    pub fn safe(&self, id: &str) -> bool {
        let item = self.item.iter().chain(self.leaving.iter()).any(|it| {
            it.id.as_deref() == Some(id) && (self.in_flight.is_some() || it.dirty() || it.erase || it.end || !it.pinned)
        });
        let load = self.load.iter().any(|c| c.id == id)
            || matches!(&self.in_flight, Some((_, InFlight::Load(c))) if c.id == id);
        let resumed = self.resumed.iter().any(|r| r.old == id)
            || matches!(&self.in_flight, Some((_, InFlight::Resumed(r))) if r.old == id);
        item || load || resumed
    }

    /// A fonte `old` virou `new`: um rascunho ainda por criar usa o nome novo.
    pub fn renamed_source(&mut self, root_dir: &Path, old: &str, new: &str) {
        let swap = |i: &mut ImageRef| {
            if i.source == old {
                i.source = new.to_string();
            }
        };
        // So as notas do vault renomeado (o ativo pode ter mudado no meio).
        for it in self.item.iter_mut().chain(self.leaving.iter_mut()).filter(|it| it.vault.root == root_dir) {
            if let Some(c) = it.ctx.as_mut() {
                swap(&mut c.image);
            }
        }
        if self.root_view.as_deref() == Some(root_dir) {
            if let Some(i) = self.view_image.as_mut() {
                swap(i);
            }
        }
    }

    /// Suspende as gravacoes (renomeacao de fonte em andamento) ou as retoma.
    pub fn set_paused(&mut self, p: bool) {
        self.paused = p;
    }

    /// Texto do item da caixa que nao conseguiu ser gravado (a caixa deve mostra-lo).
    pub fn text_to_restore(&self) -> Option<&str> {
        self.item.as_ref().filter(|it| it.error.is_some() || it.lost).map(|it| it.textbox.as_str())
    }

    /// Marcador PERSISTENTE para a caixa (a view o mostra enquanto valer): nota
    /// da caixa que sumiu do vault, ou notas guardadas so no registro de recuperacao.
    pub fn marker(&self) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(c) = &self.corrupt {
            parts.push(c.clone());
        }
        if let Some(it) = self.item.as_ref().filter(|it| it.lost) {
            parts.push(t!("quick_note.marker.vanished", it.id.as_deref().unwrap_or("")));
        }
        let changed = self.leaving.iter().filter(|it| it.conflict).count();
        if changed > 0 {
            parts.push(t!("quick_note.marker.changed_in_the_disk", changed));
        }
        let n = self.records.len()
            + self.leaving.iter().filter(|it| it.lost || (!it.conflict && it.key.is_some() && it.error.is_some())).count();
        if n > 0 {
            parts.push(t!("quick_note.marker.so_in_the_recovery", n));
        }
        (!parts.is_empty()).then(|| parts.join(" · "))
    }

    /// Acao explicita do usuario ("Descartar"): abre mao das notas que esperam
    /// por ele (sumiram do vault ou mudaram no disco) e das que nao podem ser
    /// tentadas; saem do registro de recuperacao.
    pub fn discard_lost(&mut self) -> Effect {
        let mut e = Effect::default();
        let before = self.leaving.len() + self.item.iter().count() + self.records.len();
        if self.item.as_ref().is_some_and(stopped) {
            self.item = None;
        }
        let in_flight = self.serial_in_flight();
        let used = &mut self.used_highlights;
        self.leaving.retain(|it| {
            // Uma tentativa rodando agora: fica ate o resultado voltar.
            let stays = Some(it.serial) == in_flight || (!stopped(it) && !vault_waiting(it));
            if !stays {
                if let Some(g) = it.ctx.as_ref().and_then(|c| c.highlight.as_ref()) {
                    used.remove(g);
                }
            }
            stays
        });
        self.records.clear();
        if self.leaving.len() + self.item.iter().count() != before {
            self.rewrite_recovery(&mut e);
        }
        e
    }

    /// Ha uma nota que o usuario pode recriar ou descartar: sumida do vault,
    /// mudada no disco, esperando um vault que nao responde, ou um registro
    /// que nao pode ser tentado (tipo desconhecido, sem id nem imagem).
    pub fn can_recreate(&self) -> bool {
        !self.records.is_empty() || self.item.iter().chain(self.leaving.iter()).any(|it| stopped(it) || vault_waiting(it))
    }

    /// Ultimo erro de gravacao (para `submit_quick_note`).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Serial do item com um trabalho rodando agora.
    pub(super) fn serial_in_flight(&self) -> Option<u64> {
        match &self.in_flight {
            Some((_, InFlight::Item(s))) => Some(*s),
            _ => None,
        }
    }

    pub(super) fn in_flight_item(&self) -> bool {
        matches!(&self.in_flight, Some((_, InFlight::Item(s))) if self.item.as_ref().is_some_and(|it| it.serial == *s))
    }
}
