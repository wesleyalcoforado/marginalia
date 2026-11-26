use chrono::{NaiveDate, NaiveDateTime};
use marginalia_core::model::*;
use marginalia_core::vault::*;
use marginalia_core::Error;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn now() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 11, 12).unwrap().and_hms_opt(14, 32, 0).unwrap()
}
fn img(source: &str) -> ImageRef {
    ImageRef { source: source.into(), file: "a.jpg".into(), page: 3 }
}
fn fr() -> Framing {
    Framing { zoom: 2.0, cx: 0.5, cy: 0.25 }
}

#[test]
fn open_or_create_creates_folders() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(&d.path().join("v")).unwrap();
    for p in ["fontes", "notas", "tarefas", "grifos"] {
        assert!(v.root.join(p).is_dir(), "{p}");
    }
}

#[test]
fn create_note_writes_and_list() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let p = v.create_note("Maria Alves herdeira", img("livro"), fr(), Some("g1".into()), now()).unwrap();
    assert_eq!(p.id, "20261112-1432-maria-alves-herdeira");
    assert!(d.path().join("notas/20261112-1432-maria-alves-herdeira.md").is_file());
    let l = v.list_notes().unwrap();
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].0, p);
    assert_eq!(l[0].1, "Maria Alves herdeira");
}

#[test]
fn collision_in_the_same_minute() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let a = v.create_note("herdeira", img("l"), fr(), None, now()).unwrap();
    let b = v.create_note("herdeira", img("l"), fr(), None, now()).unwrap();
    assert_eq!(a.id, "20261112-1432-herdeira");
    assert_eq!(b.id, "20261112-1432-herdeira-2");
    assert!(d.path().join("notas/20261112-1432-herdeira-2.md").is_file());
}

#[test]
fn source_duplicate_and_name_safe() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    v.create_source("Inventario 1850", SourceKind::Zip).unwrap();
    assert!(matches!(v.create_source("Inventario 1850", SourceKind::Zip), Err(Error::DuplicateName(_))));
    let f = v.create_source("a/b", SourceKind::Folder).unwrap();
    assert!(!d.path().join("fontes/a").exists());
    assert!(d.path().join("fontes").join(format!("{}.md", f.name)).is_file());
    assert!(!f.name.contains('/'));
    assert!(v.create_source("  ", SourceKind::Folder).is_err());
    assert_eq!(v.list_sources().unwrap().len(), 2);
}

#[test]
fn rename_source_rewrites_references() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    v.create_source("A", SourceKind::Zip).unwrap();
    fs::write(d.path().join("grifos/A.json"), "[1]").unwrap();
    let p = v.create_note("x\n\ncorpo  ", img("A"), fr(), None, now()).unwrap();
    let t = v.create_task("achar", Some(img("A")), None, now()).unwrap();
    let other = v.create_note("y", img("Z"), fr(), None, now()).unwrap();
    v.rename_source("A", "B").unwrap();
    assert!(!d.path().join("fontes/A.md").exists());
    assert!(!d.path().join("grifos/A.json").exists());
    assert_eq!(fs::read_to_string(d.path().join("grifos/B.json")).unwrap(), "[1]");
    let sources = v.list_sources().unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].name, "B");
    let ps = v.list_notes().unwrap();
    let pn = ps.iter().find(|(x, _)| x.id == p.id).unwrap();
    assert_eq!(pn.0.image.source, "B");
    assert_eq!(pn.1, "x\n\ncorpo  ");
    assert_eq!(ps.iter().find(|(x, _)| x.id == other.id).unwrap().0.image.source, "Z");
    let ts = v.list_tasks().unwrap();
    assert_eq!(ts.iter().find(|(x, _)| x.id == t.id).unwrap().0.image.as_ref().unwrap().source, "B");
}

#[test]
fn rename_source_errors() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    v.create_source("A", SourceKind::Zip).unwrap();
    v.create_source("B", SourceKind::Zip).unwrap();
    assert!(matches!(v.rename_source("A", "B"), Err(Error::DuplicateName(_))));
    assert!(matches!(v.rename_source("Q", "C"), Err(Error::NotFound(_))));
}

#[test]
fn input_routes() {
    assert_eq!(route_quick_note_input("[] achar óbito de João"), QuickNoteInput::Task("achar óbito de João".into()));
    assert_eq!(route_quick_note_input("  []x "), QuickNoteInput::Task("x".into()));
    assert_eq!(route_quick_note_input("Maria herdeira"), QuickNoteInput::Note("Maria herdeira".into()));
}

#[test]
fn find_by_id_note_and_task() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let p = v.create_note("p um", img("l"), fr(), None, now()).unwrap();
    let t = v.create_task("t um", None, Some(p.id.clone()), now()).unwrap();
    match v.find_by_id(&p.id).unwrap() {
        Some(Found::Note(x, b)) => {
            assert_eq!(x, p);
            assert_eq!(b, "p um");
        }
        o => panic!("{o:?}"),
    }
    match v.find_by_id(&t.id).unwrap() {
        Some(Found::Task(x, b)) => {
            assert_eq!(x, t);
            assert_eq!(b, "t um");
        }
        o => panic!("{o:?}"),
    }
    assert!(v.find_by_id("nao-existe").unwrap().is_none());
}

#[test]
fn characters_special_in_image_forward_and_back() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let image = ImageRef { source: "livro".into(), file: "pasta/Conceição: \"1ª\" 🙂.jpg".into(), page: 7 };
    let body = "Inventário: \"A\" 🙂\n- item";
    v.create_note(body, image.clone(), fr(), None, now()).unwrap();
    let l = v.list_notes().unwrap();
    assert_eq!(l[0].0.image, image);
    assert_eq!(l[0].1, body);
}

#[test]
fn rename_source_rejects_old_outside_of_sources() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(&d.path().join("v")).unwrap();
    let decoy = d.path().join("x.md");
    let content = "---\nnome: x\ntipo: zip\n---\n";
    fs::write(&decoy, content).unwrap();
    let r = v.rename_source("../x", "novo");
    assert!(r.is_err());
    assert_eq!(fs::read_to_string(&decoy).unwrap(), content);
    assert!(!d.path().join("v/fontes/novo.md").exists());
}

#[test]
fn rename_source_not_leaves_state_partial() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    v.create_source("A", SourceKind::Zip).unwrap();
    fs::write(d.path().join("grifos/A.json"), "[1]").unwrap();
    let p = v.create_note("ok", img("A"), fr(), None, now()).unwrap();
    // Ilegivel de verdade (erro de leitura, nao de parse): aborta sem mexer em nada.
    // Um `.md` sem frontmatter nao cita fonte e e pulado (ver o teste do AppleDouble).
    fs::create_dir(d.path().join("notas/zzz-ilegivel.md")).unwrap();
    let e = v.rename_source("A", "B").unwrap_err().to_string();
    assert!(e.contains("zzz-ilegivel.md"), "{e}");
    assert!(d.path().join("fontes/A.md").is_file());
    assert!(!d.path().join("fontes/B.md").exists());
    assert!(d.path().join("grifos/A.json").is_file());
    let doc = fs::read_to_string(d.path().join(format!("notas/{}.md", p.id))).unwrap();
    assert!(doc.contains("source: A"));
}

#[test]
fn mark_task_done_and_reopen_preserves_the_rest() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let t = v.create_task("achar óbito", Some(img("livro")), Some("p1".into()), now()).unwrap();
    let path = d.path().join(format!("tarefas/{}.md", t.id));
    let done = v.set_state_task(&t.id, StateTask::Done).unwrap();
    assert_eq!(done, Task { state: StateTask::Done, ..t.clone() });
    let doc = fs::read_to_string(&path).unwrap();
    assert!(doc.contains("state: done") && doc.ends_with("---\nachar óbito"), "{doc}");
    // repetir e um no-op
    v.set_state_task(&t.id, StateTask::Done).unwrap();
    let open = v.set_state_task(&t.id, StateTask::Open).unwrap();
    assert_eq!(open, t);
    assert!(fs::read_to_string(&path).unwrap().contains("state: open"));
}

#[test]
fn mark_task_missing_or_id_unsafe() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    for id in ["nao-existe", "", "../x", "a/b"] {
        assert!(matches!(v.set_state_task(id, StateTask::Done), Err(Error::NotFound(_))), "{id:?}");
    }
}

#[cfg(unix)]
#[test]
fn rename_source_interrupted_resumes_no_lose_nothing() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    v.create_source("A", SourceKind::Zip).unwrap();
    fs::write(d.path().join("grifos/A.json"), "[1]").unwrap();
    let p = v.create_note("ok", img("A"), fr(), None, now()).unwrap();
    let t = v.create_task("achar", Some(img("A")), None, now()).unwrap();
    // `notes/` sem escrita: a reescrita das referencias falha no meio
    let notes = d.path().join("notas");
    fs::set_permissions(&notes, fs::Permissions::from_mode(0o555)).unwrap();
    let r = v.rename_source("A", "B");
    fs::set_permissions(&notes, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(r.is_err());
    // nada se perdeu: a fonte e os grifos antigos continuam, os novos ja existem
    assert!(d.path().join("fontes/A.md").is_file());
    assert!(d.path().join("fontes/B.md").is_file());
    assert_eq!(fs::read_to_string(d.path().join("grifos/A.json")).unwrap(), "[1]");
    assert_eq!(fs::read_to_string(d.path().join("grifos/B.json")).unwrap(), "[1]");
    assert!(fs::read_to_string(notes.join(format!("{}.md", p.id))).unwrap().contains("source: A"));
    // outra fonte nao "retoma" a renomeacao de A
    v.create_source("C", SourceKind::Zip).unwrap();
    assert!(matches!(v.rename_source("C", "B"), Err(Error::DuplicateName(_))));
    // tentar de novo termina
    v.rename_source("A", "B").unwrap();
    assert!(!d.path().join("fontes/A.md").exists());
    assert!(!d.path().join("grifos/A.json").exists());
    assert_eq!(fs::read_to_string(d.path().join("grifos/B.json")).unwrap(), "[1]");
    let names: Vec<String> = v.list_sources().unwrap().into_iter().map(|f| f.name).collect();
    assert_eq!(names, ["B", "C"]);
    assert!(!fs::read_to_string(d.path().join("fontes/B.md")).unwrap().contains("renomeando"), "marca de retomada sai no fim");
    assert_eq!(v.list_notes().unwrap().iter().find(|(x, _)| x.id == p.id).unwrap().0.image.source, "B");
    assert_eq!(v.list_tasks().unwrap().iter().find(|(x, _)| x.id == t.id).unwrap().0.image.as_ref().unwrap().source, "B");
    // sem temporarios sobrando
    for dir in ["fontes", "notas", "tarefas"] {
        assert!(fs::read_dir(d.path().join(dir)).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().ends_with(".tmp")));
    }
}

#[cfg(unix)]
#[test]
fn renaming_interrupted_so_recognizes_the_mark_of_old_and_resume_cleared_highlights_orphans() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    v.create_source("A", SourceKind::Zip).unwrap();
    v.create_source("Outra", SourceKind::Folder).unwrap();
    fs::write(d.path().join("grifos/A.json"), "[1]").unwrap();
    v.create_note("ok", img("A"), fr(), None, now()).unwrap();
    assert!(!v.interrupted_renaming("A", "B"), "B nem existe");
    assert!(!v.interrupted_renaming("A", "Outra"), "outra fonte de fato nao e marca");
    let notes = d.path().join("notas");
    fs::set_permissions(&notes, fs::Permissions::from_mode(0o555)).unwrap();
    let r = v.rename_source("A", "B");
    fs::set_permissions(&notes, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(r.is_err());
    assert!(v.interrupted_renaming("A", "B"));
    assert!(!v.interrupted_renaming("Outra", "B"), "a marca e de A");
    // parou depois de apagar fontes/A.md mas antes de apagar grifos/A.json
    fs::remove_file(d.path().join("fontes/A.md")).unwrap();
    v.rename_source("A", "B").unwrap();
    assert!(!d.path().join("grifos/A.json").exists(), "grifos orfaos de A saem");
    assert_eq!(fs::read_to_string(d.path().join("grifos/B.json")).unwrap(), "[1]");
    assert!(!v.interrupted_renaming("A", "B"), "marca tirada");
}

/// Regravar troca o arquivo inteiro (temporario + rename) em vez de truncar no
/// lugar: quem le (Obsidian, sync do NAS) nunca ve o documento pela metade.
#[cfg(unix)]
#[test]
fn rewrite_and_atomic_swap_the_file() {
    use std::os::unix::fs::MetadataExt;
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let p = v.create_note("x", img("l"), fr(), None, now()).unwrap();
    let path = d.path().join("notas").join(format!("{}.md", p.id));
    let before = fs::metadata(&path).unwrap().ino();
    v.update_text(&p.id, "y").unwrap();
    assert_ne!(fs::metadata(&path).unwrap().ino(), before, "regravou no lugar");
    let t = v.create_task("t", None, None, now()).unwrap();
    let tpath = d.path().join("tarefas").join(format!("{}.md", t.id));
    let before = fs::metadata(&tpath).unwrap().ino();
    v.set_state_task(&t.id, StateTask::Done).unwrap();
    assert_ne!(fs::metadata(&tpath).unwrap().ino(), before, "regravou no lugar");
    let leftovers: Vec<_> = fs::read_dir(d.path().join("notas")).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name()).collect();
    assert_eq!(leftovers.len(), 1, "{leftovers:?}");
}

/// Criar fonte com nome que aparece no disco entre a checagem e a gravacao nao
/// sobrescreve: `DuplicateName`.
#[test]
fn create_source_not_overwrites_existing() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    fs::write(d.path().join("fontes/livro.md"), "---\nnome: livro\ntipo: zip\n---\nmeu").unwrap();
    assert!(matches!(v.create_source("livro", SourceKind::Folder), Err(Error::DuplicateName(_))));
    assert!(fs::read_to_string(d.path().join("fontes/livro.md")).unwrap().ends_with("meu"));
}

/// Um AppleDouble (`._x.md`, criado pelo macOS em volumes de rede) nao pode
/// bloquear "Renomear fonte": nao cita fonte nenhuma.
#[test]
fn rename_ignores_appledouble() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    v.create_source("livro", SourceKind::Folder).unwrap();
    let p = v.create_note("x", img("livro"), fr(), None, now()).unwrap();
    fs::write(d.path().join("notas/._x.md"), [0u8, 5, 22, 7, 0, 2]).unwrap();
    v.rename_source("livro", "Livro novo").unwrap();
    assert!(d.path().join("fontes/Livro novo.md").is_file());
    let (Some(Found::Note(p2, _)),) = (v.find_by_id(&p.id).unwrap(),) else { panic!() };
    assert_eq!(p2.image.source, "Livro novo");
    // A lista tambem ignora o AppleDouble.
    assert_eq!(v.list_notes().unwrap().len(), 1);
}

/// Erro de leitura de um documento diz qual arquivo.
#[test]
fn error_of_document_cites_the_path() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    fs::write(d.path().join("tarefas/quebrada.md"), "---\nid: [\n---\n").unwrap();
    let e = v.list_tasks().unwrap_err().to_string();
    assert!(e.contains("quebrada.md"), "{e}");
}

/// Estado de uma renomeacao A -> B que parou depois de apagar `sources/A.md`.
fn stopped_no_old(d: &Path, v: &Vault) {
    v.create_source("A", SourceKind::Zip).unwrap();
    fs::write(d.join("fontes/B.md"), "---\nnome: B\ntipo: zip\nrenaming_from: A\n---\n").unwrap();
    fs::remove_file(d.join("fontes/A.md")).unwrap();
}

fn highlights(ids: &[&str]) -> String {
    let gs: Vec<String> = ids.iter().map(|i| format!(r#"{{"tipo":"ponteiro","id":"{i}","x":0.5,"y":0.5}}"#)).collect();
    format!(r#"{{"versao":1,"imagens":{{"a.jpg#1":[{}]}}}}"#, gs.join(","))
}

/// Retomar com grifos dos dois lados (um grifo novo pode ter ido para o nome
/// antigo enquanto a renomeacao estava parada): mescla, nao apaga nenhum.
#[test]
fn resume_merges_highlights_of_the_two_names() {
    use marginalia_core::highlights::HighlightsFile;
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    stopped_no_old(d.path(), &v);
    fs::write(d.path().join("grifos/A.json"), highlights(&["g1", "g2"])).unwrap();
    fs::write(d.path().join("grifos/B.json"), highlights(&["g1", "g3"])).unwrap();
    v.rename_source("A", "B").unwrap();
    assert!(!d.path().join("grifos/A.json").exists());
    let g = HighlightsFile::load(&v, "B").unwrap();
    let ids: Vec<&str> = g.images["a.jpg#1"].iter().map(|g| g.id()).collect();
    assert_eq!(ids, ["g1", "g3", "g2"]);
}

/// So os grifos antigos sobraram: passam para o nome novo.
#[test]
fn resume_with_so_highlights_old_renames() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    stopped_no_old(d.path(), &v);
    fs::write(d.path().join("grifos/A.json"), highlights(&["g2"])).unwrap();
    v.rename_source("A", "B").unwrap();
    assert!(!d.path().join("grifos/A.json").exists());
    assert_eq!(fs::read_to_string(d.path().join("grifos/B.json")).unwrap(), highlights(&["g2"]));
}

/// Grifos ilegiveis num dos lados: nao apaga nada e o erro aparece (a
/// renomeacao continua retomavel).
#[test]
fn resume_with_highlights_unreadable_not_erases_and_warns() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    stopped_no_old(d.path(), &v);
    fs::write(d.path().join("grifos/A.json"), "{ quebrado").unwrap();
    fs::write(d.path().join("grifos/B.json"), highlights(&["g1"])).unwrap();
    let e = v.rename_source("A", "B").unwrap_err().to_string();
    assert!(e.contains("A.json"), "{e}");
    assert_eq!(fs::read_to_string(d.path().join("grifos/A.json")).unwrap(), "{ quebrado");
    assert_eq!(fs::read_to_string(d.path().join("grifos/B.json")).unwrap(), highlights(&["g1"]));
    assert!(v.interrupted_renaming("A", "B"));
}

/// Documento com frontmatter que nao parseia mas cita a fonte (ex.: `pagina: "1"`):
/// a renomeacao recusa citando o arquivo, sem mexer em nada (pular deixaria a
/// citacao com o nome velho). Sem frontmatter, ou yaml que nao cita: nao bloqueia.
#[test]
fn rename_refuses_document_unreadable_that_cites_the_source() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    v.create_source("A", SourceKind::Zip).unwrap();
    let p = v.create_note("ok", img("A"), fr(), None, now()).unwrap();
    fs::write(d.path().join("notas/solta.md"), "sem frontmatter").unwrap();
    fs::write(d.path().join("notas/de-outra-fonte.md"), "---\nid: x\nimagem:\n  fonte: Z\n  pagina: \"1\"\n---\n").unwrap();
    let bad = d.path().join("notas/zz-ruim.md");
    fs::write(&bad, "---\nid: zz\nestado: aberta\nimagem:\n  fonte: A\n  arquivo: a.jpg\n  pagina: \"1\"\n---\n").unwrap();
    let e = v.rename_source("A", "B").unwrap_err().to_string();
    assert!(e.contains("zz-ruim.md"), "{e}");
    assert!(d.path().join("fontes/A.md").is_file());
    assert!(!d.path().join("fontes/B.md").exists());
    assert!(fs::read_to_string(d.path().join(format!("notas/{}.md", p.id))).unwrap().contains("source: A"));
    // Consertado o arquivo, a renomeacao passa (os outros dois nao bloqueiam).
    fs::write(&bad, "---\nid: zz\nestado: aberta\nimagem:\n  fonte: A\n  arquivo: a.jpg\n  pagina: 1\nenquadramento:\n  zoom: 1.0\n  cx: 0.5\n  cy: 0.5\n---\n").unwrap();
    v.rename_source("A", "B").unwrap();
    assert!(fs::read_to_string(&bad).unwrap().contains("source: B"));
}

// ---- v1.1: rascunho, update_texto, finalizar_id, delete_* ----

fn after() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 11, 12).unwrap().and_hms_opt(15, 7, 0).unwrap()
}

/// Todos os arquivos do vault (caminho relativo -> bytes), para provar que nada
/// alem do alvo mudou.
fn photo(root: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, d: &Path, m: &mut std::collections::BTreeMap<String, Vec<u8>>) {
        for e in fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(base, &p, m);
            } else {
                // chave sempre com "/", qualquer que seja o separador do sistema
                m.insert(p.strip_prefix(base).unwrap().to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/"), fs::read(&p).unwrap());
            }
        }
    }
    let mut m = std::collections::BTreeMap::new();
    walk(root, root, &mut m);
    m
}

fn no_tmp(root: &Path) {
    for dir in ["notas", "tarefas"] {
        for e in fs::read_dir(root.join(dir)).unwrap() {
            let n = e.unwrap().file_name().to_string_lossy().into_owned();
            assert!(!n.ends_with(".tmp"), "temporario sobrando: {dir}/{n}");
        }
    }
}

/// Frontmatter cru (tudo ate a linha `---` que fecha) e corpo.
fn raw(path: &Path) -> (String, String) {
    let s = fs::read_to_string(path).unwrap();
    let rest = s.strip_prefix("---\n").unwrap();
    let end = rest.find("\n---\n").unwrap() + 5;
    (format!("---\n{}", &rest[..end]), rest[end..].to_string())
}

/// Acrescenta `tags` no topo do frontmatter, como faria o usuario no Obsidian.
fn with_tags(path: &Path) {
    let s = fs::read_to_string(path).unwrap();
    let rest = s.strip_prefix("---\n").unwrap();
    fs::write(path, format!("---\ntags:\n- genealogia\n{rest}")).unwrap();
}

#[test]
fn draft_creates_id_provisional_any_that_is_the_text() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let p = v.create_draft("M", img("livro"), fr(), Some("g1".into()), now()).unwrap();
    assert_eq!(p.id, "20261112-1432-sem-texto");
    assert_eq!(p.highlight.as_deref(), Some("g1"));
    let path = d.path().join("notas/20261112-1432-sem-texto.md");
    assert!(path.is_file());
    assert_eq!(raw(&path).1, "M");
    let q = v.create_draft("Maria Alves", img("livro"), fr(), None, now()).unwrap();
    assert_eq!(q.id, "20261112-1432-sem-texto-2");
    assert!(marginalia_core::ids::eh_provisional(&p.id) && marginalia_core::ids::eh_provisional(&q.id));
}

/// Ids provisorios evitam as duas pastas: `update_text`/`finish_id` nunca
/// ficam em duvida entre uma nota e uma tarefa de mesmo id.
#[test]
fn drafts_avoid_ids_of_notes_and_tasks() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    fs::write(d.path().join("tarefas/20261112-1432-sem-texto.md"), "---\nid: x\nestado: aberta\n---\n").unwrap();
    let p = v.create_draft("a", img("l"), fr(), None, now()).unwrap();
    assert_eq!(p.id, "20261112-1432-sem-texto-2");
    let t = v.create_task_draft("b", None, Some(p.id.clone()), now()).unwrap();
    assert_eq!(t.id, "20261112-1432-sem-texto-3");
    assert_eq!(t.note.as_deref(), Some(p.id.as_str()));
    assert_eq!(t.state, StateTask::Open);
    assert_eq!(raw(&d.path().join("tarefas/20261112-1432-sem-texto-3.md")).1, "b");
    let p2 = v.create_draft("c", img("l"), fr(), None, now()).unwrap();
    assert_eq!(p2.id, "20261112-1432-sem-texto-4");
}

#[cfg(unix)]
#[test]
fn update_text_swap_so_the_body_of_note_and_task() {
    use std::os::unix::fs::MetadataExt;
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let p = v.create_draft("M", img("livro"), fr(), Some("g1".into()), now()).unwrap();
    let path = d.path().join(format!("notas/{}.md", p.id));
    with_tags(&path);
    let (fm, _) = raw(&path);
    let ino = fs::metadata(&path).unwrap().ino();
    v.update_text(&p.id, "Maria\nAlves: \"herdeira\" 🙂\n---\nfim").unwrap();
    assert_ne!(fs::metadata(&path).unwrap().ino(), ino, "regravou no lugar");
    let (fm2, body) = raw(&path);
    assert_eq!(fm2, fm, "frontmatter intacto (byte a byte)");
    assert_eq!(body, "Maria\nAlves: \"herdeira\" 🙂\n---\nfim");
    let (Some(Found::Note(p2, b2)),) = (v.find_by_id(&p.id).unwrap(),) else { panic!() };
    assert_eq!(b2, body);
    assert_eq!(p2.highlight.as_deref(), Some("g1"));
    assert!(p2.extra.contains_key("tags"));
    // mesmo texto: nao regrava
    let ino = fs::metadata(&path).unwrap().ino();
    v.update_text(&p.id, &body).unwrap();
    assert_eq!(fs::metadata(&path).unwrap().ino(), ino);
    // tarefa
    let t = v.create_task("achar", Some(img("livro")), Some(p.id.clone()), now()).unwrap();
    let tpath = d.path().join(format!("tarefas/{}.md", t.id));
    with_tags(&tpath);
    let (tfm, _) = raw(&tpath);
    v.update_text(&t.id, "achar o obito").unwrap();
    assert_eq!(raw(&tpath), (tfm, "achar o obito".to_string()));
    no_tmp(d.path());
}

#[test]
fn update_text_errors_not_touch_nothing() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    v.create_note("x", img("l"), fr(), None, now()).unwrap();
    fs::write(d.path().join("tarefas/20261112-1432-x.md"), "---\nid: 20261112-1432-x\nestado: aberta\n---\nx").unwrap();
    fs::write(d.path().join("notas/solta.md"), "sem frontmatter").unwrap();
    let before = photo(d.path());
    for id in ["nao-existe", "", "../x", "a/b"] {
        assert!(matches!(v.update_text(id, "y"), Err(Error::NotFound(_))), "{id:?}");
    }
    // mesmo id em notas/ e tarefas/ (vault antigo): recusa em vez de adivinhar
    let e = v.update_text("20261112-1432-x", "y").unwrap_err();
    assert!(matches!(e, Error::StateInvalid(_)), "{e}");
    // sem frontmatter: recusa citando o arquivo
    let e = v.update_text("solta", "y").unwrap_err().to_string();
    assert!(e.contains("solta.md"), "{e}");
    assert_eq!(photo(d.path()), before);
}

/// Uma gravacao em curso (`.x.md.tmp`, oculto) nunca aparece nas listas.
#[test]
fn temporary_hidden_not_appears_in_the_lists() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let p = v.create_draft("a", img("l"), fr(), None, now()).unwrap();
    fs::write(d.path().join(format!("notas/.{}.md.1.0.tmp", p.id)), "---\nid: lixo\n---\n").unwrap();
    fs::write(d.path().join("notas/.oculta.md"), "---\nid: lixo\n---\n").unwrap();
    assert_eq!(v.list_notes().unwrap().len(), 1);
}

#[test]
fn finish_renames_a_time_and_updates_references() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    fs::write(d.path().join("grifos/livro.json"), "{\"versao\":1}").unwrap();
    let p = v.create_draft("M", img("livro"), fr(), Some("g1".into()), now()).unwrap();
    let old = d.path().join(format!("notas/{}.md", p.id));
    with_tags(&old);
    v.update_text(&p.id, "Maria Alves herdeira\nsegunda linha").unwrap();
    let t = v.create_task("conferir", Some(img("livro")), Some(p.id.clone()), now()).unwrap();
    let tpath = d.path().join(format!("tarefas/{}.md", t.id));
    with_tags(&tpath);
    let foreign = v.create_task("outra", None, Some("20261112-1400-outra".into()), now()).unwrap();
    let before = photo(d.path());

    // `now` mais tarde: o id guarda o minuto em que a nota foi criada
    let new = v.finish_id(&p.id, after()).unwrap();
    assert_eq!(new, "20261112-1432-maria-alves-herdeira-segunda-linha");
    assert!(!old.exists());
    let path = d.path().join(format!("notas/{new}.md"));
    let (Some(Found::Note(np, body)),) = (v.find_by_id(&new).unwrap(),) else { panic!() };
    assert_eq!(np, Note { id: new.clone(), extra: np.extra.clone(), ..p.clone() });
    assert_eq!(body, "Maria Alves herdeira\nsegunda linha");
    assert_eq!(np.extra.len(), 1, "so `tags`, sem a marca: {:?}", np.extra);
    assert!(!fs::read_to_string(&path).unwrap().contains("renomeando"));
    assert!(v.find_by_id(&p.id).unwrap().is_none());
    // referencias
    let (Some(Found::Task(nt, tb)),) = (v.find_by_id(&t.id).unwrap(),) else { panic!() };
    assert_eq!(nt.note.as_deref(), Some(new.as_str()));
    assert_eq!(tb, "conferir");
    assert!(nt.extra.contains_key("tags"));
    // nada mais mudou
    let mut expected = before.clone();
    expected.remove(&format!("notas/{}.md", p.id));
    let mut after_ = photo(d.path());
    for k in [format!("notas/{new}.md"), format!("tarefas/{}.md", t.id)] {
        after_.remove(&k).unwrap();
        expected.remove(&k);
    }
    assert!(after_.contains_key(&format!("tarefas/{}.md", foreign.id)));
    assert_eq!(after_, expected, "so a nota e a tarefa que a cita mudaram");
    no_tmp(d.path());

    // idempotente: o id fixo devolve ele mesmo sem tocar em nada
    let f = photo(d.path());
    assert_eq!(v.finish_id(&new, after()).unwrap(), new);
    assert_eq!(photo(d.path()), f);
    // fixado uma vez: mudar o texto depois nao renomeia de novo
    v.update_text(&new, "Joao").unwrap();
    assert_eq!(v.finish_id(&new, after()).unwrap(), new);
    assert!(path.is_file());
    // listas coerentes
    let ps = v.list_notes().unwrap();
    assert_eq!(ps.iter().map(|(p, _)| p.id.as_str()).collect::<Vec<_>>(), [new.as_str()]);
    assert_eq!(v.list_tasks().unwrap().len(), 2);
}

#[test]
fn finish_with_text_empty_keeps_the_id_provisional() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let p = v.create_draft("", img("l"), fr(), None, now()).unwrap();
    let q = v.create_draft("", img("l"), fr(), None, now()).unwrap();
    v.update_text(&q.id, " ?? -- ").unwrap();
    // nota do usuario criada sem texto (slug real `no-text`, sem a marca)
    let u = v.create_note("", img("l"), fr(), None, now()).unwrap();
    assert_eq!(u.id, "20261112-1432-sem-texto-3");
    let before = photo(d.path());
    for id in [&p.id, &q.id, &u.id] {
        assert_eq!(&v.finish_id(id, after()).unwrap(), id);
    }
    assert_eq!(photo(d.path()), before);
    // ganhou texto depois: fixa agora
    v.update_text(&p.id, "Maria").unwrap();
    assert_eq!(v.finish_id(&p.id, after()).unwrap(), "20261112-1432-maria");
}

#[test]
fn finish_collision_gains_suffix_inclusive_against_other_notes_and_tasks() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let ja = v.create_note("Maria", img("l"), fr(), None, now()).unwrap();
    assert_eq!(ja.id, "20261112-1432-maria");
    fs::write(d.path().join("notas/20261112-1432-maria-2.md"), "---\nid: x\n---\noutra").unwrap();
    fs::write(d.path().join("tarefas/20261112-1432-maria-3.md"), "---\nid: x\nestado: aberta\n---\nt").unwrap();
    let before = photo(d.path());
    let p = v.create_draft("Maria", img("l"), fr(), None, now()).unwrap();
    assert_eq!(v.finish_id(&p.id, now()).unwrap(), "20261112-1432-maria-4");
    let after_ = photo(d.path());
    for (k, b) in &before {
        assert_eq!(after_.get(k), Some(b), "{k}");
    }
}

#[test]
fn finish_task_draft() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let t = v.create_task_draft("a", Some(img("l")), Some("p1".into()), now()).unwrap();
    let tp = d.path().join(format!("tarefas/{}.md", t.id));
    with_tags(&tp);
    v.update_text(&t.id, "achar óbito de João").unwrap();
    let new = v.finish_id(&t.id, now()).unwrap();
    assert_eq!(new, "20261112-1432-achar-obito-de-joao");
    assert!(!tp.exists());
    let (Some(Found::Task(nt, b)),) = (v.find_by_id(&new).unwrap(),) else { panic!() };
    assert_eq!(nt, Task { id: new.clone(), extra: nt.extra.clone(), ..t.clone() });
    assert!(nt.extra.contains_key("tags") && nt.extra.len() == 1);
    assert_eq!(b, "achar óbito de João");
    assert_eq!(v.list_tasks().unwrap().len(), 1);
}

#[test]
fn finish_errors() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    for id in ["", "../x", "a/b", "nao-existe", "20261112-1432-sem-texto"] {
        assert!(matches!(v.finish_id(id, now()), Err(Error::NotFound(_))), "{id:?}");
    }
}

/// Continuacao criada a mao, como deixaria uma finalizacao que parou depois de
/// gravar o arquivo novo.
fn continuation(d: &Path, old: &str, new: &str, note_body: &str) {
    continuation_at(d, "notas", old, new, note_body);
}

fn continuation_at(d: &Path, dir: &str, old: &str, new: &str, note_body: &str) {
    let s = fs::read_to_string(d.join(format!("{dir}/{old}.md"))).unwrap();
    let rest = s.strip_prefix("---\n").unwrap();
    let end = rest.find("\n---\n").unwrap();
    let meta = rest[..end].replacen(&format!("id: {old}"), &format!("id: {new}"), 1);
    fs::write(d.join(format!("{dir}/{new}.md")), format!("---\n{meta}\nrenaming_from: {old}\n---\n{note_body}")).unwrap();
}

/// Parou com os dois arquivos e o usuario continuou digitando no antigo: a
/// retomada termina no mesmo arquivo novo e o texto mais novo (do antigo) vence.
#[test]
fn finish_interrupted_with_os_two_files_ends_and_the_text_new_wins() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let p = v.create_draft("Maria", img("l"), fr(), Some("g".into()), now()).unwrap();
    with_tags(&d.path().join(format!("notas/{}.md", p.id)));
    continuation(d.path(), &p.id, "20261112-1432-maria", "Maria");
    v.update_text(&p.id, "Maria Alves, digitado depois").unwrap();
    let new = v.finish_id(&p.id, after()).unwrap();
    assert_eq!(new, "20261112-1432-maria", "nao cria outro arquivo");
    let ps = v.list_notes().unwrap();
    assert_eq!(ps.len(), 1, "{:?}", ps);
    assert_eq!(ps[0].1, "Maria Alves, digitado depois");
    assert_eq!(ps[0].0.highlight.as_deref(), Some("g"));
    assert!(ps[0].0.extra.contains_key("tags") && ps[0].0.extra.len() == 1, "{:?}", ps[0].0.extra);
    no_tmp(d.path());
}

/// Parou depois de apagar o antigo e antes de tirar a marca: termina.
#[test]
fn finish_interrupted_no_the_old_removes_the_mark() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let p = v.create_draft("Maria", img("l"), fr(), None, now()).unwrap();
    let t = v.create_task("t", None, Some(p.id.clone()), now()).unwrap();
    continuation(d.path(), &p.id, "20261112-1432-maria", "Maria");
    fs::remove_file(d.path().join(format!("notas/{}.md", p.id))).unwrap();
    assert_eq!(v.finish_id(&p.id, now()).unwrap(), "20261112-1432-maria");
    let s = fs::read_to_string(d.path().join("notas/20261112-1432-maria.md")).unwrap();
    assert!(!s.contains("renomeando"), "{s}");
    assert!(s.ends_with("---\nMaria"));
    let (Some(Found::Task(nt, _)),) = (v.find_by_id(&t.id).unwrap(),) else { panic!() };
    assert_eq!(nt.note.as_deref(), Some("20261112-1432-maria"));
}

/// Um arquivo com o nome-alvo mas sem a marca do antigo nao e continuacao: e
/// do usuario e fica intacto.
#[test]
fn file_no_mark_not_and_continuation() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let p = v.create_draft("Maria", img("l"), fr(), None, now()).unwrap();
    let q = v.create_draft("Maria", img("l"), fr(), None, now()).unwrap();
    let target = d.path().join("notas/20261112-1432-maria.md");
    fs::write(&target, format!("---\nid: x\nestado: aberta\nrenaming_from: {}\n---\nde outra", q.id)).unwrap();
    assert_eq!(v.finish_id(&p.id, now()).unwrap(), "20261112-1432-maria-2");
    assert!(fs::read_to_string(&target).unwrap().ends_with("de outra"));
}

/// Falha real no meio (a reescrita das referencias): nada se perde e chamar de
/// novo termina, com o texto digitado entre as duas tentativas.
#[cfg(unix)]
#[test]
fn finish_that_failure_in_the_references_resumes() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let p = v.create_draft("Maria", img("l"), fr(), None, now()).unwrap();
    let t = v.create_task("t", None, Some(p.id.clone()), now()).unwrap();
    let tasks = d.path().join("tarefas");
    fs::set_permissions(&tasks, fs::Permissions::from_mode(0o555)).unwrap();
    let r = v.finish_id(&p.id, now());
    fs::set_permissions(&tasks, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(r.is_err());
    assert!(d.path().join(format!("notas/{}.md", p.id)).is_file(), "o antigo continua");
    let middle = fs::read_to_string(d.path().join("notas/20261112-1432-maria.md")).unwrap();
    assert!(middle.contains(&format!("renaming_from: {}", p.id)), "parou com a continuacao marcada: {middle}");
    assert!(fs::read_to_string(d.path().join(format!("tarefas/{}.md", t.id))).unwrap().contains(&format!("note: {}", p.id)));
    v.update_text(&p.id, "Maria e mais").unwrap();
    let new = v.finish_id(&p.id, after()).unwrap();
    assert_eq!(new, "20261112-1432-maria");
    let ps = v.list_notes().unwrap();
    assert_eq!(ps.len(), 1);
    assert_eq!(ps[0].1, "Maria e mais");
    assert!(ps[0].0.extra.is_empty(), "{:?}", ps[0].0.extra);
    let (Some(Found::Task(nt, _)),) = (v.find_by_id(&t.id).unwrap(),) else { panic!() };
    assert_eq!(nt.note.as_deref(), Some(new.as_str()));
    no_tmp(d.path());
}

/// Uma tarefa com frontmatter que nao parseia mas cita a nota: recusa antes
/// de criar o arquivo novo (pular deixaria a citacao pendurada).
#[test]
fn finish_refuses_reference_unreadable_no_touch_in_nothing() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let p = v.create_draft("Maria", img("l"), fr(), None, now()).unwrap();
    fs::write(d.path().join("tarefas/ruim.md"), format!("---\nid: ruim\nestado: talvez\nnota: {}\n---\n", p.id)).unwrap();
    fs::write(d.path().join("tarefas/outra.md"), "---\nid: outra\nestado: talvez\nnota: z\n---\n").unwrap();
    let before = photo(d.path());
    let e = v.finish_id(&p.id, now()).unwrap_err().to_string();
    assert!(e.contains("ruim.md"), "{e}");
    assert_eq!(photo(d.path()), before);
}

#[test]
fn delete_remove_so_the_file_target() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    v.create_source("l", SourceKind::Zip).unwrap();
    fs::write(d.path().join("grifos/l.json"), "{\"versao\":1}").unwrap();
    let p = v.create_note("p", img("l"), fr(), Some("g1".into()), now()).unwrap();
    let q = v.create_note("q", img("l"), fr(), None, now()).unwrap();
    let t = v.create_task("t", None, Some(p.id.clone()), now()).unwrap();
    let mut before = photo(d.path());

    v.delete_note(&p.id).unwrap();
    before.remove(&format!("notas/{}.md", p.id));
    assert_eq!(photo(d.path()), before, "so a nota saiu (tarefa e grifos intactos)");
    assert!(v.find_by_id(&p.id).unwrap().is_none());
    let e = v.delete_note(&p.id).unwrap_err();
    assert!(matches!(e, Error::NotFound(_)) && e.to_string().contains(&p.id), "{e}");

    // tipo errado: erra e nao toca em nada
    assert!(matches!(v.delete_task(&q.id), Err(Error::NotFound(_))));
    assert!(matches!(v.delete_note(&t.id), Err(Error::NotFound(_))));
    for id in ["", "../fontes/l", "a/b"] {
        assert!(matches!(v.delete_note(id), Err(Error::NotFound(_))), "{id:?}");
        assert!(matches!(v.delete_task(id), Err(Error::NotFound(_))), "{id:?}");
    }
    assert_eq!(photo(d.path()), before);

    v.delete_task(&t.id).unwrap();
    before.remove(&format!("tarefas/{}.md", t.id));
    assert_eq!(photo(d.path()), before);
    assert!(v.find_by_id(&t.id).unwrap().is_none());
    assert_eq!(v.list_notes().unwrap().iter().map(|(x, _)| x.id.as_str()).collect::<Vec<_>>(), [q.id.as_str()]);
    assert!(v.list_tasks().unwrap().is_empty());
}

// ---- fix round 1 ----

/// Estado de uma finalizacao que parou na reescrita das referencias: A e a
/// continuacao N (marcada) existem e a tarefa ainda cita A.
#[cfg(unix)]
fn stopped_finalization(d: &Path, v: &Vault, text: &str) -> (Note, String) {
    use std::os::unix::fs::PermissionsExt;
    let p = v.create_draft(text, img("l"), fr(), None, now()).unwrap();
    v.create_task("t", None, Some(p.id.clone()), now()).unwrap();
    let tasks = d.join("tarefas");
    fs::set_permissions(&tasks, fs::Permissions::from_mode(0o555)).unwrap();
    let r = v.finish_id(&p.id, now());
    fs::set_permissions(&tasks, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(r.is_err());
    let n = format!("20261112-1432-{}", marginalia_core::ids::slug(text));
    assert!(fs::read_to_string(d.join(format!("notas/{n}.md"))).unwrap().contains("renaming_from"));
    (p, n)
}

/// I1: editar a continuacao N enquanto A existe perderia o texto (a retomada
/// regrava N a partir de A): recusa e aponta A.
#[cfg(unix)]
#[test]
fn update_text_in_the_continuation_refuses_and_nothing_se_loses() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let (p, n) = stopped_finalization(d.path(), &v, "Maria");
    let before = photo(d.path());
    for r in [v.update_text(&n, "editado em N"), v.update_text_in(DocKind::Note, &n, "editado em N")] {
        let e = r.unwrap_err();
        assert!(matches!(e, Error::StateInvalid(_)) && e.to_string().contains(&p.id), "{e}");
    }
    assert_eq!(photo(d.path()), before);
    v.update_text(&p.id, "Maria, texto mais novo").unwrap();
    assert_eq!(v.finish_id(&p.id, now()).unwrap(), n);
    let ps = v.list_notes().unwrap();
    assert_eq!(ps.len(), 1);
    assert_eq!(ps[0].1, "Maria, texto mais novo");
    // terminada, N e editavel
    v.update_text(&n, "agora sim").unwrap();
}

/// (b) Excluir A ou N com a renomeacao em curso recusa; terminada, exclui.
#[cfg(unix)]
#[test]
fn delete_with_renaming_in_progress_refuses() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let (p, n) = stopped_finalization(d.path(), &v, "Ana");
    let before = photo(d.path());
    for id in [&p.id, &n] {
        let e = v.delete_note(id).unwrap_err();
        assert!(matches!(e, Error::StateInvalid(_)) && e.to_string().contains(&p.id), "{e}");
    }
    assert_eq!(photo(d.path()), before);
    assert_eq!(v.finish_id(&p.id, now()).unwrap(), n);
    v.delete_note(&n).unwrap();
    assert!(v.list_notes().unwrap().is_empty());
}

/// I2: ids novos sao unicos nas duas pastas, tambem para create_nota/create_tarefa.
#[test]
fn ids_new_are_unique_between_notes_and_tasks() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let p = v.create_note("Maria", img("l"), fr(), None, now()).unwrap();
    let t = v.create_task("Maria", None, None, now()).unwrap();
    assert_eq!((p.id.as_str(), t.id.as_str()), ("20261112-1432-maria", "20261112-1432-maria-2"));
    let r = v.create_task_draft("Jose", None, None, now()).unwrap();
    assert_eq!(v.finish_id(&r.id, now()).unwrap(), "20261112-1432-jose");
    assert_eq!(v.create_note("Jose", img("l"), fr(), None, now()).unwrap().id, "20261112-1432-jose-2");
    fs::write(d.path().join("notas/20261112-1432-ana.md"), "---\nid: x\n---\n").unwrap();
    assert_eq!(v.create_task("Ana", None, None, now()).unwrap().id, "20261112-1432-ana-2");
}

/// I2: vault com o mesmo id em notas/ e tarefas/: a versao com
/// tipo edita so o arquivo daquele tipo; a sem tipo recusa nomeando as pastas.
#[test]
fn same_id_in_several_folders_uses_the_api_with_kind() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let id = "20261112-1432-maria";
    let note = format!("---\nid: {id}\nestado: aberta\ndraft: true\nimagem:\n  fonte: l\n  arquivo: a.jpg\n  pagina: 1\nenquadramento:\n  zoom: 1.0\n  cx: 0.5\n  cy: 0.5\n---\np");
    let task = format!("---\nid: {id}\nestado: aberta\ndraft: true\n---\nt");
    fs::write(d.path().join(format!("notas/{id}.md")), &note).unwrap();
    fs::write(d.path().join(format!("tarefas/{id}.md")), &task).unwrap();
    let before = photo(d.path());
    for e in [v.update_text(id, "x").unwrap_err(), v.finish_id(id, now()).unwrap_err()] {
        let s = e.to_string();
        assert!(matches!(e, Error::StateInvalid(_)) && s.contains("notas") && s.contains("tarefas"), "{s}");
    }
    assert_eq!(photo(d.path()), before);
    for (kind, dir) in [(DocKind::Note, "notas"), (DocKind::Task, "tarefas")] {
        v.update_text_in(kind, id, &format!("novo {dir}")).unwrap();
        assert!(fs::read_to_string(d.path().join(format!("{dir}/{id}.md"))).unwrap().ends_with(&format!("---\nnovo {dir}")));
    }
    for dir in ["notas", "tarefas"] {
        assert_eq!(fs::read_to_string(d.path().join(format!("{dir}/{id}.md"))).unwrap().matches("novo").count(), 1, "{dir}");
    }
    assert!(matches!(v.update_text_in(DocKind::Task, "nao-existe", "x"), Err(Error::NotFound(_))));
    // finalizar com tipo: so a tarefa (rascunho) e renomeada
    v.update_text_in(DocKind::Task, id, "achar obito").unwrap();
    let new = v.finish_id_in(DocKind::Task, id, now()).unwrap();
    assert_eq!(new, "20261112-1432-achar-obito");
    assert!(d.path().join(format!("notas/{id}.md")).is_file());
    assert!(!d.path().join(format!("tarefas/{id}.md")).exists());
}

/// (a) O estado provisorio esta no documento (`draft: true`), nao so no id:
/// um corpo de verdade que vira o slug `no-text[-N]` fixa no lugar; uma
/// nota do usuario (sem a marca) nunca e renomeada.
#[test]
fn provisional_comes_of_the_mark_in_the_document() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let a = v.create_draft("", img("l"), fr(), None, now()).unwrap();
    let b = v.create_draft("", img("l"), fr(), None, now()).unwrap();
    assert_eq!(b.id, "20261112-1432-sem-texto-2");
    let bp = d.path().join(format!("notas/{}.md", b.id));
    assert!(fs::read_to_string(&bp).unwrap().contains("draft: true"));
    // corpo vazio: continua rascunho
    assert_eq!(v.finish_id(&b.id, now()).unwrap(), b.id);
    assert!(fs::read_to_string(&bp).unwrap().contains("draft: true"));
    // "sem texto 2" no `-no-text-2`: fixa no lugar, sem `-2-2`
    v.update_text(&b.id, "sem texto 2").unwrap();
    assert_eq!(v.finish_id(&b.id, now()).unwrap(), b.id);
    let s = fs::read_to_string(&bp).unwrap();
    assert!(!s.contains("rascunho") && s.ends_with("---\nsem texto 2"), "{s}");
    // fixado: mudar o texto nao renomeia
    v.update_text(&b.id, "Maria").unwrap();
    assert_eq!(v.finish_id(&b.id, now()).unwrap(), b.id);
    // "Sem texto" no `-no-text`: tambem no lugar
    v.update_text(&a.id, "Sem texto").unwrap();
    assert_eq!(v.finish_id(&a.id, now()).unwrap(), a.id);
    assert!(!fs::read_to_string(d.path().join(format!("notas/{}.md", a.id))).unwrap().contains("rascunho"));
    // nota do usuario sem texto: nao e rascunho, nunca renomeia
    let u = v.create_note("", img("l"), fr(), None, now()).unwrap();
    v.update_text(&u.id, "Joao").unwrap();
    let before = photo(d.path());
    assert_eq!(v.finish_id(&u.id, now()).unwrap(), u.id);
    assert_eq!(photo(d.path()), before);
}

// ---- fix round 2 ----

fn renaming_refuses(r: std::result::Result<impl std::fmt::Debug, Error>, old: &str) {
    let e = r.unwrap_err();
    let s = e.to_string();
    assert!(matches!(e, Error::StateInvalid(_)) && s.contains("renomeação em curso") && s.contains(old), "{s}");
}

/// Excluir A ou N com a renomeacao em curso recusa (a retomada regravaria N a
/// partir de A e reapontaria a tarefa); terminada, excluir funciona.
#[cfg(unix)]
#[test]
fn delete_note_with_renaming_in_progress_refuses() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let (p, n) = stopped_finalization(d.path(), &v, "Maria");
    v.update_text(&p.id, "Maria Alves").unwrap();
    let before = photo(d.path());
    for id in [&p.id, &n] {
        renaming_refuses(v.delete_note(id), &p.id);
    }
    assert_eq!(photo(d.path()), before, "nada mudou");
    // leitura e lista continuam
    assert_eq!(v.list_notes().unwrap().len(), 2);
    assert!(v.find_by_id(&n).unwrap().is_some());

    assert_eq!(v.finish_id(&p.id, now()).unwrap(), n);
    let ps = v.list_notes().unwrap();
    assert_eq!((ps.len(), ps[0].1.as_str()), (1, "Maria Alves"));
    let ts = v.list_tasks().unwrap();
    assert_eq!(ts[0].0.note.as_deref(), Some(n.as_str()));
    v.delete_note(&n).unwrap();
    assert!(v.list_notes().unwrap().is_empty());
    // A nao volta a mexer em nada
    assert!(matches!(v.finish_id(&p.id, now()), Err(Error::NotFound(_))));
}

/// Mudar o estado da tarefa A ou N durante a renomeacao recusa (a retomada
/// regravaria N a partir de A); terminada, muda e preserva o corpo.
#[test]
fn state_of_the_task_with_renaming_in_progress_refuses() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let t = v.create_task_draft("a", None, None, now()).unwrap();
    v.update_text(&t.id, "achar obito").unwrap();
    let n = "20261112-1432-achar-obito";
    continuation_at(d.path(), "tarefas", &t.id, n, "achar obito");
    let before = photo(d.path());
    for id in [t.id.as_str(), n] {
        renaming_refuses(v.set_state_task(id, StateTask::Done), &t.id);
        renaming_refuses(v.set_state_task(id, StateTask::Open), &t.id);
    }
    assert_eq!(photo(d.path()), before);
    assert_eq!(v.finish_id(&t.id, now()).unwrap(), n);
    let done = v.set_state_task(n, StateTask::Done).unwrap();
    assert_eq!(done.state, StateTask::Done);
    let ts = v.list_tasks().unwrap();
    assert_eq!(ts.len(), 1);
    assert_eq!((ts[0].0.id.as_str(), ts[0].0.state, ts[0].1.as_str()), (n, StateTask::Done, "achar obito"));
    assert!(ts[0].0.extra.is_empty(), "{:?}", ts[0].0.extra);
}

#[test]
fn note_with_state_old_continues_opening_and_the_field_and_preserved() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let id = "20261112-1432-velha";
    let doc = "---\nid: 20261112-1432-velha\nestado: aberta\nimagem:\n  fonte: l\n  arquivo: a.jpg\n  pagina: 1\nenquadramento:\n  zoom: 1.0\n  cx: 0.5\n  cy: 0.5\n---\ntexto velho";
    fs::write(d.path().join(format!("notas/{id}.md")), doc).unwrap();
    let l = v.list_notes().unwrap();
    assert_eq!((l[0].0.id.as_str(), l[0].1.as_str()), (id, "texto velho"));
    v.update_text(id, "texto novo #achado").unwrap();
    let after = fs::read_to_string(d.path().join(format!("notas/{id}.md"))).unwrap();
    assert!(after.contains("estado: aberta"), "o campo desconhecido fica como esta: {after}");
    assert!(after.ends_with("---\ntexto novo #achado"), "{after}");
}

#[test]
fn create_note_not_writes_state() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    let p = v.create_note("sem estado", img("livro"), fr(), None, now()).unwrap();
    let doc = fs::read_to_string(d.path().join(format!("notas/{}.md", p.id))).unwrap();
    assert!(!doc.contains("estado:"), "{doc}");
}
