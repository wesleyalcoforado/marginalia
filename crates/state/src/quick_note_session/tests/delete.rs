//! Excluir, editar e renomear fonte com a sessao em andamento.

use super::*;

#[test]
fn source_renamed_so_changes_as_notes_of_the_vault_renamed() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    s.set_paused(true);
    s.typed(&mut st, "Ana", t0);
    go_to_next(&mut st);
    assert!(s.observe(&mut st, ms(t0, 10)).clear_textbox, "a nota ainda por criar terminou (saindo)");
    s.renamed_source(&t.path().join("outro"), "Livro", "Errado");
    s.renamed_source(&root_dir(&st), "Livro", "Livro2");
    s.set_paused(false);
    let e = s.tick(&mut st, ms(t0, 100));
    s.run(&mut st, e, ms(t0, 100));
    assert_eq!(note_in(&st, "20261112-1432-ana").0.image.source, "Livro2");
}

// ---- Task 5: excluir de vez o item que a caixa edita ----

#[test]
fn delete_the_item_in_editing_ends_the_session_no_recreate_the_file() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "Maria herdeira", t0);
    s.run(&mut st, e, t0);
    assert_eq!(docs(&st, "notas"), [DRAFT]);
    // texto novo ainda nao gravado: nunca volta ao disco
    assert!(s.typed(&mut st, "Maria herdeira de Jose", ms(t0, 100)).work.is_none());
    let e = s.delete(&mut st, DocKind::Note, DRAFT, ms(t0, 200)).unwrap();
    assert!(e.clear_textbox, "a caixa limpa");
    assert!(e.reload);
    assert!(e.warning.as_deref().is_some_and(|a| a.contains("excluida")), "{:?}", e.warning);
    assert!(docs(&st, "notas").is_empty());
    assert!(!s.has_item() && !s.busy());
    for k in [1_000, 11_000, 60_000] {
        let e = s.tick(&mut st, ms(t0, k));
        assert!(e.work.is_none(), "{k}");
    }
    assert!(s.flush(&mut st).is_empty());
    assert!(docs(&st, "notas").is_empty(), "o texto nunca e gravado de volta");
    // a proxima tecla comeca uma nota nova
    let e = s.typed(&mut st, "Outra", ms(t0, 70_000));
    assert!(e.work.is_some());
}

#[test]
fn delete_the_item_in_editing_drops_the_highlight_that_it_reserved() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    st.commit_highlight(ret("g1")).unwrap();
    let e = s.typed(&mut st, "Maria", t0);
    s.run(&mut st, e, t0);
    assert_eq!(note_in(&st, DRAFT).0.highlight.as_deref(), Some("g1"));
    s.delete(&mut st, DocKind::Note, DRAFT, ms(t0, 100)).unwrap();
    // o grifo continua no JSON: voltar a ser pendente e ligar a nota nova
    let img = st.current.image.clone();
    st.restore_highlight_pending(&img, "g1");
    assert_eq!(st.pending_highlight(), Some(&ret("g1")));
    let e = s.typed(&mut st, "Joao", ms(t0, 200));
    s.run(&mut st, e, ms(t0, 200));
    let id = docs(&st, "notas").pop().expect("nota nova");
    assert_eq!(note_in(&st, &id).0.highlight.as_deref(), Some("g1"), "o grifo foi liberado");
}

#[test]
fn edit_loads_the_text_of_the_line_in_the_session() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let v = st.vault.clone().unwrap();
    let p = v.create_note("Texto da linha", st.current.image.clone(), st.framing().clone(), None, data()).unwrap();
    let tf = v.create_task("Tarefa da linha", None, None, data()).unwrap();
    let mut g = Drawers::default();
    let n = g.request_reload();
    g.receive(n, drawers::load(&v, &st.app_data));
    let mut s = session();
    let t0 = Instant::now();
    for (id, text) in [(&p.id, "Texto da linha"), (&tf.id, "Tarefa da linha")] {
        let kind = g.line_kind(id).unwrap();
        let e = s.load(&mut st, kind, id, t0);
        let e = s.run(&mut st, e, t0);
        assert_eq!(e.loaded.as_deref(), Some(text));
        assert_eq!(s.id_current(), Some(id.as_str()));
    }
}

#[test]
fn delete_with_write_in_flight_of_the_item_refuses_and_nothing_changes() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "Maria", t0);
    s.run(&mut st, e, t0);
    s.typed(&mut st, "Maria herdeira", ms(t0, 100));
    let tr = s.tick(&mut st, ms(t0, 1_000)).work.expect("grava");
    let r = s.delete(&mut st, DocKind::Note, DRAFT, ms(t0, 1_010));
    assert!(r.as_ref().is_err_and(|m| m.contains("aguarde")), "{:?}", r.as_ref().err());
    assert!(st.warning.as_deref().is_some_and(|a| a.contains("aguarde")));
    assert_eq!(docs(&st, "notas"), [DRAFT]);
    let e = s.conclude(&mut st, tr.execute(), ms(t0, 1_020));
    s.run(&mut st, e, ms(t0, 1_020));
    assert_eq!(note_in(&st, DRAFT).1, "Maria herdeira");
    // depois de gravar, excluir funciona
    s.delete(&mut st, DocKind::Note, DRAFT, ms(t0, 1_100)).unwrap();
    assert!(docs(&st, "notas").is_empty());
}

#[test]
fn delete_other_document_not_touches_in_the_session() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "Maria", t0);
    s.run(&mut st, e, t0);
    let v = st.vault.clone().unwrap();
    let other = v.create_note("outra", st.current.image.clone(), st.framing().clone(), None, data()).unwrap();
    let e = s.delete(&mut st, DocKind::Note, &other.id, ms(t0, 100)).unwrap();
    assert!(!e.clear_textbox);
    assert_eq!(s.id_current(), Some(DRAFT));
    assert_eq!(docs(&st, "notas"), [DRAFT]);
    // id que nao existe: erro, sessao intacta
    assert!(s.delete(&mut st, DocKind::Note, "nao-existe", ms(t0, 200)).is_err());
    assert!(st.warning.as_deref().is_some_and(|a| a.contains("nao-existe")));
    assert_eq!(s.id_current(), Some(DRAFT));
}

#[test]
fn delete_with_renaming_in_progress_refuses_and_the_session_follows() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "Maria", t0);
    s.run(&mut st, e, t0);
    continuation(&root_dir(&st), DRAFT, "20261112-1432-maria");
    let r = s.delete(&mut st, DocKind::Note, DRAFT, ms(t0, 100));
    assert!(r.as_ref().is_err_and(|m| m.contains("renomeação em curso")), "{:?}", r.as_ref().err());
    assert!(st.warning.as_deref().is_some_and(|a| a.contains("renomeação em curso")));
    assert!(s.has_item(), "a sessao continua com o item");
    assert_eq!(docs(&st, "notas").len(), 2);
}
