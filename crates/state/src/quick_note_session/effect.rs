//! O [`Effect`]: o que a view faz depois de cada chamada da sessao.
use super::*;

/// O que a view faz depois de uma chamada da sessao, nesta ordem: trocar os
/// ids das gavetas, limpar a caixa, por o texto carregado, mostrar aviso e
/// confirmacao, recarregar as listas, rodar o trabalho no fundo e agendar o tick.
#[derive(Default)]
pub struct Effect {
    /// Rode no executor de fundo ([`Work::execute`]) e entregue o
    /// resultado a [`QuickNoteSession::conclude`].
    pub work: Option<Work>,
    /// Chame [`QuickNoteSession::tick`] daqui a tanto.
    pub tick: Option<Duration>,
    pub warning: Option<String>,
    pub confirmation: Option<String>,
    /// O item da caixa terminou (Cmd+Enter com o disco em dia; troca de
    /// imagem/fonte/vault; caixa esvaziada): limpe a caixa.
    pub clear_textbox: bool,
    /// O que o Cmd+Enter criou (id ja fixado).
    pub created: Option<Created>,
    /// Texto para por na caixa ([`QuickNoteSession::load`]).
    pub loaded: Option<String>,
    /// Ids que mudaram `(antigo, novo)`, no vault ativo.
    pub renamed: Vec<(String, String)>,
    /// Documentos novos, renomeados ou apagados: as gavetas precisam reler.
    pub reload: bool,
}

impl Effect {
    /// Soma `o` a este efeito (o mais novo vence nos campos unicos).
    pub fn join(&mut self, o: Effect) {
        if o.work.is_some() {
            self.work = o.work;
        }
        self.tick = match (self.tick, o.tick) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        if o.warning.is_some() {
            self.warning = o.warning;
        }
        if o.confirmation.is_some() {
            self.confirmation = o.confirmation;
        }
        self.clear_textbox |= o.clear_textbox;
        if o.created.is_some() {
            self.created = o.created;
        }
        if o.loaded.is_some() {
            self.loaded = o.loaded;
        }
        self.renamed.extend(o.renamed);
        self.reload |= o.reload;
    }

    pub(super) fn tick_at(&mut self, d: Duration) {
        self.tick = Some(self.tick.map_or(d, |t| t.min(d)));
    }
}
