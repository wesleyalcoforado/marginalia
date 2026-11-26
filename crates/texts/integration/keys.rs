//! Regras do catalogo verificadas no codigo-fonte do workspace: toda chave usada
//! existe, nenhuma sobra, e nenhum texto cita tecla fora de um marcador.

use std::fs;
use std::path::{Path, PathBuf};

/// Pastas de codigo de producao que usam `t!`.
const CRATES: &[&str] = &["marginalia_core", "state", "ui"];
/// Pastas cujo codigo nao e de producao (testes, smoke, integracao).
const IGNORED: &[&str] = &["tests", "smoke", "integracao"];

const SYMBOL_KEYS: &[&str] = &["⌘", "⇧", "⌥", "⌃", "⌫", "⌦", "↵"];
const WORD_KEYS: &[&str] = &["Cmd", "Command", "Ctrl", "Control", "Shift", "Alt", "Option"];

fn eh_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// As chaves literais de todos os `t!("...")` do `code` (linhas de comentario
/// ignoradas). Primeiro argumento que nao e literal e erro.
fn used_keys(code: &str) -> Result<Vec<String>, String> {
    let mut keys = Vec::new();
    for (n, line) in code.lines().enumerate() {
        if line.trim_start().starts_with("//") {
            continue;
        }
        let mut de = 0;
        while let Some(i) = line[de..].find("t!(") {
            let pos = de + i;
            de = pos + 3;
            if line[..pos].chars().next_back().is_some_and(eh_ident) {
                continue; // assert!(, print!( ...
            }
            let rest = line[pos + 3..].trim_start();
            match rest.strip_prefix('"') {
                Some(r) => match r.find('"') {
                    Some(f) => keys.push(r[..f].to_string()),
                    None => return Err(format!("linha {}: chave sem fim", n + 1)),
                },
                None => return Err(format!("linha {}: a chave de t! tem de ser um literal: {}", n + 1, line.trim())),
            }
        }
    }
    Ok(keys)
}

/// Tecla escrita no texto, fora de um marcador `{shortcut:...}`.
fn key_tokens_outside_marker(value: &str) -> Vec<String> {
    let mut clean = String::new();
    let mut rest = value;
    while let Some(i) = rest.find("{shortcut:") {
        clean.push_str(&rest[..i]);
        rest = match rest[i..].find('}') {
            Some(f) => &rest[i + f + 1..],
            None => "",
        };
    }
    clean.push_str(rest);
    let mut found: Vec<String> = SYMBOL_KEYS.iter().filter(|s| clean.contains(**s)).map(|s| s.to_string()).collect();
    for word in clean.split(|c: char| !c.is_alphanumeric()) {
        if WORD_KEYS.contains(&word) {
            found.push(word.to_string());
        }
    }
    found
}

fn files_rs(dir: &Path, output: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            if !IGNORED.contains(&name.as_str()) {
                files_rs(&p, output);
            }
        } else if p.extension().is_some_and(|x| x == "rs") {
            output.push(p);
        }
    }
}

fn catalog() -> std::collections::HashMap<String, String> {
    let f = Path::new(env!("CARGO_MANIFEST_DIR")).join("languages/pt.txt");
    texts::parse(&fs::read_to_string(f).unwrap()).unwrap()
}

/// Chave -> arquivo:linha de cada uso de producao nos tres crates.
fn workspace_uses() -> Vec<(String, String)> {
    let mut uses = Vec::new();
    for c in CRATES {
        let mut files = Vec::new();
        files_rs(&Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(c).join("src"), &mut files);
        assert!(!files.is_empty(), "{c} sem arquivos");
        for p in files {
            let code = fs::read_to_string(&p).unwrap();
            match used_keys(&code) {
                Ok(keys) => uses.extend(keys.into_iter().map(|k| (k, p.display().to_string()))),
                Err(e) => panic!("{}: {e}", p.display()),
            }
        }
    }
    uses
}

#[test]
fn all_key_used_in_the_code_exists_in_the_catalog() {
    let cat = catalog();
    let missing: Vec<String> = workspace_uses().into_iter().filter(|(k, _)| !cat.contains_key(k)).map(|(k, f)| format!("{k} ({f})")).collect();
    assert!(missing.is_empty(), "chaves usadas e ausentes do catalogo:\n{}", missing.join("\n"));
}

#[test]
fn no_key_of_the_catalog_stays_no_use() {
    let used: std::collections::HashSet<String> = workspace_uses().into_iter().map(|(k, _)| k).collect();
    let mut remain: Vec<String> = catalog().into_keys().filter(|k| !used.contains(k)).collect();
    remain.sort();
    assert!(remain.is_empty(), "chaves do catalogo sem uso:\n{}", remain.join("\n"));
}

#[test]
fn none_text_cites_key_outside_of_marker() {
    let mut bad_ones: Vec<String> = catalog()
        .into_iter()
        .filter_map(|(k, v)| {
            let t = key_tokens_outside_marker(&v);
            (!t.is_empty()).then(|| format!("{k}: {t:?}"))
        })
        .collect();
    bad_ones.sort();
    assert!(bad_ones.is_empty(), "textos com tecla escrita a mao (use {{shortcut:id}}):\n{}", bad_ones.join("\n"));
}

#[test]
fn keys_used_finds_literals_and_ignores_assert_and_comment() {
    assert_eq!(used_keys("let a = t!(\"x.y\"); assert!(1)").unwrap(), vec!["x.y"]);
    assert_eq!(used_keys("texts::t!(\"a.b\", n = 1)").unwrap(), vec!["a.b"]);
    assert!(used_keys("// t!(\"fora.b\")\nlet x = 1;").unwrap().is_empty());
    assert!(used_keys("t!(var)").unwrap_err().contains("literal"));
}

#[test]
fn tokens_of_key_so_outside_of_marker() {
    assert!(key_tokens_outside_marker("abra {shortcut:abrir_fonte}").is_empty());
    assert_eq!(key_tokens_outside_marker("Cmd-O abre"), vec!["Cmd"]);
    assert_eq!(key_tokens_outside_marker("Notas (⌘1)"), vec!["⌘"]);
    assert!(key_tokens_outside_marker("Esc fecha; Enter confirma; Tab troca").is_empty());
    assert!(key_tokens_outside_marker("Commander nao e tecla").is_empty());
}

/// Textos de usuario que ja foram achados soltos no codigo e movidos ao catalogo:
/// nao podem voltar como literal (a varredura de chaves nao enxerga literais).
#[test]
fn literals_already_moved_not_return_to_the_code() {
    const FORBIDDEN: &[&str] = &[
        "\"atualizando...\"",
        "\"novo nome da fonte\"",
        "\"falha interna ao decodificar\"",
        "\"zip vazio\"",
    ];
    for c in CRATES {
        let mut files = Vec::new();
        files_rs(&Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(c).join("src"), &mut files);
        for p in files {
            let code = fs::read_to_string(&p).unwrap();
            for forbidden in FORBIDDEN {
                assert!(!code.contains(forbidden), "{}: literal {forbidden} deve vir do catalogo", p.display());
            }
        }
    }
}
