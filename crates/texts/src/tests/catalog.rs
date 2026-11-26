use crate::catalog::parse;

#[test]
fn separates_in_the_first_equality_with_spaces() {
    let m = parse("a.b = um = dois").unwrap();
    assert_eq!(m["a.b"], "um = dois");
}

#[test]
fn bar_n_becomes_break_of_line() {
    assert_eq!(parse("x = l1\\nl2").unwrap()["x"], "l1\nl2");
}

#[test]
fn comments_and_lines_empty_are_ignored() {
    let m = parse("# c\n\nk = v").unwrap();
    assert_eq!(m.len(), 1);
    assert_eq!(m["k"], "v");
}

#[test]
fn key_duplicate_and_line_no_separator_are_errors_with_the_line() {
    assert!(parse("k = v\nk = w").unwrap_err().contains("linha 2"));
    assert!(parse("sem separador").unwrap_err().contains("linha 1"));
}

#[test]
fn accents_and_symbols_stay_as_are() {
    assert_eq!(parse("k = abrir fonte · já… ").unwrap()["k"], "abrir fonte · já…");
}
