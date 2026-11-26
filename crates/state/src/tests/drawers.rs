use super::*;
use chrono::NaiveDate;
use tempfile::tempdir;

fn when(h: u32, m: u32) -> chrono::NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 11, 12).unwrap().and_hms_opt(h, m, 0).unwrap()
}
fn img(source: &str, file: &str) -> ImageRef {
    ImageRef { source: source.into(), file: file.into(), page: 2 }
}
fn fr() -> Framing {
    Framing { zoom: 1.5, cx: 0.5, cy: 0.5 }
}

/// Vault com 3 notas (Livro x2, Outro x1), 2 tarefas e 2 fontes.
fn vault() -> (tempfile::TempDir, Vault, AppData) {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(&d.path().join("v")).unwrap();
    let a = v.create_note("primeira\nsegunda linha", img("Livro", "a.jpg"), fr(), None, when(9, 0)).unwrap();
    let b = v.create_note("  \n  do outro livro", img("Outro", "x.png"), fr(), Some("g1".into()), when(10, 0)).unwrap();
    let c = v.create_note("mais nova", img("Livro", "b.jpg"), fr(), None, when(11, 0)).unwrap();
    let _ = (a, b, c);
    let t1 = v.create_task("achar óbito", Some(img("Livro", "a.jpg")), None, when(9, 30)).unwrap();
    v.create_task("ligar pro cartório", None, None, when(12, 0)).unwrap();
    v.set_state_task(&t1.id, StateTask::Done).unwrap();
    v.create_source("Livro", SourceKind::Folder).unwrap();
    v.create_source("Outro", SourceKind::Zip).unwrap();
    let mut ad = AppData::new(d.path().join("ad"));
    let folder = d.path().join("livro");
    std::fs::create_dir(&folder).unwrap();
    ad.set_source_path(&v.root, "Livro", folder);
    (d, v, ad)
}

fn ids(v: &[&NoteLine]) -> Vec<String> {
    v.iter().map(|p| p.text.clone()).collect()
}

#[test]
fn label_and_the_first_line_not_empty_cut() {
    assert_eq!(label("primeira\nsegunda"), "primeira");
    assert_eq!(label("\n  \n  depois do branco  \nx"), "depois do branco");
    assert_eq!(label("   "), "(sem texto)");
    let long = "á".repeat(100);
    let r = label(&long);
    assert_eq!(r.chars().count(), MAX_LABEL + 1);
    assert!(r.ends_with('…'));
}

#[test]
fn load_le_notes_tasks_and_sources_more_new_first() {
    let (_d, v, ad) = vault();
    let l = load(&v, &ad);
    let texts: Vec<_> = l.notes.iter().map(|p| p.text.as_str()).collect();
    assert_eq!(texts, ["mais nova", "do outro livro", "primeira"]);
    assert_eq!(l.notes[1].highlight.as_deref(), Some("g1"));
    assert_eq!(l.notes[2].local(), "Livro · a.jpg · p. 2");
    let t: Vec<_> = l.tasks.iter().map(|t| (t.text.as_str(), t.done)).collect();
    assert_eq!(t, [("ligar pro cartório", false), ("achar óbito", true)]);
    let f: Vec<_> = l.sources.iter().map(|f| (f.name.as_str(), f.status)).collect();
    assert_eq!(f, [("Livro", DrawerSourceStatus::Ok), ("Outro", DrawerSourceStatus::NoPath)]);
    assert!(l.unreadable.is_empty());
}

#[test]
fn file_unreadable_not_breaks_the_list() {
    let (_d, v, mut ad) = vault();
    std::fs::write(v.root.join("notas/zz-quebrada.md"), "sem frontmatter").unwrap();
    ad.set_source_path(&v.root, "Outro", v.root.join("sumiu.zip"));
    let l = load(&v, &ad);
    assert_eq!(l.notes.len(), 3);
    assert_eq!(l.unreadable, ["notas/zz-quebrada.md"]);
    assert_eq!(l.sources[1].status, DrawerSourceStatus::NotFound);
}

#[test]
fn filter_by_source() {
    let (_d, v, ad) = vault();
    let mut g = Drawers::default();
    let n = g.request_reload();
    assert!(g.loading());
    assert!(g.receive(n, load(&v, &ad)));
    assert!(!g.loading());
    g.toggle(Drawer::Notes);
    assert_eq!(ids(&g.notes_visible()), ["primeira", "mais nova", "do outro livro"], "todas, por fonte e pela ordem das imagens");
    g.clear_sources();
    g.toggle_source("Outro");
    assert_eq!(ids(&g.notes_visible()), ["do outro livro"]);
    g.clear_sources();
    g.toggle_source("Nenhuma");
    assert!(g.notes_visible().is_empty());
    assert!(g.empty().is_some_and(|h| !h.is_empty()), "vazio tem dica");
    g.clear_sources();
    assert_eq!(g.note_sources(), ["Livro", "Outro"]);

    assert_eq!(g.visible_tasks().iter().map(|t| t.text.as_str()).collect::<Vec<_>>(), ["ligar pro cartório"]);
    g.set_filter_tasks(TasksFilter::Done);
    assert_eq!(g.visible_tasks().iter().map(|t| t.text.as_str()).collect::<Vec<_>>(), ["achar óbito"]);
}

#[test]
fn response_of_reload_superseded_and_ignored_and_the_list_old_stays_ate_arrive_the_new() {
    let (_d, v, ad) = vault();
    let mut g = Drawers::default();
    let a = g.request_reload();
    assert!(g.receive(a, load(&v, &ad)));
    let old = g.request_reload();
    let fresh = g.request_reload();
    assert_eq!(g.lists().notes.len(), 3, "mostra a lista guardada enquanto recarrega");
    assert!(!g.receive(old, Lists::default()), "obsoleta");
    assert_eq!(g.lists().notes.len(), 3);
    assert!(g.loading());
    assert!(g.receive(fresh, Lists::default()));
    assert!(g.lists().notes.is_empty());
}

#[test]
fn open_close_drawers_and_width_of_the_image() {
    let mut g = Drawers::default();
    assert_eq!(g.open(), None);
    assert_eq!(g.busy_width(), 0.0);
    assert!(g.toggle(Drawer::Notes));
    assert_eq!(g.open(), Some(Drawer::Notes));
    assert_eq!(g.busy_width(), WIDTH);
    assert!(g.toggle(Drawer::Sources), "trocar de gaveta deixa aberta");
    assert_eq!(g.open(), Some(Drawer::Sources));
    assert!(!g.toggle(Drawer::Sources), "a mesma tecla fecha");
    assert_eq!(g.open(), None);
    assert_eq!(Drawer::key_by_the("4"), None);
}

#[test]
fn shortcuts_follow_the_order_of_the_icons() {
    crate::app_state::test::register_shortcuts_false();
    // R25: barra de atividades Notas, Fontes, Tarefas = Cmd-1, Cmd-2, Cmd-3.
    assert_eq!(Drawer::key_by_the("1"), Some(Drawer::Notes));
    assert_eq!(Drawer::key_by_the("2"), Some(Drawer::Sources));
    assert_eq!(Drawer::key_by_the("3"), Some(Drawer::Tasks));
    assert_eq!(Drawer::ALL, [Drawer::Notes, Drawer::Sources, Drawer::Tasks], "ordem dos icones");
    for (i, g) in Drawer::ALL.iter().enumerate() {
        let n = (i + 1).to_string();
        assert_eq!(Drawer::key_by_the(&n), Some(*g));
        assert_eq!(g.shortcut(), format!("⌘{n}"));
        assert!(g.hint().ends_with(&format!("(⌘{n})")), "{}", g.hint());
        assert!(g.hint().starts_with(&g.title()), "{}", g.hint());
    }
}

#[test]
fn selection_walks_and_stays_inside_of_the_list_when_ela_shrinks() {
    let (_d, v, ad) = vault();
    let mut g = Drawers::default();
    let n = g.request_reload();
    g.receive(n, load(&v, &ad));
    g.toggle(Drawer::Notes);
    g.clear_sources();
    g.toggle_source("Outro");
    assert_eq!(g.selection(), Some(0));
    g.relocate_selection(5);
    assert_eq!(g.selection(), Some(0), "so uma linha");
    assert_eq!(g.note_selected().unwrap().text, "do outro livro");
    // a lista some (filtro sem linhas): nada selecionado, sem panico
    g.clear_sources();
    g.toggle_source("Nenhuma");
    assert_eq!(g.selection(), None);
    assert!(g.note_selected().is_none());
    g.clear_sources();
    assert_eq!(g.notes_visible().len(), 3);
    g.relocate_selection(1);
    assert_eq!(g.selection(), Some(1));
    g.relocate_selection(-7);
    assert_eq!(g.selection(), Some(0));
    g.toggle(Drawer::Tasks);
    assert_eq!(g.selected_task().unwrap().text, "ligar pro cartório");
    g.mark_task(&g.selected_task().unwrap().id.clone(), true);
    assert!(g.selected_task().is_none(), "a feita sai das abertas");
    g.toggle(Drawer::Sources);
    assert_eq!(g.selected_source().unwrap().name, "Livro");
}

#[test]
fn drawers_keys() {
    use DrawerAction::*;
    assert_eq!(drawer_key(Drawer::Notes, "down"), Some(Relocate(1)));
    assert_eq!(drawer_key(Drawer::Notes, "up"), Some(Relocate(-1)));
    assert_eq!(drawer_key(Drawer::Notes, "enter"), Some(Open));
    assert_eq!(drawer_key(Drawer::Notes, "p"), None, "promover acabou");
    assert_eq!(drawer_key(Drawer::Tasks, "x"), Some(ToggleDone));
    assert_eq!(drawer_key(Drawer::Tasks, "p"), None);
    // W/A/S/D sao da imagem (fix round 1): `d` nao descarta mais
    for g in Drawer::ALL {
        for k in ["w", "a", "s", "d"] {
            assert_eq!(drawer_key(g, k), None, "{g:?} {k}");
        }
    }
    assert_eq!(drawer_key(Drawer::Sources, "enter"), Some(Open));
    assert_eq!(drawer_key(Drawer::Sources, "escape"), Some(Close));
    // as teclas da imagem continuam livres
    for k in ["f", "i", "g", "t", "r", "[", "]", "/", "left", "right"] {
        assert_eq!(drawer_key(Drawer::Notes, k), None, "{k}");
    }
}

// ---- Task 5: excluir de vez ----

#[test]
fn keys_of_edit_ended_and_as_other_follow() {
    use DrawerAction::*;
    for g in Drawer::ALL {
        assert_eq!(drawer_key(g, "e"), None, "editar acabou: clicar na linha ja edita");
        assert_eq!(drawer_key(g, "u"), None);
    }
    assert_eq!(drawer_key(Drawer::Tasks, "x"), Some(ToggleDone));
}

#[test]
fn request_deletion_counts_tasks_that_cite_and_se_the_highlight_stays_and_not_erases_nothing() {
    let (_d, v, ad) = vault();
    let mut g = Drawers::default();
    let n = g.request_reload();
    g.receive(n, load(&v, &ad));
    let open_all = v.list_notes().unwrap();
    let (open, _) = open_all.iter().find(|(_, t)| t.starts_with("primeira")).unwrap();
    let (with_highlight, _) = open_all.iter().find(|(_, t)| t.contains("do outro livro")).unwrap();
    // duas tarefas citam a primeira nota
    v.create_task("citando 1", None, Some(open.id.clone()), when(13, 0)).unwrap();
    v.create_task("citando 2", None, Some(open.id.clone()), when(13, 5)).unwrap();
    let n = g.request_reload();
    g.receive(n, load(&v, &ad));

    let p = g.request_deletion(&open.id).unwrap().clone();
    assert_eq!((p.id.as_str(), p.kind, p.tasks, p.highlight), (open.id.as_str(), DocKind::Note, 2, false));
    assert_eq!(g.deletion_confirming().map(|c| c.id.as_str()), Some(open.id.as_str()));
    assert!(v.root.join(format!("notas/{}.md", open.id)).exists(), "pedir nao apaga");
    assert!(p.warning().contains("2 tarefa"), "{}", p.warning());
    g.cancel_deletion();
    assert!(g.deletion_confirming().is_none());
    assert!(v.root.join(format!("notas/{}.md", open.id)).exists());

    // a nota do outro livro tem grifo (g1): o aviso diz que ele fica
    let p = g.request_deletion(&with_highlight.id).unwrap().clone();
    assert_eq!((p.tasks, p.highlight), (0, true));
    assert!(p.warning().contains("grifo"), "{}", p.warning());
    // tarefa
    let t = g.lists().tasks[0].id.clone();
    let p = g.request_deletion(&t).unwrap();
    assert_eq!(p.kind, DocKind::Task);
    // id que nao esta nas listas: aviso, sem pedido
    g.cancel_deletion();
    assert!(g.request_deletion("nao-existe").unwrap_err().contains("nao-existe"));
    assert!(g.deletion_confirming().is_none());
}

#[test]
fn warning_of_deletion_of_note_cites_the_highlight_and_the_of_task_not() {
    let note = RequestDeletion { id: "a".into(), kind: DocKind::Note, text: "Maria herdeira".into(), tasks: 0, highlight: true };
    assert_eq!(note.title(), "Excluir esta nota?");
    assert_eq!(
        note.warning(),
        "O arquivo é apagado do vault e não dá para desfazer. O grifo ligado a ela continua na imagem."
    );
    let task = RequestDeletion { id: "b".into(), kind: DocKind::Task, text: "achar óbito".into(), tasks: 0, highlight: false };
    assert_eq!(task.title(), "Excluir esta tarefa?");
    assert_eq!(task.warning(), "O arquivo é apagado do vault e não dá para desfazer.");
    let cited = RequestDeletion { tasks: 2, highlight: false, ..note.clone() };
    assert!(cited.warning().contains("2 tarefa(s) que a citam continuam"), "{}", cited.warning());
    // o trecho mostra so o comeco de um texto longo
    let long = RequestDeletion { text: "x".repeat(300), ..note };
    assert!(long.stretch().chars().count() <= 81 && long.stretch().ends_with('…'), "{}", long.stretch());
}

#[test]
fn same_id_in_notes_and_tasks_confirms_and_remove_the_kind_right() {
    let (_d, v, ad) = vault();
    let mut g = Drawers::default();
    let n = g.request_reload();
    let mut l = load(&v, &ad);
    let mut t = l.tasks[0].clone();
    t.id = l.notes[0].id.clone();
    let id = t.id.clone();
    l.tasks.push(t);
    g.receive(n, l);
    g.toggle(Drawer::Tasks);
    g.request_deletion(&id).unwrap();
    g.deletion_key("tab");
    assert_eq!(g.deletion_key("enter"), DeletionAction::Confirm(id.clone(), DocKind::Task));
    g.remove(&id, DocKind::Task);
    assert!(g.lists().tasks.iter().all(|t| t.id != id));
    assert!(g.lists().notes.iter().any(|p| p.id == id), "a nota de mesmo id fica");
    g.toggle(Drawer::Notes);
    g.request_deletion(&id).unwrap();
    g.deletion_key("tab");
    assert_eq!(g.deletion_key("enter"), DeletionAction::Confirm(id.clone(), DocKind::Note));
    g.remove(&id, DocKind::Note);
    assert!(g.lists().notes.iter().all(|p| p.id != id));
}

/// Grava a ordem das chamadas do clique numa linha.
#[derive(Default)]
struct Writer {
    steps: Vec<&'static str>,
    other_source: bool,
    failure: bool,
}

impl LineClick for Writer {
    fn open_image(&mut self) -> Result<bool, String> {
        self.steps.push("open_nota");
        if self.failure { Err("fonte sumiu".into()) } else { Ok(self.other_source) }
    }
    fn load(&mut self) {
        self.steps.push("carregar");
    }
    fn load_on_enter(&mut self) {
        self.steps.push("carregar_ao_entrar");
    }
}

#[test]
fn click_in_the_line_opens_the_image_before_of_load_the_text() {
    let mut g = Writer::default();
    assert_eq!(click_line(&mut g), Ok(()));
    assert_eq!(g.steps, ["open_nota", "carregar"]);
    // nota de outra fonte: o texto espera a fonte entrar (a troca terminaria a nota carregada)
    let mut g = Writer { other_source: true, ..Default::default() };
    assert_eq!(click_line(&mut g), Ok(()));
    assert_eq!(g.steps, ["open_nota", "carregar_ao_entrar"]);
    // a imagem nao abriu: nada e carregado
    let mut g = Writer { failure: true, ..Default::default() };
    assert!(click_line(&mut g).is_err());
    assert_eq!(g.steps, ["open_nota"]);
}

#[test]
fn focus_outside_of_the_drawer_cancels_the_confirmation_no_delete() {
    let (_d, v, ad) = vault();
    let mut g = Drawers::default();
    let n = g.request_reload();
    g.receive(n, load(&v, &ad));
    g.toggle(Drawer::Notes);
    let id = g.lists().notes[1].id.clone();
    g.request_deletion(&id).unwrap();
    g.deletion_key("tab");
    // o foco continua na gaveta (clique num botao do quadro): o pedido fica
    g.focus_changed(true);
    assert!(g.deletion_confirming().is_some());
    // foco na imagem ou na nota (clique, Cmd-K): o pedido cai, nada e excluido
    g.focus_changed(false);
    assert!(g.deletion_confirming().is_none());
    assert_eq!(g.deletion_key("enter"), DeletionAction::Nothing);
    assert!(v.root.join(format!("notas/{id}.md")).exists());
    assert_eq!(g.open(), Some(Drawer::Notes), "a gaveta continua aberta");
}

#[test]
fn enter_so_confirms_with_the_focus_in_the_button_delete() {
    let (_d, v, ad) = vault();
    let mut g = Drawers::default();
    let n = g.request_reload();
    g.receive(n, load(&v, &ad));
    let id = g.lists().notes[1].id.clone();
    let file = v.root.join(format!("notas/{id}.md"));
    // o dialogo abre com o foco em Cancelar: Enter cancela, nada e apagado
    g.request_deletion(&id).unwrap();
    assert_eq!(g.deletion_focus(), DeletionButton::Cancel);
    assert_eq!(g.deletion_key("enter"), DeletionAction::Cancel);
    assert!(g.deletion_confirming().is_none());
    assert!(file.exists(), "Enter em Cancelar nao apaga");
    // Tab (ou as setas) leva o foco a Excluir; so entao Enter (ou espaco) confirma
    g.request_deletion(&id).unwrap();
    assert_eq!(g.deletion_key("tab"), DeletionAction::Nothing);
    assert_eq!(g.deletion_focus(), DeletionButton::Delete);
    assert_eq!(g.deletion_key("left"), DeletionAction::Nothing);
    assert_eq!(g.deletion_focus(), DeletionButton::Cancel);
    assert_eq!(g.deletion_key("right"), DeletionAction::Nothing);
    assert_eq!(g.deletion_key("space"), DeletionAction::Confirm(id.clone(), DocKind::Note));
    // confirmar so devolve a acao: quem chama exclui; o arquivo ainda esta la
    assert!(file.exists());
    // um pedido novo volta o foco a Cancelar
    g.cancel_deletion();
    g.request_deletion(&id).unwrap();
    assert_eq!(g.deletion_focus(), DeletionButton::Cancel);
    g.focus_button_deletion(DeletionButton::Delete);
    assert_eq!(g.deletion_key("enter"), DeletionAction::Confirm(id.clone(), DocKind::Note));
    // Esc cancela com o foco em qualquer botao
    assert_eq!(g.deletion_key("escape"), DeletionAction::Cancel);
    assert!(g.deletion_confirming().is_none());
    assert_eq!(g.deletion_key("tab"), DeletionAction::Nothing, "sem pedido nada acontece");
}

#[test]
fn confirmation_enter_confirms_esc_cancels_and_the_rest_and_swallowed() {
    let (_d, v, ad) = vault();
    let mut g = Drawers::default();
    let n = g.request_reload();
    g.receive(n, load(&v, &ad));
    let id = g.lists().notes[1].id.clone();
    assert_eq!(g.deletion_key("enter"), DeletionAction::Nothing, "sem pedido nada acontece");
    g.request_deletion(&id).unwrap();
    assert_eq!(g.deletion_key("p"), DeletionAction::Nothing);
    assert!(g.deletion_confirming().is_some(), "outra tecla nao cancela nem confirma");
    assert_eq!(g.deletion_key("escape"), DeletionAction::Cancel);
    assert!(g.deletion_confirming().is_none());
    g.request_deletion(&id).unwrap();
    assert_eq!(g.deletion_key("tab"), DeletionAction::Nothing);
    assert_eq!(g.deletion_key("enter"), DeletionAction::Confirm(id.clone(), DocKind::Note));
    assert!(g.deletion_confirming().is_some(), "o pedido fica ate dar certo (recusa nao fecha)");
    g.remove(&id, DocKind::Note);
    assert!(g.deletion_confirming().is_none(), "excluida: o pedido sai");
    // fechar a gaveta ou trocar de aba descarta o pedido
    let other = g.lists().notes[0].id.clone();
    g.request_deletion(&other).unwrap();
    g.close();
    assert!(g.deletion_confirming().is_none());
}

#[test]
fn removal_optimistic_removes_the_line_and_the_reload_confirms() {
    let (_d, v, ad) = vault();
    let mut g = Drawers::default();
    let n = g.request_reload();
    g.receive(n, load(&v, &ad));
    let id = g.lists().notes[2].id.clone();
    let t = g.lists().tasks[1].id.clone();
    g.remove(&id, DocKind::Note);
    g.remove(&t, DocKind::Task);
    assert!(g.lists().notes.iter().all(|p| p.id != id));
    assert!(g.lists().tasks.iter().all(|x| x.id != t));
    // o pedido de uma linha que sumiu na recarga morre junto
    let other = g.lists().notes[0].id.clone();
    g.request_deletion(&other).unwrap();
    v.delete_note(&other).unwrap();
    v.delete_task(&t).unwrap();
    let n = g.request_reload();
    g.receive(n, load(&v, &ad));
    assert!(g.deletion_confirming().is_none());
    // tarefa que cita uma nota excluida segue na lista
    assert_eq!(g.lists().tasks.len(), 1);
}

// ---- filtros da gaveta de notas: busca no texto, fonte e etiquetas ----

/// Gavetas com 4 notas (mais nova primeiro): texto, fonte e etiquetas
/// conhecidos, para os filtros.
fn with_filters() -> Drawers {
    let line = |id: &str, note_body: &str, source: &str| NoteLine {
        record: None,
        id: id.into(),
        text: label(note_body),
        note_body: note_body.into(),
        body_min: note_body.to_lowercase(),
        tags: marginalia_core::tags::tags(note_body),
        image: img(source, "a.jpg"),
        framing: fr(),
        highlight: None,
    };
    let mut g = Drawers::default();
    let n = g.request_reload();
    let l = Lists {
        notes: vec![
            line("4", "Batismo de Maria #achado #batismo", "Livro"),
            line("3", "Casamento dos Alves\nsegunda linha com Herdeira #achado", "Outro"),
            line("2", "Inventario #hipotese", "Livro"),
            line("1", "sem etiqueta nenhuma", "Outro"),
        ],
        ..Lists::default()
    };
    g.receive(n, l);
    g.toggle(Drawer::Notes);
    g
}

fn which(g: &Drawers) -> Vec<String> {
    g.notes_visible().iter().map(|p| p.id.clone()).collect()
}

#[test]
fn search_finds_in_the_text_whole_no_distinguish_textbox() {
    let mut g = with_filters();
    g.set_search("herdeira");
    assert_eq!(which(&g), ["3"], "acha na segunda linha, que o rotulo nao mostra");
    g.set_search("BATISMO");
    assert_eq!(which(&g), ["4"]);
    g.set_search("  ");
    assert_eq!(which(&g), ["2", "4", "1", "3"], "so espacos nao filtra");
    g.set_search("");
    assert_eq!(g.notes_filters().text, "");
}

#[test]
fn tags_combine_by_and() {
    let mut g = with_filters();
    g.toggle_tag("achado");
    assert_eq!(which(&g), ["4", "3"]);
    g.toggle_tag("batismo");
    assert_eq!(which(&g), ["4"], "as duas ligadas: so a que tem as duas");
    g.toggle_tag("hipotese");
    assert!(which(&g).is_empty());
    g.toggle_tag("hipotese");
    g.toggle_tag("batismo");
    assert_eq!(which(&g), ["4", "3"], "desligar volta");
    g.toggle_tag("ACHADO");
    assert!(g.notes_filters().tags.is_empty(), "maiuscula e a mesma etiqueta");
}

#[test]
fn source_and_tag_and_text_together() {
    let mut g = with_filters();
    g.clear_sources();
    g.toggle_source("Outro");
    g.toggle_tag("achado");
    assert_eq!(which(&g), ["3"]);
    g.set_search("alves");
    assert_eq!(which(&g), ["3"]);
    g.set_search("maria");
    assert!(which(&g).is_empty(), "o E dos tres filtros");
}

#[test]
fn count_visible_and_total() {
    let mut g = with_filters();
    assert_eq!(g.count_notes(), (4, 4));
    assert_eq!(g.counter_notes(), "4 notas");
    g.toggle_tag("achado");
    assert_eq!(g.count_notes(), (2, 4));
    assert_eq!(g.counter_notes(), "2 de 4");
    g.set_search("zzz");
    assert_eq!((g.count_notes(), g.counter_notes().as_str()), ((0, 4), "0 de 4"));
}

#[test]
fn tags_of_the_vault_no_repeated_in_order() {
    let g = with_filters();
    assert_eq!(g.vault_tags(), ["achado", "batismo", "hipotese"]);
}

#[test]
fn switch_of_vault_zeroes_os_three_filters() {
    let mut g = with_filters();
    g.clear_sources();
    g.toggle_source("Livro");
    g.toggle_tag("achado");
    g.set_search("maria");
    g.clear_other_vault_filters();
    assert_eq!(g.notes_filters(), &NotesFilters::default());
    assert_eq!(which(&g).len(), 4);
}

#[test]
fn no_result_says_no_note_with_these_filters() {
    let mut g = with_filters();
    assert_eq!(g.empty(), None);
    g.set_search("nada disso existe");
    assert_eq!(g.empty().as_deref(), Some("Nenhuma nota com esses filtros"));
    let mut empty = Drawers::default();
    let n = empty.request_reload();
    empty.receive(n, Lists::default());
    empty.toggle(Drawer::Notes);
    assert!(empty.empty().is_some_and(|h| h.starts_with("Nenhuma nota ainda")), "sem filtro e sem notas: a dica de sempre");
}

#[test]
fn selection_continues_inside_of_the_list_when_the_filter_shrinks() {
    let mut g = with_filters();
    g.relocate_selection(3);
    assert_eq!(g.selection(), Some(3));
    g.toggle_tag("batismo");
    assert_eq!(g.selection(), Some(0), "uma linha so");
    assert_eq!(g.note_selected().unwrap().id, "4");
    g.set_search("zzz");
    assert_eq!(g.selection(), None);
    assert!(g.note_selected().is_none());
}

#[test]
fn reload_removes_of_the_filter_the_tag_that_vanished_of_the_vault() {
    let mut g = with_filters();
    g.toggle_tag("hipotese");
    assert_eq!(which(&g), ["2"]);
    // o texto da nota "2" foi editado e perdeu a etiqueta: a recarga nao
    // pode deixar um filtro ligado que nenhum botao mostra mais
    let n = g.request_reload();
    let mut l = g.lists().clone();
    l.notes.iter_mut().find(|p| p.id == "2").unwrap().tags.clear();
    g.receive(n, l);
    assert!(g.notes_filters().tags.is_empty());
    assert_eq!(which(&g).len(), 4);
}

#[test]
fn several_sources_marked_count_by_or() {
    let mut g = with_filters();
    g.toggle_source("Livro");
    assert_eq!(which(&g), ["2", "4"]);
    g.toggle_source("Outro");
    assert_eq!(which(&g).len(), 4, "as duas marcadas: qualquer uma");
    g.toggle_source("Livro");
    assert_eq!(which(&g), ["1", "3"]);
    g.clear_sources();
    assert_eq!(which(&g).len(), 4);
    g.toggle_source("Outro");
    g.toggle_tag("achado");
    assert_eq!(which(&g), ["3"], "fontes (OU entre si) e etiquetas (E) combinam");
}

#[test]
fn local_not_repeats_the_name_of_the_source_in_the_path() {
    let i = |f: &str, a: &str| ImageRef { source: f.into(), file: a.into(), page: 2 };
    assert_eq!(image_location(&i("Livro", "Livro/001.jpg")), "Livro · 001.jpg · p. 2");
    assert_eq!(image_location(&i("Livro", "Livro/sub/001.jpg")), "Livro · sub/001.jpg · p. 2");
    assert_eq!(image_location(&i("Livro", "001.jpg")), "Livro · 001.jpg · p. 2");
    assert_eq!(image_location(&i("Livro", "LivroB/001.jpg")), "Livro · LivroB/001.jpg · p. 2");
}

#[test]
fn cut_of_scroll_of_the_bar() {
    assert_eq!(scroll_bar(100.0, 100.0, 0.0), None, "cabe: sem barra");
    let (top, alt) = scroll_bar(100.0, 400.0, 0.0).unwrap();
    assert_eq!((top, alt), (0.0, 0.25));
    let (top, alt) = scroll_bar(100.0, 400.0, -300.0).unwrap();
    assert!((top - 0.75).abs() < 1e-6 && (alt - 0.25).abs() < 1e-6, "no fim");
    let (_, alt) = scroll_bar(10.0, 10_000.0, 0.0).unwrap();
    assert!(alt >= 0.05, "o cursor nunca some: {alt}");
}

#[test]
fn list_long_and_cut_with_count() {
    let mut g = Drawers::default();
    let n = g.request_reload();
    let note = |i: usize| NoteLine {
        record: None,
        id: format!("{i:05}"),
        text: "x".into(),
        note_body: "x".into(),
        body_min: "x".into(),
        tags: Vec::new(),
        image: img("Livro", "a.jpg"),
        framing: fr(),
        highlight: None,
    };
    let task = |i: usize| LineTask {
        record: None,
        id: format!("{i:05}"),
        text: "t".into(),
        note_body: "t".into(),
        done: false,
        image: None,
        note: None,
        note_deleted: false,
    };
    let l = Lists {
        notes: (0..MAX_LINES + 5).map(note).collect(),
        tasks: (0..MAX_LINES).map(task).collect(),
        ..Lists::default()
    };
    g.receive(n, l);
    g.toggle(Drawer::Notes);
    assert_eq!(g.cut(), Some((MAX_LINES, MAX_LINES + 5)));
    assert_eq!(g.cut_warning().as_deref(), Some("Mostrando 300 de 305 — use os filtros"));
    g.toggle(Drawer::Tasks);
    assert_eq!(g.cut(), None, "exatamente o limite: sem corte");
    g.toggle(Drawer::Sources);
    assert_eq!(g.cut_warning(), None);
}

#[test]
fn reload_keeps_the_line_chosen_by_the_id_and_os_filters() {
    let (_d, v, ad) = vault();
    let mut g = Drawers::default();
    let n = g.request_reload();
    g.receive(n, load(&v, &ad));
    g.toggle(Drawer::Tasks);
    g.set_filter_tasks(TasksFilter::Open);
    assert_eq!(g.selected_task().unwrap().text, "ligar pro cartório");
    // alguem cria por fora uma tarefa mais nova: ela entra no topo
    v.create_task("nova de fora", None, None, when(23, 0)).unwrap();
    let n = g.request_reload();
    g.receive(n, load(&v, &ad));
    assert_eq!(g.visible_tasks()[0].text, "nova de fora");
    assert_eq!(g.selected_task().unwrap().text, "ligar pro cartório", "a selecao segue a linha");
    assert_eq!(g.tasks_filter(), TasksFilter::Open);
}

#[test]
fn rename_source_in_the_drawers_follows_the_filter_and_as_lines() {
    let (_d, v, ad) = vault();
    let mut g = Drawers::default();
    let n = g.request_reload();
    g.receive(n, load(&v, &ad));
    g.clear_sources();
    g.toggle_source("Livro");
    g.rename_source("Livro", "Livro 3");
    assert_eq!(g.notes_filters().sources.iter().collect::<Vec<_>>(), ["Livro 3"]);
    assert!(g.lists().notes.iter().all(|p| p.image.source != "Livro"));
    assert!(g.sources().iter().any(|f| f.name == "Livro 3"));
    g.clear_other_vault_filters();
    assert!(g.notes_filters().sources.is_empty());
}

#[test]
fn delete_item_so_with_line_chosen_focus_in_the_drawer_and_no_confirmation() {
    let (_d, v, ad) = vault();
    let mut g = Drawers::default();
    let n = g.request_reload();
    g.receive(n, load(&v, &ad));
    assert!(!g.can_delete_item(true), "nenhuma gaveta aberta");
    g.toggle(Drawer::Sources);
    assert!(!g.can_delete_item(true), "gaveta de fontes nao exclui item");
    g.toggle(Drawer::Notes);
    assert!(g.can_delete_item(true));
    assert!(!g.can_delete_item(false), "foco fora da gaveta");
    g.toggle(Drawer::Tasks);
    assert!(g.can_delete_item(true), "tarefas tambem");
    let id = g.selected_task().unwrap().id.clone();
    g.request_deletion(&id).unwrap();
    assert!(!g.can_delete_item(true), "confirmacao aberta");
    g.cancel_deletion();
    g.toggle(Drawer::Notes);
    g.set_search("zzz");
    assert!(!g.can_delete_item(true), "sem linha escolhida");
}

#[test]
fn check_deletion_closes_the_confirmation_of_the_line_that_vanished() {
    let (_d, v, ad) = vault();
    let mut g = Drawers::default();
    let n = g.request_reload();
    g.receive(n, load(&v, &ad));
    g.toggle(Drawer::Notes);
    let id = g.note_selected().unwrap().id.clone();
    g.request_deletion(&id).unwrap();
    assert_eq!(g.check_deletion(&id, DocKind::Note), Ok(()));
    assert!(g.deletion_confirming().is_some(), "a linha existe: a confirmacao fica");
    let e = g.check_deletion("nao-existe", DocKind::Note).unwrap_err();
    assert_eq!(e, "nao encontrei \"nao-existe\" nas gavetas: nada foi excluido");
    assert!(g.deletion_confirming().is_none(), "linha sumida: a confirmacao fecha");
}

#[test]
fn texts_of_the_drawers_and_of_the_dialog_of_deletion_are_os_of_today() {
    crate::app_state::test::register_shortcuts_false();
    assert_eq!(Drawer::Notes.title(), "Notas");
    assert_eq!(Drawer::Sources.hint(), "Fontes (⌘2)");
    assert_eq!(Drawer::Tasks.shortcut(), "⌘3");
    assert_eq!(TasksFilter::Done.label(), "feitas");
    assert_eq!(DrawerSourceStatus::NoPath.label(), "sem caminho nesta maquina");
    assert_eq!(DrawerSourceStatus::NotFound.label(), "nao encontrada");
    assert_eq!(label("   \n  "), "(sem texto)");
    let p = RequestDeletion { id: "x".into(), kind: DocKind::Task, text: "t".into(), tasks: 0, highlight: false };
    assert_eq!(p.title(), "Excluir esta tarefa?");
    assert_eq!(p.warning(), "O arquivo é apagado do vault e não dá para desfazer.");
    let p = RequestDeletion { kind: DocKind::Note, tasks: 2, highlight: true, ..p };
    assert_eq!(p.title(), "Excluir esta nota?");
    assert_eq!(
        p.warning(),
        "O arquivo é apagado do vault e não dá para desfazer. 2 tarefa(s) que a citam continuam, como \"nota excluída\". O grifo ligado a ela continua na imagem."
    );
    let mut g = Drawers::default();
    g.toggle(Drawer::Sources);
    assert_eq!(g.empty().as_deref(), Some("Nenhuma fonte neste vault: ⌘O abre uma pasta ou um zip"));
    assert_eq!(g.request_deletion("nada").unwrap_err(), "nao encontrei \"nada\" nas gavetas: nada foi excluido");
}

// ---- registros: arvore nas gavetas ----

fn record_at(v: &Vault, name: &str, source: &str, ini: &str, end: Option<&str>, h: u32) -> marginalia_core::model::Record {
    let pos = |a: &str| marginalia_core::model::Position { file: a.into(), page: 2 };
    let r = v.create_record(name, source, pos(ini), when(h, 0)).unwrap();
    match end {
        Some(f) => v.update_record(&r.id, |r| r.end = Some(pos(f))).unwrap(),
        None => r,
    }
}

/// Vault de fonte "Livro" com notas em a..e.jpg (pagina 2, como `img`), criadas fora de ordem.
fn order_vault() -> (tempfile::TempDir, Vault, AppData) {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(&d.path().join("v")).unwrap();
    for (h, fname) in [(9, "d.jpg"), (10, "b.jpg"), (11, "a.jpg"), (12, "c.jpg"), (13, "e.jpg")] {
        v.create_note(&format!("nota {fname}"), img("Livro", fname), fr(), None, when(h, 0)).unwrap();
    }
    let ad = AppData::new(d.path().join("ad"));
    (d, v, ad)
}

fn loaded(v: &Vault, ad: &AppData) -> Drawers {
    let mut g = Drawers::default();
    let n = g.request_reload();
    assert!(g.receive(n, load(v, ad)));
    g
}

fn texts(g: &Drawers) -> Vec<String> {
    g.notes_visible().iter().map(|p| p.text.clone()).collect()
}

fn lines(g: &Drawers) -> Vec<String> {
    g.tree_notes()
        .iter()
        .map(|l| match l {
            crate::tree::TreeLine::Source { name, .. } => format!("F:{name}"),
            crate::tree::TreeLine::Record { record, total, collapsed, valid, .. } => {
                format!("R:{}({total}){}{}", record.name, if *collapsed { "-" } else { "" }, if *valid { "" } else { "!" })
            }
            crate::tree::TreeLine::Item(k) | crate::tree::TreeLine::Drops(k) => format!("I:{}", g.notes_visible()[*k].text),
        })
        .collect()
}

#[test]
fn no_records_the_source_and_one_header_and_as_notes_follow_the_order_of_the_images() {
    let (_d, v, ad) = order_vault();
    let g = loaded(&v, &ad);
    assert!(g.lists().records.is_empty());
    assert_eq!(texts(&g), ["nota a.jpg", "nota b.jpg", "nota c.jpg", "nota d.jpg", "nota e.jpg"], "pela ordem das imagens");
    assert_eq!(lines(&g)[0], "F:Livro");
    assert!(g.tree_notes().iter().skip(1).all(|l| matches!(l, crate::tree::TreeLine::Drops(_))));
    assert!(!d_has_folder_records(&_d));
}

fn d_has_folder_records(d: &tempfile::TempDir) -> bool {
    d.path().join("v/registros").exists()
}

#[test]
fn with_records_as_notes_comes_grouped_by_source_and_record_in_the_order_of_the_images() {
    let (_d, v, ad) = order_vault();
    record_at(&v, "R1", "Livro", "b.jpg", Some("c.jpg"), 8);
    let g = loaded(&v, &ad);
    assert_eq!(
        lines(&g),
        ["F:Livro", "R:R1(2)", "I:nota b.jpg", "I:nota c.jpg", "I:nota a.jpg", "I:nota d.jpg", "I:nota e.jpg"]
    );
    assert_eq!(texts(&g), ["nota b.jpg", "nota c.jpg", "nota a.jpg", "nota d.jpg", "nota e.jpg"], "registros primeiro, soltas depois");
}

#[test]
fn notes_loose_comes_after_of_the_records_of_the_source_in_the_order_of_the_images() {
    let (_d, v, ad) = order_vault();
    record_at(&v, "R1", "Livro", "a.jpg", Some("a.jpg"), 8);
    record_at(&v, "R2", "Livro", "d.jpg", None, 8);
    let g = loaded(&v, &ad);
    assert_eq!(
        lines(&g),
        ["F:Livro", "R:R1(1)", "I:nota a.jpg", "R:R2(2)", "I:nota d.jpg", "I:nota e.jpg", "I:nota b.jpg", "I:nota c.jpg"]
    );
}

#[test]
fn in_the_same_image_the_order_and_the_of_the_id() {
    let (_d, v, ad) = order_vault();
    v.create_note("nota a2", img("Livro", "a.jpg"), fr(), None, when(7, 0)).unwrap();
    let g = loaded(&v, &ad);
    // sem registros: ordem de hoje; com um registro que abrange tudo: ordem de criacao dentro da imagem
    record_at(&v, "R", "Livro", "a.jpg", None, 6);
    let g2 = loaded(&v, &ad);
    let t = texts(&g2);
    let i2 = t.iter().position(|x| x == "nota a2").unwrap();
    let i1 = t.iter().position(|x| x == "nota a.jpg").unwrap();
    assert!(i2 < i1, "a2 foi criada antes (id menor): {t:?}");
    assert_eq!(texts(&g).len(), 6);
}

#[test]
fn note_in_the_boundary_goes_to_the_first_record_and_the_override_the_takes_to_the_second() {
    let (_d, v, ad) = order_vault();
    let a = record_at(&v, "A", "Livro", "a.jpg", Some("c.jpg"), 8);
    let b = record_at(&v, "B", "Livro", "c.jpg", Some("e.jpg"), 8);
    let g = loaded(&v, &ad);
    assert_eq!(&lines(&g)[..5], ["F:Livro", "R:A(3)", "I:nota a.jpg", "I:nota b.jpg", "I:nota c.jpg"]);
    let c = g.notes_visible().into_iter().find(|p| p.text == "nota c.jpg").unwrap().clone();
    assert_eq!(g.note_record(&c).unwrap().id, a.id);
    assert_eq!(g.relocate_destination(&c.image, c.record.as_deref()).unwrap().destination.as_deref(), Some(b.id.as_str()));
    v.set_record_of(DocKind::Note, &c.id, Some(&b.id)).unwrap();
    let g = loaded(&v, &ad);
    let c = g.notes_visible().into_iter().find(|p| p.text == "nota c.jpg").unwrap().clone();
    assert_eq!(g.note_record(&c).unwrap().id, b.id);
    assert!(g.relocate_destination(&c.image, c.record.as_deref()).unwrap().back);
    assert!(lines(&g).contains(&"R:B(3)".to_string()), "{:?}", lines(&g));
}

#[test]
fn override_to_record_that_not_contains_the_image_and_ignored() {
    let (_d, v, ad) = order_vault();
    let a = record_at(&v, "A", "Livro", "a.jpg", Some("b.jpg"), 8);
    let b = record_at(&v, "B", "Livro", "d.jpg", Some("e.jpg"), 8);
    let _ = a;
    let target = v.list_notes().unwrap().into_iter().find(|(p, _)| p.image.file == "b.jpg").unwrap().0;
    v.set_record_of(DocKind::Note, &target.id, Some(&b.id)).unwrap();
    let g = loaded(&v, &ad);
    assert_eq!(&lines(&g)[..4], ["F:Livro", "R:A(2)", "I:nota a.jpg", "I:nota b.jpg"]);
}

#[test]
fn record_collapsed_hides_as_notes_and_the_total_appears() {
    let (_d, v, ad) = order_vault();
    let a = record_at(&v, "A", "Livro", "a.jpg", Some("c.jpg"), 8);
    let mut g = loaded(&v, &ad);
    g.toggle_record(&a.id);
    assert!(g.collapsed(&a.id));
    assert_eq!(lines(&g), ["F:Livro", "R:A(3)-", "I:nota d.jpg", "I:nota e.jpg"]);
    assert_eq!(texts(&g), ["nota d.jpg", "nota e.jpg"]);
    g.toggle_record(&a.id);
    assert_eq!(texts(&g).len(), 5);
}

#[test]
fn filter_that_hides_all_as_notes_of_the_record_hides_the_record() {
    let (_d, v, ad) = order_vault();
    record_at(&v, "A", "Livro", "a.jpg", Some("b.jpg"), 8);
    record_at(&v, "Vazio", "Livro", "z.jpg", None, 8);
    let mut g = loaded(&v, &ad);
    assert!(lines(&g).contains(&"R:Vazio(0)".to_string()), "sem filtro o registro vazio aparece");
    g.set_search("e.jpg");
    assert_eq!(lines(&g), ["F:Livro", "I:nota e.jpg"]);
    g.set_search("b.jpg");
    assert_eq!(lines(&g), ["F:Livro", "R:A(1)", "I:nota b.jpg"]);
}

#[test]
fn record_invalid_appears_marked_and_not_groups() {
    let (_d, v, ad) = order_vault();
    record_at(&v, "Quebrado", "Livro", "d.jpg", Some("b.jpg"), 8);
    let g = loaded(&v, &ad);
    let l = lines(&g);
    assert!(l.contains(&"R:Quebrado(0)!".to_string()), "{l:?}");
    assert_eq!(texts(&g).len(), 5);
}

#[test]
fn selection_and_enter_follow_the_order_of_the_tree() {
    let (_d, v, ad) = order_vault();
    record_at(&v, "A", "Livro", "b.jpg", Some("c.jpg"), 8);
    let mut g = loaded(&v, &ad);
    g.toggle(Drawer::Notes);
    assert_eq!(g.note_selected().unwrap().text, "nota b.jpg", "o registro vem primeiro");
    g.relocate_selection(1);
    assert_eq!(g.note_selected().unwrap().text, "nota c.jpg");
    g.relocate_selection(1);
    assert_eq!(g.note_selected().unwrap().text, "nota a.jpg", "depois as soltas");
}

#[test]
fn record_unreadable_enters_in_unreadable_and_as_notes_load() {
    let (_d, v, ad) = order_vault();
    std::fs::create_dir_all(v.root.join("registros")).unwrap();
    std::fs::write(v.root.join("registros/quebrado.md"), "sem frontmatter").unwrap();
    let g = loaded(&v, &ad);
    assert_eq!(g.lists().unreadable, ["registros/quebrado.md"]);
    assert_eq!(texts(&g).len(), 5);
}

#[test]
fn tasks_group_by_the_record_of_the_image() {
    let (_d, v, ad) = order_vault();
    record_at(&v, "A", "Livro", "b.jpg", Some("c.jpg"), 8);
    v.create_task("t fora", Some(img("Livro", "a.jpg")), None, when(14, 0)).unwrap();
    v.create_task("t dentro", Some(img("Livro", "b.jpg")), None, when(15, 0)).unwrap();
    v.create_task("t sem imagem", None, None, when(16, 0)).unwrap();
    let g = loaded(&v, &ad);
    let l: Vec<String> = g
        .tasks_tree()
        .iter()
        .map(|l| match l {
            crate::tree::TreeLine::Source { name, .. } => format!("F:{name}"),
            crate::tree::TreeLine::Record { record, total, .. } => format!("R:{}({total})", record.name),
            crate::tree::TreeLine::Item(k) | crate::tree::TreeLine::Drops(k) => format!("I:{}", g.visible_tasks()[*k].text),
        })
        .collect();
    assert_eq!(l, ["F:Livro", "R:A(1)", "I:t dentro", "I:t fora", "F:", "I:t sem imagem"]);
}

#[test]
fn tasks_no_image_stay_together_in_the_end() {
    let (_d, v, ad) = order_vault();
    v.create_task("velha", None, None, when(14, 0)).unwrap();
    v.create_task("nova", None, None, when(15, 0)).unwrap();
    let g = loaded(&v, &ad);
    assert_eq!(g.visible_tasks().iter().map(|t| t.text.as_str()).collect::<Vec<_>>(), ["nova", "velha"], "sem imagem: ordem recebida");
    assert!(g.tasks_tree().iter().skip(1).all(|l| matches!(l, crate::tree::TreeLine::Drops(_))));
}

#[test]
fn filter_open_done_is_worth_inside_of_the_records() {
    let (_d, v, ad) = order_vault();
    record_at(&v, "A", "Livro", "a.jpg", Some("c.jpg"), 8);
    let t = v.create_task("feita", Some(img("Livro", "b.jpg")), None, when(14, 0)).unwrap();
    v.create_task("aberta", Some(img("Livro", "b.jpg")), None, when(15, 0)).unwrap();
    v.set_state_task(&t.id, StateTask::Done).unwrap();
    let mut g = loaded(&v, &ad);
    assert_eq!(g.visible_tasks().iter().map(|t| t.text.as_str()).collect::<Vec<_>>(), ["aberta"]);
    g.set_filter_tasks(TasksFilter::Done);
    assert_eq!(g.visible_tasks().iter().map(|t| t.text.as_str()).collect::<Vec<_>>(), ["feita"]);
}

#[test]
fn record_no_tasks_of_the_filter_current_not_appears() {
    let (_d, v, ad) = order_vault();
    record_at(&v, "A", "Livro", "a.jpg", Some("c.jpg"), 8);
    let g = loaded(&v, &ad);
    assert!(g.tasks_tree().is_empty());
}

#[test]
fn task_in_the_boundary_follows_the_default_and_the_override() {
    let (_d, v, ad) = order_vault();
    let _a = record_at(&v, "A", "Livro", "a.jpg", Some("c.jpg"), 8);
    let b = record_at(&v, "B", "Livro", "c.jpg", Some("e.jpg"), 8);
    let t = v.create_task("na fronteira", Some(img("Livro", "c.jpg")), None, when(14, 0)).unwrap();
    let g = loaded(&v, &ad);
    let n = |g: &Drawers, name: &str| g.tasks_tree().iter().find_map(|l| match l {
        crate::tree::TreeLine::Record { record, total, .. } if record.name == name => Some(*total),
        _ => None,
    });
    assert_eq!((n(&g, "A"), n(&g, "B")), (Some(1), None), "B some: sem tarefas");
    v.set_record_of(DocKind::Task, &t.id, Some(&b.id)).unwrap();
    let g = loaded(&v, &ad);
    assert_eq!((n(&g, "A"), n(&g, "B")), (None, Some(1)));
}

#[test]
fn width_starts_in_the_default_and_stays_between_the_min_and_the_max() {
    let mut g = Drawers::default();
    assert_eq!(g.width(), WIDTH);
    g.set_width(100.0);
    assert_eq!(g.width(), WIDTH_MIN);
    g.set_width(5000.0);
    assert_eq!(g.width(), WIDTH_MAX);
    g.set_width(f32::NAN);
    assert_eq!(g.width(), WIDTH, "valor invalido volta ao padrao");
    g.set_width(410.0);
    assert_eq!(g.width(), 410.0);
    assert_eq!(g.busy_width(), 0.0, "fechada nao ocupa");
    g.toggle(Drawer::Notes);
    assert_eq!(g.busy_width(), 410.0);
}

#[test]
fn offset_of_the_bar_centers_the_thumb_in_the_click() {
    // visivel 100, total 400: polegar de 25% (alt 0.25), max 300
    assert_eq!(bar_offset(0.0, 100.0, 400.0), 0.0, "topo");
    assert_eq!(bar_offset(100.0, 100.0, 400.0), -300.0, "fundo");
    let middle = bar_offset(50.0, 100.0, 400.0);
    assert!((middle + 150.0).abs() < 1e-3, "{middle}");
    assert_eq!(bar_offset(50.0, 100.0, 100.0), 0.0, "cabe: nada rola");
    assert_eq!(bar_offset(-30.0, 100.0, 400.0), 0.0, "fora do topo");
}

#[test]
fn first_lines_cuts_with_ellipsis_and_skips_empty() {
    assert_eq!(lines_first("uma", 5), "uma");
    assert_eq!(lines_first("a\n\n\nb\n", 5), "a\nb");
    assert_eq!(lines_first("1\n2\n3\n4\n5", 5), "1\n2\n3\n4\n5");
    assert_eq!(lines_first("1\n2\n3\n4\n5\n6\n7", 5), "1\n2\n3\n4\n5…");
    assert_eq!(lines_first("", 5), "");
}

#[test]
fn kind_of_the_source_by_the_kind_and_by_the_extension() {
    let f = |kind, file_path: Option<&str>| LineSource {
        name: "F".into(),
        kind,
        status: DrawerSourceStatus::Ok,
        file_path: file_path.map(PathBuf::from),
    };
    assert_eq!(f(SourceKind::Zip, Some("/x/livro.zip")).kind(), DrawerSourceKind::Zip);
    assert_eq!(f(SourceKind::Folder, Some("/x/livro.PDF")).kind(), DrawerSourceKind::Pdf);
    assert_eq!(f(SourceKind::Folder, Some("/x/foto.jpeg")).kind(), DrawerSourceKind::Image);
    assert_eq!(f(SourceKind::Folder, Some("/x/foto.tif")).kind(), DrawerSourceKind::Image);
    assert_eq!(f(SourceKind::Folder, Some("/x/Livro 1.2")).kind(), DrawerSourceKind::Folder);
    assert_eq!(f(SourceKind::Folder, None).kind(), DrawerSourceKind::Folder);
}
