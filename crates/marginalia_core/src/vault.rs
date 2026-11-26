use texts::t;
use crate::frontmatter::{body_start, parse, render};
use crate::ids::{created_at, make_id};
use crate::model::*;
use crate::Error;
use chrono::NaiveDateTime;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, Error>;

const DIRS: [&str; 4] = ["fontes", "notas", "tarefas", "grifos"];

#[derive(Debug, Clone, PartialEq)]
pub enum Found {
    Note(Note, String),
    Task(Task, String),
}

/// Pasta de um documento, para as operacoes que precisam saber o tipo (um
/// vault antigo pode ter o mesmo id em mais de uma pasta).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocKind {
    Note,
    Task,
}

impl DocKind {
    fn dir(self) -> &'static str {
        match self {
            DocKind::Note => "notas",
            DocKind::Task => "tarefas",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuickNoteInput {
    Note(String),
    Task(String),
}

/// Texto que começa com `[]` vira tarefa; o resto vira nota.
pub fn route_quick_note_input(text: &str) -> QuickNoteInput {
    let t = text.trim();
    match t.strip_prefix("[]") {
        Some(rest) => QuickNoteInput::Task(rest.trim().to_string()),
        None => QuickNoteInput::Note(t.to_string()),
    }
}

/// Nome de fonte seguro como nome de arquivo (sem separadores de caminho nem `:`).
pub(crate) fn safe_name(name: &str) -> Result<String> {
    let s: String = name
        .trim()
        .chars()
        .map(|c| if matches!(c, '/' | '\\' | ':' | '\0') || c.is_control() { '-' } else { c })
        .collect();
    let s = s.trim().trim_start_matches('.').to_string();
    if s.is_empty() {
        return Err(Error::InvalidName(name.to_string()));
    }
    Ok(s)
}

/// Le e interpreta um documento; o erro cita o arquivo.
fn read_doc<T: DeserializeOwned>(path: &Path) -> Result<(T, String)> {
    let text = fs::read_to_string(path).map_err(|e| Error::Document(path.to_path_buf(), Box::new(e.into())))?;
    parse(&text).map_err(|e| Error::Document(path.to_path_buf(), Box::new(e)))
}

/// Regrava um documento existente sem truncar no lugar (ver `write_atomic`).
fn write_doc<T: Serialize>(path: &Path, meta: &T, body: &str) -> Result<()> {
    write_atomic(path, &render(meta, body))
}

/// Cria `path` so se ele ainda nao existir (`O_EXCL`): duas instancias (ou duas
/// maquinas no mesmo NAS) nunca sobrescrevem o documento uma da outra. Falha na
/// escrita apaga o arquivo criado.
fn create_new(path: &Path, content: &str) -> std::io::Result<()> {
    let mut f = fs::OpenOptions::new().write(true).create_new(true).open(path)?;
    if let Err(e) = f.write_all(content.as_bytes()) {
        drop(f);
        let _ = fs::remove_file(path);
        return Err(e);
    }
    Ok(())
}

/// `.<name>.<pid>.<n>.tmp` ao lado de `path` (oculto e nao `.md`): unico por
/// processo e por gravacao, entao duas gravacoes simultaneas (threads ou
/// instancias no mesmo NAS) nunca dividem o temporario.
fn temporary_name(path: &Path) -> PathBuf {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    path.with_file_name(format!(".{name}.{}.{n}.tmp", std::process::id()))
}

/// Grava `content` num temporario ao lado (ver `temporary_name`) e
/// troca pelo rename: quem le nunca ve o arquivo pela metade.
pub(crate) fn write_atomic(path: &Path, content: &str) -> Result<()> {
    let tmp = temporary_name(path);
    if let Err(e) = fs::write(&tmp, content).and_then(|()| fs::rename(&tmp, path)) {
        let _ = fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

/// `sources/<name>.md` durante uma renomeacao (a marca permite retomar).
#[derive(serde::Serialize, serde::Deserialize)]
struct RenamingSource {
    #[serde(rename = "name", alias = "nome")]
    name: String,
    #[serde(rename = "kind", alias = "tipo")]
    kind: SourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "renaming_from", alias = "renomeando_de")]
    renaming_from: Option<String>,
    #[serde(flatten, default, skip_serializing_if = "Extras::is_empty")]
    extra: Extras,
}

impl Fields for RenamingSource {
    const FIELDS: &'static [&'static str] = &["name", "kind", "renaming_from"];
}

/// `*.md` de um documento do vault: ignora ocultos (`._x.md` do AppleDouble,
/// `.x.md.tmp` de uma gravacao em curso).
pub fn eh_document(p: &Path) -> bool {
    p.extension().is_some_and(|x| x == "md") && !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'))
}

fn md_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut v = Vec::new();
    for e in fs::read_dir(dir).map_err(|e| Error::Document(dir.to_path_buf(), Box::new(e.into())))? {
        let p = e?.path();
        if eh_document(&p) {
            v.push(p);
        }
    }
    v.sort();
    Ok(v)
}

#[derive(Debug, Clone)]
pub struct Vault {
    pub root: PathBuf,
}

impl Vault {
    /// Vault do formato antigo: tem `pistas/` e ainda nao tem `notes/`.
    pub fn old_format(root: &Path) -> bool {
        root.join("pistas").is_dir() && !root.join("notas").is_dir()
    }

    pub fn open_or_create(root: &Path) -> Result<Vault> {
        if Self::old_format(root) {
            return Err(Error::VaultFormatOld(root.to_path_buf()));
        }
        for d in DIRS {
            fs::create_dir_all(root.join(d))?;
        }
        Ok(Vault { root: root.to_path_buf() })
    }

    fn dir(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    /// Id livre em `notes/` e `tasks/` ao mesmo tempo: o id de um documento
    /// novo nunca repete o de outra pasta (`update_text`/`finish_id` acham
    /// o documento so pelo id). `skip`: ids que outra instancia criou depois da
    /// checagem (ver `create`). `free`: id que conta como livre (o proprio
    /// documento que `finish_id` esta fixando).
    fn id_free(&self, text: &str, now: NaiveDateTime, skip: &HashSet<String>, free: Option<&str>) -> String {
        let dirs = ["notas", "tarefas"].map(|d| self.dir(d));
        make_id(now, text, &|id| {
            let f = format!("{id}.md");
            Some(id) != free && (skip.contains(id) || dirs.iter().any(|d| d.join(&f).exists()))
        })
    }

    /// Cria `<dir>/<id>.md` com um id livre. Se o arquivo aparecer entre a
    /// escolha do id e a criacao, tenta o proximo sufixo. `before` roda logo
    /// antes de criar (gancho de teste para simular a corrida).
    fn create<T: Serialize>(
        &self,
        dir: &str,
        text: &str,
        now: NaiveDateTime,
        mk: impl Fn(String) -> T,
        before: &dyn Fn(&Path),
    ) -> Result<T> {
        self.create_with_id(dir, text, &|skip| self.id_free(text, now, skip, None), mk, before)
    }

    /// `create` com a escolha do id separada do corpo (`choose` recebe os ids
    /// que ja apareceram no disco nesta tentativa).
    fn create_with_id<T: Serialize>(
        &self,
        dir: &str,
        body: &str,
        choose: &dyn Fn(&HashSet<String>) -> String,
        mk: impl Fn(String) -> T,
        before: &dyn Fn(&Path),
    ) -> Result<T> {
        let mut skip = HashSet::new();
        loop {
            let id = choose(&skip);
            let path = self.dir(dir).join(format!("{id}.md"));
            let doc = mk(id.clone());
            before(&path);
            match create_new(&path, &render(&doc, body)) {
                Ok(()) => return Ok(doc),
                Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                    skip.insert(id);
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    pub fn create_source(&self, name: &str, kind: SourceKind) -> Result<Source> {
        let name = safe_name(name)?;
        let path = self.dir("fontes").join(format!("{name}.md"));
        let f = Source { name, kind, extra: Extras::new() };
        match create_new(&path, &render(&f, "")) {
            Ok(()) => Ok(f),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => Err(Error::DuplicateName(f.name)),
            Err(e) => Err(e.into()),
        }
    }

    pub fn create_note(
        &self,
        text: &str,
        image: ImageRef,
        fr: Framing,
        highlight: Option<String>,
        now: NaiveDateTime,
    ) -> Result<Note> {
        self.create_note_with(text, image, fr, highlight, now, &|_| {})
    }

    fn create_note_with(
        &self,
        text: &str,
        image: ImageRef,
        fr: Framing,
        highlight: Option<String>,
        now: NaiveDateTime,
        before: &dyn Fn(&Path),
    ) -> Result<Note> {
        let mk = |id| Note {
            id,
            image: image.clone(),
            framing: fr.clone(),
            highlight: highlight.clone(),
            record: None,
            extra: Extras::new(),
        };
        self.create("notas", text, now, mk, before)
    }

    pub fn create_task(
        &self,
        text: &str,
        image: Option<ImageRef>,
        note: Option<String>,
        now: NaiveDateTime,
    ) -> Result<Task> {
        self.create_task_with(text, image, note, now, &|_| {})
    }

    fn create_task_with(
        &self,
        text: &str,
        image: Option<ImageRef>,
        note: Option<String>,
        now: NaiveDateTime,
        before: &dyn Fn(&Path),
    ) -> Result<Task> {
        let mk = |id| Task {
            id,
            state: StateTask::Open,
            image: image.clone(),
            note: note.clone(),
            record: None,
            extra: Extras::new(),
        };
        self.create("tarefas", text, now, mk, before)
    }

    fn list<T: DeserializeOwned>(&self, dir: &str) -> Result<Vec<(T, String)>> {
        md_files(&self.dir(dir))?.iter().map(|p| read_doc(p)).collect()
    }

    pub fn list_notes(&self) -> Result<Vec<(Note, String)>> {
        self.list("notas")
    }

    pub fn list_tasks(&self) -> Result<Vec<(Task, String)>> {
        self.list("tarefas")
    }

    /// Registros do vault, por id. Sem a pasta `records/` (vault antigo,
    /// ou nenhum registro ainda): vazio. Um arquivo ilegivel e erro, como nas notas.
    pub fn list_records(&self) -> Result<Vec<(Record, String)>> {
        if !self.dir("registros").is_dir() {
            return Ok(Vec::new());
        }
        self.list("registros")
    }

    /// Como `list_records`, mas pula o arquivo ilegivel (um registro quebrado
    /// a mao nao pode travar marcar, mover ou o resto do vault).
    pub fn list_records_readable(&self) -> Result<Vec<(Record, String)>> {
        if !self.dir("registros").is_dir() {
            return Ok(Vec::new());
        }
        Ok(md_files(&self.dir("registros"))?.iter().filter_map(|p| read_doc(p).ok()).collect())
    }

    /// Cria `records/<id>.md` (a pasta nasce aqui) com o inicio e sem fim.
    /// Id como o das notas (`AAAAMMDD-HHMM-<slug do nome>`), com sufixo na
    /// colisao contra `records/`. Nome vazio: `InvalidName`.
    pub fn create_record(&self, name: &str, source: &str, start: Position, now: NaiveDateTime) -> Result<Record> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::InvalidName(name.to_string()));
        }
        fs::create_dir_all(self.dir("registros"))?;
        let dir = self.dir("registros");
        let choose = |skip: &HashSet<String>| {
            make_id(now, name, &|id| skip.contains(id) || dir.join(format!("{id}.md")).exists())
        };
        let mk = |id| Record {
            id,
            source: source.to_string(),
            name: name.to_string(),
            start: start.clone(),
            end: None,
            extra: Extras::new(),
        };
        self.create_with_id("registros", "", &choose, mk, &|_| {})
    }

    fn path_record(&self, id: &str) -> Result<PathBuf> {
        if id.is_empty() || id.contains(['/', '\\']) {
            return Err(Error::NotFound(id.to_string()));
        }
        let p = self.dir("registros").join(format!("{id}.md"));
        if p.is_file() {
            Ok(p)
        } else {
            Err(Error::NotFound(id.to_string()))
        }
    }

    /// Aplica `f` ao registro `id` e regrava (atomico), preservando o corpo e
    /// os campos que o Marginalia nao conhece.
    pub fn update_record(&self, id: &str, f: impl FnOnce(&mut Record)) -> Result<Record> {
        let path = self.path_record(id)?;
        let (mut r, body): (Record, String) = read_doc(&path)?;
        f(&mut r);
        write_doc(&path, &r, &body)?;
        Ok(r)
    }

    /// Apaga so o arquivo do registro; notas e tarefas nao sao tocadas.
    pub fn delete_record(&self, id: &str) -> Result<()> {
        let path = self.path_record(id)?;
        fs::remove_file(path)?;
        Ok(())
    }

    pub fn list_sources(&self) -> Result<Vec<Source>> {
        Ok(self.list::<Source>("fontes")?.into_iter().map(|(f, _)| f).collect())
    }

    /// Procura em `notes/` e `tasks/` pelo arquivo `<id>.md`.
    pub fn find_by_id(&self, id: &str) -> Result<Option<Found>> {
        if id.is_empty() || id.contains(['/', '\\']) {
            return Ok(None);
        }
        let file = format!("{id}.md");
        let p = self.dir("notas").join(&file);
        if p.is_file() {
            let (m, b) = read_doc(&p)?;
            return Ok(Some(Found::Note(m, b)));
        }
        let p = self.dir("tarefas").join(&file);
        if p.is_file() {
            let (m, b) = read_doc(&p)?;
            return Ok(Some(Found::Task(m, b)));
        }
        Ok(None)
    }

    /// Marca a tarefa `id` como `state` (feita ou aberta de novo), preservando
    /// o corpo e os outros campos. Mesmo estado: regrava igual (no-op). Com
    /// uma finalizacao parada no meio (ver `no_renaming_in`) recusa.
    pub fn set_state_task(&self, id: &str, state: StateTask) -> Result<Task> {
        if id.is_empty() || id.contains(['/', '\\']) {
            return Err(Error::NotFound(id.to_string()));
        }
        let path = self.dir("tarefas").join(format!("{id}.md"));
        if !path.is_file() {
            return Err(Error::NotFound(id.to_string()));
        }
        self.no_renaming_in(DocKind::Task, id, &path)?;
        let (mut t, body): (Task, String) = read_doc(&path)?;
        if t.state != state {
            t.state = state;
            write_doc(&path, &t, &body)?;
        }
        Ok(t)
    }

    /// Grava (ou, com `None`, remove) o campo `record:` de uma nota ou
    /// tarefa, preservando o corpo e os outros campos. Mesma regra de
    /// `set_state_task` diante de uma finalizacao parada no meio.
    pub fn set_record_of(&self, kind: DocKind, id: &str, record: Option<&str>) -> Result<()> {
        let path = self.file_path(kind, id);
        if !id_safe(id) || !path.is_file() {
            return Err(Error::NotFound(id.to_string()));
        }
        self.no_renaming_in(kind, id, &path)?;
        let new = record.map(str::to_string);
        match kind {
            DocKind::Note => {
                let (mut m, body): (Note, String) = read_doc(&path)?;
                if m.record != new {
                    m.record = new;
                    write_doc(&path, &m, &body)?;
                }
            }
            DocKind::Task => {
                let (mut m, body): (Task, String) = read_doc(&path)?;
                if m.record != new {
                    m.record = new;
                    write_doc(&path, &m, &body)?;
                }
            }
        }
        Ok(())
    }

    /// Renomeia a fonte `old` para `new` e reescreve as notas e tarefas
    /// que a citam. Pode ser retomada: se parar no meio (disco cheio, NAS
    /// caiu), nada se perde e chamar de novo com os mesmos nomes termina. Ordem:
    /// `sources/<new>.md` (com a marca `renaming_from`) e a copia dos grifos
    /// primeiro, depois os documentos (temporario + rename), e so no fim apaga
    /// `sources/<old>.md` e os grifos antigos e tira a marca.
    /// `sources/<new>.md` e a marca de uma renomeacao de `old` que parou no
    /// meio (chamar `rename_fonte(antigo, novo)` de novo termina). Nome invalido,
    /// arquivo ausente, ilegivel ou de outra fonte: `false`.
    pub fn interrupted_renaming(&self, old: &str, new: &str) -> bool {
        if safe_name(new).ok().as_deref() != Some(new) {
            return false;
        }
        let to = self.dir("fontes").join(format!("{new}.md"));
        to.is_file()
            && read_doc::<RenamingSource>(&to).is_ok_and(|(f, _)| f.name == new && f.renaming_from.as_deref() == Some(old))
    }

    pub fn rename_source(&self, old: &str, new: &str) -> Result<()> {
        if safe_name(old)? != old {
            return Err(Error::InvalidName(old.to_string()));
        }
        let new = safe_name(new)?;
        let from = self.dir("fontes").join(format!("{old}.md"));
        let to = self.dir("fontes").join(format!("{new}.md"));
        // `new` ja existe: so vale se for a marca de uma renomeacao de `old` interrompida.
        let resumed = self.interrupted_renaming(old, &new);
        if to.exists() && !resumed {
            return Err(Error::DuplicateName(new));
        }
        let g_from = self.dir("grifos").join(format!("{old}.json"));
        let g_to = self.dir("grifos").join(format!("{new}.json"));
        if !from.is_file() {
            if resumed {
                // Faltava tirar os grifos antigos e a marca. Os antigos podem ter
                // grifos novos (gravados enquanto a renomeacao estava parada):
                // sem copia passam para o nome novo; com copia, mesclam nela.
                if g_from.is_file() {
                    if g_to.is_file() {
                        crate::highlights::merge_at(&g_from, &g_to)?;
                    } else {
                        fs::rename(&g_from, &g_to)?;
                    }
                }
                let (f, body): (RenamingSource, String) = read_doc(&to)?;
                let f = Source { name: f.name, kind: f.kind, extra: f.extra };
                return write_atomic(&to, &render(&f, &body));
            }
            return Err(Error::NotFound(old.to_string()));
        }
        let (f, body): (Source, String) = read_doc(&from)?;
        // Sem chaves da marca (uma `renaming_from` que sobrou nao passa adiante).
        let final_ = Source { name: new.clone(), kind: f.kind, extra: to_extras::<RenamingSource>(f.extra) };
        // Valida tudo antes de mutar: qualquer documento ilegivel aborta sem alterar nada.
        let mut writes = Vec::new();
        self.retarget::<Note>("notas", old, &new, |p| Some(&mut p.image), &mut writes)?;
        self.retarget::<Task>("tarefas", old, &new, |t| t.image.as_mut(), &mut writes)?;
        self.retarget_records(old, &new, &mut writes)?;
        let marked = RenamingSource {
            name: new.clone(),
            kind: f.kind,
            renaming_from: Some(old.to_string()),
            extra: final_.extra.clone(),
        };
        write_atomic(&to, &render(&marked, &body))?;
        if g_from.is_file() {
            fs::copy(&g_from, &g_to)?;
        }
        for (path, doc) in &writes {
            write_atomic(path, doc)?;
        }
        fs::remove_file(&from)?;
        if g_from.is_file() {
            fs::remove_file(&g_from)?;
        }
        write_atomic(&to, &render(&final_, &body))
    }

    /// Registros cuja `source` e `old` (em memoria, como `retarget`). Sem a
    /// pasta, nada a fazer.
    fn retarget_records(&self, old: &str, new: &str, writes: &mut Vec<(PathBuf, String)>) -> Result<()> {
        if !self.dir("registros").is_dir() {
            return Ok(());
        }
        for path in md_files(&self.dir("registros"))? {
            let text = fs::read_to_string(&path).map_err(|e| Error::Document(path.clone(), Box::new(e.into())))?;
            let (mut r, body) = match parse::<Record>(&text) {
                Ok(x) => x,
                Err(Error::NoFrontmatter) => continue,
                Err(e) => match parse::<serde_yaml::Value>(&text) {
                    Ok((v, _)) if v["source"].as_str().or(v["fonte"].as_str()) != Some(old) => continue,
                    _ => return Err(Error::Document(path, Box::new(e))),
                },
            };
            if r.source == old {
                r.source = new.to_string();
                writes.push((path, render(&r, &body)));
            }
        }
        Ok(())
    }

    /// Lê e reescreve (em memória) os documentos de `dir` que citam `old`.
    fn retarget<T: Serialize + DeserializeOwned>(
        &self,
        dir: &str,
        old: &str,
        new: &str,
        img: impl Fn(&mut T) -> Option<&mut ImageRef>,
        writes: &mut Vec<(PathBuf, String)>,
    ) -> Result<()> {
        for path in md_files(&self.dir(dir))? {
            let text = fs::read_to_string(&path).map_err(|e| Error::Document(path.clone(), Box::new(e.into())))?;
            let (mut m, body) = match parse::<T>(&text) {
                Ok(x) => x,
                // Sem frontmatter (nota escrita a mao): nao cita fonte nenhuma.
                Err(Error::NoFrontmatter) => continue,
                // Frontmatter que nao vira `T`: so pula se der para ver que nao
                // cita `old`; senao recusa (pular deixaria a citacao velha).
                Err(e) => match parse::<serde_yaml::Value>(&text) {
                    Ok((v, _)) if v["image"]["source"].as_str().or(v["imagem"]["fonte"].as_str()).or(v["image"]["fonte"].as_str()).or(v["imagem"]["source"].as_str()) != Some(old) => continue,
                    _ => return Err(Error::Document(path, Box::new(e))),
                },
            };
            if let Some(i) = img(&mut m) {
                if i.source == old {
                    i.source = new.to_string();
                    writes.push((path, render(&m, &body)));
                }
            }
        }
        Ok(())
    }


    // ---- v1.1: rascunho editavel, id fixado uma vez, exclusao ----

    /// Nota rascunho: id provisorio `AAAAMMDD-HHMM-no-text` (sufixo `-2`,
    /// `-3` na colisao, contra `notes/` e `tasks/`), seja qual for
    /// `text` (que vira o corpo), e a marca `rascunho: true` no frontmatter.
    /// `finish_id` troca o id pelo slug do corpo e tira a marca.
    pub fn create_draft(
        &self,
        text: &str,
        image: ImageRef,
        fr: Framing,
        highlight: Option<String>,
        now: NaiveDateTime,
    ) -> Result<Note> {
        let mk = |id| Note {
            id,
            image: image.clone(),
            framing: fr.clone(),
            highlight: highlight.clone(),
            record: None,
            extra: draft_mark(),
        };
        self.create_with_id("notas", text, &|skip| self.id_free("", now, skip, None), mk, &|_| {})
    }

    /// Tarefa rascunho (ver `create_draft`).
    pub fn create_task_draft(
        &self,
        text: &str,
        image: Option<ImageRef>,
        note: Option<String>,
        now: NaiveDateTime,
    ) -> Result<Task> {
        let mk = |id| Task { id, state: StateTask::Open, image: image.clone(), note: note.clone(), record: None, extra: draft_mark() };
        self.create_with_id("tarefas", text, &|skip| self.id_free("", now, skip, None), mk, &|_| {})
    }

    /// Tipo de cada pasta (na ordem de `find_by_id`) onde `id` existe.
    fn where_(&self, id: &str) -> Vec<DocKind> {
        if !id_safe(id) {
            return Vec::new();
        }
        [DocKind::Note, DocKind::Task].into_iter().filter(|t| self.file_path(*t, id).is_file()).collect()
    }

    fn file_path(&self, kind: DocKind, id: &str) -> PathBuf {
        self.dir(kind.dir()).join(format!("{id}.md"))
    }

    /// Troca so o corpo do documento `id` (nota ou tarefa), achado so
    /// pelo id. Mesmo id em mais de uma pasta (vault antigo) -> `StateInvalid`
    /// nomeando as pastas: use `update_text_in`. Ver `update_text_in`.
    pub fn update_text(&self, id: &str, text: &str) -> Result<()> {
        match self.where_(id).as_slice() {
            [] => Err(Error::NotFound(id.to_string())),
            [t] => self.update_text_in(*t, id, text),
            v => Err(ambiguous(id, v)),
        }
    }

    /// Troca so o corpo de `<pasta de tipo>/<id>.md`: o frontmatter fica byte a
    /// byte como estava (campos, `extra`, ordem, comentarios). Gravacao
    /// atomica; texto igual nao regrava. Id ausente -> `NotFound`; sem
    /// frontmatter -> erro citando o arquivo. Recusa (`StateInvalid`) editar a
    /// continuacao de uma finalizacao parada no meio enquanto o antigo existe:
    /// a retomada regrava a continuacao a partir do antigo e o texto se perderia.
    pub fn update_text_in(&self, kind: DocKind, id: &str, text: &str) -> Result<()> {
        let path = self.file_path(kind, id);
        if !id_safe(id) || !path.is_file() {
            return Err(Error::NotFound(id.to_string()));
        }
        let doc = fs::read_to_string(&path).map_err(|e| Error::Document(path.clone(), Box::new(e.into())))?;
        let ini = body_start(&doc).ok_or_else(|| Error::Document(path.clone(), Box::new(Error::NoFrontmatter)))?;
        // O frontmatter precisa ser legivel: regravar nao conserta nem estraga ele.
        let (meta, _): (serde_yaml::Mapping, String) = parse(&doc).map_err(|e| Error::Document(path.clone(), Box::new(e)))?;
        self.no_renaming_pending(kind, id, &meta)?;
        if &doc[ini..] == text {
            return Ok(());
        }
        write_atomic(&path, &format!("{}{text}", &doc[..ini]))
    }

    /// `id` e a continuacao (marca `renomeando_de: A`) de uma finalizacao que
    /// parou com `A` ainda no disco: `StateInvalid` apontando `A`.
    fn no_renaming_pending(&self, kind: DocKind, id: &str, meta: &serde_yaml::Mapping) -> Result<()> {
        match meta.get(MARK).and_then(|v| v.as_str()) {
            Some(a) if a != id && id_safe(a) && self.file_path(kind, a).is_file() => Err(renaming_in_progress(a)),
            _ => Ok(()),
        }
    }

    /// Fixa o id do documento `id` (nota ou tarefa rascunho), achado so pelo
    /// id. Mesmo id em `notes/` e `tasks/` ->
    /// `StateInvalid` nomeando as pastas: use `finish_id_in`.
    pub fn finish_id(&self, id: &str, now: NaiveDateTime) -> Result<String> {
        let where_ = self.where_(id);
        match where_.as_slice() {
            [t] => self.finish_id_in(*t, id, now),
            [_, _] => Err(ambiguous(id, &where_)),
            _ => {
                // Nada em notas/ nem tarefas/: o antigo pode ja ter saido de uma
                // finalizacao parada no meio (falta tirar a marca).
                for t in [DocKind::Note, DocKind::Task] {
                    if self.continuation(t, id)?.is_some() {
                        return self.finish_id_in(t, id, now);
                    }
                }
                Err(Error::NotFound(id.to_string()))
            }
        }
    }

    /// Fixa o id de `<pasta de tipo>/<id>.md` se ele for rascunho (marca
    /// `rascunho: true`, gravada so por `create_draft` e
    /// `create_task_draft`): `AAAAMMDD-HHMM-<slug do corpo atual>`
    /// (sufixo na colisao contra `notes/` e `tasks/`), renomeia o
    /// arquivo, tira a marca e reaponta `Task.note` que citava o id antigo. Devolve o id novo.
    ///
    /// - O estado provisorio vem do documento, nao do id: uma nota do usuario
    ///   cujo slug real e `no-text` nunca e renomeada.
    /// - O minuto do id e o da criacao (o do id provisorio), nao `now`: o id diz
    ///   quando a nota foi tomada e uma retomada horas depois chega ao mesmo
    ///   prefixo. `now` so entra se o id do rascunho nao comecar por
    ///   `AAAAMMDD-HHMM-` (editado a mao).
    /// - Idempotente: documento que nao e rascunho (ja fixado, do usuario)
    ///   devolve o proprio id sem tocar em nada. Corpo sem nenhuma letra
    ///   ou digito mantem o id e a marca (continua rascunho). Um corpo cujo
    ///   slug da o proprio id (ex.: "sem texto 2" em `...-no-text-2`) fixa no
    ///   lugar: so tira a marca.
    /// - Retomavel: grava primeiro o arquivo novo (`create_new`) com a marca
    ///   `renomeando_de: <antigo>`, reaponta as referencias, apaga o antigo e so
    ///   entao tira a marca. Parou no meio: chamar de novo com o id antigo acha a
    ///   continuacao pela marca e termina. O conteudo final vem sempre do
    ///   arquivo antigo enquanto ele existir (o app pode ter salvo mais texto
    ///   nele depois da tentativa interrompida): o texto mais novo vence. Por
    ///   isso `update_text` recusa a continuacao enquanto o antigo existe, e
    ///   `delete_*` e `set_state_task` recusam o antigo e a continuacao.
    /// - Chamadas sobre o mesmo id devem ser serializadas pelo chamador (como
    ///   todas as gravacoes do vault); a retomada cobre o que acontece entre tentativas.
    pub fn finish_id_in(&self, kind: DocKind, id: &str, now: NaiveDateTime) -> Result<String> {
        let path = self.file_path(kind, id);
        let exists = id_safe(id) && path.is_file();
        match kind {
            DocKind::Note if exists => self.finish_at::<Note>(kind, id, now),
            DocKind::Task if exists => self.finish_at::<Task>(kind, id, now),
            DocKind::Note if id_safe(id) && self.continuation(kind, id)?.is_some() => {
                self.conclude::<Note>(kind, id, &self.continuation(kind, id)?.expect("acabou de achar"))
            }
            DocKind::Task if id_safe(id) && self.continuation(kind, id)?.is_some() => {
                self.conclude::<Task>(kind, id, &self.continuation(kind, id)?.expect("acabou de achar"))
            }
            _ => Err(Error::NotFound(id.to_string())),
        }
    }

    fn finish_at<T: Renamable>(&self, kind: DocKind, old: &str, now: NaiveDateTime) -> Result<String> {
        let from = self.file_path(kind, old);
        let (mut meta, body): (T, String) = read_doc(&from)?;
        let continuation = self.continuation(kind, old)?;
        if !eh_draft(meta.extra_mut()) && continuation.is_none() {
            return Ok(old.to_string());
        }
        let extra = meta.extra_mut();
        extra.shift_remove(MARK);
        extra.shift_remove(DRAFT);
        let new = match continuation {
            Some(new) => new,
            None => {
                if !text_has(&body) {
                    return Ok(old.to_string());
                }
                // Valida as referencias antes de criar qualquer arquivo.
                self.citations(kind, old)?;
                let created = created_at(old).unwrap_or(now);
                let free = self.id_free(&body, created, &HashSet::new(), Some(old));
                if free == old {
                    // O slug do corpo e o proprio id: fixa no lugar.
                    write_doc(&from, &meta, &body)?;
                    return Ok(old.to_string());
                }
                let mk = |id: String| {
                    let mut m = meta.clone();
                    *m.id_mut() = id;
                    m.extra_mut().insert(MARK.into(), old.into());
                    m
                };
                let choose = |skip: &HashSet<String>| self.id_free(&body, created, skip, None);
                let m = self.create_with_id(kind.dir(), &body, &choose, mk, &|_| {})?;
                m.id().to_string()
            }
        };
        self.repoint(kind, old, &new)?;
        let to = self.file_path(kind, &new);
        // O antigo (o mais novo) e o conteudo final; a marca fica ate ele sair.
        *meta.id_mut() = new.clone();
        meta.extra_mut().insert(MARK.into(), old.into());
        let marked = render(&meta, &body);
        if fs::read_to_string(&to).ok().as_deref() != Some(marked.as_str()) {
            write_atomic(&to, &marked)?;
        }
        fs::remove_file(&from)?;
        meta.extra_mut().shift_remove(MARK);
        write_atomic(&to, &render(&meta, &body))?;
        Ok(new)
    }

    /// Antigo ja apagado: reaponta (de novo, barato e idempotente) e tira as marcas.
    fn conclude<T: Renamable>(&self, kind: DocKind, old: &str, new: &str) -> Result<String> {
        self.repoint(kind, old, new)?;
        let to = self.file_path(kind, new);
        let (mut meta, body): (T, String) = read_doc(&to)?;
        meta.extra_mut().shift_remove(MARK);
        meta.extra_mut().shift_remove(DRAFT);
        write_atomic(&to, &render(&meta, &body))?;
        Ok(new.to_string())
    }

    /// Documento da pasta de `kind` com a marca `renomeando_de: <antigo>` (a
    /// continuacao de uma finalizacao interrompida). Com o id comecando por
    /// `AAAAMMDD-HHMM-` so olha os de mesmo prefixo (o id novo guarda o
    /// minuto). Ilegivel nao e continuacao (o antigo segue intacto, nada se
    /// perde); erro de leitura do disco aborta.
    fn continuation(&self, kind: DocKind, old: &str) -> Result<Option<String>> {
        let prefix = created_at(old).map(|_| &old[..14]).unwrap_or("");
        for path in md_files(&self.dir(kind.dir()))? {
            let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else { continue };
            if stem == old || !stem.starts_with(prefix) {
                continue;
            }
            let text = fs::read_to_string(&path).map_err(|e| Error::Document(path.clone(), Box::new(e.into())))?;
            if let Ok((m, _)) = parse::<serde_yaml::Mapping>(&text) {
                if m.get(MARK).and_then(|v| v.as_str()) == Some(old) && m.get("id").and_then(|v| v.as_str()) == Some(stem.as_str()) {
                    return Ok(Some(stem));
                }
            }
        }
        Ok(None)
    }

    /// Tarefas (`note`) que citam a nota `old`. Le e valida tudo: um
    /// documento ilegivel que cita `old` aborta (pular deixaria a citacao
    /// pendurada). So notas sao citadas por id.
    fn citations(&self, kind: DocKind, old: &str) -> Result<Vec<Citation<Task>>> {
        if kind != DocKind::Note {
            return Ok(Vec::new());
        }
        self.cite::<Task>("tarefas", "note", old, |t| t.note.as_ref())
    }

    fn cite<T: DeserializeOwned>(
        &self,
        dir: &str,
        key: &str,
        old: &str,
        field: impl Fn(&T) -> Option<&String>,
    ) -> Result<Vec<Citation<T>>> {
        let mut v = Vec::new();
        for path in md_files(&self.dir(dir))? {
            let text = fs::read_to_string(&path).map_err(|e| Error::Document(path.clone(), Box::new(e.into())))?;
            match parse::<T>(&text) {
                Ok((m, body)) => {
                    if field(&m).is_some_and(|c| c == old) {
                        v.push((path, m, body));
                    }
                }
                Err(Error::NoFrontmatter) => {}
                Err(e) => match parse::<serde_yaml::Value>(&text) {
                    Ok((y, _)) if y[key].as_str().or(y[legacy_key(key)].as_str()) != Some(old) => {}
                    _ => return Err(Error::Document(path, Box::new(e))),
                },
            }
        }
        Ok(v)
    }

    fn repoint(&self, kind: DocKind, old: &str, new: &str) -> Result<()> {
        for (path, mut t, body) in self.citations(kind, old)? {
            t.note = Some(new.to_string());
            write_doc(&path, &t, &body)?;
        }
        Ok(())
    }

    /// Apaga `<pasta de tipo>/<id>.md` de vez. So aquele arquivo: grifos e quem
    /// cita o id ficam como estao (o app avisa e confirma antes). Com uma
    /// finalizacao parada no meio (de `id`, ou `id` sendo a continuacao com o
    /// antigo ainda no disco) recusa: termine com `finish_id` antes.
    fn delete_at(&self, kind: DocKind, name: &str, id: &str) -> Result<()> {
        let path = self.file_path(kind, id);
        if !id_safe(id) || !path.is_file() {
            return Err(Error::NotFound(format!("{name} {id}")));
        }
        self.no_renaming_in(kind, id, &path)?;
        fs::remove_file(&path).map_err(|e| Error::Document(path, Box::new(e.into())))
    }

    /// Recusa (`renaming_in_progress`) mexer em `path` (`<pasta de tipo>/<id>.md`)
    /// no meio de uma finalizacao parada: `id` e o antigo e tem continuacao, ou
    /// `id` e a continuacao e o antigo ainda existe. A retomada regrava a
    /// continuacao a partir do antigo e reaponta quem cita o antigo, entao
    /// mudar o estado ou excluir qualquer um dos dois
    /// duplicaria ou desfaria a mudanca. Ler e listar continuam livres.
    fn no_renaming_in(&self, kind: DocKind, id: &str, path: &Path) -> Result<()> {
        if self.continuation(kind, id)?.is_some() {
            return Err(renaming_in_progress(id));
        }
        let text = fs::read_to_string(path).map_err(|e| Error::Document(path.to_path_buf(), Box::new(e.into())))?;
        if let Ok((meta, _)) = parse::<serde_yaml::Mapping>(&text) {
            self.no_renaming_pending(kind, id, &meta)?;
        }
        Ok(())
    }

    /// Exclui de vez a nota `id` (irreversivel).
    pub fn delete_note(&self, id: &str) -> Result<()> {
        self.delete_at(DocKind::Note, "nota", id)
    }

    /// Exclui de vez a tarefa `id`.
    pub fn delete_task(&self, id: &str) -> Result<()> {
        self.delete_at(DocKind::Task, "tarefa", id)
    }
}

/// Marca da continuacao de `finish_id` (mesma chave de `rename_source`).
const MARK: &str = "renaming_from";
/// Marca de documento com id provisorio (`create_draft`). Chave reservada:
/// uma nota ou tarefa com `rascunho: true` escrito a mao e tratada como
/// rascunho (`finish_id` a renomeia pelo corpo).
const DRAFT: &str = "draft";

fn draft_mark() -> Extras {
    let mut m = Extras::new();
    m.insert(DRAFT.into(), true.into());
    m
}

fn eh_draft(extra: &Extras) -> bool {
    extra.get(DRAFT).and_then(|v| v.as_bool()) == Some(true)
}

/// O corpo tem alguma letra ou digito (vazio ou so pontuacao: nao fixa o id).
fn text_has(body: &str) -> bool {
    body.chars().any(char::is_alphanumeric)
}

fn renaming_in_progress(old: &str) -> Error {
    Error::StateInvalid(t!("error.vault.renaming_in_progress", old))
}

type Citation<T> = (PathBuf, T, String);

fn id_safe(id: &str) -> bool {
    !id.is_empty() && !id.contains(['/', '\\'])
}

fn ambiguous(id: &str, where_: &[DocKind]) -> Error {
    let dirs: Vec<&str> = where_.iter().map(|t| t.dir()).collect();
    Error::StateInvalid(t!("error.vault.id_ambiguous", id, dirs.join(", ")))
}

/// Documento que `finish_id` renomeia.
trait Renamable: Serialize + DeserializeOwned + Clone {
    fn id(&self) -> &str;
    fn id_mut(&mut self) -> &mut String;
    fn extra_mut(&mut self) -> &mut Extras;
}

impl Renamable for Note {
    fn id(&self) -> &str {
        &self.id
    }
    fn id_mut(&mut self) -> &mut String {
        &mut self.id
    }
    fn extra_mut(&mut self) -> &mut Extras {
        &mut self.extra
    }
}

impl Renamable for Task {
    fn id(&self) -> &str {
        &self.id
    }
    fn id_mut(&mut self) -> &mut String {
        &mut self.id
    }
    fn extra_mut(&mut self) -> &mut Extras {
        &mut self.extra
    }
}


#[cfg(test)]
#[path = "tests/vault.rs"]
mod tests;

/// Nome antigo (portugues) de uma chave de frontmatter citada por `cite`.
fn legacy_key(k: &str) -> &str {
    match k {
        "note" => "nota",
        _ => k,
    }
}
