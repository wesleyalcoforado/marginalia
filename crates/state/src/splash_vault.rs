//! Tela de escolha do vault na partida, sem gpui (regra R4): quando ela
//! aparece, as linhas (os vaults conhecidos, mais recente primeiro, com o que
//! nao existe mais marcado) e o que escolher uma linha devolve. A selecao pelo
//! teclado e o "abrir pasta"/"criar vault" reusam [`crate::vault_picker`].

use std::path::{Path, PathBuf};

use crate::vault_picker;

/// A tela aparece quando nem a linha de comando (`--vault`) nem o ambiente
/// (`MARGINALIA_VAULT`) disseram o vault; um valor vazio conta como nao dito.
pub fn screen_needs(cli: Option<&Path>, env: Option<&Path>) -> bool {
    let said = |p: Option<&Path>| p.is_some_and(|p| !p.as_os_str().is_empty());
    !said(cli) && !said(env)
}

/// Uma linha da tela.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemSplash {
    pub file_path: PathBuf,
    /// Nome da pasta.
    pub label: String,
    /// A pasta ainda existe (um NAS desmontado ou uma pasta apagada nao).
    pub exists: bool,
}

/// Os vaults conhecidos na ordem dos dados do app; `exists` diz se a pasta
/// ainda esta la (a interface passa `Path::is_dir`; BLOQUEIA num NAS).
pub fn items(known_all: &[PathBuf], exists: impl Fn(&Path) -> bool) -> Vec<ItemSplash> {
    vault_picker::items(known_all, None)
        .into_iter()
        .map(|i| ItemSplash { exists: exists(&i.file_path), file_path: i.file_path, label: i.label })
        .collect()
}

/// O vault da linha `i`, se ela existe e a pasta ainda esta la.
pub fn choose(items: &[ItemSplash], i: usize) -> Option<PathBuf> {
    items.get(i).filter(|it| it.exists).map(|it| it.file_path.clone())
}

#[cfg(test)]
#[path = "tests/splash_vault.rs"]
mod tests;
