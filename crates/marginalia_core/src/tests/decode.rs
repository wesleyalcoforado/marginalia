use super::*;
use std::ffi::OsString;

fn root_dir() -> PathBuf {
    PathBuf::from("raiz")
}

#[test]
fn folders_of_the_pdfium_no_nothing_so_has_the_vendor() {
    let d = pdfium_folders(None, None, &root_dir());
    assert_eq!(d, vec![root_dir().join("vendor").join("pdfium").join("lib")]);
}

#[test]
fn folders_of_the_pdfium_variable_comes_first_and_empty_and_ignored() {
    let d = pdfium_folders(Some(OsString::from("/x")), None, &root_dir());
    assert_eq!(d[0], PathBuf::from("/x"));
    assert_eq!(d.len(), 2);
    let d = pdfium_folders(Some(OsString::new()), None, &root_dir());
    assert_eq!(d.len(), 1);
}

#[test]
fn folders_of_the_pdfium_looks_to_the_side_of_the_executable_before_of_the_vendor() {
    let exe = Path::new("/app/bin/marginalia");
    let d = pdfium_folders(Some(OsString::from("/x")), Some(exe), &root_dir());
    assert_eq!(d[0], PathBuf::from("/x"));
    assert_eq!(d[1], PathBuf::from("/app/bin"));
    assert_eq!(d[2], PathBuf::from("/app/bin").join("..").join("Frameworks"));
    assert_eq!(d[3], root_dir().join("vendor").join("pdfium").join("lib"));
}
