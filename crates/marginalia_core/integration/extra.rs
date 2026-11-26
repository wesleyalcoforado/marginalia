//! Campos de frontmatter que o Marginalia nao conhece (`tags:`, `aliases:`,
//! escritos a mao ou por plugins do Obsidian) sobrevivem a toda regravacao.
use chrono::{NaiveDate, NaiveDateTime};
use marginalia_core::model::*;
use marginalia_core::vault::*;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn now() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 11, 12).unwrap().and_hms_opt(14, 32, 0).unwrap()
}
fn img(source: &str) -> ImageRef {
    ImageRef { source: source.into(), file: "a.jpg".into(), page: 3 }
}
fn fr() -> Framing {
    Framing { zoom: 2.0, cx: 0.5, cy: 0.25 }
}

/// Acrescenta `tags` e `aliases` no topo do frontmatter, como faria o usuario.
fn with_extras(path: &Path) {
    let s = fs::read_to_string(path).unwrap();
    let rest = s.strip_prefix("---\n").unwrap();
    fs::write(path, format!("---\ntags:\n- genealogia\n- alves\naliases:\n- Maria A.\n{rest}")).unwrap();
}

/// Os extras continuam la (lidos como yaml cru) e o corpo nao mudou.
fn extras_has(path: &Path, note_body: &str) {
    let s = fs::read_to_string(path).unwrap();
    let (meta, body): (serde_yaml::Mapping, String) = marginalia_core::frontmatter::parse(&s).unwrap();
    let tags: Vec<String> = serde_yaml::from_value(meta["tags"].clone()).unwrap();
    let aliases: Vec<String> = serde_yaml::from_value(meta["aliases"].clone()).unwrap();
    assert_eq!(tags, ["genealogia", "alves"], "{s}");
    assert_eq!(aliases, ["Maria A."], "{s}");
    assert_eq!(body, note_body);
    assert!(!meta.contains_key("extra"), "{s}");
}

fn note_with_extras(v: &Vault) -> (Note, std::path::PathBuf) {
    let p = v.create_note("Maria herdeira", img("livro"), fr(), Some("g1".into()), now()).unwrap();
    let path = v.root.join("notas").join(format!("{}.md", p.id));
    with_extras(&path);
    (p, path)
}

#[test]
fn edit_the_text_of_the_note_preserves_extras() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let (p, path) = note_with_extras(&v);
    v.update_text(&p.id, "Maria herdeira #achado").unwrap();
    extras_has(&path, "Maria herdeira #achado");
    // Os campos conhecidos continuam lidos normalmente.
    let l = v.list_notes().unwrap();
    assert_eq!(l[0].0.highlight.as_deref(), Some("g1"));
    assert_eq!(l[0].0.extra.len(), 2);
}

#[test]
fn mark_task_preserves_extras() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let t = v.create_task("ver batismo", Some(img("livro")), None, now()).unwrap();
    let path = v.root.join("tarefas").join(format!("{}.md", t.id));
    with_extras(&path);
    v.set_state_task(&t.id, StateTask::Done).unwrap();
    extras_has(&path, "ver batismo");
    v.set_state_task(&t.id, StateTask::Open).unwrap();
    extras_has(&path, "ver batismo");
}

#[test]
fn rename_source_preserves_extras_in_all() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    v.create_source("livro", SourceKind::Folder).unwrap();
    with_extras(&v.root.join("fontes/livro.md"));
    let (p, ppath) = note_with_extras(&v);
    let t = v.create_task("ver batismo", Some(img("livro")), None, now()).unwrap();
    let tpath = v.root.join("tarefas").join(format!("{}.md", t.id));
    with_extras(&tpath);
    v.rename_source("livro", "Livro 2").unwrap();
    extras_has(&v.root.join("fontes/Livro 2.md"), "");
    extras_has(&ppath, "Maria herdeira");
    extras_has(&tpath, "ver batismo");
    assert_eq!(v.list_notes().unwrap()[0].0.image.source, "Livro 2");
    assert_eq!(p.image.source, "livro");
    // A marca de renomeacao nao vaza para a fonte final.
    let s = fs::read_to_string(v.root.join("fontes/Livro 2.md")).unwrap();
    assert!(!s.contains("renaming_from"), "{s}");
}

#[test]
fn resume_renaming_not_writes_the_mark_as_extra() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    v.create_source("livro", SourceKind::Folder).unwrap();
    // Estado de uma renomeacao que parou depois de apagar `sources/book.md`.
    fs::write(
        v.root.join("fontes/novo.md"),
        "---\nnome: novo\ntipo: pasta\nrenomeando_de: livro\ntags:\n- genealogia\n- alves\naliases:\n- Maria A.\n---\n",
    )
    .unwrap();
    fs::remove_file(v.root.join("fontes/livro.md")).unwrap();
    v.rename_source("livro", "novo").unwrap();
    let path = v.root.join("fontes/novo.md");
    extras_has(&path, "");
    let s = fs::read_to_string(&path).unwrap();
    assert!(!s.contains("renaming_from"), "{s}");
}

#[test]
fn no_extras_the_output_not_changes() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let p = v.create_note("x", img("livro"), fr(), None, now()).unwrap();
    let s = fs::read_to_string(v.root.join("notas").join(format!("{}.md", p.id))).unwrap();
    assert_eq!(
        s,
        format!(
            "---\nid: {}\nimage:\n  source: livro\n  file: a.jpg\n  page: 3\nframing:\n  zoom: 2.0\n  cx: 0.5\n  cy: 0.25\n---\nx",
            p.id
        )
    );
}

// ---- Round 2: chaves extras que colidem com campos de outro tipo ----

fn with(path: &Path, lines: &str) {
    let s = fs::read_to_string(path).unwrap();
    let rest = s.strip_prefix("---\n").unwrap();
    fs::write(path, format!("---\n{lines}{rest}")).unwrap();
}

#[test]
fn edit_and_mark_with_keys_of_others_kinds_continue_readable() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let p = v.create_note("x", img("livro"), fr(), None, now()).unwrap();
    let pp = v.root.join("notas").join(format!("{}.md", p.id));
    with(&pp, "tipo: casamento\norigem: livro\nnota: outra\n");
    v.update_text(&p.id, "y").unwrap();
    let l = v.list_notes().unwrap();
    assert_eq!(l[0].0.extra.len(), 3);
    let t = v.create_task("t", None, None, now()).unwrap();
    let tp = v.root.join("tarefas").join(format!("{}.md", t.id));
    with(&tp, "tipo: dossie\ngrifo: g9\nenquadramento: perto\n");
    v.set_state_task(&t.id, StateTask::Done).unwrap();
    v.set_state_task(&t.id, StateTask::Open).unwrap();
    let lt = v.list_tasks().unwrap();
    assert_eq!(lt[0].0.extra.len(), 3);
    assert_eq!(lt[0].0.state, StateTask::Open);
}

/// A ordem das chaves do usuario sobrevive (nao e reordenada alfabeticamente).
#[test]
fn extras_keeps_the_order_of_the_user() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let p = v.create_note("x", img("livro"), fr(), None, now()).unwrap();
    let pp = v.root.join("notas").join(format!("{}.md", p.id));
    with(&pp, "zeta: 1\nalfa: 2\nmeio: 3\n");
    v.update_text(&p.id, "y").unwrap();
    let s = fs::read_to_string(&pp).unwrap();
    let (z, a, m) = (s.find("zeta:").unwrap(), s.find("alfa:").unwrap(), s.find("meio:").unwrap());
    assert!(z < a && a < m, "{s}");
}
