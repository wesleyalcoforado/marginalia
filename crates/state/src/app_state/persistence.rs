//! Persistencia dos dados do app (posicao, caminhos) fora da thread da interface.

use super::*;

/// Instantaneo dos dados do app para gravar fora da thread da interface
/// ([`AppState::app_data_write`]). As gravacoes sao numeradas na ordem em que
/// foram montadas; uma mais velha que a ultima que chegou ao disco nao
/// sobrescreve o arquivo (a gravacao continua atomica: `AppData::save`).
pub struct WriteAppData {
    data: AppData,
    version: u64,
    written: Arc<Mutex<u64>>,
}

impl WriteAppData {
    /// Bloqueia (disco): rode no executor de fundo.
    pub fn execute(self) -> marginalia_core::vault::Result<()> {
        let mut written = self.written.lock().unwrap_or_else(|e| e.into_inner());
        if *written > self.version {
            // Uma gravacao mais nova (com estes ajustes ou mais novos) ja esta no disco.
            return Ok(());
        }
        self.data.save()?;
        *written = self.version;
        Ok(())
    }
}

/// A fila numerada: cada gravacao montada ganha o numero seguinte, e o numero
/// da ultima que chegou ao disco fica numa trava compartilhada (que tambem
/// serializa as gravacoes).
#[derive(Default)]
pub(super) struct WriteQueue {
    /// Numero da ultima gravacao montada aqui.
    version: u64,
    /// Numero da ultima gravacao que chegou ao disco (uma mais velha nao sobrescreve).
    written: Arc<Mutex<u64>>,
}

impl WriteQueue {
    /// Numera uma gravacao dos `data` como estao agora.
    pub(super) fn build(&mut self, data: AppData) -> WriteAppData {
        self.version += 1;
        WriteAppData { data, version: self.version, written: Arc::clone(&self.written) }
    }

    /// Quantas gravacoes foram montadas nesta sessao.
    pub(super) fn assembled(&self) -> u64 {
        self.version
    }

    /// Ha gravacao montada que ainda nao chegou ao disco (em voo ou que falhou).
    pub(super) fn pending(&self) -> bool {
        self.version > *self.written.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl AppState {
    /// Salva a ultima posicao aberta (se mudou). Chamado na troca de fonte e
    /// ao sair do app; a troca de vault salva os dados do app de qualquer jeito.
    pub fn save_position(&mut self) {
        if !self.dirty_position {
            return;
        }
        if let Err(e) = self.save_app_data() {
            self.warning = Some(t!("warning.remember_position", e));
        }
    }

    /// Instantaneo dos dados do app como estao agora, para gravar no fundo.
    pub fn app_data_write(&mut self) -> WriteAppData {
        self.app_data_queue.build(self.app_data.clone())
    }

    /// Quantas gravacoes dos dados do app foram montadas nesta sessao (fundo ou
    /// na hora): o smoke confere que uma sequencia de barra grava uma vez so.
    pub fn app_data_writes(&self) -> u64 {
        self.app_data_queue.assembled()
    }

    /// Ha dados do app que ainda nao chegaram ao disco: uma gravacao do fundo em
    /// voo ou uma que falhou (falha nao conta como gravada). Sair com isto
    /// verdadeiro chama [`AppState::save_app_data`].
    pub fn pending_app_data(&self) -> bool {
        self.app_data_queue.pending()
    }

    /// Guarda `nome -> caminho` nos dados do app e grava pela fila numerada
    /// (uma gravacao do fundo mais velha nao desfaz o caminho novo).
    pub fn write_path(&mut self, root_dir: &Path, name: &str, file_path: &Path) -> Result<(), String> {
        open_source::store_path(&mut self.app_data, root_dir, name, file_path);
        self.save_app_data().map_err(|e| t!("error.source.store_path", name = format!("{name:?}"), e = e))
    }

    /// Grava os dados do app agora (bloqueia: troca de fonte/vault, sair). Fica
    /// na mesma fila numerada das gravacoes do fundo: uma delas, mais velha, que
    /// termine depois nao desfaz esta.
    pub fn save_app_data(&mut self) -> marginalia_core::vault::Result<()> {
        self.app_data_write().execute()?;
        // O arquivo inteiro foi gravado, posicao inclusive.
        self.dirty_position = false;
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/persistence.rs"]
mod tests;
