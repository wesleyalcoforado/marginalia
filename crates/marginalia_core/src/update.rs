//! Aviso de versao nova, sem rede e sem gpui (regra R4): ler a versao, ler o JSON da release do
//! GitHub, comparar, decidir se avisa e quando checar de novo. O HTTP fica em `state::update_check`.

use std::fmt;

/// Intervalo da checagem automatica (24 h), contado desde a ultima checagem bem-sucedida.
pub const INTERVAL_SECS: i64 = 86_400;

/// Tres numeros `X.Y.Z`; a ordem derivada compara numero a numero (major, minor, patch).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    /// `1.2.3` ou `v1.2.3`; qualquer outra forma (sufixo, dois numeros) e `None`.
    pub fn parse(s: &str) -> Option<Version> {
        let s = s.strip_prefix('v').unwrap_or(s);
        let mut parts = s.split('.');
        let mut next = || parts.next()?.parse::<u32>().ok();
        let v = Version { major: next()?, minor: next()?, patch: next()? };
        if parts.next().is_some() {
            return None;
        }
        Some(v)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// A versao deste executavel (a do `Cargo.toml` do crate que compila com este valor).
pub fn parse_own(cargo_version: &str) -> Version {
    Version::parse(cargo_version).unwrap_or(Version { major: 0, minor: 0, patch: 0 })
}

/// Uma release publicada: versao, pagina e notas (podem ser vazias).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    pub url: String,
    pub notes: String,
}

/// Le o JSON de `releases/latest`: `tag_name`, `html_url` (so `https://`) e `body` (opcional).
pub fn parse_release(json: &str) -> Option<Release> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let version = Version::parse(v.get("tag_name")?.as_str()?)?;
    let url = v.get("html_url")?.as_str()?;
    if !url.starts_with("https://") {
        return None;
    }
    let notes = v.get("body").and_then(|b| b.as_str()).unwrap_or("").trim().to_string();
    Some(Release { version, url: url.to_string(), notes })
}

/// O resultado de uma checagem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Check {
    New(Release),
    UpToDate,
    Failed,
}

/// Compara a release com a versao atual; `body == None` e falha de rede.
pub fn evaluate(current: Version, body: Option<&str>) -> Check {
    match body.and_then(parse_release) {
        None => Check::Failed,
        Some(r) if r.version > current => Check::New(r),
        Some(_) => Check::UpToDate,
    }
}

/// O que mostrar a quem usa.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    Show(Release),
    UpToDate,
    Failed,
    Nothing,
}

/// Automatica: so avisa de versao nova nao pulada, calada nos demais casos. Manual: sempre responde
/// (e avisa mesmo da versao pulada).
pub fn notice(check: Check, skipped: Option<Version>, manual: bool) -> Notice {
    match check {
        Check::New(r) if manual || skipped != Some(r.version) => Notice::Show(r),
        Check::New(_) => Notice::Nothing,
        Check::UpToDate if manual => Notice::UpToDate,
        Check::Failed if manual => Notice::Failed,
        _ => Notice::Nothing,
    }
}

/// Ja passou o intervalo desde a ultima checagem bem-sucedida? Relogio para tras conta como vencido.
pub fn due(last_check: Option<i64>, now: i64) -> bool {
    match last_check {
        None => true,
        Some(t) => t > now || now - t >= INTERVAL_SECS,
    }
}

#[cfg(test)]
#[path = "tests/update.rs"]
mod tests;
