use serde::{Deserialize, Serialize};
/// Campos do frontmatter que o Marginalia nao conhece (`tags:`, `aliases:`
/// escritos a mao ou por plugins). Sao lidos e regravados como estao, na ordem
/// do usuario, depois dos campos conhecidos.
///
/// Chaves reservadas (o Marginalia as le em notas e tarefas mesmo estando em
/// `extra`): `draft: true` marca um rascunho com id provisorio, que
/// `finish_id` renomeia (uma nota escrita a mao com `draft: true` e
/// tratada como rascunho); `renaming_from: <id>` marca a continuacao de uma
/// finalizacao interrompida.
pub type Extras = serde_yaml::Mapping;

/// Chaves de frontmatter que o proprio tipo grava (o teste
/// `fields_match_with_the_serialization` confere contra a serializacao).
pub trait Fields {
    const FIELDS: &'static [&'static str];
}

/// `extra` de outro documento pronto para ir para um `T`: sem as chaves que
/// `T` ja grava (senao a chave sairia duas vezes e o documento ficaria
/// ilegivel). Os campos do `T` vencem.
pub fn to_extras<T: Fields>(mut extra: Extras) -> Extras {
    for c in T::FIELDS {
        extra.remove(*c);
    }
    extra
}

/// Ponto numa imagem de uma fonte. `page` e 1-based.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageRef {
    #[serde(rename = "source", alias = "fonte")]
    pub source: String,
    #[serde(rename = "file", alias = "arquivo")]
    pub file: String,
    #[serde(rename = "page", alias = "pagina")]
    pub page: u32,
}

/// `zoom` 1.0 = ajustar a janela; `cx`, `cy` em 0..1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Framing {
    pub zoom: f32,
    pub cx: f32,
    pub cy: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StateTask {
    #[serde(rename = "open", alias = "aberta")]
    Open,
    #[serde(rename = "done", alias = "feita")]
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceKind {
    #[serde(rename = "zip")]
    Zip,
    #[serde(rename = "folder", alias = "pasta")]
    Folder,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    #[serde(rename = "image", alias = "imagem")]
    pub image: ImageRef,
    #[serde(rename = "framing", alias = "enquadramento")]
    pub framing: Framing,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "highlight", alias = "grifo")]
    pub highlight: Option<String>,
    /// So quando o usuario moveu a nota para o segundo registro de uma imagem
    /// de fronteira compartilhada; fora disso o pertencimento e pela posicao.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "record", alias = "registro")]
    pub record: Option<String>,
    #[serde(flatten, default, skip_serializing_if = "Extras::is_empty")]
    pub extra: Extras,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    #[serde(rename = "state", alias = "estado")]
    pub state: StateTask,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "image", alias = "imagem")]
    pub image: Option<ImageRef>,
    #[serde(default, rename = "note", alias = "nota", skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Override de registro (ver `Note::record`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "record", alias = "registro")]
    pub record: Option<String>,
    #[serde(flatten, default, skip_serializing_if = "Extras::is_empty")]
    pub extra: Extras,
}

/// Posicao de uma imagem na ordem de uma fonte, sem o nome da fonte (o
/// registro ja a tem). `page` e 1-based, como em `ImageRef`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    #[serde(rename = "file", alias = "arquivo")]
    pub file: String,
    #[serde(rename = "page", alias = "pagina")]
    pub page: u32,
}

impl Position {
    pub fn de(i: &ImageRef) -> Position {
        Position { file: i.file.clone(), page: i.page }
    }
}

/// Intervalo nomeado de imagens de uma fonte. `end` ausente = em aberto (vai
/// ate o inicio do proximo registro, ou o fim da fonte); nunca e gravado
/// por conta propria.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    #[serde(rename = "source", alias = "fonte")]
    pub source: String,
    #[serde(rename = "name", alias = "nome")]
    pub name: String,
    #[serde(rename = "start", alias = "inicio")]
    pub start: Position,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "end", alias = "fim")]
    pub end: Option<Position>,
    #[serde(flatten, default, skip_serializing_if = "Extras::is_empty")]
    pub extra: Extras,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Source {
    #[serde(rename = "name", alias = "nome")]
    pub name: String,
    #[serde(rename = "kind", alias = "tipo")]
    pub kind: SourceKind,
    #[serde(flatten, default, skip_serializing_if = "Extras::is_empty")]
    pub extra: Extras,
}

impl Fields for Note {
    const FIELDS: &'static [&'static str] = &["id", "image", "framing", "highlight", "record"];
}
impl Fields for Task {
    const FIELDS: &'static [&'static str] = &["id", "state", "image", "note", "record"];
}
impl Fields for Record {
    const FIELDS: &'static [&'static str] = &["id", "source", "name", "start", "end"];
}
impl Fields for Source {
    const FIELDS: &'static [&'static str] = &["name", "kind"];
}

#[cfg(test)]
#[path = "tests/model.rs"]
mod tests;
