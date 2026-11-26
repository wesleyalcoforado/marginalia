use super::*;
use chrono::NaiveDate;

fn now() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 11, 12).unwrap().and_hms_opt(14, 32, 0).unwrap()
}
fn img() -> ImageRef {
    ImageRef { source: "l".into(), file: "a.jpg".into(), page: 1 }
}
fn fr() -> Framing {
    Framing { zoom: 1.0, cx: 0.5, cy: 0.5 }
}

/// Outra instancia (ou outra maquina no NAS) grava o mesmo id entre a
/// escolha do id e a gravacao: a nossa nota ganha o proximo sufixo e a
/// dela fica intacta.
/// Duas gravacoes do mesmo arquivo (duas threads, ou duas instancias no
/// mesmo NAS) nunca usam o mesmo temporario.
#[test]
fn temporary_are_unique_by_process_and_write() {
    let p = Path::new("/v/notas/a.md");
    let (a, b) = (temporary_name(p), temporary_name(p));
    assert_ne!(a, b);
    let n = a.file_name().unwrap().to_string_lossy().into_owned();
    assert!(n.starts_with(".a.md.") && n.ends_with(".tmp"), "{n}");
    assert!(n.contains(&std::process::id().to_string()), "{n}");
    assert_eq!(a.parent(), p.parent());
}

#[test]
fn create_note_with_file_appearing_in_the_middle_takes_the_next_id() {
    let d = tempfile::tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let foreign = d.path().join("notas/20261112-1432-herdeira.md");
    let race = |p: &Path| {
        if p == foreign && !p.exists() {
            fs::write(p, "da outra instancia").unwrap();
        }
    };
    let p = v.create_note_with("herdeira", img(), fr(), None, now(), &race).unwrap();
    assert_eq!(p.id, "20261112-1432-herdeira-2");
    assert_eq!(fs::read_to_string(&foreign).unwrap(), "da outra instancia");
    assert!(d.path().join("notas/20261112-1432-herdeira-2.md").is_file());
    let t = v.create_task_with("achar", None, None, now(), &|p: &Path| {
        let target = d.path().join("tarefas/20261112-1432-achar.md");
        if p == target && !p.exists() {
            fs::write(p, "outra").unwrap();
        }
    });
    assert_eq!(t.unwrap().id, "20261112-1432-achar-2");
}

// ---- registros ----

fn pos(file: &str, page: u32) -> Position {
    Position { file: file.into(), page }
}

#[test]
fn record_creates_list_and_le_of_back_with_start_and_no_end() {
    let d = tempfile::tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let r = v.create_record("Inventario de Joao", "caixa", pos("0012.jpg", 1), now()).unwrap();
    assert_eq!(r.id, "20261112-1432-inventario-de-joao");
    assert_eq!(r.end, None);
    let l = v.list_records().unwrap();
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].0, r);
    assert!(!fs::read_to_string(d.path().join("registros/20261112-1432-inventario-de-joao.md")).unwrap().contains("fim"));
}

#[test]
fn record_no_folder_records_list_empty_and_open_or_create_not_creates_the_folder() {
    let d = tempfile::tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    assert!(!d.path().join("registros").exists());
    assert!(v.list_records().unwrap().is_empty());
}

#[test]
fn id_of_the_record_repeats_with_suffix_in_the_collision() {
    let d = tempfile::tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let a = v.create_record("Inv", "f", pos("a.jpg", 1), now()).unwrap();
    let b = v.create_record("Inv", "f", pos("b.jpg", 1), now()).unwrap();
    assert_eq!(a.id, "20261112-1432-inv");
    assert_eq!(b.id, "20261112-1432-inv-2");
}

#[test]
fn name_of_record_empty_and_refused() {
    let d = tempfile::tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    assert!(matches!(v.create_record("   ", "f", pos("a.jpg", 1), now()), Err(Error::InvalidName(_))));
    assert!(v.list_records().unwrap().is_empty());
}

#[test]
fn update_record_preserves_body_and_fields_unknown_of_the_obsidian() {
    let d = tempfile::tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let r = v.create_record("Inv", "f", pos("a.jpg", 1), now()).unwrap();
    let path = d.path().join(format!("registros/{}.md", r.id));
    let txt = fs::read_to_string(&path).unwrap().replacen("---\n", "---\ntags: [x]\n", 2);
    // `tags` entra depois do primeiro `---` de abertura; o corpo livre vem depois do fecho
    let txt = format!("{}\nDescricao livre\n", txt.trim_end());
    fs::write(&path, txt).unwrap();
    let new = v.update_record(&r.id, |r| r.end = Some(pos("b.jpg", 2))).unwrap();
    assert_eq!(new.end, Some(pos("b.jpg", 2)));
    let after = fs::read_to_string(&path).unwrap();
    assert!(after.contains("tags"), "{after}");
    assert!(after.contains("Descricao livre"), "{after}");
    assert!(matches!(v.update_record("nao-existe", |_| {}), Err(Error::NotFound(_))));
}

#[test]
fn delete_record_remove_so_the_file() {
    let d = tempfile::tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let r = v.create_record("Inv", "f", pos("a.jpg", 1), now()).unwrap();
    v.create_note("x", img(), fr(), None, now()).unwrap();
    v.delete_record(&r.id).unwrap();
    assert!(v.list_records().unwrap().is_empty());
    assert_eq!(v.list_notes().unwrap().len(), 1);
    assert!(matches!(v.delete_record(&r.id), Err(Error::NotFound(_))));
}

#[test]
fn rename_source_rewrites_the_source_of_the_records() {
    let d = tempfile::tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    v.create_source("caixa", SourceKind::Folder).unwrap();
    let r = v.create_record("Inv", "caixa", pos("a.jpg", 1), now()).unwrap();
    let other = v.create_record("Outro", "livro", pos("a.jpg", 1), now()).unwrap();
    v.rename_source("caixa", "caixa2").unwrap();
    let l = v.list_records().unwrap();
    let by_id = |id: &str| l.iter().find(|(r, _)| r.id == id).unwrap().0.source.clone();
    assert_eq!(by_id(&r.id), "caixa2");
    assert_eq!(by_id(&other.id), "livro");
}

#[test]
fn rename_source_resumed_also_rewrites_os_records() {
    let d = tempfile::tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    v.create_source("caixa", SourceKind::Folder).unwrap();
    let r = v.create_record("Inv", "caixa", pos("a.jpg", 1), now()).unwrap();
    // simula parada: marca do novo ja escrita, fonte antiga ainda la, registro com a fonte antiga
    let from = d.path().join("fontes/caixa.md");
    let to = d.path().join("fontes/caixa2.md");
    let txt = fs::read_to_string(&from).unwrap().replace("name: caixa", "name: caixa2\nrenaming_from: caixa");
    fs::write(&to, txt).unwrap();
    assert!(v.interrupted_renaming("caixa", "caixa2"));
    v.rename_source("caixa", "caixa2").unwrap();
    assert_eq!(v.list_records().unwrap().iter().find(|(x, _)| x.id == r.id).unwrap().0.source, "caixa2");
}

#[test]
fn record_unreadable_not_breaks_the_reading_of_the_notes() {
    let d = tempfile::tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    v.create_note("x", img(), fr(), None, now()).unwrap();
    fs::create_dir_all(d.path().join("registros")).unwrap();
    fs::write(d.path().join("registros/quebrado.md"), "sem frontmatter").unwrap();
    assert!(matches!(v.list_records(), Err(Error::Document(..))));
    assert_eq!(v.list_notes().unwrap().len(), 1);
}

// ---- override de registro ----

#[test]
fn note_old_no_the_field_record_le_with_none() {
    let d = tempfile::tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let p = v.create_note("x", img(), fr(), None, now()).unwrap();
    assert_eq!(p.record, None);
    let t = v.create_task("y", None, None, now()).unwrap();
    assert_eq!(t.record, None);
    let txt = fs::read_to_string(d.path().join(format!("notas/{}.md", p.id))).unwrap();
    assert!(!txt.contains("registro"), "{txt}");
    assert_eq!(v.list_notes().unwrap()[0].0.record, None);
}

#[test]
fn set_record_of_writes_and_remove_the_field_preserving_the_body() {
    let d = tempfile::tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let p = v.create_note("corpo da nota", img(), fr(), None, now()).unwrap();
    let t = v.create_task("corpo da tarefa", None, None, now()).unwrap();
    v.set_record_of(DocKind::Note, &p.id, Some("reg-b")).unwrap();
    v.set_record_of(DocKind::Task, &t.id, Some("reg-b")).unwrap();
    let (np, note_body) = v.list_notes().unwrap().remove(0);
    assert_eq!(np.record.as_deref(), Some("reg-b"));
    assert_eq!(note_body.trim(), "corpo da nota");
    let (nt, note_body) = v.list_tasks().unwrap().remove(0);
    assert_eq!(nt.record.as_deref(), Some("reg-b"));
    assert_eq!(note_body.trim(), "corpo da tarefa");
    v.set_record_of(DocKind::Note, &p.id, None).unwrap();
    v.set_record_of(DocKind::Task, &t.id, None).unwrap();
    assert_eq!(v.list_notes().unwrap()[0].0.record, None);
    assert_eq!(v.list_tasks().unwrap()[0].0.record, None);
    assert!(!fs::read_to_string(d.path().join(format!("notas/{}.md", p.id))).unwrap().contains("registro"));
}

#[test]
fn set_record_of_id_missing_of_the_not_found() {
    let d = tempfile::tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    assert!(matches!(v.set_record_of(DocKind::Note, "nada", Some("r")), Err(Error::NotFound(_))));
    assert!(matches!(v.set_record_of(DocKind::Task, "../x", None), Err(Error::NotFound(_))));
}

#[test]
fn finish_id_and_update_text_preserve_the_field_record() {
    let d = tempfile::tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let r = v.create_draft("", img(), fr(), None, now()).unwrap();
    v.set_record_of(DocKind::Note, &r.id, Some("reg-b")).unwrap();
    v.update_text(&r.id, "herdeira Maria").unwrap();
    let new = v.finish_id(&r.id, now()).unwrap();
    let (p, _) = v.list_notes().unwrap().into_iter().find(|(p, _)| p.id == new).unwrap();
    assert_eq!(p.record.as_deref(), Some("reg-b"));
}

#[test]
fn list_records_readable_skips_the_file_broken() {
    let d = tempfile::tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let r = v.create_record("Bom", "f", pos("a.jpg", 1), now()).unwrap();
    fs::write(d.path().join("registros/quebrado.md"), "sem frontmatter").unwrap();
    assert!(v.list_records().is_err());
    let l = v.list_records_readable().unwrap();
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].0.id, r.id);
}
