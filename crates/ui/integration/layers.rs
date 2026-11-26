//! Regra R4 verificada por teste: so os modulos da lista `WITH_GPUI` tocam o
//! gpui. O resto do app (estado, carga, notas, gavetas...) e Rust puro e
//! testavel sem janela. Para um modulo novo usar o gpui, ele entra na lista de
//! proposito; sem isso o teste falha e diz qual.
//!
//! O `marginalia_core` e o `state` nao podem depender do gpui de jeito nenhum (nem no Cargo.toml).

use std::fs;
use std::path::{Path, PathBuf};

/// Pastas inteiras que podem usar o gpui: o smoke dirige a janela de verdade e `main_view/` e a view raiz.
const FOLDERS_WITH_GPUI: &[&str] = &["smoke/", "main_view/"];

/// Modulos da interface que podem usar o gpui (nomes de arquivo em `src/`).
const WITH_GPUI: &[&str] = &["main.rs", "splash.rs", "viewer.rs", "quick_note_view.rs", "icons.rs"];

fn files_rs(dir: &Path, output: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            files_rs(&p, output);
        } else if p.extension().is_some_and(|x| x == "rs") {
            output.push(p);
        }
    }
}

/// Linhas de codigo que usam o crate `gpui` (`gpui::...` ou `use gpui`),
/// ignorando comentarios de linha. O nome solto (`like_gpui`) nao conta.
fn gpui_uses(text: &str) -> Vec<(usize, String)> {
    text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim_start().starts_with("//"))
        .filter(|(_, l)| l.contains("gpui::") || l.contains("use gpui") || l.contains("extern crate gpui"))
        .map(|(i, l)| (i + 1, l.trim().to_string()))
        .collect()
}

/// O modulo "dono" de um arquivo de testes: `tests/menus.rs` e dono `menus.rs` e
/// `folder/tests/x.rs` e dono `folder/x.rs` (o teste de um modulo herda a permissao
/// dele); o de `main.rs` e `tests/main.rs`. Os outros arquivos sao donos de si.
fn file_owner(rel: &str) -> String {
    if let Some(rest) = rel.strip_prefix("tests/") {
        return rest.to_string();
    }
    match rel.rsplit_once("/tests/") {
        Some((folder, name)) => format!("{folder}/{name}"),
        None => rel.to_string(),
    }
}

#[test]
fn so_os_modules_of_the_list_use_the_gpui() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    files_rs(&src, &mut files);
    assert!(!files.is_empty());

    let mut violations = Vec::new();
    for p in &files {
        let rel = p.strip_prefix(&src).unwrap().to_string_lossy().replace('\\', "/");
        let owner = file_owner(&rel);
        if WITH_GPUI.contains(&owner.as_str()) || FOLDERS_WITH_GPUI.iter().any(|p| owner.starts_with(p)) {
            continue;
        }
        for (line, text) in gpui_uses(&fs::read_to_string(p).unwrap()) {
            violations.push(format!("src/{rel}:{line}: {text}"));
        }
    }
    assert!(
        violations.is_empty(),
        "estes modulos nao podem usar o gpui (regra R4); ponha a logica num modulo sem gpui ou, se for \
         mesmo interface, acrescente o arquivo a COM_GPUI:\n{}",
        violations.join("\n")
    );
}

#[test]
fn the_list_with_gpui_so_has_files_that_exist_and_use_the_gpui() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for name in WITH_GPUI {
        let text = fs::read_to_string(src.join(name)).unwrap_or_else(|_| panic!("{name} da lista nao existe mais"));
        assert!(!gpui_uses(&text).is_empty(), "{name} nao usa mais o gpui: tire-o de COM_GPUI");
    }
}

#[test]
fn marginalia_core_and_state_not_depend_of_the_gpui() {
    for crate_ in ["marginalia_core", "state"] {
        let root_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(crate_);
        let cargo = fs::read_to_string(root_dir.join("Cargo.toml")).unwrap();
        assert!(!cargo.contains("gpui"), "{crate_}/Cargo.toml cita o gpui");

        let mut files = Vec::new();
        files_rs(&root_dir.join("src"), &mut files);
        assert!(!files.is_empty(), "{crate_} sem arquivos");
        let mut violations = Vec::new();
        for p in &files {
            for (line, text) in gpui_uses(&fs::read_to_string(p).unwrap()) {
                violations.push(format!("{}:{line}: {text}", p.display()));
            }
        }
        assert!(violations.is_empty(), "{crate_} nao pode usar o gpui:\n{}", violations.join("\n"));
    }
}

#[test]
fn marginalia_core_not_depends_of_the_state_nor_of_the_ui() {
    let root_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../marginalia_core");
    let cargo = fs::read_to_string(root_dir.join("Cargo.toml")).unwrap();
    assert!(!cargo.contains("state") && !cargo.contains("../ui"), "marginalia_core/Cargo.toml depende de camada acima");
    let cargo = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../state/Cargo.toml")).unwrap();
    assert!(!cargo.contains("../ui"), "state/Cargo.toml depende da ui");
}

#[test]
fn the_file_of_tests_inherits_the_owner_of_the_modulo() {
    assert_eq!(file_owner("tests/menus.rs"), "menus.rs");
    assert_eq!(file_owner("tests/main.rs"), "main.rs");
    assert_eq!(file_owner("pasta/tests/x.rs"), "pasta/x.rs");
    assert_eq!(file_owner("viewer.rs"), "viewer.rs");
}

#[test]
fn the_detector_finds_use_and_ignores_comment() {
    assert!(gpui_uses("// sem gpui aqui\n//! gpui na doc\nlet x = 1;").is_empty());
    let found = gpui_uses("fn a() {}\nuse gpui::px;\n");
    assert_eq!(found, vec![(2, "use gpui::px;".to_string())]);
}

#[test]
fn texts_not_depends_of_none_crate_of_the_project_nor_of_the_gpui() {
    let root_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../texts");
    let cargo = fs::read_to_string(root_dir.join("Cargo.toml")).unwrap();
    for forbidden in ["marginalia_core", "state", "ui", "gpui"] {
        assert!(!cargo.contains(forbidden), "texts/Cargo.toml cita {forbidden}");
    }
    let mut files = Vec::new();
    files_rs(&root_dir.join("src"), &mut files);
    assert!(!files.is_empty(), "textos sem arquivos");
    for p in &files {
        let uses = gpui_uses(&fs::read_to_string(p).unwrap());
        assert!(uses.is_empty(), "{} usa o gpui: {uses:?}", p.display());
    }
}
