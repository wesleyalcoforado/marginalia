//! Testes do `AppState` (ver `mod.rs`).

use super::*;
use crate::load::{self, GridRegion};
use crate::quick_note_session::{Created, QuickNoteSession};
use chrono::NaiveDateTime;
use marginalia_core::model::Note;
use marginalia_core::vault::{route_quick_note_input, QuickNoteInput};
use image::{Rgba, RgbaImage};
use marginalia_core::highlights::HighlightsFile;
use marginalia_core::source::ImageEntry;
use marginalia_core::Error;
use std::io::Cursor;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tempfile::tempdir;

/// Fonte em memoria que registra cada leitura.
struct Fake {
    entries: Vec<ImageEntry>,
    data: HashMap<String, Vec<u8>>,
    read_log: Mutex<Vec<String>>,
}

impl Fake {
    fn new(files: &[(&str, Vec<u8>)]) -> Arc<Fake> {
        Arc::new(Fake {
            entries: files.iter().map(|(n, _)| ImageEntry { path: n.to_string() }).collect(),
            data: files.iter().map(|(n, b)| (n.to_string(), b.clone())).collect(),
            read_log: Mutex::new(Vec::new()),
        })
    }

    fn has_read(&self, p: &str) -> bool {
        self.read_log.lock().unwrap().iter().any(|l| l == p)
    }
}

impl ImageSource for Fake {
    fn entries(&self) -> &[ImageEntry] {
        &self.entries
    }
    fn read(&self, path: &str) -> marginalia_core::vault::Result<Vec<u8>> {
        self.read_log.lock().unwrap().push(path.to_string());
        self.data.get(path).cloned().ok_or_else(|| Error::InputNotFound(path.into()))
    }
}

fn png(color: [u8; 4]) -> Vec<u8> {
    let img = RgbaImage::from_pixel(3, 2, Rgba(color));
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

fn names(n: usize) -> Vec<(String, Vec<u8>)> {
    (1..=n).map(|i| (format!("p{i:02}.png"), png([10, 20, 30, 255]))).collect()
}

fn fake(files: &[(String, Vec<u8>)]) -> Arc<Fake> {
    let v: Vec<(&str, Vec<u8>)> = files.iter().map(|(n, b)| (n.as_str(), b.clone())).collect();
    Fake::new(&v)
}

fn state(base: &std::path::Path) -> AppState {
    test::register_shortcuts_false();
    AppState::new(None, AppData::new(base.join("appdata")), base.join("cache"))
}

fn wait(cond: impl Fn() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(3);
    while Instant::now() < end {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    cond()
}

fn image(r: Received) -> Display {
    match r {
        Received::Image(e, _, _) => e,
        Received::Error(m) => panic!("esperava imagem, veio erro {m}"),
        Received::Unavailable(m) => panic!("esperava imagem, veio fonte indisponivel {m}"),
        Received::Ignored => panic!("esperava imagem, veio ignorado"),
        Received::Layer(_) => panic!("esperava imagem, veio so a camada de fundo"),
        Received::WindowChanged => panic!("esperava imagem, veio tile de janela velha"),
    }
}

#[test]
fn go_next_changes_current_and_calls_hint() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let source = fake(&names(10));
    let p = st.adopt_source("Livro", source.clone()).expect("primeiro quadro");
    assert_eq!(st.current, Frame::new("Livro", "p01.png", 1));
    image(st.load_now(p));

    let p = st.go_next().expect("ha proximo");
    assert_eq!(st.current, Frame::new("Livro", "p02.png", 1));
    assert_eq!(st.index(), Some((2, 10)));
    assert!(st.loading());
    // hint(p01) agenda ate p01+AHEAD; so hint(p02) chega a p02+AHEAD, sem nenhum `get`.
    let beyond = format!("p{:02}.png", 2 + AHEAD);
    assert!(wait(|| source.has_read(&beyond)), "hint nao pre-carregou {beyond}");
    image(st.load_now(p));
    assert!(!st.loading());

    assert!(st.go_prev().is_some());
    assert_eq!(st.current.image.file, "p01.png");
    assert!(st.go_prev().is_none(), "nao ha anterior ao primeiro");
}

#[test]
fn frame_that_failure_leaves_error_and_go_next_continues() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let source = fake(&[
        ("a.png".into(), png([1, 2, 3, 255])),
        ("b.png".into(), b"isto nao e png".to_vec()),
        ("c.png".into(), png([1, 2, 3, 255])),
    ]);
    let p = st.adopt_source("Livro", source).unwrap();
    image(st.load_now(p));

    let p = st.open_frame(Frame::new("Livro", "b.png", 1)).unwrap();
    match st.load_now(p) {
        Received::Error(m) => assert!(m.contains("b.png"), "{m}"),
        _ => panic!("esperava erro"),
    }
    assert!(st.error.is_some());
    assert!(!st.loading());

    let p = st.go_next().expect("go_next depois do erro");
    assert_eq!(st.current.image.file, "c.png");
    assert_eq!(st.error, None);
    image(st.load_now(p));
    assert_eq!(st.error, None);
    assert!(st.go_next().is_none());
}

#[test]
fn adjustments_write_in_app_data_by_image_and_return_to_the_reopen() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let p = st.adopt_source("Livro", fake(&names(3))).unwrap();
    image(st.load_now(p));

    let inv = Adjustments { invert: true, ..Adjustments::default() };
    let p = st.set_adjustments(inv.clone()).expect("reaplicar");
    assert_eq!(p.frame, Reframe::Keep);
    assert!(p.decoded.is_some(), "nao rele a fonte para reaplicar filtros");
    let e = image(st.load_now(p));
    assert_eq!(e.bgra.get_pixel(0, 0).0, [225, 235, 245, 255]);

    let p01 = ImageRef { source: "Livro".into(), file: "p01.png".into(), page: 1 };
    // o disco so recebe na gravacao (feita no fundo pela view, ver `schedule_adjustment`)
    st.app_data_write().execute().unwrap();
    let in_disk = AppData::load(t.path().join("appdata")).unwrap();
    assert_eq!(in_disk.adjustments(&p01), inv);

    let p = st.go_next().unwrap();
    assert_eq!(st.adjustments, Adjustments::default());
    image(st.load_now(p));
    let p = st.go_prev().unwrap();
    assert_eq!(st.adjustments, inv);
    let e = image(st.load_now(p));
    assert_eq!(e.bgra.get_pixel(0, 0).0, [225, 235, 245, 255]);

    // outro estado com os dados do disco tambem restaura
    let mut st2 = AppState::new(None, in_disk, t.path().join("cache2"));
    st2.adopt_source("Livro", fake(&names(3))).unwrap();
    assert_eq!(st2.adjustments, inv);
}

#[test]
fn response_stale_and_ignored() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let p0 = st.adopt_source("Livro", fake(&names(4))).unwrap();
    image(st.load_now(p0));
    let p1 = st.go_next().unwrap();
    let p2 = st.open_frame(Frame::new("Livro", "p04.png", 1)).unwrap();
    assert!(matches!(st.load_now(p1), Received::Ignored));
    assert!(st.loading());
    image(st.load_now(p2));
    let p3 = st.set_adjustments(Adjustments { invert: true, ..Adjustments::default() }).unwrap();
    let p4 = st.set_adjustments(Adjustments { grayscale: true, ..Adjustments::default() }).unwrap();
    assert!(matches!(st.load_now(p3), Received::Ignored));
    image(st.load_now(p4));
    assert_eq!(st.current.image.file, "p04.png");
}

/// Base e tile dos mesmos valores (a agenda de hoje pede nesta ordem).
fn preview_and_final(st: &mut AppState, a: Adjustments) -> (Request, Request) {
    if st.view_window().is_none() {
        // O visualizador informa a pagina inteira (grampeada nela quando chegar):
        // sem janela o tile sairia como base.
        let n = 1 << 20;
        st.set_window(ViewWindow { grid: (n, n), region: GridRegion { x0: 0, y0: 0, x1: n, y1: n } });
    }
    let pv = st.set_adjustments_in(a.clone(), Quality::Base).expect("base");
    let fi = st.set_adjustments_in(a, Quality::Tile).expect("tile");
    assert_eq!(pv.quality, Quality::Base);
    assert_eq!(fi.quality, Quality::Tile);
    assert_eq!(pv.generation, fi.generation, "o tile dos mesmos valores nao invalida a base em voo");
    (pv, fi)
}

/// Como [`state_no_window`], com o visualizador ja tendo informado a pagina
/// inteira como janela (o tile padrao dos testes).
fn state_with_page(t: &std::path::Path, w: u32, h: u32) -> AppState {
    let mut st = state_no_window(t, w, h);
    assert!(st.set_window(ViewWindow::whole_page((w, h))));
    st
}

/// Pagina unica `w x h` com textura (o realce tem o que mudar), ja na tela,
/// sem janela informada.
fn state_no_window(t: &std::path::Path, w: u32, h: u32) -> AppState {
    let mut st = state(t);
    let img = RgbaImage::from_fn(w, h, |x, y| {
        let v = ((x * 37 + y * 91) % 256) as u8;
        Rgba([v, 255 - v, v / 2, 255])
    });
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    let p = st.adopt_source("Livro", fake(&[("g.png".into(), out.into_inner())])).unwrap();
    image(st.load_now(p));
    st
}

fn highlight() -> Adjustments {
    Adjustments { sharpen: 0.6, highpass: 0.7, ..Adjustments::default() }
}

#[test]
fn base_uses_the_copy_reduced_and_tile_the_window() {
    let t = tempdir().unwrap();
    let mut st = state_with_page(t.path(), 3000, 1000);
    assert!(st.tile_display().is_none() && st.base_display().is_none(), "sem ajuste: so a cheia");
    assert_eq!(st.view_window(), Some(ViewWindow::whole_page((3000, 1000))), "o visualizador informou a pagina inteira");

    // janela da pagina inteira: o tile cobre a pagina inteira (a grade da textura)
    let (pv, fi) = preview_and_final(&mut st, Adjustments { invert: true, ..Adjustments::default() });
    assert_eq!(fi.view_window, Some(ViewWindow::whole_page((3000, 1000))));
    let e = image(st.load_now(pv));
    assert_eq!((e.bgra.dimensions(), e.sz_original), ((2048, 683), (3000, 1000)));
    assert!(st.base_display().is_some());
    let e = image(st.load_now(fi));
    assert_eq!((e.bgra.dimensions(), e.sz_original), ((3000, 1000), (3000, 1000)));
    let (px, j) = st.tile_display().expect("tile na tela");
    assert_eq!(j, ViewWindow::whole_page((3000, 1000)));
    assert_eq!(px.sz, e.bgra.dimensions());
    assert_eq!(Arc::strong_count(&e.bgra), 1, "o estado nao guarda os pixels: vao inteiros (sem copia) para o visualizador");
    assert!(st.base_display().is_some(), "a base dos mesmos valores fica por baixo");

    // a janela visivel informada: o tile tem exatamente o tamanho dela
    let j = ViewWindow { grid: (1500, 500), region: GridRegion { x0: 100, y0: 50, x1: 400, y1: 250 } };
    assert!(st.set_window(j));
    assert!(!st.set_window(j), "a mesma janela nao muda nada");
    assert_eq!(st.view_window(), Some(j));
    let p = st.set_adjustments_in(highlight(), Quality::Tile).unwrap();
    assert_eq!(p.view_window, Some(j));
    let e = image(st.load_now(p));
    assert_eq!((e.bgra.dimensions(), e.sz_original), ((300, 200), (3000, 1000)));
    assert_eq!(st.tile_display().map(|(px, j)| (px.sz, j)), Some(((300, 200), j)));

    // ajuste neutro: a textura cheia sem filtro, como antes; base e tile saem
    let p = st.set_adjustments_in(Adjustments::default(), Quality::Base).unwrap();
    assert_eq!(p.quality, Quality::Full);
    assert_eq!(image(st.load_now(p)).bgra.dimensions(), (3000, 1000));
    assert!(st.tile_display().is_none() && st.base_display().is_none());
    // e um pedido de "cheia" com ajuste ativo vira tile (nunca a pagina cheia filtrada)
    let p = st.set_adjustments_in(highlight(), Quality::Full).unwrap();
    assert_eq!((p.quality, p.view_window), (Quality::Tile, Some(j)));
}

#[test]
fn rotate_asks_the_texture_rotated_and_breaks_as_layers_old() {
    let t = tempdir().unwrap();
    let mut st = state_with_page(t.path(), 3000, 1000);
    assert_eq!(st.rotation(), Rotation::R0);
    // Um tile em voo antes do giro: a resposta dele nao vale mais.
    let old = st.set_adjustments_in(highlight(), Quality::Tile).unwrap();
    assert_eq!(old.rotation, Rotation::R0);
    assert!(st.rotate(Rotation::R90));
    assert!(!st.rotate(Rotation::R90), "a mesma rotacao nao muda nada");
    assert_eq!(st.rotation(), Rotation::R90);
    assert_eq!(st.app_data.rotation(&st.current.image), Rotation::R90, "vale nos dados do app, por imagem");
    assert!(matches!(st.receive(load::execute(old)), Received::Ignored), "a geracao velha cai");

    // Com ajuste: o tile sai na janela de sempre (grade da pagina SEM girar), os pixels girados.
    let j = st.view_window().expect("janela informada");
    let p = st.set_adjustments_in(highlight(), Quality::Tile).unwrap();
    assert_eq!((p.rotation, p.view_window), (Rotation::R90, Some(j)));
    let e = image(st.load_now(p));
    assert_eq!((e.bgra.dimensions(), e.sz_original, e.rot), ((1000, 3000), (3000, 1000), Rotation::R90));
    assert_eq!(st.tile_display().map(|(c, _)| c.sz), Some((1000, 3000)));

    // Girar de novo derruba a base e o tile (eram da orientacao velha).
    let b = st.set_adjustments_in(highlight(), Quality::Base).unwrap();
    // Com o tile dos mesmos valores na tela, a base so vai para o fundo (`Received::Layer`).
    assert!(matches!(st.load_now(b), Received::Layer(e) if e.rot == Rotation::R90));
    assert!(st.base_display().is_some() && st.tile_display().is_some());
    assert!(st.rotate(Rotation::R180));
    assert!(st.base_display().is_none() && st.tile_display().is_none());

    // Sem ajuste na tela: a cheia, girada.
    let p = st.set_adjustments_in(Adjustments::default(), Quality::Full).unwrap();
    assert_eq!((p.quality, p.rotation), (Quality::Full, Rotation::R180));
    let e = image(st.load_now(p));
    assert_eq!((e.bgra.dimensions(), e.rot), ((3000, 1000), Rotation::R180));
}

#[test]
fn rotation_back_with_the_image_and_not_leaks_to_as_other() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let p = st.adopt_source("Livro", fake(&names(3))).unwrap();
    assert_eq!(p.rotation, Rotation::R0);
    image(st.load_now(p));
    assert!(st.rotate(Rotation::R270));

    let p = st.go_next().unwrap();
    assert_eq!((st.rotation(), p.rotation), (Rotation::R0, Rotation::R0), "a outra imagem nao herda a rotacao");
    image(st.load_now(p));
    assert!(st.rotate(Rotation::R90));

    let p = st.go_prev().unwrap();
    assert_eq!((st.rotation(), p.rotation), (Rotation::R270, Rotation::R270), "a primeira volta como foi deixada");
    let e = image(st.load_now(p));
    assert_eq!((e.bgra.dimensions(), e.sz_original), ((2, 3), (3, 2)), "a pagina de 3x2 chega de pe, 2x3");
    let p = st.go_next().unwrap();
    assert_eq!(p.rotation, Rotation::R90);

    // Girar de volta ao 0 tira a entrada dos dados do app.
    assert!(st.rotate(Rotation::R0));
    assert_eq!(st.app_data.rotation(&st.current.image), Rotation::R0);
}

#[test]
fn tile_so_enters_se_the_generation_and_the_window_still_count() {
    let t = tempdir().unwrap();
    let mut st = state_with_page(t.path(), 400, 300);
    let j1 = ViewWindow { grid: (200, 150), region: GridRegion { x0: 0, y0: 0, x1: 100, y1: 100 } };
    let j2 = ViewWindow { grid: (200, 150), region: GridRegion { x0: 50, y0: 0, x1: 150, y1: 100 } };
    st.set_window(j1);
    let p1 = st.set_adjustments_in(highlight(), Quality::Tile).unwrap();
    st.set_window(j2);
    let r1 = load::execute(p1);
    assert_eq!(r1.view_window, Some(j1));
    assert!(matches!(st.receive(r1), Received::WindowChanged), "a janela mudou: o tile da antiga cai");
    assert!(st.tile_display().is_none());
    let p2 = st.set_adjustments_in(highlight(), Quality::Tile).unwrap();
    assert_eq!(p2.view_window, Some(j2));
    image(st.load_now(p2));
    assert_eq!(st.tile_display().map(|(_, j)| j), Some(j2));
    assert!(!st.loading());
}

#[test]
fn window_empty_and_ignored_and_the_given_and_clamped_in_the_page() {
    let t = tempdir().unwrap();
    let mut st = state_no_window(t.path(), 400, 300);
    let empty =ViewWindow { grid: (200, 150), region: GridRegion { x0: 10, y0: 10, x1: 10, y1: 90 } };
    assert!(!st.set_window(empty));
    assert_eq!(st.view_window(), None);
    let large = ViewWindow { grid: (800, 600), region: GridRegion { x0: 0, y0: 0, x1: 800, y1: 600 } };
    assert!(st.set_window(large));
    assert_eq!(st.view_window(), Some(ViewWindow::whole_page((400, 300))), "a grade nunca amplia a pagina");
    let p = st.set_adjustments_in(highlight(), Quality::Tile).unwrap();
    assert_eq!(image(st.load_now(p)).bgra.dimensions(), (400, 300));
}

#[test]
fn tile_late_not_replaces_one_more_new() {
    let t = tempdir().unwrap();
    let mut st = state_with_page(t.path(), 200, 100);
    let old = st.set_adjustments_in(Adjustments { invert: true, ..Adjustments::default() }, Quality::Tile).unwrap();
    let r_old = load::execute(old);
    let new = st.set_adjustments_in(highlight(), Quality::Tile).unwrap();
    image(st.load_now(new));
    assert!(matches!(st.receive(r_old), Received::Ignored));
    assert_eq!(st.screen_adjustments(), Some(&highlight()));
}

/// Base e tile na tela sempre dos MESMOS valores: nenhum pedaco mostra o filtro de outro valor.
#[test]
fn base_and_tile_never_stay_with_values_different() {
    let t = tempdir().unwrap();
    let mut st = state_with_page(t.path(), 3000, 1000);
    let inv = Adjustments { invert: true, ..Adjustments::default() };
    let (pv, fi) = preview_and_final(&mut st, inv.clone());
    image(st.load_now(pv));
    image(st.load_now(fi));
    assert!(st.tile_display().is_some() && st.base_display().is_some());
    // a base de um valor novo chega primeiro: o tile do valor velho sai
    let pv = st.set_adjustments_in(highlight(), Quality::Base).unwrap();
    image(st.load_now(pv));
    assert!(st.tile_display().is_none(), "o tile de outro valor nao fica por cima da base nova");
    assert!(st.base_display().is_some());
    // o tile de outro valor que chega depois tira a base velha
    let fi = st.set_adjustments_in(inv, Quality::Tile).unwrap();
    image(st.load_now(fi));
    assert!(st.tile_display().is_some());
    assert!(st.base_display().is_none(), "a base de outro valor nao fica por baixo do tile novo");
}

/// Com o tile pedido primeiro (a agenda da Task 3), a base dos mesmos valores
/// reusa a geracao dele: nao derruba o tile em voo.
#[test]
fn base_of_the_same_values_of_the_tile_pending_reuses_the_generation() {
    let t = tempdir().unwrap();
    let mut st = state_with_page(t.path(), 200, 100);
    let fi = st.set_adjustments_in(highlight(), Quality::Tile).unwrap();
    let pv = st.set_adjustments_in(highlight(), Quality::Base).unwrap();
    assert_eq!(fi.generation, pv.generation);
    let r_pv = load::execute(pv);
    image(st.load_now(fi));
    let (tile, j) = st.tile_display().unwrap();
    let (displays, fr) = (st.displays().1, st.fr.clone());
    // a base atrasada nao cobre o tile: entra so como camada de fundo
    assert!(matches!(st.receive(r_pv), Received::Layer(_)));
    assert_eq!(st.tile_display(), Some((tile, j)));
    assert!(st.base_display().is_some(), "as duas camadas ficam");
    assert_eq!(st.screen_adjustments(), Some(&highlight()));
    assert_eq!((st.displays().1, st.fr.clone()), (displays, fr), "nada foi para a tela nem reenquadrou");
    assert!(!st.loading());
}

#[test]
fn base_after_of_the_tile_and_tile_after_of_the_base_leave_as_two_layers() {
    let t = tempdir().unwrap();
    let mut st = state_with_page(t.path(), 3000, 1000);
    // base primeiro (a agenda de hoje)
    let (pv, fi) = preview_and_final(&mut st, highlight());
    assert!(matches!(st.load_now(pv), Received::Image(..)));
    assert!(matches!(st.load_now(fi), Received::Image(..)));
    assert!(st.tile_display().is_some() && st.base_display().is_some());
    // tile primeiro (a agenda da Task 3), outro valor
    let inv = Adjustments { invert: true, ..Adjustments::default() };
    let fi = st.set_adjustments_in(inv.clone(), Quality::Tile).unwrap();
    let pv = st.set_adjustments_in(inv.clone(), Quality::Base).unwrap();
    assert!(matches!(st.load_now(fi), Received::Image(..)));
    assert!(st.base_display().is_none(), "a base do valor velho saiu");
    assert!(matches!(st.load_now(pv), Received::Layer(_)));
    assert!(st.tile_display().is_some() && st.base_display().is_some());
    assert_eq!(st.screen_adjustments(), Some(&inv));
}

/// A agenda pede a base so depois de o tile do valor terminar: ela abre a
/// propria geracao e, com o tile dos mesmos valores na tela, so vai para o
/// fundo (nunca tira o tile, nao reenquadra e nao deixa o quadro carregando).
#[test]
fn base_requested_after_of_the_tile_in_the_screen_so_goes_to_the_background() {
    let t = tempdir().unwrap();
    let mut st = state_with_page(t.path(), 3000, 1000);
    let j = ViewWindow { grid: (1500, 500), region: GridRegion { x0: 100, y0: 50, x1: 400, y1: 250 } };
    assert!(st.set_window(j));
    let fi = st.set_adjustments_in(highlight(), Quality::Tile).unwrap();
    image(st.load_now(fi));
    let (tile, jt) = st.tile_display().unwrap();
    assert_eq!(jt, j);
    let (displays, fr) = (st.displays().1, st.fr.clone());
    let pv = st.set_adjustments_in(highlight(), Quality::Base).unwrap();
    assert!(st.loading());
    assert!(matches!(st.load_now(pv), Received::Layer(_)), "a base nao cobre o tile dos mesmos valores");
    assert!(!st.loading(), "a base que so vai para o fundo nao deixa o quadro carregando");
    assert_eq!(st.tile_display().unwrap().0, tile, "o tile fica");
    let base = st.base_display().expect("a camada de fundo");
    assert_eq!(st.screen_adjustments(), Some(&highlight()));
    assert_eq!((st.displays().1, st.fr.clone()), (displays, fr), "nada foi para a tela nem reenquadrou");
    // a janela muda: o tile novo dos mesmos valores mantem a base
    let j2 = ViewWindow { grid: (1500, 500), region: GridRegion { x0: 600, y0: 50, x1: 900, y1: 250 } };
    assert!(st.set_window(j2));
    let fi2 = st.set_adjustments_in(highlight(), Quality::Tile).unwrap();
    assert_eq!(image(st.load_now(fi2)).bgra.dimensions(), (300, 200));
    assert_eq!(st.tile_display().unwrap().1, j2);
    assert_eq!(st.base_display().unwrap(), base, "a base do mesmo valor fica por baixo");
    assert!(!st.loading());
}

#[test]
fn filters_in_the_screen_follows_the_adjustment_and_the_eye() {
    let t = tempdir().unwrap();
    let mut st = state_with_page(t.path(), 300, 100);
    assert!(!st.screen_filters(), "sem ajuste nada a filtrar");
    st.apply_adjustments(highlight());
    assert!(st.screen_filters());
    st.set_show_adjustments(false);
    assert!(!st.screen_filters(), "olho desligado: a tela mostra a imagem sem ajustes");
}

/// O que a agenda precisa saber da camada de fundo: o tile na tela esta sem
/// base (o pan expoe o fundo escuro: a base sai ja) e os valores da base na
/// tela (a base da abertura ja e a do valor: nao pedir outra igual).
#[test]
fn tile_no_base_and_adjustments_of_the_base() {
    let t = tempdir().unwrap();
    let mut st = state_with_page(t.path(), 3000, 1000);
    assert!(!st.tile_no_base(), "nada na tela");
    assert_eq!(st.base_adjustments(), None);
    let pv = st.set_adjustments_in(highlight(), Quality::Base).unwrap();
    image(st.load_now(pv));
    assert!(!st.tile_no_base(), "so a base: nada a pedir");
    assert_eq!(st.base_adjustments(), Some(&highlight()));
    let fi = st.set_adjustments_in(highlight(), Quality::Tile).unwrap();
    image(st.load_now(fi));
    assert!(!st.tile_no_base(), "tile e base do mesmo valor");
    // o tile de um valor novo tira a base velha: o tile fica sem base
    let inv = Adjustments { invert: true, ..Adjustments::default() };
    let fi = st.set_adjustments_in(inv.clone(), Quality::Tile).unwrap();
    image(st.load_now(fi));
    assert!(st.tile_no_base());
    assert_eq!(st.base_adjustments(), None);
    let pv = st.set_adjustments_in(inv.clone(), Quality::Base).unwrap();
    assert!(matches!(st.load_now(pv), Received::Layer(_)));
    assert!(!st.tile_no_base());
    assert_eq!(st.base_adjustments(), Some(&inv));
}

/// Antes de o visualizador informar a regiao (a imagem ainda carrega), um pedido
/// de tile nao filtra a pagina inteira em resolucao cheia: sai como base (no
/// maximo `SIDE_BASE`), e com os valores da base em voo reusa a geracao dela (nao
/// a derruba). Informada a janela, volta a ser tile.
#[test]
fn tile_no_window_leaves_as_base_no_break_the_base_in_flight() {
    let t = tempdir().unwrap();
    let mut st = state_no_window(t.path(), 3000, 1000);
    let pv = st.set_adjustments_in(highlight(), Quality::Base).unwrap();
    let fi = st.set_adjustments_in(highlight(), Quality::Tile).unwrap();
    assert_eq!((fi.quality, fi.view_window), (Quality::Base, None));
    assert_eq!(fi.generation, pv.generation, "a base em voo continua valendo");
    let full = st.set_adjustments_in(highlight(), Quality::Full).unwrap();
    assert_eq!(full.quality, Quality::Base, "a cheia com ajuste tambem");
    let e = image(st.load_now(fi));
    assert!(e.bgra.width().max(e.bgra.height()) <= load::SIDE_BASE, "{:?}", e.bgra.dimensions());
    assert!(!matches!(st.load_now(pv), Received::Ignored), "a base em voo nao caiu");
    assert!(st.set_window(ViewWindow::whole_page((3000, 1000))));
    assert_eq!(st.set_adjustments_in(highlight(), Quality::Tile).unwrap().quality, Quality::Tile);
}

/// So a carga da pagina (decodificar) marca o visualizador carregando; refiltrar
/// a pagina ja decodificada (tile e base a cada pan/zoom) nao.
#[test]
fn so_the_load_of_the_page_loads() {
    let t = tempdir().unwrap();
    let mut st = state_with_page(t.path(), 300, 100);
    assert!(!st.set_adjustments_in(highlight(), Quality::Tile).unwrap().page_loads());
    assert!(!st.set_adjustments_in(highlight(), Quality::Base).unwrap().page_loads());
    let f = st.current.clone();
    assert!(st.open_frame(f).unwrap().page_loads(), "abrir o quadro decodifica");
}

/// Uma base que chega numa `Page` nova da MESMA pagina decodificada nao troca a
/// guardada: o recorte do tile guardado nela continua valendo.
#[test]
fn base_of_the_same_page_decoded_keeps_the_crop_of_the_tile() {
    let t = tempdir().unwrap();
    let mut st = state_with_page(t.path(), 3000, 1000);
    let j = ViewWindow { grid: (1500, 500), region: GridRegion { x0: 100, y0: 50, x1: 400, y1: 250 } };
    assert!(st.set_window(j));
    let fi = st.set_adjustments_in(highlight(), Quality::Tile).unwrap();
    image(st.load_now(fi));
    let stored = Arc::clone(st.page.as_ref().unwrap());
    assert_eq!(stored.sources_tile_created(), 1);
    let inv = Adjustments { invert: true, ..Adjustments::default() };
    let mut pv = st.set_adjustments_in(inv.clone(), Quality::Base).unwrap();
    pv.decoded = Some(Arc::new(Page::new(Arc::clone(&stored.decoded))));
    image(st.load_now(pv));
    assert!(Arc::ptr_eq(st.page.as_ref().unwrap(), &stored), "a Pagina guardada fica");
    let fi = st.set_adjustments_in(inv, Quality::Tile).unwrap();
    image(st.load_now(fi));
    assert_eq!(stored.sources_tile_created(), 1, "o recorte guardado serviu ao tile");
}

#[test]
fn base_old_never_replaces_a_more_new() {
    let t = tempdir().unwrap();
    let mut st = state_with_page(t.path(), 3000, 1000);
    let old = st.set_adjustments_in(Adjustments { invert: true, ..Adjustments::default() }, Quality::Base).unwrap();
    let r_old = load::execute(old);
    let fresh = st.set_adjustments_in(highlight(), Quality::Base).unwrap();
    image(st.load_now(fresh));
    let base = st.base_display().unwrap();
    assert!(matches!(st.receive(r_old), Received::Ignored));
    assert_eq!(st.base_display().unwrap(), base);
    // nem depois do tile da geracao nova
    let fi = st.set_adjustments_in(highlight(), Quality::Tile).unwrap();
    image(st.load_now(fi));
    assert_eq!(st.base_display().unwrap(), base);
}

/// PNG `w x h` com textura.
fn png_default(w: u32, h: u32) -> Vec<u8> {
    let img = RgbaImage::from_fn(w, h, |x, y| {
        let v = ((x * 37 + y * 91) % 256) as u8;
        Rgba([v, 255 - v, v / 2, 255])
    });
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

/// A janela informada antes de a pagina decodificar: o tile da pagina inteira
/// que estava em voo cai como `WindowChanged` (nada fica carregando), a janela
/// e grampeada na pagina quando ela chega e o tile pedido de novo a usa.
#[test]
fn window_before_of_the_page_decode_and_clamped_and_the_tile_and_request_of_new() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let g = ImageRef { source: "Livro".into(), file: "g.png".into(), page: 1 };
    st.app_data.set_adjustments(&g, highlight());
    let forward = st.adopt_source("Livro", fake(&[("g.png".into(), png_default(400, 300))])).unwrap();
    assert_eq!((forward.quality, forward.view_window), (Quality::Base, None));
    // um tile pedido antes da pagina chegar e de qualquer janela: sai como base
    // (nunca a pagina inteira em resolucao cheia), na mesma geracao
    let early = st.set_adjustments_in(highlight(), Quality::Tile).unwrap();
    assert_eq!((early.quality, early.view_window, early.generation), (Quality::Base, None, forward.generation));
    // ainda sem pagina: guardada como veio (grade maior que a pagina)
    let j = ViewWindow { grid: (800, 300), region: GridRegion { x0: 100, y0: 0, x1: 900, y1: 200 } };
    assert!(st.set_window(j));
    // um tile pedido com a janela crua, antes da pagina
    let tile = st.set_adjustments_in(highlight(), Quality::Tile).unwrap();
    assert_eq!((tile.quality, tile.view_window, tile.generation), (Quality::Tile, Some(j), forward.generation));
    assert!(matches!(st.load_now(forward), Received::Image(_, _, Layer::Base)));
    assert!(!matches!(st.load_now(early), Received::Ignored), "a base cedo vale");
    // a carga grampeia a janela crua como o estado: o tile vale
    assert!(matches!(st.load_now(tile), Received::Image(_, _, Layer::Tile(_))));
    assert!(!st.loading());
    let normal = ViewWindow { grid: (400, 300), region: GridRegion { x0: 100, y0: 0, x1: 400, y1: 200 } };
    assert_eq!(st.view_window(), Some(normal), "grampeada na pagina quando ela chegou");
    let p = st.re_request_tile().expect("o tile da janela vigente");
    assert_eq!(p.view_window, Some(normal));
    assert_eq!(image(st.load_now(p)).bgra.dimensions(), (300, 200));
    assert!(!st.loading());
    // janela que fica vazia na pagina: esquecida (volta a pagina inteira)
    let outside = ViewWindow { grid: (400, 300), region: GridRegion { x0: 500, y0: 0, x1: 600, y1: 10 } };
    assert!(!st.set_window(outside));
    assert_eq!(st.view_window(), Some(normal));
}

#[test]
fn tile_that_falls_by_the_window_and_request_of_new() {
    let t = tempdir().unwrap();
    let mut st = state_with_page(t.path(), 400, 300);
    let j1 = ViewWindow { grid: (200, 150), region: GridRegion { x0: 0, y0: 0, x1: 100, y1: 100 } };
    let j2 = ViewWindow { grid: (200, 150), region: GridRegion { x0: 50, y0: 0, x1: 150, y1: 100 } };
    st.set_window(j1);
    let p1 = st.set_adjustments_in(highlight(), Quality::Tile).unwrap();
    st.set_window(j2);
    assert!(matches!(st.load_now(p1), Received::WindowChanged));
    assert!(!st.loading());
    let p2 = st.re_request_tile().unwrap();
    assert_eq!((p2.view_window, p2.image_adjustments.clone()), (Some(j2), highlight()));
    image(st.load_now(p2));
    assert!(!st.loading());
    // sem ajuste nao ha tile a pedir
    let p = st.set_adjustments_in(Adjustments::default(), Quality::Tile).unwrap();
    image(st.load_now(p));
    assert!(st.re_request_tile().is_none());
}

#[test]
fn switch_of_image_forgets_window_base_and_tile() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let p = st.adopt_source("Livro", fake(&names(3))).unwrap();
    image(st.load_now(p));
    let j = ViewWindow { grid: (3, 2), region: GridRegion { x0: 0, y0: 0, x1: 2, y1: 2 } };
    st.set_window(j);
    let (pv, fi) = preview_and_final(&mut st, Adjustments { invert: true, ..Adjustments::default() });
    image(st.load_now(pv));
    image(st.load_now(fi));
    assert!(st.tile_display().is_some() && st.base_display().is_some());
    let forward = st.go_next().unwrap();
    assert_eq!(st.view_window(), None, "a janela era da pagina anterior");
    assert!(st.tile_display().is_none() && st.base_display().is_none());
    image(st.load_now(forward));
}

/// Cada imagem aceita diz em que camada vai (o visualizador espelha o estado)
/// e chega com a UNICA referencia aos pixels: o estado guarda so tamanho e
/// numero, entao o visualizador fica com eles sem copia na thread da interface.
#[test]
fn received_says_the_layer_and_delivers_os_pixels_no_copy() {
    let t = tempdir().unwrap();
    let mut st = state_with_page(t.path(), 3000, 1000);
    let p = st.set_adjustments_in(Adjustments::default(), Quality::Tile).unwrap();
    match st.load_now(p) {
        Received::Image(e, _, Layer::Full) => assert_eq!(Arc::strong_count(&e.bgra), 1),
        _ => panic!("sem ajuste: a cheia"),
    }
    let j = ViewWindow { grid: (1500, 500), region: GridRegion { x0: 100, y0: 50, x1: 400, y1: 250 } };
    st.set_window(j);
    let fi = st.set_adjustments_in(highlight(), Quality::Tile).unwrap();
    let base = match st.load_now(fi) {
        Received::Image(e, Reframe::Keep, Layer::Tile(jt)) => {
            assert_eq!(jt, j);
            assert_eq!(Arc::strong_count(&e.bgra), 1);
            assert_eq!(st.tile_display().map(|(c, j)| (c.sz, j)), Some((e.bgra.dimensions(), jt)));
            st.set_adjustments_in(highlight(), Quality::Base).unwrap()
        }
        _ => panic!("o tile da janela"),
    };
    match st.load_now(base) {
        Received::Layer(e) => {
            assert_eq!(Arc::strong_count(&e.bgra), 1);
            assert_eq!(st.base_display().map(|c| c.sz), Some(e.bgra.dimensions()));
        }
        _ => panic!("a base dos mesmos valores vai para o fundo"),
    }
    let pv = st.set_adjustments_in(Adjustments { invert: true, ..Adjustments::default() }, Quality::Base).unwrap();
    match st.load_now(pv) {
        Received::Image(e, _, Layer::Base) => {
            assert_eq!(Arc::strong_count(&e.bgra), 1);
            assert!(st.tile_display().is_none(), "o tile do valor velho saiu: o visualizador tira o dele");
        }
        _ => panic!("a base de outro valor"),
    }
}

/// Imagem aberta com ajuste: a primeira carga e a BASE (pagina inteira
/// reduzida, rapida, cobre tudo com o valor certo); o tile so sai quando o
/// visualizador informa a janela, na grade da tela. Nunca um tile da pagina
/// inteira na resolucao nativa (copia cheia da pagina + recorte guardado).
#[test]
fn open_with_adjustment_shows_the_base_and_the_tile_wait_the_window() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let g = ImageRef { source: "Livro".into(), file: "g.png".into(), page: 1 };
    st.app_data.set_adjustments(&g, highlight());
    let forward = st.adopt_source("Livro", fake(&[("g.png".into(), png_default(3000, 1000))])).unwrap();
    assert_eq!((forward.quality, forward.view_window, forward.image_adjustments.clone()), (Quality::Base, None, highlight()));
    let generation = forward.generation;
    match st.load_now(forward) {
        Received::Image(e, Reframe::Adjust, Layer::Base) => {
            assert_eq!((e.bgra.dimensions(), e.sz_original), ((2048, 683), (3000, 1000)))
        }
        _ => panic!("a base primeiro"),
    }
    assert!(st.base_display().is_some() && st.tile_display().is_none());
    assert!(!st.loading());
    assert_eq!(st.screen_adjustments(), Some(&highlight()));
    // a janela da tela (zoom 1: a pagina inteira na grade da tela)
    let j = ViewWindow { grid: (1500, 500), region: GridRegion { x0: 0, y0: 0, x1: 1500, y1: 500 } };
    assert!(st.set_window(j));
    let fi = st.set_adjustments_in(highlight(), Quality::Tile).unwrap();
    assert_eq!((fi.view_window, fi.generation), (Some(j), generation), "o tile dos mesmos valores nao abre geracao nova");
    match st.load_now(fi) {
        Received::Image(e, Reframe::Keep, Layer::Tile(jt)) => {
            assert_eq!((e.bgra.dimensions(), jt), ((1500, 500), j));
        }
        _ => panic!("o tile da janela"),
    }
    assert!(st.base_display().is_some(), "a base do mesmo valor fica por baixo");
}

#[test]
fn request_stale_not_delivers_result() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let p = st.adopt_source("Livro", fake(&names(3))).unwrap();
    image(st.load_now(p));
    let old = st.set_adjustments_in(Adjustments { invert: true, ..Adjustments::default() }, Quality::Tile).unwrap();
    let _new = st.set_adjustments_in(Adjustments { grayscale: true, ..Adjustments::default() }, Quality::Base).unwrap();
    assert!(matches!(load::execute(old).outcome, Outcome::Stale));
}

#[test]
fn interrupt_makes_the_final_in_flight_stale_and_the_result_its_falls() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let p = st.adopt_source("Livro", fake(&names(3))).unwrap();
    image(st.load_now(p));
    let inv = Adjustments { invert: true, ..Adjustments::default() };
    let fi = st.set_adjustments_in(inv, Quality::Tile).unwrap();
    st.interrupt_filter();
    let r = load::execute(fi);
    assert!(matches!(r.outcome, Outcome::Stale));
    assert!(matches!(st.receive(r), Received::Ignored));
    // o valor novo vem depois, com a previa e a final dele
    let gray = Adjustments { grayscale: true, ..Adjustments::default() };
    let (pv, fi) = preview_and_final(&mut st, gray.clone());
    image(st.load_now(pv));
    image(st.load_now(fi));
    assert_eq!(st.screen_adjustments(), Some(&gray));
    assert!(!st.loading());
}

/// De ponta a ponta: a final de uma pagina grande ja esta filtrando no fundo
/// quando o valor muda; ela para no meio (antes de terminar) e nao entrega nada.
/// O limite de latencia e folgado de proposito: em maquina carregada (CI) o teto de 500 ms oscilava;
/// o que o teste garante e o `Stale` (parou antes de terminar), nao o tempo exato.
#[test]
fn interrupt_to_the_final_in_the_middle_of_the_filter() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let mut out = Cursor::new(Vec::new());
    RgbaImage::from_pixel(3000, 2000, Rgba([10, 20, 30, 255])).write_to(&mut out, image::ImageFormat::Png).unwrap();
    let p = st.adopt_source("Livro", fake(&[("g.png".into(), out.into_inner())])).unwrap();
    image(st.load_now(p));
    let a = Adjustments { sharpen: 0.6, highpass: 0.7, ..Adjustments::default() };
    let fi = st.set_adjustments_in(a, Quality::Tile).unwrap();
    let job = std::thread::spawn(move || {
        let t0 = std::time::Instant::now();
        let r = load::execute(fi);
        (r, t0.elapsed())
    });
    // A pagina ja esta decodificada: o filtro comeca na hora e leva bem mais que isto.
    std::thread::sleep(Duration::from_millis(15));
    let t0 = std::time::Instant::now();
    st.interrupt_filter();
    let (r, total) = job.join().unwrap();
    let latency = t0.elapsed();
    assert!(matches!(r.outcome, Outcome::Stale), "a final terminou em {total:?} sem parar");
    assert!(latency < Duration::from_millis(3000), "parou {latency:?} depois da interrupcao");
    assert!(matches!(st.receive(r), Received::Ignored));
}

#[test]
fn preview_after_final_of_the_same_generation_stays_with_the_final() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let p = st.adopt_source("Livro", fake(&names(3))).unwrap();
    image(st.load_now(p));
    let inv = Adjustments { invert: true, ..Adjustments::default() };
    let (pv, fi) = preview_and_final(&mut st, inv.clone());
    let e = image(st.load_now(pv));
    assert_eq!(e.bgra.get_pixel(0, 0).0, [225, 235, 245, 255]);
    match st.load_now(fi) {
        Received::Image(e, fr, _) => {
            assert_eq!(e.bgra.get_pixel(0, 0).0, [225, 235, 245, 255]);
            assert_eq!(fr, Reframe::Keep);
        }
        _ => panic!("a final substitui a previa"),
    }
    assert!(!st.loading());
    assert_eq!(st.adjustments, inv);
}

#[test]
fn preview_late_not_replaces_the_final() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let p = st.adopt_source("Livro", fake(&names(3))).unwrap();
    image(st.load_now(p));
    let (pv, fi) = preview_and_final(&mut st, Adjustments { invert: true, ..Adjustments::default() });
    let r_pv = load::execute(pv);
    image(st.load_now(fi));
    let screen = st.tile_display().unwrap().0;
    assert!(matches!(st.receive(r_pv), Received::Layer(_)), "a base que chega depois do tile so vai para o fundo");
    assert_eq!(st.tile_display().unwrap().0, screen, "o tile continua na tela");
    assert!(st.base_display().is_some());
}

#[test]
fn first_load_with_preview_after_final_not_reframes() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let forward = st.adopt_source("Livro", fake(&names(3))).unwrap();
    assert_eq!(forward.frame, Reframe::Adjust);
    // quadro ainda nao chegou: a previa e a final o substituem e herdam o enquadramento
    let (pv, fi) = preview_and_final(&mut st, Adjustments { invert: true, ..Adjustments::default() });
    assert!(matches!(st.load_now(forward), Received::Ignored));
    match st.load_now(pv) {
        Received::Image(_, fr, _) => assert_eq!(fr, Reframe::Adjust),
        _ => panic!("previa"),
    }
    match st.load_now(fi) {
        Received::Image(_, fr, _) => assert_eq!(fr, Reframe::Keep, "a previa ja enquadrou"),
        _ => panic!("final"),
    }
}

/// O olho do painel desligado: a imagem aparece sem ajustes (nada apagado,
/// nem na memoria nem nos dados do app); trocar de imagem continua sem
/// ajustes na tela; ligar de novo mostra os ajustes da imagem atual.
#[test]
fn eye_disabled_shows_the_image_no_adjustments_no_erase_nothing() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let p = st.adopt_source("Livro", fake(&names(3))).unwrap();
    image(st.load_now(p));
    assert!(st.show_adjustments(), "a sessao comeca mostrando os ajustes");
    let inv = Adjustments { invert: true, ..Adjustments::default() };
    let p = st.set_adjustments(inv.clone()).unwrap();
    image(st.load_now(p));
    assert_eq!(st.screen_adjustments(), Some(&inv));

    st.set_show_adjustments(false);
    let p = st.set_adjustments_in(inv.clone(), Quality::Base).unwrap();
    assert_eq!(p.image_adjustments, Adjustments::default(), "a tela pede a imagem sem ajustes");
    assert_eq!(p.quality, Quality::Full, "sem ajuste na tela nao ha base nem tile");
    let e = image(st.load_now(p));
    assert_eq!(e.bgra.get_pixel(0, 0).0, [30, 20, 10, 255], "sem o inverter");
    assert_eq!(st.screen_adjustments(), Some(&Adjustments::default()));
    let p01 = ImageRef { source: "Livro".into(), file: "p01.png".into(), page: 1 };
    assert_eq!(st.adjustments, inv, "nada apagado na memoria");
    assert_eq!(st.app_data.adjustments(&p01), inv, "nem nos dados do app");

    // outra imagem com ajuste guardado, o olho ainda desligado: abre sem ajustes
    let gray = Adjustments { grayscale: true, ..Adjustments::default() };
    let p02 = ImageRef { source: "Livro".into(), file: "p02.png".into(), page: 1 };
    st.app_data.set_adjustments(&p02, gray.clone());
    let forward = st.go_next().unwrap();
    assert_eq!(forward.image_adjustments, Adjustments::default());
    image(st.load_now(forward));
    assert_eq!(st.adjustments, gray, "os ajustes da imagem nova estao la");

    // ligar de novo: o pedido leva os ajustes guardados (geracao nova: nada atrasado vale)
    let g = st.generation.current();
    st.set_show_adjustments(true);
    let p = st.set_adjustments_in(st.adjustments.clone(), Quality::Tile).unwrap();
    assert_eq!(p.image_adjustments, gray);
    assert!(p.generation > g);
    image(st.load_now(p));
    assert_eq!(st.screen_adjustments(), Some(&gray));
}

/// A previa pedida com o olho ligado e a final pedida com ele desligado (o
/// mesmo valor guardado) nao compartilham a geracao: a previa nao vale mais.
#[test]
fn preview_of_before_of_the_eye_change_not_is_worth() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let p = st.adopt_source("Livro", fake(&names(3))).unwrap();
    image(st.load_now(p));
    let inv = Adjustments { invert: true, ..Adjustments::default() };
    let pv = st.set_adjustments_in(inv.clone(), Quality::Base).unwrap();
    st.set_show_adjustments(false);
    let fi = st.set_adjustments_in(inv, Quality::Tile).unwrap();
    assert!(fi.generation > pv.generation);
    assert!(matches!(load::execute(pv).outcome, Outcome::Stale));
    image(st.load_now(fi));
}

#[test]
fn final_of_generation_smaller_and_discarded() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let p = st.adopt_source("Livro", fake(&names(3))).unwrap();
    image(st.load_now(p));
    let old = st.set_adjustments_in(Adjustments { invert: true, ..Adjustments::default() }, Quality::Tile).unwrap();
    let r_old = load::execute(old);
    let fresh = st.set_adjustments_in(Adjustments { grayscale: true, ..Adjustments::default() }, Quality::Base).unwrap();
    image(st.load_now(fresh));
    assert!(matches!(st.receive(r_old), Received::Ignored));
    // final nova de outros valores tambem derruba a previa pendente
    let pv = st.set_adjustments_in(Adjustments { sharpen: 0.5, ..Adjustments::default() }, Quality::Base).unwrap();
    let fi = st.set_adjustments_in(Adjustments { sharpen: 0.6, ..Adjustments::default() }, Quality::Tile).unwrap();
    assert!(fi.generation > pv.generation);
    assert!(matches!(st.load_now(pv), Received::Ignored));
    image(st.load_now(fi));
}

#[test]
fn switch_of_image_invalid_preview_and_final_pending() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let p = st.adopt_source("Livro", fake(&names(3))).unwrap();
    image(st.load_now(p));
    let (pv, fi) = preview_and_final(&mut st, Adjustments { invert: true, ..Adjustments::default() });
    let forward = st.go_next().unwrap();
    assert!(matches!(load::execute(pv).outcome, Outcome::Stale));
    assert!(matches!(load::execute(fi).outcome, Outcome::Stale));
    let e = image(st.load_now(forward));
    assert_eq!(e.bgra.get_pixel(0, 0).0, [30, 20, 10, 255], "p02 sem o ajuste da p01");
    assert_eq!(st.current.image.file, "p02.png");
}

#[test]
fn adjustments_not_touch_the_disk_in_the_thread_of_the_ui() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let p = st.adopt_source("Livro", fake(&names(3))).unwrap();
    image(st.load_now(p));
    let fname = t.path().join("appdata/appdata.json");
    let inv = Adjustments { invert: true, ..Adjustments::default() };
    // valor escolhido: ja vale na memoria (painel e menus o leem), sem pedido nem disco
    st.apply_adjustments(inv.clone());
    assert_eq!(st.adjustments, inv);
    assert_eq!(st.app_data.adjustments(&st.current.image), inv);
    assert!(!st.loading(), "definir nao pede filtro");
    let pv = st.set_adjustments_in(inv.clone(), Quality::Base).unwrap();
    let fi = st.set_adjustments_in(inv.clone(), Quality::Tile).unwrap();
    image(st.load_now(pv));
    image(st.load_now(fi));
    assert!(!fname.exists(), "previa e final nao gravam o appdata");
    // a gravacao e um instantaneo executado no fundo
    let g = st.app_data_write();
    st.apply_adjustments(Adjustments { grayscale: true, ..Adjustments::default() });
    g.execute().unwrap();
    let p01 = ImageRef { source: "Livro".into(), file: "p01.png".into(), page: 1 };
    assert_eq!(AppData::load(t.path().join("appdata")).unwrap().adjustments(&p01), inv, "o valor do instantaneo");
}

#[test]
fn write_more_old_not_overwrites_the_more_new() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    st.adopt_source("Livro", fake(&names(3))).unwrap();
    let p01 = ImageRef { source: "Livro".into(), file: "p01.png".into(), page: 1 };
    let inv = Adjustments { invert: true, ..Adjustments::default() };
    let gray = Adjustments { grayscale: true, ..Adjustments::default() };
    st.apply_adjustments(inv);
    let old = st.app_data_write();
    st.apply_adjustments(gray.clone());
    let fresh = st.app_data_write();
    fresh.execute().unwrap();
    old.execute().unwrap(); // chegou atrasada (outra thread do fundo)
    assert_eq!(AppData::load(t.path().join("appdata")).unwrap().adjustments(&p01), gray);
    // uma gravacao sincrona (posicao, vault) depois tambem conta como mais nova
    st.apply_adjustments(Adjustments::default());
    let late = st.app_data_write();
    st.apply_adjustments(gray.clone());
    st.save_app_data().unwrap();
    late.execute().unwrap();
    assert_eq!(AppData::load(t.path().join("appdata")).unwrap().adjustments(&p01), gray);
}

#[test]
fn app_data_pending_ate_the_write_more_new_arrive_to_the_disk() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    st.adopt_source("Livro", fake(&names(3))).unwrap();
    assert!(!st.pending_app_data());
    st.apply_adjustments(Adjustments { invert: true, ..Adjustments::default() });
    let g = st.app_data_write(); // emitida para o fundo, ainda nao terminou
    assert!(st.pending_app_data(), "sair agora perderia a gravacao em voo");
    g.execute().unwrap();
    assert!(!st.pending_app_data());
    // a gravacao do fundo falhou: continua pendente ate uma gravar
    std::fs::remove_dir_all(t.path().join("appdata")).unwrap();
    std::fs::write(t.path().join("appdata"), b"arquivo no lugar da pasta").unwrap();
    st.apply_adjustments(Adjustments { grayscale: true, ..Adjustments::default() });
    assert!(st.app_data_write().execute().is_err());
    assert!(st.pending_app_data(), "falha nao conta como gravada");
    std::fs::remove_file(t.path().join("appdata")).unwrap();
    st.save_app_data().unwrap();
    assert!(!st.pending_app_data());
    let p01 = ImageRef { source: "Livro".into(), file: "p01.png".into(), page: 1 };
    assert!(AppData::load(t.path().join("appdata")).unwrap().adjustments(&p01).grayscale);
}

#[test]
fn path_of_source_pass_by_the_queue_numbered() {
    // uma gravacao de ajustes montada ANTES e terminada DEPOIS nao desfaz o caminho novo
    let t = tempdir().unwrap();
    let folder = folder_png(&t.path().join("Livro"), 3);
    let mut st = with_vault(t.path());
    let vault = st.vault.clone().unwrap();
    let old = st.app_data_write();
    st.write_path(&vault.root, "Livro", &folder).unwrap();
    assert!(!st.pending_app_data());
    old.execute().unwrap();
    assert_eq!(AppData::load(t.path().join("appdata")).unwrap().source_path(&vault.root, "Livro"), Some(folder));
}

#[test]
fn write_after_of_rename_so_takes_the_memory() {
    // a gravacao pendente e so um instantaneo da memoria: o valor escolhido
    // depois da renomeacao fica no nome novo, nunca no antigo
    let t = tempdir().unwrap();
    let folder = folder_png(&t.path().join("Livro"), 3);
    let mut st = with_vault(t.path());
    let vault = st.vault.clone().unwrap();
    open_source::register(Some(&vault), &mut st.app_data, "Livro", SourceKind::Folder, &folder).unwrap();
    st.remember_path("Livro", folder);
    st.apply_adjustments(Adjustments { invert: true, ..Adjustments::default() });
    st.rename_source("Livro", "Novo").unwrap();
    let gray = Adjustments { grayscale: true, ..Adjustments::default() };
    st.apply_adjustments(gray.clone());
    st.app_data_write().execute().unwrap();
    let disk = AppData::load(t.path().join("appdata")).unwrap();
    let em = |f: &str| ImageRef { source: f.into(), file: "p01.png".into(), page: 1 };
    assert_eq!(disk.adjustments(&em("Novo")), gray);
    assert_ne!(disk.adjustments(&em("Livro")), gray, "o valor novo nunca vai para o nome antigo");
}

#[test]
fn final_of_the_same_page_keeps_the_copy_reduced() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let p = st.adopt_source("Livro", fake(&names(3))).unwrap();
    image(st.load_now(p));
    let stored = st.page.clone().unwrap();
    let fi = st.set_adjustments_in(Adjustments { invert: true, ..Adjustments::default() }, Quality::Tile).unwrap();
    let mut r = load::execute(fi);
    // a final veio com OUTRA `Page` sobre a mesma pagina decodificada
    if let Outcome::Ready { page, .. } = &mut r.outcome {
        *page = Arc::new(Page::new(Arc::clone(&stored.decoded)));
    }
    image(st.receive(r));
    assert!(Arc::ptr_eq(st.page.as_ref().unwrap(), &stored), "a copia reduzida nao e refeita");
}

#[test]
fn final_of_others_values_that_os_of_the_preview_gains_generation_new() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let p = st.adopt_source("Livro", fake(&names(3))).unwrap();
    image(st.load_now(p));
    let inv = Adjustments { invert: true, ..Adjustments::default() };
    let pv = st.set_adjustments_in(inv, Quality::Base).unwrap();
    // o usuario ja escolheu outro valor (memoria), e a final e desse valor
    let gray = Adjustments { grayscale: true, ..Adjustments::default() };
    st.apply_adjustments(gray.clone());
    let fi = st.set_adjustments_in(gray, Quality::Tile).unwrap();
    assert!(fi.generation > pv.generation, "a previa de outro valor nao vale mais");
}

fn fixture(name: &str) -> Vec<u8> {
    let p = concat!(env!("CARGO_MANIFEST_DIR"), "/../marginalia_core/integration/fixtures/");
    std::fs::read(format!("{p}{name}")).unwrap()
}

#[test]
fn navigate_before_of_know_the_total_of_pages_wait_and_resumes() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let source = fake(&[("a.tif".into(), fixture("duas.tif")), ("b.png".into(), png([1, 2, 3, 255]))]);
    let p0 = st.adopt_source("Livro", source).unwrap();
    // a.tif ainda carregando: o total de paginas e desconhecido, nao pode pular para b.png
    assert!(st.go_next().is_none());
    assert_eq!(st.current, Frame::new("Livro", "a.tif", 1));
    assert!(st.has_navigation_pending());
    image(st.load_now(p0));
    let p = st.resume_navigation().expect("retoma o go_next pendente");
    assert_eq!(st.current, Frame::new("Livro", "a.tif", 2));
    assert!(!st.has_navigation_pending());
    assert!(st.resume_navigation().is_none());
    image(st.load_now(p));
    assert_eq!(st.page(), (2, Some(2)));
    // com o total conhecido, navega na hora
    assert!(st.go_next().is_some());
    assert_eq!(st.current.image.file, "b.png");
}

#[test]
fn navigation_pending_is_worth_the_more_new_and_falls_to_the_open_other_frame() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let source = fake(&[("a.tif".into(), fixture("duas.tif")), ("b.png".into(), png([1, 2, 3, 255]))]);
    st.adopt_source("Livro", source.clone()).unwrap();
    assert!(st.go_next().is_none());
    assert!(st.go_prev().is_none());
    let p = st.open_frame(Frame::new("Livro", "a.tif", 1)).unwrap();
    assert!(!st.has_navigation_pending(), "abrir outro quadro descarta a intencao");
    image(st.load_now(p));
    assert!(st.resume_navigation().is_none());
    // a mais nova vale: next depois prev = prev (nao ha anterior a a.tif p1, entao nada)
    let p = st.adopt_source("Livro", source).unwrap();
    st.go_next();
    st.go_prev();
    image(st.load_now(p));
    assert!(st.resume_navigation().is_none());
    assert_eq!(st.current, Frame::new("Livro", "a.tif", 1));
}

#[test]
fn navigation_pending_about_file_corrupt_resumes_in_the_error() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let source = fake(&[("a.png".into(), b"corrompido".to_vec()), ("b.png".into(), png([1, 2, 3, 255]))]);
    let p0 = st.adopt_source("Livro", source).unwrap();
    assert!(st.go_next().is_none());
    assert!(matches!(st.load_now(p0), Received::Error(_)));
    assert!(st.resume_navigation().is_some());
    assert_eq!(st.current.image.file, "b.png");
    // e com o erro ja na tela, navega na hora
    let p = st.open_frame(Frame::new("Livro", "a.png", 1)).unwrap();
    assert!(matches!(st.load_now(p), Received::Error(_)));
    assert!(st.go_next().is_some());
}

#[test]
fn navigate_cleared_the_warning() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let p = st.adopt_source("Livro", fake(&names(3))).unwrap();
    image(st.load_now(p));
    st.warning = Some("velho".into());
    st.go_next().unwrap();
    assert_eq!(st.warning, None);
    // a gravacao dos ajustes (no fundo) que falhou avisa; a seguinte, com sucesso, limpa
    st.set_adjustments(Adjustments { invert: true, ..Adjustments::default() });
    st.wrote_adjustments(Err("disco cheio".into()));
    assert_eq!(st.warning.as_deref(), Some("nao foi possivel salvar os ajustes: disco cheio"));
    st.wrote_adjustments(Ok(()));
    assert_eq!(st.warning, None, "salvar com sucesso limpa o aviso");
    // um aviso de outra coisa, mais novo, nao some com a gravacao atrasada
    st.warning = Some("outro".into());
    st.wrote_adjustments(Ok(()));
    assert_eq!(st.warning.as_deref(), Some("outro"));
}

/// Fonte cuja leitura fica presa ate `release` (um NAS travado).
struct Stuck {
    entries: Vec<ImageEntry>,
    released: Mutex<bool>,
    cv: std::sync::Condvar,
    reading: std::sync::atomic::AtomicUsize,
}

impl Stuck {
    fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.cv.notify_all();
    }
}

impl ImageSource for Stuck {
    fn entries(&self) -> &[ImageEntry] {
        &self.entries
    }
    fn read(&self, _: &str) -> marginalia_core::vault::Result<Vec<u8>> {
        self.reading.fetch_add(1, Ordering::SeqCst);
        let mut l = self.released.lock().unwrap();
        while !*l {
            l = self.cv.wait(l).unwrap();
        }
        Ok(png([0, 0, 0, 255]))
    }
}

#[test]
fn drop_state_and_requests_not_wait_reading_stuck() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let source = Arc::new(Stuck {
        entries: (1..=8).map(|i| ImageEntry { path: format!("p{i}.png") }).collect(),
        released: Mutex::new(false),
        cv: std::sync::Condvar::new(),
        reading: Default::default(),
    });
    let request = st.adopt_source("Livro", source.clone()).unwrap();
    assert!(wait(|| source.reading.load(Ordering::SeqCst) >= 2), "workers presos na leitura");
    let f = source.clone();
    let drop_item = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(2));
        f.release();
    });
    let t0 = Instant::now();
    drop(request);
    drop(st);
    let spent = t0.elapsed();
    drop_item.join().unwrap();
    assert!(spent < Duration::from_millis(1500), "soltar esperou a leitura presa: {spent:?}");
}

#[test]
fn open_frame_uses_the_framing_and_navigate_back_the_adjust() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    st.adopt_source("Livro", fake(&names(3))).unwrap();
    let fr = Framing { zoom: 3.0, cx: 0.2, cy: 0.7 };
    let mut f = Frame::new("Livro", "p02.png", 1);
    f.framing = Some(fr.clone());
    let p = st.open_frame(f).unwrap();
    assert_eq!(p.frame, Reframe::Adopt(fr.clone()));
    // ajuste antes de a imagem chegar herda o enquadramento pendente
    let p = st.set_adjustments(Adjustments { grayscale: true, ..Adjustments::default() }).unwrap();
    assert_eq!(p.frame, Reframe::Adopt(fr));
    match st.load_now(p) {
        Received::Image(_, Reframe::Adopt(_), _) => {}
        _ => panic!("esperava imagem com o enquadramento"),
    }
    assert_eq!(st.go_next().unwrap().frame, Reframe::Adjust);
}

#[test]
fn open_frame_of_other_source_or_file_missing() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    assert!(st.open_frame(Frame::new("Livro", "p01.png", 1)).is_none(), "sem fonte aberta");
    st.adopt_source("Livro", fake(&names(2))).unwrap();
    assert!(st.open_frame(Frame::new("Outra", "p01.png", 1)).is_none());
    assert!(st.warning.is_some());
    let p = st.open_frame(Frame::new("Livro", "sumiu.png", 1)).unwrap();
    assert!(matches!(st.load_now(p), Received::Error(_)));
    assert!(st.error.is_some());
    // arquivo fora da fonte nao tem vizinhos; voltar ao inicio exige abrir um quadro valido
    assert!(st.go_next().is_none());
    assert!(st.open_frame(Frame::new("Livro", "p02.png", 1)).is_some());
}

#[test]
fn source_empty_of_the_error() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    assert!(st.adopt_source("Vazia", fake(&[])).is_none());
    assert!(st.error.is_some());
    assert!(st.go_next().is_none());
}

#[test]
fn open_source_no_vault_uses_path_of_the_session() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    assert!(st.open_source("Livro", ZipMode::Direct).is_err());
    let folder = t.path().join("Livro");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("a.png"), png([0, 0, 0, 255])).unwrap();
    st.remember_path("Livro", folder.clone());
    let a = st.open_source("Livro", ZipMode::Cache).unwrap();
    assert_eq!(a.file_path, folder);
    assert!(a.cache_dir.starts_with(t.path().join("cache")));
    let src = a.execute().unwrap();
    let p = st.adopt_source(&a.name, src).unwrap();
    image(st.load_now(p));
    assert_eq!(st.page(), (1, Some(1)));
}

#[test]
fn open_source_with_vault_resolve_by_the_app_data() {
    let t = tempdir().unwrap();
    let vault = Vault::open_or_create(&t.path().join("vault")).unwrap();
    let mut ad = AppData::new(t.path().join("ad"));
    ad.set_source_path(&vault.root, "Livro", t.path().join("nao-existe"));
    let mut st = AppState::new(Some(vault), ad, t.path().join("cache"));
    let a = st.open_source("Livro", ZipMode::Direct).unwrap();
    assert!(a.file_path.ends_with("nao-existe"), "resolve sem tocar o disco");
    let e = st.open_source_now("Livro", ZipMode::Direct).err().unwrap();
    assert_eq!(e, SourceError::AbsentSource { name: "Livro".into() });
    assert!(st.pending().unwrap().file_path.as_ref().is_some_and(|c| c.ends_with("nao-existe")));
    assert!(st.open_source("Outro", ZipMode::Direct).is_err());
}

// ---- Task 13: grifos, nota rapida, notas e tarefas ----

fn current_time() -> chrono::NaiveDateTime {
    chrono::NaiveDate::from_ymd_opt(2026, 11, 12).unwrap().and_hms_opt(14, 32, 0).unwrap()
}

/// Estado com vault em `t/vault` e a fonte falsa "Livro" (3 quadros) aberta no 1o.
fn with_vault(t: &std::path::Path) -> AppState {
    let vault = Vault::open_or_create(&t.join("vault")).unwrap();
    let mut st = AppState::new(Some(vault), AppData::new(t.join("appdata")), t.join("cache"));
    let p = st.adopt_source("Livro", fake(&names(3))).unwrap();
    image(st.load_now(p));
    st
}

fn ret(id: &str) -> Highlight {
    Highlight::Rectangle { id: id.into(), x: 0.1, y: 0.2, w: 0.3, h: 0.4, color: None }
}

fn note(c: Created) -> Note {
    match c {
        Created::Note(p) => p,
        Created::Task(t) => panic!("esperava nota, veio tarefa {t:?}"),
    }
}

#[test]
fn submit_note_creates_note_with_image_page_and_framing_of_the_frame_current() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    let p = st.go_next().unwrap();
    image(st.load_now(p));
    // o visualizador deu zoom e moveu: o estado espelha o enquadramento atual
    let fr = Framing { zoom: 2.5, cx: 0.3, cy: 0.6 };
    st.set_framing(fr.clone());
    let p = note(st.submit_quick_note("Maria herdeira", current_time()).unwrap());
    assert_eq!(p.id, "20261112-1432-maria-herdeira");
    assert_eq!(p.image, ImageRef { source: "Livro".into(), file: "p02.png".into(), page: 1 });
    assert_eq!(p.framing, fr);
    assert_eq!(p.highlight, None);
    let fname = t.path().join("vault/notas/20261112-1432-maria-herdeira.md");
    let doc = std::fs::read_to_string(fname).unwrap();
    assert!(doc.contains("file: p02.png") && doc.ends_with("---\nMaria herdeira"), "{doc}");
    assert!(st.confirmation.as_deref().is_some_and(|c| c.contains(&p.id)));
}

#[test]
fn framing_mirrored_follows_the_frame_open() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    assert_eq!(st.framing(), &viewer_math::default_framing());
    let fr = Framing { zoom: 4.0, cx: 0.1, cy: 0.9 };
    let mut f = Frame::new("Livro", "p03.png", 1);
    f.framing = Some(fr.clone());
    let p = st.open_frame(f).unwrap();
    assert_eq!(st.framing(), &fr, "uma nota criada logo apos abrir usa o enquadramento aberto");
    image(st.load_now(p));
    st.go_prev().unwrap();
    assert_eq!(st.framing(), &viewer_math::default_framing(), "navegar volta a ajustar");
    // fora dos limites e limitado como no visualizador
    st.set_framing(Framing { zoom: 99.0, cx: -1.0, cy: f32::NAN });
    assert_eq!(st.framing(), &Framing { zoom: viewer_math::MAX_ZOOM, cx: 0.0, cy: 0.5 });
}

#[test]
fn commit_highlight_writes_and_the_next_note_takes_the_highlight() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    st.commit_highlight(ret("g1")).unwrap();
    let f = HighlightsFile::load(st.vault.as_ref().unwrap(), "Livro").unwrap();
    let img01 = ImageRef { source: "Livro".into(), file: "p01.png".into(), page: 1 };
    assert_eq!(f.for_image(&img01), &[ret("g1")]);
    assert_eq!(st.image_highlights(), &[ret("g1")]);
    assert_eq!(st.pending_highlight(), Some(&ret("g1")));

    let p = note(st.submit_quick_note("Joao padrinho", current_time()).unwrap());
    assert_eq!(p.highlight.as_deref(), Some("g1"));
    assert_eq!(st.pending_highlight(), None, "o grifo pendente e consumido");
    let doc = std::fs::read_to_string(t.path().join(format!("vault/notas/{}.md", p.id))).unwrap();
    assert!(doc.contains("highlight: g1"), "{doc}");

    let p2 = note(st.submit_quick_note("outra", current_time()).unwrap());
    assert_eq!(p2.highlight, None);
    // um segundo grifo na mesma imagem soma ao arquivo
    st.commit_highlight(Highlight::Pointer { id: "g2".into(), x: 0.5, y: 0.5, tail: None, color: None }).unwrap();
    assert_eq!(HighlightsFile::load(st.vault.as_ref().unwrap(), "Livro").unwrap().for_image(&img01).len(), 2);
}

#[test]
fn highlight_pending_falls_to_the_navigate_to_other_image() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    st.commit_highlight(ret("g1")).unwrap();
    let p = st.go_next().unwrap();
    assert_eq!(st.pending_highlight(), None);
    image(st.load_now(p));
    assert!(st.image_highlights().is_empty(), "p02 nao tem grifos");
    let p = note(st.submit_quick_note("na p02", current_time()).unwrap());
    assert_eq!(p.highlight, None, "o grifo da p01 nao vai para uma nota da p02");
    // reabrir a mesma imagem (ajuste de filtro nao navega) mantem o pendente
    st.go_prev().unwrap();
    st.commit_highlight(ret("g2")).unwrap();
    st.set_adjustments(Adjustments { invert: true, ..Adjustments::default() });
    assert_eq!(st.pending_highlight(), Some(&ret("g2")));
}

#[test]
fn brackets_create_task_linked_the_image_current() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    st.commit_highlight(ret("g1")).unwrap();
    match st.submit_quick_note("[] achar óbito", current_time()).unwrap() {
        Created::Task(tf) => {
            assert_eq!(tf.id, "20261112-1432-achar-obito");
            assert_eq!(tf.image, Some(st.current.image.clone()));
            let doc = std::fs::read_to_string(t.path().join(format!("vault/tarefas/{}.md", tf.id))).unwrap();
            assert!(doc.ends_with("---\nachar óbito"), "{doc}");
        }
        Created::Note(p) => panic!("esperava tarefa, veio {p:?}"),
    }
    assert_eq!(st.pending_highlight(), Some(&ret("g1")), "tarefa nao consome o grifo pendente");
    assert!(st.confirmation.as_deref().is_some_and(|c| c.starts_with("tarefa criada")));
}

#[test]
fn text_empty_or_so_spaces_not_creates_nothing() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    st.commit_highlight(ret("g1")).unwrap();
    for s in ["", "   ", " \n\t ", "[]", "  []   "] {
        assert_eq!(st.submit_quick_note(s, current_time()).unwrap_err(), QuickNoteError::Empty, "{s:?}");
    }
    let v = st.vault.as_ref().unwrap();
    assert!(v.list_notes().unwrap().is_empty());
    assert!(v.list_tasks().unwrap().is_empty());
    assert_eq!(st.pending_highlight(), Some(&ret("g1")));
}

#[test]
fn no_vault_or_no_image_nothing_and_written() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    st.adopt_source("Livro", fake(&names(2))).unwrap();
    assert_eq!(st.submit_quick_note("x", current_time()).unwrap_err(), QuickNoteError::NoVault);
    assert!(st.commit_highlight(ret("g")).is_err());
    assert_eq!(st.pending_highlight(), None);

    let vault = Vault::open_or_create(&t.path().join("v")).unwrap();
    let mut st = AppState::new(Some(vault), AppData::new(t.path().join("ad")), t.path().join("c"));
    assert_eq!(st.submit_quick_note("x", current_time()).unwrap_err(), QuickNoteError::NoImage);
    assert!(st.commit_highlight(ret("g")).is_err());
}

#[test]
fn failure_to_the_write_returns_error_and_keeps_the_highlight_pending() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    st.commit_highlight(ret("g1")).unwrap();
    // notas/ vira arquivo: a gravacao falha
    let notes = t.path().join("vault/notas");
    std::fs::remove_dir_all(&notes).unwrap();
    std::fs::write(&notes, "nao e pasta").unwrap();
    match st.submit_quick_note("Maria", current_time()) {
        Err(QuickNoteError::Write(m)) => assert!(!m.is_empty()),
        o => panic!("esperava erro de gravacao: {o:?}"),
    }
    assert_eq!(st.pending_highlight(), Some(&ret("g1")), "nada perdido: tenta de novo depois");
}

#[test]
fn highlights_unreadable_not_are_overwritten() {
    let t = tempdir().unwrap();
    let vault = Vault::open_or_create(&t.path().join("vault")).unwrap();
    let bad = t.path().join("vault/grifos/Livro.json");
    std::fs::write(&bad, "{ isto nao e json").unwrap();
    let mut st = AppState::new(Some(vault), AppData::new(t.path().join("ad")), t.path().join("c"));
    st.adopt_source("Livro", fake(&names(2))).unwrap();
    assert!(st.warning.as_deref().is_some_and(|a| a.contains("grifos")), "{:?}", st.warning);
    assert!(st.commit_highlight(ret("g")).is_err());
    assert_eq!(std::fs::read_to_string(&bad).unwrap(), "{ isto nao e json");
    // grifo fora de 0..1 tambem e recusado sem gravar
    let t2 = tempdir().unwrap();
    let mut st = with_vault(t2.path());
    assert!(st.commit_highlight(Highlight::Pointer { id: "x".into(), x: 1.5, y: 0.0, tail: None, color: None }).is_err());
    assert!(st.image_highlights().is_empty());
    assert_eq!(st.pending_highlight(), None);
}

// ---- v1.1 Task 4: selecao, exclusao com desfazer e cor de grifos ----

fn img01() -> ImageRef {
    ImageRef { source: "Livro".into(), file: "p01.png".into(), page: 1 }
}
fn of_disk(st: &AppState) -> HighlightsFile {
    HighlightsFile::load(st.vault.as_ref().unwrap(), "Livro").unwrap()
}
/// Faz o arquivo de grifos nao gravar: `highlights/` vira arquivo.
fn break_highlights(t: &std::path::Path) {
    let g = t.join("vault/grifos");
    std::fs::remove_dir_all(&g).unwrap();
    std::fs::write(&g, "nao e pasta").unwrap();
}

#[test]
fn select_so_accepts_highlight_of_the_image_current_and_cleared_to_the_switch_of_image() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    st.commit_highlight(ret("g1")).unwrap();
    st.select(Some("g1".into()));
    assert_eq!(st.annot.selected.as_deref(), Some("g1"));
    st.select(Some("nao-existe".into()));
    assert_eq!(st.annot.selected, None, "id desconhecido nao seleciona");
    st.select(Some("g1".into()));
    st.select(None);
    assert_eq!(st.annot.selected, None);
    st.select(Some("g1".into()));
    let p = st.go_next().unwrap();
    assert_eq!(st.annot.selected, None, "trocar de imagem limpa a selecao");
    image(st.load_now(p));
    st.go_prev().unwrap();
    assert_eq!(st.annot.selected, None, "voltar nao ressuscita a selecao");
}

#[test]
fn switch_of_source_or_vault_cleared_selection_and_undo() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    st.commit_highlight(ret("g1")).unwrap();
    st.select(Some("g1".into()));
    st.delete_highlight("g1").unwrap();
    st.commit_highlight(ret("g2")).unwrap();
    st.select(Some("g2".into()));
    st.adopt_source("Livro", fake(&names(3))).unwrap();
    assert_eq!(st.annot.selected, None);
    assert_eq!(st.undo_highlight(), Ok(false), "o desfazer ficou na fonte anterior");
}

#[test]
fn delete_writes_cleared_selection_emphasis_and_pending() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    st.commit_highlight(ret("g1")).unwrap();
    st.commit_highlight(ret("g2")).unwrap(); // g2 e o pendente
    st.select(Some("g2".into()));
    st.highlight(Some("g2".into()));
    let v0 = st.annot.highlights_version;
    st.delete_highlight("g2").unwrap();
    assert_eq!(st.image_highlights(), &[ret("g1")]);
    assert_eq!(of_disk(&st).for_image(&img01()), &[ret("g1")], "gravado");
    assert_eq!(st.annot.selected, None);
    assert_eq!(st.annot.emphasis, None);
    assert_eq!(st.pending_highlight(), None);
    assert!(st.annot.highlights_version > v0);
    // excluir outro grifo nao mexe no que aponta para um terceiro
    st.commit_highlight(ret("g3")).unwrap();
    st.select(Some("g3".into()));
    st.highlight(Some("g3".into()));
    st.delete_highlight("g1").unwrap();
    assert_eq!(st.annot.selected.as_deref(), Some("g3"));
    assert_eq!(st.annot.emphasis.as_deref(), Some("g3"));
    assert_eq!(st.pending_highlight(), Some(&ret("g3")));
    assert!(st.delete_highlight("nada").is_err());
}

#[test]
fn undo_returns_the_highlight_with_same_id_and_color_and_so_a_time() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    assert_eq!(st.undo_highlight(), Ok(false), "nada para desfazer");
    let green = ret("g1").with_color("#43a047");
    st.commit_highlight(green.clone()).unwrap();
    st.commit_highlight(ret("g2")).unwrap();
    st.delete_highlight("g1").unwrap();
    st.delete_highlight("g2").unwrap(); // so um nivel: vale o ultimo
    assert_eq!(st.undo_highlight(), Ok(true));
    assert_eq!(st.image_highlights(), &[ret("g2")]);
    assert_eq!(of_disk(&st).for_image(&img01()), &[ret("g2")]);
    assert_eq!(st.annot.selected.as_deref(), Some("g2"), "o grifo devolvido fica selecionado");
    assert_eq!(st.undo_highlight(), Ok(false));
    // mesmo id e cor
    st.delete_highlight("g2").unwrap();
    st.commit_highlight(green.clone()).unwrap();
    st.delete_highlight("g1").unwrap();
    assert_eq!(st.undo_highlight(), Ok(true));
    let g = st.image_highlights().iter().find(|g| g.id() == "g1").unwrap().clone();
    assert_eq!(g, green);
    assert_eq!(g.color(), "#43a047");
    assert_eq!(of_disk(&st).find(&img01(), "g1"), Some(&green));
}

#[test]
fn undo_not_leaks_to_other_image() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    st.commit_highlight(ret("g1")).unwrap();
    st.delete_highlight("g1").unwrap();
    let p = st.go_next().unwrap();
    image(st.load_now(p));
    assert_eq!(st.undo_highlight(), Ok(false));
    assert!(st.image_highlights().is_empty());
    assert!(of_disk(&st).for_image(&img01()).is_empty(), "nada foi devolvido a p01");
    let p = st.go_prev().unwrap();
    image(st.load_now(p));
    assert_eq!(st.undo_highlight(), Ok(false), "a posicao de desfazer caiu ao sair da imagem");
}

#[test]
fn can_undo_highlight_so_in_the_same_image_and_ate_undo() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    assert!(!st.can_undo_highlight());
    st.commit_highlight(ret("g1")).unwrap();
    st.delete_highlight("g1").unwrap();
    assert!(st.can_undo_highlight());
    assert_eq!(st.undo_highlight(), Ok(true));
    assert!(!st.can_undo_highlight(), "uma vez so");
    st.delete_highlight("g1").unwrap();
    let p = st.go_next().unwrap();
    image(st.load_now(p));
    assert!(!st.can_undo_highlight(), "outra imagem: Cmd+Z nao vale");
}

#[test]
fn close_source_open_cleared_frame_highlights_and_keeps_the_position() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    st.commit_highlight(ret("g1")).unwrap();
    st.delete_highlight("g1").unwrap();
    let p = st.go_next().unwrap();
    image(st.load_now(p));
    let (file, name) = (st.current.image.file.clone(), st.current.image.source.clone());
    let root_dir = st.vault.as_ref().unwrap().root.clone();
    assert_eq!(st.close_source_open().as_deref(), Some(name.as_str()));
    assert!(!st.source_has());
    assert!(st.image_highlights().is_empty() && st.annot.selected.is_none() && !st.can_undo_highlight());
    assert!(st.error.is_none() && st.pending().is_none() && st.go_next().is_none());
    // a posicao foi para o disco: reabrir a fonte volta a essa imagem
    let was_read = AppData::load(st.app_data.base().to_path_buf()).unwrap();
    assert_eq!(was_read.position(&root_dir, &name).map(|(a, _)| a), Some(file));
    assert_eq!(st.close_source_open(), None, "sem fonte: nada a fechar");
}

#[test]
fn undo_refuses_id_that_returned_the_exist() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    st.commit_highlight(ret("g1")).unwrap();
    st.delete_highlight("g1").unwrap();
    st.commit_highlight(ret("g1")).unwrap();
    assert!(st.undo_highlight().is_err());
    assert_eq!(st.image_highlights(), &[ret("g1")]);
}

#[test]
fn note_with_highlight_erased_opens_no_emphasis() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    st.commit_highlight(ret("g1")).unwrap();
    let p = note(st.submit_quick_note("achei", current_time()).unwrap());
    assert_eq!(p.highlight.as_deref(), Some("g1"));
    st.delete_highlight("g1").unwrap();
    let request = image_frame(st.open_note(&p.id).unwrap());
    assert_eq!(st.current.image, p.image);
    assert_eq!(request.frame, Reframe::Adopt(p.framing.clone()));
    assert_eq!(st.emphasis(), None);
    assert_eq!(st.error, None);
    assert_eq!(st.warning, None);
    // desfazer devolve o grifo e a nota volta a destacar
    st.undo_highlight().unwrap();
    image_frame(st.open_note(&p.id).unwrap());
    assert_eq!(st.emphasis(), Some("g1"));
}

#[test]
fn failure_to_the_write_the_deletion_keeps_all_in_the_memory() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    st.commit_highlight(ret("g1")).unwrap();
    st.select(Some("g1".into()));
    st.highlight(Some("g1".into()));
    let v0 = st.annot.highlights_version;
    break_highlights(t.path());
    assert!(st.delete_highlight("g1").unwrap_err().contains("gravar"));
    assert_eq!(st.image_highlights(), &[ret("g1")]);
    assert_eq!(st.annot.selected.as_deref(), Some("g1"));
    assert_eq!(st.emphasis(), Some("g1"));
    assert_eq!(st.pending_highlight(), Some(&ret("g1")));
    assert_eq!(st.annot.highlights_version, v0);
    assert_eq!(st.undo_highlight(), Ok(false), "nada foi apagado, nada a desfazer");
}

#[test]
fn failure_to_the_write_the_undo_keeps_the_position_of_undo() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    st.commit_highlight(ret("g1")).unwrap();
    st.delete_highlight("g1").unwrap();
    break_highlights(t.path());
    assert!(st.undo_highlight().is_err());
    assert!(st.image_highlights().is_empty());
    std::fs::remove_file(t.path().join("vault/grifos")).unwrap();
    std::fs::create_dir(t.path().join("vault/grifos")).unwrap();
    assert_eq!(st.undo_highlight(), Ok(true), "tenta de novo depois");
}

#[test]
fn highlights_unreadable_refuse_delete_undo_and_recolor() {
    let t = tempdir().unwrap();
    let vault = Vault::open_or_create(&t.path().join("vault")).unwrap();
    let bad = t.path().join("vault/grifos/Livro.json");
    std::fs::write(&bad, "{ isto nao e json").unwrap();
    let mut st = AppState::new(Some(vault), AppData::new(t.path().join("ad")), t.path().join("c"));
    st.adopt_source("Livro", fake(&names(2))).unwrap();
    assert!(st.delete_highlight("g").is_err());
    st.annot.last_highlight_erased = Some((img01(), ret("g")));
    assert!(st.undo_highlight().is_err());
    assert_eq!(std::fs::read_to_string(&bad).unwrap(), "{ isto nao e json");
    st.set_color("#e53935").unwrap();
    assert_eq!(st.current_color(), "#e53935", "so a cor padrao muda; nenhum grifo e tocado");
    assert_eq!(std::fs::read_to_string(&bad).unwrap(), "{ isto nao e json");
}

#[test]
fn highlights_new_born_with_the_color_current() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    assert_eq!(st.current_color(), "#ffc400");
    st.set_color("#1e88e5").unwrap();
    st.commit_highlight(ret("g1")).unwrap();
    assert_eq!(st.image_highlights()[0].color(), "#1e88e5");
    assert_eq!(of_disk(&st).find(&img01(), "g1").unwrap().color(), "#1e88e5");
    // cor explicita do grifo vence a atual
    st.commit_highlight(ret("g2").with_color("#43a047")).unwrap();
    assert_eq!(st.image_highlights()[1].color(), "#43a047");
    // cor padrao: nada guardado no grifo (arquivo continua como antes)
    st.set_color("#ffc400").unwrap();
    st.commit_highlight(ret("g3")).unwrap();
    assert_eq!(st.image_highlights()[2], ret("g3"));
    assert_eq!(st.image_highlights()[2].color(), "#ffc400");
}

#[test]
fn set_color_with_selection_recolors_so_ela_and_persists() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    st.commit_highlight(ret("g1")).unwrap();
    st.commit_highlight(ret("g2")).unwrap();
    st.select(Some("g1".into()));
    let v0 = st.annot.highlights_version;
    st.set_color("#d81b60").unwrap();
    assert_eq!(st.current_color(), "#d81b60");
    assert!(st.annot.highlights_version > v0);
    let f = of_disk(&st);
    assert_eq!(f.find(&img01(), "g1").unwrap().color(), "#d81b60", "recolorido e gravado");
    assert_eq!(f.find(&img01(), "g2").unwrap().color(), "#ffc400", "o outro nao muda");
    assert_eq!(st.annot.selected.as_deref(), Some("g1"), "a selecao continua");
    // a cor padrao persiste nos dados do app
    let ad = AppData::load(t.path().join("appdata")).unwrap();
    assert_eq!(ad.highlight_color(), "#d81b60");
}

#[test]
fn recolor_to_the_color_default_keeps_no_color() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    st.commit_highlight(ret("g1").with_color("#e53935")).unwrap();
    st.select(Some("g1".into()));
    st.set_color("#ffc400").unwrap();
    assert_eq!(st.image_highlights(), &[ret("g1")], "None, nao #ffc400 explicito");
    assert_eq!(st.pending_highlight(), Some(&ret("g1")), "o pendente acompanha");
    let raw = std::fs::read_to_string(t.path().join("vault/grifos/Livro.json")).unwrap();
    assert!(!raw.contains("cor"), "{raw}");
    assert_eq!(st.current_color(), "#ffc400");
}

#[test]
fn set_color_no_selection_so_changes_the_default_persisted() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    st.commit_highlight(ret("g1")).unwrap();
    let before = of_disk(&st);
    let v0 = st.annot.highlights_version;
    st.set_color("#43a047").unwrap();
    assert_eq!(of_disk(&st), before, "nenhum grifo mexido");
    assert_eq!(st.annot.highlights_version, v0);
    assert_eq!(st.image_highlights()[0].color(), "#ffc400");
    let ad = AppData::load(t.path().join("appdata")).unwrap();
    assert_eq!(ad.highlight_color(), "#43a047");
}

#[test]
fn color_outside_of_the_palette_and_refused_no_touch_in_nothing() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    st.commit_highlight(ret("g1")).unwrap();
    st.select(Some("g1".into()));
    for bad in ["#123456", "", "vermelho", "#FFC400"] {
        assert!(st.set_color(bad).is_err(), "{bad:?}");
    }
    assert_eq!(st.current_color(), "#ffc400");
    assert_eq!(st.image_highlights(), &[ret("g1")]);
    assert_eq!(of_disk(&st).for_image(&img01()), &[ret("g1")]);
}

#[test]
fn failure_to_the_recolor_keeps_the_state_and_the_color_default() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    st.commit_highlight(ret("g1")).unwrap();
    st.select(Some("g1".into()));
    let v0 = st.annot.highlights_version;
    break_highlights(t.path());
    assert!(st.set_color("#e53935").unwrap_err().contains("gravar"));
    assert_eq!(st.image_highlights(), &[ret("g1")]);
    assert_eq!(st.current_color(), "#ffc400", "a cor padrao so muda se o grifo gravou");
    assert_eq!(st.annot.highlights_version, v0);
    assert_eq!(st.annot.selected.as_deref(), Some("g1"));
}

#[test]
fn highlight_no_id_and_refused() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    assert!(st.commit_highlight(ret("")).is_err());
    assert!(st.image_highlights().is_empty());
}

#[test]
fn switch_id_swap_note_open_and_pending() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    let img = st.current.image.clone();
    st.note_open = Some(("A".into(), img));
    let mut p = Pending::fresh("Livro", None, Reason::Unavailable);
    p.note = Some("A".into());
    st.pending = Some(p);
    st.switch_id("X", "Y");
    assert_eq!(st.note_open(), Some("A"), "outro id: nada muda");
    st.switch_id("A", "N");
    assert_eq!(st.note_open(), Some("N"));
    assert_eq!(st.pending().and_then(|p| p.note.as_deref()), Some("N"));
}

fn image_frame(a: OpenNote) -> Request {
    match a {
        OpenNote::ImageFrame(Some(p)) => p,
        OpenNote::ImageFrame(None) => panic!("sem pedido"),
        OpenNote::Source { .. } => panic!("esperava a mesma fonte"),
    }
}

#[test]
fn open_note_no_highlight_opens_the_image_in_the_framing_no_emphasis() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    st.go_next().unwrap();
    let fr = Framing { zoom: 3.0, cx: 0.2, cy: 0.8 };
    st.set_framing(fr.clone());
    let p = note(st.submit_quick_note("sem grifo", current_time()).unwrap());
    let pd = st.go_next();
    assert!(pd.is_none(), "p02 ainda carregando: a navegacao espera");
    st.open_frame(Frame::new("Livro", "p03.png", 1)).unwrap();
    assert_eq!(st.current.image.file, "p03.png");

    let request = image_frame(st.open_note(&p.id).unwrap());
    assert_eq!(request.frame, Reframe::Adopt(fr.clone()));
    assert_eq!(st.current.image, p.image);
    assert_eq!(st.framing(), &fr);
    assert_eq!(st.emphasis(), None);
    assert_eq!(st.warning, None);
    image(st.load_now(request));
}

#[test]
fn open_note_highlights_the_highlight_and_navigate_removes_the_emphasis() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    st.commit_highlight(ret("g1")).unwrap();
    let p = note(st.submit_quick_note("com grifo", current_time()).unwrap());
    let pd = st.go_next().unwrap();
    image(st.load_now(pd));
    let pd = image_frame(st.open_note(&p.id).unwrap());
    assert_eq!(st.emphasis(), Some("g1"));
    image(st.load_now(pd));
    assert_eq!(st.emphasis(), Some("g1"), "o destaque fica ate navegar");
    st.go_next().unwrap();
    assert_eq!(st.emphasis(), None);
}

#[test]
fn open_note_whose_highlight_was_erased_opens_no_emphasis_and_no_error() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    st.commit_highlight(ret("g1")).unwrap();
    let p = note(st.submit_quick_note("grifo some", current_time()).unwrap());
    // o grifo e apagado do arquivo (outra maquina, edicao manual)
    let vault = st.vault.clone().unwrap();
    let mut f = HighlightsFile::load(&vault, "Livro").unwrap();
    f.remove(&p.image, "g1");
    f.save(&vault, "Livro").unwrap();
    let mut st = AppState::new(Some(vault), AppData::new(t.path().join("appdata")), t.path().join("c2"));
    st.adopt_source("Livro", fake(&names(3))).unwrap();
    st.open_frame(Frame::new("Livro", "p03.png", 1)).unwrap();

    let request = image_frame(st.open_note(&p.id).unwrap());
    assert_eq!(st.current.image, p.image);
    assert_eq!(request.frame, Reframe::Adopt(p.framing.clone()));
    assert_eq!(st.emphasis(), None);
    assert_eq!(st.error, None);
    assert_eq!(st.warning, None);
}

#[test]
fn open_note_of_source_no_path_or_missing_of_the_error_light() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    let vault = st.vault.clone().unwrap();
    let fr = Framing { zoom: 1.0, cx: 0.5, cy: 0.5 };
    let img = |f: &str| ImageRef { source: f.into(), file: "a.jpg".into(), page: 1 };
    let no = vault.create_note("sem caminho", img("Outra"), fr.clone(), None, current_time()).unwrap();
    let vanished = vault.create_note("sumiu", img("Sumida"), fr, None, current_time()).unwrap();
    st.app_data.set_source_path(&vault.root, "Sumida", t.path().join("nao-existe"));
    let before = st.current.clone();

    let e = st.open_note(&no.id).err().expect("fonte sem caminho");
    assert_eq!(e, SourceError::AbsentSource { name: "Outra".into() });
    assert!(e.to_string().contains("Outra") && e.to_string().contains("localizacao"), "{e}");
    assert_eq!(st.pending().unwrap().reason, Reason::NoPath);
    let r = st.open_note(&vanished.id);
    let e = open_now(&mut st, r).err().expect("caminho inexistente");
    assert_eq!(e, SourceError::AbsentSource { name: "Sumida".into() });
    assert!(st.pending().unwrap().file_path.as_ref().is_some_and(|c| c.ends_with("nao-existe")));
    assert!(st.open_note("nao-existe").is_err());
    let tf = vault.create_task("uma tarefa", None, None, current_time()).unwrap();
    assert!(st.open_note(&tf.id).err().unwrap().to_string().contains("nota"));
    assert_eq!(st.current, before, "nada mudou");
}

#[test]
fn open_note_of_other_source_opens_the_source_and_goes_to_the_frame() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    let vault = st.vault.clone().unwrap();
    let folder = t.path().join("Outra");
    std::fs::create_dir(&folder).unwrap();
    for n in ["a.png", "b.png"] {
        std::fs::write(folder.join(n), png([9, 9, 9, 255])).unwrap();
    }
    st.app_data.set_source_path(&vault.root, "Outra", folder.clone());
    let fr = Framing { zoom: 2.0, cx: 0.4, cy: 0.4 };
    let img_b = ImageRef { source: "Outra".into(), file: "b.png".into(), page: 1 };
    let p = vault.create_note("na outra", img_b.clone(), fr.clone(), Some("gx".into()), current_time()).unwrap();

    let (opening, f, highlight) = match st.open_note(&p.id).unwrap() {
        OpenNote::Source { opening, image_frame, highlight } => (opening, image_frame, highlight),
        OpenNote::ImageFrame(_) => panic!("esperava abrir a fonte"),
    };
    assert_eq!(opening.file_path, folder);
    assert_eq!(highlight.as_deref(), Some("gx"));
    let src = opening.execute().unwrap();
    let request = st.adopt_source_in_the(&opening.name, src, Some(f)).unwrap();
    st.highlight(highlight);
    assert_eq!(st.current.image, img_b);
    assert_eq!(request.frame, Reframe::Adopt(fr));
    assert_eq!(st.emphasis(), None, "gx nao existe nos grifos de Outra");
    image(st.load_now(request));
}

// ---- fix round 1 ----

#[test]
fn mirror_of_the_viewer_so_is_worth_with_the_image_current_in_the_screen() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    let a = Framing { zoom: 2.0, cx: 0.3, cy: 0.3 };
    assert!(st.mirror_framing(a.clone()), "p01 na tela: vale");
    assert_eq!(st.framing(), &a);
    // p02 carregando: o visualizador ainda mostra a p01 com o enquadramento dela
    let p = st.go_next().unwrap();
    let old = Framing { zoom: 5.0, cx: 0.9, cy: 0.9 };
    assert!(!st.mirror_framing(old.clone()));
    assert_eq!(st.framing(), &viewer_math::default_framing(), "vale o alvo do quadro novo");
    let note_early = note(st.submit_quick_note("cedo", current_time()).unwrap());
    assert_eq!(note_early.image.file, "p02.png");
    assert_eq!(note_early.framing, viewer_math::default_framing());
    image(st.load_now(p));
    assert!(st.mirror_framing(old.clone()), "p02 chegou: vale");
    // reaplicar filtro nao troca a imagem da tela: o espelho continua valendo
    st.set_adjustments(Adjustments { invert: true, ..Adjustments::default() }).unwrap();
    assert!(st.mirror_framing(a.clone()));
    // quadro aberto com enquadramento guardado, ainda carregando: vale o guardado
    let target = Framing { zoom: 3.0, cx: 0.6, cy: 0.1 };
    let mut f = Frame::new("Livro", "p03.png", 1);
    f.framing = Some(target.clone());
    st.open_frame(f).unwrap();
    assert!(!st.mirror_framing(old));
    assert_eq!(st.framing(), &target);
}

#[test]
fn mirror_ignored_with_error_in_the_frame() {
    let t = tempdir().unwrap();
    let vault = Vault::open_or_create(&t.path().join("vault")).unwrap();
    let mut st = AppState::new(Some(vault), AppData::new(t.path().join("ad")), t.path().join("c"));
    let p = st.adopt_source("Livro", fake(&[("a.png".into(), b"ruim".to_vec())])).unwrap();
    assert!(matches!(st.load_now(p), Received::Error(_)));
    assert!(!st.mirror_framing(Framing { zoom: 4.0, cx: 0.2, cy: 0.2 }));
    assert_eq!(st.framing(), &viewer_math::default_framing());
}

#[test]
fn highlight_drawn_in_other_image_and_refused() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    let p01 = st.current.image.clone();
    st.go_next().unwrap();
    let e = st.commit_highlight_in(&p01, ret("g1")).unwrap_err();
    assert!(e.contains("outra imagem"), "{e}");
    assert!(st.image_highlights().is_empty());
    assert_eq!(st.pending_highlight(), None);
    let f = HighlightsFile::load(st.vault.as_ref().unwrap(), "Livro").unwrap();
    assert!(f.for_image(&p01).is_empty(), "nada gravado");
    let current = st.current.image.clone();
    st.commit_highlight_in(&current, ret("g2")).unwrap();
    assert_eq!(st.pending_highlight(), Some(&ret("g2")));
}

#[test]
fn highlight_with_id_repeated_and_refused() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    st.commit_highlight(ret("g1")).unwrap();
    st.submit_quick_note("leva g1", current_time()).unwrap();
    assert!(st.commit_highlight(Highlight::Pointer { id: "g1".into(), x: 0.5, y: 0.5, tail: None, color: None }).is_err());
    let p = st.go_next().unwrap();
    image(st.load_now(p));
    assert!(st.commit_highlight(ret("g1")).is_err(), "id repetido em outra imagem da mesma fonte");
    assert_eq!(st.pending_highlight(), None);
    let f = HighlightsFile::load(st.vault.as_ref().unwrap(), "Livro").unwrap();
    assert_eq!(f.images.values().flatten().count(), 1);
}

#[test]
fn order_of_write_highlight_before_of_the_note_and_nothing_points_to_highlight_missing() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    let highlights = t.path().join("vault/grifos");
    // 1) o JSON de grifos nao grava: nenhum grifo pendente, a nota sai sem grifo
    std::fs::remove_dir_all(&highlights).unwrap();
    std::fs::write(&highlights, "nao e pasta").unwrap();
    assert!(st.commit_highlight(ret("g1")).is_err());
    assert_eq!(st.pending_highlight(), None);
    let p = note(st.submit_quick_note("sem grifo gravado", current_time()).unwrap());
    assert_eq!(p.highlight, None);
    // 2) grifo gravado primeiro; a nota falha depois: o grifo existe, segue pendente, nenhuma nota
    std::fs::remove_file(&highlights).unwrap();
    std::fs::create_dir(&highlights).unwrap();
    st.commit_highlight(ret("g2")).unwrap();
    let vault = st.vault.clone().unwrap();
    assert!(HighlightsFile::load(&vault, "Livro").unwrap().find(&st.current.image, "g2").is_some());
    let notes = t.path().join("vault/notas");
    std::fs::rename(&notes, t.path().join("notas-fora")).unwrap();
    std::fs::write(&notes, "nao e pasta").unwrap();
    assert!(matches!(st.submit_quick_note("falha", current_time()), Err(QuickNoteError::Write(_))));
    assert_eq!(st.pending_highlight(), Some(&ret("g2")));
    // 3) de volta: a nota leva g2, que existe no JSON
    std::fs::remove_file(&notes).unwrap();
    std::fs::rename(t.path().join("notas-fora"), &notes).unwrap();
    let p = note(st.submit_quick_note("com g2", current_time()).unwrap());
    assert_eq!(p.highlight.as_deref(), Some("g2"));
    for (p, _) in vault.list_notes().unwrap() {
        if let Some(g) = &p.highlight {
            assert!(HighlightsFile::load(&vault, "Livro").unwrap().find(&p.image, g).is_some(), "{p:?}");
        }
    }
}

#[test]
fn vault_choice() {
    let t = tempdir().unwrap();
    let exists = t.path().join("existe");
    std::fs::create_dir(&exists).unwrap();
    let cli = Some(t.path().join("cli"));
    let env = Some(t.path().join("env"));
    let missing = t.path().join("falta");
    assert_eq!(choose_vault(cli.clone(), env.clone(), Some(&exists)), cli);
    assert_eq!(choose_vault(None, env.clone(), Some(&exists)), env);
    assert_eq!(choose_vault(None, None, Some(&exists)), Some(exists.clone()));
    assert_eq!(choose_vault(None, None, Some(&missing)), None, "ultimo vault que sumiu nao e recriado");
    assert_eq!(choose_vault(None, None, None), None);
    assert_eq!(choose_vault(None, Some(PathBuf::new()), None), None, "MARGINALIA_VAULT vazio e ignorado");
}

#[test]
fn steps_of_the_controls_respect_os_limits() {
    let d = Adjustments::default();
    assert!((step(&d, Control::Brightness, 1).brightness - 0.05).abs() < 1e-6);
    assert_eq!(step(&d, Control::Brightness, 100).brightness, 1.0);
    assert_eq!(step(&d, Control::Brightness, -100).brightness, -1.0);
    assert!((step(&d, Control::Contrast, 1).contrast - 1.1).abs() < 1e-6);
    assert_eq!(step(&d, Control::Contrast, -100).contrast, 0.0);
    assert_eq!(step(&d, Control::Contrast, 100).contrast, 4.0);
    assert!((step(&d, Control::Sharpness, 2).sharpen - 0.2).abs() < 1e-6);
    assert_eq!(step(&d, Control::Highpass, -1).highpass, 0.0);
    assert_eq!(step(&d, Control::Threshold, 1).threshold, Some(128));
    let l = step(&d, Control::Threshold, 1);
    assert_eq!(step(&l, Control::Threshold, 1).threshold, Some(136));
    assert_eq!(step(&l, Control::Threshold, 100).threshold, Some(255));
    assert_eq!(step(&Adjustments { threshold: Some(0), ..d.clone() }, Control::Threshold, -1).threshold, None);
    assert_eq!(step(&d, Control::Threshold, -1).threshold, None);
}

// ---- Task 14: gavetas, promover, descartar, tarefas ----

use crate::drawers::{self, Drawers};
use marginalia_core::model::StateTask;

fn drawers(st: &AppState) -> Drawers {
    let mut g = Drawers::default();
    let n = g.request_reload();
    g.receive(n, drawers::load(st.vault.as_ref().unwrap(), &st.app_data));
    g
}

fn open_all(st: &AppState) -> Vec<String> {
    drawers(st).notes_visible().iter().map(|p| p.id.clone()).collect()
}

// ---- Task 5: excluir de vez ----

fn fname(t: &std::path::Path, folder: &str, id: &str) -> std::path::PathBuf {
    t.join(format!("vault/{folder}/{id}.md"))
}

#[test]
fn delete_note_remove_the_file_and_the_line() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    let p = note(st.submit_quick_note("Maria herdeira", current_time()).unwrap());
    let other = note(st.submit_quick_note("Joao", current_time()).unwrap());
    assert_eq!(open_all(&st).len(), 2);
    st.delete_item(&p.id).unwrap();
    assert!(!fname(t.path(), "notas", &p.id).exists());
    assert!(fname(t.path(), "notas", &other.id).exists(), "so o alvo");
    assert_eq!(open_all(&st), [other.id.clone()]);
    assert!(st.confirmation.as_deref().is_some_and(|c| c.contains("excluida")), "{:?}", st.confirmation);
    assert_eq!(st.warning, None);
}

#[test]
fn delete_note_and_task_remove_file_and_line() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    let p = note(st.submit_quick_note("sera apagada", current_time()).unwrap());
    let tf = match st.submit_quick_note("[] achar obito", current_time()).unwrap() {
        Created::Task(tf) => tf,
        o => panic!("{o:?}"),
    };
    assert_eq!(drawers(&st).notes_visible().len(), 1);
    st.delete_item(&p.id).unwrap();
    assert!(!fname(t.path(), "notas", &p.id).exists());
    assert!(drawers(&st).notes_visible().is_empty());
    st.delete_item(&tf.id).unwrap();
    assert!(!fname(t.path(), "tarefas", &tf.id).exists());
    assert!(drawers(&st).visible_tasks().is_empty());
}

#[test]
fn delete_the_note_open_cleared_the_state_and_the_highlight_stays_in_the_json() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    st.commit_highlight(ret("g1")).unwrap();
    let p = note(st.submit_quick_note("aberta agora", current_time()).unwrap());
    let pd = image_frame(st.open_note(&p.id).unwrap());
    image(st.load_now(pd));
    assert_eq!((st.note_open(), st.emphasis()), (Some(p.id.as_str()), Some("g1")));
    st.delete_item(&p.id).unwrap();
    assert_eq!(st.note_open(), None);
    assert_eq!(st.emphasis(), None);
    assert_eq!(st.error, None);
    let v = st.vault.as_ref().unwrap();
    let gf = HighlightsFile::load(v, "Livro").unwrap();
    assert!(gf.find(&p.image, "g1").is_some(), "o grifo permanece");
    // reabrir a nota excluida: aviso claro, sem panico
    let e = st.open_note(&p.id).err().expect("nao existe mais");
    assert!(e.to_string().contains("nao encontrada"), "{e}");
}

#[test]
fn delete_id_missing_warns_and_nothing_changes() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    let p = note(st.submit_quick_note("fica", current_time()).unwrap());
    for id in ["nao-existe", "", "../x", "a/b"] {
        let e = st.delete_item(id).unwrap_err();
        assert!(!e.is_empty());
        assert_eq!(st.warning.as_deref(), Some(e.as_str()), "{id:?}");
    }
    assert!(st.warning.is_some_and(|a| a.contains("../x") || a.contains("nao")));
    assert!(fname(t.path(), "notas", &p.id).exists());
    let mut no = AppState::new(None, AppData::new(t.path().join("ad2")), t.path().join("cache2"));
    assert!(no.delete_item("x").is_err());
}

#[test]
fn delete_with_renaming_in_progress_refuses_and_the_item_stays() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    let v = st.vault.clone().unwrap();
    let a = v.create_draft("Maria", st.current.image.clone(), st.framing().clone(), None, current_time()).unwrap();
    let text = std::fs::read_to_string(fname(t.path(), "notas", &a.id)).unwrap();
    let n = "20261112-1432-maria";
    let new = text.replace(&format!("id: {}", a.id), &format!("id: {n}")).replacen("\n---\n", &format!("\nrenaming_from: {}\n---\n", a.id), 1);
    std::fs::write(fname(t.path(), "notas", n), new).unwrap();
    let e = st.delete_item(&a.id).unwrap_err();
    assert!(e.contains("renomeação em curso"), "{e}");
    assert!(st.warning.as_deref().is_some_and(|a| a.contains("renomeação em curso")));
    assert!(fname(t.path(), "notas", &a.id).exists() && fname(t.path(), "notas", n).exists());
    assert_eq!(st.confirmation, None);
}

#[test]
fn task_that_cites_note_deleted_continues_listable() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    let p = note(st.submit_quick_note("origem", current_time()).unwrap());
    let v = st.vault.clone().unwrap();
    let tf = v.create_task("conferir", Some(p.image.clone()), Some(p.id.clone()), current_time()).unwrap();
    let g = drawers(&st);
    assert!(!g.visible_tasks()[0].note_deleted);
    st.delete_item(&p.id).unwrap();
    let mut g = drawers(&st);
    let line = g.visible_tasks()[0].clone();
    assert_eq!((line.id.as_str(), line.note.as_deref(), line.note_deleted), (tf.id.as_str(), Some(p.id.as_str()), true));
    // abrir a tarefa abre a imagem; nada panica
    let _ = st.open_line_task(&line).unwrap();
    g.toggle(drawers::Drawer::Tasks);
    assert_eq!(g.selected_task().unwrap().id, tf.id);
}

#[test]
fn mark_task_done_writes_state_done_and_uncheck_reopens() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    let tf = match st.submit_quick_note("[] achar óbito", current_time()).unwrap() {
        Created::Task(tf) => tf,
        o => panic!("{o:?}"),
    };
    let fname = t.path().join(format!("vault/tarefas/{}.md", tf.id));
    let r = st.mark_task_done(&tf.id, true).unwrap();
    assert_eq!(r.state, StateTask::Done);
    let doc = std::fs::read_to_string(&fname).unwrap();
    assert!(doc.contains("state: done") && doc.ends_with("---\nachar óbito"), "{doc}");
    assert!(drawers(&st).visible_tasks().is_empty(), "sai das abertas");
    st.mark_task_done(&tf.id, false).unwrap();
    assert!(std::fs::read_to_string(&fname).unwrap().contains("state: open"));
    assert_eq!(drawers(&st).visible_tasks()[0].id, tf.id);
    assert!(st.mark_task_done("nao-existe", true).is_err());
    assert!(st.warning.as_deref().is_some_and(|a| a.contains("nao-existe")));
}

#[test]
fn click_in_a_line_of_note_opens_the_note() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    st.commit_highlight(ret("g1")).unwrap();
    let fr = Framing { zoom: 2.0, cx: 0.4, cy: 0.6 };
    st.set_framing(fr.clone());
    let p = note(st.submit_quick_note("na p01", current_time()).unwrap());
    let pd = st.go_next().unwrap();
    image(st.load_now(pd));
    let mut g = drawers(&st);
    g.toggle(drawers::Drawer::Notes);
    let line = g.note_selected().unwrap().clone();
    let pd = image_frame(st.open_line_note(&line).unwrap());
    assert_eq!(st.current.image, p.image);
    assert_eq!(pd.frame, Reframe::Adopt(fr));
    assert_eq!(st.emphasis(), Some("g1"));
    assert_eq!(st.note_open(), Some(p.id.as_str()));
    image(st.load_now(pd));
    st.go_next().unwrap();
    assert_eq!(st.note_open(), None, "navegar para outra imagem fecha a nota");
}

#[test]
fn task_with_image_opens_the_image_and_no_image_warns() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    let pd = st.go_next().unwrap();
    image(st.load_now(pd));
    let tf = match st.submit_quick_note("[] conferir p02", current_time()).unwrap() {
        Created::Task(tf) => tf,
        o => panic!("{o:?}"),
    };
    let pd = st.go_next().unwrap();
    image(st.load_now(pd));
    let drops = st.vault.as_ref().unwrap().create_task("solta", None, None, current_time()).unwrap();
    let g = drawers(&st);
    let line = g.lists().tasks.iter().find(|x| x.id == tf.id).unwrap().clone();
    let pd = image_frame(st.open_line_task(&line).unwrap());
    assert_eq!(st.current.image.file, "p02.png");
    assert_eq!(pd.frame, Reframe::Adjust);
    let line = g.lists().tasks.iter().find(|x| x.id == drops.id).unwrap().clone();
    assert!(st.open_line_task(&line).is_err());
}

// ---- Task 15: seletor de vault, caminho perdido, nova localizacao, recarga, renomear ----

use crate::relink::Reason;
use marginalia_core::model::SourceKind;
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;

/// Pasta `dir` com `p01.png`..`pNN.png`.
fn folder_png(dir: &std::path::Path, n: usize) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    for (name, b) in names(n) {
        std::fs::write(dir.join(name), b).unwrap();
    }
    dir.to_path_buf()
}

/// Todos os arquivos do vault com o conteudo (para provar que nada mudou).
fn photo(root: &std::path::Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut m = BTreeMap::new();
    for d in ["fontes", "notas", "tarefas", "grifos"] {
        for e in std::fs::read_dir(root.join(d)).unwrap() {
            let p = e.unwrap().path();
            m.insert(p.clone(), std::fs::read(&p).unwrap());
        }
    }
    m
}

/// Fonte que passa a falhar com erro de E/S (NAS desmontado) e depois volta.
struct Unstable {
    base: Arc<Fake>,
    fail: AtomicBool,
}

impl ImageSource for Unstable {
    fn entries(&self) -> &[ImageEntry] {
        self.base.entries()
    }
    fn read(&self, path: &str) -> marginalia_core::vault::Result<Vec<u8>> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(Error::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "NAS desmontado")));
        }
        self.base.read(path)
    }
}

fn vault_with_note(root: &std::path::Path, text: &str) -> Vault {
    let v = Vault::open_or_create(root).unwrap();
    let img = ImageRef { source: "Livro".into(), file: "p01.png".into(), page: 1 };
    v.create_note(text, img, viewer_math::default_framing(), None, current_time()).unwrap();
    v
}

fn texts(st: &AppState) -> Vec<String> {
    drawers(st).notes_visible().iter().map(|p| p.text.clone()).collect()
}

#[test]
fn switch_vault_swap_as_lists_and_remembers_the_vault() {
    let t = tempdir().unwrap();
    let a = vault_with_note(&t.path().join("a"), "nota do vault A");
    let b = vault_with_note(&t.path().join("b"), "nota do vault B");
    let mut st = AppState::new(Some(a.clone()), AppData::new(t.path().join("appdata")), t.path().join("cache"));
    assert_eq!(texts(&st), ["nota do vault A"]);
    st.switch_vault(&a.root).unwrap();
    st.switch_vault(&b.root).unwrap();
    assert_eq!(st.vault.as_ref().unwrap().root, b.root);
    assert_eq!(texts(&st), ["nota do vault B"]);
    let disk = AppData::load(t.path().join("appdata")).unwrap();
    assert_eq!(disk.last_vault(), Some(b.root.as_path()));
    assert_eq!(disk.known_vaults(), &[b.root.clone(), a.root.clone()]);
    // de volta ao A
    st.switch_vault(&a.root).unwrap();
    assert_eq!(texts(&st), ["nota do vault A"]);
}

#[test]
fn switch_vault_that_failure_keeps_the_vault_old_and_warns() {
    let t = tempdir().unwrap();
    let a = vault_with_note(&t.path().join("a"), "A");
    let mut st = AppState::new(Some(a.clone()), AppData::new(t.path().join("appdata")), t.path().join("cache"));
    let e = st.switch_vault(&t.path().join("nao-existe")).unwrap_err();
    assert!(e.contains("nao existe"), "{e}");
    assert_eq!(st.warning.as_deref(), Some(e.as_str()));
    let common = t.path().join("comum");
    std::fs::create_dir(&common).unwrap();
    assert!(st.switch_vault(&common).unwrap_err().contains("nao e um vault"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let closed = t.path().join("fechada");
        std::fs::create_dir_all(closed.join("notas")).unwrap();
        std::fs::set_permissions(&closed, std::fs::Permissions::from_mode(0o000)).unwrap();
        let r = st.switch_vault(&closed);
        std::fs::set_permissions(&closed, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(r.unwrap_err().contains("nao foi possivel ler"));
    }
    assert_eq!(st.vault.as_ref().unwrap().root, a.root, "o vault antigo continua ativo");
    assert_eq!(texts(&st), ["A"]);
    assert_eq!(AppData::load(t.path().join("appdata")).unwrap().last_vault(), None, "falha nao vira ultimo vault");
}

#[test]
fn switch_with_highlight_pending_discards_the_highlight_and_closes_source_of_other_vault() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    st.commit_highlight(ret("g1")).unwrap();
    let b = Vault::open_or_create(&t.path().join("b")).unwrap();
    let swap = st.switch_vault(&b.root).unwrap();
    assert!(swap.discarded_highlight);
    assert_eq!(swap.closed_source.as_deref(), Some("Livro"), "Livro nao e fonte do vault B");
    assert!(!st.source_has());
    assert_eq!(st.pending_highlight(), None);
    assert!(st.submit_quick_note("sem imagem", current_time()).is_err());
    assert!(b.list_notes().unwrap().is_empty());
}

#[test]
fn switch_keeps_the_source_that_the_vault_new_knows_in_the_same_path() {
    let t = tempdir().unwrap();
    let folder = folder_png(&t.path().join("Livro"), 3);
    let mut st = with_vault(t.path());
    st.remember_path("Livro", folder.clone());
    let b = Vault::open_or_create(&t.path().join("b")).unwrap();
    b.create_source("Livro", SourceKind::Folder).unwrap();
    HighlightsFile::default().save(&b, "Livro").unwrap();
    let mut gb = HighlightsFile::load(&b, "Livro").unwrap();
    gb.add(&st.current.image.clone(), ret("do-b")).unwrap();
    gb.save(&b, "Livro").unwrap();
    st.app_data.set_source_path(&b.root, "Livro", folder.clone());
    let swap = st.switch_vault(&b.root).unwrap();
    assert_eq!(swap.closed_source, None);
    assert!(st.source_has());
    assert_eq!(st.image_highlights().iter().map(|g| g.id()).collect::<Vec<_>>(), ["do-b"], "grifos do vault novo");
    // caminhos da sessao antiga nao valem no vault novo
    let a = st.open_source("Livro", ZipMode::Direct).unwrap();
    assert_eq!(a.file_path, folder);
}

#[test]
fn open_source_not_found_or_no_path_not_opens_nothing_and_returns_source_absent() {
    let t = tempdir().unwrap();
    let vault = Vault::open_or_create(&t.path().join("vault")).unwrap();
    let mut ad = AppData::new(t.path().join("ad"));
    let vanished = t.path().join("nao-existe");
    ad.set_source_path(&vault.root, "Livro", vanished.clone());
    let mut st = AppState::new(Some(vault), ad, t.path().join("cache"));
    let before = st.current.clone();
    assert_eq!(st.open_source_now("Livro", ZipMode::Direct).err().unwrap(), SourceError::AbsentSource { name: "Livro".into() });
    let p = st.pending().expect("pedido de nova localizacao");
    assert_eq!((p.name.as_str(), p.reason, p.file_path.as_ref()), ("Livro", Reason::NotFound, Some(&vanished)));
    assert!(!st.source_has());
    assert_eq!(st.current, before);
    assert_eq!(st.open_source_now("Outro", ZipMode::Direct).err().unwrap(), SourceError::AbsentSource { name: "Outro".into() });
    assert_eq!(st.pending().unwrap().reason, Reason::NoPath, "sem caminho e tratado como ausente");
    st.dismiss_pending();
    assert!(st.pending().is_none());
}

#[test]
fn after_of_relink_open_works_and_the_map_stays_in_the_data_of_the_app() {
    let t = tempdir().unwrap();
    let vault = Vault::open_or_create(&t.path().join("vault")).unwrap();
    vault.create_source("Livro", SourceKind::Folder).unwrap();
    let fr = Framing { zoom: 2.0, cx: 0.3, cy: 0.6 };
    let img = ImageRef { source: "Livro".into(), file: "p02.png".into(), page: 1 };
    let p = vault.create_note("no p02", img.clone(), fr.clone(), None, current_time()).unwrap();
    let mut ad = AppData::new(t.path().join("ad"));
    ad.set_source_path(&vault.root, "Livro", t.path().join("antigo"));
    let mut st = AppState::new(Some(vault.clone()), ad, t.path().join("cache"));
    let r = st.open_note(&p.id);
    assert!(matches!(open_now(&mut st, r), Err(SourceError::AbsentSource { .. })));
    assert_eq!(st.pending().unwrap().image_frame.as_ref().map(|q| &q.image), Some(&img));
    let before = photo(&vault.root);

    let fresh = folder_png(&t.path().join("montado-em-outro-lugar"), 3);
    let request = st.relink_source("Livro", fresh.clone()).unwrap().expect("reabre a nota pedida");
    assert_eq!(st.current.image, img);
    assert_eq!(request.frame, Reframe::Adopt(fr));
    assert_eq!(st.note_open(), Some(p.id.as_str()));
    image(st.load_now(request));
    assert!(st.pending().is_none());
    assert_eq!(AppData::load(t.path().join("ad")).unwrap().source_path(&vault.root, "Livro"), Some(fresh.clone()));
    assert_eq!(photo(&vault.root), before, "o vault nao muda");
    // abrir de novo funciona
    let a = st.open_source("Livro", ZipMode::Direct).unwrap();
    assert_eq!(a.file_path, fresh);
    assert!(a.execute().is_ok());
}

fn zip_png(file_path: &std::path::Path, n: usize) {
    use std::io::Write;
    let f = std::fs::File::create(file_path).unwrap();
    let mut z = zip::ZipWriter::new(f);
    for (name, b) in names(n) {
        z.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(&b).unwrap();
    }
    z.finish().unwrap();
}

#[test]
fn relink_of_zip_and_of_folder_respects_the_kind() {
    let t = tempdir().unwrap();
    let vault = Vault::open_or_create(&t.path().join("vault")).unwrap();
    vault.create_source("Z", SourceKind::Zip).unwrap();
    vault.create_source("P", SourceKind::Folder).unwrap();
    let mut st = AppState::new(Some(vault.clone()), AppData::new(t.path().join("ad")), t.path().join("cache"));
    let zip = t.path().join("movido.zip");
    zip_png(&zip, 2);
    let folder = folder_png(&t.path().join("pasta"), 2);
    assert!(st.relink_source("Z", folder.clone()).err().unwrap().contains(".zip"));
    assert!(st.relink_source("P", zip.clone()).err().unwrap().contains("pasta"));
    assert_eq!(st.app_data.source_path(&vault.root, "Z"), None);
    assert!(st.relink_source("Z", zip.clone()).unwrap().is_none(), "nao havia quadro a reabrir");
    assert!(st.relink_source("P", folder.clone()).unwrap().is_none());
    assert_eq!(st.app_data.source_path(&vault.root, "Z"), Some(zip));
    assert_eq!(st.app_data.source_path(&vault.root, "P"), Some(folder));
}

#[test]
fn relink_to_folder_no_as_images_of_the_source_of_the_error_and_not_changes_the_map() {
    let t = tempdir().unwrap();
    let vault = vault_with_note(&t.path().join("vault"), "cita p01");
    vault.create_source("Livro", SourceKind::Folder).unwrap();
    let old = t.path().join("antigo");
    let mut ad = AppData::new(t.path().join("ad"));
    ad.set_source_path(&vault.root, "Livro", old.clone());
    ad.save().unwrap();
    let mut st = AppState::new(Some(vault.clone()), ad, t.path().join("cache"));
    let other = t.path().join("outra");
    std::fs::create_dir(&other).unwrap();
    std::fs::write(other.join("x.png"), png([1, 1, 1, 255])).unwrap();
    let e = st.relink_source("Livro", other).err().unwrap();
    assert!(e.contains("0 das 1") && e.contains("p01.png"), "{e}");
    let empty = t.path().join("vazia");
    std::fs::create_dir(&empty).unwrap();
    assert!(st.relink_source("Livro", empty).err().unwrap().contains("nao tem imagens"));
    assert_eq!(st.app_data.source_path(&vault.root, "Livro"), Some(old.clone()));
    assert_eq!(AppData::load(t.path().join("ad")).unwrap().source_path(&vault.root, "Livro"), Some(old));
    assert_eq!(st.warning.as_deref().map(|a| a.contains("nao tem imagens")), Some(true));
}

#[test]
fn source_that_failure_in_the_middle_of_the_session_stays_unavailable_no_accumulate_and_recovers() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let source = Arc::new(Unstable { base: fake(&names(10)), fail: AtomicBool::new(false) });
    let p = st.adopt_source("Livro", source.clone()).unwrap();
    image(st.load_now(p));
    source.fail.store(true, Ordering::SeqCst);
    let mut f = Frame::new("Livro", "p09.png", 1);
    let fr = Framing { zoom: 3.0, cx: 0.4, cy: 0.5 };
    f.framing = Some(fr.clone());
    let p = st.open_frame(f.clone()).unwrap();
    match st.load_now(p) {
        Received::Unavailable(m) => assert!(m.contains("NAS desmontado"), "{m}"),
        _ => panic!("esperava indisponivel"),
    }
    let pending = st.pending().expect("aviso de fonte indisponivel").clone();
    assert_eq!((pending.name.as_str(), pending.reason), ("Livro", Reason::Unavailable));
    assert_eq!(pending.image_frame, Some(f.clone()));
    assert!(pending.title().contains("Fonte indisponível: Livro"));
    assert!(st.error.is_some());
    // setas durante a queda nao empilham trabalho
    assert!(st.go_next().is_none());
    assert!(st.go_prev().is_none());
    assert!(!st.has_navigation_pending());
    assert_eq!(st.current, f);
    // tentar de novo enquanto cai: um pedido so
    let p = image_frame(st.new_attempt().unwrap());
    assert!(st.new_attempt().is_err(), "uma nova tentativa ja esta em andamento");
    assert!(matches!(st.load_now(p), Received::Unavailable(_)));
    // o NAS volta
    source.fail.store(false, Ordering::SeqCst);
    let p = image_frame(st.new_attempt().unwrap());
    assert_eq!(p.frame, Reframe::Adopt(fr));
    image(st.load_now(p));
    assert!(st.pending().is_none());
    assert_eq!(st.error, None);
    assert!(st.go_next().is_some(), "navega de novo");
}

#[test]
fn image_corrupt_not_and_source_unavailable() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let p = st.adopt_source("Livro", fake(&[("a.png".into(), b"corrompido".to_vec())])).unwrap();
    assert!(matches!(st.load_now(p), Received::Error(_)));
    assert!(st.pending().is_none());
}

#[test]
fn relink_of_the_source_open_that_vanished_reopens_the_same_frame_in_the_same_framing() {
    let t = tempdir().unwrap();
    let vault = Vault::open_or_create(&t.path().join("vault")).unwrap();
    let folder = folder_png(&t.path().join("Livro"), 12);
    let mut st = AppState::new(Some(vault.clone()), AppData::new(t.path().join("ad")), t.path().join("cache"));
    open_source::register(Some(&vault), &mut st.app_data, "Livro", SourceKind::Folder, &folder).unwrap();
    let a = st.open_source("Livro", ZipMode::Direct).unwrap();
    let src = a.execute().unwrap();
    let pd = st.adopt_source("Livro", src).unwrap();
    image(st.load_now(pd));
    st.remember_path("Livro", folder.clone());
    st.commit_highlight(ret("g-p01")).unwrap();
    let moved = t.path().join("Livro movido");
    std::fs::rename(&folder, &moved).unwrap();
    let fr = Framing { zoom: 2.5, cx: 0.7, cy: 0.2 };
    let mut f = Frame::new("Livro", "p11.png", 1);
    f.framing = Some(fr.clone());
    let p = st.open_frame(f).unwrap();
    assert!(matches!(st.load_now(p), Received::Unavailable(_)));
    let request = st.relink_source("Livro", moved.clone()).unwrap().expect("reabre o quadro");
    assert_eq!(st.current.image.file, "p11.png");
    assert_eq!(request.frame, Reframe::Adopt(fr));
    image(st.load_now(request));
    assert!(st.pending().is_none());
    // a sessao e o vault apontam para o lugar novo; os grifos continuam
    assert_eq!(st.open_source("Livro", ZipMode::Direct).unwrap().file_path, moved);
    let p01 = st.open_frame(Frame::new("Livro", "p01.png", 1)).unwrap();
    image(st.load_now(p01));
    assert_eq!(st.image_highlights().len(), 1);
}

#[test]
fn reload_sees_note_and_highlight_written_by_outside_and_skips_file_broken() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    let root_dir = st.vault.as_ref().unwrap().root.clone();
    std::fs::write(
        root_dir.join("notas/20990101-0000-de-fora.md"),
        "---\nid: 20990101-0000-de-fora\nestado: aberta\nimagem:\n  fonte: Livro\n  arquivo: p02.png\n  pagina: 1\nenquadramento:\n  zoom: 1.0\n  cx: 0.5\n  cy: 0.5\n---\nescrita no Obsidian\n",
    )
    .unwrap();
    std::fs::write(root_dir.join("notas/quebrada.md"), "sem frontmatter").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(root_dir.join("notas/sumiu.md"), root_dir.join("notas/link-morto.md")).unwrap();
    let other = Vault { root: root_dir.clone() };
    let mut gf = HighlightsFile::default();
    gf.add(&st.current.image.clone(), ret("de-fora")).unwrap();
    gf.save(&other, "Livro").unwrap();

    let r = st.reload().expect("ha vault");
    let reread = r.execute();
    let lists = st.receive_reload(reread);
    assert!(lists.notes.iter().any(|p| p.text == "escrita no Obsidian"));
    assert_eq!(lists.unreadable, ["notas/quebrada.md"], "o link morto some calado; o quebrado e contado");
    assert_eq!(st.image_highlights().iter().map(|g| g.id()).collect::<Vec<_>>(), ["de-fora"]);
    assert_eq!(st.current.image.file, "p01.png", "o quadro aberto fica");
}

#[test]
fn reload_that_arrives_late_not_undoes_one_highlight_new() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    let old = st.reload().unwrap().execute();
    st.commit_highlight(ret("novo")).unwrap();
    st.receive_reload(old);
    assert_eq!(st.image_highlights().len(), 1);
    assert!(st.pending_highlight().is_some());
    assert!(state(t.path()).reload().is_none(), "sem vault nao ha o que reler");
}

#[test]
fn rename_source_rewrites_the_vault_os_data_of_the_app_and_the_frame_follows() {
    let t = tempdir().unwrap();
    let folder = folder_png(&t.path().join("Livro"), 3);
    let mut st = with_vault(t.path());
    let vault = st.vault.clone().unwrap();
    open_source::register(Some(&vault), &mut st.app_data, "Livro", SourceKind::Folder, &folder).unwrap();
    st.remember_path("Livro", folder.clone());
    st.commit_highlight(ret("g1")).unwrap();
    let p = note(st.submit_quick_note("Maria", current_time()).unwrap());
    let inv = Adjustments { invert: true, ..Adjustments::default() };
    if let Some(pd) = st.set_adjustments(inv.clone()) {
        image(st.load_now(pd));
    }
    let pd = image_frame(st.open_note(&p.id).unwrap());
    image(st.load_now(pd));
    st.rename_source("Livro", "Livro 3 de Batismos").unwrap();
    let new = "Livro 3 de Batismos";
    assert_eq!(st.current.image.source, new);
    assert_eq!(st.note_open(), Some(p.id.as_str()), "a nota aberta continua");
    assert!(matches!(vault.find_by_id(&p.id).unwrap(), Some(Found::Note(q, _)) if q.image.source == new));
    assert!(vault.root.join("grifos/Livro 3 de Batismos.json").is_file());
    assert!(!vault.root.join("fontes/Livro.md").exists());
    let disk = AppData::load(t.path().join("appdata")).unwrap();
    assert_eq!(disk.source_path(&vault.root, new), Some(folder.clone()));
    assert_eq!(disk.source_path(&vault.root, "Livro"), None);
    assert_eq!(st.adjustments, inv);
    assert_eq!(st.image_highlights().len(), 1);
    // o quadro aberto continua funcionando e os grifos novos vao para o arquivo novo
    let pd = st.go_next().unwrap();
    assert_eq!(st.current.image.source, new);
    image(st.load_now(pd));
    st.commit_highlight(ret("g2")).unwrap();
    assert_eq!(HighlightsFile::load(&vault, new).unwrap().images.values().flatten().count(), 2);
    assert_eq!(st.open_source(new, ZipMode::Direct).unwrap().file_path, folder);
}

#[test]
fn rename_refuses_duplicate_invalid_and_traversal_no_change_nothing() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    let vault = st.vault.clone().unwrap();
    vault.create_source("Livro", SourceKind::Folder).unwrap();
    vault.create_source("Outro", SourceKind::Zip).unwrap();
    let before = photo(&vault.root);
    for bad in ["Outro", "outro", "", "a/b", "../fora", "..", ".x", "a:b"] {
        assert!(st.rename_source("Livro", bad).is_err(), "{bad:?}");
    }
    assert!(st.rename_source("Nao existe", "Novo").is_err());
    assert_eq!(photo(&vault.root), before);
    assert_eq!(st.current.image.source, "Livro");
    assert!(!t.path().join("fora.md").exists());
}

#[test]
fn attempt_of_new_resolve_the_source_absent_when_the_in_the_back() {
    let t = tempdir().unwrap();
    let vault = Vault::open_or_create(&t.path().join("vault")).unwrap();
    let in_the = t.path().join("Volumes/nas/Livro");
    let mut ad = AppData::new(t.path().join("ad"));
    ad.set_source_path(&vault.root, "Livro", in_the.clone());
    let mut st = AppState::new(Some(vault), ad, t.path().join("cache"));
    assert!(st.open_source_now("Livro", ZipMode::Direct).is_err());
    let r = st.new_attempt();
    assert!(open_now(&mut st, r).is_err(), "NAS ainda fora: continua ausente");
    assert_eq!(st.pending().unwrap().reason, Reason::NotFound);
    folder_png(&in_the, 2); // o NAS foi montado
    let r = st.new_attempt();
    assert!(matches!(r, Ok(OpenNote::Source { .. })), "esperava abrir a fonte");
    let pd = open_now(&mut st, r).unwrap().expect("primeiro quadro");
    assert_eq!(st.current, Frame::new("Livro", "p01.png", 1));
    image(st.load_now(pd));
    assert!(st.pending().is_none());
}

// ---- Task 15, fix round 1 ----

/// Abre de verdade o que `open_note`/`open_point` pediram (o fundo da view numa chamada so).
fn open_now(st: &mut AppState, r: Result<OpenNote, SourceError>) -> Result<Option<Request>, SourceError> {
    match r? {
        OpenNote::ImageFrame(p) => Ok(p),
        OpenNote::Source { opening, image_frame, highlight } => {
            let r = opening.execute_ready().map(|ready| open_source::Open { ready, record: None });
            match st.enter_source(&opening, r, Some(image_frame), highlight) {
                Input::Open { request, .. } => Ok(request),
                Input::Failed(e) => Err(e),
                Input::Stale => panic!("obsoleta"),
            }
        }
    }
}

#[test]
fn open_source_not_touches_the_disk_and_the_absence_appears_in_the_background() {
    let t = tempdir().unwrap();
    let vault = Vault::open_or_create(&t.path().join("vault")).unwrap();
    let mut ad = AppData::new(t.path().join("ad"));
    let in_the = t.path().join("Volumes/nas/Livro");
    ad.set_source_path(&vault.root, "Livro", in_the.clone());
    let mut st = AppState::new(Some(vault), ad, t.path().join("cache"));
    // sem stat na thread da interface: o caminho guardado vira uma abertura
    let a = st.open_source("Livro", ZipMode::Direct).unwrap();
    assert_eq!(a.file_path, in_the);
    assert!(matches!(a.execute_ready(), Err(OpeningFailure::Absent)), "o stat e no fundo");
    assert!(st.pending().is_none());
    let r = a.execute_ready().map(|ready| open_source::Open { ready, record: None });
    match st.enter_source(&a, r, None, None) {
        Input::Failed(e) => assert_eq!(e, SourceError::AbsentSource { name: "Livro".into() }),
        _ => panic!("esperava fonte ausente"),
    }
    let p = st.pending().unwrap();
    assert_eq!((p.reason, p.file_path.as_ref()), (Reason::NotFound, Some(&in_the)));
    assert!(!st.source_has());
}

#[test]
fn opening_that_arrives_after_of_switch_of_vault_and_discarded() {
    let t = tempdir().unwrap();
    let a = Vault::open_or_create(&t.path().join("a")).unwrap();
    let b = Vault::open_or_create(&t.path().join("b")).unwrap();
    let folder = folder_png(&t.path().join("Livro"), 2);
    let mut st = AppState::new(Some(a.clone()), AppData::new(t.path().join("ad")), t.path().join("cache"));
    let ab = st.path_opening("Livro", folder.clone(), ZipMode::Direct);
    let r = open_source::open_in_background(&ab, &st.app_data.clone());
    st.switch_vault(&b.root).unwrap();
    assert!(matches!(st.enter_source(&ab, r, None, None), Input::Stale));
    assert!(!st.source_has());
    assert_eq!(st.app_data.source_path(&a.root, "Livro"), None, "o caminho nao foi gravado");
    assert_eq!(st.app_data.source_path(&b.root, "Livro"), None);
    assert!(st.open_source("Livro", ZipMode::Direct).is_err(), "nem lembrado na sessao");
    // a mesma abertura no vault certo entra e grava o caminho
    let ab = st.path_opening("Livro", folder.clone(), ZipMode::Direct);
    let r = open_source::open_in_background(&ab, &st.app_data.clone());
    match st.enter_source(&ab, r, None, None) {
        Input::Open { request, fresh } => assert!(request.is_some() && fresh),
        _ => panic!("esperava abrir"),
    }
    assert_eq!(st.app_data.source_path(&b.root, "Livro"), Some(folder));
}

#[test]
fn relink_that_arrives_after_of_switch_of_vault_not_touches_in_the_source_open() {
    let t = tempdir().unwrap();
    let a = Vault::open_or_create(&t.path().join("a")).unwrap();
    let b = Vault::open_or_create(&t.path().join("b")).unwrap();
    let mut st = AppState::new(Some(a.clone()), AppData::new(t.path().join("ad")), t.path().join("cache"));
    let fresh = folder_png(&t.path().join("nova"), 2);
    let r = st.prepare_relink("Livro", fresh.clone(), Expected::default());
    let v = r.execute().unwrap();
    assert_eq!(v.confirm, Some(2), "nada citado: pede confirmacao com a contagem");
    st.switch_vault(&b.root).unwrap();
    st.warning = Some("aviso do vault B".into());
    assert!(st.conclude_relink(&r, v.ready).is_none());
    assert_eq!(st.warning.as_deref(), Some("aviso do vault B"), "o aviso do vault novo fica");
    assert!(!st.source_has());
    assert_eq!(st.app_data.source_path(&a.root, "Livro"), Some(fresh), "o mapa e do vault em que foi pedido");
    assert_eq!(st.app_data.source_path(&b.root, "Livro"), None);
}

#[cfg(unix)]
#[test]
fn rename_interrupted_not_loses_the_path_and_attempt_of_new_ends() {
    use std::os::unix::fs::PermissionsExt;
    let t = tempdir().unwrap();
    let folder = folder_png(&t.path().join("Livro"), 3);
    let mut st = with_vault(t.path());
    let vault = st.vault.clone().unwrap();
    open_source::register(Some(&vault), &mut st.app_data, "Livro", SourceKind::Folder, &folder).unwrap();
    st.submit_quick_note("cita Livro", current_time()).unwrap();
    let notes = vault.root.join("notas");
    std::fs::set_permissions(&notes, std::fs::Permissions::from_mode(0o555)).unwrap();
    let r = st.rename_source("Livro", "Livro 3");
    std::fs::set_permissions(&notes, std::fs::Permissions::from_mode(0o755)).unwrap();
    let e = r.unwrap_err();
    assert!(e.contains("tente de novo"), "{e}");
    let disk = AppData::load(t.path().join("appdata")).unwrap();
    assert_eq!(disk.source_path(&vault.root, "Livro"), Some(folder.clone()), "o caminho antigo fica");
    assert_eq!(disk.source_path(&vault.root, "Livro 3"), Some(folder.clone()), "e o novo ja tem o caminho");
    assert_eq!(st.current.image.source, "Livro", "a fonte aberta so muda quando terminar");
    st.rename_source("Livro", "Livro 3").unwrap();
    let disk = AppData::load(t.path().join("appdata")).unwrap();
    assert_eq!(disk.source_path(&vault.root, "Livro"), None);
    assert_eq!(disk.source_path(&vault.root, "Livro 3"), Some(folder));
    assert_eq!(st.current.image.source, "Livro 3");
    assert!(vault.list_notes().unwrap().iter().all(|(p, _)| p.image.source == "Livro 3"));
}

#[test]
fn rename_done_in_other_vault_so_move_the_map_of_that_vault() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    let a = st.vault.clone().unwrap();
    st.app_data.set_source_path(&a.root, "Livro", t.path().join("l"));
    let b = Vault::open_or_create(&t.path().join("b")).unwrap();
    b.create_source("Livro", SourceKind::Folder).unwrap();
    st.app_data.set_source_path(&b.root, "Livro", t.path().join("l"));
    st.switch_vault(&b.root).unwrap();
    // a renomeacao foi pedida no vault A e terminou depois da troca
    st.conclude_rename(&a.root, "Livro", "Livro 3");
    assert_eq!(st.app_data.source_path(&a.root, "Livro 3"), Some(t.path().join("l")));
    assert_eq!(st.app_data.source_path(&b.root, "Livro"), Some(t.path().join("l")), "o vault B nao muda");
    assert_eq!(st.current.image.source, "Livro", "a fonte aberta e do vault B");
}

// ---- Task 15, fix round 2 ----

#[test]
fn rename_to_name_of_other_source_in_the_disk_not_and_partial_and_not_writes_map() {
    let t = tempdir().unwrap();
    let folder = folder_png(&t.path().join("Livro"), 2);
    let mut st = with_vault(t.path());
    let vault = st.vault.clone().unwrap();
    open_source::register(Some(&vault), &mut st.app_data, "Livro", SourceKind::Folder, &folder).unwrap();
    // outra fonte "Livro 3" apareceu no disco (outra maquina), sem caminho aqui
    vault.create_source("Livro 3", SourceKind::Folder).unwrap();
    let e = vault.rename_source("Livro", "Livro 3").unwrap_err();
    let failure = sort_failure_rename(&vault, "Livro", "Livro 3", &e);
    assert_eq!(failure, FailureRename::Common);
    let m = st.rename_failed(&vault.root, "Livro", "Livro 3", &e.to_string(), failure);
    assert!(!m.contains("parou no meio") && m.contains("Livro 3"), "{m}");
    assert_eq!(st.app_data.source_path(&vault.root, "Livro 3"), None, "nenhum mapa para a outra fonte");
    assert_eq!(AppData::load(t.path().join("appdata")).unwrap().source_path(&vault.root, "Livro 3"), None);
    // o caminho composto tambem recusa e nao grava nada
    assert!(st.rename_source("Livro", "Livro 3").is_err());
    assert_eq!(st.app_data.source_path(&vault.root, "Livro 3"), None);
    assert_eq!(st.app_data.source_path(&vault.root, "Livro"), Some(folder));
}

#[test]
fn rename_stopped_after_of_remove_the_old_asks_to_reload() {
    let t = tempdir().unwrap();
    let mut st = with_vault(t.path());
    let vault = st.vault.clone().unwrap();
    st.app_data.set_source_path(&vault.root, "Livro", t.path().join("l"));
    st.current.image.source = "Livro".into();
    let m = st.rename_failed(&vault.root, "Livro", "Livro 3", "erro de E/S", FailureRename::PartialNoOld);
    assert!(m.contains("recarregue") && !m.contains("tente de novo"), "{m}");
    assert_eq!(st.app_data.source_path(&vault.root, "Livro 3"), Some(t.path().join("l")));
    // `sources/Livro.md` ja saiu: o nome novo vale (notas e grifos novos usam "Livro 3").
    assert_eq!(st.app_data.source_path(&vault.root, "Livro"), None);
    assert_eq!(st.current.image.source, "Livro 3");
    assert!(st.interrupted_renaming.is_none());
    assert_eq!(st.warning.as_deref(), Some(m.as_str()));
}

// ---- ultimas posicoes abertas ----

fn with_two_sources(t: &std::path::Path) -> (AppState, Vault) {
    let book = folder_png(&t.join("Livro"), 3);
    let other = folder_png(&t.join("Outra"), 2);
    let mut st = with_vault(t);
    let vault = st.vault.clone().unwrap();
    open_source::register(Some(&vault), &mut st.app_data, "Livro", SourceKind::Folder, &book).unwrap();
    open_source::register(Some(&vault), &mut st.app_data, "Outra", SourceKind::Folder, &other).unwrap();
    (st, vault)
}

fn open(st: &mut AppState, name: &str) {
    let p = st.open_source_now(name, ZipMode::Direct).unwrap().unwrap();
    image(st.load_now(p));
}

#[test]
fn reopen_source_back_the_last_position_and_switch_of_source_persists() {
    let t = tempdir().unwrap();
    let (mut st, vault) = with_two_sources(t.path());
    open(&mut st, "Livro");
    assert_eq!(st.current.image.file, "p01.png");
    for _ in 0..2 {
        let p = st.go_next().unwrap();
        image(st.load_now(p));
    }
    assert_eq!(st.current.image.file, "p03.png");
    open(&mut st, "Outra");
    assert_eq!(st.current.image.file, "p01.png", "fonte sem posicao abre no primeiro");
    let disk = AppData::load(t.path().join("appdata")).unwrap();
    assert_eq!(disk.position(&vault.root, "Livro"), Some(("p03.png".to_string(), 1)), "a troca de fonte salvou");
    open(&mut st, "Livro");
    assert_eq!(st.current.image.file, "p03.png");
}

#[test]
fn position_remembered_of_file_that_vanished_opens_in_the_first() {
    let t = tempdir().unwrap();
    let (mut st, vault) = with_two_sources(t.path());
    st.app_data.set_position(&vault.root, "Livro", "sumiu.png", 4);
    open(&mut st, "Livro");
    assert_eq!(st.current.image.file, "p01.png");
    assert_eq!(st.current.image.page, 1);
}

#[test]
fn save_position_writes_and_swap_of_vault_also() {
    let t = tempdir().unwrap();
    let (mut st, vault) = with_two_sources(t.path());
    open(&mut st, "Outra");
    let p = st.go_next().unwrap();
    image(st.load_now(p));
    // Navegar so muda a memoria; sair do app chama `save_position`.
    assert_ne!(AppData::load(t.path().join("appdata")).unwrap().position(&vault.root, "Outra"), Some(("p02.png".to_string(), 1)));
    st.save_position();
    assert_eq!(AppData::load(t.path().join("appdata")).unwrap().position(&vault.root, "Outra"), Some(("p02.png".to_string(), 1)));
    // Trocar de vault tambem persiste a posicao do vault que sai.
    let p = st.go_prev().unwrap();
    image(st.load_now(p));
    let b = Vault::open_or_create(&t.path().join("b")).unwrap();
    st.switch_vault(&b.root).unwrap();
    assert_eq!(AppData::load(t.path().join("appdata")).unwrap().position(&vault.root, "Outra"), Some(("p01.png".to_string(), 1)));
}

/// Fechar a janela e sair do app chamam os dois `save_position`: o segundo
/// acha tudo salvo e nao toca o disco.
#[test]
fn save_position_two_times_writes_a() {
    let t = tempdir().unwrap();
    let (mut st, _vault) = with_two_sources(t.path());
    open(&mut st, "Outra");
    let p = st.go_next().unwrap();
    image(st.load_now(p));
    st.save_position();
    let fname = t.path().join("appdata/appdata.json");
    assert!(fname.is_file());
    std::fs::remove_file(&fname).unwrap();
    st.save_position();
    assert!(!fname.exists(), "a segunda chamada nao devia gravar");
}

impl AppState {
    /// Exclui de vez o documento `id` (nota aberta, descartada ou promovida em
    /// nota, ou tarefa): resolve o tipo pelo vault (`find_by_id`; um id em mais
    /// de uma pasta prefere a nota, ver [`AppState::delete_item_in`]). So o
    /// arquivo some: o grifo ligado fica no JSON e as tarefas/notas que citam a
    /// nota continuam (a gaveta mostra "nota excluida"). Quem chama ja
    /// confirmou com o usuario (`Drawers::request_deletion`). Devolve o tipo.
    // A interface sempre sabe o tipo (a linha da gaveta) e chama `delete_item_in`;
    // esta forma (tipo pelo vault, a regra do id em duas pastas) so existe nos testes.

    pub fn delete_item(&mut self, id: &str) -> Result<DocKind, String> {
        let vault = self.vault_or_error()?;
        let kind = match vault.find_by_id(id) {
            Ok(Some(Found::Note(..))) => DocKind::Note,
            Ok(Some(Found::Task(..))) => DocKind::Task,
            Ok(None) => return self.fail(format!("nao foi possivel excluir {id:?}: nao encontrado")),
            Err(e) => return self.fail(format!("nao foi possivel excluir {id:?}: {e}")),
        };
        self.delete_item_in(kind, id).map(|()| kind)
    }

    /// Executa o pedido na thread atual (so testes: bloqueia).
    pub fn load_now(&mut self, p: Request) -> Received {
        let r = crate::load::execute(p);
        self.receive(r)
    }
}

impl AppState {
    /// Nota rapida de uma vez: cria uma nota (ou, com o prefixo `[]`, uma
    /// tarefa) para a imagem atual, com a pagina e o enquadramento atuais e, se
    /// houver, o grifo pendente, e ja fixa o id. Casca fina sobre
    /// [`QuickNoteSession`]: digitar o texto todo e Cmd+Enter, com os trabalhos
    /// rodando aqui mesmo. BLOQUEIA (grava no vault): a interface usa a sessao,
    /// com os trabalhos no executor de fundo.
    pub fn submit_quick_note(&mut self, text: &str, now: NaiveDateTime) -> Result<Created, QuickNoteError> {
        let input = route_quick_note_input(text);
        let note_body = match &input {
            QuickNoteInput::Note(t) | QuickNoteInput::Task(t) => t.as_str(),
        };
        if note_body.is_empty() {
            return Err(QuickNoteError::Empty);
        }
        self.vault.as_ref().ok_or(QuickNoteError::NoVault)?;
        self.current_image().ok_or(QuickNoteError::NoImage)?;
        let mut s = QuickNoteSession::with_clock(Arc::new(move || now));
        let current_time = std::time::Instant::now();
        let e = s.typed(self, text, current_time);
        let mut e = s.run(self, e, current_time);
        let f = s.finish(self, current_time);
        e.join(s.run(self, f, current_time));
        match e.created {
            Some(c) => {
                self.confirmation = e.confirmation;
                Ok(c)
            }
            None => Err(QuickNoteError::Write(s.last_error().unwrap_or("nada foi gravado").to_string())),
        }
    }
}

#[test]
fn message_of_the_swap_of_vault_says_the_that_stayed_to_back() {
    test::register_shortcuts_false();
    let cleared = Swap::default();
    assert_eq!(cleared.message("v2", false, false), "vault: v2");
    assert_eq!(
        cleared.message("v2", true, true),
        "vault: v2 \u{b7} a nota em edicao e salva no vault anterior",
        "com item na sessao o rascunho da caixa nao e citado"
    );
    assert_eq!(cleared.message("v2", false, true), "vault: v2 \u{b7} o rascunho da nota continua na caixa");
    let t = Swap { discarded_highlight: true, closed_source: Some("f".into()) };
    assert_eq!(t.message("v2", false, false), "vault: v2 \u{b7} o grifo pendente ficou no vault anterior");
    assert_eq!(
        t.source_warning().as_deref(),
        Some("a fonte \"f\" nao e deste vault e foi fechada: ⌘O abre uma fonte aqui")
    );
    assert_eq!(cleared.source_warning(), None);
}

#[test]
fn the_other_layer_so_and_kept_se_the_state_the_has_in_the_screen() {
    let t = tempdir().unwrap();
    let mut st = state_with_page(t.path(), 3000, 1000);
    let j = ViewWindow::whole_page((3000, 1000));
    for c in [Layer::Full, Layer::Base, Layer::Tile(j)] {
        assert!(!st.keep_other_layer(c), "sem ajuste nada fica por baixo/por cima: {c:?}");
    }
    let (pv, fi) = preview_and_final(&mut st, highlight());
    image(st.load_now(fi));
    assert!(st.keep_other_layer(Layer::Base), "o tile esta na tela");
    assert!(!st.keep_other_layer(Layer::Tile(j)), "ainda sem base");
    image_or_layer(st.load_now(pv));
    assert!(st.keep_other_layer(Layer::Tile(j)), "a base chegou");
    assert!(!st.keep_other_layer(Layer::Full), "a cheia nunca mantem outra");
}

fn image_or_layer(r: Received) {
    assert!(matches!(r, Received::Image(..) | Received::Layer(_)));
}

#[test]
fn the_schedule_follows_the_image_when_the_base_arrives_and_so_a_time() {
    use crate::schedule_adjustment::ScheduleAdjustment;
    let t = tempdir().unwrap();
    let mut st = state_with_page(t.path(), 3000, 1000);
    let mut ag = ScheduleAdjustment::new();
    let current_time = Instant::now();
    assert_eq!(ag.image(), None);
    // uma imagem cheia nao mexe na agenda de imagem
    let r = Received::Error("x".into());
    assert_eq!(st.schedule_after_receive(&mut ag, &r, None, current_time), Followed::default());
    assert_eq!(ag.image(), None);
    let (pv, _fi) = preview_and_final(&mut st, highlight());
    let r = st.load_now(pv);
    assert!(matches!(r, Received::Image(_, _, Layer::Base)));
    let s = st.schedule_after_receive(&mut ag, &r, None, current_time);
    assert!(s.switched, "a agenda passou a imagem do quadro");
    assert_eq!(ag.image(), Some(&st.current.image));
    let s = st.schedule_after_receive(&mut ag, &r, None, current_time);
    assert_eq!(s, Followed::default(), "a mesma imagem de novo nao troca nada");
}

#[test]
fn the_schedule_asks_the_tile_and_the_write_and_so_in_the_image_its() {
    use crate::schedule_adjustment::ScheduleAdjustment;
    let t = tempdir().unwrap();
    let mut st = state_with_page(t.path(), 3000, 1000);
    let mut ag = ScheduleAdjustment::new();
    // a agenda ainda nao seguiu o quadro: nada a filtrar e o trabalho termina
    assert!(st.request_schedule(&mut ag, 1, highlight(), Quality::Tile).is_none());
    assert!(st.schedule_follow_image(&mut ag).switched);
    let t0 = Instant::now();
    assert!(ag.changed(highlight(), t0));
    ag.window_changed(ViewWindow::whole_page((3000, 1000)), t0, true);
    let steps = st.schedule_tick(&mut ag, t0 + Duration::from_millis(600));
    let Some(ScheduleStep::Filter { work, image_adjustments, quality }) = steps.first().cloned() else {
        panic!("o tile vencido sai primeiro: {steps:?}");
    };
    assert_eq!((quality, &image_adjustments), (Quality::Tile, &highlight()));
    assert!(steps.contains(&ScheduleStep::Write), "a gravacao vencida sai junto: {steps:?}");
    // o valor e do estado: o estado ainda esta no valor neutro, entao o pedido sai com o novo
    let p = st.request_schedule(&mut ag, work, image_adjustments, quality).expect("pedido do tile");
    assert_eq!(p.quality, Quality::Tile);
}

#[test]
fn adjustment_and_window_by_the_schedule_so_count_with_frame_and_ask_the_tick_when_ha_work() {
    use crate::schedule_adjustment::ScheduleAdjustment;
    let t = tempdir().unwrap();
    let mut empty = state(t.path());
    let mut ag = ScheduleAdjustment::new();
    let current_time = Instant::now();
    let j = ViewWindow::whole_page((3000, 1000));
    assert_eq!(empty.schedule_adjustment_changed(&mut ag, highlight(), current_time), ScheduleAfter::default(), "sem quadro nada muda");
    assert_eq!(empty.schedule_window_changed(&mut ag, j, current_time), ScheduleAfter::default());

    let mut st = state_no_window(t.path(), 3000, 1000);
    let r = st.schedule_window_changed(&mut ag, j, current_time);
    assert!(r.followed.switched, "a agenda seguiu o quadro");
    assert_eq!(st.view_window(), Some(j));
    assert!(!r.tick, "sem ajuste na tela nada a refiltrar");
    let r = st.schedule_adjustment_changed(&mut ag, highlight(), current_time);
    assert!(r.tick, "valor novo: o tile sai pela agenda");
    assert_eq!(st.adjustments, highlight(), "o valor vale ja na memoria");
    let r = st.schedule_adjustment_changed(&mut ag, highlight(), current_time);
    assert!(!r.tick, "o mesmo valor nao gera trabalho");
}

#[test]
fn touch_num_adjustment_with_the_eye_disabled_links_the_eye() {
    use crate::schedule_adjustment::ScheduleAdjustment;
    let t = tempdir().unwrap();
    let mut st = state_with_page(t.path(), 3000, 1000);
    let mut ag = ScheduleAdjustment::new();
    let current_time = Instant::now();
    st.schedule_follow_image(&mut ag);
    st.schedule_adjustment_changed(&mut ag, highlight(), current_time);
    st.schedule_toggle_preview(&mut ag, current_time);
    assert!(!st.show_adjustments());
    st.schedule_adjustment_changed(&mut ag, Adjustments { invert: true, ..Adjustments::default() }, current_time);
    assert!(st.show_adjustments(), "mexer religa o olho");
}

#[test]
fn rotate_the_frame_by_the_schedule_refilters_with_adjustment_and_asks_the_full_no() {
    use crate::schedule_adjustment::ScheduleAdjustment;
    let t = tempdir().unwrap();
    let mut st = state_with_page(t.path(), 3000, 1000);
    let mut ag = ScheduleAdjustment::new();
    let current_time = Instant::now();
    let (_, fresh) = st.rotate_frame(&mut ag, true);
    let fresh = fresh.expect("girou");
    assert_eq!(st.rotation(), fresh);
    match st.schedule_after_rotate(&mut ag, Some(ViewWindow::whole_page((1000, 3000))), current_time) {
        RotationAfter::Full(p) => assert_eq!(p.quality, Quality::Full),
        _ => panic!("sem ajuste a cheia girada sai direto"),
    }
    st.schedule_adjustment_changed(&mut ag, highlight(), current_time);
    let (_, other) = st.rotate_frame(&mut ag, true);
    assert!(other.is_some());
    assert!(matches!(
        st.schedule_after_rotate(&mut ag, Some(ViewWindow::whole_page((3000, 1000))), current_time),
        RotationAfter::Refilter { .. }
    ));
    let mut empty = state(t.path());
    assert!(empty.rotate_frame(&mut ag, true).1.is_none(), "sem quadro nao gira");
}

#[test]
fn close_source_requested_confirms_with_the_name_or_warns_that_not_ha() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    assert_eq!(st.close_source_requested(), None);
    assert_eq!(st.warning.as_deref(), Some("nenhuma fonte aberta"));
    let mut st = state_with_page(t.path(), 30, 10);
    st.warning = None;
    assert_eq!(st.close_source_requested().as_deref(), Some("fonte fechada: Livro \u{b7} \u{2318}O abre outra"));
    assert!(!st.source_has() && st.warning.is_none());
}

#[test]
fn moved_is_worth_with_request_or_navigation_pending() {
    let t = tempdir().unwrap();
    let mut st = state_with_page(t.path(), 30, 10);
    assert!(!st.moved(&None));
    let request = st.go_next();
    assert_eq!(st.moved(&request), request.is_some() || st.has_navigation_pending());
    let p = st.open_frame(Frame::new("Livro", "g.png", 1));
    assert!(st.moved(&p), "um pedido de quadro e andar");
}

/// O texto pedido para quando a outra fonte entrar tambem segue a renomeacao;
/// outro id nao muda; sem pedido nada acontece.
#[test]
fn load_postponed_follows_the_renaming_and_leaves_a_time() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    st.load_postponed_follow_renaming("x", "y");
    assert!(!st.has_load_postponed());
    st.postpone_load(DocKind::Note, "20261112-1432-sem-texto");
    st.load_postponed_follow_renaming("outro", "x");
    st.load_postponed_follow_renaming("20261112-1432-sem-texto", "20261112-1432-maria");
    assert_eq!(st.resume_load_postponed(), Some((DocKind::Note, "20261112-1432-maria".to_string())));
    assert_eq!(st.resume_load_postponed(), None);
}

#[test]
fn drop_load_postponed_warns_no_erase_the_warning_that_already_was() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    st.drop_load_postponed("nada a avisar");
    assert!(st.warning.is_none(), "sem pedido nao ha aviso");
    st.postpone_load(DocKind::Task, "t1");
    st.drop_load_postponed("a fonte nao abriu");
    assert_eq!(st.warning.as_deref(), Some("o texto de t1 nao foi carregado: a fonte nao abriu"));
    st.postpone_load(DocKind::Task, "t2");
    st.drop_load_postponed("outra fonte");
    assert_eq!(
        st.warning.as_deref(),
        Some("o texto de t1 nao foi carregado: a fonte nao abriu \u{b7} o texto de t2 nao foi carregado: outra fonte")
    );
}

#[test]
fn click_in_a_source_no_path_valid_becomes_pending_and_warning() {
    use crate::drawers::{LineSource, DrawerSourceStatus};
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let ok = LineSource { name: "Livro".into(), kind: SourceKind::Folder, status: DrawerSourceStatus::Ok, file_path: Some("/x/livro".into()) };
    assert_eq!(st.source_line_path(&ok), Some("/x/livro".into()));
    assert!(st.warning.is_none() && st.pending().is_none());
    let vanished = LineSource { status: DrawerSourceStatus::NoPath, file_path: None, ..ok };
    assert_eq!(st.source_line_path(&vanished), None);
    assert!(st.warning.is_some(), "o aviso diz por que");
    assert_eq!(st.pending().map(|p| p.name.as_str()), Some("Livro"));
}

#[test]
fn who_follows_the_renaming_depends_of_the_result_and_of_the_vault() {
    let all = RenamingFollows { session: true, screen: true, confirms: true };
    assert_eq!(renaming_follower(Ok(()), true), all);
    assert_eq!(renaming_follower(Ok(()), false), RenamingFollows { session: true, screen: false, confirms: false });
    assert_eq!(
        renaming_follower(Err(FailureRename::PartialNoOld), true),
        RenamingFollows { session: true, screen: true, confirms: false },
        "so o nome novo ficou: segue, sem confirmar"
    );
    assert_eq!(
        renaming_follower(Err(FailureRename::PartialNoOld), false),
        RenamingFollows { session: true, screen: false, confirms: false }
    );
    let nothing = RenamingFollows { session: false, screen: false, confirms: false };
    assert_eq!(renaming_follower(Err(FailureRename::Common), true), nothing);
    assert_eq!(renaming_follower(Err(FailureRename::Partial), true), nothing);
}

#[test]
fn status_info_brings_vault_source_and_frame() {
    let t = tempdir().unwrap();
    let st = state(t.path());
    assert_eq!(st.status_info(), crate::layout::StatusInfo::default(), "sem vault e sem fonte");
    let st = state_with_page(t.path(), 30, 10);
    let i = st.status_info();
    assert_eq!(i.source.as_deref(), Some("Livro"));
    assert_eq!(i.file.as_deref(), Some("g.png"));
    assert_eq!(i.page, Some(1));
    assert_eq!(i.total_images, Some(1));
}

#[test]
fn messages_of_status_follow_the_order_progress_confirmation_warning() {
    use crate::layout::StatusMessage as M;
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    assert!(st.status_messages(false, false, None).is_empty());
    assert_eq!(st.status_messages(true, false, None), vec![M::Progress("abrindo fonte...".into())]);
    st.confirmation = Some("ok".into());
    st.warning = Some("cuidado".into());
    assert_eq!(
        st.status_messages(true, true, Some("renomeando a fonte...")),
        vec![
            M::Progress("abrindo fonte...".into()),
            M::Progress("carregando nota\u{2026}".into()),
            M::Progress("renomeando a fonte...".into()),
            M::Confirmation("ok".into()),
            M::Warning("cuidado".into()),
        ]
    );
}

#[test]
fn go_to_number_limits_to_the_first_and_to_the_last() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    let p = st.adopt_source("Livro", fake(&names(10))).expect("primeiro quadro");
    image(st.load_now(p));
    assert_eq!(st.unit_go_to(), Some((UnitIr::Image, 1, 10)));

    let p = st.go_to_number(5).unwrap().expect("vai");
    assert_eq!(st.current, Frame::new("Livro", "p05.png", 1));
    image(st.load_now(p));
    // acima do limite: a ultima; 0 e negativo: a primeira
    let p = st.go_to_number(999).unwrap().expect("vai");
    assert_eq!(st.current, Frame::new("Livro", "p10.png", 1));
    image(st.load_now(p));
    let p = st.go_to_number(0).unwrap().expect("vai");
    assert_eq!(st.current, Frame::new("Livro", "p01.png", 1));
    image(st.load_now(p));
    let p = st.go_to_number(-7).unwrap();
    assert!(p.is_none(), "ja esta na primeira");
    // ja estamos na imagem pedida: nada a fazer
    assert!(st.go_to_number(1).unwrap().is_none());
}

#[test]
fn go_to_number_no_source_warns() {
    let t = tempdir().unwrap();
    let mut st = state(t.path());
    assert!(st.go_to_number(3).is_err());
}

#[test]
fn target_and_text_of_the_go_to() {
    assert_eq!(target_go_to(0, 10), 1);
    assert_eq!(target_go_to(-4, 10), 1);
    assert_eq!(target_go_to(7, 10), 7);
    assert_eq!(target_go_to(11, 10), 10);
    assert_eq!(target_go_to(i64::MAX, 10), 10);
    assert_eq!(interpret_go_to(" 12 "), Some(12));
    assert_eq!(interpret_go_to("0"), Some(0));
    assert_eq!(interpret_go_to("-3"), Some(-3));
    assert_eq!(interpret_go_to("99999999999999999999999"), Some(i64::MAX));
    assert_eq!(interpret_go_to(""), None);
    assert_eq!(interpret_go_to("abc"), None);
    assert_eq!(interpret_go_to("1,5"), None);
    assert_eq!(interpret_go_to("-"), None);
}
