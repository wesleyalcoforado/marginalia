use super::*;
use crate::model::Extras;
use std::cmp::Ordering;

fn pos(file: &str, page: u32) -> Position {
    Position { file: file.into(), page }
}
fn p(file: &str) -> Position {
    pos(file, 1)
}
fn record(id: &str, ini: &str, end: Option<&str>) -> Record {
    Record { id: id.into(), source: "f".into(), name: format!("nome-{id}"), start: p(ini), end: end.map(p), extra: Extras::new() }
}
fn img(file: &str) -> ImageRef {
    ImageRef { source: "f".into(), file: file.into(), page: 1 }
}
fn ids(v: Vec<&Record>) -> Vec<&str> {
    v.into_iter().map(|r| r.id.as_str()).collect()
}

#[test]
fn order_and_the_of_the_source_numbers_natural_and_textbox() {
    assert_eq!(compare(("9.jpg", 1), ("10.jpg", 1)), Ordering::Less);
    assert_eq!(compare(("b.jpg", 1), ("A.jpg", 1)), Ordering::Greater);
    // iguais ignorando caixa: desempata pelo texto, como as entradas da fonte
    assert_eq!(compare(("A.jpg", 1), ("a.jpg", 1)), "A.jpg".cmp("a.jpg"));
}

#[test]
fn order_compares_the_page_in_the_same_pdf() {
    assert_eq!(compare(("l.pdf", 2), ("l.pdf", 10)), Ordering::Less);
    assert_eq!(compare(("l.pdf", 3), ("l.pdf", 3)), Ordering::Equal);
    assert_eq!(compare(("a.pdf", 99), ("b.pdf", 1)), Ordering::Less);
}

#[test]
fn range_contains_start_end_and_the_middle_but_not_the_rest() {
    let rs = [record("a", "3.jpg", Some("7.jpg"))];
    let e = extensions(&rs);
    for (fname, inside) in [("2.jpg", false), ("3.jpg", true), ("5.jpg", true), ("7.jpg", true), ("8.jpg", false)] {
        assert_eq!(!candidates(&e, &img(fname)).is_empty(), inside, "{fname}");
    }
}

#[test]
fn end_in_open_goes_ate_the_start_of_the_next_inclusive() {
    let rs = [record("a", "1.jpg", None), record("b", "5.jpg", Some("8.jpg"))];
    let e = extensions(&rs);
    assert_eq!(e[0].end, Some(p("5.jpg")));
    assert!(e[0].valid && e[1].valid);
    assert_eq!(ids(candidates(&e, &img("4.jpg"))), ["a"]);
    assert_eq!(ids(candidates(&e, &img("5.jpg"))), ["a", "b"]);
}

#[test]
fn last_no_end_goes_ate_the_end_of_the_source() {
    let rs = [record("a", "2.jpg", None)];
    let e = extensions(&rs);
    assert_eq!(e[0].end, None);
    assert_eq!(ids(candidates(&e, &img("999.jpg"))), ["a"]);
    assert!(candidates(&e, &img("1.jpg")).is_empty());
}

#[test]
fn boundary_shared_of_the_two_candidates_the_first_and_the_that_ends() {
    let rs = [record("b", "5.jpg", Some("9.jpg")), record("a", "1.jpg", Some("5.jpg"))];
    let e = extensions(&rs);
    assert_eq!(ids(candidates(&e, &img("5.jpg"))), ["a", "b"]);
    assert!(shared(&e, &img("5.jpg")));
    assert!(!shared(&e, &img("4.jpg")));
    assert_eq!(record_of(&e, &img("5.jpg"), None).unwrap().id, "a");
}

#[test]
fn override_valid_move_to_the_second() {
    let rs = [record("a", "1.jpg", Some("5.jpg")), record("b", "5.jpg", Some("9.jpg"))];
    let e = extensions(&rs);
    assert_eq!(record_of(&e, &img("5.jpg"), Some("b")).unwrap().id, "b");
    assert_eq!(record_of(&e, &img("5.jpg"), Some("a")).unwrap().id, "a");
}

#[test]
fn override_of_record_missing_or_that_not_contains_the_image_and_ignored() {
    let rs = [record("a", "1.jpg", Some("5.jpg")), record("b", "5.jpg", Some("9.jpg"))];
    let e = extensions(&rs);
    assert_eq!(record_of(&e, &img("5.jpg"), Some("fantasma")).unwrap().id, "a");
    // em 3.jpg so "a" contem; override para "b" nao vale
    assert_eq!(record_of(&e, &img("3.jpg"), Some("b")).unwrap().id, "a");
    assert!(record_of(&e, &img("20.jpg"), Some("b")).is_none());
}

#[test]
fn end_before_of_the_start_and_invalid_and_not_groups() {
    let rs = [record("a", "7.jpg", Some("3.jpg"))];
    let e = extensions(&rs);
    assert!(!e[0].valid);
    assert!(candidates(&e, &img("5.jpg")).is_empty());
    assert!(record_of(&e, &img("7.jpg"), None).is_none());
}

#[test]
fn overlap_beyond_of_the_boundary_invalid_os_two() {
    let rs = [record("a", "1.jpg", Some("6.jpg")), record("b", "5.jpg", Some("9.jpg")), record("c", "20.jpg", Some("22.jpg"))];
    let e = extensions(&rs);
    assert!(!e[0].valid && !e[1].valid);
    assert!(e[2].valid);
    assert!(candidates(&e, &img("5.jpg")).is_empty());
    // aninhado
    let rs = [record("a", "1.jpg", Some("9.jpg")), record("b", "3.jpg", Some("4.jpg"))];
    let e = extensions(&rs);
    assert!(!e[0].valid && !e[1].valid);
}

#[test]
fn two_records_with_the_same_start_are_invalid() {
    let rs = [record("a", "2.jpg", None), record("b", "2.jpg", Some("4.jpg"))];
    let e = extensions(&rs);
    assert!(!e[0].valid && !e[1].valid);
}

#[test]
fn mark_start_inside_of_record_with_end_and_refused() {
    let rs = [record("a", "1.jpg", Some("9.jpg"))];
    assert_eq!(can_mark_start(&rs, &p("5.jpg")), Err(RecordError::StartInsideOfRecord("nome-a".into())));
    assert_eq!(can_mark_start(&rs, &p("1.jpg")), Err(RecordError::StartInsideOfRecord("nome-a".into())));
    assert_eq!(can_mark_start(&rs, &p("10.jpg")), Ok(()));
    assert_eq!(can_mark_start(&rs, &p("0.jpg")), Ok(()));
}

#[test]
fn mark_start_in_the_image_of_the_end_of_other_and_accepted() {
    let rs = [record("a", "1.jpg", Some("9.jpg"))];
    assert_eq!(can_mark_start(&rs, &p("9.jpg")), Ok(()));
}

#[test]
fn mark_start_after_of_the_start_of_record_no_end_and_accepted() {
    let rs = [record("a", "1.jpg", None)];
    assert_eq!(can_mark_start(&rs, &p("6.jpg")), Ok(()));
    assert_eq!(can_mark_start(&rs, &p("1.jpg")), Err(RecordError::StartInsideOfRecord("nome-a".into())));
}

#[test]
fn mark_end_before_of_the_start_and_refused() {
    let rs = [record("a", "5.jpg", None)];
    assert_eq!(can_mark_end(&rs, "a", &p("4.jpg")), Err(RecordError::EndBeforeOfTheStart));
    assert_eq!(can_mark_end(&rs, "a", &p("5.jpg")), Ok(()));
}

#[test]
fn mark_end_that_swallows_other_record_and_refused() {
    let rs = [record("a", "1.jpg", None), record("b", "5.jpg", Some("8.jpg"))];
    assert_eq!(can_mark_end(&rs, "a", &p("6.jpg")), Err(RecordError::EndSwallowsRecord("nome-b".into())));
    assert_eq!(can_mark_end(&rs, "a", &p("4.jpg")), Ok(()));
}

#[test]
fn mark_end_in_the_start_of_the_next_and_accepted() {
    let rs = [record("a", "1.jpg", None), record("b", "5.jpg", Some("8.jpg"))];
    assert_eq!(can_mark_end(&rs, "a", &p("5.jpg")), Ok(()));
}

#[test]
fn invalid_in_finds_the_record_broken_of_the_image() {
    let rs = [record("a", "7.jpg", Some("3.jpg")), record("b", "10.jpg", Some("12.jpg"))];
    let e = extensions(&rs);
    assert_eq!(invalid_at(&e, &img("7.jpg")).unwrap().id, "a");
    assert!(invalid_at(&e, &img("5.jpg")).is_none());
    assert!(invalid_at(&e, &img("11.jpg")).is_none());
}
