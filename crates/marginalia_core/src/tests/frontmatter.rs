use super::*;
use crate::model::*;

fn note() -> Note {
    Note {
        id: "20260101-000000".into(),
        image: ImageRef { source: "livro".into(), file: "a.jpg".into(), page: 1 },
        framing: Framing { zoom: 2.0, cx: 0.5, cy: 0.25 },
        highlight: Some("Maria".into()),
        record: None,
        extra: Extras::new(),
    }
}

#[test]
fn forward_and_back() {
    let body = "Maria: herdeira? \"talvez\" — 1ª linha\n- item";
    let doc = render(&note(), body);
    let (meta, b): (Note, String) = parse(&doc).unwrap();
    assert_eq!(meta, note());
    assert_eq!(b, body);
}

#[test]
fn empty_body() {
    let doc = render(&note(), "");
    let (meta, b): (Note, String) = parse(&doc).unwrap();
    assert_eq!(meta, note());
    assert_eq!(b, "");
}

#[test]
fn no_frontmatter() {
    let r = parse::<Note>("so texto");
    assert!(matches!(r, Err(Error::NoFrontmatter)));
}

#[test]
fn note_starts_by_the_id_and_not_has_state() {
    let doc = render(&note(), "x");
    assert!(!doc.contains("estado"));
    assert!(doc.starts_with("---\nid: "));
}

/// `body_start` corta exatamente onde `parse` corta.
#[test]
fn start_of_the_body_hits_with_parse() {
    for doc in ["---\nid: x\n---\ncorpo\n---\nmais", "---\r\nid: x\r\n---\r\nc", "---\nid: x\n---", "---\nid: x\n---\n"] {
        let i = body_start(doc).unwrap();
        let (_, b): (serde_yaml::Mapping, String) = parse(doc).unwrap();
        assert_eq!(&doc[i..], b, "{doc:?}");
    }
    assert_eq!(body_start("so texto"), None);
    assert_eq!(body_start("---\nid: x\n"), None);
}

/// Chaves reservadas gravadas por versoes antigas (`rascunho`, `renomeando_de`) valem como as novas.
#[test]
fn reserved_keys_old_in_portuguese_are_read_as_the_new_ones() {
    let (m, _): (std::collections::BTreeMap<String, serde_yaml::Value>, _) =
        parse("---\nid: x\nrascunho: true\nrenomeando_de: y\n---\ncorpo").unwrap();
    assert_eq!(m.get("draft"), Some(&serde_yaml::Value::Bool(true)));
    assert_eq!(m.get("renaming_from").and_then(|v| v.as_str()), Some("y"));
    assert!(!m.contains_key("rascunho") && !m.contains_key("renomeando_de"));
}
