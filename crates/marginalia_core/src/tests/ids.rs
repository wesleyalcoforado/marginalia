use super::*;
use chrono::NaiveDate;

fn now() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 11, 12).unwrap().and_hms_opt(14, 32, 0).unwrap()
}

#[test]
fn slug_basic() {
    assert_eq!(slug("Maria Alves, herdeira?"), "maria-alves-herdeira");
}

#[test]
fn slug_no_accents_nor_separators() {
    assert_eq!(slug("Conceição / José: 1ª"), "conceicao-jose-1a");
}

#[test]
fn slug_empty() {
    assert_eq!(slug("???"), "sem-texto");
    assert_eq!(slug(""), "sem-texto");
}

#[test]
fn slug_limits() {
    assert_eq!(slug("um dois tres quatro cinco seis sete oito"), "um-dois-tres-quatro-cinco-seis");
    let s = slug("abcdefghij abcdefghij abcdefghij abcdefghij abcdefghij");
    assert!(s.len() <= 40, "{s}");
    assert!(!s.ends_with('-'));
    let s = slug("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa b c");
    assert!(s.len() <= 40 && !s.ends_with('-'), "{s}");
}

#[test]
fn id_base() {
    assert_eq!(make_id(now(), "Maria Alves herdeira", &|_| false), "20261112-1432-maria-alves-herdeira");
}

#[test]
fn id_collision() {
    let base = "20261112-1432-maria-alves-herdeira";
    assert_eq!(make_id(now(), "Maria Alves herdeira", &|i| i == base), format!("{base}-2"));
    assert_eq!(
        make_id(now(), "Maria Alves herdeira", &|i| i == base || i == format!("{base}-2")),
        format!("{base}-3")
    );
}

#[test]
fn provisional() {
    for id in ["20261112-1432-sem-texto", "20261112-1432-sem-texto-2", "20261112-1432-sem-texto-13"] {
        assert!(eh_provisional(id), "{id}");
    }
    for id in [
        "20261112-1432-maria",
        "20261112-1432-sem-texto-alves",
        "20261112-1432-sem-texto-",
        "20261112-1432-sem-texto-02",
        "20261112-1432sem-texto",
        "2026111-1432-sem-texto",
        "20261312-1432-sem-texto",
        "x0261112-1432-sem-texto",
        "sem-texto",
        "",
        "20261112-1432-sem-textó",
    ] {
        assert!(!eh_provisional(id), "{id}");
    }
    assert_eq!(make_id(now(), "???", &|_| false), "20261112-1432-sem-texto");
    assert!(eh_provisional(&make_id(now(), "", &|i| i == "20261112-1432-sem-texto")));
}

#[test]
fn minute_of_the_id() {
    assert_eq!(created_at("20261112-1432-maria"), Some(now()));
    assert_eq!(created_at("20261112-1432"), None);
    assert_eq!(created_at("20261112-2460-x"), None);
    assert_eq!(created_at("é"), None);
}
