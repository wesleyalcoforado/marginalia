use crate::Error;
use serde::{de::DeserializeOwned, Serialize};

/// Chaves reservadas gravadas por versoes antigas (em portugues), lidas como as novas.
const LEGACY_KEYS: [(&str, &str); 2] = [("rascunho:", "draft:"), ("renomeando_de:", "renaming_from:")];

fn legacy_keys(yaml: &str) -> String {
    yaml.split_inclusive('\n')
        .map(|l| match LEGACY_KEYS.iter().find(|(old, new)| l.starts_with(old) && !yaml.lines().any(|x| x.starts_with(new))) {
            Some((old, new)) => format!("{new}{}", &l[old.len()..]),
            None => l.to_string(),
        })
        .collect()
}

pub fn parse<T: DeserializeOwned>(doc: &str) -> Result<(T, String), Error> {
    let rest = doc
        .strip_prefix("---\r\n")
        .or_else(|| doc.strip_prefix("---\n"))
        .ok_or(Error::NoFrontmatter)?;
    let mut pos = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']) == "---" {
            let legacy = legacy_keys(&rest[..pos]);
            let yaml = legacy.as_str();
            let meta = match serde_yaml::from_str(yaml) {
                Ok(m) => m,
                // Chave de topo repetida (escrita a mao, ou por versoes antigas
                // do app): vale a primeira, como os campos do app vem primeiro.
                Err(e) => match no_keys_repeated(yaml) {
                    Some(clean) => serde_yaml::from_str(&clean).map_err(|_| e)?,
                    None => return Err(e.into()),
                },
            };
            return Ok((meta, rest[pos + line.len()..].to_string()));
        }
        pos += line.len();
    }
    Err(Error::NoFrontmatter)
}

/// Chave de uma linha de topo do yaml (`chave: ...` sem recuo), se for uma.
fn top_key(line: &str) -> Option<&str> {
    if line.starts_with([' ', '\t', '#', '-']) || line.trim().is_empty() {
        return None;
    }
    let k = line.split_once(':')?.0.trim();
    Some(k.trim_matches(['"', '\'']))
}

/// O yaml sem os blocos de chaves de topo repetidas (fica a primeira
/// ocorrencia de cada uma). `None` se nao havia repeticao.
fn no_keys_repeated(yaml: &str) -> Option<String> {
    let mut views = std::collections::HashSet::new();
    let mut out = String::new();
    let mut skipping = false;
    let mut changed = false;
    for line in yaml.split_inclusive('\n') {
        if let Some(k) = top_key(line) {
            skipping = !views.insert(k.to_string());
            changed |= skipping;
        }
        if !skipping {
            out.push_str(line);
        }
    }
    changed.then_some(out)
}

/// Byte de `doc` onde comeca o corpo (logo depois da linha `---` que fecha o
/// frontmatter), pela mesma regra de `parse`. `None` sem frontmatter.
pub(crate) fn body_start(doc: &str) -> Option<usize> {
    let ini = if doc.starts_with("---\r\n") {
        5
    } else if doc.starts_with("---\n") {
        4
    } else {
        return None;
    };
    let mut pos = ini;
    for line in doc[ini..].split_inclusive('\n') {
        pos += line.len();
        if line.trim_end_matches(['\r', '\n']) == "---" {
            return Some(pos);
        }
    }
    None
}

pub fn render<T: Serialize>(meta: &T, body: &str) -> String {
    let yaml = serde_yaml::to_string(meta).expect("metadado serializavel");
    format!("---\n{yaml}---\n{body}")
}

#[cfg(test)]
#[path = "tests/frontmatter.rs"]
mod tests;
