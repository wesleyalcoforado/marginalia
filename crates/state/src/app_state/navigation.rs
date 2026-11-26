//! Navegacao: abrir fonte, trocar de quadro, fechar.

use super::*;

impl AppState {
    /// Primeiro quadro da fonte aberta.
    pub fn first_frame(&self) -> Option<Frame> {
        let e = self.source.as_ref()?.entries().first()?;
        Some(Frame::new(&self.current.image.source, &e.path, 1))
    }

    pub fn source_has(&self) -> bool {
        self.source.is_some()
    }

    pub fn loading(&self) -> bool {
        self.loading
    }

    /// Posicao do arquivo atual na fonte: `(i, total)`, `i` 1-based.
    pub fn index(&self) -> Option<(usize, usize)> {
        let entries = self.source.as_ref()?.entries();
        let i = entries.iter().position(|e| e.path == self.current.image.file)?;
        Some((i + 1, entries.len()))
    }

    /// `(pagina, total)`; o total so e conhecido depois de decodificar.
    pub fn page(&self) -> (u32, Option<u32>) {
        (self.current.image.page, self.total_pages)
    }

    /// Em que o "Ir para" conta e onde estamos: `(unidade, atual, total)`. Um arquivo so com
    /// varias paginas (PDF) conta paginas; qualquer outra fonte conta imagens (arquivos).
    /// `None` sem fonte, ou enquanto o total de paginas de um arquivo unico ainda nao se sabe.
    pub fn unit_go_to(&self) -> Option<(UnitIr, usize, usize)> {
        let entries = self.source.as_ref()?.entries();
        if entries.is_empty() {
            return None;
        }
        if entries.len() == 1 && self.error.is_none() {
            return match self.total_pages {
                Some(n) if n > 1 => Some((UnitIr::Page, self.current.image.page as usize, n as usize)),
                Some(_) => Some((UnitIr::Image, 1, 1)),
                None => None,
            };
        }
        let (i, n) = self.index()?;
        Some((UnitIr::Image, i, n))
    }

    /// "Ir para N": N abaixo de 1 vale 1 e acima do total vale o ultimo (`target_go_to`).
    /// `Ok(None)`: ja estamos la. Erro: nao ha fonte ou o total ainda nao e conhecido.
    pub fn go_to_number(&mut self, n: i64) -> Result<Option<Request>, String> {
        if !self.source_has() {
            return Err(t!("warning.ir_to.no_source"));
        }
        let Some((unit, current, total)) = self.unit_go_to() else {
            return Err(t!("warning.ir_to.aguarde"));
        };
        let target = target_go_to(n, total);
        if target == current {
            return Ok(None);
        }
        let entries = self.source.as_ref().map(|s| s.entries().to_vec()).unwrap_or_default();
        let source = self.current.image.source.clone();
        let frame = match unit {
            UnitIr::Page => Frame::new(&source, &entries[0].path, target as u32),
            UnitIr::Image => Frame::new(&source, &entries[target - 1].path, 1),
        };
        self.warning = None;
        Ok(self.ir_to(frame))
    }

    /// Guarda o caminho de uma fonte aberta sem vault (so nesta sessao).
    pub fn remember_path(&mut self, name: &str, file_path: PathBuf) {
        self.session_paths.insert(name.to_string(), file_path);
    }

    /// Resolve `name` (vault + dados do app, ou caminhos desta sessao) e
    /// prepara a abertura. Nao toca o NAS: rode `Opening::execute` no fundo e
    /// entregue o resultado a [`AppState::adopt_source`].
    ///
    /// Nao toca o disco. Sem caminho nesta maquina: nada abre, devolve
    /// [`SourceError::AbsentSource`] e deixa o pedido em [`AppState::pending`].
    /// Um caminho guardado que nao existe mais so aparece no fundo
    /// (`Opening::execute_ready` -> [`OpeningFailure::Absent`] ->
    /// [`AppState::enter_source`], que devolve o mesmo `AbsentSource`): o `stat`
    /// de um NAS morto nunca prende a thread da interface.
    pub fn open_source(&mut self, name: &str, mode: ZipMode) -> Result<Opening, SourceError> {
        // Caminhos desta sessao primeiro (fonte sem vault, arquivo avulso).
        let file_path = match (&self.vault, self.session_paths.get(name)) {
            (_, Some(p)) => p.clone(),
            (Some(v), None) => match self.app_data.source_path(&v.root, name) {
                Some(p) => p,
                None => return Err(self.absent(name, None, Reason::NoPath)),
            },
            (None, None) => {
                return Err(SourceError::Other(t!("error.source.unknown", name = format!("{name:?}"))))
            }
        };
        Ok(self.path_opening(name, file_path, mode))
    }

    /// Abertura de `file_path` como `name`, sem resolver nem tocar o disco
    /// (Cmd-O, gaveta de fontes). O caminho so e lembrado depois que abrir.
    pub fn path_opening(&mut self, name: &str, file_path: PathBuf, mode: ZipMode) -> Opening {
        // Uma subpasta por abertura: dois zips com o mesmo nome de arquivo nao
        // se sobrescrevem, nem a copia de uma fonte ainda aberta.
        self.openings += 1;
        let cache_dir = self.cache_dir.join(self.openings.to_string());
        Opening { name: name.to_string(), file_path, mode, cache_dir, vault: self.vault.clone() }
    }

    /// Marca `name` como ausente (pedido de nova localizacao) e devolve o erro.
    pub(super) fn absent(&mut self, name: &str, file_path: Option<PathBuf>, reason: Reason) -> SourceError {
        self.pending = Some(Pending::fresh(name, file_path, reason));
        SourceError::AbsentSource { name: name.to_string() }
    }

    /// A gaveta de fontes mostrou `name` sem caminho valido: pede a nova localizacao.
    pub fn mark_absent(&mut self, name: &str, file_path: Option<PathBuf>, no_path: bool) -> SourceError {
        let reason = if no_path { Reason::NoPath } else { Reason::NotFound };
        self.absent(name, file_path, reason)
    }

    /// Clique numa fonte da gaveta: o caminho a abrir. Sem caminho valido a
    /// fonte vira pendencia ("Apontar nova localizacao...") e o aviso diz por que.
    pub fn source_line_path(&mut self, l: &drawers::LineSource) -> Option<PathBuf> {
        match (l.status, &l.file_path) {
            (drawers::DrawerSourceStatus::Ok, Some(c)) => Some(c.clone()),
            (status, c) => {
                let e = self.mark_absent(&l.name, c.clone(), status == drawers::DrawerSourceStatus::NoPath);
                self.warning = Some(e.to_string());
                None
            }
        }
    }

    /// O pedido de nova localizacao (ou de tentar de novo) em aberto, se houver.
    pub fn pending(&self) -> Option<&Pending> {
        self.pending.as_ref()
    }

    /// Fecha o aviso sem resolver (a fonte continua como estava).
    pub fn dismiss_pending(&mut self) {
        self.pending = None;
    }

    /// A fonte aberta esta indisponivel (erro de E/S no meio da sessao).
    pub(super) fn source_open_unavailable(&self) -> bool {
        self.source_has()
            && self
                .pending
                .as_ref()
                .is_some_and(|p| p.reason == Reason::Unavailable && p.name == self.current.image.source)
    }

    /// Passa a usar `source` (com o nome `name`) e abre o primeiro quadro.
    #[cfg_attr(not(test), allow(dead_code))] // fluxo inteiro numa chamada: testes e referencia
    pub fn adopt_source(&mut self, name: &str, source: Arc<dyn ImageSource>) -> Option<Request> {
        self.adopt_source_with(name, source, None, None)
    }

    /// [`AppState::adopt_source_in_the`] com a fonte e os grifos lidos no fundo
    /// ([`Opening::execute_ready`]): nada do vault e lido aqui.
    pub fn adopt_source_ready(&mut self, name: &str, p: Ready, start: Option<Frame>) -> Option<Request> {
        self.adopt_source_with(name, p.source, p.highlights, start)
    }

    pub(super) fn adopt_source_with(
        &mut self,
        name: &str,
        source: Arc<dyn ImageSource>,
        highlights: Option<Result<HighlightsFile, String>>,
        start: Option<Frame>,
    ) -> Option<Request> {
        if let Some(f) = start.filter(|f| f.image.source == name && !f.image.file.is_empty()) {
            self.prepare_source(name, source, highlights);
            return self.ir_to(f);
        }
        let first = source.entries().first().map(|e| e.path.clone());
        self.prepare_source(name, source, highlights);
        match first {
            Some(file) => self.ir_to(Frame::new(name, &file, 1)),
            None => {
                self.current = Frame::new(name, "", 1);
                self.clear_frame();
                self.error = Some(t!("error.source.no_images", name = format!("{name:?}")));
                None
            }
        }
    }

    /// Passa a usar `source`: prefetcher novo, grifos da fonte, sem pendencias.
    /// `highlights`: lidos no fundo; `None` le agora (testes, sem vault).
    pub(super) fn prepare_source(&mut self, name: &str, source: Arc<dyn ImageSource>, highlights: Option<Result<HighlightsFile, String>>) {
        // A posicao na fonte que sai fica guardada.
        self.save_position();
        if let Some(v) = &self.vault {
            if self.app_data.mark_recent(&v.root, name) {
                self.dirty_position = true;
            }
        }
        // O antigo e solto fora desta thread (`PrefetcherShared`).
        self.prefetcher = Some(PrefetcherShared::new(Prefetcher::new(Arc::clone(&source), AHEAD)));
        self.source = Some(source);
        self.warning = None;
        self.annot.image_drop();
        // Uma fonte abriu: o aviso de fonte ausente/indisponivel perdeu o contexto.
        self.pending = None;
        match highlights {
            Some(g) => self.adopt_highlights(name, g),
            None => self.load_highlights(name),
        }
    }

    pub(super) fn clear_frame(&mut self) {
        self.page = None;
        self.view_window = None;
        self.clear_layers();
        self.total_pages = None;
        self.loading = false;
        self.frame_pending = None;
        self.nav_pending = None;
        self.error = None;
        self.displayed = false;
        self.adjustments = Adjustments::default();
        self.rotation = Rotation::R0;
        self.new_generation();
    }

    /// Posicao lembrada da fonte `name` neste vault, se o arquivo ainda esta em `source`.
    pub(super) fn remembered_position(&self, name: &str, source: &dyn ImageSource) -> Option<Frame> {
        let (file, page) = self.app_data.position(&self.vault.as_ref()?.root, name)?;
        source.entries().iter().any(|e| e.path == file).then(|| Frame::new(name, &file, page.max(1)))
    }

    /// Troca o quadro atual e pede a carga. `f` precisa ser da fonte aberta.
    pub(super) fn ir_to(&mut self, f: Frame) -> Option<Request> {
        let prefetcher = self.prefetcher.clone()?;
        let frame = match &f.framing {
            Some(e) => Reframe::Adopt(e.clone()),
            None => Reframe::Adjust,
        };
        let target = f.target();
        self.fr = viewer_math::clamp(f.framing.clone().unwrap_or_else(viewer_math::default_framing));
        self.annot.go_to(&f.image);
        if self.note_open.as_ref().is_some_and(|(_, img)| *img != f.image) {
            self.note_open = None;
        }
        self.current = f;
        if let Some(v) = &self.vault {
            let i = &self.current.image;
            if self.app_data.position(&v.root, &i.source) != Some((i.file.clone(), i.page)) {
                self.app_data.set_position(&v.root, &i.source, &i.file, i.page);
                self.dirty_position = true;
            }
        }
        self.clear_frame();
        self.adjustments = self.app_data.adjustments(&self.current.image);
        self.rotation = self.app_data.rotation(&self.current.image);
        self.loading = true;
        self.frame_pending = Some(frame.clone());
        prefetcher.hint(&target);
        let image_adjustments = self.in_screen(&self.adjustments);
        // Com ajuste: a BASE (pagina inteira reduzida: rapida, cobre a pagina toda
        // com o valor certo). O tile sai quando o visualizador informar a janela,
        // na grade da tela (nunca um tile da pagina inteira na resolucao nativa,
        // que custaria uma copia cheia da pagina mais o recorte guardado).
        let quality = if image_adjustments.is_neutral() { Quality::Full } else { Quality::Base };
        if quality == Quality::Base {
            // O tile dos mesmos valores reusa esta geracao (nao derruba a base em voo).
            self.request_adjustment = Some((self.generation.current(), image_adjustments.clone()));
        }
        Some(Request {
            generation: self.generation.current(),
            target,
            decoded: None,
            quality,
            image_adjustments,
            view_window: None,
            frame,
            rotation: self.rotation,
            prefetcher,
            effective: self.generation.effective(),
        })
    }

    /// Abre `f` (da fonte aberta). Com `f.framing`, a imagem abre nele;
    /// sem, ajustada a janela.
    pub fn open_frame(&mut self, f: Frame) -> Option<Request> {
        if !self.source_has() || f.image.source != self.current.image.source {
            // Notas de outra fonte abrem por `open_note` (resolve e abre a fonte).
            self.warning = Some(t!("warning.source_not_open", source = format!("{:?}", f.image.source)));
            return None;
        }
        self.ir_to(f)
    }

    /// Proximo quadro (pagina seguinte ou proximo arquivo). `None` no fim.
    /// Enquanto o quadro atual carrega sem o total de paginas conhecido, nao
    /// navega (pularia paginas de um TIFF/PDF): guarda a intencao e devolve
    /// `None`; a view chama `resume_navigation` depois de `receive`. Com erro
    /// no quadro, navega na hora.
    pub fn go_next(&mut self) -> Option<Request> {
        self.navigate(Direction::Next)
    }

    /// Quadro anterior. `None` no inicio.
    pub fn go_prev(&mut self) -> Option<Request> {
        self.navigate(Direction::Previous)
    }

    pub(super) fn navigate(&mut self, d: Direction) -> Option<Request> {
        if self.source_open_unavailable() {
            // Fonte fora do ar: setas nao empilham leituras presas no NAS.
            self.warning = Some(t!("warning.unavailable_source", source = format!("{:?}", self.current.image.source)));
            self.nav_pending = None;
            return None;
        }
        let prefetcher = self.prefetcher.as_ref()?;
        if self.loading && self.total_pages.is_none() && self.error.is_none() {
            self.nav_pending = Some(d);
            return None;
        }
        let current = self.current.target();
        let n = match d {
            Direction::Next => prefetcher.next_frame(&current),
            Direction::Previous => prefetcher.prev_frame(&current),
        }?;
        self.warning = None;
        self.ir_to(Frame::new(&self.current.image.source.clone(), &n.path, n.page))
    }

    /// Ha um go_next/go_prev esperando o quadro atual chegar.
    pub fn has_navigation_pending(&self) -> bool {
        self.nav_pending.is_some()
    }

    /// Depois de `receive`: executa a navegacao pendente, se houver.
    pub fn resume_navigation(&mut self) -> Option<Request> {
        if self.loading {
            return None;
        }
        let d = self.nav_pending.take()?;
        self.navigate(d)
    }

    /// Caminho conhecido da fonte `name` (sessao, senao dados do app). Nao toca o disco.
    pub(super) fn path_of(&self, name: &str) -> Option<PathBuf> {
        self.session_paths
            .get(name)
            .cloned()
            .or_else(|| self.vault.as_ref().and_then(|v| self.app_data.source_path(&v.root, name)))
    }

    /// Como [`AppState::adopt_source`], mas abre em `start` (de `name`) em vez
    /// do primeiro quadro.
    #[cfg_attr(not(test), allow(dead_code))] // fluxo inteiro numa chamada: testes e referencia
    pub fn adopt_source_in_the(&mut self, name: &str, source: Arc<dyn ImageSource>, start: Option<Frame>) -> Option<Request> {
        self.adopt_source_with(name, source, None, start)
    }

    /// Entra com uma abertura que voltou do fundo (Cmd-O / gaveta de fontes:
    /// `open_source::open_in_background`; nota ou tarefa de outra fonte, "Tentar de
    /// novo": `Opening::execute_ready`). Descarta tudo se o vault mudou desde
    /// o pedido. Caminho que sumiu vira `AbsentSource` com o pedido de nova
    /// localizacao (e o quadro `start`, para abrir la depois). Senao usa a
    /// fonte (em `start`, ou no primeiro quadro), lembra o caminho e grava o
    /// registro nos dados do app.
    pub fn enter_source(
        &mut self,
        a: &Opening,
        r: Result<open_source::Open, OpeningFailure>,
        start: Option<Frame>,
        highlight: Option<String>,
    ) -> Input {
        if !self.vault_same(a.vault.as_ref()) {
            std::thread::spawn(move || drop(r));
            return Input::Stale;
        }
        let start = start.filter(|f| f.image.source == a.name && !f.image.file.is_empty());
        let open = match r {
            Ok(x) => x,
            Err(OpeningFailure::Absent) => {
                let e = self.absent(&a.name, Some(a.file_path.clone()), Reason::NotFound);
                if let Some(p) = self.pending.as_mut() {
                    p.note = self.note_open.as_ref().filter(|(_, i)| i.source == a.name).map(|(id, _)| id.clone());
                    p.image_frame = start;
                    p.highlight = highlight;
                }
                self.warning = Some(e.to_string());
                return Input::Failed(e);
            }
            Err(OpeningFailure::Other(m)) => {
                self.warning = Some(m.clone());
                return Input::Failed(SourceError::Other(m));
            }
        };
        let with_start = start.is_some();
        // Sem quadro pedido: volta a ultima posicao aberta nesta fonte (se o arquivo ainda existe).
        let start = start.or_else(|| self.remembered_position(&a.name, open.ready.source.as_ref()));
        let request = self.adopt_source_ready(&a.name, open.ready, start);
        if with_start {
            self.highlight(highlight);
        }
        if request.is_some() {
            self.remember_path(&a.name, a.file_path.clone());
        }
        let mut fresh = false;
        match (open.record, &a.vault) {
            (Some(Ok(record)), Some(v)) => {
                let root_dir = v.root.clone();
                if let Err(e) = self.write_path(&root_dir, &a.name, &a.file_path) {
                    self.warning = Some(e);
                }
                fresh = record == open_source::Record::New;
            }
            (Some(Err(e)), _) => self.warning = Some(e),
            _ => {}
        }
        Input::Open { request, fresh }
    }

    /// Abre a fonte `name` de uma vez (resolver, abrir, entrar). BLOQUEIA: a
    /// interface faz o mesmo com o meio no fundo. Caminho que nao existe ou sem
    /// caminho: nada abre e devolve [`SourceError::AbsentSource`].
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn open_source_now(&mut self, name: &str, mode: ZipMode) -> Result<Option<Request>, SourceError> {
        let a = self.open_source(name, mode)?;
        let r = a.execute_ready().map(|ready| open_source::Open { ready, record: None });
        match self.enter_source(&a, r, None, None) {
            Input::Open { request, .. } => Ok(request),
            Input::Failed(e) => Err(e),
            Input::Stale => Err(SourceError::Other(t!("error.vault_changed"))),
        }
    }

    /// "Tentar de novo": reabre o quadro que falhou (fonte indisponivel) ou
    /// resolve de novo a fonte ausente (o NAS pode ter sido montado). Com uma
    /// tentativa ainda carregando, recusa (nada se acumula num NAS preso).
    pub fn new_attempt(&mut self) -> Result<OpenNote, SourceError> {
        let Some(p) = self.pending.clone() else {
            return Err(SourceError::Other(t!("error.nothing_to_attempt")));
        };
        if self.loading {
            return Err(SourceError::Other(t!("error.still_tentando")));
        }
        let image_frame = p.image_frame.clone();
        if p.reason == Reason::Unavailable && self.source_has() && self.current.image.source == p.name {
            let f = image_frame.unwrap_or_else(|| self.current.clone());
            // A pendencia fica ate a imagem chegar (`receive`).
            let request = self.ir_to(f);
            self.annot.emphasis = p.highlight.clone();
            return Ok(OpenNote::ImageFrame(request));
        }
        self.pending = None;
        match image_frame {
            Some(q) => self.open_point(q.image, q.framing, p.highlight, p.note),
            None => {
                let opening = self.open_source(&p.name, ZipMode::Direct)?;
                Ok(OpenNote::Source { opening, image_frame: Frame::new(&p.name, "", 1), highlight: None })
            }
        }
    }

    /// Caminho da fonte aberta nesta maquina (o smoke reabre depois de fechar).
    pub fn open_source_path(&self) -> Option<PathBuf> {
        self.source_has().then(|| self.path_of(&self.current.image.source)).flatten()
    }

    /// "Arquivo > Fechar fonte": guarda a posicao (reabrir volta a ela) e fecha
    /// a fonte aberta. Devolve o nome dela; sem fonte, `None` e nada muda. A
    /// nota em edicao termina na view (`QuickNoteSession::observe` percebe a troca).
    pub fn close_source_open(&mut self) -> Option<String> {
        if !self.source_has() {
            return None;
        }
        let name = self.current.image.source.clone();
        self.save_position();
        self.close_source();
        self.pending = None;
        Some(name)
    }

    /// "Arquivo > Fechar fonte" pelo usuario: como [`AppState::close_source_open`],
    /// devolvendo a confirmacao da barra; sem fonte, `None` e o aviso diz por que.
    pub fn close_source_requested(&mut self) -> Option<String> {
        match self.close_source_open() {
            Some(name) => Some(t!("confirmation.closed_source", name)),
            None => {
                self.warning = Some(t!("warning.no_source_open"));
                None
            }
        }
    }

    /// O que a barra de status mostra (vault, fonte, pagina, imagem, arquivo).
    pub fn status_info(&self) -> crate::layout::StatusInfo {
        let vault = self.vault.as_ref().map(|v| crate::layout::folder_name(&v.root));
        if !self.source_has() {
            return crate::layout::StatusInfo { vault, ..Default::default() };
        }
        let (page, total_pages) = self.page();
        let (index, total_images) = self.index().map_or((None, None), |(i, n)| (Some(i), Some(n)));
        crate::layout::StatusInfo {
            vault,
            source: Some(self.current.image.source.clone()),
            page: Some(page),
            total_pages,
            index,
            total_images,
            file: Some(self.current.image.file.clone()),
        }
    }

    /// As mensagens passageiras da barra de status, na ordem em que aparecem.
    /// `opening`: uma abertura de fonte no fundo; `quick_note_loading`: o texto
    /// pedido para a caixa ainda nao chegou (a caixa esta desligada);
    /// `exclusive`: o trabalho exclusivo em andamento (troca de vault etc.).
    pub fn status_messages(&self, opening: bool, quick_note_loading: bool, exclusive: Option<&str>) -> Vec<crate::layout::StatusMessage> {
        use crate::layout::StatusMessage as M;
        let mut v = Vec::new();
        if opening {
            v.push(M::Progress(t!("status.source_opening")));
        } else if self.source_has() && self.loading() {
            v.push(M::Progress(t!("status.loading")));
        }
        if quick_note_loading {
            v.push(M::Progress(t!("status.quick_note_loading")));
        }
        if let Some(o) = exclusive {
            v.push(M::Progress(o.to_string()));
        }
        if let Some(c) = &self.confirmation {
            v.push(M::Confirmation(c.clone()));
        }
        if let Some(a) = &self.warning {
            v.push(M::Warning(a.clone()));
        }
        v
    }

    /// A seta andou: saiu um pedido de quadro ou ha navegacao esperando a pagina
    /// que carrega (`request`: o que `go_next`/`go_prev` devolveu).
    pub fn moved(&self, request: &Option<Request>) -> bool {
        request.is_some() || self.has_navigation_pending()
    }

    /// Fecha a fonte aberta (troca de vault): sem quadro, sem grifos, sem pendencias.
    pub(super) fn close_source(&mut self) {
        self.prefetcher = None;
        self.source = None;
        self.current = Frame::new("", "", 1);
        self.clear_frame();
        self.annot.zero();
        self.note_open = None;
    }
}

/// O que o "Ir para" conta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitIr {
    Image,
    Page,
}

/// O numero digitado no "Ir para": so digitos (com `-` ou `+` na frente). Numero enorme
/// vale o maior possivel (vai para o ultimo); o que nao e numero, `None`.
pub fn interpret_go_to(text: &str) -> Option<i64> {
    let t = text.trim();
    let digits = t.strip_prefix(['-', '+']).unwrap_or(t);
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(t.parse::<i64>().unwrap_or(if t.starts_with('-') { i64::MIN } else { i64::MAX }))
}

/// O alvo (1-based) de um "Ir para n" numa fonte de `total` itens: abaixo de 1, o primeiro;
/// acima do total, o ultimo.
pub fn target_go_to(n: i64, total: usize) -> usize {
    n.clamp(1, total.max(1) as i64) as usize
}
