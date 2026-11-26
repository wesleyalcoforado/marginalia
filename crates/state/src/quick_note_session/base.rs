//! Tipos pequenos que o resto da sessao compartilha: relogio, atraso e o resultado de uma criacao.
use super::*;

/// Data e hora "da parede" para os ids (injetavel nos testes).
pub type Clock = Arc<dyn Fn() -> NaiveDateTime + Send + Sync>;
/// Chamado no inicio de cada trabalho, antes de tocar o vault (testes e smoke:
/// simula um vault lento).
pub type Delay = Arc<dyn Fn() + Send + Sync>;

/// O que a nota rapida criou.
#[derive(Clone, Debug, PartialEq)]
pub enum Created {
    Note(Note),
    Task(Task),
}
