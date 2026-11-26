use super::*;

fn img() -> ImageRef {
    ImageRef { source: "f".into(), file: "a".into(), page: 1 }
}
fn fr() -> Framing {
    Framing { zoom: 1.0, cx: 0.0, cy: 0.0 }
}

/// Uma lista que nao acompanha o tipo quebra aqui, nao no vault do usuario.
#[test]
fn fields_match_with_the_serialization() {
    let e = Extras::new;
    let p = Note { id: "i".into(), image: img(), framing: fr(), highlight: Some("g".into()), record: Some("r".into()), extra: e() };
    assert_eq!(serialized_keys(&p), Note::FIELDS);
    let t = Task { id: "i".into(), state: StateTask::Open, image: Some(img()), note: Some("p".into()), record: Some("r".into()), extra: e() };
    assert_eq!(serialized_keys(&t), Task::FIELDS);
    let f = Source { name: "n".into(), kind: SourceKind::Zip, extra: e() };
    assert_eq!(serialized_keys(&f), Source::FIELDS);
    let pos = Position { file: "a.jpg".into(), page: 1 };
    let r = Record { id: "i".into(), source: "f".into(), name: "n".into(), start: pos.clone(), end: Some(pos), extra: e() };
    assert_eq!(serialized_keys(&r), Record::FIELDS);
}

#[test]
fn extras_to_removes_so_as_keys_of_the_destination() {
    let mut m = Extras::new();
    m.insert("highlight".into(), "g".into());
    m.insert("tags".into(), "x".into());
    m.insert("image".into(), "y".into());
    let r = to_extras::<Note>(m);
    assert_eq!(r.len(), 1);
    assert!(r.contains_key("tags"));
}

/// Chaves de `v` serializado (com todos os opcionais preenchidos e `extra` vazio).
pub(crate) fn serialized_keys<T: Serialize>(v: &T) -> Vec<String> {
    match serde_yaml::to_value(v).unwrap() {
        serde_yaml::Value::Mapping(m) => m.keys().map(|k| k.as_str().unwrap().to_string()).collect(),
        _ => panic!("nao e mapa"),
    }
}
