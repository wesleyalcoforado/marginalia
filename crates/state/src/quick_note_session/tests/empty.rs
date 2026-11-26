//! Troca de pagina e caixa esvaziada.

use super::*;

#[test]
fn switch_of_page_ends_the_quick_note_and_the_next_and_new_in_the_frame_new() {
    // R18: Ana em p01, p02, apagar e digitar Jose -> Ana intacta em p01, Jose nova em p02
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "Ana", t0);
    s.run(&mut st, e, t0);
    let p01 = st.current.image.clone();
    go_to_next(&mut st);
    st.commit_highlight(ret("g2")).unwrap();
    // a caixa ainda mostrava "Ana" (a view nao sincronizou): apagar uma letra
    let e = s.typed(&mut st, "An", ms(t0, 100));
    assert!(e.clear_textbox, "a nota termina e a caixa limpa");
    let e = s.run(&mut st, e, ms(t0, 100));
    assert_eq!(e.confirmation.as_deref(), Some("nota salva: 20261112-1432-ana"));
    let e = s.typed(&mut st, "", ms(t0, 150));
    assert!(e.work.is_none());
    let e = s.typed(&mut st, "José", ms(t0, 200));
    s.run(&mut st, e, ms(t0, 200));
    let e = s.finish(&mut st, ms(t0, 300));
    assert!(s.run(&mut st, e, ms(t0, 300)).clear_textbox);
    assert_eq!(docs(&st, "notas"), ["20261112-1432-ana", "20261112-1432-jose"]);
    let (ana, note_body) = note_in(&st, "20261112-1432-ana");
    assert_eq!((ana.image, ana.highlight, note_body.as_str()), (p01, None, "Ana"), "Ana intacta");
    let (jose, note_body) = note_in(&st, "20261112-1432-jose");
    assert_eq!(jose.image, st.current.image);
    assert_eq!((jose.highlight.as_deref(), note_body.as_str()), (Some("g2"), "José"), "o grifo de p02 vai so para Jose");
    assert_eq!(st.pending_highlight(), None);
}

#[test]
fn switch_of_page_with_the_creation_in_flight_also_ends_the_quick_note() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    let tr = s.typed(&mut st, "Ana", t0).work.unwrap();
    go_to_next(&mut st);
    let e = s.observe(&mut st, ms(t0, 10));
    assert!(e.clear_textbox && e.work.is_none());
    // a proxima nota comeca enquanto a de antes ainda grava
    assert!(s.typed(&mut st, "Bia", ms(t0, 20)).work.is_none(), "uma fila so");
    let e = s.conclude(&mut st, tr.execute(), ms(t0, 30));
    let e = s.run(&mut st, e, ms(t0, 30));
    let e2 = s.finish(&mut st, ms(t0, 40));
    s.run(&mut st, e2, ms(t0, 40));
    assert!(e.confirmation.is_some());
    assert_eq!(docs(&st, "notas"), ["20261112-1432-ana", "20261112-1432-bia"]);
    assert_eq!(note_in(&st, "20261112-1432-ana").1, "Ana");
    assert_eq!(note_in(&st, "20261112-1432-bia").0.image, st.current.image);
}

#[test]
fn empty_the_textbox_erases_the_draft_and_returns_the_highlight() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    st.commit_highlight(ret("g1")).unwrap();
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "M", t0);
    s.run(&mut st, e, t0);
    assert_eq!(docs(&st, "notas"), [DRAFT]);
    assert_eq!(st.pending_highlight(), None);
    let e = s.typed(&mut st, "", ms(t0, 100));
    assert!(e.work.is_some(), "apaga na fila, ja");
    let e = s.run(&mut st, e, ms(t0, 100));
    assert!(e.reload);
    assert!(docs(&st, "notas").is_empty(), "o rascunho saiu do disco");
    assert_eq!(st.pending_highlight(), Some(&ret("g1")), "o grifo volta a ser o pendente");
    assert!(!s.has_item() && !s.busy());
    // a proxima letra comeca um rascunho limpo, com o grifo
    let e = s.typed(&mut st, "N", ms(t0, 200));
    s.run(&mut st, e, ms(t0, 200));
    let (p, note_body) = note_in(&st, DRAFT);
    assert_eq!((p.highlight.as_deref(), note_body.as_str()), (Some("g1"), "N"));
}

#[test]
fn empty_the_textbox_with_the_creation_in_flight_erases_after_its() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    st.commit_highlight(ret("g1")).unwrap();
    let mut s = session();
    let t0 = Instant::now();
    let tr = s.typed(&mut st, "M", t0).work.unwrap();
    let e = s.typed(&mut st, "", ms(t0, 10));
    assert!(e.work.is_none(), "espera a criacao em voo");
    assert!(!s.has_item(), "a sessao ja esqueceu a nota");
    let e = s.conclude(&mut st, tr.execute(), ms(t0, 20));
    assert!(e.work.is_some(), "o apagar vem logo depois, na mesma fila");
    s.run(&mut st, e, ms(t0, 20));
    assert!(docs(&st, "notas").is_empty());
    assert_eq!(st.pending_highlight(), Some(&ret("g1")));
    // sem nada no disco ainda, esvaziar so esquece
    let e = s.typed(&mut st, "", ms(t0, 30));
    assert!(e.work.is_none());
    let e = s.typed(&mut st, "x", ms(t0, 40));
    let tr = e.work.unwrap();
    // o trabalho nao rodou: esvaziar enquanto voa
    s.typed(&mut st, " ", ms(t0, 50));
    let e = s.conclude(&mut st, tr.execute(), ms(t0, 60));
    s.run(&mut st, e, ms(t0, 60));
    assert!(docs(&st, "notas").is_empty());
}

#[test]
fn draft_cited_emptied_stays_with_body_empty() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "M", t0);
    s.run(&mut st, e, t0);
    st.vault.clone().unwrap().create_task("ver", None, Some(DRAFT.into()), data()).unwrap();
    let e = s.typed(&mut st, "", ms(t0, 100));
    s.run(&mut st, e, ms(t0, 100));
    assert_eq!(docs(&st, "notas"), [DRAFT], "uma tarefa cita o rascunho: nao apaga");
    assert_eq!(note_in(&st, DRAFT).1, "");
    assert!(!s.busy());
}

#[test]
fn note_with_id_pinned_emptied_stays_with_body_empty() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "Maria", t0);
    s.run(&mut st, e, t0);
    let e = s.textbox_exit(&mut st, ms(t0, 100));
    s.run(&mut st, e, ms(t0, 100));
    let id = "20261112-1432-maria";
    assert_eq!(docs(&st, "notas"), [id]);
    let e = s.typed(&mut st, "", ms(t0, 200));
    assert!(e.work.is_none(), "nota com id fixado nao e apagada: so grava o vazio (debounce)");
    let e = s.tick(&mut st, ms(t0, 800));
    s.run(&mut st, e, ms(t0, 800));
    assert_eq!(docs(&st, "notas"), [id]);
    assert_eq!(note_in(&st, id).1, "");
    assert_eq!(s.id_current(), Some(id), "o item continua na caixa");
}
