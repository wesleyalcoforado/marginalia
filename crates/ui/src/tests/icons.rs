use super::*;
use gpui::AssetSource;
use std::collections::HashSet;

#[test]
fn assets_serves_whole_icon() {
    for i in ALL {
        let bytes = Assets.load(i.file()).unwrap().unwrap_or_else(|| panic!("{:?} sem bytes", i));
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(text.starts_with("<svg"), "{i:?}: nao comeca com <svg");
        assert!(text.trim_end().ends_with("</svg>"), "{i:?}: nao termina com </svg>");
        assert!(text.contains("currentColor"), "{i:?}: sem currentColor");
    }
}

#[test]
fn name_missing_returns_none() {
    assert!(Assets.load("icons/nao-existe.svg").unwrap().is_none());
    assert!(Assets.load("").unwrap().is_none());
}

#[test]
fn list_returns_os_icons() {
    let list: HashSet<String> = Assets.list("icons").unwrap().into_iter().map(|s| s.to_string()).collect();
    let expected: HashSet<String> = ALL.iter().map(|i| i.file().to_string()).collect();
    assert_eq!(list, expected);
    assert!(Assets.list("outra").unwrap().is_empty());
}

#[test]
fn variants_and_files_match() {
    // Caminhos distintos, um por variante.
    let file_paths: HashSet<&str> = ALL.iter().map(|i| i.file()).collect();
    assert_eq!(file_paths.len(), ALL.len());
    // Trocar vault tem icone proprio (nao reaproveita folder-open).
    assert_eq!(Icon::Vault.file(), "icons/database.svg");
    assert_ne!(Icon::Vault.file(), Icon::OpenFolder.file());
    // Todo arquivo da pasta de icones esta embutido por uma variante, e vice-versa.
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icons");
    let in_disk: HashSet<String> = std::fs::read_dir(folder)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".svg"))
        .collect();
    let used: HashSet<String> =
        ALL.iter().map(|i| i.file().strip_prefix("icons/").unwrap().to_string()).collect();
    assert_eq!(in_disk, used);
}
