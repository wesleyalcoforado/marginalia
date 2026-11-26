use marginalia_core::source::{cleanup_cache, FolderSource, ImageSource, ZipSource};
use std::fs;
use std::io::Write;
use std::path::Path;
use zip::write::SimpleFileOptions;

const FILES: &[(&str, &[u8])] = &[
    ("img_10.jpg", b"dez"),
    ("img_2.jpg", b"dois"),
    ("__MACOSX/._img_2.jpg", b"lixo"),
    (".DS_Store", b"lixo"),
    ("pasta/sub/img_1.PNG", b"um"),
    ("leia-me.txt", b"texto"),
];

fn zip_bytes(names: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (n, b) in names {
        w.start_file(*n, SimpleFileOptions::default()).unwrap();
        w.write_all(b).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn writes_zip(dir: &Path, names: &[(&str, &[u8])]) -> std::path::PathBuf {
    let p = dir.join("fonte.zip");
    fs::write(&p, zip_bytes(names)).unwrap();
    p
}

fn file_paths(s: &dyn ImageSource) -> Vec<String> {
    s.entries().iter().map(|e| e.path.clone()).collect()
}

const EXPECTED: [&str; 3] = ["img_2.jpg", "img_10.jpg", "pasta/sub/img_1.PNG"];

#[test]
fn zip_list_filters_and_sorts_naturally() {
    let d = tempfile::tempdir().unwrap();
    let z = ZipSource::open_direct(&writes_zip(d.path(), FILES)).unwrap();
    assert_eq!(file_paths(&z), EXPECTED);
    assert_eq!(z.read("img_10.jpg").unwrap(), b"dez");
    assert_eq!(z.read("pasta/sub/img_1.PNG").unwrap(), b"um");
}

#[test]
fn zip_not_le_input_outside_of_the_list() {
    let d = tempfile::tempdir().unwrap();
    let z = ZipSource::open_direct(&writes_zip(d.path(), FILES)).unwrap();
    assert!(z.read("leia-me.txt").is_err());
    assert!(z.read("../x").is_err());
    assert!(z.read("nao/existe.jpg").is_err());
}

/// Troca `de` por `to` (mesmo tamanho) em todo o zip; com `flag_utf8`, liga o
/// bit 11 nos cabecalhos locais (offset 6) e central (offset 8).
fn patches(mut b: Vec<u8>, de: &[u8], to: &[u8], flag_utf8: bool) -> Vec<u8> {
    assert_eq!(de.len(), to.len());
    let mut i = 0;
    while i + de.len() <= b.len() {
        if &b[i..i + de.len()] == de {
            b[i..i + de.len()].copy_from_slice(to);
            i += de.len();
        } else {
            i += 1;
        }
    }
    if flag_utf8 {
        for i in 0..b.len().saturating_sub(10) {
            if &b[i..i + 4] == b"PK\x03\x04" {
                b[i + 7] |= 0x08;
            } else if &b[i..i + 4] == b"PK\x01\x02" {
                b[i + 9] |= 0x08;
            }
        }
    }
    b
}

#[test]
fn zip_with_name_utf_8_invalid_with_flag_skips_the_input() {
    let d = tempfile::tempdir().unwrap();
    let b = zip_bytes(&[("img_2.jpg", b"dois"), ("ZZZ_X.jpg", b"ruim")]);
    let b = patches(b, b"ZZZ_X.jpg", b"\xff\xfe\xfd_X.jpg", true);
    let p = d.path().join("raw.zip");
    fs::write(&p, b).unwrap();
    let z = ZipSource::open_direct(&p).unwrap();
    assert_eq!(file_paths(&z), ["img_2.jpg"]);
}

#[test]
fn zip_no_flag_utf_8_decodes_cp_437_and_le() {
    let d = tempfile::tempdir().unwrap();
    let b = zip_bytes(&[
        ("Inventzrio 1850/img_10.jpg", b"dez"),
        ("Inventzrio 1850/img_2.jpg", b"dois"),
    ]);
    let b = patches(b, b"Inventzrio", b"Invent\xa0rio", false);
    let p = d.path().join("cp437.zip");
    fs::write(&p, b).unwrap();
    let expected = [
        "Inventário 1850/img_2.jpg",
        "Inventário 1850/img_10.jpg",
    ];
    let z = ZipSource::open_direct(&p).unwrap();
    assert_eq!(file_paths(&z), expected);
    assert_eq!(z.read("Inventário 1850/img_10.jpg").unwrap(), b"dez");
    let c = ZipSource::open_cached(&p, &d.path().join("cache")).unwrap();
    assert_eq!(file_paths(&c), expected);
    assert_eq!(c.read("Inventário 1850/img_2.jpg").unwrap(), b"dois");
}

#[test]
fn zip_cached_copy_and_cleanup_remove() {
    let d = tempfile::tempdir().unwrap();
    let orig = writes_zip(d.path(), FILES);
    let cache = d.path().join("a/b/cache");
    let z = ZipSource::open_cached(&orig, &cache).unwrap();
    assert_eq!(file_paths(&z), EXPECTED);
    assert_eq!(z.read("img_2.jpg").unwrap(), b"dois");
    assert!(fs::read_dir(&cache).unwrap().count() >= 1);
    // sobrevive a remocao do original (le da copia)
    fs::remove_file(&orig).unwrap();
    assert_eq!(z.read("img_10.jpg").unwrap(), b"dez");
    cleanup_cache(&cache);
    assert!(!cache.exists());
    cleanup_cache(&cache); // dir ausente: sem panico
}

#[test]
fn folder_has_same_list_and_ignores_junk() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("fonte");
    for (n, b) in FILES {
        let p = root.join(n);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, b).unwrap();
    }
    let f = FolderSource::open(&root).unwrap();
    assert_eq!(file_paths(&f), EXPECTED);
    assert_eq!(f.read("pasta/sub/img_1.PNG").unwrap(), b"um");
}

#[test]
fn folder_not_le_outside_of_the_list() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("fonte");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.jpg"), b"a").unwrap();
    fs::write(d.path().join("x"), b"segredo").unwrap();
    let f = FolderSource::open(&root).unwrap();
    assert!(f.read("../x").is_err());
    assert!(f.read("nao/existe.jpg").is_err());
    assert!(FolderSource::open(&d.path().join("nada")).is_err());
}

#[test]
fn zip_empty_and_corrupt_give_error() {
    let d = tempfile::tempdir().unwrap();
    let empty = writes_zip(d.path(), &[]);
    assert!(ZipSource::open_direct(&empty).is_err());
    let junk = d.path().join("lixo.zip");
    fs::write(&junk, b"isto nao e um zip").unwrap();
    assert!(ZipSource::open_direct(&junk).is_err());
    assert!(ZipSource::open_direct(&d.path().join("nada.zip")).is_err());
}

#[test]
fn zip_with_names_duplicates_not_enters_in_panic() {
    let d = tempfile::tempdir().unwrap();
    let mut b = zip_bytes(&[("a_1.jpg", b"um"), ("b_1.jpg", b"dois")]);
    // renomeia b_1.jpg -> a_1.jpg (mesmo tamanho) em ambos os cabecalhos
    let mut i = 0;
    while i + 7 <= b.len() {
        if &b[i..i + 7] == b"b_1.jpg" {
            b[i..i + 7].copy_from_slice(b"a_1.jpg");
            i += 7;
        } else {
            i += 1;
        }
    }
    let p = d.path().join("dup.zip");
    fs::write(&p, b).unwrap();
    if let Ok(z) = ZipSource::open_direct(&p) {
        assert!(z.entries().iter().all(|e| e.path == "a_1.jpg"));
        let _ = z.read("a_1.jpg");
    }
}
