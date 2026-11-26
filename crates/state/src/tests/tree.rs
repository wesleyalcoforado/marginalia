use super::*;
use marginalia_core::model::Extras;

fn img(source: &str, file: &str) -> ImageRef {
    ImageRef { source: source.into(), file: file.into(), page: 1 }
}
fn pos(a: &str) -> Position {
    Position { file: a.into(), page: 1 }
}
fn record(id: &str, source: &str, ini: &str, end: Option<&str>) -> Record {
    Record { id: id.into(), source: source.into(), name: format!("nome-{id}"), start: pos(ini), end: end.map(pos), extra: Extras::new() }
}
fn ent<'a>(id: &'a str, i: &'a ImageRef) -> Input<'a> {
    Input { id, image: Some(i), record: None }
}
fn no_filter(_: &str) -> bool {
    true
}
fn none() -> HashSet<String> {
    HashSet::new()
}
/// Resumo: `F:source`, `R:id(total)`, `I:id`.
fn summary(a: &Tree, es: &[Input<'_>]) -> Vec<String> {
    a.lines
        .iter()
        .map(|l| match l {
            TreeLine::Source { name, collapsed, .. } => format!("F:{name}{}", if *collapsed { "-" } else { "" }),
            TreeLine::Record { record, total, valid, collapsed, .. } => {
                format!("R:{}({total}){}{}", record.id, if *valid { "" } else { "!" }, if *collapsed { "-" } else { "" })
            }
            TreeLine::Item(k) | TreeLine::Drops(k) => format!("I:{}", es[a.order[*k]].id),
        })
        .collect()
}

#[test]
fn no_records_the_source_becomes_header_and_os_items_follow_the_order_of_the_images() {
    let (a, b) = (img("L", "9.jpg"), img("L", "1.jpg"));
    let es = [ent("x", &a), ent("y", &b)];
    let tree = build(&es, &[], &none(), true, &no_filter);
    assert_eq!(summary(&tree, &es), ["F:L", "I:y", "I:x"]);
}

#[test]
fn groups_by_source_and_record_in_the_order_of_the_images() {
    let (i1, i3, i5, i8) = (img("L", "1.jpg"), img("L", "3.jpg"), img("L", "5.jpg"), img("L", "8.jpg"));
    let es = [ent("e8", &i8), ent("e3", &i3), ent("e5", &i5), ent("e1", &i1)];
    let records = [record("A", "L", "3.jpg", Some("5.jpg"))];
    let tree = build(&es, &records, &none(), true, &no_filter);
    // registros primeiro (A com 3 e 5), depois as soltas na ordem das imagens (1 e 8)
    assert_eq!(summary(&tree, &es), ["F:L", "R:A(2)", "I:e3", "I:e5", "I:e1", "I:e8"]);
}

#[test]
fn in_the_same_image_the_order_and_the_of_the_id() {
    let i = img("L", "3.jpg");
    let es = [ent("b", &i), ent("a", &i)];
    let records = [record("A", "L", "3.jpg", None)];
    let tree = build(&es, &records, &none(), true, &no_filter);
    assert_eq!(summary(&tree, &es), ["F:L", "R:A(2)", "I:a", "I:b"]);
}

#[test]
fn note_in_the_boundary_goes_to_the_first_record_and_the_override_the_takes_to_the_second() {
    let i = img("L", "5.jpg");
    let records = [record("A", "L", "1.jpg", Some("5.jpg")), record("B", "L", "5.jpg", Some("9.jpg"))];
    let es = [Input { id: "p", image: Some(&i), record: None }];
    assert_eq!(summary(&build(&es, &records, &none(), true, &no_filter), &es), ["F:L", "R:A(1)", "I:p", "R:B(0)"]);
    let es = [Input { id: "p", image: Some(&i), record: Some("B") }];
    assert_eq!(summary(&build(&es, &records, &none(), true, &no_filter), &es), ["F:L", "R:A(0)", "R:B(1)", "I:p"]);
}

#[test]
fn override_to_record_that_not_contains_the_image_and_ignored() {
    let i = img("L", "3.jpg");
    let records = [record("A", "L", "1.jpg", Some("5.jpg")), record("B", "L", "5.jpg", Some("9.jpg"))];
    let es = [Input { id: "p", image: Some(&i), record: Some("B") }];
    assert_eq!(summary(&build(&es, &records, &none(), true, &no_filter), &es), ["F:L", "R:A(1)", "I:p", "R:B(0)"]);
}

#[test]
fn record_collapsed_hides_os_items_and_the_count_stays() {
    let (i3, i4) = (img("L", "3.jpg"), img("L", "4.jpg"));
    let es = [ent("a", &i3), ent("b", &i4)];
    let records = [record("A", "L", "3.jpg", Some("4.jpg"))];
    let rec: HashSet<String> = ["A".to_string()].into();
    let tree = build(&es, &records, &rec, true, &no_filter);
    assert_eq!(summary(&tree, &es), ["F:L", "R:A(2)-"]);
    assert!(tree.order.is_empty());
}

#[test]
fn record_empty_so_appears_no_filter_of_text() {
    let i = img("L", "1.jpg");
    let es = [ent("a", &i)];
    let records = [record("A", "L", "5.jpg", None)];
    assert_eq!(summary(&build(&es, &records, &none(), true, &no_filter), &es), ["F:L", "R:A(0)", "I:a"]);
    assert_eq!(summary(&build(&es, &records, &none(), false, &no_filter), &es), ["F:L", "I:a"]);
}

#[test]
fn record_invalid_appears_marked_and_not_groups() {
    let i = img("L", "5.jpg");
    let es = [ent("a", &i)];
    let records = [record("A", "L", "7.jpg", Some("3.jpg"))];
    assert_eq!(summary(&build(&es, &records, &none(), true, &no_filter), &es), ["F:L", "R:A(0)!", "I:a"]);
}

#[test]
fn item_no_image_stays_by_last_no_source() {
    let i = img("L", "1.jpg");
    let es = [Input { id: "t", image: None, record: None }, ent("a", &i)];
    let records = [record("A", "L", "1.jpg", None)];
    assert_eq!(summary(&build(&es, &records, &none(), true, &no_filter), &es), ["F:L", "R:A(1)", "I:a", "F:", "I:t"]);
}

#[test]
fn range_uses_the_name_of_the_file_and_the_page() {
    let mut r = record("A", "L", "L/0012.jpg", Some("L/0031.jpg"));
    assert_eq!(range(&r), "0012.jpg – 0031.jpg");
    r.end = None;
    r.start.page = 3;
    assert_eq!(range(&r), "0012.jpg p. 3 – …");
}

#[test]
fn destination_of_relocate_so_in_the_boundary() {
    let records = [record("A", "L", "1.jpg", Some("5.jpg")), record("B", "L", "5.jpg", Some("9.jpg"))];
    let f = img("L", "5.jpg");
    assert_eq!(relocate_destination(&records, &f, None), Some(DestinationRelocate { destination: Some("B".into()), name: "nome-B".into(), back: false }));
    assert_eq!(relocate_destination(&records, &f, Some("B")), Some(DestinationRelocate { destination: None, name: "nome-A".into(), back: true }));
    assert_eq!(relocate_destination(&records, &img("L", "3.jpg"), None), None);
}

#[test]
fn drops_after_of_one_record_not_and_item_its() {
    let (i3, i8) = (img("L", "3.jpg"), img("L", "8.jpg"));
    let es = [ent("dentro", &i3), ent("fora", &i8)];
    let records = [record("A", "L", "3.jpg", Some("5.jpg"))];
    let tree = build(&es, &records, &none(), true, &no_filter);
    let kinds: Vec<&str> = tree.lines.iter().map(|l| match l {
        TreeLine::Source { .. } => "F",
        TreeLine::Record { .. } => "R",
        TreeLine::Item(_) => "I",
        TreeLine::Drops(_) => "S",
    }).collect();
    assert_eq!(kinds, ["F", "R", "I", "S"]);
}

#[test]
fn source_no_records_also_gains_header_and_order_of_the_images() {
    let (x, y, e) = (img("Caixa", "9.jpg"), img("Caixa", "1.jpg"), img("Livro", "3.jpg"));
    let es = [ent("x", &x), ent("y", &y), ent("e", &e)];
    let records = [record("A", "Livro", "3.jpg", Some("5.jpg"))];
    let tree = build(&es, &records, &none(), true, &no_filter);
    assert_eq!(summary(&tree, &es), ["F:Caixa", "I:y", "I:x", "F:Livro", "R:A(1)", "I:e"]);
}

#[test]
fn source_collapsed_hides_records_and_items_its() {
    let (i3, i8) = (img("L", "3.jpg"), img("L", "8.jpg"));
    let es = [ent("a", &i3), ent("b", &i8)];
    let records = [record("A", "L", "3.jpg", Some("5.jpg"))];
    let rec: HashSet<String> = ["fonte:L".to_string()].into();
    let tree = build(&es, &records, &rec, true, &no_filter);
    assert_eq!(summary(&tree, &es), ["F:L-"]);
    assert!(tree.order.is_empty());
    match &tree.lines[0] {
        TreeLine::Source { total, .. } => assert_eq!(*total, 2),
        o => panic!("{o:?}"),
    }
}

#[test]
fn guides_of_source_record_and_loose() {
    use Guide::*;
    let (i3, i4, i9) = (img("L", "3.jpg"), img("L", "4.jpg"), img("L", "9.jpg"));
    let es = [ent("a", &i3), ent("b", &i4), ent("c", &i9)];
    let records = [record("A", "L", "3.jpg", Some("5.jpg"))];
    let tree = build(&es, &records, &none(), true, &no_filter);
    // F, R(a, b), solta c
    assert_eq!(
        guides(&tree.lines),
        vec![vec![], vec![Branch], vec![Continues, Branch], vec![Continues, Last], vec![Last]]
    );
}

#[test]
fn guides_record_last_leaves_the_column_empty() {
    use Guide::*;
    let i3 = img("L", "3.jpg");
    let es = [ent("a", &i3)];
    let records = [record("A", "L", "3.jpg", Some("5.jpg"))];
    let tree = build(&es, &records, &none(), true, &no_filter);
    assert_eq!(guides(&tree.lines), vec![vec![], vec![Last], vec![Empty, Last]]);
}

#[test]
fn guides_not_leak_between_sources() {
    use Guide::*;
    let (a, b) = (img("A", "1.jpg"), img("B", "1.jpg"));
    let es = [ent("a", &a), ent("b", &b)];
    let tree = build(&es, &[], &none(), true, &no_filter);
    assert_eq!(guides(&tree.lines), vec![vec![], vec![Last], vec![], vec![Last]]);
}

fn entries(names: &[&str]) -> Vec<ImageEntry> {
    names.iter().map(|n| ImageEntry { path: (*n).into() }).collect()
}

#[test]
fn label_index_image_pdf_and_file_vanished() {
    let es = entries(&["a.jpg", "b.jpg", "c.jpg"]);
    assert_eq!(index_label(&es, &pos("b.jpg")), "img 2");
    assert_eq!(index_label(&es, &pos("zzz.jpg")), "zzz.jpg", "fora da fonte: o caminho");
    // PDF unico: so a pagina; PDF entre outros arquivos: indice e pagina
    let pdf = entries(&["livro.pdf"]);
    assert_eq!(index_label(&pdf, &Position { file: "livro.pdf".into(), page: 7 }), "pág 7");
    let several = entries(&["a.jpg", "livro.pdf"]);
    assert_eq!(index_label(&several, &Position { file: "livro.pdf".into(), page: 7 }), "img 2 · pág 7");
}

#[test]
fn range_by_index_closed_and_open() {
    let es = entries(&["a.jpg", "b.jpg", "c.jpg"]);
    let closed = record("A", "L", "a.jpg", Some("c.jpg"));
    assert_eq!(range_by_index(&es, &closed), "img 1 – img 3");
    let is_open = record("B", "L", "b.jpg", None);
    assert_eq!(range_by_index(&es, &is_open), "img 2 – …");
}
