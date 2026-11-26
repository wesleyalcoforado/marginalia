//! Caminho perdido, nova localizacao e renomear fonte, sem gpui (regra R4).
//!
//! Uma [`Pending`] e o pedido que a interface mostra quando uma fonte nao
//! pode ser lida: sem caminho nesta maquina, caminho que nao existe mais, ou
//! erro de E/S no meio da sessao (NAS desmontado). A interface oferece "Tentar
//! de novo" e "Apontar nova localizacao...". A nova localizacao so e aceita se
//! tiver as imagens da fonte ([`check`]); senao o mapa nome -> caminho nos
//! dados do app fica como estava. As referencias do vault nunca mudam.
//!
//! [`validate`] e [`validate_name`] decidem; quem grava e o `AppState`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use marginalia_core::model::SourceKind;
use marginalia_core::source::ImageSource;
use texts::t;

use crate::open_source::{self, Kind, ZipMode};
use crate::image_frame::Frame;

/// Quantas imagens citadas (notas, tarefas) entram na conferencia.
pub const MAX_CITED: usize = 200;
/// Maior nome de fonte em bytes (vira `<name>.md` e `<name>.json`; o macOS aceita 255).
pub const MAX_NAME: usize = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// Nenhum caminho guardado nesta maquina (fonte vinda de outro computador).
    NoPath,
    /// O caminho guardado nao existe (NAS montado em outro lugar, zip movido).
    NotFound,
    /// A leitura de uma imagem falhou por erro de E/S no meio da sessao.
    Unavailable,
}

/// O pedido de nova localizacao (ou de tentar de novo) para a fonte `name`.
#[derive(Clone, Debug, PartialEq)]
pub struct Pending {
    pub name: String,
    /// O caminho guardado, se havia um.
    pub file_path: Option<PathBuf>,
    pub reason: Reason,
    /// Mensagem do erro de E/S (so em `Unavailable`).
    pub detail: Option<String>,
    /// Quadro a abrir quando a fonte voltar (o que falhou, ou o da nota pedida).
    pub image_frame: Option<Frame>,
    /// Grifo a destacar nesse quadro.
    pub highlight: Option<String>,
    /// Nota que fica marcada como aberta.
    pub note: Option<String>,
}

impl Pending {
    pub fn fresh(name: &str, file_path: Option<PathBuf>, reason: Reason) -> Pending {
        Pending { name: name.to_string(), file_path, reason, detail: None, image_frame: None, highlight: None, note: None }
    }

    /// Titulo do aviso: "Fonte indisponivel: <nome>" e afins.
    pub fn title(&self) -> String {
        match self.reason {
            Reason::Unavailable => t!("warning.source.unavailable_title", self.name),
            Reason::NotFound => t!("warning.source.title_not_found", self.name),
            Reason::NoPath => t!("warning.source.title_no_path", self.name),
        }
    }

    /// Uma linha explicando o que aconteceu e o que fazer.
    pub fn explanation(&self) -> String {
        let where_ = self.file_path.as_ref().map(|c| c.display().to_string()).unwrap_or_default();
        match self.reason {
            Reason::Unavailable => t!("warning.source.unavailable_explanation",
                where_ = where_,
                detail = self.detail.clone().unwrap_or_else(|| t!("warning.source.reading_error"))
            ),
            Reason::NotFound => {
                t!("warning.source.explanation_not_found", where_ = where_)
            }
            Reason::NoPath => t!("warning.source.explanation_no_path"),
        }
    }
}

/// O que a nova localizacao precisa ter para ser a mesma fonte.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Expected {
    /// Tipo registrado no vault (zip ou pasta), se conhecido.
    pub kind: Option<SourceKind>,
    /// Imagens citadas no vault (`file` das notas/tarefas desta fonte).
    pub cited: Vec<String>,
    /// A imagem que vai ser reaberta: precisa existir.
    pub current: Option<String>,
}

/// Arquivos citados sem repeticao, em ordem, no maximo [`MAX_CITED`].
pub fn cited<'a>(files: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut v: Vec<String> = files.into_iter().filter(|a| !a.is_empty()).map(str::to_string).collect();
    v.sort();
    v.dedup();
    v.truncate(MAX_CITED);
    v
}

/// O tipo do caminho escolhido combina com o tipo registrado.
pub fn check_kind(name: &str, expected: Option<SourceKind>, found: Kind, file_path: &Path) -> Result<(), String> {
    match (expected, found) {
        (Some(SourceKind::Zip), Kind::Zip) | (Some(SourceKind::Folder), Kind::Folder) | (None, _) => Ok(()),
        (Some(SourceKind::Zip), _) => {
            Err(t!("error.relink.deve_be_zip", name = format!("{name:?}"), file_path = file_path.display()))
        }
        (Some(SourceKind::Folder), _) => {
            Err(t!("error.relink.deve_be_folder", name = format!("{name:?}"), file_path = file_path.display()))
        }
    }
}

/// Resultado de [`check`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Checked {
    /// Tem as imagens: pode gravar.
    Accepted,
    /// Nada no vault cita imagens desta fonte (nem ha imagem aberta) para
    /// conferir: a interface pergunta antes (o numero e o de imagens achadas).
    Confirm(usize),
}

/// Quantas das `n` citadas precisam existir: pelo menos metade (arredondada
/// para cima) e pelo menos uma.
pub fn min_cited(n: usize) -> usize {
    n.div_ceil(2).max(1)
}

/// A fonte aberta em `file_path` tem as imagens de `name`: alguma imagem, a
/// imagem a reabrir (se houver) e pelo menos metade das citadas no vault
/// (amostra de ate [`MAX_CITED`]). Sem nada para conferir: [`Checked::Confirm`].
pub fn check(src: &dyn ImageSource, e: &Expected, name: &str, file_path: &Path) -> Result<Checked, String> {
    let entries = src.entries();
    if entries.is_empty() {
        return Err(t!("error.relink.no_images", file_path = file_path.display(), name = format!("{name:?}")));
    }
    let has = |a: &str| entries.iter().any(|x| x.path == a);
    if let Some(a) = &e.current {
        if !has(a) {
            return Err(t!("error.relink.no_the_image_open", file_path = file_path.display(), a = a, name = format!("{name:?}")));
        }
    }
    if !e.cited.is_empty() {
        let found = e.cited.iter().filter(|a| has(a)).count();
        let min = min_cited(e.cited.len());
        if found < min {
            let missing = e.cited.iter().find(|a| !has(a)).map(String::as_str).unwrap_or("");
            return Err(t!("error.relink.poucas_images",
                file_path = file_path.display(),
                found = found,
                total = e.cited.len(),
                name = format!("{name:?}"),
                min = min,
                missing = missing
            ));
        }
        return Ok(Checked::Accepted);
    }
    if e.current.is_some() {
        return Ok(Checked::Accepted);
    }
    Ok(Checked::Confirm(entries.len()))
}

/// Texto da confirmacao quando nada pode ser conferido.
pub fn confirmation_question(name: &str, file_path: &Path, images: usize) -> String {
    t!("dialog.relink.confirm", file_path = file_path.display(), images = images, name = name)
}

/// Abre `file_path` e confere que e a fonte `name`. Devolve a fonte e, se nada
/// pode ser conferido, `Some(n)` (pedir confirmacao). BLOQUEIA (percorre a
/// pasta ou le o diretorio do zip no NAS): rode no executor de fundo.
pub fn validate(
    file_path: &Path,
    e: &Expected,
    name: &str,
    cache_dir: &Path,
) -> Result<(Arc<dyn ImageSource>, Option<usize>), String> {
    let kind = open_source::kind_of(file_path).ok_or_else(|| t!("error.path_not_found", file_path.display()))?;
    check_kind(name, e.kind, kind, file_path)?;
    // Direto: um zip so tem o diretorio central lido; o modo cache fica para a proxima abertura.
    let src = open_source::open(file_path, ZipMode::Direct, cache_dir)?;
    let confirm = match check(src.as_ref(), e, name, file_path)? {
        Checked::Accepted => None,
        Checked::Confirm(n) => Some(n),
    };
    Ok((src, confirm))
}

/// Um trabalho longo por vez (trocar de vault, apontar localizacao, renomear):
/// comecar outro enquanto um roda e recusado com um aviso.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Exclusive {
    current: Option<String>,
}

impl Exclusive {
    pub fn start(&mut self, label: impl Into<String>) -> Result<(), String> {
        if let Some(a) = &self.current {
            return Err(t!("warning.aguarde", a));
        }
        self.current = Some(label.into());
        Ok(())
    }

    pub fn finish(&mut self) {
        self.current = None;
    }

    pub fn busy(&self) -> Option<&str> {
        self.current.as_deref()
    }

    /// Abrir uma fonte (Cmd-O, gaveta, nota de outra fonte) durante uma
    /// renomeacao: recusa (o nome e os arquivos da fonte estao mudando).
    pub fn allows_open(&self) -> Result<(), String> {
        match &self.current {
            Some(a) if *a == renaming() => Err(t!("warning.aguarde", a)),
            _ => Ok(()),
        }
    }

    /// Gravar no vault (descartar, desfazer, promover, marcar tarefa, nova
    /// nota/tarefa, grifo) durante uma renomeacao: recusa com "aguarde" (a
    /// renomeacao esta reescrevendo os mesmos documentos e grifos).
    pub fn allows_write(&self) -> Result<(), String> {
        self.allows_open()
    }
}

/// Soltar uma fonte so vale com um vault aberto e sem renomeacao em curso
/// (`Exclusive::allows_open`); senao o aviso diz por que.
pub fn release_drop(vault_has: bool, exclusive: &Exclusive) -> Result<(), String> {
    if !vault_has {
        return Err(t!("warning.drop_no_vault"));
    }
    // Qualquer trabalho exclusivo (renomear, trocar de vault, nova localizacao):
    // o vault ou a fonte que o drop registraria estao mudando.
    match exclusive.busy() {
        Some(a) => Err(t!("warning.aguarde_drop", a)),
        None => Ok(()),
    }
}

/// Rotulo da renomeacao em [`Exclusive`].
pub fn renaming() -> String {
    t!("status.renaming")
}

/// Nome novo para a fonte `old`: sem espacos nas pontas, nao vazio, sem
/// `/ \ :` nem controle, sem comecar com ponto (nada de `..`), ate [`MAX_NAME`]
/// bytes, e diferente das outras fontes (sem distinguir maiusculas: o disco do
/// Mac nao distingue). Devolve o nome limpo.
pub fn validate_name(new: &str, old: &str, existing: &[String]) -> Result<String, String> {
    let n = new.trim();
    if n.is_empty() {
        return Err(t!("error.source_name.empty"));
    }
    if n.chars().any(|c| matches!(c, '/' | '\\' | ':') || c.is_control()) {
        return Err(t!("error.source_name.characters", n = format!("{n:?}")));
    }
    if n.starts_with('.') {
        return Err(t!("error.source_name.image_point", n = format!("{n:?}")));
    }
    if n.len() > MAX_NAME {
        return Err(t!("error.source_name.long", bytes = n.len(), max = MAX_NAME));
    }
    if n == old {
        return Err(t!("error.source_name.same"));
    }
    let down = n.to_lowercase();
    if down == old.to_lowercase() {
        return Err(t!("error.source_name.textbox"));
    }
    if existing.iter().any(|e| e != old && e.to_lowercase() == down) {
        return Err(t!("error.source_name.exists", n = format!("{n:?}")));
    }
    Ok(n.to_string())
}

#[cfg(test)]
#[path = "tests/relink.rs"]
mod tests;
