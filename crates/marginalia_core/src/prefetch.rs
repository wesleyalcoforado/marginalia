//! Pre-carregamento: decodifica em segundo plano os proximos quadros enquanto
//! o usuario le o atual, para que avancar nao trave (a fonte costuma ser um NAS).
//!
//! Todo o estado compartilhado (fila, cache, em-andamento) fica atras de UM
//! unico `Mutex`, que nunca e segurado durante leitura da fonte nem decodificacao.

use std::collections::{HashMap, HashSet, VecDeque};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;

use crate::decode::{decode_page, DecodeError, DecodedPage};
use crate::source::ImageSource;

/// Threads de fundo. Mais que isso so brigaria pelo NAS e pelo PDFium (que e
/// serializado internamente).
const WORKERS: usize = 2;

/// Teto de memoria dos quadros decodificados (RGBA, `w*h*4`). Um scan de
/// 200 MP decodifica para 800 MB: sem teto, `ahead*2+2` quadros desses
/// esgotariam a RAM.
pub const BUDGET_BYTES: u64 = 1 << 30;

/// Maior arquivo cujos bytes lidos ficam guardados para as outras paginas.
const LIMIT_BYTES_READ: usize = 256 << 20;

/// Uma pagina de um arquivo da fonte (`page` e 1-based).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Frame {
    pub path: String,
    pub page: u32,
}

#[derive(Clone)]
enum Cached {
    Ok(Arc<DecodedPage>),
    Err(DecodeError),
}

impl Cached {
    fn to_result(&self) -> Result<Arc<DecodedPage>, DecodeError> {
        match self {
            Cached::Ok(p) => Ok(Arc::clone(p)),
            Cached::Err(e) => Err(clone_err(e)),
        }
    }
}

// `DecodeError` nao implementa `Clone`; erros ficam em cache e precisam ser devolvidos varias vezes.
impl Clone for DecodeError {
    fn clone(&self) -> Self {
        clone_err(self)
    }
}

fn clone_err(e: &DecodeError) -> DecodeError {
    match e {
        DecodeError::Invalid(s) => DecodeError::Invalid(s.clone()),
        DecodeError::MissingPage { requested, total } => {
            DecodeError::MissingPage { requested: *requested, total: *total }
        }
        DecodeError::NotSupported(s) => DecodeError::NotSupported(s.clone()),
        DecodeError::PdfiumUnavailable(s) => DecodeError::PdfiumUnavailable(s.clone()),
        DecodeError::Reading(s) => DecodeError::Reading(s.clone()),
    }
}

#[derive(Default)]
struct State {
    cache: HashMap<Frame, (Cached, u64)>,
    /// Soma de `bytes_of` do que esta em `cache`.
    bytes: u64,
    /// Tamanho do ultimo quadro decodificado (estimativa para a janela do `hint`).
    last_size: u64,
    tick: u64,
    in_flight: HashSet<Frame>,
    queue: VecDeque<Frame>,
    page_counts: HashMap<String, u32>,
    current: Option<Frame>,
    shutdown: bool,
    /// `get`s bloqueados esperando o job em andamento de cada quadro.
    waiters: HashMap<Frame, usize>,
    /// Erro de LEITURA entregue uma vez a cada espera; (erro, esperas restantes).
    read_failures: HashMap<Frame, (DecodeError, usize)>,
}

/// Resultado de um job. Erro de leitura da fonte e transitorio (o NAS pode voltar)
/// e nunca fica em cache; erro de decodificacao e deterministico e fica.
enum Outcome {
    Done(Cached),
    ReadFailed(DecodeError),
}

struct Shared {
    source: Arc<dyn ImageSource>,
    index: HashMap<String, usize>,
    ahead: usize,
    budget: u64,
    /// Bytes do ultimo arquivo de varias paginas (PDF/TIFF) lido da fonte: as
    /// paginas seguintes decodificam deles sem reler o arquivo inteiro do NAS.
    /// Mutex proprio, nunca segurado durante leitura ou decodificacao.
    read_log: Mutex<Option<(String, Arc<Vec<u8>>)>>,
    state: Mutex<State>,
    cv: Condvar,
}

pub struct Prefetcher {
    shared: Arc<Shared>,
    workers: Vec<JoinHandle<()>>,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn next_in(&self, st: &State, f: &Frame) -> Option<Frame> {
        let total = st.page_counts.get(&f.path).copied().unwrap_or(1);
        if f.page < total {
            return Some(Frame { path: f.path.clone(), page: f.page + 1 });
        }
        let i = *self.index.get(&f.path)?;
        let e = self.source.entries().get(i + 1)?;
        Some(Frame { path: e.path.clone(), page: 1 })
    }

    fn prev_in(&self, st: &State, f: &Frame) -> Option<Frame> {
        let i = *self.index.get(&f.path)?;
        if f.page > 1 {
            return Some(Frame { path: f.path.clone(), page: f.page - 1 });
        }
        let e = self.source.entries().get(i.checked_sub(1)?)?;
        let page = st.page_counts.get(&e.path).copied().unwrap_or(1);
        Some(Frame { path: e.path.clone(), page })
    }

    /// Os proximos `n` quadros depois de `current`.
    fn window_next(&self, st: &State, current: &Frame, n: usize) -> Vec<Frame> {
        let mut out = Vec::new();
        let mut f = current.clone();
        while out.len() < n {
            match self.next_in(st, &f) {
                Some(n) => {
                    out.push(n.clone());
                    f = n;
                }
                None => break,
            }
        }
        out
    }

    /// Quadros que o despejo nao pode tirar do cache: o atual e `ahead` para cada lado.
    fn protected(&self, st: &State) -> HashSet<Frame> {
        let mut set = HashSet::new();
        if let Some(cur) = &st.current {
            set.insert(cur.clone());
            set.extend(self.window_next(st, cur, self.ahead));
            let mut f = cur.clone();
            for _ in 0..self.ahead {
                match self.prev_in(st, &f) {
                    Some(p) => {
                        set.insert(p.clone());
                        f = p;
                    }
                    None => break,
                }
            }
        }
        set
    }

    /// Le e decodifica, sem segurar o lock. Panico vira erro deste quadro.
    fn run_job(&self, frame: &Frame) -> Outcome {
        let r = catch_unwind(AssertUnwindSafe(|| {
            let bytes = match self.read_file(&frame.path) {
                Ok(b) => b,
                Err(e) => return Outcome::ReadFailed(reading_error(e)),
            };
            match decode_page(&frame.path, &bytes, frame.page) {
                Ok(p) => {
                    self.store_read(&frame.path, bytes, p.page_count);
                    Outcome::Done(Cached::Ok(Arc::new(p)))
                }
                Err(e) => {
                    self.store_read(&frame.path, bytes, 0);
                    Outcome::Done(Cached::Err(e))
                }
            }
        }));
        r.unwrap_or_else(|_| {
            Outcome::Done(Cached::Err(DecodeError::Invalid(texts::t!("error.decode.failure_interna"))))
        })
    }

    fn read_log(&self) -> MutexGuard<'_, Option<(String, Arc<Vec<u8>>)>> {
        self.read_log.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Bytes de `path`: os guardados, se forem deste arquivo, senao le da fonte.
    /// Erro de leitura nunca fica guardado.
    fn read_file(&self, path: &str) -> crate::vault::Result<Arc<Vec<u8>>> {
        if let Some((p, b)) = self.read_log().as_ref() {
            if p == path {
                return Ok(Arc::clone(b));
            }
        }
        self.source.read(path).map(Arc::new)
    }

    /// Guarda os bytes so de arquivo com mais de uma pagina e ate o limite; um
    /// de pagina unica (ou com erro) tira o que estava guardado para ele.
    fn store_read(&self, path: &str, bytes: Arc<Vec<u8>>, pages: u32) {
        let mut g = self.read_log();
        if pages > 1 && bytes.len() <= LIMIT_BYTES_READ {
            *g = Some((path.to_string(), bytes));
        } else if g.as_ref().is_some_and(|(p, _)| p == path) {
            *g = None;
        }
    }

    /// Guarda o resultado, tira o quadro de "em andamento", despeja o excesso e acorda quem espera.
    fn finish(&self, frame: &Frame, out: Outcome) -> Result<Arc<DecodedPage>, DecodeError> {
        let mut st = self.lock();
        let res = match out {
            Outcome::ReadFailed(e) => {
                st.in_flight.remove(frame);
                let n = st.waiters.get(frame).copied().unwrap_or(0);
                if n > 0 {
                    let ent = st.read_failures.entry(frame.clone()).or_insert((clone_err(&e), 0));
                    ent.0 = clone_err(&e);
                    ent.1 += n;
                }
                drop(st);
                self.cv.notify_all();
                return Err(e);
            }
            Outcome::Done(c) => c,
        };
        if let Cached::Ok(p) = &res {
            st.page_counts.insert(frame.path.clone(), p.page_count);
        }
        st.tick += 1;
        let t = st.tick;
        let new = bytes_of(&res);
        if new > 0 {
            st.last_size = new;
        }
        if let Some((old, _)) = st.cache.insert(frame.clone(), (res.clone(), t)) {
            st.bytes -= bytes_of(&old);
        }
        st.bytes += new;
        st.in_flight.remove(frame);
        let cap = self.ahead * 2 + 2;
        if st.cache.len() > cap {
            let prot = self.protected(&st);
            while st.cache.len() > cap {
                if !Self::dump_lru(&mut st, |k| k != frame && !prot.contains(k)) {
                    break;
                }
            }
        }
        // Orcamento de bytes: so o atual e o recem-terminado sao poupados.
        while st.bytes > self.budget {
            let current = st.current.clone();
            if !Self::dump_lru(&mut st, |k| k != frame && Some(k) != current.as_ref()) {
                break;
            }
        }
        drop(st);
        self.cv.notify_all();
        res.to_result()
    }
}

fn bytes_of(c: &Cached) -> u64 {
    match c {
        Cached::Ok(p) => p.image.as_raw().len() as u64,
        Cached::Err(_) => 0,
    }
}

impl Shared {
    /// Tira do cache o quadro menos usado entre os que `can` aceita; `false` se nenhum.
    fn dump_lru(st: &mut State, can: impl Fn(&Frame) -> bool) -> bool {
        let victim = st.cache.iter().filter(|(k, _)| can(k)).min_by_key(|(_, (_, t))| *t).map(|(k, _)| k.clone());
        match victim.and_then(|v| st.cache.remove(&v)) {
            Some((c, _)) => {
                st.bytes -= bytes_of(&c);
                true
            }
            None => false,
        }
    }

    /// Quantos quadros pre-carregar a frente de `current`: `ahead`, encolhido
    /// para que `(n+1) * tamanho do atual` caiba no orcamento (minimo 1), senao
    /// o despejo tiraria o que acabou de ser decodificado (decodificar/despejar
    /// em ciclo).
    fn effective_window(&self, st: &State, current: &Frame) -> usize {
        let sz = match st.cache.get(current) {
            Some((c, _)) if bytes_of(c) > 0 => bytes_of(c),
            _ => st.last_size,
        };
        if sz == 0 {
            return self.ahead;
        }
        let fit = (self.budget / sz) as usize;
        fit.saturating_sub(1).clamp(1, self.ahead)
    }
}

/// Erro de E/S (inclusive dentro do zip) vira `Reading`: a fonte sumiu ou o NAS
/// caiu, e a interface oferece tentar de novo / nova localizacao. O resto (entrada
/// que nao existe na fonte, zip invalido) continua `Invalid`.
fn reading_error(e: crate::Error) -> DecodeError {
    match e {
        crate::Error::Io(io) | crate::Error::Zip(zip::result::ZipError::Io(io)) => DecodeError::Reading(io.to_string()),
        other => DecodeError::Invalid(other.to_string()),
    }
}

fn worker(shared: Arc<Shared>) {
    loop {
        let frame = {
            let mut st = shared.lock();
            loop {
                if st.shutdown {
                    return;
                }
                match st.queue.pop_front() {
                    Some(f) => {
                        if st.cache.contains_key(&f) || st.in_flight.contains(&f) {
                            continue;
                        }
                        st.in_flight.insert(f.clone());
                        break f;
                    }
                    None => {
                        st = shared.cv.wait(st).unwrap_or_else(|e| e.into_inner());
                    }
                }
            }
        };
        let res = shared.run_job(&frame);
        let _ = shared.finish(&frame, res);
    }
}

impl Prefetcher {
    /// `ahead` e ajustado para 3..=5.
    pub fn new(source: Arc<dyn ImageSource>, ahead: usize) -> Prefetcher {
        Self::with_budget(source, ahead, BUDGET_BYTES)
    }

    /// Como `new`, com outro teto de bytes decodificados em cache.
    pub fn with_budget(source: Arc<dyn ImageSource>, ahead: usize, budget: u64) -> Prefetcher {
        let index = source
            .entries()
            .iter()
            .enumerate()
            .map(|(i, e)| (e.path.clone(), i))
            .collect();
        let shared = Arc::new(Shared {
            source,
            index,
            ahead: ahead.clamp(3, 5),
            budget,
            read_log: Mutex::new(None),
            state: Mutex::new(State::default()),
            cv: Condvar::new(),
        });
        let workers = (0..WORKERS)
            .map(|_| {
                let s = Arc::clone(&shared);
                std::thread::spawn(move || worker(s))
            })
            .collect();
        Prefetcher { shared, workers }
    }

    pub fn ahead(&self) -> usize {
        self.shared.ahead
    }

    pub fn cached_len(&self) -> usize {
        self.shared.lock().cache.len()
    }

    /// Bytes dos quadros decodificados em cache.
    pub fn cached_bytes(&self) -> u64 {
        self.shared.lock().bytes
    }

    /// Bloqueia ate o quadro estar decodificado: espera um job em andamento ou
    /// decodifica na propria thread.
    pub fn get(&self, frame: &Frame) -> Result<Arc<DecodedPage>, DecodeError> {
        let sh = &self.shared;
        let mut st = sh.lock();
        let mut waited = false;
        loop {
            if waited {
                if let Some((e, left)) = st.read_failures.get_mut(frame) {
                    let err = clone_err(e);
                    *left -= 1;
                    if *left == 0 {
                        st.read_failures.remove(frame);
                    }
                    Self::stop_waiting(&mut st, frame);
                    return Err(err);
                }
            }
            if st.cache.contains_key(frame) {
                if waited {
                    Self::stop_waiting(&mut st, frame);
                }
                st.tick += 1;
                let t = st.tick;
                let (c, last) = st.cache.get_mut(frame).expect("presente");
                *last = t;
                return c.to_result();
            }
            if st.in_flight.contains(frame) {
                if !waited {
                    waited = true;
                    *st.waiters.entry(frame.clone()).or_insert(0) += 1;
                }
                st = sh.cv.wait(st).unwrap_or_else(|e| e.into_inner());
                continue;
            }
            break;
        }
        if waited {
            Self::stop_waiting(&mut st, frame);
        }
        st.in_flight.insert(frame.clone());
        st.queue.retain(|f| f != frame);
        drop(st);
        let res = sh.run_job(frame);
        sh.finish(frame, res)
    }

    fn stop_waiting(st: &mut State, frame: &Frame) {
        if let Some(n) = st.waiters.get_mut(frame) {
            *n -= 1;
            if *n == 0 {
                st.waiters.remove(frame);
            }
        }
    }

    /// Nao bloqueia: agenda os proximos `ahead` quadros e descarta da fila o
    /// que saiu da janela.
    pub fn hint(&self, current: &Frame) {
        let sh = &self.shared;
        let mut st = sh.lock();
        st.current = Some(current.clone());
        let n = sh.effective_window(&st, current);
        let win = sh.window_next(&st, current, n);
        st.queue.retain(|f| win.contains(f));
        for f in win {
            if !st.cache.contains_key(&f) && !st.in_flight.contains(&f) && !st.queue.contains(&f) {
                st.queue.push_back(f);
            }
        }
        drop(st);
        sh.cv.notify_all();
    }

    pub fn next_frame(&self, current: &Frame) -> Option<Frame> {
        let st = self.shared.lock();
        self.shared.next_in(&st, current)
    }

    pub fn prev_frame(&self, current: &Frame) -> Option<Frame> {
        let st = self.shared.lock();
        self.shared.prev_in(&st, current)
    }
}

impl Drop for Prefetcher {
    fn drop(&mut self) {
        {
            let mut st = self.shared.lock();
            st.shutdown = true;
            st.queue.clear();
        }
        self.shared.cv.notify_all();
        for h in self.workers.drain(..) {
            let _ = h.join();
        }
    }
}
