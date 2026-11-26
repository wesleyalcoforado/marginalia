//! Leitura do catalogo: `chave = texto`, uma entrada por linha.

use std::collections::HashMap;

/// Le o catalogo. A primeira ` = ` separa chave e texto; linhas vazias e `#...`
/// sao ignoradas; `\n` dentro do texto vira quebra de linha. Chave repetida ou
/// linha sem separador e erro, com o numero da linha.
pub fn parse(source: &str) -> Result<HashMap<String, String>, String> {
    let mut map = HashMap::new();
    for (i, line) in source.lines().enumerate() {
        let n = i + 1;
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let Some((key, text)) = l.split_once(" = ") else {
            return Err(format!("catalogo: linha {n} sem ` = `: {l:?}"));
        };
        let (key, text) = (key.trim(), text.trim().replace("\\n", "\n"));
        if map.insert(key.to_string(), text).is_some() {
            return Err(format!("catalogo: linha {n} repete a chave {key:?}"));
        }
    }
    Ok(map)
}

#[cfg(test)]
#[path = "tests/catalog.rs"]
mod tests;
