//! Seletor de vault, sem gpui (regra R4): a lista dos vaults conhecidos (dos
//! dados do app, mais recente primeiro) com o ativo marcado, a selecao pelo
//! teclado, e a validacao de uma pasta escolhida ([`open`]).
//!
//! Um vault e uma pasta com `sources/`, `notes/`, `tasks/` ou
//! `highlights/`. "Abrir pasta..." recusa uma pasta que nao e vault
//! ([`VaultError::NotAVault`]: a interface pergunta se deve criar); "Criar vault
//! aqui" cria as subpastas (`Vault::open_or_create`). [`open`] BLOQUEIA (o
//! vault pode estar num NAS): rode no executor de fundo.

use std::path::{Path, PathBuf};

use marginalia_core::highlights::HighlightsFile;
use marginalia_core::vault::Vault;
use texts::t;

/// Atalho do seletor de vault, como se mostra na tela.
pub fn shortcut() -> String {
    t!("picker.shortcut")
}

const FOLDERS: [&str; 4] = ["fontes", "notas", "tarefas", "grifos"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VaultError {
    DoesNotExist(PathBuf),
    NotAFolder(PathBuf),
    Unreadable(PathBuf, String),
    /// Pasta legivel sem nenhuma das subpastas de um vault.
    NotAVault(PathBuf),
    /// Nao foi possivel criar as subpastas.
    Create(PathBuf, String),
    /// Vault do formato antigo (`pistas/`): precisa do script de migracao.
    OldFormat(PathBuf),
}

impl std::fmt::Display for VaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VaultError::DoesNotExist(p) => write!(f, "{}", t!("error.vault.not_exists", p.display())),
            VaultError::NotAFolder(p) => write!(f, "{}", t!("error.vault.not_and_folder", p.display())),
            VaultError::Unreadable(p, e) => write!(f, "{}", t!("error.vault.unreadable", p.display(), e)),
            VaultError::NotAVault(p) => write!(f, "{}", t!("error.vault.not_and_vault", p.display())),
            VaultError::Create(p, e) => write!(f, "{}", t!("error.vault.create", p.display(), e)),
            VaultError::OldFormat(p) => write!(f, "{}", t!("error.vault.old_format", p.display())),
        }
    }
}

/// Confere `root` e abre o vault (criando as subpastas que faltarem). Uma
/// pasta que nao e vault so vira vault com `create`. BLOQUEIA.
pub fn open(root: &Path, create: bool) -> Result<Vault, VaultError> {
    let meta = match std::fs::metadata(root) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(VaultError::DoesNotExist(root.into())),
        Err(e) => return Err(VaultError::Unreadable(root.into(), e.to_string())),
    };
    if !meta.is_dir() {
        return Err(VaultError::NotAFolder(root.into()));
    }
    // Ler a pasta prova que ela e legivel (sem permissao, `read_dir` falha).
    if let Err(e) = std::fs::read_dir(root) {
        return Err(VaultError::Unreadable(root.into(), e.to_string()));
    }
    if Vault::old_format(root) {
        return Err(VaultError::OldFormat(root.into()));
    }
    let is_vault = FOLDERS.iter().any(|d| root.join(d).is_dir());
    if !is_vault && !create {
        return Err(VaultError::NotAVault(root.into()));
    }
    Vault::open_or_create(root).map_err(|e| VaultError::Create(root.into(), e.to_string()))
}

/// [`open`] mais os grifos da fonte aberta (`source`) no vault novo, para a
/// troca nao ler o vault na thread da interface. BLOQUEIA.
pub fn open_with_highlights(root: &Path, create: bool, source: Option<&str>) -> Result<Open, VaultError> {
    let vault = open(root, create)?;
    let highlights = source.map(|f| HighlightsFile::load(&vault, f).map_err(|e| e.to_string()));
    Ok(Open { vault, highlights })
}

/// Vault aberto no fundo, pronto para `AppState::adopt_vault`.
pub struct Open {
    pub vault: Vault,
    pub highlights: Option<Result<HighlightsFile, String>>,
}

/// Uma linha do seletor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemVault {
    pub file_path: PathBuf,
    /// Nome da pasta.
    pub label: String,
    pub active: bool,
}

/// Os vaults conhecidos (ordem dos dados do app: mais recente primeiro) com o
/// ativo marcado; o ativo entra primeiro se ainda nao for conhecido. Nao toca o disco.
pub fn items(known_all: &[PathBuf], active: Option<&Path>) -> Vec<ItemVault> {
    let item = |p: &Path| ItemVault {
        file_path: p.to_path_buf(),
        label: p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string()),
        // `Path ==` compara componentes: `/a/b/` e `/a/./b` sao `/a/b`.
        active: active.is_some_and(|a| a == p),
    };
    let mut v: Vec<ItemVault> = known_all.iter().map(|p| item(p)).collect();
    if let Some(a) = active {
        if !v.iter().any(|i| i.active) {
            v.insert(0, item(a));
        }
    }
    v
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerAction {
    Relocate(i32),
    /// Abrir o vault selecionado.
    Choose,
    /// "Abrir pasta..." (painel de pastas).
    OpenFolder,
    /// "Criar vault aqui" (painel de pastas; cria as subpastas).
    CreateHere,
    Close,
}

/// Teclas com o seletor aberto: setas, Enter, Esc, `o` (abrir pasta), `n` (criar).
pub fn picker_key(key: &str) -> Option<PickerAction> {
    use PickerAction::*;
    Some(match key {
        "up" => Relocate(-1),
        "down" => Relocate(1),
        "enter" => Choose,
        "escape" => Close,
        "o" => OpenFolder,
        "n" => CreateHere,
        _ => return None,
    })
}

/// O seletor na tela: aberto ou nao e a linha escolhida.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Picker {
    is_open: bool,
    selection: usize,
}

impl Picker {
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Abre com a selecao no vault ativo (ou no primeiro).
    pub fn open(&mut self, items: &[ItemVault]) {
        self.is_open = true;
        self.selection = items.iter().position(|i| i.active).unwrap_or(0);
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Linha escolhida, limitada a lista.
    pub fn selection(&self, n: usize) -> Option<usize> {
        (n > 0).then(|| self.selection.min(n - 1))
    }

    pub fn relocate(&mut self, delta: i32, n: usize) {
        if let Some(s) = self.selection(n) {
            self.selection = (s as i64 + delta as i64).clamp(0, n as i64 - 1) as usize;
        }
    }
}

#[cfg(test)]
#[path = "tests/vault_picker.rs"]
mod tests;
