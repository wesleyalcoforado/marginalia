//! Trava o formato em disco (vault e appdata.json): o que o app grava hoje tem de
//! ser lido e regravado igual depois de qualquer renomeacao no codigo. Os arquivos
//! dourados ficam em `integracao/data/`. Para regerar de proposito (mudanca de
//! formato intencional): `GERAR_DOURADO=1 cargo test -p marginalia_core --test disk_format`.

use chrono::{NaiveDate, NaiveDateTime};
use marginalia_core::app_data::{AppData, QuickNoteRhythm, FontSize, Theme};
use marginalia_core::filters::Adjustments;
use marginalia_core::highlights::{Highlight, HighlightsFile};
use marginalia_core::model::*;
use marginalia_core::rotation::Rotation;
use marginalia_core::vault::*;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn data() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("integration/data")
}

fn current_time() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 11, 12).unwrap().and_hms_opt(14, 32, 0).unwrap()
}

fn generate() -> bool {
    std::env::var_os("GERAR_DOURADO").is_some()
}

/// Compara `real` com o dourado `name` (ou o regrava com `GERAR_DOURADO`).
/// Fins de linha normalizados: o git no Windows pode trocar `\n` por `\r\n`.
fn checks(name: &str, real: &str) {
    let file_path = data().join(name);
    if generate() {
        fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        fs::write(&file_path, real).unwrap();
        return;
    }
    let expected = fs::read_to_string(&file_path).unwrap_or_else(|_| panic!("falta o dourado {name}"));
    assert_eq!(expected.replace("\r\n", "\n"), real.replace("\r\n", "\n"), "{name}");
}

fn tree(root_dir: &Path) -> Vec<(String, String)> {
    fn rec(base: &Path, dir: &Path, output: &mut Vec<(String, String)>) {
        let mut items: Vec<_> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
        items.sort();
        for p in items {
            if p.is_dir() {
                rec(base, &p, output);
            } else {
                let rel = p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                output.push((rel, fs::read_to_string(&p).unwrap()));
            }
        }
    }
    let mut v = Vec::new();
    rec(root_dir, root_dir, &mut v);
    v
}

fn img() -> ImageRef {
    ImageRef { source: "livro".into(), file: "a.jpg".into(), page: 3 }
}

/// Tudo que o vault grava: fonte, nota, tarefa (com e sem nota), registro e grifos.
#[test]
fn vault_writes_the_format_golden() {
    let d = tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    v.create_source("livro", SourceKind::Zip).unwrap();
    v.create_source("album", SourceKind::Folder).unwrap();
    let fr = Framing { zoom: 2.0, cx: 0.5, cy: 0.25 };
    let p = v.create_note("Maria Alves herdeira #familia", img(), fr, Some("g1".into()), current_time()).unwrap();
    v.create_task("achar o testamento", Some(img()), Some(p.id.clone()), current_time()).unwrap();
    v.create_task("pesquisar no cartorio", None, None, current_time()).unwrap();
    let r = v
        .create_record("Inventario 1850", "livro", Position { file: "a.jpg".into(), page: 1 }, current_time())
        .unwrap();
    v.update_record(&r.id, |r| r.end = Some(Position { file: "a.jpg".into(), page: 5 })).unwrap();

    let mut g = HighlightsFile::default();
    g.add(&img(), Highlight::Rectangle { id: "g1".into(), x: 0.1, y: 0.2, w: 0.3, h: 0.4, color: Some("#e53935".into()) }).unwrap();
    g.add(&img(), Highlight::Pointer { id: "g2".into(), x: 0.5, y: 0.5, tail: Some((0.7, 0.8)), color: None }).unwrap();
    g.save(&v, "livro").unwrap();

    for (rel, content) in tree(d.path()) {
        checks(&format!("golden_vault/{rel}"), &content);
    }
}

/// Os arquivos dourados voltam inteiros pelas APIs de leitura.
#[test]
fn vault_le_the_format_golden() {
    read_vault("golden_vault");
}

/// O formato antigo (chaves em portugues) continua legivel e e regravado no novo.
#[test]
fn vault_le_the_format_legacy() {
    read_vault("golden_vault_legacy");
}

fn read_vault(folder: &str) {
    if generate() {
        return;
    }
    let d = tempdir().unwrap();
    for (rel, content) in tree(&data().join(folder)) {
        let destination = d.path().join(&rel);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::write(destination, content).unwrap();
    }
    let v = Vault::open_or_create(d.path()).unwrap();
    let notes = v.list_notes().unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].0.image, img());
    assert_eq!(notes[0].0.highlight.as_deref(), Some("g1"));
    assert_eq!(notes[0].1, "Maria Alves herdeira #familia");
    let tasks = v.list_tasks().unwrap();
    assert_eq!(tasks.len(), 2);
    assert!(tasks.iter().any(|(t, _)| t.note.as_deref() == Some(notes[0].0.id.as_str())));
    assert!(tasks.iter().all(|(t, _)| t.state == StateTask::Open));
    let records = v.list_records().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].0.end, Some(Position { file: "a.jpg".into(), page: 5 }));
    let sources = v.list_sources().unwrap();
    assert_eq!(sources.iter().map(|f| (f.name.as_str(), f.kind)).collect::<Vec<_>>(), [("album", SourceKind::Folder), ("livro", SourceKind::Zip)]);
    let g = HighlightsFile::load(&v, "livro").unwrap();
    assert_eq!(g.for_image(&img()).len(), 2);
    // regravar da o mesmo arquivo
    g.save(&v, "livro").unwrap();
    let of_new = fs::read_to_string(d.path().join("grifos/livro.json")).unwrap();
    checks("golden_vault/grifos/livro.json", &of_new);
}

/// `app_data.json` com todo campo preenchido.
#[test]
fn app_data_writes_and_le_the_format_golden() {
    let d = tempdir().unwrap();
    let base = d.path().join("app");
    let mut a = AppData::new(base.clone());
    let vault = Path::new("/vault");
    a.set_source_path(vault, "livro", PathBuf::from("/nas/livro.zip"));
    a.set_position(vault, "livro", "a.jpg", 3);
    a.mark_recent(vault, "livro");
    a.set_adjustments(&img(), Adjustments { brightness: 0.2, contrast: 1.5, invert: true, grayscale: true, sharpen: 0.5, highpass: 0.25, threshold: Some(128) });
    a.set_rotation(&img(), Rotation::R90);
    a.set_tools_hidden(true);
    a.set_width_drawers(Some(320.0));
    a.set_reopen_last_source(true);
    a.set_rhythm_quick_note(QuickNoteRhythm::Calm);
    a.set_font_quick_note(FontSize::Large);
    a.set_theme(Theme::Sepia);
    a.set_color_highlight("#e53935").unwrap();
    a.set_last_vault(PathBuf::from("/vault"));
    a.save().unwrap();
    // no Windows a chave do vault grava com "\\" (o caminho "/vault" e normalizado)
    let text = fs::read_to_string(base.join("appdata.json")).unwrap().replace("\"\\\\vault\"", "\"/vault\"");
    checks("appdata_dourado.json", &text);

    if generate() {
        return;
    }
    // le o dourado de volta
    read_appdata("appdata_dourado.json");
    read_appdata("appdata_legacy.json");
}

fn read_appdata(file: &str) {
    let vault = Path::new("/vault");
    let d2 = tempdir().unwrap();
    let mut golden = fs::read_to_string(data().join(file)).unwrap();
    if cfg!(windows) {
        golden = golden.replace("\"/vault\"", "\"\\\\vault\"");
    }
    fs::write(d2.path().join("appdata.json"), golden).unwrap();
    let b = AppData::load(d2.path().to_path_buf()).unwrap();
    assert_eq!(b.theme(), Theme::Sepia);
    assert_eq!(b.quick_note_rhythm(), QuickNoteRhythm::Calm);
    assert_eq!(b.quick_note_font(), FontSize::Large);
    assert_eq!(b.rotation(&img()), Rotation::R90);
    assert_eq!(b.highlight_color(), "#e53935");
    assert_eq!(b.drawers_width(), Some(320.0));
    assert_eq!(b.position(vault, "livro"), Some(("a.jpg".to_string(), 3)));
    assert_eq!(b.adjustments(&img()).threshold, Some(128));
}
