//! Carregar um item existente na caixa (clique numa linha) e o que vai ao disco.

use super::*;

#[test]
fn safe_the_id_of_a_load_requested() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "Ana", t0);
    s.run(&mut st, e, t0);
    let e = s.finish(&mut st, ms(t0, 100));
    s.run(&mut st, e, ms(t0, 100));
    let id = "20261112-1432-ana";
    assert!(!s.safe(id), "gravada, fixada e solta");
    let e = s.load(&mut st, DocKind::Note, id, ms(t0, 200));
    assert!(s.safe(id), "a carga do texto dele esta pedida");
    s.run(&mut st, e, ms(t0, 200));
    assert!(!s.safe(id), "carregada, fixada e nada por gravar: livre");
}

#[test]
fn load_edits_note_quick_note_and_task_existing_no_rename() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let v = st.vault.clone().unwrap();
    let img = st.current.image.clone();
    let fr = st.framing().clone();
    let p = v.create_note("Velha nota", img.clone(), fr.clone(), None, data()).unwrap();
    let mut s = session();
    let t0 = Instant::now();
    let e = s.load(&mut st, DocKind::Note, &p.id, t0);
    let e = s.run(&mut st, e, t0);
    assert_eq!(e.loaded.as_deref(), Some("Velha nota"));
    assert_eq!(s.id_current(), Some(p.id.as_str()));
    s.typed(&mut st, "Velha nota editada", ms(t0, 100));
    let e = s.tick(&mut st, ms(t0, 700));
    s.run(&mut st, e, ms(t0, 700));
    assert!(s.tick(&mut st, ms(t0, 30_000)).work.is_none(), "nota carregada nao e renomeada");
    assert_eq!(docs(&st, "notas"), [p.id.clone()]);
    assert_eq!(note_in(&st, &p.id).1, "Velha nota editada");
    let e = s.finish(&mut st, ms(t0, 30_100));
    assert!(s.run(&mut st, e, ms(t0, 30_100)).clear_textbox);
    assert_eq!(docs(&st, "notas"), [p.id.clone()]);

    // tarefa ligada a uma nota que ja nao existe (regra 6): edita sem quebrar
    let tf = v.create_task("ligada", Some(img), Some("20990101-0000-apagada".into()), data()).unwrap();
    let e = s.load(&mut st, DocKind::Task, &tf.id, ms(t0, 46_000));
    assert_eq!(s.run(&mut st, e, ms(t0, 46_000)).loaded.as_deref(), Some("ligada"));
    s.typed(&mut st, "ligada e editada", ms(t0, 46_100));
    let e = s.tick(&mut st, ms(t0, 60_000));
    s.run(&mut st, e, ms(t0, 60_000));
    let (tf2, note_body): (Task, String) = parse(&read_file(&st, "tarefas", &tf.id)).unwrap();
    assert_eq!((tf2.note.as_deref(), note_body.as_str()), (Some("20990101-0000-apagada"), "ligada e editada"));
}

#[test]
fn click_in_a_line_ends_the_quick_note_current_and_loads_the_text() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    let img1 = st.current.image.clone();
    let e = s.typed(&mut st, "Ana na primeira", t0);
    s.run(&mut st, e, t0);
    let e = s.finish(&mut st, ms(t0, 100));
    assert!(s.run(&mut st, e, ms(t0, 100)).clear_textbox);
    let ana = "20261112-1432-ana-na-primeira";
    assert_eq!(docs(&st, "notas"), [ana]);

    // outra imagem: nota multilinha em edicao, a ultima tecla ainda no debounce
    go_to_next(&mut st);
    let e = s.typed(&mut st, "Bia\nsegunda", ms(t0, 1000));
    s.run(&mut st, e, ms(t0, 1000));
    assert!(s.typed(&mut st, "Bia\nsegunda linha", ms(t0, 1100)).work.is_none(), "debounce");

    // clique na linha de Ana: a nota de Bia termina inteira (texto e id) antes de carregar
    let e = line_click(&mut s, &mut st, ana, ms(t0, 1200));
    assert!(e.clear_textbox, "a caixa limpa da nota de Bia");
    assert_eq!(e.loaded.as_deref(), Some("Ana na primeira"));
    assert_eq!(st.current.image, img1, "a imagem da nota abriu");
    let bia: Vec<String> = docs(&st, "notas").into_iter().filter(|d| d != ana).collect();
    assert_eq!(bia.len(), 1, "{bia:?}");
    assert!(bia[0].starts_with("20261112-1432-bia") && !bia[0].contains("sem-texto"), "id fixado: {bia:?}");
    let (p, note_body) = note_in(&st, &bia[0]);
    assert_eq!(note_body, "Bia\nsegunda linha", "nada perdido, as duas linhas");
    assert_ne!(p.image, img1, "a nota de Bia e da imagem em que comecou");
    assert_eq!(s.id_current(), Some(ana));

    // editar o texto carregado grava em Ana, sem renomear
    s.typed(&mut st, "Ana na primeira\nmadrinha", ms(t0, 1300));
    let e = s.tick(&mut st, ms(t0, 2000));
    s.run(&mut st, e, ms(t0, 2000));
    assert_eq!(note_in(&st, ana).1, "Ana na primeira\nmadrinha");
    assert!(s.tick(&mut st, ms(t0, 30_000)).work.is_none(), "carregada nao renomeia");

    // na MESMA imagem: outra nota em edicao e o clique na linha de Ana tambem a termina primeiro
    let e = s.finish(&mut st, ms(t0, 30_100));
    s.run(&mut st, e, ms(t0, 30_100));
    let e = s.typed(&mut st, "Caio", ms(t0, 31_000));
    s.run(&mut st, e, ms(t0, 31_000));
    s.typed(&mut st, "Caio padrinho", ms(t0, 31_100));
    let e = line_click(&mut s, &mut st, ana, ms(t0, 31_200));
    assert_eq!(e.loaded.as_deref(), Some("Ana na primeira\nmadrinha"));
    assert_eq!(note_in(&st, "20261112-1432-caio-padrinho").1, "Caio padrinho");
    assert_eq!(s.id_current(), Some(ana));
    // clicar de novo na linha que ja esta na caixa devolve o texto da caixa, sem disco
    s.typed(&mut st, "Ana na primeira\nmadrinha Rosa", ms(t0, 31_300));
    let e = line_click(&mut s, &mut st, ana, ms(t0, 31_400));
    assert_eq!(e.loaded.as_deref(), Some("Ana na primeira\nmadrinha Rosa"));
    assert!(!e.clear_textbox);
}

#[test]
fn load_cleared_the_textbox_and_warns_that_is_loading() {
    // Minor 7: entre pedir o texto e ele chegar, a caixa nao pode ficar com o texto
    // da nota que terminou (uma tecla ali criaria outra nota com ele).
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let v = st.vault.clone().unwrap();
    let p = v.create_note("Velha", st.current.image.clone(), st.framing().clone(), None, data()).unwrap();
    let mut s = session();
    let t0 = Instant::now();
    let e = s.typed(&mut st, "Bia", t0);
    s.run(&mut st, e, t0);
    assert!(!s.loading());
    let e = s.load(&mut st, DocKind::Note, &p.id, ms(t0, 100));
    assert!(e.clear_textbox, "a nota de Bia saiu da caixa: a caixa limpa ja");
    assert!(s.loading(), "a view desliga a caixa ate o texto chegar");
    let e = s.run(&mut st, e, ms(t0, 100));
    assert_eq!(e.loaded.as_deref(), Some("Velha"));
    assert!(!s.loading());
    assert_eq!(note_in(&st, "20261112-1432-bia").1, "Bia");
    // carregar o que ja esta na caixa nao limpa nem espera
    let e = s.load(&mut st, DocKind::Note, &p.id, ms(t0, 200));
    assert!(!e.clear_textbox && !s.loading());
    assert_eq!(e.loaded.as_deref(), Some("Velha"));
    // carga que falha (id que nao existe): o carregando acaba, com aviso
    let e = s.load(&mut st, DocKind::Note, "20990101-0000-nao-existe", ms(t0, 300));
    let e = s.run(&mut st, e, ms(t0, 300));
    assert!(!s.loading());
    assert!(e.loaded.is_none() && e.warning.is_some(), "{:?}", e.warning);
}

#[test]
fn the_that_goes_to_the_disk_and_the_text_no_spaces_in_the_tips() {
    // Minor 8: o corpo gravado e `text.trim()` (pontas do texto inteiro); recuos e
    // linhas em branco DENTRO do texto ficam. Tarefa: tambem tira o `[]` do inicio.
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let mut s = session();
    let t0 = Instant::now();
    let textbox = "  \n  Linha um\n    recuo interno\n\n\tfim com tab  \n\n";
    let e = s.typed(&mut st, textbox, t0);
    s.run(&mut st, e, t0);
    let e = s.finish(&mut st, ms(t0, 100));
    s.run(&mut st, e, ms(t0, 100));
    let id = docs(&st, "notas").pop().unwrap();
    assert_eq!(note_in(&st, &id).1, "Linha um\n    recuo interno\n\n\tfim com tab");
    // carregar devolve o que esta no disco (e e isso que a caixa mostra ao editar de novo)
    let e = s.load(&mut st, DocKind::Note, &id, ms(t0, 200));
    assert_eq!(s.run(&mut st, e, ms(t0, 200)).loaded.as_deref(), Some("Linha um\n    recuo interno\n\n\tfim com tab"));
    let e = s.finish(&mut st, ms(t0, 300));
    s.run(&mut st, e, ms(t0, 300));
    let e = s.typed(&mut st, "  [] comprar\n  livro  ", ms(t0, 400));
    s.run(&mut st, e, ms(t0, 400));
    let e = s.finish(&mut st, ms(t0, 500));
    s.run(&mut st, e, ms(t0, 500));
    let tid = docs(&st, "tarefas").pop().unwrap();
    let (_, note_body): (Task, String) = parse(&read_file(&st, "tarefas", &tid)).unwrap();
    assert_eq!(note_body, "comprar\n  livro");
}

#[test]
fn typing_after_of_request_to_load_discards_the_load() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let v = st.vault.clone().unwrap();
    let p = v.create_note("Velha", st.current.image.clone(), st.framing().clone(), None, data()).unwrap();
    let mut s = session();
    let t0 = Instant::now();
    let e = s.load(&mut st, DocKind::Note, &p.id, t0);
    let tr = e.work.unwrap();
    s.typed(&mut st, "x", ms(t0, 10));
    let e = s.conclude(&mut st, tr.execute(), ms(t0, 20));
    assert!(e.loaded.is_none(), "o usuario ja digitou outra coisa: a carga e obsoleta");
    let e = s.run(&mut st, e, ms(t0, 20));
    assert!(e.loaded.is_none());
    assert_eq!(note_in(&st, &p.id).1, "Velha", "a nota carregada nao recebeu o texto novo");
}
