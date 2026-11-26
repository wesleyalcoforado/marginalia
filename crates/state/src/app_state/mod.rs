//! Estado do app, sem gpui (regra R4): fonte aberta, quadro atual, ajustes e
//! erro do quadro. Nada aqui bloqueia a thread da interface: navegar so troca o
//! quadro, chama `Prefetcher::hint` e devolve um [`Request`] que a view executa
//! no executor de fundo (`load::execute`); a [`Response`] volta por
//! [`AppState::receive`], que descarta respostas obsoletas (vale a mais nova).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use marginalia_core::app_data::AppData;
use marginalia_core::filters::Adjustments;
use marginalia_core::highlights::{Highlight, HighlightsFile, DEFAULT_COLOR, PALETTE};
use marginalia_core::model::{StateTask, Framing, ImageRef, Task, SourceKind};
use marginalia_core::prefetch::Prefetcher;
use marginalia_core::rotation::Rotation;
use marginalia_core::source::ImageSource;
use marginalia_core::vault::{Found, DocKind, Vault};
use texts::t;

use crate::load::{Layer, Reframe, Display, ViewWindow, Page, Request, PrefetcherShared, Quality, Response, Outcome};
use crate::drawers::{self, Lists, NoteLine, LineTask};
use crate::open_source;
use crate::relink::{self, Expected, Reason, Pending};
use crate::vault_picker;
pub use records::RecordStrip;
use crate::viewer_math;

/// Uma imagem aceita para a tela: numero na sessao, qualidade e ajustes que levou.
#[derive(Clone, Debug, PartialEq)]
pub struct Displayed {
    pub number: u64,
    pub quality: Quality,
    pub image_adjustments: Adjustments,
}

/// Uma camada (base ou tile) na tela, sem os pixels: eles vao inteiros para o
/// visualizador (dono da textura; nada de copia na thread da interface). O
/// tamanho da textura e o numero da camada na sessao (cada aceita ganha um).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayerInTheScreen {
    pub sz: (u32, u32),
    pub number: u64,
}

/// Quantas exibicoes [`AppState::displays`] guarda.
const MAX_DISPLAYS: usize = 32;

/// Inicio do aviso de gravacao dos ajustes que falhou (o texto sem o erro).
fn warning_adjustments_start() -> String {
    t!("warning.image_adjustments", "")
}

/// Quantos quadros pre-carregar a frente (o `Prefetcher` aceita 3..=5).
pub const AHEAD: usize = 4;

/// Por que uma fonte nao abriu. `AbsentSource`: sem caminho nesta maquina ou
/// caminho que nao existe; nada foi aberto e [`AppState::pending`] tem o
/// pedido de nova localizacao para a interface mostrar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceError {
    AbsentSource { name: String },
    Other(String),
}

impl std::fmt::Display for SourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceError::AbsentSource { name } => {
                f.write_str(&t!("error.source.absent", name = format!("{name:?}")))
            }
            SourceError::Other(m) => f.write_str(m),
        }
    }
}

impl From<String> for SourceError {
    fn from(m: String) -> Self {
        SourceError::Other(m)
    }
}

impl From<SourceError> for String {
    fn from(e: SourceError) -> Self {
        e.to_string()
    }
}

/// Como `Vault::rename_source` falhou ([`sort_failure_rename`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureRename {
    /// Nada foi reescrito (nome de outra fonte, documento ilegivel...).
    Common,
    /// Parou no meio: `sources/<new>.md` tem a marca de `old`; repetir termina.
    Partial,
    /// Parou depois de tirar `sources/<old>.md`: so falta a limpeza; recarregar mostra o nome novo.
    PartialNoOld,
}

/// Quem acompanha o novo nome de uma fonte renomeada ([`renaming_follower`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenamingFollows {
    /// A sessao da nota usa o nome novo (toda nota do vault, ate a que ainda nao foi criada).
    pub session: bool,
    /// A tela segue: a agenda de ajustes, as gavetas e a caixa de renomear.
    pub screen: bool,
    /// Confirmacao da barra e foco de volta na imagem.
    pub confirms: bool,
}

/// O resultado de `vault.rename_source` (`Ok`, ou como falhou) e se o vault
/// ativo ainda e aquele em que a renomeacao foi pedida: o que acompanha o nome
/// novo. Sucesso: tudo. Parou depois de tirar o arquivo antigo: so o nome novo
/// ficou, entao a sessao e a tela o usam, sem confirmacao. Qualquer outra
/// falha: nada muda de nome.
pub fn renaming_follower(r: Result<(), FailureRename>, vault_same: bool) -> RenamingFollows {
    match r {
        Ok(()) => RenamingFollows { session: true, screen: vault_same, confirms: vault_same },
        Err(FailureRename::PartialNoOld) => RenamingFollows { session: true, screen: vault_same, confirms: false },
        Err(_) => RenamingFollows { session: false, screen: false, confirms: false },
    }
}

/// Classifica a falha de `vault.rename_fonte(antigo, novo)`. Toca o disco: no fundo.
/// So e parcial se o erro nao for `DuplicateName` E `sources/<new>.md` tiver a
/// marca de uma renomeacao de `old` (`Vault::interrupted_renaming`).
pub fn sort_failure_rename(vault: &Vault, old: &str, new: &str, error: &marginalia_core::Error) -> FailureRename {
    if matches!(error, marginalia_core::Error::DuplicateName(_)) || !vault.interrupted_renaming(old, new) {
        FailureRename::Common
    } else if vault.root.join("fontes").join(format!("{old}.md")).is_file() {
        FailureRename::Partial
    } else {
        FailureRename::PartialNoOld
    }
}

/// O que [`AppState::enter_source`] fez com uma abertura que chegou do fundo.
pub enum Input {
    /// O vault mudou desde o pedido: o resultado foi descartado.
    Stale,
    /// A fonte entrou; `fresh`: foi registrada agora no vault (recarregar as gavetas).
    Open { request: Option<Request>, fresh: bool },
    /// Nada abriu (`AbsentSource` deixa o pedido de nova localizacao).
    Failed(SourceError),
}

/// O que a view deve fazer com uma resposta.
pub enum Received {
    /// Uma imagem para a tela na camada dita (os pixels vem so aqui: o estado
    /// guarda so o tamanho, [`LayerInTheScreen`]). Com `Layer::Tile` o visualizador
    /// mantem a base so se [`AppState::base_display`] ainda a tem; com
    /// `Layer::Base`, o tile so se [`AppState::tile_display`] o tem.
    Image(Display, Reframe, Layer),
    Error(String),
    /// A fonte nao pode ser lida (E/S): marcador de erro E o aviso "Fonte
    /// indisponivel" com "Tentar de novo" / "Apontar nova localizacao...".
    Unavailable(String),
    /// Uma base dos mesmos valores do tile que ja esta na tela: so a camada de
    /// fundo muda ([`AppState::base_display`]); o tile e o enquadramento ficam.
    Layer(Display),
    /// Um tile caiu so porque a janela dele deixou de valer: nada mais esta a
    /// caminho para ele; a view pede o da janela vigente pela agenda
    /// (`ScheduleAdjustment::fell_tile`).
    WindowChanged,
    /// Resposta obsoleta: nada muda.
    Ignored,
}

pub struct AppState {
    /// Vault ativo (seletor: `switch_vault`). `None`: fontes abrem sem registro
    /// e a nota rapida fica desligada.
    pub vault: Option<Vault>,
    pub app_data: AppData,
    /// Solta a ultima copia fora da thread atual (ver `PrefetcherShared`).
    pub prefetcher: Option<PrefetcherShared>,
    /// Quadro atual. Sem fonte aberta: fonte e arquivo vazios.
    pub current: Frame,
    /// Ajustes do quadro atual (lidos e gravados em `app_data` por imagem).
    pub adjustments: Adjustments,
    /// Rotacao de exibicao do quadro atual (lida e gravada em `app_data` por imagem).
    rotation: Rotation,
    /// Erro do quadro atual (leitura/decodificacao/fonte).
    pub error: Option<String>,
    /// Aviso que nao impede ver a imagem (falha ao salvar, ao abrir fonte...).
    pub warning: Option<String>,
    pub confirmation: Option<String>,
    source: Option<Arc<dyn ImageSource>>,
    /// Pagina decodificada do quadro atual (para reaplicar filtros sem reler).
    /// Com a copia reduzida da base e o recorte do ultimo tile (no fundo).
    page: Option<Arc<Page>>,
    /// Geracao e valores do ultimo pedido de base ou tile: outro pedido (base ou
    /// tile) dos MESMOS ajustes reusa a geracao (nao derruba o que esta em voo).
    request_adjustment: Option<(u64, Adjustments)>,
    /// Janela visivel informada pelo visualizador (`None`: ainda nao informou, o
    /// tile cobre a pagina inteira). Cai ao trocar de imagem/pagina.
    view_window: Option<ViewWindow>,
    /// Tile na tela: a textura (so tamanho e numero; os pixels foram para o
    /// visualizador), janela e ajustes que levou.
    tile: Option<(LayerInTheScreen, ViewWindow, Adjustments)>,
    /// Base na tela (pagina inteira reduzida, filtrada) e ajustes que levou.
    base: Option<(LayerInTheScreen, Adjustments)>,
    /// Quantas camadas (base ou tile) foram aceitas: o numero de [`LayerInTheScreen`].
    layers_total: u64,
    /// Fila numerada das gravacoes dos dados do app.
    app_data_queue: WriteQueue,
    /// Geracao da ultima base mostrada (o tile dela nao reenquadra).
    displayed_base: Option<u64>,
    /// Geracao do ultimo tile (ou cheia) mostrado: base dela (ou mais velha) nunca o cobre.
    displayed_tile: Option<u64>,
    /// O olho do painel: a tela mostra os ajustes (ligado) ou a imagem sem
    /// eles (desligado; nada apagado). Vale para a sessao, nao vai ao appdata.
    show_adjustments: bool,
    /// Os ajustes da imagem que esta na tela (o que o ultimo resultado aceito levou).
    screen_adjustments: Option<Adjustments>,
    /// As ultimas imagens aceitas para a tela, numeradas (o smoke confere que
    /// uma final interrompida nunca apareceu). No maximo [`MAX_DISPLAYS`].
    displays: std::collections::VecDeque<Displayed>,
    displays_total: u64,
    total_pages: Option<u32>,
    loading: bool,
    /// Como enquadrar quando o quadro atual (ainda carregando) chegar.
    frame_pending: Option<Reframe>,
    generation: Generation,
    cache_dir: PathBuf,
    openings: u64,
    /// Caminhos de fontes abertas sem vault, so nesta sessao.
    session_paths: HashMap<String, PathBuf>,
    /// Renomeacao que parou no meio `(vault, antigo, novo)`: `sources/<new>.md`
    /// ja existe mas o nome continua livre para terminar a mesma renomeacao.
    interrupted_renaming: Option<(PathBuf, String, String)>,
    /// A posicao atual mudou nos dados do app e ainda nao foi salva (salva na
    /// troca de fonte, de vault e ao sair; nao a cada tecla).
    dirty_position: bool,
    /// go_next/go_prev pedido enquanto o quadro atual carregava sem total de
    /// paginas conhecido; vale o mais novo, retomado por `resume_navigation`.
    nav_pending: Option<Direction>,
    /// Espelho do enquadramento do visualizador (zoom/centro) do quadro atual.
    fr: Framing,
    /// Grifos, pendente, destaque, selecao e desfazer da fonte aberta.
    pub annot: Annotations,
    /// A imagem do quadro atual chegou (o visualizador a mostra).
    displayed: bool,
    /// Nota aberta por `open_note` e a imagem dela (cai ao ir para outra imagem).
    note_open: Option<(String, ImageRef)>,
    /// Ultima nota descartada (a barra oferece desfazer).
    /// Fonte que nao pode ser lida: o aviso com "Tentar de novo" / "Apontar nova localizacao".
    pending: Option<Pending>,
    /// Clique numa linha de outra fonte: o texto que vai para a caixa quando a
    /// fonte entrar (`postponed_load`).
    postponed_load: Option<(DocKind, String)>,
    /// Geracao da ultima [`Reload`] pedida (uma mais velha nao troca os grifos).
    reload: u64,
    /// Registros do vault (para a faixa do leitor; ver `records.rs`).
    records: Vec<marginalia_core::model::Record>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Direction {
    Next,
    Previous,
}

impl AppState {
    /// `cache_dir`: raiz do cache de zips (apagada ao fechar o app).
    pub fn new(vault: Option<Vault>, app_data: AppData, cache_dir: PathBuf) -> AppState {
        AppState {
            vault,
            app_data,
            prefetcher: None,
            current: Frame::new("", "", 1),
            adjustments: Adjustments::default(),
            rotation: Rotation::R0,
            error: None,
            warning: None,
            confirmation: None,
            source: None,
            page: None,
            request_adjustment: None,
            view_window: None,
            tile: None,
            base: None,
            layers_total: 0,
            app_data_queue: WriteQueue::default(),
            displayed_base: None,
            displayed_tile: None,
            show_adjustments: true,
            screen_adjustments: None,
            displays: Default::default(),
            displays_total: 0,
            total_pages: None,
            loading: false,
            frame_pending: None,
            generation: Generation::default(),
            cache_dir,
            openings: 0,
            session_paths: HashMap::new(),
            interrupted_renaming: None,
            dirty_position: false,
            nav_pending: None,
            fr: viewer_math::default_framing(),
            annot: Annotations::default(),
            displayed: false,
            note_open: None,
            pending: None,
            postponed_load: None,
            reload: 0,
            records: Vec::new(),
        }
    }


}

/// Por que a nota rapida nao criou nada. So `Empty` e silenciosa na view.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))] // `Empty` e `Write` so saem de `submit_quick_note`
pub enum QuickNoteError {
    /// Texto vazio ou so espacos (ou `[]` sem texto): nada a criar.
    Empty,
    /// Nenhum vault aberto (o seletor, Cmd-Shift-V, escolhe um).
    NoVault,
    /// Nenhuma imagem aberta.
    NoImage,
    /// A gravacao no vault falhou; o texto continua na caixa.
    Write(String),
}

impl std::fmt::Display for QuickNoteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QuickNoteError::Empty => f.write_str(&t!("error.quick_note.empty")),
            QuickNoteError::NoVault => f.write_str(&t!("error.quick_note.no_vault")),
            QuickNoteError::NoImage => f.write_str(&t!("error.quick_note.no_image")),
            QuickNoteError::Write(m) => f.write_str(&t!("error.quick_note.write", m)),
        }
    }
}

/// Resultado de [`AppState::open_note`].
pub enum OpenNote {
    /// A nota e da fonte aberta: o quadro ja e o atual; execute o pedido.
    ImageFrame(Option<Request>),
    /// A nota e de outra fonte: rode `opening.execute()` no fundo, entregue
    /// a fonte a [`AppState::adopt_source_in_the`] com `image_frame` e depois chame
    /// [`AppState::highlight`] com `highlight`.
    Source { opening: Opening, image_frame: Frame, highlight: Option<String> },
}

/// Vault com que o app abre: `--vault`, senao
/// `MARGINALIA_VAULT`, senao o ultimo vault dos dados do app SE a pasta ainda
/// existir (um vault que sumiu, por exemplo num disco desmontado, nao e recriado).
pub fn choose_vault(cli: Option<PathBuf>, env: Option<PathBuf>, last: Option<&Path>) -> Option<PathBuf> {
    let not_empty = |p: &PathBuf| !p.as_os_str().is_empty();
    cli.filter(not_empty)
        .or_else(|| env.filter(not_empty))
        .or_else(|| last.filter(|p| p.is_dir()).map(Path::to_path_buf))
}

/// Nova localizacao de uma fonte, a validar no fundo ([`Relink::execute`]) e
/// depois aplicar com [`AppState::conclude_relink`].
#[derive(Clone, Debug)]
pub struct Relink {
    pub name: String,
    pub file_path: PathBuf,
    pub expected: Expected,
    cache_dir: PathBuf,
    vault: Option<Vault>,
}

impl Relink {
    /// Abre `file_path`, confere que tem as imagens da fonte e le os grifos dela.
    /// BLOQUEIA (NAS): rode no executor de fundo.
    pub fn execute(&self) -> Result<Validated, String> {
        let (source, confirm) = relink::validate(&self.file_path, &self.expected, &self.name, &self.cache_dir)?;
        let highlights = self.vault.as_ref().map(|v| HighlightsFile::load(v, &self.name).map_err(|e| e.to_string()));
        Ok(Validated { ready: Ready { source, highlights }, confirm })
    }
}

/// Nova localizacao aberta e conferida. `confirmar: Some(n)`: nada no vault cita
/// imagens da fonte; a interface pergunta (`relink::confirmation_question`)
/// antes de [`AppState::conclude_relink`].
pub struct Validated {
    pub ready: Ready,
    pub confirm: Option<usize>,
}

/// O que [`AppState::adopt_vault`] teve de largar ao trocar de vault.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Swap {
    /// A fonte aberta nao e deste vault (nesse caminho) e foi fechada.
    pub closed_source: Option<String>,
    /// Havia um grifo pendente (do vault anterior): nao vai para a proxima nota.
    pub discarded_highlight: bool,
}

impl Swap {
    /// Confirmacao da barra depois de trocar para o vault `name`: o que a troca
    /// deixou para tras. `item_in_editing`: a sessao da nota tem um item (ele
    /// termina no vault anterior); `textbox_with_text`: a caixa tem rascunho.
    pub fn message(&self, name: &str, item_in_editing: bool, textbox_with_text: bool) -> String {
        let mut msg = t!("status.swap.vault", name);
        if item_in_editing {
            msg = t!("warning.join", msg, t!("status.swap.quick_note_in_editing"));
        } else if textbox_with_text {
            msg = t!("warning.join", msg, t!("status.swap.draft"));
        }
        if self.discarded_highlight {
            msg = t!("warning.join", msg, t!("status.swap.pending_highlight"));
        }
        msg
    }

    /// Aviso quando a fonte aberta nao era deste vault e foi fechada.
    pub fn source_warning(&self) -> Option<String> {
        self.closed_source.as_ref().map(|f| t!("warning.source_closed_other_vault", f = format!("{f:?}")))
    }
}

/// Recarga do vault (ao voltar o foco da janela, depois de gravar, trocar de
/// vault, renomear ou apontar uma fonte): roda no fundo com [`Reload::execute`].
pub struct Reload {
    generation: u64,
    vault: Vault,
    app_data: AppData,
    source: Option<String>,
    highlights_version: u64,
}

/// O que a [`Reload`] leu.
pub struct Reread {
    generation: u64,
    root_dir: PathBuf,
    source: Option<String>,
    highlights_version: u64,
    pub lists: Lists,
    highlights: Option<Result<HighlightsFile, String>>,
}

impl Reload {
    /// Le as listas das gavetas e os grifos da fonte aberta. BLOQUEIA (vault,
    /// talvez no NAS; status do caminho de cada fonte): rode no fundo. Arquivo
    /// que sumiu ou nao se le e pulado (contado em `lists.unreadable`).
    pub fn execute(self) -> Reread {
        let lists = drawers::load(&self.vault, &self.app_data);
        let highlights = self.source.as_ref().map(|f| HighlightsFile::load(&self.vault, f).map_err(|e| e.to_string()));
        Reread {
            generation: self.generation,
            root_dir: self.vault.root.clone(),
            source: self.source,
            highlights_version: self.highlights_version,
            lists,
            highlights,
        }
    }
}

/// Controles do painel de ajustes com passo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Brightness,
    Contrast,
    Sharpness,
    Highpass,
    Threshold,
}

/// `a` com `control` movido `steps` passos (negativo = menos), nos limites
/// de `filters::apply`. Limiar: o primeiro passo a partir de desligado liga em
/// 128; descer abaixo de 0 desliga.
pub fn step(a: &Adjustments, control: Control, steps: i32) -> Adjustments {
    let mut r = a.clone();
    let n = steps as f32;
    // Arredonda ao passo para nao acumular erro de ponto flutuante.
    let relocate = |v: f32, step: f32, min: f32, max: f32| {
        let v = if v.is_finite() { v } else { 0.0 };
        (((v / step).round() + n) * step).clamp(min, max)
    };
    match control {
        Control::Brightness => r.brightness = relocate(a.brightness, 0.05, -1.0, 1.0),
        Control::Contrast => {
            let c = if a.contrast.is_finite() { a.contrast } else { 1.0 };
            r.contrast = relocate(c, 0.1, 0.0, 4.0)
        }
        Control::Sharpness => r.sharpen = relocate(a.sharpen, 0.1, 0.0, 1.0),
        Control::Highpass => r.highpass = relocate(a.highpass, 0.1, 0.0, 1.0),
        Control::Threshold => {
            r.threshold = match a.threshold {
                None if steps > 0 => Some(128),
                None => None,
                Some(t) => {
                    let v = t as i32 + steps * 8;
                    if v < 0 { None } else { Some(v.min(255) as u8) }
                }
            }
        }
    }
    r
}

mod schedule;
mod annotations;
mod display;
mod generation;
mod highlights;
mod navigation;
mod notes;
mod persistence;
mod records;
mod vault;

pub use schedule::{ScheduleAfter, RotationAfter, ScheduleStep, Followed};
pub use annotations::Annotations;
pub use crate::open_source::{Opening, OpeningFailure, Ready, ZipMode};
pub use crate::image_frame::Frame;
pub use navigation::{target_go_to, interpret_go_to, UnitIr};
pub use notes::Context;
pub use persistence::WriteAppData;
use generation::Generation;
use persistence::WriteQueue;

#[cfg(test)]
#[path = "tests/app_state.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/test.rs"]
pub(crate) mod test;
