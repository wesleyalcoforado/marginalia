use std::fmt;

use texts::t;
use thiserror::Error;

/// O texto de cada variante vem do catalogo (`error.*`); o `thiserror` so
/// cuida de `source` e `From`.
#[derive(Debug, Error)]
pub enum Error {
    NoFrontmatter,
    Yaml(#[from] serde_yaml::Error),
    DuplicateName(String),
    NotFound(String),
    StateInvalid(String),
    InvalidName(String),
    OutsideHighlight(String),
    ColorOutsideOfThePalette(String),
    Json(#[from] serde_json::Error),
    VersionNotSupported(u32),
    Zip(#[from] zip::result::ZipError),
    /// Zip sem nenhuma entrada (o `zip` so aceita texto `'static` no seu erro).
    ZipEmpty,
    InputNotFound(String),
    InputLarge(String),
    InvalidPath(String),
    Decode(#[from] crate::decode::DecodeError),
    Io(#[from] std::io::Error),
    /// Erro de leitura de um documento do vault, com o caminho do arquivo.
    Document(std::path::PathBuf, Box<Error>),
    /// Vault no formato antigo (pasta `pistas/` e nenhuma `notes/`): o app nao
    /// migra sozinho, quem migra e `scripts/migrar_pistas_para_notas.py`.
    VaultFormatOld(std::path::PathBuf),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Error::NoFrontmatter => t!("error.no_frontmatter"),
            Error::Yaml(e) => t!("error.yaml", e),
            Error::DuplicateName(n) => t!("error.duplicate_name", n),
            Error::NotFound(n) => t!("error.not_found", n),
            Error::StateInvalid(m) => t!("error.state_invalid", m),
            Error::InvalidName(n) => t!("error.invalid_name", format!("{n:?}")),
            Error::OutsideHighlight(m) => t!("error.outside_highlight", m),
            Error::ColorOutsideOfThePalette(c) => t!("error.color_outside_of_the_palette", c),
            Error::Json(e) => t!("error.json", e),
            Error::VersionNotSupported(v) => t!("error.version_not_supported", v),
            Error::Zip(e) => t!("error.zip", e),
            Error::ZipEmpty => t!("error.zip_empty"),
            Error::InputNotFound(n) => t!("error.input_not_found", n),
            Error::InputLarge(n) => t!("error.input_large", n),
            Error::InvalidPath(c) => t!("error.invalid_path", c),
            Error::Decode(e) => t!("error.decode", e),
            Error::Io(e) => t!("error.io", e),
            Error::Document(file_path, e) => t!("error.document", file_path.display(), e),
            Error::VaultFormatOld(p) => t!("error.vault.old_format", p.display()),
        };
        f.write_str(&text)
    }
}

#[cfg(test)]
#[path = "tests/error.rs"]
mod tests;
