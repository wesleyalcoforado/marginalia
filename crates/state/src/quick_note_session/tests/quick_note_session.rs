use super::*;
use crate::app_state::test::with_vault;
use crate::drawers::{self, Drawer, Drawers};
use marginalia_core::frontmatter::{parse, render};
use marginalia_core::highlights::Highlight;
use marginalia_core::model::{Note, Task};
use std::path::Path;
use tempfile::tempdir;

pub(super) fn data() -> NaiveDateTime {
    chrono::NaiveDate::from_ymd_opt(2026, 11, 12).unwrap().and_hms_opt(14, 32, 0).unwrap()
}

pub(super) fn session() -> QuickNoteSession {
    QuickNoteSession::with_clock(Arc::new(data))
}

pub(super) fn ms(t0: Instant, n: u64) -> Instant {
    t0 + Duration::from_millis(n)
}

pub(super) fn root_dir(st: &AppState) -> PathBuf {
    st.vault.as_ref().unwrap().root.clone()
}

/// Ids (nomes sem `.md`) em `folder/`, em ordem; ignora temporarios ocultos.
pub(super) fn docs_in(root_dir: &Path, folder: &str) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(root_dir.join(folder))
        .map(|d| {
            d.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.ends_with(".md") && !n.starts_with('.'))
                .map(|n| n.trim_end_matches(".md").to_string())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

pub(super) fn docs(st: &AppState, folder: &str) -> Vec<String> {
    docs_in(&root_dir(st), folder)
}

pub(super) fn read_file(st: &AppState, folder: &str, id: &str) -> String {
    std::fs::read_to_string(root_dir(st).join(folder).join(format!("{id}.md"))).unwrap()
}

pub(super) fn note_in(st: &AppState, id: &str) -> (Note, String) {
    parse(&read_file(st, "notas", id)).unwrap()
}

pub(super) fn ret(id: &str) -> Highlight {
    Highlight::Rectangle { id: id.into(), x: 0.1, y: 0.2, w: 0.3, h: 0.4, color: None }
}

pub(super) const DRAFT: &str = "20261112-1432-sem-texto";

/// Os passos de `drawers::click_line` com o estado real (sem a view):
/// `AppState::open_line_note` (`open_note`) troca o quadro, a
/// sincronizacao (`observe`) percebe e `QuickNoteSession::load` pede o texto.
pub(super) struct ClickReal<'a> {
    pub(super) s: &'a mut QuickNoteSession,
    pub(super) st: &'a mut AppState,
    pub(super) l: drawers::NoteLine,
    pub(super) current_time: Instant,
    pub(super) effect: Effect,
}

impl drawers::LineClick for ClickReal<'_> {
    fn open_image(&mut self) -> Result<bool, String> {
        match self.st.open_line_note(&self.l).map_err(|e| e.to_string())? {
            crate::app_state::OpenNote::ImageFrame(Some(p)) => {
                let _ = self.st.load_now(p);
            }
            crate::app_state::OpenNote::ImageFrame(None) => {}
            crate::app_state::OpenNote::Source { .. } => return Ok(true),
        }
        self.effect.join(self.s.observe(self.st, self.current_time));
        Ok(false)
    }

    fn load(&mut self) {
        let e = self.s.load(self.st, DocKind::Note, &self.l.id, self.current_time);
        self.effect.join(e);
    }

    fn load_on_enter(&mut self) {
        panic!("a nota e da fonte aberta");
    }
}

/// O clique numa linha da gaveta pela mesma `drawers::click_line` da view (R25c).
pub(super) fn line_click(s: &mut QuickNoteSession, st: &mut AppState, id: &str, current_time: Instant) -> Effect {
    let v = st.vault.clone().unwrap();
    let l = drawers::load(&v, &st.app_data).notes.into_iter().find(|p| p.id == id).expect("linha na gaveta");
    let mut c = ClickReal { s: &mut *s, st: &mut *st, l, current_time, effect: Effect::default() };
    drawers::click_line(&mut c).unwrap();
    let e = c.effect;
    s.run(st, e, current_time)
}

/// Escreve `folder/<new>.md` como a continuacao de `old` (o que
/// `finish_id_in` deixa quando para no meio): o documento de `old`
/// com o id novo e a marca `renaming_from`.
pub(super) fn continuation(root_dir: &Path, old: &str, new: &str) {
    let fname = |id: &str| root_dir.join("notas").join(format!("{id}.md"));
    let (mut meta, note_body): (Note, String) = parse(&std::fs::read_to_string(fname(old)).unwrap()).unwrap();
    meta.id = new.into();
    meta.extra.shift_remove("draft");
    meta.extra.insert("renaming_from".into(), old.into());
    std::fs::write(fname(new), render(&meta, &note_body)).unwrap();
}

pub(super) fn rec(st: &AppState) -> PathBuf {
    recovery_file(st.app_data.base())
}

pub(super) fn go_to_next(st: &mut AppState) {
    let p = st.go_next().unwrap();
    let _ = st.load_now(p);
}

pub(super) fn record(st: &AppState, key: &str, id: Option<&str>, text: &str) -> Record {
    Record {
        key: key.into(),
        vault: root_dir(st),
        kind: "note".into(),
        id: id.map(str::to_string),
        text: text.into(),
        image: Some(st.current.image.clone()),
        framing: Some(st.framing().clone()),
        highlight: None,
        lost: false,
        no_disk: Vec::new(),
    }
}

/// Nota "Maria" com id fixado, apagada por fora; a sessao a marca perdida com "Maria Rita".
pub(super) fn lost(st: &mut AppState, s: &mut QuickNoteSession, t0: Instant) -> String {
    let e = s.typed(st, "Maria", t0);
    s.run(st, e, t0);
    let e = s.textbox_exit(st, ms(t0, 10));
    s.run(st, e, ms(t0, 10));
    let id = "20261112-1432-maria";
    std::fs::remove_file(root_dir(st).join(format!("notas/{id}.md"))).unwrap();
    s.typed(st, "Maria Rita", ms(t0, 100));
    let e = s.tick(st, ms(t0, 700));
    s.run(st, e, ms(t0, 700));
    assert!(s.can_recreate());
    id.to_string()
}

/// Restaura um registro sem id ("Maria", `no_disk`) num vault onde o trabalho
/// atrasado deixou `left`; devolve as notas e corpos depois.
pub(super) fn restore_about(left: impl Fn(&Vault, &AppState), no_disk: &[&str]) -> Vec<(String, String)> {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let v = st.vault.clone().unwrap();
    left(&v, &st);
    let mut record = record(&st, "x-1", None, "Maria");
    record.no_disk = no_disk.iter().map(|c| c.to_string()).collect();
    write_recovery(&rec(&st), &[record]).unwrap();
    let mut s = session();
    let t0 = Instant::now();
    let e = s.restore(&mut st, t0);
    s.run(&mut st, e, t0);
    assert!(!rec(&st).exists(), "o texto foi para o vault");
    docs(&st, "notas").into_iter().map(|id| (note_in(&st, &id).1, id)).map(|(c, id)| (id, c)).collect()
}

/// Restaura um registro sem id (`text`, `no_disk`, `highlight`) sobre o vault
/// preparado por `before`; devolve (id, corpo) de cada nota depois.
pub(super) fn restore_record(
    before: impl Fn(&Vault, &mut AppState),
    text: &str,
    no_disk: &[&str],
    highlight: Option<&str>,
) -> Vec<(String, String)> {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path(), "vault");
    let v = st.vault.clone().unwrap();
    before(&v, &mut st);
    let mut record = record(&st, "x-1", None, text);
    record.no_disk = no_disk.iter().map(|c| c.to_string()).collect();
    record.highlight = highlight.map(str::to_string);
    write_recovery(&rec(&st), &[record]).unwrap();
    let mut s = session();
    let t0 = Instant::now();
    let e = s.restore(&mut st, t0);
    s.run(&mut st, e, t0);
    docs(&st, "notas").into_iter().map(|id| (id.clone(), note_in(&st, &id).1)).collect()
}

pub(super) fn bodies(r: &[(String, String)]) -> Vec<&str> {
    let mut v: Vec<&str> = r.iter().map(|(_, c)| c.as_str()).collect();
    v.sort();
    v
}

mod typing;
mod load;
mod swap;
mod empty;
mod delete;
mod recovery;
mod restore;
