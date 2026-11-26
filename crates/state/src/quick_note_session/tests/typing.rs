//! Criar o rascunho, digitar com debounce, fixar o id e falhas de gravacao.

use super::*;

#[test]
fn nothing_and_created_with_text_empty_or_so_bracket() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    for (i, txt) in ["", "   ", " \n\t", "[", " [ ", "[]", "  []  "].iter().enumerate() {
        let e = s.typed(&mut st, txt, ms(t0, i as u64 * 100));
        assert!(e.work.is_none(), "{txt:?}");
    }
    assert!(s.tick(&mut st, ms(t0, 30_000)).work.is_none());
    assert!(s.finish(&mut st, ms(t0, 31_000)).work.is_none());
    assert!(s.flush(&mut st).is_empty());
    assert!(docs(&st, "notas").is_empty() && docs(&st, "tarefas").is_empty());
    assert!(!s.busy());
    assert_eq!(s.id_current(), None);
}

#[test]
fn write_text_new_in_a_note_existing_asks_the_reread_of_the_drawers() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "Maria", t0);
    s.run(&mut st, e, t0);
    // editar depois (a etiqueta nova precisa aparecer na gaveta)
    let e = s.typed(&mut st, "Maria #achado", ms(t0, 10));
    let e = s.run(&mut st, e, ms(t0, 10));
    let e2 = s.tick(&mut st, ms(t0, 1_000));
    let e2 = s.run(&mut st, e2, ms(t0, 1_000));
    assert!(e.reload || e2.reload, "gravou o texto novo: a gaveta relê");
    let (_, note_body) = note_in(&st, DRAFT);
    assert_eq!(note_body, "Maria #achado");
}

#[test]
fn first_font_creates_the_draft_with_the_frame_current() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let p = st.go_next().unwrap();
    let _ = st.load_now(p);
    let fr = Framing { zoom: 2.5, cx: 0.3, cy: 0.6 };
    st.set_framing(fr.clone());
    st.commit_highlight(ret("g1")).unwrap();
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "M", t0);
    let tr = e.work.expect("a 1a letra cria o rascunho");
    assert!(docs(&st, "notas").is_empty(), "nada e gravado antes de o trabalho rodar (no fundo)");
    let e = s.conclude(&mut st, tr.execute(), t0);
    assert!(e.reload, "a gaveta precisa reler");
    assert_eq!(docs(&st, "notas"), [DRAFT]);
    let (p, note_body) = note_in(&st, DRAFT);
    assert_eq!(p.image, ImageRef { source: "Livro".into(), file: "p02.png".into(), page: 1 });
    assert_eq!(p.framing, fr);
    assert_eq!(p.highlight.as_deref(), Some("g1"));
    assert_eq!(p.extra.get("draft").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(note_body, "M");
    assert_eq!(st.pending_highlight(), None, "o grifo pendente foi para a nota");
    assert_eq!(s.id_current(), Some(DRAFT));
}

#[test]
fn typing_of_new_so_updates_the_text_with_debounce() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "M", t0);
    s.run(&mut st, e, t0);
    let e = s.typed(&mut st, "Ma", ms(t0, 100));
    assert!(e.work.is_none(), "debounce: nada grava a cada tecla");
    assert!(e.tick.is_some_and(|d| d <= DEBOUNCE), "a view agenda o tick: {:?}", e.tick);
    assert!(s.typed(&mut st, "Mar", ms(t0, 200)).work.is_none());
    assert!(s.tick(&mut st, ms(t0, 600)).work.is_none(), "ainda dentro do debounce da ultima tecla");
    let e = s.tick(&mut st, ms(t0, 750));
    assert!(e.work.is_some());
    s.run(&mut st, e, ms(t0, 750));
    assert_eq!(docs(&st, "notas"), [DRAFT], "um arquivo so, sem renomear");
    assert_eq!(note_in(&st, DRAFT).1, "Mar");
    // digitando sem parar (teclas a cada 300 ms) o texto ainda vai ao disco a cada ~2 s
    let mut text = String::from("Mar");
    let mut writes = 0;
    for i in 1..=9u64 {
        text.push('i');
        let mut e = s.typed(&mut st, &text, ms(t0, 750 + i * 300));
        e.join(s.tick(&mut st, ms(t0, 760 + i * 300)));
        writes += e.work.is_some() as u32;
        s.run(&mut st, e, ms(t0, 760 + i * 300));
    }
    assert_eq!(writes, 1, "uma gravacao pela espera maxima em ~2 s de digitacao continua");
    assert_ne!(note_in(&st, DRAFT).1, "Mar", "a espera maxima gravou no meio da digitacao");
    assert_eq!(docs(&st, "notas"), [DRAFT]);
    let e = s.tick(&mut st, ms(t0, 4000));
    s.run(&mut st, e, ms(t0, 4000));
    assert_eq!(note_in(&st, DRAFT).1, text);
}

#[test]
fn ten_seconds_no_typing_pin_the_id_a_time() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "Maria herdeira", t0);
    s.run(&mut st, e, t0);
    assert!(s.tick(&mut st, ms(t0, 9_000)).work.is_none(), "texto ja gravado e menos de 10 s");
    let e = s.tick(&mut st, ms(t0, 10_100));
    assert!(e.work.is_some(), "10 s sem digitar: fixa o id");
    let e = s.run(&mut st, e, ms(t0, 10_100));
    let new = "20261112-1432-maria-herdeira";
    assert_eq!(e.renamed, [(DRAFT.to_string(), new.to_string())]);
    assert_eq!(docs(&st, "notas"), [new]);
    assert_eq!(s.id_current(), Some(new));
    let (p, note_body) = note_in(&st, new);
    assert!(p.extra.get("draft").is_none(), "{:?}", p.extra);
    assert_eq!(note_body, "Maria herdeira");
    // ticks repetidos nao renomeiam de novo
    for k in [11_000, 12_000, 20_000, 40_000] {
        assert!(s.tick(&mut st, ms(t0, k)).work.is_none(), "{k}");
    }
    // digitar depois de fixado atualiza o mesmo arquivo, sem renomear
    let e = s.typed(&mut st, "Maria herdeira filha de Jose", ms(t0, 41_000));
    assert!(e.work.is_none());
    let e = s.tick(&mut st, ms(t0, 41_600));
    let e = s.run(&mut st, e, ms(t0, 41_600));
    assert!(e.renamed.is_empty());
    assert_eq!(docs(&st, "notas"), [new]);
    assert_eq!(note_in(&st, new).1, "Maria herdeira filha de Jose");
    assert!(s.tick(&mut st, ms(t0, 80_000)).work.is_none());
    assert_eq!(docs(&st, "notas"), [new]);
}

#[test]
fn exit_of_the_textbox_and_change_of_image_pin_the_id() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "Joao", t0);
    s.run(&mut st, e, t0);
    assert!(s.typed(&mut st, "Joao padrinho", ms(t0, 100)).work.is_none());
    let e = s.textbox_exit(&mut st, ms(t0, 200));
    assert!(e.work.is_some(), "sair da caixa grava e fixa na hora");
    let e = s.run(&mut st, e, ms(t0, 200));
    assert!(!e.clear_textbox, "sair da caixa nao limpa (o texto fica para continuar)");
    assert_eq!(docs(&st, "notas"), ["20261112-1432-joao-padrinho"]);
    assert_eq!(note_in(&st, "20261112-1432-joao-padrinho").1, "Joao padrinho");
    let e = s.finish(&mut st, ms(t0, 300));
    assert!(s.run(&mut st, e, ms(t0, 300)).clear_textbox);

    // mudar de imagem (aviso explicito da view)
    let e = s.typed(&mut st, "Ana", ms(t0, 1000));
    s.run(&mut st, e, ms(t0, 1000));
    let img1 = st.current.image.clone();
    let p = st.go_next().unwrap();
    let _ = st.load_now(p);
    let e = s.image_changed(&mut st, ms(t0, 1100));
    assert!(e.clear_textbox, "trocar de imagem termina a nota: a caixa limpa");
    let e = s.run(&mut st, e, ms(t0, 1100));
    assert_eq!(e.confirmation.as_deref(), Some("nota salva: 20261112-1432-ana"));
    assert_eq!(note_in(&st, "20261112-1432-ana").0.image, img1, "a nota e da imagem em que comecou");
    assert!(!s.has_item());
    // a proxima nota e outra nota, da imagem nova
    let e = s.typed(&mut st, "Lucia", ms(t0, 1200));
    assert!(e.work.is_some());
    s.run(&mut st, e, ms(t0, 1200));
    let e = s.finish(&mut st, ms(t0, 1900));
    s.run(&mut st, e, ms(t0, 1900));
    assert_eq!(note_in(&st, "20261112-1432-ana").1, "Ana");
    assert_ne!(note_in(&st, "20261112-1432-lucia").0.image, img1);

    // mudar de imagem sem aviso: o tick percebe
    let e = s.typed(&mut st, "Bia", ms(t0, 2000));
    s.run(&mut st, e, ms(t0, 2000));
    let p = st.go_next().unwrap();
    let _ = st.load_now(p);
    let e = s.tick(&mut st, ms(t0, 2100));
    assert!(e.work.is_some());
    s.run(&mut st, e, ms(t0, 2100));
    assert!(docs(&st, "notas").contains(&"20261112-1432-bia".to_string()), "{:?}", docs(&st, "notas"));
    assert!(!docs(&st, "notas").iter().any(|d| d.contains("sem-texto")));
}

#[test]
fn brackets_become_task_and_follow_the_same_cycle() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    st.commit_highlight(ret("g1")).unwrap();
    let mut s = session();
    let t0 = Instant::now();
    assert!(s.typed(&mut st, "[", t0).work.is_none());
    assert!(s.typed(&mut st, "[]", ms(t0, 100)).work.is_none());
    assert!(s.typed(&mut st, "[] ", ms(t0, 150)).work.is_none());
    let e = s.typed(&mut st, "[] a", ms(t0, 200));
    assert!(e.work.is_some(), "texto alem de [] cria a tarefa");
    s.run(&mut st, e, ms(t0, 200));
    assert!(docs(&st, "notas").is_empty());
    assert_eq!(docs(&st, "tarefas"), [DRAFT]);
    let (tf, note_body): (Task, String) = parse(&read_file(&st, "tarefas", DRAFT)).unwrap();
    assert_eq!((tf.image.as_ref(), note_body.as_str()), (Some(&st.current.image), "a"));
    assert_eq!(st.pending_highlight(), Some(&ret("g1")), "tarefa nao leva o grifo");
    s.typed(&mut st, "[] achar óbito", ms(t0, 300));
    let e = s.tick(&mut st, ms(t0, 900));
    s.run(&mut st, e, ms(t0, 900));
    assert_eq!(parse::<Task>(&read_file(&st, "tarefas", DRAFT)).unwrap().1, "achar óbito");
    let e = s.tick(&mut st, ms(t0, 10_400));
    let e = s.run(&mut st, e, ms(t0, 10_400));
    let new = "20261112-1432-achar-obito";
    assert_eq!(e.renamed, [(DRAFT.to_string(), new.to_string())]);
    let e = s.finish(&mut st, ms(t0, 10_500));
    let e = s.run(&mut st, e, ms(t0, 10_500));
    assert!(e.clear_textbox);
    match e.created {
        Some(Created::Task(tf)) => assert_eq!(tf.id, new),
        o => panic!("esperava tarefa, veio {o:?}"),
    }
    assert!(e.confirmation.is_some_and(|c| c.starts_with("tarefa criada")));
    assert_eq!(docs(&st, "tarefas"), [new]);
}

#[test]
fn failure_of_write_keeps_the_text_warns_and_tries_of_new() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    st.commit_highlight(ret("g1")).unwrap();
    let notes = root_dir(&st).join("notas");
    std::fs::remove_dir_all(&notes).unwrap();
    std::fs::write(&notes, "nao e pasta").unwrap();
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "Maria", t0);
    let e = s.run(&mut st, e, t0);
    assert!(e.warning.as_deref().is_some_and(|a| a.contains("texto continua")), "{:?}", e.warning);
    assert!(e.created.is_none() && !e.clear_textbox);
    assert_eq!(s.text_to_restore(), Some("Maria"));
    assert!(s.busy());
    assert_eq!(st.pending_highlight(), Some(&ret("g1")), "o grifo continua pendente");
    // Cmd+Enter com a gravacao falhando: a caixa nao limpa
    let e = s.finish(&mut st, ms(t0, 100));
    assert!(!s.run(&mut st, e, ms(t0, 100)).clear_textbox);
    assert!(s.tick(&mut st, ms(t0, 500)).work.is_none(), "espera um pouco antes de tentar de novo");
    std::fs::remove_file(&notes).unwrap();
    std::fs::create_dir(&notes).unwrap();
    let e = s.tick(&mut st, ms(t0, 1_300));
    assert!(e.work.is_some(), "tenta de novo no tick seguinte");
    let e = s.run(&mut st, e, ms(t0, 1_300));
    assert!(e.clear_textbox, "o Cmd+Enter pedido antes termina agora");
    assert_eq!(docs(&st, "notas"), ["20261112-1432-maria"]);
    assert_eq!(note_in(&st, "20261112-1432-maria").0.highlight.as_deref(), Some("g1"));
    assert_eq!(st.pending_highlight(), None);
    assert_eq!(s.text_to_restore(), None);
}

#[cfg(unix)]
#[test]
fn failure_to_the_update_text_keeps_and_tries_of_new() {
    use std::os::unix::fs::PermissionsExt;
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "Maria", t0);
    s.run(&mut st, e, t0);
    let notes = root_dir(&st).join("notas");
    std::fs::set_permissions(&notes, std::fs::Permissions::from_mode(0o555)).unwrap();
    s.typed(&mut st, "Maria Jose", ms(t0, 100));
    let e = s.tick(&mut st, ms(t0, 700));
    let e = s.run(&mut st, e, ms(t0, 700));
    std::fs::set_permissions(&notes, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(e.warning.is_some(), "a falha aparece");
    assert_eq!(s.text_to_restore(), Some("Maria Jose"));
    assert_eq!(note_in(&st, DRAFT).1, "Maria");
    let e = s.tick(&mut st, ms(t0, 1_800));
    s.run(&mut st, e, ms(t0, 1_800));
    assert_eq!(note_in(&st, DRAFT).1, "Maria Jose");
    assert_eq!(s.text_to_restore(), None);
}

/// Fix round 2: um item saindo (troca de imagem) cuja fixacao falhou uma vez
/// ainda segura o id (promover espera); se a nota virar nota mesmo assim, a
/// nova tentativa grava na nota (o item nao fica "perdido").
#[cfg(unix)]
#[test]
fn item_leaving_with_pinning_failed_safe_the_id_ate_pin() {
    use std::os::unix::fs::PermissionsExt;
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "Maria", t0);
    s.run(&mut st, e, t0);
    assert!(s.safe(DRAFT), "rascunho sem id fixado na caixa: promover agora disputaria o arquivo");
    assert_eq!(s.free(DRAFT), Err("aguarde: a nota em edicao esta sendo salva".to_string()));
    s.typed(&mut st, "Maria Jose", ms(t0, 100));
    let notes = root_dir(&st).join("notas");
    std::fs::set_permissions(&notes, std::fs::Permissions::from_mode(0o555)).unwrap();
    let p = st.go_next().unwrap();
    let _ = st.load_now(p);
    let e = s.image_changed(&mut st, ms(t0, 200));
    let e = s.run(&mut st, e, ms(t0, 200));
    std::fs::set_permissions(&notes, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(e.warning.is_some(), "a falha aparece");
    assert!(!s.has_item(), "o item saiu da caixa");
    assert_eq!(docs(&st, "notas"), [DRAFT], "a fixacao nao aconteceu");
    assert!(s.safe(DRAFT), "item saindo, ainda por gravar e fixar: segura o id");
    let mut current_time = 1_000;
    while current_time < 60_000 && (s.busy() || s.in_flight()) {
        let e = s.tick(&mut st, ms(t0, current_time));
        s.run(&mut st, e, ms(t0, current_time));
        current_time += 500;
    }
    assert_eq!(s.marker(), None, "nao ficou perdido");
    assert_eq!(s.text_to_restore(), None);
    let ids = docs(&st, "notas");
    assert_eq!(ids.len(), 1, "a nota foi fixada com o id definitivo");
    let (_, note_body): (Note, String) = parse(&read_file(&st, "notas", &ids[0])).unwrap();
    assert_eq!(note_body, "Maria Jose", "o texto chegou a nota");
    assert!(!s.safe(DRAFT));
    assert_eq!(s.free(DRAFT), Ok(()));
}

/// Cria o rascunho de "M" (primeira gravacao) e devolve o instante em que acabou.
fn created_draft(s: &mut QuickNoteSession, st: &mut AppState, t0: Instant) {
    let e = s.typed(st, "M", t0);
    s.run(st, e, t0);
}

#[test]
fn rhythm_fast_writes_after_of_250_ms() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    s.set_rhythm(QuickNoteRhythm::Fast);
    let t0 = Instant::now();
    created_draft(&mut s, &mut st, t0);
    let e = s.typed(&mut st, "Ma", ms(t0, 100));
    assert!(e.work.is_none());
    assert!(e.tick.is_some_and(|d| d <= Duration::from_millis(250)), "{:?}", e.tick);
    assert!(s.tick(&mut st, ms(t0, 300)).work.is_none(), "250 ms desde a ultima tecla ainda nao passaram");
    assert!(s.tick(&mut st, ms(t0, 360)).work.is_some(), "250 ms depois da ultima tecla: grava");
}

#[test]
fn rhythm_calm_wait_2_s_and_typing_no_stop_writes_the_each_5_s() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    s.set_rhythm(QuickNoteRhythm::Calm);
    let t0 = Instant::now();
    created_draft(&mut s, &mut st, t0);
    let e = s.typed(&mut st, "Ma", ms(t0, 100));
    assert!(e.tick.is_some_and(|d| d <= Duration::from_secs(2)), "{:?}", e.tick);
    assert!(s.tick(&mut st, ms(t0, 1_900)).work.is_none(), "o normal ja teria gravado, o calmo nao");
    assert!(s.tick(&mut st, ms(t0, 2_200)).work.is_some(), "2 s depois da ultima tecla: grava");

    // Digitando sem parar (uma tecla a cada 300 ms): nada em 4 s, grava perto dos 5 s.
    let mut s = session();
    s.set_rhythm(QuickNoteRhythm::Calm);
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    created_draft(&mut s, &mut st, t0);
    let mut text = String::from("M");
    let mut first = None;
    for i in 1..=20u64 {
        text.push('a');
        let mut e = s.typed(&mut st, &text, ms(t0, i * 300));
        e.join(s.tick(&mut st, ms(t0, i * 300 + 10)));
        if e.work.is_some() && first.is_none() {
            first = Some(i * 300);
        }
        s.run(&mut st, e, ms(t0, i * 300 + 10));
    }
    let first = first.expect("a espera maxima do calmo grava no meio da digitacao");
    assert!((4_800..=5_400).contains(&first), "primeira gravacao em {first} ms");
}

#[test]
fn switch_the_rhythm_is_worth_to_the_that_already_is_in_the_textbox_and_the_default_and_the_normal() {
    let mut s = session();
    assert_eq!(s.rhythm(), QuickNoteRhythm::Normal);
    assert_eq!(rhythm_wait(QuickNoteRhythm::Normal), DEBOUNCE);
    assert_eq!(rhythm_max_wait(QuickNoteRhythm::Normal), MAX_WAIT);
    for r in QuickNoteRhythm::ALL {
        s.set_rhythm(r);
        assert_eq!(s.rhythm(), r);
        assert!(rhythm_wait(r) < rhythm_max_wait(r), "{r:?}");
    }
    assert!(rhythm_wait(QuickNoteRhythm::Fast) < DEBOUNCE && DEBOUNCE < rhythm_wait(QuickNoteRhythm::Calm));
    // O id definitivo (10 s) nao depende do ritmo: o calmo tem de gravar antes de fixar.
    assert!(rhythm_max_wait(QuickNoteRhythm::Calm) < PIN_AFTER);
}
