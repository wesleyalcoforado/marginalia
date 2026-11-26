//! Gavetas laterais (notas, tarefas, fontes), sem gpui (regra R4): qual
//! gaveta esta aberta, as listas lidas do vault, filtros (notas por fonte;
//! tarefas abertas/feitas), ordem (mais novas primeiro: o id
//! comeca com data e hora), rotulos de linha, selecao pelo teclado.
//!
//! Ler o vault (centenas de `.md`, talvez sincronizados) e conferir o caminho
//! de cada fonte (pode ser o NAS) bloqueia: [`load`] roda no executor de
//! fundo. A view guarda as [`Lists`] em [`Drawers`], mostra a lista guardada
//! enquanto recarrega e so troca quando chega a resposta da recarga mais nova
//! (`request_reload` / `receive`). Depois de cada gravacao a view marca a linha
//! na hora (`mark_task`) e pede uma recarga.

use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use marginalia_core::app_data::{AppData, SourceStatus};
use marginalia_core::frontmatter::parse;
use marginalia_core::model::{Framing, StateTask, Source, ImageRef, Note, Record, Task, SourceKind};
use marginalia_core::vault::{DocKind, Vault};
use texts::t;

use crate::tree::{self, Tree, DestinationRelocate, Input, TreeLine};

/// Largura de uma gaveta aberta (px). A imagem fica com o resto da janela.
pub const WIDTH: f32 = 320.0;
/// Menor e maior largura que o usuario pode dar ao painel das gavetas (px).
pub const WIDTH_MIN: f32 = 220.0;
pub const WIDTH_MAX: f32 = 700.0;
/// Linhas desenhadas por gaveta (o resto fica para os filtros).
pub const MAX_LINES: usize = 300;
/// Caracteres do rotulo de uma linha (antes do `…`).
pub const MAX_LABEL: usize = 80;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drawer {
    Notes,
    Tasks,
    Sources,
}

impl Drawer {
    /// Na ordem dos icones da barra de atividades (e dos atalhos Cmd-1/2/3).
    pub const ALL: [Drawer; 3] = [Drawer::Notes, Drawer::Sources, Drawer::Tasks];

    /// A gaveta aberta depois de clicar no icone de `g` com `current` aberta: `g`
    /// (trocando o que estiver aberto) ou nenhuma, se `g` ja era a aberta.
    pub fn toggled(current: Option<Drawer>, g: Drawer) -> Option<Drawer> {
        if current == Some(g) { None } else { Some(g) }
    }

    /// Cmd-1 notas / Cmd-2 fontes / Cmd-3 tarefas (a ordem dos icones, R25).
    pub fn key_by_the(key: &str) -> Option<Drawer> {
        match key {
            "1" => Some(Drawer::Notes),
            "2" => Some(Drawer::Sources),
            "3" => Some(Drawer::Tasks),
            _ => None,
        }
    }

    pub fn title(self) -> String {
        match self {
            Drawer::Notes => t!("drawer.title.notes"),
            Drawer::Tasks => t!("drawer.title.tasks"),
            Drawer::Sources => t!("drawer.title.sources"),
        }
    }

    /// Dica do icone na barra de atividades.
    pub fn hint(self) -> String {
        match self {
            Drawer::Notes => t!("drawer.hint.notes"),
            Drawer::Sources => t!("drawer.hint.sources"),
            Drawer::Tasks => t!("drawer.hint.tasks"),
        }
    }

    /// A tecla que abre/fecha a gaveta, como se mostra (`⌘1`).
    pub fn shortcut(self) -> String {
        match self {
            Drawer::Notes => t!("drawer.shortcut.notes"),
            Drawer::Sources => t!("drawer.shortcut.sources"),
            Drawer::Tasks => t!("drawer.shortcut.tasks"),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TasksFilter {
    #[default]
    Open,
    Done,
}

impl TasksFilter {
    pub const ALL: [TasksFilter; 2] = [TasksFilter::Open, TasksFilter::Done];

    pub fn label(self) -> String {
        match self {
            TasksFilter::Open => t!("drawer.filter.open_all"),
            TasksFilter::Done => t!("drawer.filter.done_all"),
        }
    }
}

/// Uma nota (de `notes/`). O ponto da imagem vem junto para abrir.
#[derive(Clone, Debug, PartialEq)]
pub struct NoteLine {
    pub id: String,
    /// O rotulo curto da linha (a primeira linha do texto).
    pub text: String,
    /// O texto inteiro, como foi escrito (o card da gaveta mostra as primeiras linhas).
    pub note_body: String,
    /// O texto inteiro em minusculas, para a busca.
    pub body_min: String,
    /// As `#tags` do texto (minusculas, sem o `#`, sem repetir).
    pub tags: Vec<String>,
    pub image: ImageRef,
    pub framing: Framing,
    pub highlight: Option<String>,
    /// O `record:` do frontmatter (override numa imagem de fronteira).
    pub record: Option<String>,
}

impl NoteLine {
    /// `fonte · arquivo · p. N`.
    pub fn local(&self) -> String {
        image_location(&self.image)
    }

    /// `arquivo · p. N`: a gaveta em arvore ja diz a fonte no cabecalho.
    pub fn local_short(&self) -> String {
        short_image_location(&self.image)
    }
}

/// `arquivo · p. N` (sem a fonte).
pub fn short_image_location(i: &ImageRef) -> String {
    let file = i.file.strip_prefix(&format!("{}/", i.source)).unwrap_or(&i.file);
    format!("{} · p. {}", file, i.page)
}

/// `fonte · arquivo · p. N`. O caminho relativo do arquivo numa fonte de pasta ou
/// zip costuma comecar pelo proprio nome da fonte (`Livro/001.jpg` na fonte
/// `Livro`): esse prefixo repetido sai.
pub fn image_location(i: &ImageRef) -> String {
    let file = i.file.strip_prefix(&format!("{}/", i.source)).unwrap_or(&i.file);
    format!("{} · {} · p. {}", i.source, file, i.page)
}

#[derive(Clone, Debug, PartialEq)]
pub struct LineTask {
    pub id: String,
    pub text: String,
    /// O texto inteiro, como foi escrito (o card da gaveta mostra as primeiras linhas).
    pub note_body: String,
    pub done: bool,
    pub image: Option<ImageRef>,
    pub note: Option<String>,
    /// A tarefa cita uma nota que nao existe mais (excluida): a linha mostra
    /// "nota excluida" e segue listavel.
    pub note_deleted: bool,
    /// O `record:` do frontmatter (override numa imagem de fronteira).
    pub record: Option<String>,
}

impl LineTask {
    pub fn local(&self) -> Option<String> {
        self.image.as_ref().map(image_location)
    }

    /// `arquivo · p. N`, sem a fonte (o cabecalho da arvore a diz).
    pub fn local_short(&self) -> Option<String> {
        self.image.as_ref().map(short_image_location)
    }
}

/// Situacao do caminho da fonte nesta maquina (dados do app).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawerSourceStatus {
    Ok,
    NoPath,
    NotFound,
}

impl DrawerSourceStatus {
    pub fn label(self) -> String {
        match self {
            DrawerSourceStatus::Ok => t!("drawer.source_status.ok"),
            DrawerSourceStatus::NoPath => t!("drawer.source_status.no_path"),
            DrawerSourceStatus::NotFound => t!("drawer.source_status.not_found"),
        }
    }
}

/// O que a fonte e, para o icone da gaveta de fontes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawerSourceKind {
    /// Um arquivo de imagem so.
    Image,
    /// Uma pasta de imagens.
    Folder,
    Pdf,
    Zip,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LineSource {
    pub name: String,
    pub kind: SourceKind,
    pub status: DrawerSourceStatus,
    /// O caminho guardado (existindo ou nao).
    pub file_path: Option<PathBuf>,
}

impl LineSource {
    /// Pelo tipo do vault e pela extensao do caminho (sem tocar o disco: o NAS pode estar
    /// lento): zip, pdf ou imagem avulsa; o resto (ou sem caminho) e pasta.
    pub fn kind(&self) -> DrawerSourceKind {
        if self.kind == SourceKind::Zip {
            return DrawerSourceKind::Zip;
        }
        let ext = self.file_path.as_ref().and_then(|c| c.extension()).and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
        match ext.as_deref() {
            Some("pdf") => DrawerSourceKind::Pdf,
            Some("jpg" | "jpeg" | "png" | "tif" | "tiff") => DrawerSourceKind::Image,
            _ => DrawerSourceKind::Folder,
        }
    }
}

/// O que as gavetas mostram, lido de uma vez por [`load`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lists {
    /// Mais novas primeiro.
    pub notes: Vec<NoteLine>,
    /// Mais novas primeiro.
    pub tasks: Vec<LineTask>,
    /// Por nome.
    pub sources: Vec<LineSource>,
    /// Registros do vault, por id.
    pub records: Vec<Record>,
    /// Arquivos que nao puderam ser lidos (`folder/file.md`), pulados.
    pub unreadable: Vec<String>,
    /// Fixacoes de id paradas no meio (documento N com `renomeando_de: A`):
    /// `(tipo, A)`, para a nota rapida terminar com `finalizar_id_em(tipo, A)`
    /// antes de qualquer outra gravacao. Enquanto A existe, N fica fora das
    /// listas (A tem o texto mais novo e e o que aparece).
    pub resume: Vec<(DocKind, String)>,
}

/// Chave da continuacao de `Vault::finish_id_in` (ver `Lists::resume`).
const RENAMING_FROM: &str = "renaming_from";

/// `extra` tem a marca de uma fixacao de id parada no meio: devolve `A`.
fn renaming_from<'a>(extra: &'a marginalia_core::model::Extras, id: &str) -> Option<&'a str> {
    extra.get(RENAMING_FROM).and_then(|v| v.as_str()).filter(|a| !a.is_empty() && *a != id && !a.contains(['/', '\\']))
}

/// Junta `(tipo, A)` em `resume` se o documento `id` for a continuacao de A;
/// devolve se ele deve ficar fora da lista (A ainda existe).
fn continuation(root_dir: &Path, folder: &str, kind: DocKind, id: &str, extra: &marginalia_core::model::Extras, resume: &mut Vec<(DocKind, String)>) -> bool {
    let Some(a) = renaming_from(extra, id) else { return false };
    if !resume.iter().any(|(t, x)| *t == kind && x == a) {
        resume.push((kind, a.to_string()));
    }
    root_dir.join(folder).join(format!("{a}.md")).is_file()
}

/// A nota `id` existe em `notes/`.
fn exists_note(root_dir: &Path, id: &str) -> bool {
    if id.is_empty() || id.contains(['/', '\\']) {
        return false;
    }
    let f = format!("{id}.md");
    root_dir.join("notas").join(&f).is_file()
}

/// Primeira linha nao vazia, sem espacos nas pontas, com no maximo
/// [`MAX_LABEL`] caracteres (mais `…` se cortada).
pub fn label(note_body: &str) -> String {
    let Some(l) = note_body.lines().map(str::trim).find(|l| !l.is_empty()) else {
        return t!("drawer.no_text");
    };
    if l.chars().count() > MAX_LABEL {
        let mut s: String = l.chars().take(MAX_LABEL).collect();
        s.push('…');
        s
    } else {
        l.to_string()
    }
}

/// Le `folder/*.md` do vault; arquivo ilegivel vai para `unreadable` (uma nota
/// quebrada, por exemplo editada a mao, nao esconde as outras).
fn read_file<T>(
    root_dir: &Path,
    folder: &str,
    unreadable: &mut Vec<String>,
    read_doc: fn(&str) -> Result<(T, String), marginalia_core::Error>,
) -> Vec<(T, String)> {
    let Ok(dir) = fs::read_dir(root_dir.join(folder)) else {
        return Vec::new();
    };
    let mut file_paths: Vec<PathBuf> =
        dir.flatten().map(|e| e.path()).filter(|p| marginalia_core::vault::eh_document(p)).collect();
    file_paths.sort();
    let mut out = Vec::new();
    for p in file_paths {
        let text = match fs::read_to_string(&p) {
            Ok(t) => Ok(t),
            // Sumiu entre listar e ler (apagado/movido por fora, link morto): so pula.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => Err(e.to_string()),
        };
        match text.and_then(|s| read_doc(&s).map_err(|e| e.to_string())) {
            Ok(doc) => out.push(doc),
            Err(_) => {
                let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                unreadable.push(format!("{folder}/{name}"));
            }
        }
    }
    out
}

/// Le notas, tarefas e fontes do vault e confere o caminho
/// de cada fonte. BLOQUEIA (disco, talvez NAS): rode no executor de fundo.
pub fn load(vault: &Vault, app_data: &AppData) -> Lists {
    let root_dir = &vault.root;
    let mut unreadable = Vec::new();
    let mut resume = Vec::new();
    let mut notes: Vec<NoteLine> = read_file(root_dir, "notas", &mut unreadable, parse::<Note>)
        .into_iter()
        .filter(|(p, _)| !continuation(root_dir, "notas", DocKind::Note, &p.id, &p.extra, &mut resume))
        .map(|(p, note_body)| NoteLine {
            id: p.id,
            text: label(&note_body),
            body_min: note_body.to_lowercase(),
            note_body: note_body.trim().to_string(),
            tags: marginalia_core::tags::tags(&note_body),
            image: p.image,
            framing: p.framing,
            highlight: p.highlight,
            record: p.record,
        })
        .collect();
    notes.sort_by(|a, b| b.id.cmp(&a.id));
    let mut tasks: Vec<LineTask> = read_file(root_dir, "tarefas", &mut unreadable, parse::<Task>)
        .into_iter()
        .filter(|(t, _)| !continuation(root_dir, "tarefas", DocKind::Task, &t.id, &t.extra, &mut resume))
        .map(|(t, note_body)| LineTask {
            id: t.id,
            text: label(&note_body),
            note_body: note_body.trim().to_string(),
            done: t.state == StateTask::Done,
            image: t.image,
            note_deleted: t.note.as_deref().is_some_and(|p| !exists_note(root_dir, p)),
            note: t.note,
            record: t.record,
        })
        .collect();
    tasks.sort_by(|a, b| b.id.cmp(&a.id));
    let mut sources: Vec<LineSource> = read_file(root_dir, "fontes", &mut unreadable, parse::<Source>)
        .into_iter()
        .map(|(f, _)| {
            let (status, file_path) = match app_data.resolve_source(root_dir, &f.name) {
                SourceStatus::Ok(p) => (DrawerSourceStatus::Ok, Some(p)),
                SourceStatus::NoPath => (DrawerSourceStatus::NoPath, None),
                SourceStatus::NotFound(p) => (DrawerSourceStatus::NotFound, Some(p)),
            };
            LineSource { name: f.name, kind: f.kind, status, file_path }
        })
        .collect();
    sources.sort_by(|a, b| a.name.cmp(&b.name));
    let records: Vec<Record> = read_file(root_dir, "registros", &mut unreadable, parse::<Record>).into_iter().map(|(r, _)| r).collect();
    Lists { notes, tasks, sources, records, unreadable, resume }
}

/// O que uma tecla faz com uma gaveta aberta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawerAction {
    Relocate(i32),
    Open,
    ToggleDone,
    Close,
    /// Backspace: pede a confirmacao para excluir a linha escolhida.
    Delete,
}

/// O que uma tecla faz com o pedido de exclusao aberto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeletionAction {
    Nothing,
    /// Enter: exclua o documento `id` do tipo pedido. O pedido FICA ate a
    /// exclusao dar certo ([`Drawers::remove`]) ou o usuario cancelar: uma
    /// recusa (gravacao em voo, renomeacao em curso) nao fecha a confirmacao.
    Confirm(String, DocKind),
    /// Esc: desistiu.
    Cancel,
}

/// Os botoes do dialogo "Excluir de vez". O foco abre em `Cancel`: Enter so
/// exclui com o foco em `Delete` (Tab ou as setas o levam la).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DeletionButton {
    #[default]
    Cancel,
    Delete,
}

/// Os passos do clique (ou Enter) numa linha de nota ou tarefa, na ordem
/// que [`click_line`] impoe. A view os implementa com o estado real.
pub trait LineClick {
    /// Abre a imagem da linha (`AppState::open_note` / `open_point`): a nota
    /// da caixa, de outra imagem, termina ali. `Ok(true)`: a linha e de outra
    /// fonte, que ainda vai entrar.
    fn open_image(&mut self) -> Result<bool, String>;
    /// Pede o texto da linha para a caixa (`QuickNoteSession::load`).
    fn load(&mut self);
    /// Guarda o pedido do texto para quando a outra fonte entrar.
    fn load_on_enter(&mut self);
}

/// Clique numa linha: primeiro a imagem, depois o texto (R25c, R26). Ao contrario,
/// a troca de imagem que vem depois terminaria a nota recem-carregada e limparia
/// a caixa. Se a imagem nao abre, nada e carregado.
pub fn click_line(c: &mut impl LineClick) -> Result<(), String> {
    if c.open_image()? {
        c.load_on_enter();
    } else {
        c.load();
    }
    Ok(())
}

/// O que a confirmacao de excluir mostra: o documento e o que fica para tras.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestDeletion {
    pub id: String,
    pub kind: DocKind,
    pub text: String,
    /// Tarefas que citam este documento (continuam, apontando para nada).
    pub tasks: usize,
    /// Tem grifo ligado: o grifo permanece na imagem.
    pub highlight: bool,
}

impl RequestDeletion {
    /// O titulo do dialogo.
    pub fn title(&self) -> String {
        match self.kind {
            DocKind::Note => t!("dialog.delete.quick_note_title"),
            DocKind::Task => t!("dialog.delete.task_title"),
        }
    }

    /// O comeco do texto do item (ate 80 caracteres, com reticencias).
    pub fn stretch(&self) -> String {
        let t = self.text.trim();
        if t.chars().count() <= 80 {
            return t.to_string();
        }
        let mut s: String = t.chars().take(80).collect();
        s.push('…');
        s
    }

    /// O aviso do dialogo: o que acontece e o que fica para tras.
    pub fn warning(&self) -> String {
        let mut s = t!("dialog.delete.warning");
        if self.tasks > 0 {
            s.push(' ');
            s.push_str(&t!("dialog.delete.tasks_warning", n = self.tasks));
        }
        if self.highlight {
            s.push(' ');
            s.push_str(&t!("dialog.delete.highlight_warning"));
        }
        s
    }
}

/// Teclas das gavetas. Ficam fora das da imagem (`f i g t r [ ] { } /`, setas
/// para os lados) e da nota rapida.
pub fn drawer_key(g: Drawer, key: &str) -> Option<DrawerAction> {
    use DrawerAction::*;
    let a = match (g, key) {
        (_, "up") => Relocate(-1),
        (_, "down") => Relocate(1),
        (_, "enter") => Open,
        (_, "escape") => Close,
        (Drawer::Notes | Drawer::Tasks, "backspace") => Delete,
        (Drawer::Tasks, "x" | "space") => ToggleDone,
        _ => return None,
    };
    Some(a)
}

/// Posicao e tamanho (fracoes 0..1 da nota da barra) do cursor de uma lista
/// rolavel: `visible` e `total` em px, `offset` e o deslocamento do gpui (0 ou
/// negativo). `None` quando tudo cabe. O cursor tem ao menos 5% da nota.
pub fn scroll_bar(visible: f32, total: f32, offset: f32) -> Option<(f32, f32)> {
    if total <= visible + 0.5 || visible <= 0.0 {
        return None;
    }
    let alt = (visible / total).max(0.05);
    let max = total - visible;
    let top = (-offset / max).clamp(0.0, 1.0) * (1.0 - alt);
    Some((top, alt))
}

/// O `offset` de rolagem (negativo, de `-max` a 0) para o polegar da barra ficar
/// centrado em `y_rel` (distancia do clique ao topo da lista, em px).
pub fn bar_offset(y_rel: f32, visible: f32, total: f32) -> f32 {
    if total <= visible || visible <= 0.0 {
        return 0.0;
    }
    let alt = (visible / total).max(0.05);
    let max = total - visible;
    let frac = (((y_rel / visible) - alt / 2.0) / (1.0 - alt)).clamp(0.0, 1.0);
    -frac * max
}

/// As `n` primeiras linhas nao vazias de `note_body`; `…` no fim da ultima se havia mais.
pub fn lines_first(note_body: &str, n: usize) -> String {
    let mut lines = note_body.lines().map(str::trim_end).filter(|l| !l.trim().is_empty());
    let mut output: Vec<&str> = lines.by_ref().take(n).collect();
    let more = lines.next().is_some();
    let last = output.pop().unwrap_or("");
    let mut text = output.join("\n");
    if !text.is_empty() {
        text.push('\n');
    }
    text.push_str(last);
    if more {
        text.push('…');
    }
    text
}

/// Os tres filtros da gaveta de notas, combinados por E.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NotesFilters {
    /// Busca no texto inteiro da nota (contem, sem diferenciar maiusculas).
    pub text: String,
    /// Fontes marcadas (vazio = todas); a nota pode ser de qualquer uma delas.
    pub sources: BTreeSet<String>,
    /// Etiquetas ligadas (minusculas): a nota precisa ter todas.
    pub tags: BTreeSet<String>,
}

impl NotesFilters {
    pub fn active(&self) -> bool {
        !self.text.trim().is_empty() || !self.sources.is_empty() || !self.tags.is_empty()
    }

    fn accepts(&self, p: &NoteLine) -> bool {
        let search = self.text.trim().to_lowercase();
        (self.sources.is_empty() || self.sources.contains(&p.image.source))
            && self.tags.iter().all(|e| p.tags.contains(e))
            && (search.is_empty() || p.body_min.contains(&search))
    }
}

/// Estado das gavetas (o view-model): qual esta aberta, listas guardadas,
/// filtros e selecao.
#[derive(Debug, Default)]
pub struct Drawers {
    open: Option<Drawer>,
    lists: Lists,
    filters: NotesFilters,
    tasks_filter: TasksFilter,
    selection: usize,
    /// Pedido de exclusao esperando Enter/Esc (nada e excluido antes disso).
    deletion_confirming: Option<RequestDeletion>,
    /// O botao com o foco no dialogo de exclusao.
    deletion_focus: DeletionButton,
    requested: u64,
    received: u64,
    /// Registros recolhidos na arvore (ids); so desta sessao.
    collapsed_all: HashSet<String>,
    /// Largura escolhida arrastando a borda (`None`: [`WIDTH`]).
    width: Option<f32>,
}

impl Drawers {
    pub fn open(&self) -> Option<Drawer> {
        self.open
    }

    /// Abre `g` (ou fecha, se ja era ela): a regra do painel esquerdo de
    /// [`Drawer::toggled`] (um por vez). Devolve se ficou aberta.
    pub fn toggle(&mut self, g: Drawer) -> bool {
        self.deletion_confirming = None;
        let fresh = Drawer::toggled(self.open, g);
        if fresh != self.open && fresh.is_some() {
            self.selection = 0;
        }
        self.open = fresh;
        self.open.is_some()
    }

    pub fn close(&mut self) {
        self.open = None;
        self.deletion_confirming = None;
    }

    /// Quanto a gaveta tira da largura da imagem.
    pub fn busy_width(&self) -> f32 {
        if self.open.is_some() { self.width() } else { 0.0 }
    }

    /// Largura do painel (a escolhida, entre [`WIDTH_MIN`] e [`WIDTH_MAX`], ou a padrao).
    pub fn width(&self) -> f32 {
        self.width.unwrap_or(WIDTH)
    }

    /// Guarda a largura (limitada); valor invalido volta ao padrao.
    pub fn set_width(&mut self, px: f32) {
        self.width = px.is_finite().then(|| px.clamp(WIDTH_MIN, WIDTH_MAX));
    }

    pub fn lists(&self) -> &Lists {
        &self.lists
    }

    /// Nova recarga: devolve a geracao a passar a [`Drawers::receive`].
    pub fn request_reload(&mut self) -> u64 {
        self.requested += 1;
        self.requested
    }

    /// Ha recarga pedida que ainda nao chegou (a lista guardada continua na tela).
    pub fn loading(&self) -> bool {
        self.received != self.requested
    }

    /// Entra com as listas se `generation` for a recarga mais nova; senao ignora.
    /// Filtros ficam; a linha escolhida segue o mesmo id (uma linha nova no topo
    /// nao muda a escolha); se ela sumiu, fica a mesma posicao (limitada).
    pub fn receive(&mut self, generation: u64, l: Lists) -> bool {
        if generation != self.requested {
            return false;
        }
        let chosen = self.id_selected();
        self.received = generation;
        self.lists = l;
        // Etiqueta ligada que nenhuma nota tem mais (o texto foi editado): sem
        // botao para desligar, o filtro ficaria preso. Cai com a recarga.
        let existing = self.vault_tags();
        self.filters.tags.retain(|e| existing.contains(e));
        // O pedido vale para aquele tipo.
        self.deletion_confirming = self.deletion_confirming.take().filter(|c| self.line_has(&c.id, c.kind));
        if let Some(id) = chosen {
            let pos = match self.open {
                Some(Drawer::Notes) => self.notes_visible().iter().position(|p| p.id == id),
                Some(Drawer::Tasks) => self.visible_tasks().iter().position(|t| t.id == id),
                Some(Drawer::Sources) => self.lists.sources.iter().position(|f| f.name == id),
                None => None,
            };
            if let Some(i) = pos {
                self.selection = i;
            }
        }
        true
    }

    fn id_selected(&self) -> Option<String> {
        match self.open? {
            Drawer::Notes => self.note_selected().map(|p| p.id.clone()),
            Drawer::Tasks => self.selected_task().map(|t| t.id.clone()),
            Drawer::Sources => self.selected_source().map(|f| f.name.clone()),
        }
    }

    /// A fonte `old` virou `new` (ja gravado): o filtro e as linhas mudam na
    /// hora; a recarga confirma depois.
    pub fn rename_source(&mut self, old: &str, new: &str) {
        if self.filters.sources.remove(old) {
            self.filters.sources.insert(new.to_string());
        }
        for p in self.lists.notes.iter_mut().filter(|p| p.image.source == old) {
            p.image.source = new.to_string();
        }
        for i in self.lists.tasks.iter_mut().filter_map(|t| t.image.as_mut()).filter(|i| i.source == old) {
            i.source = new.to_string();
        }
        for r in self.lists.records.iter_mut().filter(|r| r.source == old) {
            r.source = new.to_string();
        }
        for f in self.lists.sources.iter_mut().filter(|f| f.name == old) {
            f.name = new.to_string();
        }
        self.lists.sources.sort_by(|a, b| a.name.cmp(&b.name));
    }

    /// O documento `old` passou a se chamar `new` (id fixado pela nota
    /// rapida): linhas, a nota citada pelas tarefas e o menu de promocao
    /// trocam na hora. A linha escolhida e a mesma (a selecao e a posicao).
    pub fn switch_id(&mut self, old: &str, new: &str) {
        for p in self.lists.notes.iter_mut().filter(|p| p.id == old) {
            p.id = new.to_string();
        }
        for t in self.lists.tasks.iter_mut() {
            if t.id == old {
                t.id = new.to_string();
            }
            if t.note.as_deref() == Some(old) {
                t.note = Some(new.to_string());
            }
        }
        if let Some(c) = self.deletion_confirming.as_mut().filter(|c| c.id == old) {
            c.id = new.to_string();
        }
    }

    /// Que documento e a linha `id` (a da gaveta aberta primeiro, se o mesmo id
    /// existe em duas pastas): nota aberta/descartada, nota promovida ou tarefa.
    pub fn line_kind(&self, id: &str) -> Option<DocKind> {
        let note = || {
            self.lists
                .notes
                .iter()
                .find(|p| p.id == id)
                .map(|_| DocKind::Note)
        };
        let task = || self.lists.tasks.iter().any(|t| t.id == id).then_some(DocKind::Task);
        if self.open == Some(Drawer::Tasks) {
            task().or_else(note)
        } else {
            note().or_else(task)
        }
    }

    /// Ha uma linha `id` do tipo `kind` nas listas.
    pub fn line_has(&self, id: &str, kind: DocKind) -> bool {
        match kind {
            DocKind::Task => self.lists.tasks.iter().any(|t| t.id == id),
            DocKind::Note => self.lists.notes.iter().any(|p| p.id == id),
        }
    }

    /// Confirmado excluir `id`: a linha ainda esta nas gavetas? Se sumiu, a
    /// confirmacao fecha e a mensagem diz que nada foi excluido.
    pub fn check_deletion(&mut self, id: &str, kind: DocKind) -> Result<(), String> {
        if self.line_has(id, kind) {
            return Ok(());
        }
        self.cancel_deletion();
        Err(t!("warning.line_not_found", id = format!("{id:?}")))
    }

    pub fn deletion_confirming(&self) -> Option<&RequestDeletion> {
        self.deletion_confirming.as_ref()
    }

    /// Abre a confirmacao para excluir de vez `id` (nada e excluido): conta as
    /// tarefas e notas que o citam e se um grifo fica ligado a ele. Erro (e
    /// nenhum pedido) se `id` nao esta nas listas.
    pub fn request_deletion(&mut self, id: &str) -> Result<&RequestDeletion, String> {
        let Some(kind) = self.line_kind(id) else {
            return Err(t!("warning.line_not_found", id = format!("{id:?}")));
        };
        let (text, highlight) = match kind {
            DocKind::Task => {
                let t = self.lists.tasks.iter().find(|t| t.id == id);
                (t.map(|t| t.text.clone()).unwrap_or_default(), false)
            }
            _ => {
                let p = self.lists.notes.iter().find(|p| p.id == id);
                (p.map(|p| p.text.clone()).unwrap_or_default(), p.is_some_and(|p| p.highlight.is_some()))
            }
        };
        let tasks = self.lists.tasks.iter().filter(|t| t.note.as_deref() == Some(id)).count();
        // Tarefa nao e citada por ninguem.
        let tasks = if kind == DocKind::Task { 0 } else { tasks };
        self.deletion_focus = DeletionButton::Cancel;
        Ok(&*self.deletion_confirming.insert(RequestDeletion { id: id.to_string(), kind, text, tasks, highlight }))
    }

    pub fn cancel_deletion(&mut self) {
        self.deletion_confirming = None;
    }

    /// O foco do teclado mudou. A confirmacao de excluir so ouve as teclas com
    /// a gaveta em foco: se o foco foi para a imagem, a nota ou outro lugar, o
    /// pedido cai (senao o quadro ficaria na tela sem responder). Nada e excluido.
    pub fn focus_changed(&mut self, in_drawer: bool) {
        if !in_drawer {
            self.deletion_confirming = None;
        }
    }

    /// O botao com o foco no dialogo (abre em `Cancel`).
    pub fn deletion_focus(&self) -> DeletionButton {
        self.deletion_focus
    }

    pub fn focus_button_deletion(&mut self, b: DeletionButton) {
        self.deletion_focus = b;
    }

    /// Com a confirmacao aberta ela fica com as teclas: Tab e as setas para os
    /// lados trocam o botao em foco; Enter (ou espaco) aciona o botao em foco,
    /// entao so exclui com o foco em `Delete`; Esc cancela. As outras teclas
    /// nao fazem nada (e o chamador as engole).
    pub fn deletion_key(&mut self, key: &str) -> DeletionAction {
        let Some(p) = self.deletion_confirming.as_ref() else {
            return DeletionAction::Nothing;
        };
        match key {
            "tab" | "left" | "right" => {
                self.deletion_focus = match self.deletion_focus {
                    DeletionButton::Cancel => DeletionButton::Delete,
                    DeletionButton::Delete => DeletionButton::Cancel,
                };
                DeletionAction::Nothing
            }
            "enter" | "space" if self.deletion_focus == DeletionButton::Delete => {
                DeletionAction::Confirm(p.id.clone(), p.kind)
            }
            "enter" | "space" | "escape" => {
                self.deletion_confirming = None;
                DeletionAction::Cancel
            }
            _ => DeletionAction::Nothing,
        }
    }

    /// Depois de excluir (ja gravado): a linha sai na hora; a recarga confirma.
    /// So a linha do tipo excluido sai (o mesmo id pode existir em outra pasta).
    pub fn remove(&mut self, id: &str, kind: DocKind) {
        match kind {
            DocKind::Task => self.lists.tasks.retain(|t| t.id != id),
            DocKind::Note => self.lists.notes.retain(|p| p.id != id),
        }
        if kind != DocKind::Task {
            for t in self.lists.tasks.iter_mut().filter(|t| t.note.as_deref() == Some(id)) {
                t.note_deleted = true;
            }
        }
        self.deletion_confirming = self.deletion_confirming.take().filter(|c| !(c.id == id && c.kind == kind));
    }

    /// Troca de vault: os filtros (fonte, texto, etiquetas) eram do vault anterior.
    pub fn clear_other_vault_filters(&mut self) {
        self.filters = NotesFilters::default();
        self.deletion_confirming = None;
        self.selection = 0;
    }

    pub fn notes_filters(&self) -> &NotesFilters {
        &self.filters
    }

    pub fn tasks_filter(&self) -> TasksFilter {
        self.tasks_filter
    }

    /// Marca ou desmarca a fonte `f` no filtro (varias podem ficar marcadas).
    pub fn toggle_source(&mut self, f: &str) {
        if !self.filters.sources.remove(f) {
            self.filters.sources.insert(f.to_string());
        }
        self.selection = 0;
    }

    /// Desmarca todas as fontes (= todas valem).
    pub fn clear_sources(&mut self) {
        self.filters.sources.clear();
        self.selection = 0;
    }

    /// A busca no texto das notas (`""` limpa).
    pub fn set_search(&mut self, t: &str) {
        self.filters.text = t.to_string();
        self.selection = 0;
    }

    /// Liga ou desliga a etiqueta `e` (com ou sem `#`, qualquer caixa) no filtro.
    pub fn toggle_tag(&mut self, e: &str) {
        let e = e.trim_start_matches('#').to_lowercase();
        if !self.filters.tags.remove(&e) {
            self.filters.tags.insert(e);
        }
        self.selection = 0;
    }

    /// Etiquetas de todas as notas do vault, sem repetir, em ordem alfabetica.
    /// A cor pastel de cada etiqueta do vault, todas diferentes entre si.
    pub fn tags_colors(&self) -> std::collections::BTreeMap<String, u32> {
        marginalia_core::tags::colors_pastel(&self.vault_tags())
    }

    pub fn vault_tags(&self) -> Vec<String> {
        let all: BTreeSet<&String> = self.lists.notes.iter().flat_map(|p| p.tags.iter()).collect();
        all.into_iter().cloned().collect()
    }

    /// `(visiveis, total)` das notas, com os filtros.
    pub fn count_notes(&self) -> (usize, usize) {
        (self.notes_filtered().len(), self.lists.notes.len())
    }

    /// O contador do titulo: `"2 de 4"` com filtro ligado, `"4 notes"` sem.
    pub fn counter_notes(&self) -> String {
        let (v, t) = self.count_notes();
        if self.filters.active() {
            t!("drawer.counter.filtered", v = v, t = t)
        } else if t == 1 {
            t!("drawer.counter.uma")
        } else {
            t!("drawer.counter.several", t = t)
        }
    }

    pub fn set_filter_tasks(&mut self, f: TasksFilter) {
        self.tasks_filter = f;
        self.selection = 0;
    }

    /// Fontes citadas pelas notas (para os botoes de filtro), em ordem.
    pub fn note_sources(&self) -> Vec<String> {
        let mut v: Vec<String> = self.lists.notes.iter().map(|p| p.image.source.clone()).collect();
        v.sort();
        v.dedup();
        v
    }

    /// Notas que passam nos filtros, na ordem das listas (recolher nao tira nenhuma).
    fn notes_filtered(&self) -> Vec<&NoteLine> {
        self.lists.notes.iter().filter(|p| self.filters.accepts(p)).collect()
    }

    fn build_notes_tree(&self) -> (Vec<&NoteLine>, Tree) {
        let filtered = self.notes_filtered();
        let es: Vec<Input<'_>> = filtered
            .iter()
            .map(|p| Input { id: &p.id, image: Some(&p.image), record: p.record.as_deref() })
            .collect();
        // Registro vazio so some quando um filtro de texto ou etiqueta esta ligado.
        let empty = self.filters.text.trim().is_empty() && self.filters.tags.is_empty();
        let source_ok = |f: &str| self.filters.sources.is_empty() || self.filters.sources.contains(f);
        let a = tree::build(&es, &self.lists.records, &self.collapsed_all, empty, &source_ok);
        (filtered, a)
    }

    /// As notas na ordem em que a gaveta as mostra: a arvore fonte -> registro
    /// (por posicao da imagem) quando o vault tem registros, senao as mais novas
    /// primeiro; sem as de registros recolhidos. A selecao e Enter indexam esta lista.
    pub fn notes_visible(&self) -> Vec<&NoteLine> {
        let (f, a) = self.build_notes_tree();
        a.order.iter().map(|&i| f[i]).collect()
    }

    /// As linhas da gaveta de notas (`Item(k)` e a k-esima de `notes_visible`).
    pub fn tree_notes(&self) -> Vec<TreeLine> {
        self.build_notes_tree().1.lines
    }

    fn filtered_tasks(&self) -> Vec<&LineTask> {
        let done_all = self.tasks_filter == TasksFilter::Done;
        self.lists.tasks.iter().filter(|t| t.done == done_all).collect()
    }

    fn build_tasks_tree(&self) -> (Vec<&LineTask>, Tree) {
        let filtered = self.filtered_tasks();
        let es: Vec<Input<'_>> =
            filtered.iter().map(|t| Input { id: &t.id, image: t.image.as_ref(), record: t.record.as_deref() }).collect();
        let a = tree::build(&es, &self.lists.records, &self.collapsed_all, false, &|_| true);
        (filtered, a)
    }

    pub fn visible_tasks(&self) -> Vec<&LineTask> {
        let (f, a) = self.build_tasks_tree();
        a.order.iter().map(|&i| f[i]).collect()
    }

    /// As linhas da gaveta de tarefas (`Item(k)` e a k-esima de `visible_tasks`).
    pub fn tasks_tree(&self) -> Vec<TreeLine> {
        self.build_tasks_tree().1.lines
    }

    /// Recolhe ou expande o registro `id` na arvore.
    /// Recolhe ou expande a fonte `name` na arvore (chave `source:<name>`).
    pub fn toggle_tree_source(&mut self, name: &str) {
        self.toggle_record(&format!("fonte:{name}"));
    }

    pub fn toggle_record(&mut self, id: &str) {
        if !self.collapsed_all.remove(id) {
            self.collapsed_all.insert(id.to_string());
        }
    }

    pub fn collapsed(&self, id: &str) -> bool {
        self.collapsed_all.contains(id)
    }

    fn record_at(&self, img: &ImageRef, override_id: Option<&str>) -> Option<&Record> {
        let from_source: Vec<Record> = self.lists.records.iter().filter(|r| r.source == img.source).cloned().collect();
        let ext = marginalia_core::records::extensions(&from_source);
        let id = marginalia_core::records::record_of(&ext, img, override_id)?.id.clone();
        self.lists.records.iter().find(|r| r.id == id)
    }

    /// O registro em que a nota esta (o do override se valer, senao o primeiro da imagem).
    pub fn note_record(&self, p: &NoteLine) -> Option<&Record> {
        self.record_at(&p.image, p.record.as_deref())
    }

    /// Idem para a tarefa (sem imagem: nenhum).
    pub fn task_record(&self, t: &LineTask) -> Option<&Record> {
        self.record_at(t.image.as_ref()?, t.record.as_deref())
    }

    /// "Mover para..." de um item numa imagem de fronteira; `None` fora dela.
    pub fn relocate_destination(&self, img: &ImageRef, override_id: Option<&str>) -> Option<DestinationRelocate> {
        tree::relocate_destination(&self.lists.records, img, override_id)
    }

    pub fn sources(&self) -> &[LineSource] {
        &self.lists.sources
    }

    /// As fontes do vault na ordem de `recent` (a mais recente primeiro); as
    /// que a lista nao cita ficam de fora, e nome que o vault nao tem some.
    pub fn recent_sources(&self, recent: &[String]) -> Vec<&LineSource> {
        recent.iter().filter_map(|n| self.lists.sources.iter().find(|f| &f.name == n)).collect()
    }

    fn visible_size(&self) -> usize {
        match self.open {
            Some(Drawer::Notes) => self.notes_visible().len(),
            Some(Drawer::Tasks) => self.visible_tasks().len(),
            Some(Drawer::Sources) => self.lists.sources.len(),
            None => 0,
        }
    }

    /// Linha selecionada da gaveta aberta (limitada a lista atual, que pode
    /// ter encolhido depois de uma gravacao ou recarga).
    pub fn selection(&self) -> Option<usize> {
        let n = self.visible_size();
        (n > 0).then(|| self.selection.min(n - 1))
    }

    pub fn select(&mut self, i: usize) {
        self.selection = i;
    }

    pub fn relocate_selection(&mut self, delta: i32) {
        if let Some(s) = self.selection() {
            let n = self.visible_size() as i64;
            self.selection = (s as i64 + delta as i64).clamp(0, n - 1) as usize;
        }
    }

    pub fn note_selected(&self) -> Option<&NoteLine> {
        if self.open != Some(Drawer::Notes) {
            return None;
        }
        self.notes_visible().get(self.selection()?).copied()
    }

    pub fn selected_task(&self) -> Option<&LineTask> {
        if self.open != Some(Drawer::Tasks) {
            return None;
        }
        self.visible_tasks().get(self.selection()?).copied()
    }

    /// Backspace / "Excluir nota/tarefa...": so com a gaveta de notas ou de
    /// tarefas aberta, uma linha escolhida, nenhuma confirmacao aberta e o
    /// foco (`drawer_focus`, que a view sabe) na gaveta.
    pub fn can_delete_item(&self, drawer_focus: bool) -> bool {
        let line = self.note_selected().is_some() || self.selected_task().is_some();
        line && drawer_focus && self.deletion_confirming().is_none()
    }

    pub fn selected_source(&self) -> Option<&LineSource> {
        if self.open != Some(Drawer::Sources) {
            return None;
        }
        self.lists.sources.get(self.selection()?)
    }

    /// Depois de gravar: muda a linha na hora (a recarga confirma depois).
    pub fn mark_task(&mut self, id: &str, done: bool) {
        if let Some(t) = self.lists.tasks.iter_mut().find(|t| t.id == id) {
            t.done = done;
        }
    }

    /// `(mostradas, total)` quando a lista da gaveta aberta passa de [`MAX_LINES`].
    pub fn cut(&self) -> Option<(usize, usize)> {
        let n = self.visible_size();
        (n > MAX_LINES).then_some((MAX_LINES, n))
    }

    /// "Mostrando 300 de N — use os filtros", se a lista foi cortada.
    pub fn cut_warning(&self) -> Option<String> {
        self.cut().map(|(m, n)| t!("drawer.cut", m = m, n = n))
    }

    /// Dica de uma linha quando a lista da gaveta aberta esta vazia.
    pub fn empty(&self) -> Option<String> {
        if self.visible_size() > 0 {
            return None;
        }
        let hint = match self.open? {
            Drawer::Notes if self.filters.active() && !self.lists.notes.is_empty() => t!("drawer.empty.filtered_notes"),
            Drawer::Notes => t!("drawer.empty.notes"),
            Drawer::Tasks => match self.tasks_filter {
                TasksFilter::Open => t!("drawer.empty.open_tasks"),
                TasksFilter::Done => t!("drawer.empty.done_tasks"),
            },
            Drawer::Sources => t!("drawer.empty.sources"),
        };
        Some(hint)
    }
}

#[cfg(test)]
#[path = "tests/drawers.rs"]
mod tests;
