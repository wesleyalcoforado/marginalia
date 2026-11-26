use super::*;
use marginalia_core::source::ImageEntry;
use tempfile::tempdir;

struct List(Vec<ImageEntry>);
impl ImageSource for List {
    fn entries(&self) -> &[ImageEntry] {
        &self.0
    }
    fn read(&self, _: &str) -> marginalia_core::vault::Result<Vec<u8>> {
        Ok(Vec::new())
    }
}
fn list(n: &[&str]) -> List {
    List(n.iter().map(|p| ImageEntry { path: p.to_string() }).collect())
}

#[test]
fn check_requires_the_current_and_half_of_the_cited() {
    let c = Path::new("/nova");
    let e = Expected { kind: None, cited: vec!["a.jpg".into(), "z.jpg".into()], current: Some("b.jpg".into()) };
    assert_eq!(check(&list(&["a.jpg", "b.jpg"]), &e, "L", c), Ok(Checked::Accepted), "1 de 2 = metade");
    let err = check(&list(&["a.jpg"]), &e, "L", c).unwrap_err();
    assert!(err.contains("b.jpg"), "{err}");
    let err = check(&list(&["b.jpg", "x.jpg"]), &e, "L", c).unwrap_err();
    assert!(err.contains("0 das 2") && err.contains("a.jpg"), "{err}");
    let four = Expected { cited: ["a", "b", "c", "d"].map(String::from).to_vec(), ..Expected::default() };
    assert!(check(&list(&["a", "x"]), &four, "L", c).unwrap_err().contains("1 das 4"), "um nome em comum nao basta");
    assert_eq!(check(&list(&["a", "b", "x"]), &four, "L", c), Ok(Checked::Accepted));
    let three = Expected { cited: ["a", "b", "c"].map(String::from).to_vec(), ..Expected::default() };
    assert!(check(&list(&["a"]), &three, "L", c).is_err(), "teto de 50%: 2 de 3");
    assert!(check(&list(&[]), &Expected::default(), "L", c).unwrap_err().contains("nao tem imagens"));
}

#[test]
fn no_nothing_cited_asks_confirmation_with_the_count() {
    let c = Path::new("/nova");
    assert_eq!(check(&list(&["q.png", "r.png"]), &Expected::default(), "L", c), Ok(Checked::Confirm(2)));
    let so_current = Expected { current: Some("q.png".into()), ..Expected::default() };
    assert_eq!(check(&list(&["q.png"]), &so_current, "L", c), Ok(Checked::Accepted), "a imagem aberta confere");
    let p = confirmation_question("Livro", c, 2);
    assert!(p.contains("/nova") && p.contains("2 imagens") && p.contains("Livro"), "{p}");
}

#[test]
fn one_work_of_each_time() {
    let mut x = Exclusive::default();
    assert_eq!(x.busy(), None);
    x.start("trocando de vault...").unwrap();
    let e = x.start("renomeando...").unwrap_err();
    assert!(e.contains("trocando de vault"), "{e}");
    assert_eq!(x.busy(), Some("trocando de vault..."));
    x.finish();
    assert!(x.start("renomeando...").is_ok());
}

#[test]
fn open_source_wait_so_the_renaming() {
    let mut x = Exclusive::default();
    assert!(x.allows_open().is_ok());
    x.start(renaming()).unwrap();
    assert!(x.allows_open().unwrap_err().contains("renomeando"));
    x.finish();
    x.start("abrindo o vault...").unwrap();
    assert!(x.allows_open().is_ok(), "a troca de vault descarta a abertura velha sozinha");
}

#[test]
fn write_in_the_vault_during_renaming_and_refused() {
    let mut x = Exclusive::default();
    assert!(x.allows_write().is_ok());
    x.start(renaming()).unwrap();
    let e = x.allows_write().unwrap_err();
    assert!(e.starts_with("aguarde") && e.contains("renomeando"), "{e}");
    x.finish();
    assert!(x.allows_write().is_ok());
    x.start("conferindo a nova localizacao...").unwrap();
    assert!(x.allows_write().is_ok(), "so a renomeacao reescreve o vault");
}

#[test]
fn kind_of_the_path_needs_combine() {
    let c = Path::new("/x");
    assert!(check_kind("L", Some(SourceKind::Zip), Kind::Zip, c).is_ok());
    assert!(check_kind("L", Some(SourceKind::Folder), Kind::Folder, c).is_ok());
    assert!(check_kind("L", None, Kind::File, c).is_ok());
    assert!(check_kind("L", Some(SourceKind::Zip), Kind::Folder, c).unwrap_err().contains(".zip"));
    assert!(check_kind("L", Some(SourceKind::Folder), Kind::Zip, c).unwrap_err().contains("pasta"));
}

#[test]
fn validate_opens_and_checks_no_touch_nothing_when_failure() {
    let t = tempdir().unwrap();
    let p = t.path().join("Livro");
    std::fs::create_dir(&p).unwrap();
    std::fs::write(p.join("a.jpg"), b"x").unwrap();
    let e = Expected { kind: Some(SourceKind::Folder), cited: vec!["a.jpg".into()], current: None };
    let cache = t.path().join("cache");
    let (src, confirm) = validate(&p, &e, "Livro", &cache).unwrap();
    assert_eq!((src.entries().len(), confirm), (1, None));
    assert!(validate(&t.path().join("nada"), &e, "Livro", &cache).err().unwrap().contains("nao encontrado"));
    let other = Expected { cited: vec!["b.jpg".into()], ..e };
    assert!(validate(&p, &other, "Livro", &cache).is_err());
    assert!(!cache.exists(), "zip/pasta abertos direto, sem copia");
}

#[test]
fn cited_no_repetition_and_limited() {
    assert_eq!(cited(["b", "a", "b", ""]), ["a", "b"]);
    let many: Vec<String> = (0..MAX_CITED + 10).map(|i| format!("{i:04}")).collect();
    assert_eq!(cited(many.iter().map(String::as_str)).len(), MAX_CITED);
}

#[test]
fn name_new_refuses_empty_invalid_traversal_and_duplicate() {
    let ex = vec!["Livro".to_string(), "Outro".to_string()];
    assert_eq!(validate_name("  Livro 3  ", "Livro", &ex), Ok("Livro 3".into()));
    assert_eq!(validate_name("Óbitos 1850 (cópia)", "Livro", &ex), Ok("Óbitos 1850 (cópia)".into()));
    for bad in ["", "   ", "a/b", "a\\b", "a:b", "../x", "..", ".oculto", "linha\nnova", "ta\tb"] {
        assert!(validate_name(bad, "Livro", &ex).is_err(), "{bad:?}");
    }
    assert!(validate_name(&"x".repeat(MAX_NAME + 1), "Livro", &ex).is_err());
    assert!(validate_name("outro", "Livro", &ex).unwrap_err().contains("ja existe"));
    assert!(validate_name("Livro", "Livro", &ex).is_err(), "mesmo nome");
    assert!(validate_name("LIVRO", "Livro", &ex).is_err(), "so caixa");
}

#[test]
fn pending_texts() {
    let mut p = Pending::fresh("Livro 3", Some(PathBuf::from("/Volumes/nas/l3")), Reason::Unavailable);
    p.detail = Some("Input/output error".into());
    assert_eq!(p.title(), "Fonte indisponível: Livro 3");
    assert!(p.explanation().contains("/Volumes/nas/l3") && p.explanation().contains("Input/output"));
    assert!(Pending::fresh("L", None, Reason::NoPath).title().contains("sem caminho"));
    assert!(Pending::fresh("L", Some("/x".into()), Reason::NotFound).explanation().contains("/x"));
}

#[test]
fn drop_no_vault_or_during_renaming_and_refused_with_warning() {
    let free = Exclusive::default();
    assert_eq!(release_drop(true, &free), Ok(()));
    assert!(release_drop(false, &free).unwrap_err().contains("vault"));
    let mut renaming = Exclusive::default();
    renaming.start(super::renaming()).unwrap();
    assert!(release_drop(true, &renaming).unwrap_err().contains("aguarde"));
    assert!(release_drop(false, &renaming).unwrap_err().contains("vault"), "sem vault: o aviso do vault primeiro");
    // Troca de vault ou nova localizacao em andamento: o drop tambem espera
    // (o vault ou a fonte que ele registraria estao mudando).
    for label in ["abrindo o vault...", "conferindo a nova localizacao..."] {
        let mut busy = Exclusive::default();
        busy.start(label).unwrap();
        let m = release_drop(true, &busy).unwrap_err();
        assert!(m.contains("aguarde") && m.contains(label) && m.contains("solte"), "{m}");
    }
}
