//! Fontes de imagem: uma pasta ou um zip (normalmente num NAS montado por SMB,
//! ou seja, apenas um caminho de arquivo).

use crate::vault::Result;
use crate::Error;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use zip::ZipArchive;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageEntry {
    /// Caminho relativo dentro da fonte, com `/` como separador.
    pub path: String,
}

pub trait ImageSource: Send + Sync {
    fn entries(&self) -> &[ImageEntry];
    /// Lê uma entrada. Só caminhos presentes em `entries()` são legíveis.
    fn read(&self, path: &str) -> Result<Vec<u8>>;
}

const EXTENSIONS: [&str; 6] = ["jpg", "jpeg", "png", "tif", "tiff", "pdf"];

/// Maior entrada de zip que se le (o tamanho declarado no cabecalho nao e confiavel).
const INPUT_LIMIT: u64 = 2 << 30;
/// Reserva inicial maxima ao ler uma entrada, qualquer que seja o tamanho declarado.
const RESERVE_MAX: usize = 64 << 20;

/// Le no maximo `limit` bytes de `r`; mais que isso e `InputLarge`. Reserva
/// so `min(declarado, RESERVA_MAX)`: um cabecalho forjado nao faz alocar 4 GB.
fn input_read(r: impl Read, declared: u64, limit: u64, path: &str) -> Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(declared.min(RESERVE_MAX as u64) as usize);
    r.take(limit + 1).read_to_end(&mut buf)?;
    if buf.len() as u64 > limit {
        return Err(Error::InputLarge(path.to_string()));
    }
    Ok(buf)
}

/// `path` é elegível se: sem componente vazio/`.`/`..`/oculto (inclui `.DS_Store`,
/// `._x.jpg`), fora de `__MACOSX`, e com extensão de imagem/PDF.
fn eligible(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') {
        return false;
    }
    if path
        .split('/')
        .any(|c| c.is_empty() || c.starts_with('.') || c == "__MACOSX")
    {
        return false;
    }
    match path.rsplit_once('.') {
        Some((_, ext)) => EXTENSIONS.iter().any(|e| ext.eq_ignore_ascii_case(e)),
        None => false,
    }
}

/// Ordem das entradas de uma fonte (numeros naturais, sem diferenciar caixa,
/// desempate pelo texto): a mesma que os registros usam para comparar posicoes.
pub fn compare_paths(a: &str, b: &str) -> std::cmp::Ordering {
    natord::compare_ignore_case(a, b).then_with(|| a.cmp(b))
}

fn sorts(paths: &mut [String]) {
    paths.sort_by(|a, b| compare_paths(a, b));
}

fn entries_to(paths: Vec<String>) -> Vec<ImageEntry> {
    paths.into_iter().map(|path| ImageEntry { path }).collect()
}

pub struct FolderSource {
    root: PathBuf,
    entries: Vec<ImageEntry>,
}

impl FolderSource {
    pub fn open(root: &Path) -> Result<FolderSource> {
        if !root.is_dir() {
            return Err(Error::NotFound(root.display().to_string()));
        }
        let mut paths = Vec::new();
        walks(root, "", &mut paths)?;
        sorts(&mut paths);
        Ok(FolderSource {
            root: root.to_path_buf(),
            entries: entries_to(paths),
        })
    }
}

fn walks(dir: &Path, prefix: &str, out: &mut Vec<String>) -> Result<()> {
    for ent in std::fs::read_dir(dir)? {
        let ent = ent?;
        let Ok(name) = ent.file_name().into_string() else {
            continue; // nome nao-UTF8
        };
        if name.starts_with('.') || name == "__MACOSX" {
            continue;
        }
        let rel = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        let kind = ent.file_type()?;
        if kind.is_dir() {
            walks(&ent.path(), &rel, out)?;
        } else if kind.is_file() && eligible(&rel) {
            out.push(rel);
        }
    }
    Ok(())
}

impl ImageSource for FolderSource {
    fn entries(&self) -> &[ImageEntry] {
        &self.entries
    }

    fn read(&self, path: &str) -> Result<Vec<u8>> {
        if !self.entries.iter().any(|e| e.path == path) {
            return Err(Error::InputNotFound(path.to_string()));
        }
        Ok(std::fs::read(self.root.join(path))?)
    }
}

pub struct ZipSource {
    archive: Mutex<ZipArchive<File>>,
    entries: Vec<ImageEntry>,
    indices: HashMap<String, usize>,
}

/// Nome da entrada: os bytes crus se forem UTF-8 valido (zips de macOS/Linux
/// que omitem a flag UTF-8); senao o nome decodificado pelo crate, que para
/// entradas sem a flag (bit 11) e CP437 (palpite: zips CP850 podem exibir
/// glifos estranhos, mas continuam listados e legiveis). So pula a entrada se o
/// nome contiver U+FFFD (flag UTF-8 marcada com bytes realmente invalidos).
fn input_name(f: &zip::read::ZipFile<'_>) -> Option<String> {
    let name = match std::str::from_utf8(f.name_raw()) {
        Ok(s) => s.to_string(),
        Err(_) => f.name().to_string(),
    };
    if name.contains('\u{FFFD}') {
        None
    } else {
        Some(name)
    }
}

impl ZipSource {
    /// Abre direto do caminho dado (o NAS): guarda o diretorio central e le
    /// cada entrada sob demanda, sem carregar o arquivo em memoria.
    pub fn open_direct(zip: &Path) -> Result<ZipSource> {
        let mut archive = ZipArchive::new(File::open(zip)?)?;
        if archive.is_empty() {
            return Err(Error::ZipEmpty);
        }
        let mut indices: HashMap<String, usize> = HashMap::new();
        for i in 0..archive.len() {
            let f = archive.by_index_raw(i)?;
            if f.is_dir() {
                continue;
            }
            let Some(name) = input_name(&f) else {
                continue;
            };
            let name = name.as_str();
            if eligible(name) {
                // nomes duplicados: vale a primeira ocorrencia
                indices.entry(name.to_string()).or_insert(i);
            }
        }
        let mut paths: Vec<String> = indices.keys().cloned().collect();
        sorts(&mut paths);
        Ok(ZipSource {
            archive: Mutex::new(archive),
            entries: entries_to(paths),
            indices,
        })
    }

    /// Copia o zip para `cache_dir` (criado se faltar) e abre da copia.
    pub fn open_cached(zip: &Path, cache_dir: &Path) -> Result<ZipSource> {
        let name = zip
            .file_name()
            .ok_or_else(|| Error::InvalidPath(zip.display().to_string()))?;
        std::fs::create_dir_all(cache_dir)?;
        let destination = cache_dir.join(name);
        std::fs::copy(zip, &destination)?;
        ZipSource::open_direct(&destination)
    }
}

impl ImageSource for ZipSource {
    fn entries(&self) -> &[ImageEntry] {
        &self.entries
    }

    fn read(&self, path: &str) -> Result<Vec<u8>> {
        let idx = *self
            .indices
            .get(path)
            .ok_or_else(|| Error::InputNotFound(path.to_string()))?;
        let mut archive = self.archive.lock().unwrap_or_else(|e| e.into_inner());
        let f = archive.by_index(idx)?;
        let declared = f.size();
        input_read(f, declared, INPUT_LIMIT, path)
    }
}

/// Apaga a pasta de cache, best-effort: nunca entra em panico.
pub fn cleanup_cache(cache_dir: &Path) {
    let _ = std::fs::remove_dir_all(cache_dir);
}

#[cfg(test)]
#[path = "tests/source.rs"]
mod tests;
