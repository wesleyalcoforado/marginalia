//! A geracao do quadro na tela: cada mudanca de imagem, rotacao ou valor de
//! ajuste abre uma geracao nova, e o que ficou em voo de uma geracao velha e
//! descartado (`AppState::receive` ignora a resposta; a tarefa de fundo olha
//! `effective` e desiste cedo).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

pub(super) struct Generation {
    current: u64,
    /// A mesma geracao, lida de fora da thread da interface pelas tarefas de fundo.
    effective: Arc<AtomicU64>,
}

impl Default for Generation {
    fn default() -> Self {
        Generation { current: 0, effective: Arc::new(AtomicU64::new(0)) }
    }
}

impl Generation {
    /// Abre uma geracao nova e a publica em `effective`; devolve o numero.
    pub(super) fn fresh(&mut self) -> u64 {
        self.current += 1;
        self.effective.store(self.current, Ordering::SeqCst);
        self.current
    }

    pub(super) fn current(&self) -> u64 {
        self.current
    }

    /// O marcador que vai no `Request`: as tarefas de fundo comparam com o numero do pedido.
    pub(super) fn effective(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.effective)
    }
}

#[cfg(test)]
#[path = "tests/generation.rs"]
mod tests;
