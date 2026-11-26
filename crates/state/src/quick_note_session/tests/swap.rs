//! Flush, troca de vault e renomeacao de item em curso.

use super::*;

#[test]
fn flush_writes_the_that_missing_and_pins_the_id() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "Maria", t0);
    s.run(&mut st, e, t0);
    s.typed(&mut st, "Maria José", ms(t0, 100));
    assert!(s.flush(&mut st).is_empty());
    assert_eq!(docs(&st, "notas"), ["20261112-1432-maria-jose"]);
    assert_eq!(note_in(&st, "20261112-1432-maria-jose").1, "Maria José");
    assert!(!s.busy());
    assert!(s.flush(&mut st).is_empty(), "o segundo flush (janela e app) nao faz nada");
}

#[test]
fn flush_cancels_work_that_not_started_and_the_late_not_duplicates() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    let tr = s.typed(&mut st, "Ana", t0).work.unwrap();
    assert!(s.flush(&mut st).is_empty());
    assert_eq!(docs(&st, "notas"), ["20261112-1432-ana"]);
    // o trabalho que estava na fila do executor roda depois: nao faz nada
    let e = s.conclude(&mut st, tr.execute(), ms(t0, 10));
    assert!(e.work.is_none());
    assert_eq!(docs(&st, "notas"), ["20261112-1432-ana"]);
}

#[test]
fn switch_of_vault_writes_in_the_vault_old() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "a");
    let a = root_dir(&st);
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "Ana", t0);
    s.run(&mut st, e, t0);
    s.typed(&mut st, "Ana Maria", ms(t0, 100));
    let b = Vault::open_or_create(&t.path().join("b")).unwrap();
    st.switch_vault(&b.root).unwrap();
    // a nota termina com o texto que tinha na troca, no vault em que comecou;
    // a tecla depois da troca nao vai para ela
    let e = s.typed(&mut st, "Ana Maria Rita", ms(t0, 200));
    let e = s.run(&mut st, e, ms(t0, 200));
    assert!(e.clear_textbox, "a nota do vault anterior termina e a caixa limpa");
    assert_eq!(e.confirmation.as_deref(), Some("nota salva no vault anterior: 20261112-1432-ana-maria"));
    assert_eq!(docs_in(&a, "notas"), ["20261112-1432-ana-maria"]);
    assert_eq!(parse::<Note>(&std::fs::read_to_string(a.join("notas/20261112-1432-ana-maria.md")).unwrap()).unwrap().1, "Ana Maria");
    assert!(docs_in(&b.root, "notas").is_empty() && docs_in(&b.root, "tarefas").is_empty());
    // uma nota nova no vault B (a fonte fechou na troca: nao ha imagem)
    let e = s.typed(&mut st, "Bia", ms(t0, 300));
    assert!(e.work.is_none());
    assert!(e.warning.is_some_and(|a| a.contains("imagem")));
    assert!(docs_in(&b.root, "notas").is_empty());
    assert_eq!(docs_in(&a, "notas"), ["20261112-1432-ana-maria"]);
}

#[test]
fn switch_of_vault_no_typing_also_writes_in_the_old() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "a");
    let a = root_dir(&st);
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "Ana", t0);
    s.run(&mut st, e, t0);
    s.typed(&mut st, "Ana Lu", ms(t0, 100));
    let b = Vault::open_or_create(&t.path().join("b")).unwrap();
    st.switch_vault(&b.root).unwrap();
    let e = s.observe(&mut st, ms(t0, 150));
    let e = s.run(&mut st, e, ms(t0, 150));
    assert!(e.clear_textbox);
    assert_eq!(docs_in(&a, "notas"), ["20261112-1432-ana-lu"]);
    assert!(docs_in(&b.root, "notas").is_empty());
}

#[test]
fn highlight_pending_goes_so_in_the_creation_and_after_falls() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    st.commit_highlight(ret("g1")).unwrap();
    let mut s = session();
    let t0 = Instant::now();
    let tr = s.typed(&mut st, "M", t0).work.unwrap();
    // outro grifo desenhado enquanto a criacao roda no fundo
    st.commit_highlight(ret("g2")).unwrap();
    let e = s.conclude(&mut st, tr.execute(), ms(t0, 10));
    s.run(&mut st, e, ms(t0, 10));
    assert_eq!(note_in(&st, DRAFT).0.highlight.as_deref(), Some("g1"));
    assert_eq!(st.pending_highlight(), Some(&ret("g2")), "o grifo desenhado depois continua pendente");
    let e = s.finish(&mut st, ms(t0, 20));
    s.run(&mut st, e, ms(t0, 20));
    let e = s.typed(&mut st, "N", ms(t0, 30));
    s.run(&mut st, e, ms(t0, 30));
    assert_eq!(note_in(&st, DRAFT).0.highlight.as_deref(), Some("g2"));
    assert_eq!(st.pending_highlight(), None);
    let e = s.finish(&mut st, ms(t0, 40));
    s.run(&mut st, e, ms(t0, 40));
    let e = s.typed(&mut st, "O", ms(t0, 50));
    s.run(&mut st, e, ms(t0, 50));
    assert_eq!(note_in(&st, DRAFT).0.highlight, None, "o grifo vai uma vez so");
}

#[test]
fn not_found_ends_the_pinning_and_writes_in_the_id_new() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "Maria", t0);
    s.run(&mut st, e, t0);
    let _ = st.open_note(DRAFT).unwrap();
    assert_eq!(st.note_open(), Some(DRAFT));
    // a fixacao parou depois de apagar o antigo (outra instancia, queda): so falta tirar a marca
    let new = "20261112-1432-maria";
    continuation(&root_dir(&st), DRAFT, new);
    std::fs::remove_file(root_dir(&st).join(format!("notas/{DRAFT}.md"))).unwrap();
    s.typed(&mut st, "Maria Rita", ms(t0, 100));
    let e = s.tick(&mut st, ms(t0, 700));
    let e = s.run(&mut st, e, ms(t0, 700));
    assert_eq!(e.renamed, [(DRAFT.to_string(), new.to_string())]);
    assert_eq!(s.id_current(), Some(new));
    assert_eq!(st.note_open(), Some(new), "a nota aberta segue o id novo");
    assert_eq!(docs(&st, "notas"), [new], "nenhum rascunho novo foi criado");
    let (p, note_body) = note_in(&st, new);
    assert_eq!(note_body, "Maria Rita");
    assert!(p.extra.get("renaming_from").is_none() && p.extra.get("draft").is_none(), "{:?}", p.extra);
    assert!(s.tick(&mut st, ms(t0, 30_000)).work.is_none(), "o id ja esta fixado");
}

#[test]
fn renaming_in_progress_ends_before_and_the_text_not_se_loses() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let v = st.vault.clone().unwrap();
    let (img, fr) = (st.current.image.clone(), st.framing().clone());
    let n = v.create_note("Joao", img.clone(), fr.clone(), None, data()).unwrap();
    let mut s = session();
    let t0 = Instant::now();
    let e = s.load(&mut st, DocKind::Note, &n.id, t0);
    s.run(&mut st, e, t0);
    // outra instancia: um rascunho A cuja fixacao parou no meio, com `n` como continuacao
    let a = v.create_draft("Joao velho", img, fr, None, data()).unwrap();
    let (mut meta, note_body): (Note, String) = parse(&read_file(&st, "notas", &n.id)).unwrap();
    meta.extra.insert("renaming_from".into(), a.id.clone().into());
    std::fs::write(root_dir(&st).join(format!("notas/{}.md", n.id)), render(&meta, &note_body)).unwrap();
    s.typed(&mut st, "Joao novo", ms(t0, 100));
    let e = s.tick(&mut st, ms(t0, 700));
    let e = s.run(&mut st, e, ms(t0, 700));
    assert!(e.warning.is_none(), "{:?}", e.warning);
    assert_eq!(docs(&st, "notas"), [n.id.clone()], "a fixacao de A terminou antes");
    let (p, note_body) = note_in(&st, &n.id);
    assert_eq!(note_body, "Joao novo", "o texto digitado vence");
    assert!(p.extra.get("renaming_from").is_none());
}

#[test]
fn id_new_arrives_the_note_open_and_as_drawers() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "Maria herdeira", t0);
    s.run(&mut st, e, t0);
    let v = st.vault.clone().unwrap();
    v.create_task("ver o batismo", None, Some(DRAFT.into()), data()).unwrap();
    let _ = st.open_note(DRAFT).unwrap();
    let mut g = Drawers::default();
    let n = g.request_reload();
    g.receive(n, drawers::load(&v, &st.app_data));
    g.toggle(Drawer::Notes);
    assert_eq!(g.note_selected().map(|p| p.id.as_str()), Some(DRAFT));
    let e = s.textbox_exit(&mut st, ms(t0, 100));
    let e = s.run(&mut st, e, ms(t0, 100));
    let new = "20261112-1432-maria-herdeira";
    assert_eq!(e.renamed, [(DRAFT.to_string(), new.to_string())]);
    for (a, n) in &e.renamed {
        g.switch_id(a, n);
    }
    assert_eq!(st.note_open(), Some(new));
    assert_eq!(g.note_selected().map(|p| p.id.as_str()), Some(new));
    assert_eq!(g.lists().tasks[0].note.as_deref(), Some(new));
    let tasks = v.list_tasks().unwrap();
    assert_eq!(tasks[0].0.note.as_deref(), Some(new), "no disco tambem (core)");
}

#[test]
fn renaming_stopped_and_resumed_and_the_continuation_stays_hidden() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let v = st.vault.clone().unwrap();
    let a = v.create_draft("Maria", st.current.image.clone(), st.framing().clone(), None, data()).unwrap();
    let new = "20261112-1432-maria";
    continuation(&root_dir(&st), &a.id, new);
    let l = drawers::load(&v, &st.app_data);
    assert_eq!(l.notes.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), [a.id.as_str()], "N fica escondida enquanto A existe");
    assert_eq!(l.resume, [(DocKind::Note, a.id.clone())]);
    let mut s = session();
    let t0 = Instant::now();
    let mut e = Effect::default();
    for (kind, old) in &l.resume {
        e.join(s.resume(&mut st, *kind, old, t0));
    }
    let e = s.run(&mut st, e, t0);
    assert_eq!(e.renamed, [(a.id.clone(), new.to_string())]);
    assert!(e.reload);
    let l = drawers::load(&v, &st.app_data);
    assert_eq!(l.notes.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), [new]);
    assert!(l.resume.is_empty());
    assert!(note_in(&st, new).0.extra.get("renaming_from").is_none());
}

/// A carga pedida com o id provisorio (linha ainda velha na gaveta) segue o
/// id novo quando a fixacao em voo termina: o texto chega, sem "nao foi possivel abrir".
#[test]
fn load_requested_with_the_id_provisional_follows_the_renaming() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    let img1 = st.current.image.clone();
    let e = s.typed(&mut st, "Maria herdeira", t0);
    s.run(&mut st, e, t0);
    // outra imagem: a nota termina (fixacao em voo) e a caixa limpa
    go_to_next(&mut st);
    let mut e = s.observe(&mut st, ms(t0, 100));
    assert!(e.work.is_some(), "fixacao em voo");
    assert!(s.id_current().is_none(), "a caixa largou a nota");
    // clique na linha que a gaveta ainda mostra com o id provisorio
    st.open_frame(crate::app_state::Frame::new(&img1.source, &img1.file, img1.page));
    e.join(s.load(&mut st, DocKind::Note, DRAFT, ms(t0, 100)));
    let e = s.run(&mut st, e, ms(t0, 100));
    assert!(e.warning.is_none(), "{:?}", e.warning);
    assert_eq!(e.loaded.as_deref(), Some("Maria herdeira"));
    assert_eq!(s.id_current(), Some("20261112-1432-maria-herdeira"));
}

#[test]
fn load_the_continuation_ends_the_pinning_before() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let v = st.vault.clone().unwrap();
    let a = v.create_draft("Maria", st.current.image.clone(), st.framing().clone(), None, data()).unwrap();
    let new = "20261112-1432-maria";
    continuation(&root_dir(&st), &a.id, new);
    let mut s = session();
    let t0 = Instant::now();
    let e = s.load(&mut st, DocKind::Note, new, t0);
    let e = s.run(&mut st, e, t0);
    assert_eq!(e.loaded.as_deref(), Some("Maria"));
    assert_eq!(e.renamed, [(a.id.clone(), new.to_string())]);
    assert_eq!(docs(&st, "notas"), [new]);
    s.typed(&mut st, "Maria Rita", ms(t0, 100));
    let e = s.tick(&mut st, ms(t0, 700));
    s.run(&mut st, e, ms(t0, 700));
    assert_eq!(note_in(&st, new).1, "Maria Rita");
}

#[test]
fn finish_cleared_the_textbox_so_after_of_write() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    let tr = s.typed(&mut st, "Maria", t0).work.unwrap();
    let e = s.finish(&mut st, ms(t0, 50));
    assert!(e.work.is_none(), "um trabalho de cada vez");
    assert!(!e.clear_textbox);
    let e = s.conclude(&mut st, tr.execute(), ms(t0, 60));
    assert!(!e.clear_textbox && e.work.is_some(), "falta fixar o id");
    let e = s.run(&mut st, e, ms(t0, 60));
    assert!(e.clear_textbox);
    match &e.created {
        Some(Created::Note(p)) => assert_eq!(p.id, "20261112-1432-maria"),
        o => panic!("{o:?}"),
    }
    assert_eq!(e.confirmation.as_deref(), Some("nota criada: 20261112-1432-maria"));
    assert_eq!(s.id_current(), None);
    assert!(!s.busy());
    // o que foi digitado entre o Cmd+Enter e a resposta tambem vai
    let tr = s.typed(&mut st, "Ana", ms(t0, 100)).work.unwrap();
    s.finish(&mut st, ms(t0, 110));
    s.typed(&mut st, "Ana Lu", ms(t0, 120));
    let e = s.conclude(&mut st, tr.execute(), ms(t0, 130));
    let e = s.run(&mut st, e, ms(t0, 130));
    assert!(e.clear_textbox);
    assert!(docs(&st, "notas").contains(&"20261112-1432-ana-lu".to_string()), "{:?}", docs(&st, "notas"));
    assert_eq!(note_in(&st, "20261112-1432-ana-lu").1, "Ana Lu");
}

/// A recarga das gavetas acha A no meio da renomeacao QUE A PROPRIA SESSAO
/// esta fazendo (fixacao em voo): a retomada nao entra na fila; e uma que
/// chegue a rodar depois de A sumir (`NotFound`) conta como feita, sem
/// aviso nem nova tentativa a cada 5 s.
#[test]
fn resumed_of_the_with_the_pinning_of_the_in_flight_not_tries_to_always() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "Maria herdeira", t0);
    s.run(&mut st, e, t0);
    // a fixacao de A sai na saida da caixa; antes de ela rodar, a recarga pede a retomada de A
    let e = s.textbox_exit(&mut st, ms(t0, 100));
    assert!(e.work.is_some(), "fixacao em voo");
    let mut e = e;
    e.join(s.resume(&mut st, DocKind::Note, DRAFT, ms(t0, 100)));
    let e = s.run(&mut st, e, ms(t0, 100));
    let new = "20261112-1432-maria-herdeira";
    assert_eq!(e.renamed, [(DRAFT.to_string(), new.to_string())]);
    assert!(e.warning.is_none(), "{:?}", e.warning);
    assert_eq!(docs(&st, "notas"), [new]);
    for n in [6_000, 12_000, 60_000] {
        let e = s.tick(&mut st, ms(t0, n));
        assert!(e.work.is_none(), "nada a tentar de novo em {n} ms");
        assert!(e.warning.is_none(), "{:?}", e.warning);
    }
    // uma retomada de um id que ja nao existe (renomeado por outro caminho): feita, sem erro
    let e = s.resume(&mut st, DocKind::Note, DRAFT, ms(t0, 61_000));
    let e = s.run(&mut st, e, ms(t0, 61_000));
    assert!(e.warning.is_none(), "{:?}", e.warning);
    assert!(e.reload, "a lista e relida");
    assert!(s.tick(&mut st, ms(t0, 70_000)).work.is_none(), "nao volta a fila");
}
