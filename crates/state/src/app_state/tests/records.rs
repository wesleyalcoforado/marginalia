use super::*;
use crate::app_state::test::with_vault;
use chrono::NaiveDate;
use marginalia_core::model::Position;

fn current_time() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 11, 18).unwrap().and_hms_opt(10, 5, 0).unwrap()
}

/// Vai para a imagem `n` (1-based) da fonte "Livro" de 3 quadros, carregando cada passo.
fn ir(st: &mut AppState, n: usize) {
    let target = format!("p{n:02}.png");
    for _ in 0..6 {
        if st.current.image.file == target {
            return;
        }
        let p = if st.current.image.file < target { st.go_next() } else { st.go_prev() };
        if let Some(p) = p {
            let _ = st.load_now(p);
        }
    }
    panic!("nao cheguei em {target}: estou em {}", st.current.image.file);
}

fn pos(n: usize) -> Position {
    Position { file: format!("p{n:02}.png"), page: 1 }
}

#[test]
fn no_vault_or_no_source_the_strip_and_none() {
    let t = tempfile::tempdir().unwrap();
    let st = AppState::new(None, AppData::new(t.path().join("appdata")), t.path().join("cache"));
    assert_eq!(st.record_strip(), None);
    let vault = Vault::open_or_create(&t.path().join("v")).unwrap();
    let st = AppState::new(Some(vault), AppData::new(t.path().join("appdata")), t.path().join("cache"));
    assert_eq!(st.record_strip(), None);
}

#[test]
fn outside_of_any_record_the_strip_and_outside() {
    let t = tempfile::tempdir().unwrap();
    let st = with_vault(t.path(), "v");
    assert_eq!(st.record_strip(), Some(RecordStrip::Outside));
}

#[test]
fn mark_start_creates_the_record_no_end_and_the_strip_shows_no_end() {
    let t = tempfile::tempdir().unwrap();
    let mut st = with_vault(t.path(), "v");
    ir(&mut st, 2);
    let r = st.mark_start_record("  Inventario de Joao ", current_time()).unwrap();
    assert_eq!((r.name.as_str(), r.source.as_str(), r.end.clone()), ("Inventario de Joao", "Livro", None));
    assert_eq!(r.start, pos(2));
    assert!(st.confirmation.is_some());
    match st.record_strip() {
        Some(RecordStrip::Em { record, shared, .. }) => {
            assert_eq!(record.id, r.id);
            assert_eq!(record.end, None);
            assert!(!shared);
        }
        o => panic!("{o:?}"),
    }
    // antes do inicio continua fora
    ir(&mut st, 1);
    assert_eq!(st.record_strip(), Some(RecordStrip::Outside));
}

#[test]
fn mark_start_inside_of_record_with_end_warns_and_not_writes() {
    let t = tempfile::tempdir().unwrap();
    let mut st = with_vault(t.path(), "v");
    let a = st.mark_start_record("A", current_time()).unwrap();
    ir(&mut st, 3);
    st.mark_end_record(&a.id).unwrap();
    ir(&mut st, 2);
    let e = st.mark_start_record("B", current_time()).unwrap_err();
    assert!(e.contains("\"A\""), "{e}");
    assert_eq!(st.warning.as_deref(), Some(e.as_str()));
    assert_eq!(st.vault.as_ref().unwrap().list_records().unwrap().len(), 1);
}

#[test]
fn name_empty_and_refused_in_mark_and_in_rename() {
    let t = tempfile::tempdir().unwrap();
    let mut st = with_vault(t.path(), "v");
    assert!(st.mark_start_record("   ", current_time()).is_err());
    let a = st.mark_start_record("A", current_time()).unwrap();
    assert!(st.rename_record(&a.id, " ").is_err());
    assert_eq!(st.vault.as_ref().unwrap().list_records().unwrap()[0].0.name, "A");
}

#[test]
fn mark_end_pins_the_end_in_the_image_current() {
    let t = tempfile::tempdir().unwrap();
    let mut st = with_vault(t.path(), "v");
    let a = st.mark_start_record("A", current_time()).unwrap();
    ir(&mut st, 2);
    st.mark_end_record(&a.id).unwrap();
    assert_eq!(st.vault.as_ref().unwrap().list_records().unwrap()[0].0.end, Some(pos(2)));
    ir(&mut st, 3);
    assert_eq!(st.record_strip(), Some(RecordStrip::Outside));
}

#[test]
fn mark_end_before_of_the_start_refuses() {
    let t = tempfile::tempdir().unwrap();
    let mut st = with_vault(t.path(), "v");
    ir(&mut st, 2);
    let a = st.mark_start_record("A", current_time()).unwrap();
    ir(&mut st, 1);
    assert!(st.mark_end_record(&a.id).is_err());
    assert_eq!(st.vault.as_ref().unwrap().list_records().unwrap()[0].0.end, None);
}

#[test]
fn adjust_end_in_record_already_closed_swap_the_end() {
    let t = tempfile::tempdir().unwrap();
    let mut st = with_vault(t.path(), "v");
    let a = st.mark_start_record("A", current_time()).unwrap();
    ir(&mut st, 2);
    st.mark_end_record(&a.id).unwrap();
    ir(&mut st, 3);
    st.mark_end_record(&a.id).unwrap();
    assert_eq!(st.vault.as_ref().unwrap().list_records().unwrap()[0].0.end, Some(pos(3)));
}

#[test]
fn discard_record_not_erases_notes_nor_tasks() {
    let t = tempfile::tempdir().unwrap();
    let mut st = with_vault(t.path(), "v");
    let a = st.mark_start_record("A", current_time()).unwrap();
    let v = st.vault.clone().unwrap();
    v.create_note("x", st.current.image.clone(), marginalia_core::model::Framing { zoom: 1.0, cx: 0.5, cy: 0.5 }, None, current_time()).unwrap();
    v.create_task("y", Some(st.current.image.clone()), None, current_time()).unwrap();
    st.discard_record(&a.id).unwrap();
    assert!(v.list_records().unwrap().is_empty());
    assert_eq!((v.list_notes().unwrap().len(), v.list_tasks().unwrap().len()), (1, 1));
    assert_eq!(st.record_strip(), Some(RecordStrip::Outside));
}

#[test]
fn rename_record_changes_so_the_name() {
    let t = tempfile::tempdir().unwrap();
    let mut st = with_vault(t.path(), "v");
    let a = st.mark_start_record("A", current_time()).unwrap();
    st.rename_record(&a.id, "Novo nome").unwrap();
    let (r, _) = st.vault.as_ref().unwrap().list_records().unwrap().remove(0);
    assert_eq!((r.id, r.name), (a.id, "Novo nome".to_string()));
}

/// A em p01..p02 e B em p02..p03: a imagem p02 e a fronteira.
fn with_boundary(st: &mut AppState) -> (Record, Record) {
    let a = st.mark_start_record("A", current_time()).unwrap();
    ir(st, 2);
    st.mark_end_record(&a.id).unwrap();
    let b = st.mark_start_record("B", current_time()).unwrap();
    ir(st, 3);
    st.mark_end_record(&b.id).unwrap();
    (a, b)
}

#[test]
fn image_shared_mark_the_strip_and_relocate_writes_the_override() {
    let t = tempfile::tempdir().unwrap();
    let mut st = with_vault(t.path(), "v");
    let (a, b) = with_boundary(&mut st);
    ir(&mut st, 2);
    match st.record_strip() {
        Some(RecordStrip::Em { record, shared, .. }) => assert_eq!((record.id, shared), (a.id.clone(), true)),
        o => panic!("{o:?}"),
    }
    let v = st.vault.clone().unwrap();
    let p = v.create_note("x", st.current.image.clone(), marginalia_core::model::Framing { zoom: 1.0, cx: 0.5, cy: 0.5 }, None, current_time()).unwrap();
    st.relocate_to_record(DocKind::Note, &p.id, Some(&b.id)).unwrap();
    assert_eq!(v.list_notes().unwrap()[0].0.record.as_deref(), Some(b.id.as_str()));
    st.relocate_to_record(DocKind::Note, &p.id, None).unwrap();
    assert_eq!(v.list_notes().unwrap()[0].0.record, None);
    // destino que nao contem a imagem
    assert!(st.relocate_to_record(DocKind::Note, &p.id, Some("fantasma")).is_err());
}

#[test]
fn relocate_in_image_not_shared_and_refused() {
    let t = tempfile::tempdir().unwrap();
    let mut st = with_vault(t.path(), "v");
    let (_, b) = with_boundary(&mut st);
    ir(&mut st, 3);
    let v = st.vault.clone().unwrap();
    let p = v.create_note("x", st.current.image.clone(), marginalia_core::model::Framing { zoom: 1.0, cx: 0.5, cy: 0.5 }, None, current_time()).unwrap();
    assert!(st.relocate_to_record(DocKind::Note, &p.id, Some(&b.id)).is_err());
    assert_eq!(v.list_notes().unwrap()[0].0.record, None);
}

#[test]
fn record_invalid_becomes_strip_invalid() {
    let t = tempfile::tempdir().unwrap();
    let mut st = with_vault(t.path(), "v");
    ir(&mut st, 3);
    let a = st.mark_start_record("A", current_time()).unwrap();
    // editado a mao: fim antes do inicio
    st.vault.as_ref().unwrap().update_record(&a.id, |r| r.end = Some(pos(1))).unwrap();
    st.reload_records();
    match st.record_strip() {
        Some(RecordStrip::Invalid { record }) => assert_eq!(record.id, a.id),
        o => panic!("{o:?}"),
    }
    ir(&mut st, 2);
    assert_eq!(st.record_strip(), Some(RecordStrip::Outside));
}

#[test]
fn failure_to_the_write_warns_and_not_changes_the_state() {
    let t = tempfile::tempdir().unwrap();
    let mut st = with_vault(t.path(), "v");
    // `records` e um arquivo: nao da para criar a pasta
    std::fs::write(t.path().join("v/registros"), "x").unwrap();
    let e = st.mark_start_record("A", current_time()).unwrap_err();
    assert_eq!(st.warning.as_deref(), Some(e.as_str()));
    assert_eq!(st.record_strip(), Some(RecordStrip::Outside));
    assert!(st.records().is_empty());
}

#[test]
fn file_unreadable_in_records_not_locks_as_operations() {
    let t = tempfile::tempdir().unwrap();
    let mut st = with_vault(t.path(), "v");
    std::fs::create_dir_all(t.path().join("v/registros")).unwrap();
    std::fs::write(t.path().join("v/registros/quebrado.md"), "sem frontmatter").unwrap();
    let a = st.mark_start_record("A", current_time()).expect("marcar inicio com um arquivo quebrado ao lado");
    ir(&mut st, 2);
    st.mark_end_record(&a.id).unwrap();
}

#[test]
fn outside_of_the_record_offers_extend_the_that_finished_before() {
    let t = tempfile::tempdir().unwrap();
    let mut st = with_vault(t.path(), "v");
    assert!(st.record_to_extend().is_none());
    let a = st.mark_start_record("A", current_time()).unwrap();
    ir(&mut st, 2);
    st.mark_end_record(&a.id).unwrap();
    ir(&mut st, 3);
    assert_eq!(st.record_strip(), Some(RecordStrip::Outside));
    assert_eq!(st.record_to_extend().map(|r| r.id), Some(a.id.clone()));
    st.mark_end_record(&a.id).unwrap();
    assert_eq!(st.vault.as_ref().unwrap().list_records().unwrap()[0].0.end, Some(pos(3)));
}
