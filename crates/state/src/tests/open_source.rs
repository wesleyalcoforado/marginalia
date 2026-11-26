use super::*;
use std::io::Write;
use tempfile::tempdir;

fn zip_with(file_path: &Path, names: &[&str]) {
    let f = std::fs::File::create(file_path).unwrap();
    let mut z = zip::ZipWriter::new(f);
    for n in names {
        z.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(b"x").unwrap();
    }
    z.finish().unwrap();
}

#[test]
fn name_default_of_folder_and_zip() {
    assert_eq!(default_name(Path::new("/nas/Livro 3 de Batismos")), "Livro 3 de Batismos");
    assert_eq!(default_name(Path::new("/nas/Livro 3 de Batismos/")), "Livro 3 de Batismos");
    assert_eq!(default_name(Path::new("/nas/Obitos 1850.ZIP")), "Obitos 1850");
    assert_eq!(default_name(Path::new("/nas/a.b.zip")), "a.b");
}

#[test]
fn kind_by_the_disk_and_extension() {
    let t = tempdir().unwrap();
    let z = t.path().join("x.Zip");
    zip_with(&z, &["a.jpg"]);
    let j = t.path().join("a.jpg");
    std::fs::write(&j, b"x").unwrap();
    assert_eq!(kind_of(t.path()), Some(Kind::Folder));
    assert_eq!(kind_of(&z), Some(Kind::Zip));
    assert_eq!(kind_of(&j), Some(Kind::File));
    assert_eq!(kind_of(&t.path().join("nada")), None);
}

#[test]
fn name_free_adds_suffix() {
    assert_eq!(free_name("L", |_| false), "L");
    assert_eq!(free_name("L", |n| n == "L"), "L (2)");
    assert_eq!(free_name("L", |n| n == "L" || n == "L (2)"), "L (3)");
}

#[test]
fn register_creates_source_and_writes_path() {
    let t = tempdir().unwrap();
    let vault = Vault::open_or_create(&t.path().join("vault")).unwrap();
    let mut ad = AppData::new(t.path().join("ad"));
    let a = t.path().join("Livro");
    std::fs::create_dir(&a).unwrap();
    assert_eq!(register(Some(&vault), &mut ad, "Livro", SourceKind::Folder, &a), Ok(Record::New));
    assert_eq!(vault.list_sources().unwrap().len(), 1);
    assert_eq!(ad.source_path(&vault.root, "Livro"), Some(a.clone()));
    // salvo em disco
    let was_read = AppData::load(t.path().join("ad")).unwrap();
    assert_eq!(was_read.source_path(&vault.root, "Livro"), Some(a.clone()));
    // reabrir o mesmo caminho com o mesmo nome nao duplica
    assert_eq!(register(Some(&vault), &mut ad, "Livro", SourceKind::Folder, &a), Ok(Record::AlreadyRegistered));
    assert_eq!(vault.list_sources().unwrap().len(), 1);
}

#[test]
fn name_of_other_source_and_refused() {
    let t = tempdir().unwrap();
    let vault = Vault::open_or_create(&t.path().join("vault")).unwrap();
    let mut ad = AppData::new(t.path().join("ad"));
    let a = t.path().join("um/Livro");
    let b = t.path().join("dois/Livro");
    register(Some(&vault), &mut ad, "Livro", SourceKind::Folder, &a).unwrap();
    assert!(conflicts(&vault, &ad, "Livro", &b).unwrap());
    assert!(!conflicts(&vault, &ad, "Livro", &a).unwrap());
    assert!(!conflicts(&vault, &ad, "Outro", &b).unwrap());
    assert!(register(Some(&vault), &mut ad, "Livro", SourceKind::Folder, &b).is_err());
    assert_eq!(ad.source_path(&vault.root, "Livro"), Some(a));
}

#[test]
fn source_of_the_vault_no_path_in_this_machine_adopts_the_path() {
    let t = tempdir().unwrap();
    let vault = Vault::open_or_create(&t.path().join("vault")).unwrap();
    vault.create_source("Livro", SourceKind::Folder).unwrap();
    let mut ad = AppData::new(t.path().join("ad"));
    let a = t.path().join("Livro");
    assert!(!conflicts(&vault, &ad, "Livro", &a).unwrap());
    assert_eq!(register(Some(&vault), &mut ad, "Livro", SourceKind::Folder, &a), Ok(Record::AlreadyRegistered));
    assert_eq!(ad.source_path(&vault.root, "Livro"), Some(a));
}

#[test]
fn no_vault_not_registers() {
    let t = tempdir().unwrap();
    let mut ad = AppData::new(t.path().join("ad"));
    assert_eq!(register(None, &mut ad, "L", SourceKind::Folder, t.path()), Ok(Record::NoVault));
    assert!(!t.path().join("ad").exists());
}

#[test]
fn open_folder_zip_direct_zip_cache_and_file() {
    let t = tempdir().unwrap();
    let folder = t.path().join("p");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("b.jpg"), b"x").unwrap();
    std::fs::write(folder.join("a.jpg"), b"x").unwrap();
    let z = t.path().join("z.zip");
    zip_with(&z, &["1.png", "2.png", "3.png"]);
    let cache = t.path().join("cache");

    let s = open(&folder, ZipMode::Direct, &cache).unwrap();
    assert_eq!(s.entries().len(), 2);
    assert_eq!(s.entries()[0].path, "a.jpg");
    assert_eq!(open(&z, ZipMode::Direct, &cache).unwrap().entries().len(), 3);
    assert!(!cache.exists());
    let c = open(&z, ZipMode::Cache, &cache).unwrap();
    assert_eq!(c.read("2.png").unwrap(), b"x");
    assert!(cache.join("z.zip").is_file());
    let u = open(&folder.join("b.jpg"), ZipMode::Direct, &cache).unwrap();
    assert_eq!(u.entries().len(), 1);
    assert_eq!(u.entries()[0].path, "b.jpg");
    assert_eq!(u.read("b.jpg").unwrap(), b"x");
    assert!(u.read("a.jpg").is_err());
    assert!(open(&t.path().join("nada"), ZipMode::Direct, &cache).is_err());
}

#[test]
fn conflict_and_record_in_the_background_not_touch_os_data_of_the_app() {
    let t = tempdir().unwrap();
    let vault = Vault::open_or_create(&t.path().join("vault")).unwrap();
    let mut ad = AppData::new(t.path().join("ad"));
    let a = t.path().join("um/Livro");
    let b = t.path().join("dois/Livro");
    register(Some(&vault), &mut ad, "Livro", SourceKind::Folder, &a).unwrap();
    let sources = vault.list_sources().unwrap();
    assert!(conflicts_at(&sources, &ad, &vault.root, "Livro", &b));
    assert!(!conflicts_at(&sources, &ad, &vault.root, "Livro", &a));
    assert!(!conflicts_at(&sources, &ad, &vault.root, "Novo", &b));
    assert_eq!(register_in_vault(&vault, "Novo", SourceKind::Zip, &sources), Ok(Record::New));
    assert!(vault.root.join("fontes/Novo.md").is_file());
    assert_eq!(register_in_vault(&vault, "Livro", SourceKind::Folder, &sources), Ok(Record::AlreadyRegistered));
    assert_eq!(ad.source_path(&vault.root, "Novo"), None, "o caminho e gravado depois, na thread da interface");
    store_path(&mut ad, &vault.root, "Novo", &b);
    assert_eq!(ad.source_path(&vault.root, "Novo"), Some(b.clone()), "na memoria");
    assert_eq!(
        AppData::load(t.path().join("ad")).unwrap().source_path(&vault.root, "Novo"),
        None,
        "o disco fica com quem chama (fila numerada do AppState)"
    );
}

fn refuses(a: DropAction) -> String {
    match a {
        DropAction::Refuse(m) => m,
        DropAction::Open(p) => panic!("esperava recusa, abriu {}", p.display()),
    }
}

#[test]
fn drop_of_folder_zip_image_and_pdf_opens_the_own_item() {
    let t = tempdir().unwrap();
    let folder = t.path().join("Livro");
    std::fs::create_dir(&folder).unwrap();
    let z = t.path().join("Obitos.ZIP");
    zip_with(&z, &["1.png"]);
    for name in ["a.jpg", "b.JPEG", "c.png", "d.tif", "e.TIFF", "f.pdf"] {
        let f = folder.join(name);
        std::fs::write(&f, b"x").unwrap();
        // Imagem/PDF avulso: abre o proprio arquivo (fonte de uma imagem), nao a pasta.
        assert_eq!(sort_drop(std::slice::from_ref(&f)), DropAction::Open(f.clone()), "{name}");
    }
    assert_eq!(sort_drop(std::slice::from_ref(&folder)), DropAction::Open(folder.clone()));
    assert_eq!(sort_drop(std::slice::from_ref(&z)), DropAction::Open(z.clone()));
}

#[test]
fn drop_empty_several_unknown_or_missing_and_refused() {
    let t = tempdir().unwrap();
    let a = t.path().join("a.jpg");
    let b = t.path().join("b.jpg");
    std::fs::write(&a, b"x").unwrap();
    std::fs::write(&b, b"x").unwrap();
    let txt = t.path().join("notas.txt");
    std::fs::write(&txt, b"x").unwrap();
    let no_ext = t.path().join("LEIAME");
    std::fs::write(&no_ext, b"x").unwrap();

    assert!(refuses(sort_drop(&[])).contains("solte uma pasta"));
    let m = refuses(sort_drop(&[a.clone(), b.clone()]));
    assert!(m.contains("um item so") && m.contains('2'), "{m}");
    let m = refuses(sort_drop(std::slice::from_ref(&txt)));
    assert!(m.contains("notas.txt") && m.contains("solte uma pasta"), "{m}");
    assert!(refuses(sort_drop(std::slice::from_ref(&no_ext))).contains("LEIAME"));
    let missing = t.path().join("sumiu.zip");
    let m = refuses(sort_drop(std::slice::from_ref(&missing)));
    assert!(m.contains("nao encontrado") && m.contains("sumiu.zip"), "{m}");
}

#[test]
fn open_in_the_background_refuses_conflict_before_of_open_and_registers_so_source_with_images() {
    let t = tempdir().unwrap();
    let vault = Vault::open_or_create(&t.path().join("vault")).unwrap();
    let mut ad = AppData::new(t.path().join("ad"));
    let a = t.path().join("um/Livro");
    std::fs::create_dir_all(&a).unwrap();
    std::fs::write(a.join("p.jpg"), b"x").unwrap();
    register(Some(&vault), &mut ad, "Livro", SourceKind::Folder, &a).unwrap();
    let ab = |name: &str, file_path: &Path| Opening {
        name: name.into(),
        file_path: file_path.to_path_buf(),
        mode: ZipMode::Direct,
        cache_dir: t.path().join("cache"),
        vault: Some(vault.clone()),
    };
    let e = open_in_background(&ab("Livro", &t.path().join("nao-existe")), &ad).err().unwrap().to_string();
    assert!(e.contains("outra fonte"), "conflito antes de tocar o caminho: {e}");
    let ok = open_in_background(&ab("Livro", &a), &ad).unwrap();
    assert_eq!(ok.record, Some(Ok(Record::AlreadyRegistered)));
    assert!(ok.ready.highlights.is_some());
    let b = t.path().join("Outro");
    std::fs::create_dir(&b).unwrap();
    std::fs::write(b.join("q.png"), b"x").unwrap();
    assert_eq!(open_in_background(&ab("Outro", &b), &ad).unwrap().record, Some(Ok(Record::New)));
    assert_eq!(ad.source_path(&vault.root, "Outro"), None, "dados do app: so na thread da interface");
    let empty = t.path().join("Vazia");
    std::fs::create_dir(&empty).unwrap();
    assert_eq!(open_in_background(&ab("Vazia", &empty), &ad).unwrap().record, None);
    assert_eq!(open_in_background(&ab("p.jpg", &a.join("p.jpg")), &ad).unwrap().record, None, "avulso nao vira fonte");
    assert!(!vault.root.join("fontes/Vazia.md").exists());
}
