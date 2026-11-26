use super::*;
use tempfile::tempdir;

#[test]
fn items_mark_the_active_and_the_put_in_the_list() {
    let k = vec![PathBuf::from("/v/familia"), PathBuf::from("/v/outro")];
    let l = items(&k, Some(Path::new("/v/outro/")));
    assert_eq!(l.iter().map(|i| (i.label.as_str(), i.active)).collect::<Vec<_>>(), [("familia", false), ("outro", true)]);
    let l = items(&k, Some(Path::new("/novo")));
    assert_eq!(l[0], ItemVault { file_path: "/novo".into(), label: "novo".into(), active: true });
    assert_eq!(l.len(), 3);
    assert!(items(&k, None).iter().all(|i| !i.active));
    assert!(items(&[], None).is_empty());
}

#[test]
fn selection_starts_in_the_active_and_stays_in_the_list() {
    let k = vec![PathBuf::from("/a"), PathBuf::from("/b"), PathBuf::from("/c")];
    let l = items(&k, Some(Path::new("/b")));
    let mut s = Picker::default();
    assert!(!s.is_open());
    s.open(&l);
    assert!(s.is_open());
    assert_eq!(s.selection(l.len()), Some(1));
    s.relocate(5, l.len());
    assert_eq!(s.selection(l.len()), Some(2));
    s.relocate(-9, l.len());
    assert_eq!(s.selection(l.len()), Some(0));
    assert_eq!(s.selection(0), None);
    s.close();
    assert!(!s.is_open());
}

#[test]
fn picker_keys() {
    assert_eq!(picker_key("down"), Some(PickerAction::Relocate(1)));
    assert_eq!(picker_key("enter"), Some(PickerAction::Choose));
    assert_eq!(picker_key("escape"), Some(PickerAction::Close));
    assert_eq!(picker_key("o"), Some(PickerAction::OpenFolder));
    assert_eq!(picker_key("n"), Some(PickerAction::CreateHere));
    assert_eq!(picker_key("x"), None);
}

#[test]
fn open_distinguishes_vault_folder_common_missing_and_file() {
    let t = tempdir().unwrap();
    let v = t.path().join("vault");
    std::fs::create_dir_all(v.join("notas")).unwrap();
    let is_open = open(&v, false).unwrap();
    assert!(v.join("fontes").is_dir() && v.join("grifos").is_dir(), "subpastas que faltavam foram criadas");
    assert_eq!(is_open.root, v);
    let common = t.path().join("comum");
    std::fs::create_dir(&common).unwrap();
    assert_eq!(open(&common, false).unwrap_err(), VaultError::NotAVault(common.clone()));
    assert!(!common.join("notas").exists(), "nada criado sem pedir");
    open(&common, true).unwrap();
    assert!(common.join("notas").is_dir());
    assert!(matches!(open(&t.path().join("nada"), true), Err(VaultError::DoesNotExist(_))), "nao cria a pasta raiz");
    let fname = t.path().join("a.txt");
    std::fs::write(&fname, "x").unwrap();
    assert!(matches!(open(&fname, true), Err(VaultError::NotAFolder(_))));
}

/// Vault do formato antigo (`pistas/` e nenhuma `notes/`): nao abre nem cria nada, e a mensagem manda rodar o script.
#[test]
fn vault_of_the_format_old_asks_the_migration_and_not_touches_in_nothing() {
    let t = tempdir().unwrap();
    let v = t.path().join("vault");
    std::fs::create_dir_all(v.join("pistas")).unwrap();
    for create in [false, true] {
        let e = open(&v, create).unwrap_err();
        assert_eq!(e, VaultError::OldFormat(v.clone()));
        assert!(e.to_string().contains("migrar_pistas_para_notas.py"), "{e}");
    }
    assert!(!v.join("notas").exists(), "nada criado");
    // com `notes/` ja criada (migracao feita ou vault novo) abre normalmente
    std::fs::create_dir(v.join("notas")).unwrap();
    open(&v, false).unwrap();
}

#[cfg(unix)]
#[test]
fn folder_no_permission_and_unreadable() {
    use std::os::unix::fs::PermissionsExt;
    let t = tempdir().unwrap();
    let p = t.path().join("fechada");
    std::fs::create_dir_all(p.join("notas")).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o000)).unwrap();
    let r = open(&p, false);
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(r, Err(VaultError::Unreadable(..))), "{r:?}");
    assert!(r.unwrap_err().to_string().contains("nao foi possivel ler"));
}

#[test]
fn open_with_highlights_le_os_highlights_of_the_source_in_the_vault_new() {
    let t = tempdir().unwrap();
    let v = t.path().join("v");
    let vault = Vault::open_or_create(&v).unwrap();
    std::fs::write(vault.root.join("grifos/Livro.json"), "{ quebrado").unwrap();
    let a = open_with_highlights(&v, false, Some("Livro")).unwrap();
    assert!(a.highlights.unwrap().is_err());
    assert!(open_with_highlights(&v, false, Some("Outra")).unwrap().highlights.unwrap().unwrap().images.is_empty());
    assert!(open_with_highlights(&v, false, None).unwrap().highlights.is_none());
}
