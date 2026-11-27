//! A checagem de versao nova: o GET ao GitHub (bloqueante, para rodar no executor de fundo) e a
//! decisao de quando checar. A logica pura (JSON, comparacao, aviso) mora em `marginalia_core::update`.

use marginalia_core::app_data::AppData;
use marginalia_core::update::{self, Check, Notice, Version};
use std::io::Read;
use std::time::Duration;

/// A release publicada mais recente (o GitHub ignora rascunhos e pre-lancamentos neste endereco).
pub const API_URL: &str = "https://api.github.com/repos/wesleyalcoforado/marginalia/releases/latest";

/// Variavel de ambiente que troca o endereco (smoke e testes locais).
const ENV_URL: &str = "MARGINALIA_UPDATE_URL";

const TIMEOUT: Duration = Duration::from_secs(5);
const MAX_BODY: u64 = 1024 * 1024;

/// O endereco a consultar: o da variavel de ambiente, se definida e nao vazia, senao `API_URL`.
pub fn endpoint() -> String {
    match std::env::var(ENV_URL) {
        Ok(u) if !u.trim().is_empty() => u,
        _ => API_URL.to_string(),
    }
}

/// Um endereco https nunca pode ser redirecionado para http (o desvio entregaria uma release falsa).
fn https_only_for(url: &str) -> bool {
    url.starts_with("https://")
}

/// GET simples, sem cookie nem identificador (so o User-Agent com a versao do app, `app_version`).
/// `None` em qualquer erro, status diferente de 200 ou corpo maior que 1 MiB.
pub fn fetch(url: &str, app_version: &str) -> Option<String> {
    let agent = ureq::AgentBuilder::new().timeout(TIMEOUT).redirects(3).https_only(https_only_for(url)).build();
    let resp = agent
        .get(url)
        .set("User-Agent", &format!("Marginalia/{app_version}"))
        .set("Accept", "application/vnd.github+json")
        .call()
        .ok()?;
    if resp.status() != 200 {
        return None;
    }
    let mut body = String::new();
    resp.into_reader().take(MAX_BODY + 1).read_to_string(&mut body).ok()?;
    (body.len() as u64 <= MAX_BODY).then_some(body)
}

/// Checa agora (bloqueante). Devolve o resultado e o que mostrar; quem chama grava o instante da
/// checagem so quando o resultado nao e `Check::Failed`.
pub fn run(current: Version, skipped: Option<Version>, manual: bool) -> (Check, Notice) {
    let body = fetch(&endpoint(), &current.to_string());
    let check = update::evaluate(current, body.as_deref());
    let notice = update::notice(check.clone(), skipped, manual);
    (check, notice)
}

/// A checagem automatica deve rodar agora? Ligada, intervalo vencido e, no smoke, so com endereco
/// proprio (o smoke nunca vai a internet).
pub fn auto_enabled(app_data: &AppData, now: i64, smoke: bool) -> bool {
    if smoke && std::env::var(ENV_URL).map_or(true, |u| u.trim().is_empty()) {
        return false;
    }
    app_data.auto_update_check() && update::due(app_data.last_update_check(), now)
}

#[cfg(test)]
#[path = "tests/update_check.rs"]
mod tests;
