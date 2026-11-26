use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use marginalia_core::decode::DecodeError;
use marginalia_core::prefetch::{Frame, Prefetcher};
use marginalia_core::source::{ImageEntry, ImageSource};
use marginalia_core::Error;

const TIF: &[u8] = include_bytes!("fixtures/duas.tif");

struct Fake {
    entries: Vec<ImageEntry>,
    reads: Mutex<HashMap<String, usize>>,
    delay: Duration,
    fail: HashSet<String>,
    png: Vec<u8>,
    total: AtomicUsize,
    fail_n: Mutex<HashMap<String, usize>>,
    corrupt: Mutex<HashSet<String>>,
    /// Leitura falha com erro de E/S (NAS desmontado), ate ser tirado daqui.
    io_fail: Mutex<HashSet<String>>,
}

impl Fake {
    fn new(paths: &[&str], delay_ms: u64, fail: &[&str]) -> Arc<Fake> {
        let mut png = Vec::new();
        image::RgbaImage::new(2, 2)
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        Arc::new(Fake {
            entries: paths.iter().map(|p| ImageEntry { path: p.to_string() }).collect(),
            reads: Mutex::new(HashMap::new()),
            delay: Duration::from_millis(delay_ms),
            fail: fail.iter().map(|s| s.to_string()).collect(),
            png,
            total: AtomicUsize::new(0),
            fail_n: Mutex::new(HashMap::new()),
            corrupt: Mutex::new(HashSet::new()),
            io_fail: Mutex::new(HashSet::new()),
        })
    }
    fn fail_times(&self, p: &str, n: usize) {
        self.fail_n.lock().unwrap().insert(p.to_string(), n);
    }
    fn count(&self, p: &str) -> usize {
        *self.reads.lock().unwrap().get(p).unwrap_or(&0)
    }
}

impl ImageSource for Fake {
    fn entries(&self) -> &[ImageEntry] {
        &self.entries
    }
    fn read(&self, path: &str) -> marginalia_core::vault::Result<Vec<u8>> {
        *self.reads.lock().unwrap().entry(path.to_string()).or_insert(0) += 1;
        self.total.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(self.delay);
        if let Some(n) = self.fail_n.lock().unwrap().get_mut(path) {
            if *n > 0 {
                *n -= 1;
                return Err(Error::InputNotFound(path.to_string()));
            }
        }
        if self.io_fail.lock().unwrap().contains(path) {
            return Err(Error::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "NAS desmontado")));
        }
        if self.corrupt.lock().unwrap().contains(path) {
            return Ok(b"lixo".to_vec());
        }
        if self.fail.contains(path) {
            return Err(Error::InputNotFound(path.to_string()));
        }
        Ok(if path.ends_with(".tif") { TIF.to_vec() } else { self.png.clone() })
    }
}

fn names(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("{}.png", (b'a' + i as u8) as char)).collect()
}

fn fake_n(n: usize, delay_ms: u64, fail: &[&str]) -> Arc<Fake> {
    let v = names(n);
    let refs: Vec<&str> = v.iter().map(|s| s.as_str()).collect();
    Fake::new(&refs, delay_ms, fail)
}

fn fr(path: &str, page: u32) -> Frame {
    Frame { path: path.to_string(), page }
}

fn wait_until(mut cond: impl FnMut() -> bool) -> bool {
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(5) {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

#[test]
fn ahead_and_adjusted() {
    let f = fake_n(3, 0, &[]);
    assert_eq!(Prefetcher::new(f.clone(), 10).ahead(), 5);
    assert_eq!(Prefetcher::new(f.clone(), 0).ahead(), 3);
    assert_eq!(Prefetcher::new(f, 4).ahead(), 4);
}

#[test]
fn hint_le_so_the_window_and_get_not_le_of_new() {
    let f = fake_n(10, 0, &[]);
    let p = Prefetcher::new(f.clone(), 4);
    p.hint(&fr("a.png", 1));
    assert!(wait_until(|| (1..=4).all(|i| f.count(&names(10)[i]) == 1)));
    std::thread::sleep(Duration::from_millis(100));
    for i in [0, 5, 6, 7, 8, 9] {
        assert_eq!(f.count(&names(10)[i]), 0, "quadro {i}");
    }
    assert!(wait_until(|| p.cached_len() == 4));
    p.get(&fr("b.png", 1)).unwrap();
    assert_eq!(f.count("b.png"), 1);
}

#[test]
fn error_in_one_frame_not_affects_the_neighbor_and_stays_in_cache() {
    let f = fake_n(5, 0, &[]);
    f.corrupt.lock().unwrap().insert("c.png".into());
    let p = Prefetcher::new(f.clone(), 3);
    assert!(p.get(&fr("c.png", 1)).is_err());
    assert!(p.get(&fr("c.png", 1)).is_err());
    assert_eq!(f.count("c.png"), 1);
    assert!(p.get(&fr("d.png", 1)).is_ok());
}

#[test]
fn reading_with_error_becomes_invalid() {
    let f = fake_n(2, 0, &["a.png"]);
    let p = Prefetcher::new(f, 3);
    assert!(matches!(p.get(&fr("a.png", 1)), Err(DecodeError::Invalid(_))));
}

#[test]
fn error_of_and_s_of_the_source_becomes_reading_and_recovers() {
    let f = fake_n(3, 0, &[]);
    f.io_fail.lock().unwrap().insert("b.png".into());
    let p = Prefetcher::new(f.clone(), 3);
    match p.get(&fr("b.png", 1)) {
        Err(DecodeError::Reading(m)) => assert!(m.contains("NAS desmontado"), "{m}"),
        o => panic!("esperava Leitura, veio {o:?}"),
    }
    // corrompido continua sendo erro de decodificacao, nao de leitura
    f.corrupt.lock().unwrap().insert("c.png".into());
    assert!(matches!(p.get(&fr("c.png", 1)), Err(DecodeError::Invalid(_))));
    // o NAS volta: a mesma leitura funciona (erro de leitura nunca fica em cache)
    f.io_fail.lock().unwrap().clear();
    assert!(p.get(&fr("b.png", 1)).is_ok());
}

#[test]
fn next_and_prev_cross_pages_and_files() {
    let f = Fake::new(&["a.png", "b.tif", "c.png"], 0, &[]);
    let p = Prefetcher::new(f, 3);
    // contagem de paginas ainda desconhecida: assume 1
    assert_eq!(p.next_frame(&fr("b.tif", 1)), Some(fr("c.png", 1)));
    assert_eq!(p.get(&fr("b.tif", 1)).unwrap().page_count, 2);
    assert_eq!(p.next_frame(&fr("b.tif", 1)), Some(fr("b.tif", 2)));
    assert_eq!(p.next_frame(&fr("b.tif", 2)), Some(fr("c.png", 1)));
    assert_eq!(p.prev_frame(&fr("c.png", 1)), Some(fr("b.tif", 2)));
    assert_eq!(p.prev_frame(&fr("b.tif", 2)), Some(fr("b.tif", 1)));
    assert_eq!(p.prev_frame(&fr("b.tif", 1)), Some(fr("a.png", 1)));
    assert_eq!(p.prev_frame(&fr("a.png", 1)), None);
    assert_eq!(p.next_frame(&fr("c.png", 1)), None);
}

#[test]
fn skip_far_discards_jobs_not_started() {
    let f = fake_n(10, 300, &[]);
    let p = Prefetcher::new(f.clone(), 4);
    p.hint(&fr("a.png", 1));
    assert!(wait_until(|| f.count("b.png") == 1 && f.count("c.png") == 1));
    p.hint(&fr("i.png", 1));
    assert!(wait_until(|| f.count("j.png") == 1));
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(f.count("d.png"), 0);
    assert_eq!(f.count("e.png"), 0);
}

#[test]
fn two_threads_get_in_the_same_frame_read_a_time() {
    let f = fake_n(3, 150, &[]);
    let p = Arc::new(Prefetcher::new(f.clone(), 3));
    let hs: Vec<_> = (0..2)
        .map(|_| {
            let p = Arc::clone(&p);
            std::thread::spawn(move || p.get(&fr("b.png", 1)).unwrap())
        })
        .collect();
    for h in hs {
        h.join().unwrap();
    }
    assert_eq!(f.count("b.png"), 1);
}

#[test]
fn get_wait_job_of_background_in_progress() {
    let f = fake_n(5, 200, &[]);
    let p = Prefetcher::new(f.clone(), 3);
    p.hint(&fr("a.png", 1));
    assert!(wait_until(|| f.count("b.png") == 1));
    p.get(&fr("b.png", 1)).unwrap();
    assert_eq!(f.count("b.png"), 1);
}

#[test]
fn drop_with_queue_slow_returns_fast() {
    let f = fake_n(10, 300, &[]);
    let p = Prefetcher::new(f.clone(), 5);
    p.hint(&fr("a.png", 1));
    assert!(wait_until(|| f.count("b.png") == 1));
    let t = Instant::now();
    drop(p);
    assert!(t.elapsed() < Duration::from_secs(2));
}

#[test]
fn cache_respects_the_limit() {
    let f = fake_n(20, 0, &[]);
    let p = Prefetcher::new(f, 3);
    let cap = 3 * 2 + 2;
    for i in 0..20 {
        let fm = fr(&names(20)[i], 1);
        p.hint(&fm);
        p.get(&fm).unwrap();
        assert!(p.cached_len() <= cap, "{} > {cap}", p.cached_len());
    }
    std::thread::sleep(Duration::from_millis(100));
    assert!(p.cached_len() <= cap);
}

#[test]
fn error_of_reading_not_stays_in_cache_in_the_get() {
    let f = fake_n(5, 0, &[]);
    f.fail_times("c.png", 1);
    let p = Prefetcher::new(f.clone(), 3);
    assert!(p.get(&fr("c.png", 1)).is_err());
    assert_eq!(p.cached_len(), 0);
    assert!(p.get(&fr("c.png", 1)).is_ok());
    assert_eq!(f.count("c.png"), 2);
}

#[test]
fn error_of_reading_in_second_plane_and_rescheduled_by_the_hint() {
    let f = fake_n(6, 0, &[]);
    f.fail_times("c.png", 1);
    let p = Prefetcher::new(f.clone(), 3);
    p.hint(&fr("a.png", 1));
    assert!(wait_until(|| f.count("c.png") == 1 && f.count("b.png") == 1 && f.count("d.png") == 1));
    assert!(wait_until(|| p.cached_len() == 2));
    p.hint(&fr("a.png", 1));
    assert!(wait_until(|| f.count("c.png") == 2 && p.cached_len() == 3));
    assert_eq!(f.count("b.png"), 1);
}

#[test]
fn error_of_decoding_continues_in_cache() {
    let f = fake_n(6, 0, &[]);
    f.corrupt.lock().unwrap().insert("d.png".into());
    let p = Prefetcher::new(f.clone(), 3);
    assert!(p.get(&fr("d.png", 1)).is_err());
    assert!(p.get(&fr("d.png", 1)).is_err());
    assert_eq!(f.count("d.png"), 1);
}

#[test]
fn waits_receive_the_error_of_reading_and_after_recovers() {
    let f = fake_n(5, 200, &[]);
    f.fail_times("b.png", 1);
    let p = Arc::new(Prefetcher::new(f.clone(), 3));
    let hs: Vec<_> = (0..2)
        .map(|_| {
            let p = Arc::clone(&p);
            std::thread::spawn(move || p.get(&fr("b.png", 1)).is_err())
        })
        .collect();
    for h in hs {
        assert!(h.join().unwrap());
    }
    assert_eq!(f.count("b.png"), 1);
    assert_eq!(p.cached_len(), 0);
    assert!(p.get(&fr("b.png", 1)).is_ok());
    assert_eq!(f.count("b.png"), 2);
}

/// Cada quadro do fake decodifica para 2x2 RGBA = 16 bytes.
const FRAME: u64 = 16;

#[test]
fn cache_respects_the_budget_of_bytes() {
    let f = fake_n(8, 0, &[]);
    // Cabem 2 quadros; o limite por contagem (ahead*2+2 = 8) nao entraria em jogo.
    let p = Prefetcher::with_budget(f.clone(), 3, 2 * FRAME + 8);
    for n in names(8) {
        p.get(&fr(&n, 1)).unwrap();
        assert!(p.cached_bytes() <= 2 * FRAME + 8, "{} bytes", p.cached_bytes());
    }
    assert_eq!(p.cached_len(), 2);
    assert_eq!(p.cached_bytes(), 2 * FRAME);
    // O ultimo terminado sobrevive.
    p.get(&fr("h.png", 1)).unwrap();
    assert_eq!(f.count("h.png"), 1);
}

#[test]
fn budget_spares_the_current_and_the_just_finished_same_overflowing() {
    let f = fake_n(4, 0, &[]);
    // Nem um quadro cabe: o atual e o recem-terminado ficam assim mesmo.
    let p = Prefetcher::with_budget(f.clone(), 3, FRAME / 2);
    p.get(&fr("a.png", 1)).unwrap();
    p.hint(&fr("a.png", 1));
    p.get(&fr("c.png", 1)).unwrap();
    assert!(wait_until(|| f.count("b.png") == 1));
    std::thread::sleep(Duration::from_millis(50));
    p.get(&fr("a.png", 1)).unwrap();
    assert_eq!(f.count("a.png"), 1, "o atual nao podia sair do cache");
    assert!(p.cached_len() <= 2);
}

#[test]
fn hint_shrinks_the_window_to_fit_in_the_budget() {
    let f = fake_n(10, 0, &[]);
    // (n+1) * 16 <= 40  =>  n = 1: so o proximo quadro e pre-carregado.
    let p = Prefetcher::with_budget(f.clone(), 4, 40);
    p.get(&fr("a.png", 1)).unwrap();
    p.hint(&fr("a.png", 1));
    assert!(wait_until(|| f.count("b.png") == 1));
    std::thread::sleep(Duration::from_millis(100));
    for n in ["c.png", "d.png", "e.png"] {
        assert_eq!(f.count(n), 0, "{n}");
    }
    // Minimo de 1 a frente mesmo quando nem o atual cabe.
    let f = fake_n(10, 0, &[]);
    let p = Prefetcher::with_budget(f.clone(), 4, 1);
    p.get(&fr("a.png", 1)).unwrap();
    p.hint(&fr("a.png", 1));
    assert!(wait_until(|| f.count("b.png") == 1));
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(f.count("c.png"), 0);
}

/// Um TIFF de varias paginas e lido da fonte uma vez so para todas as paginas.
#[test]
fn pages_of_the_same_file_reuse_os_bytes_read() {
    let f = Fake::new(&["a.png", "b.tif", "c.png"], 0, &[]);
    let p = Prefetcher::new(f.clone(), 3);
    p.get(&fr("b.tif", 1)).unwrap();
    p.get(&fr("b.tif", 2)).unwrap();
    assert_eq!(f.count("b.tif"), 1);
    // Outro arquivo troca o cache; voltar a uma pagina ainda nao decodificada le de novo.
    p.get(&fr("c.png", 1)).unwrap();
    assert_eq!(f.count("c.png"), 1);
}

/// Erro de leitura nao fica no cache de bytes: o NAS volta e a pagina 2 le de novo.
#[test]
fn cache_of_bytes_not_keeps_error_of_reading() {
    let f = Fake::new(&["b.tif"], 0, &[]);
    let p = Prefetcher::new(f.clone(), 3);
    f.io_fail.lock().unwrap().insert("b.tif".into());
    assert!(matches!(p.get(&fr("b.tif", 1)), Err(DecodeError::Reading(_))));
    f.io_fail.lock().unwrap().clear();
    p.get(&fr("b.tif", 1)).unwrap();
    p.get(&fr("b.tif", 2)).unwrap();
    assert_eq!(f.count("b.tif"), 2);
}
