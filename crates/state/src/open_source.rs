//! Abertura de fonte, sem gpui (regra R4): nome padrao, registro no vault e
//! nos dados do app, e construcao da `ImageSource` (pasta, zip direto, zip em
//! cache ou um arquivo avulso passado na linha de comando).
//!
//! `open` bloqueia (percorre a pasta no NAS ou copia o zip): a view a chama
//! no executor de fundo.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use marginalia_core::app_data::AppData;
use marginalia_core::highlights::HighlightsFile;
use marginalia_core::model::{Source, SourceKind};
use marginalia_core::source::{FolderSource, ImageEntry, ImageSource, ZipSource};
use marginalia_core::vault::{Result as CoreResult, Vault};
use marginalia_core::Error;
use texts::t;


/// Como abrir um zip (pastas sempre abrem direto).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZipMode {
    /// Le do caminho original (o NAS), entrada a entrada.
    Direct,
    /// Copia para o cache local (apagado ao fechar o app) e le da copia.
    Cache,
}

/// Abertura de fonte ja resolvida (nome -> caminho), pronta para rodar no
/// fundo com [`Opening::execute`] e depois entrar com `AppState::adopt_source`.
#[derive(Clone, Debug)]
pub struct Opening {
    pub name: String,
    pub file_path: PathBuf,
    pub mode: ZipMode,
    pub cache_dir: PathBuf,
    /// Vault ativo (para ler os grifos da fonte no fundo).
    pub vault: Option<Vault>,
}

impl Opening {
    /// Bloqueia (percorre a pasta / copia o zip).
    pub fn execute(&self) -> Result<Arc<dyn ImageSource>, String> {
        open(&self.file_path, self.mode, &self.cache_dir)
    }

    /// [`Opening::execute`] e os grifos da fonte, tudo no fundo. Bloqueia.
    /// Confere aqui (no fundo, nunca na thread da interface: um NAS morto pode
    /// prender um `stat`) se o caminho ainda existe: [`OpeningFailure::Absent`].
    pub fn execute_ready(&self) -> Result<Ready, OpeningFailure> {
        if !self.file_path.exists() {
            return Err(OpeningFailure::Absent);
        }
        let source = self.execute().map_err(OpeningFailure::Other)?;
        let highlights = self.vault.as_ref().map(|v| HighlightsFile::load(v, &self.name).map_err(|e| e.to_string()));
        Ok(Ready { source, highlights })
    }
}

/// Por que uma abertura no fundo falhou.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpeningFailure {
    /// O caminho guardado nao existe (NAS desmontado, zip movido).
    Absent,
    Other(String),
}

impl std::fmt::Display for OpeningFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpeningFailure::Absent => f.write_str(&t!("error.absent_path")),
            OpeningFailure::Other(m) => f.write_str(m),
        }
    }
}

/// Fonte aberta no fundo com os grifos dela ja lidos (`None`: sem vault).
pub struct Ready {
    pub source: Arc<dyn ImageSource>,
    pub highlights: Option<Result<HighlightsFile, String>>,
}

/// O que um caminho escolhido pelo usuario e.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Folder,
    Zip,
    /// Uma imagem/PDF avulso (linha de comando); nao vira fonte do vault.
    File,
}

/// Toca o disco (`is_dir`). `None` se o caminho nao existe.
pub fn kind_of(file_path: &Path) -> Option<Kind> {
    if file_path.is_dir() {
        Some(Kind::Folder)
    } else if !file_path.is_file() {
        None
    } else if e_zip(file_path) {
        Some(Kind::Zip)
    } else {
        Some(Kind::File)
    }
}

/// Pela extensao `.zip` (sem distinguir maiusculas); nao toca o disco.
pub fn e_zip(file_path: &Path) -> bool {
    file_path.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip"))
}

/// Nome sugerido: o nome da pasta, ou o do zip sem a extensao `.zip`.
pub fn default_name(file_path: &Path) -> String {
    let name = if e_zip(file_path) { file_path.file_stem() } else { file_path.file_name() };
    name.map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Fonte".to_string())
}

/// `base`, ou `base (2)`, `base (3)`... o primeiro que `busy` nao recusa.
pub fn free_name(base: &str, busy: impl Fn(&str) -> bool) -> String {
    if !busy(base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base} ({n})"))
        .find(|c| !busy(c))
        .expect("algum sufixo livre")
}

/// `name` ja e de OUTRA fonte deste vault (mesmo nome com outro caminho nesta
/// maquina). Mesmo nome sem caminho nesta maquina nao conflita: e a mesma
/// fonte vista de outro computador.
#[cfg_attr(not(test), allow(dead_code))] // a view usa `conflicts_at` com a lista lida no fundo
pub fn conflicts(vault: &Vault, app_data: &AppData, name: &str, file_path: &Path) -> CoreResult<bool> {
    Ok(conflicts_at(&vault.list_sources()?, app_data, &vault.root, name, file_path))
}

/// [`conflicts`] com a lista de fontes ja lida (no fundo); nao toca o disco.
pub fn conflicts_at(sources: &[Source], app_data: &AppData, root_dir: &Path, name: &str, file_path: &Path) -> bool {
    sources.iter().any(|f| f.name == name) && app_data.source_path(root_dir, name).is_some_and(|p| p != file_path)
}

/// Parte do registro que toca o vault: cria `sources/<name>.md` se `name` nao
/// esta em `sources` (lida no fundo). BLOQUEIA (vault, talvez no NAS).
pub fn register_in_vault(vault: &Vault, name: &str, kind: SourceKind, sources: &[Source]) -> Result<Record, String> {
    if sources.iter().any(|f| f.name == name) {
        return Ok(Record::AlreadyRegistered);
    }
    vault
        .create_source(name, kind)
        .map(|_| Record::New)
        .map_err(|e| t!("error.source.register", name = format!("{name:?}"), e = e))
}

/// Parte do registro que fica nos dados do app (pasta local): so na memoria.
/// Quem chama grava ([`AppState::write_path`](crate::app_state::AppState::gravar_caminho):
/// pela fila numerada de gravacoes).
pub fn store_path(app_data: &mut AppData, root_dir: &Path, name: &str, file_path: &Path) {
    app_data.set_source_path(root_dir, name, file_path.to_path_buf());
}

#[derive(Debug, PartialEq, Eq)]
pub enum Record {
    /// Fonte criada em `sources/` e caminho gravado nos dados do app.
    New,
    /// Ja existia com este nome; caminho gravado (ou confirmado).
    AlreadyRegistered,
    /// Nenhum vault aberto: nada registrado (o caminho vale so nesta sessao).
    #[cfg_attr(not(test), allow(dead_code))]
    NoVault,
}

/// Registra `nome -> caminho`: cria a fonte no vault (se nova) e grava o
/// caminho nos dados do app (e salva). Recusa nome de outra fonte (`conflicts`).
#[cfg_attr(not(test), allow(dead_code))] // a view usa `open_in_background` + `write_path`
pub fn register(
    vault: Option<&Vault>,
    app_data: &mut AppData,
    name: &str,
    kind: SourceKind,
    file_path: &Path,
) -> Result<Record, String> {
    let Some(vault) = vault else {
        return Ok(Record::NoVault);
    };
    let error = |e: Error| t!("error.source.register", name = format!("{name:?}"), e = e);
    let sources = vault.list_sources().map_err(error)?;
    if conflicts_at(&sources, app_data, &vault.root, name, file_path) {
        return Err(t!("error.source.ja_exists", name = format!("{name:?}")));
    }
    let r = register_in_vault(vault, name, kind, &sources)?;
    store_path(app_data, &vault.root, name, file_path);
    app_data.save().map_err(|e| t!("error.source.store_path", name = format!("{name:?}"), e = e))?;
    Ok(r)
}

/// Pelo nome, sem tocar o disco (o NAS pode estar lento): zip, imagem avulsa
/// (`None`: nao vira fonte do vault) ou pasta.
pub fn kind_by_extension(file_path: &Path) -> Option<SourceKind> {
    if e_zip(file_path) {
        return Some(SourceKind::Zip);
    }
    let ext = file_path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("jpg" | "jpeg" | "png" | "tif" | "tiff" | "pdf") => None,
        _ => Some(SourceKind::Folder),
    }
}

/// O que fazer com o que foi solto na janela (arrastar e soltar).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DropAction {
    /// Abrir pelo mesmo caminho do Cmd-O (inclui a pergunta direto/cache do zip).
    Open(PathBuf),
    /// Nada abre; a mensagem vai para o aviso da barra.
    Refuse(String),
}

/// Extensoes de arquivo avulso que o visualizador abre (imagem ou PDF).
const STANDALONE: [&str; 6] = ["jpg", "jpeg", "png", "tif", "tiff", "pdf"];

/// Classifica o que foi solto: exatamente UM item, pasta, zip, imagem
/// (jpg/png/tiff) ou PDF, que exista. Uma imagem/PDF avulso e aceito e abre
/// sozinho como fonte de uma imagem (`SingleFile`), igual ao Cmd-O, que ja
/// aceita escolher um arquivo (Ruling R16): nao abre a pasta que o contem.
/// Toca o disco (`is_dir`/`exists`) so para o item solto.
pub fn sort_drop(paths: &[PathBuf]) -> DropAction {
    let file_path = match paths {
        [] => return DropAction::Refuse(t!("warning.drop.nothing")),
        [c] => c,
        several => {
            return DropAction::Refuse(t!("warning.drop.several", several.len()))
        }
    };
    if !file_path.exists() {
        return DropAction::Refuse(t!("error.path_not_found", file_path.display()));
    }
    if file_path.is_dir() || e_zip(file_path) {
        return DropAction::Open(file_path.clone());
    }
    let ext = file_path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase());
    if ext.as_deref().is_some_and(|e| STANDALONE.contains(&e)) {
        return DropAction::Open(file_path.clone());
    }
    let name = file_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    DropAction::Refuse(t!("warning.drop.kind", name = format!("{name:?}")))
}


/// Fonte aberta no fundo por [`open_in_background`]; `record` diz o que foi feito
/// no vault (`None`: sem vault, arquivo avulso ou fonte sem imagens). O caminho
/// nos dados do app e gravado depois, na thread da interface (`write_path`).
pub struct Open {
    pub ready: Ready,
    pub record: Option<Result<Record, String>>,
}

/// Cmd-O / gaveta de fontes, tudo o que toca o vault ou a fonte: recusa um nome
/// que ja e de outra fonte, abre (percorre/copia), le os grifos e cria
/// `sources/<name>.md` se for nova. BLOQUEIA: rode no executor de fundo.
pub fn open_in_background(a: &Opening, app_data: &AppData) -> Result<Open, OpeningFailure> {
    let sources = a.vault.as_ref().map(|v| v.list_sources());
    if let (Some(v), Some(Ok(fs))) = (&a.vault, &sources) {
        if conflicts_at(fs, app_data, &v.root, &a.name, &a.file_path) {
            return Err(OpeningFailure::Other(t!("error.source.ja_exists", name = format!("{:?}", a.name))));
        }
    }
    let ready = a.execute_ready()?;
    let record = match (&a.vault, kind_by_extension(&a.file_path), &sources) {
        (Some(v), Some(t), Some(fs)) if !ready.source.entries().is_empty() => Some(match fs {
            Ok(fs) => register_in_vault(v, &a.name, t, fs),
            Err(e) => Err(t!("error.source.register", name = format!("{:?}", a.name), e = e)),
        }),
        _ => None,
    };
    Ok(Open { ready, record })
}

/// Constroi a fonte. Bloqueia: rode fora da thread da interface.
pub fn open(file_path: &Path, mode: ZipMode, cache_dir: &Path) -> Result<Arc<dyn ImageSource>, String> {
    let error = |e: Error| t!("error.source.open", file_path = file_path.display(), e = e);
    let source: Arc<dyn ImageSource> = match kind_of(file_path) {
        None => return Err(t!("error.path_not_found", file_path.display())),
        Some(Kind::Folder) => Arc::new(FolderSource::open(file_path).map_err(error)?),
        Some(Kind::Zip) => match mode {
            ZipMode::Direct => Arc::new(ZipSource::open_direct(file_path).map_err(error)?),
            ZipMode::Cache => Arc::new(ZipSource::open_cached(file_path, cache_dir).map_err(error)?),
        },
        Some(Kind::File) => Arc::new(SingleFile::new(file_path)?),
    };
    Ok(source)
}

/// Fonte de um unico arquivo (imagem/PDF passado na linha de comando).
pub struct SingleFile {
    file_path: PathBuf,
    entries: Vec<ImageEntry>,
}

impl SingleFile {
    pub fn new(file_path: &Path) -> Result<SingleFile, String> {
        let name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| t!("error.name_file_invalid", file_path.display()))?;
        Ok(SingleFile { file_path: file_path.to_path_buf(), entries: vec![ImageEntry { path: name.to_string() }] })
    }
}

impl ImageSource for SingleFile {
    fn entries(&self) -> &[ImageEntry] {
        &self.entries
    }

    fn read(&self, path: &str) -> CoreResult<Vec<u8>> {
        if self.entries.first().is_none_or(|e| e.path != path) {
            return Err(Error::InputNotFound(path.to_string()));
        }
        Ok(std::fs::read(&self.file_path)?)
    }
}

#[cfg(test)]
#[path = "tests/open_source.rs"]
mod tests;
