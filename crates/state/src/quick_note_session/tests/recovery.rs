//! Registro de recuperacao: flush com vault lento, nota perdida, registro corrompido.

use super::*;

#[test]
fn failures_consecutive_wait_each_time_more_and_the_warning_not_se_repeats() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let notes = root_dir(&st).join("notas");
    std::fs::remove_dir_all(&notes).unwrap();
    std::fs::write(&notes, "nao e pasta").unwrap();
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "Maria", t0);
    assert!(s.run(&mut st, e, t0).warning.is_some());
    assert!(s.tick(&mut st, ms(t0, 900)).work.is_none());
    let e = s.tick(&mut st, ms(t0, 1_000));
    assert!(e.work.is_some(), "1 s depois da 1a falha");
    let e = s.run(&mut st, e, ms(t0, 1_000));
    assert!(e.warning.is_none(), "mesma mensagem: o aviso nao e posto de novo");
    let e = s.tick(&mut st, ms(t0, 2_900));
    assert!(e.work.is_none(), "2 s depois da 2a falha");
    assert!(e.tick.is_some_and(|d| d <= Duration::from_millis(100)), "{:?}", e.tick);
    let e = s.tick(&mut st, ms(t0, 3_000));
    assert!(e.work.is_some());
    s.run(&mut st, e, ms(t0, 3_000));
    let e = s.tick(&mut st, ms(t0, 6_900));
    assert!(e.work.is_none(), "4 s depois da 3a");
    assert_eq!(wait(1), Duration::from_secs(1));
    assert_eq!(wait(4), Duration::from_secs(8));
    assert_eq!(wait(6), RETRY_MAX);
    assert_eq!(wait(40), RETRY_MAX);
}

#[test]
fn flush_with_vault_slow_leaves_the_record_and_the_work_late_not_writes() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    st.commit_highlight(ret("g1")).unwrap();
    let mut s = session();
    s.set_delay(Some(Arc::new(|| std::thread::sleep(Duration::from_millis(400)))));
    let t0 = Instant::now();
    let tr = s.typed(&mut st, "Maria", t0).work.unwrap();
    // o executor de fundo esta rodando a criacao (lenta)
    let background = std::thread::spawn(move || tr.execute());
    let start = Instant::now();
    let warnings = s.flush_with_deadline(&mut st, Duration::from_millis(100));
    assert!(start.elapsed() < Duration::from_millis(350), "o flush nao espera o vault: {:?}", start.elapsed());
    assert!(warnings.iter().any(|a| a.contains("registro de recuperacao")), "{warnings:?}");
    let records = recovery_read(&rec(&st)).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!((records[0].text.as_str(), records[0].id.as_deref(), records[0].kind.as_str()), ("Maria", None, "note"));
    assert_eq!(records[0].vault, root_dir(&st));
    assert_eq!(records[0].image.as_ref(), Some(&st.current.image));
    assert_eq!(records[0].highlight.as_deref(), Some("g1"));
    // os trabalhos atrasados terminam depois e nao gravam (foram cancelados)
    assert!(background.join().unwrap().cancelled);
    std::thread::sleep(Duration::from_millis(500));
    assert!(docs(&st, "notas").is_empty());
    assert!(!s.busy());

    // proxima abertura: o registro volta e vai para o vault
    let mut s2 = session();
    let e = s2.restore(&mut st, t0);
    assert!(e.work.is_some());
    s2.run(&mut st, e, t0);
    assert_eq!(docs(&st, "notas"), ["20261112-1432-maria"]);
    let (p, note_body) = note_in(&st, "20261112-1432-maria");
    assert_eq!((p.highlight.as_deref(), note_body.as_str()), (Some("g1"), "Maria"));
    assert!(p.extra.get("draft").is_none(), "id fixado");
    assert!(!rec(&st).exists(), "o registro sai quando o texto esta no disco");
    assert_eq!(s2.marker(), None);
}

#[test]
fn flush_with_renaming_of_source_so_writes_the_record() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    s.set_paused(true);
    let e = s.typed(&mut st, "Maria", t0);
    assert!(e.work.is_none(), "pausada: nada vai ao vault");
    let warnings = s.flush(&mut st);
    assert!(warnings.iter().any(|a| a.contains("renomeacao")), "{warnings:?}");
    assert!(docs(&st, "notas").is_empty());
    let records = recovery_read(&rec(&st)).unwrap();
    assert_eq!(records[0].text, "Maria");
}

#[test]
fn quick_note_lost_goes_to_the_record_and_so_the_user_recreates() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "Maria", t0);
    s.run(&mut st, e, t0);
    std::fs::remove_file(root_dir(&st).join(format!("notas/{DRAFT}.md"))).unwrap();
    s.typed(&mut st, "Maria Rita", ms(t0, 100));
    let e = s.tick(&mut st, ms(t0, 700));
    let e = s.run(&mut st, e, ms(t0, 700));
    assert!(e.warning.is_some_and(|a| a.contains("sumiu")));
    assert!(docs(&st, "notas").is_empty(), "regra 3: nao recria sozinha");
    assert!(s.marker().is_some_and(|m| m.contains("sumiu")), "marcador persistente");
    assert!(s.can_recreate());
    assert_eq!(recovery_read(&rec(&st)).unwrap()[0].text, "Maria Rita");
    // continuar digitando atualiza o registro (com debounce)
    let e = s.typed(&mut st, "Maria Rita filha", ms(t0, 800));
    assert!(e.work.is_none() && e.tick.is_some());
    assert_eq!(recovery_read(&rec(&st)).unwrap()[0].text, "Maria Rita");
    s.tick(&mut st, ms(t0, 1_400));
    assert_eq!(recovery_read(&rec(&st)).unwrap()[0].text, "Maria Rita filha");
    assert!(s.tick(&mut st, ms(t0, 30_000)).work.is_none(), "nada e gravado no vault");
    assert!(s.marker().is_some());
    // o usuario pede: vira uma nota nova no quadro atual
    let e = s.recreate_as_new_note(&mut st, ms(t0, 31_000));
    assert!(e.work.is_some());
    s.run(&mut st, e, ms(t0, 31_000));
    assert_eq!(docs(&st, "notas"), [DRAFT]);
    assert_eq!(note_in(&st, DRAFT).1, "Maria Rita filha");
    assert_eq!(s.marker(), None);
    assert!(!rec(&st).exists());
}

#[test]
fn record_of_recovery_and_atomic_and_empty_erases() {
    let t = tempdir().unwrap();
    let fname = recovery_file(&t.path().join("dados"));
    assert_eq!(recovery_read(&fname).unwrap(), []);
    let r = Record {
        key: "a".into(),
        vault: t.path().into(),
        kind: "tarefa".into(),
        id: Some("x".into()),
        text: "y".into(),
        image: None,
        framing: Some(Framing { zoom: 1.0, cx: 0.5, cy: 0.5 }),
        highlight: None,
        lost: true,
        no_disk: vec!["z".into()],
    };
    write_recovery(&fname, std::slice::from_ref(&r)).unwrap();
    assert_eq!(recovery_read(&fname).unwrap(), [r]);
    let tmp = temporary_file(&fname);
    assert!(tmp.to_string_lossy().contains(&std::process::id().to_string()), "temporario por processo: {tmp:?}");
    let names: Vec<String> =
        std::fs::read_dir(fname.parent().unwrap()).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, [RECOVERY_FILE]);
    write_recovery(&fname, &[]).unwrap();
    assert!(!fname.exists());
    std::fs::write(&fname, "{ quebrado").unwrap();
    assert!(recovery_read(&fname).is_err(), "ilegivel nao vira vazio");
}

#[test]
fn second_flush_after_of_the_deadline_not_erases_the_record() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    s.set_delay(Some(Arc::new(|| std::thread::sleep(Duration::from_millis(300)))));
    let t0 = Instant::now();
    let _in_flight = s.typed(&mut st, "Maria", t0).work.unwrap();
    s.flush_with_deadline(&mut st, Duration::from_millis(50));
    assert_eq!(recovery_read(&rec(&st)).unwrap()[0].text, "Maria");
    // on_release (janela) e on_app_quit (app): o segundo flush
    assert!(s.flush(&mut st).is_empty());
    let records = recovery_read(&rec(&st)).unwrap();
    assert_eq!(records.len(), 1, "o segundo flush nao apaga o registro");
    assert_eq!(records[0].text, "Maria");
    std::thread::sleep(Duration::from_millis(400));
}

#[test]
fn record_of_other_session_not_and_erased() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let other = record(&st, "outra-1", None, "da outra janela");
    write_recovery(&rec(&st), std::slice::from_ref(&other)).unwrap();
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "Ana", t0);
    s.run(&mut st, e, t0);
    assert!(s.flush(&mut st).is_empty());
    assert_eq!(recovery_read(&rec(&st)).unwrap(), [other], "so as chaves desta sessao saem");
}

#[test]
fn record_corrupt_and_isolated_and_never_overwritten() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let base = st.app_data.base().to_path_buf();
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(rec(&st), "{ quebrado").unwrap();
    let mut s = session();
    let t0 = Instant::now();
    s.restore(&mut st, t0);
    assert!(s.marker().is_some_and(|m| m.contains("corromp")), "{:?}", s.marker());
    let isolated = |b: &Path| -> Vec<PathBuf> {
        std::fs::read_dir(b)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with("recuperacao.json.corrompido-"))
            .collect()
    };
    let iso = isolated(&base);
    assert_eq!(iso.len(), 1, "{:?}", std::fs::read_dir(&base).unwrap().flatten().map(|e| e.file_name()).collect::<Vec<_>>());
    assert_eq!(std::fs::read_to_string(&iso[0]).unwrap(), "{ quebrado");
    // gravacoes seguintes do registro nao tocam no isolado
    s.set_delay(Some(Arc::new(|| std::thread::sleep(Duration::from_millis(300)))));
    s.typed(&mut st, "Ana", ms(t0, 10));
    assert!(s.marker().is_some_and(|m| m.contains("corromp")), "marcador persistente");
    s.flush_with_deadline(&mut st, Duration::from_millis(50));
    assert_eq!(recovery_read(&rec(&st)).unwrap()[0].text, "Ana");
    assert_eq!(std::fs::read_to_string(&iso[0]).unwrap(), "{ quebrado");
    std::thread::sleep(Duration::from_millis(400));
}

#[test]
fn record_follows_the_id_new_of_the_item() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    lost(&mut st, &mut s, t0);
    let tr = s.recreate_as_new_note(&mut st, ms(t0, 1_000)).work.unwrap();
    // digitado enquanto a nota nova e criada: o texto ainda nao esta seguro
    s.typed(&mut st, "Maria Rita filha", ms(t0, 1_010));
    s.conclude(&mut st, tr.execute(), ms(t0, 1_020));
    let records = recovery_read(&rec(&st)).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id.as_deref(), Some(DRAFT), "o registro sabe o id novo: restaurar nao duplica");
    assert_eq!(records[0].text, "Maria Rita filha");
    assert!(!records[0].lost);
}

#[test]
fn flush_keeps_in_the_record_the_body_that_was_in_the_disk() {
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
    // o confirmado e o do trabalho que estava em voo no prazo
    assert_eq!(records[0].no_disk, ["Maria", "Maria Rita"]);
    std::thread::sleep(Duration::from_millis(400));
}

#[test]
fn erase_and_retype_before_of_the_erase_run_takes_the_highlight() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    st.commit_highlight(ret("g1")).unwrap();
    let mut s = session();
    let t0 = Instant::now();
    let tr = s.typed(&mut st, "M", t0).work.unwrap();
    s.typed(&mut st, "", ms(t0, 10));
    let e = s.typed(&mut st, "N", ms(t0, 20));
    let mut e2 = s.conclude(&mut st, tr.execute(), ms(t0, 30));
    e2.join(e);
    s.run(&mut st, e2, ms(t0, 30));
    let e = s.finish(&mut st, ms(t0, 40));
    s.run(&mut st, e, ms(t0, 40));
    assert_eq!(docs(&st, "notas"), ["20261112-1432-n"]);
    assert_eq!(note_in(&st, "20261112-1432-n").0.highlight.as_deref(), Some("g1"), "o grifo vai para a nota redigitada");
    assert_eq!(st.pending_highlight(), None);
}

#[test]
fn quick_note_lost_not_some_to_the_erase_the_textbox() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    lost(&mut st, &mut s, t0);
    // Cmd+A, Delete
    s.typed(&mut st, "", ms(t0, 800));
    s.tick(&mut st, ms(t0, 2_000));
    assert_eq!(recovery_read(&rec(&st)).unwrap()[0].text, "Maria Rita", "apagar a caixa nao descarta");
    assert!(s.marker().is_some() && s.can_recreate());
    // so o "Descartar" explicito
    let e = s.discard_lost();
    assert!(e.warning.is_none(), "{:?}", e.warning);
    assert!(!rec(&st).exists());
    assert_eq!(s.marker(), None);
    assert!(!s.can_recreate());
}

#[test]
fn flush_keeps_in_the_record_the_body_of_the_work_in_flight() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    s.set_delay(Some(Arc::new(|| std::thread::sleep(Duration::from_millis(300)))));
    let t0 = Instant::now();
    let _in_flight = s.typed(&mut st, "M", t0).work.unwrap();
    s.typed(&mut st, "Maria", ms(t0, 10));
    s.flush_with_deadline(&mut st, Duration::from_millis(50));
    let records = recovery_read(&rec(&st)).unwrap();
    assert_eq!(records[0].text, "Maria");
    assert!(records[0].no_disk.iter().any(|c| c == "M"), "o trabalho em voo pode chegar ao disco depois: {:?}", records[0].no_disk);
    std::thread::sleep(Duration::from_millis(400));
}

#[test]
fn quick_note_of_vault_that_vanished_can_be_discarded() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut record = record(&st, "x-1", None, "Texto do NAS");
    record.vault = t.path().join("sumiu");
    write_recovery(&rec(&st), &[record]).unwrap();
    let mut s = session();
    let t0 = Instant::now();
    let e = s.restore(&mut st, t0);
    s.run(&mut st, e, t0);
    assert!(s.marker().is_some());
    assert!(s.can_recreate(), "o usuario pode resolver a nota");
    s.discard_lost();
    assert!(!rec(&st).exists());
    assert_eq!(s.marker(), None);
    assert!(!s.can_recreate());
}

#[test]
fn record_that_not_can_be_tried_can_be_recreated() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut record = record(&st, "x-1", None, "Texto estranho");
    record.kind = "xyz".into();
    write_recovery(&rec(&st), &[record]).unwrap();
    let mut s = session();
    let t0 = Instant::now();
    let e = s.restore(&mut st, t0);
    s.run(&mut st, e, t0);
    assert!(s.marker().is_some() && s.can_recreate());
    let e = s.recreate_as_new_note(&mut st, ms(t0, 10));
    s.run(&mut st, e, ms(t0, 10));
    let ps = docs(&st, "notas");
    assert_eq!(ps.len(), 1);
    let (p, note_body) = note_in(&st, &ps[0]);
    assert_eq!((p.image, note_body.as_str()), (st.current.image.clone(), "Texto estranho"));
    assert!(!rec(&st).exists());
    assert_eq!(s.marker(), None);
}

#[test]
fn discard_and_recreate_skip_the_quick_note_with_work_in_flight() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut record = record(&st, "x-1", None, "Texto do NAS");
    record.vault = t.path().join("sumiu");
    write_recovery(&rec(&st), &[record]).unwrap();
    let mut s = session();
    let t0 = Instant::now();
    let e = s.restore(&mut st, t0);
    s.run(&mut st, e, t0);
    let tr = s.tick(&mut st, ms(t0, 1_100)).work.expect("nova tentativa");
    // com a tentativa rodando, nem descartar nem recriar mexem nela
    s.discard_lost();
    assert!(rec(&st).exists());
    let e = s.recreate_as_new_note(&mut st, ms(t0, 1_110));
    assert!(e.work.is_none());
    assert!(docs(&st, "notas").is_empty());
    let e = s.conclude(&mut st, tr.execute(), ms(t0, 1_120));
    s.run(&mut st, e, ms(t0, 1_120));
    s.discard_lost();
    assert!(!rec(&st).exists(), "depois de voltar, descartar funciona");
}

/// Registro de recuperacao gravado antes de "pista" virar "nota": o tipo antigo ainda e lido.
#[test]
fn record_old_with_kind_note_still_and_read_as_quick_note() {
    assert_eq!(kind_of("pista"), Some(DocKind::Note));
    assert_eq!(kind_of("nota"), Some(DocKind::Note));
    assert_eq!(kind_name(DocKind::Note), "note");
}

/// Registro gravado com as chaves em portugues continua legivel.
#[test]
fn record_with_keys_old_in_portuguese_still_and_read() {
    let json = r#"{"versao":1,"notas":[{"chave":"k1","vault":"/v","tipo":"nota","texto":"Ana","imagem":{"fonte":"L","arquivo":"a.jpg","pagina":2},"enquadramento":{"zoom":2.0,"cx":0.5,"cy":0.5},"grifo":"g1","perdido":true,"no_disco":"antigo"}]}"#;
    let f: RecoveryFile = serde_json::from_str(json).unwrap();
    assert_eq!(f.version, 1);
    let r = &f.notes[0];
    assert_eq!((r.key.as_str(), r.kind.as_str(), r.text.as_str(), r.highlight.as_deref(), r.lost), ("k1", "nota", "Ana", Some("g1"), true));
    assert_eq!(r.image.as_ref().unwrap().file, "a.jpg");
    assert_eq!(r.no_disk, ["antigo".to_string()]);
}
