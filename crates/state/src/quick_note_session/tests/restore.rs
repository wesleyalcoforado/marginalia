//! Restaurar o registro ao abrir e adotar o que ja esta no vault.

use super::*;

#[test]
fn restore_rewrites_the_text_of_note_existing() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "Maria", t0);
    s.run(&mut st, e, t0);
    s.typed(&mut st, "Maria Rita", ms(t0, 100));
    s.set_delay(Some(Arc::new(|| std::thread::sleep(Duration::from_millis(300)))));
    s.flush_with_deadline(&mut st, Duration::from_millis(50));
    let records = recovery_read(&rec(&st)).unwrap();
    assert_eq!((records[0].id.as_deref(), records[0].text.as_str()), (Some(DRAFT), "Maria Rita"));
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(note_in(&st, DRAFT).1, "Maria");
    let mut s2 = session();
    let e = s2.restore(&mut st, t0);
    s2.run(&mut st, e, t0);
    assert_eq!(docs(&st, "notas"), ["20261112-1432-maria-rita"]);
    assert_eq!(note_in(&st, "20261112-1432-maria-rita").1, "Maria Rita");
    assert!(!rec(&st).exists());
}

#[test]
fn restore_with_vault_unavailable_keeps_the_record_and_shows_the_marker() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let record = Record {
        key: "x-1".into(),
        vault: t.path().join("nas-desmontado"),
        kind: "note".into(),
        id: None,
        text: "Texto do NAS".into(),
        image: Some(st.current.image.clone()),
        // (um registro sem enquadramento volta com o padrao)
        framing: Some(crate::viewer_math::default_framing()),
        highlight: None,
        lost: false,
        no_disk: Vec::new(),
    };
    write_recovery(&rec(&st), std::slice::from_ref(&record)).unwrap();
    let mut s = session();
    let t0 = Instant::now();
    let e = s.restore(&mut st, t0);
    // a pasta do vault (talvez no NAS) e conferida no trabalho, fora da interface
    assert!(e.work.is_some(), "restaurar nao toca o vault na thread da interface");
    s.run(&mut st, e, t0);
    assert!(s.marker().is_some_and(|m| m.contains("recuperacao")), "{:?}", s.marker());
    assert!(s.tick(&mut st, ms(t0, 500)).work.is_none(), "tenta de novo mais tarde");
    assert!(s.tick(&mut st, ms(t0, 1_100)).work.is_some());
    assert_eq!(recovery_read(&rec(&st)).unwrap(), [record.clone()], "nunca e descartado");
    // uma nota nova e o flush nao apagam o registro antigo
    let e = s.typed(&mut st, "Outra", ms(t0, 10));
    s.run(&mut st, e, ms(t0, 10));
    s.flush(&mut st);
    assert_eq!(recovery_read(&rec(&st)).unwrap(), [record]);
    assert_eq!(docs(&st, "notas"), ["20261112-1432-outra"]);
}

#[test]
fn restore_finds_the_draft_already_created_and_not_duplicates() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let v = st.vault.clone().unwrap();
    // a criacao chegou ao disco, mas o app caiu antes de o registro saber o id
    v.create_draft("Maria", st.current.image.clone(), st.framing().clone(), None, data()).unwrap();
    write_recovery(&rec(&st), &[record(&st, "x-1", None, "Maria")]).unwrap();
    let mut s = session();
    let t0 = Instant::now();
    let e = s.restore(&mut st, t0);
    s.run(&mut st, e, t0);
    assert_eq!(docs(&st, "notas"), ["20261112-1432-maria"], "nenhuma nota duplicada");
    assert!(!rec(&st).exists());
}

#[test]
fn restore_not_overwrites_text_more_new_in_the_disk() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let v = st.vault.clone().unwrap();
    let p = v.create_note("Maria", st.current.image.clone(), st.framing().clone(), None, data()).unwrap();
    let mut record = record(&st, "x-1", Some(&p.id), "Maria Rita");
    record.no_disk = vec!["Maria".into()];
    write_recovery(&rec(&st), std::slice::from_ref(&record)).unwrap();
    v.update_text_in(DocKind::Note, &p.id, "Maria editada no Obsidian").unwrap();
    let mut s = session();
    let t0 = Instant::now();
    let e = s.restore(&mut st, t0);
    s.run(&mut st, e, t0);
    assert_eq!(note_in(&st, &p.id).1, "Maria editada no Obsidian", "o disco mais novo fica");
    assert_eq!(recovery_read(&rec(&st)).unwrap()[0].text, "Maria Rita", "o registro fica");
    assert!(s.marker().is_some_and(|m| m.contains("mudou")), "{:?}", s.marker());
    assert!(s.tick(&mut st, ms(t0, 60_000)).work.is_none(), "nao tenta de novo sozinho");
    // com o disco como a sessao o deixou, o registro vai para o vault
    v.update_text_in(DocKind::Note, &p.id, "Maria").unwrap();
    let mut s2 = session();
    let e = s2.restore(&mut st, t0);
    s2.run(&mut st, e, t0);
    assert_eq!(note_in(&st, &p.id).1, "Maria Rita");
    assert!(!rec(&st).exists());
}

#[test]
fn restore_not_relinks_highlight_that_other_note_already_uses() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    st.commit_highlight(ret("g1")).unwrap();
    let v = st.vault.clone().unwrap();
    v.create_note("Outra", st.current.image.clone(), st.framing().clone(), Some("g1".into()), data()).unwrap();
    let mut record = record(&st, "x-1", None, "Maria");
    record.highlight = Some("g1".into());
    write_recovery(&rec(&st), &[record]).unwrap();
    let mut s = session();
    let t0 = Instant::now();
    let e = s.restore(&mut st, t0);
    s.run(&mut st, e, t0);
    let maria: Vec<Note> =
        v.list_notes().unwrap().into_iter().filter(|(_, c)| c == "Maria").map(|(p, _)| p).collect();
    assert_eq!(maria.len(), 1);
    assert_eq!(maria[0].highlight, None, "o grifo ja e de outra nota");
}

#[test]
fn restore_finds_the_note_that_the_work_late_left() {
    let img = |st: &AppState| (st.current.image.clone(), st.framing().clone());
    // o trabalho atrasado criou o rascunho com o corpo antigo
    let r = restore_about(|v, st| { let (i, e) = img(st); v.create_draft("M", i, e, None, data()).unwrap(); }, &["M"]);
    assert_eq!(r, [("20261112-1432-maria".to_string(), "Maria".to_string())], "uma nota so");
    // ... e ja fixou o id (nao e mais rascunho)
    let r = restore_about(|v, st| { let (i, e) = img(st); v.create_note("M", i, e, None, data()).unwrap(); }, &["M"]);
    assert_eq!(r, [("20261112-1432-m".to_string(), "Maria".to_string())]);
    // rascunho com um comeco do texto (sem corpo conhecido)
    let r = restore_about(|v, st| { let (i, e) = img(st); v.create_draft("Mar", i, e, None, data()).unwrap(); }, &[]);
    assert_eq!(r, [("20261112-1432-maria".to_string(), "Maria".to_string())]);
    // uma nota fixada de outra nota com um comeco igual nao e tomada
    let r = restore_about(|v, st| { let (i, e) = img(st); v.create_note("Mar", i, e, None, data()).unwrap(); }, &[]);
    assert_eq!(r.len(), 2, "{r:?}");
}

#[test]
fn restore_with_the_body_late_in_the_disk_not_and_conflict() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let v = st.vault.clone().unwrap();
    let p = v.create_note("Maria", st.current.image.clone(), st.framing().clone(), None, data()).unwrap();
    // a gravacao em voo na saida ("Maria R") chegou depois do prazo
    v.update_text_in(DocKind::Note, &p.id, "Maria R").unwrap();
    let mut record = record(&st, "x-1", Some(&p.id), "Maria Rita");
    record.no_disk = vec!["Maria".into(), "Maria R".into()];
    write_recovery(&rec(&st), &[record]).unwrap();
    let mut s = session();
    let t0 = Instant::now();
    let e = s.restore(&mut st, t0);
    s.run(&mut st, e, t0);
    assert_eq!(note_in(&st, &p.id).1, "Maria Rita");
    assert_eq!(s.marker(), None);
    assert!(!rec(&st).exists());
}

#[test]
fn restore_no_body_known_compares_before_of_write() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let v = st.vault.clone().unwrap();
    let p = v.create_note("Maria", st.current.image.clone(), st.framing().clone(), None, data()).unwrap();
    v.update_text_in(DocKind::Note, &p.id, "Maria editada").unwrap();
    // registro da rodada 1 (sem `no_disk`)
    write_recovery(&rec(&st), &[record(&st, "x-1", Some(&p.id), "Maria Rita")]).unwrap();
    let mut s = session();
    let t0 = Instant::now();
    let e = s.restore(&mut st, t0);
    s.run(&mut st, e, t0);
    assert_eq!(note_in(&st, &p.id).1, "Maria editada", "nunca grava as cegas");
    assert!(s.marker().is_some_and(|m| m.contains("mudou")), "{:?}", s.marker());
    assert!(rec(&st).exists());
}

#[test]
fn restore_with_document_unreadable_not_writes_as_blind() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let v = st.vault.clone().unwrap();
    let p = v.create_note("Maria", st.current.image.clone(), st.framing().clone(), None, data()).unwrap();
    let fname = root_dir(&st).join(format!("notas/{}.md", p.id));
    let good = std::fs::read_to_string(&fname).unwrap();
    std::fs::write(&fname, "---\nid: [quebrado\n---\nMaria").unwrap();
    let mut record = record(&st, "x-1", Some(&p.id), "Maria Rita");
    record.no_disk = vec!["Maria".into()];
    write_recovery(&rec(&st), &[record]).unwrap();
    let mut s = session();
    let t0 = Instant::now();
    let e = s.restore(&mut st, t0);
    s.run(&mut st, e, t0);
    assert_eq!(std::fs::read_to_string(&fname).unwrap(), "---\nid: [quebrado\n---\nMaria", "nao reescreve o que nao leu");
    assert!(rec(&st).exists());
    // o documento volta a ser legivel: a tentativa seguinte grava
    std::fs::write(&fname, good).unwrap();
    let e = s.tick(&mut st, ms(t0, 1_100));
    s.run(&mut st, e, ms(t0, 1_100));
    assert_eq!(note_in(&st, &p.id).1, "Maria Rita");
    assert!(!rec(&st).exists());
}

#[test]
fn point_of_assembly_empty_not_becomes_vault() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let assembly = t.path().join("nas");
    std::fs::create_dir(&assembly).unwrap();
    let mut record = record(&st, "x-1", None, "Texto do NAS");
    record.vault = assembly.clone();
    write_recovery(&rec(&st), &[record]).unwrap();
    let mut s = session();
    let t0 = Instant::now();
    let e = s.restore(&mut st, t0);
    s.run(&mut st, e, t0);
    assert_eq!(std::fs::read_dir(&assembly).unwrap().count(), 0, "nada criado no disco local");
    assert!(s.marker().is_some());
    assert!(rec(&st).exists());
}

#[test]
fn note_pinned_of_other_quick_note_not_and_adopted() {
    let other_framing = Framing { zoom: 3.0, cx: 0.2, cy: 0.8 };
    for text in ["Mar", "Maria Rita"] {
        // mesma imagem e mesmo corpo conhecido, mas outro enquadramento: e outra nota
        let r = restore_record(
            |v, st| {
                v.create_note("Maria", st.current.image.clone(), other_framing.clone(), None, data()).unwrap();
            },
            text,
            &["Maria"],
            None,
        );
        let mut expected = vec!["Maria", text];
        expected.sort();
        assert_eq!(bodies(&r), expected, "a outra nota fica com \"Maria\" e o registro vira nota nova");
        // mesmo enquadramento, outro grifo: tambem e outra nota
        let r = restore_record(
            |v, st| {
                st.commit_highlight(ret("g9")).unwrap();
                v.create_note("Maria", st.current.image.clone(), st.framing().clone(), Some("g9".into()), data()).unwrap();
            },
            text,
            &["Maria"],
            None,
        );
        assert_eq!(bodies(&r), expected);
    }
}

#[test]
fn two_candidates_equal_none_and_adopted() {
    let r = restore_record(
        |v, st| {
            for _ in 0..2 {
                v.create_draft("M", st.current.image.clone(), st.framing().clone(), None, data()).unwrap();
            }
        },
        "Maria",
        &["M"],
        None,
    );
    assert_eq!(bodies(&r), ["M", "M", "Maria"], "na duvida cria nova e nao sobrescreve nenhuma");
}

#[test]
fn draft_with_the_same_highlight_and_adopted_and_with_other_not() {
    let with = |g: &'static str| {
        move |v: &Vault, st: &mut AppState| {
            st.commit_highlight(ret(g)).unwrap();
            v.create_draft("Mar", st.current.image.clone(), st.framing().clone(), Some(g.into()), data()).unwrap();
        }
    };
    assert_eq!(bodies(&restore_record(with("g1"), "Maria", &[], Some("g1"))), ["Maria"], "o proprio rascunho");
    assert_eq!(bodies(&restore_record(with("g2"), "Maria", &[], None)), ["Mar", "Maria"]);
}

#[test]
fn adopted_that_changes_before_of_write_and_conflict() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let v = st.vault.clone().unwrap();
    let p = v.create_draft("M", st.current.image.clone(), st.framing().clone(), None, data()).unwrap();
    let mut record = record(&st, "x-1", None, "Maria");
    record.no_disk = vec!["M".into()];
    write_recovery(&rec(&st), &[record]).unwrap();
    let mut s = session();
    let t0 = Instant::now();
    let tr = s.restore(&mut st, t0).work.unwrap();
    let tr = s.conclude(&mut st, tr.execute(), t0).work.expect("adotado: falta gravar o texto");
    // editado por fora entre a adocao e a gravacao
    v.update_text_in(DocKind::Note, &p.id, "M editado").unwrap();
    s.run(&mut st, Effect { work: Some(tr), ..Effect::default() }, t0);
    assert_eq!(note_in(&st, &p.id).1, "M editado", "a gravacao seguinte compara antes");
    assert!(s.marker().is_some_and(|m| m.contains("mudou")), "{:?}", s.marker());
    assert!(rec(&st).exists());
}
