use crate::{search, fill_in};

#[test]
fn key_absent_returns_the_key() {
    assert_eq!(search("nao.existe"), "nao.existe");
    assert_eq!(t!("nao.existe"), "nao.existe");
}

#[test]
fn fill_and_the_macro_replace_values_by_the_key_returned() {
    // Chave ausente: o "modelo" e a propria chave, entao o preenchimento nao muda nada.
    assert_eq!(fill_in("x {n}", &[("n", "3".into())]), "x {n}");
    assert_eq!(t!("sem.chave", n = 3), "sem.chave");
    assert_eq!(t!("sem.chave", 1, 2), "sem.chave");
}

#[test]
fn the_catalog_embedded_and_valid() {
    crate::catalog::parse(include_str!("../../languages/pt.txt")).unwrap();
}
