use super::*;
use image::Rgba;

#[test]
fn reduces_so_above_of_the_limit_keeping_proportion() {
    assert_eq!(reduced_size(100, 50, 200), (100, 50));
    assert_eq!(reduced_size(400, 100, 200), (200, 50));
    assert_eq!(reduced_size(100, 400, 200), (50, 200));
    assert_eq!(reduced_size(20000, 1, 16384), (16384, 1));
    assert_eq!(reduced_size(0, 0, 16384), (0, 0));
}

/// A copia reduzida da base (agora pela reamostragem paralela do recorte) e a
/// mesma, byte a byte, do `imageops::resize` Triangle serial de antes.
#[test]
fn base_reduced_identical_to_the_resize_of_the_image() {
    for (w, h) in [(3000u32, 1000u32), (1000, 4097), (2049, 2049)] {
        let mut s: u32 = w ^ (h << 8);
        let img = RgbaImage::from_fn(w, h, |_, _| {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let b = s.to_le_bytes();
            Rgba([b[3], b[2], b[1], b[0] | 0x80])
        });
        let (rw, rh) = reduced_size(w, h, SIDE_BASE);
        let old = imageops::resize(&img, rw, rh, FilterType::Triangle);
        let p = Page::new(Arc::new(DecodedPage { image: img, page_count: 1 }));
        assert!(*p.base() == old, "{w}x{h}");
    }
}

#[test]
fn display_in_bgra_with_size_original() {
    let img = RgbaImage::from_pixel(4, 2, Rgba([10, 20, 30, 255]));
    let e = to_display_with_limit(&img, &Adjustments::default(), 16384);
    assert_eq!(e.sz_original, (4, 2));
    assert_eq!(e.bgra.dimensions(), (4, 2));
    assert_eq!(e.bgra.get_pixel(0, 0).0, [30, 20, 10, 255]);
}

#[test]
fn display_rotated_swap_os_sides_and_keeps_the_size_original() {
    let mut img = RgbaImage::from_pixel(4, 2, Rgba([0, 0, 0, 255]));
    img.put_pixel(0, 0, Rgba([10, 20, 30, 255]));
    img.put_pixel(3, 1, Rgba([40, 50, 60, 255]));
    for (rot, dims, corner_a, corner_b) in [
        (Rotation::R0, (4, 2), (0, 0), (3, 1)),
        // 90 graus horarios: o canto de cima, a esquerda, vai para o de cima, a direita.
        (Rotation::R90, (2, 4), (1, 0), (0, 3)),
        (Rotation::R180, (4, 2), (3, 1), (0, 0)),
        (Rotation::R270, (2, 4), (0, 3), (1, 0)),
    ] {
        let e = to_display_with_limit(&img, &Adjustments::default(), 16384).rotated(rot);
        assert_eq!((e.bgra.dimensions(), e.sz_original, e.rot), (dims, (4, 2), rot), "{rot:?}");
        // BGRA: os dois pixels marcados, nos cantos certos.
        assert_eq!(e.bgra.get_pixel(corner_a.0, corner_a.1).0, [30, 20, 10, 255], "{rot:?}");
        assert_eq!(e.bgra.get_pixel(corner_b.0, corner_b.1).0, [60, 50, 40, 255], "{rot:?}");
    }
}

#[test]
fn display_no_rotation_not_copy_os_pixels() {
    let img = RgbaImage::from_pixel(4, 2, Rgba([1, 2, 3, 255]));
    let e = to_display_with_limit(&img, &Adjustments::default(), 16384);
    let before = Arc::as_ptr(&e.bgra);
    let e = e.rotated(Rotation::R0);
    assert_eq!(Arc::as_ptr(&e.bgra), before);
    assert_eq!(e.rot, Rotation::R0);
}

#[test]
fn image_larger_that_the_texture_and_reduced_but_keeps_the_size_original() {
    let img = RgbaImage::from_pixel(40, 10, Rgba([200, 0, 0, 255]));
    let e = to_display_with_limit(&img, &Adjustments::default(), 16);
    assert_eq!(e.sz_original, (40, 10));
    assert_eq!(e.bgra.dimensions(), (16, 4));
    assert_eq!(e.bgra.get_pixel(8, 2).0, [0, 0, 200, 255]);
}

#[test]
fn filter_interrupted_not_delivers_image() {
    let img = RgbaImage::from_pixel(64, 64, Rgba([10, 20, 30, 255]));
    let a = Adjustments { sharpen: 0.5, highpass: 0.5, invert: true, ..Adjustments::default() };
    let calls = std::sync::atomic::AtomicUsize::new(0);
    // vira verdadeiro no meio do trabalho (depois de algumas checagens)
    let stop = || calls.fetch_add(1, Ordering::SeqCst) >= 3;
    assert!(cancellable_display(&img, (64, 64), &a, SIDE_MAX_TEXTURE, &stop).is_none());
    assert!(calls.load(Ordering::SeqCst) > 3, "o filtro consultou `parar` no meio");
    assert!(cancellable_display(&img, (64, 64), &a, SIDE_MAX_TEXTURE, &|| true).is_none());
    // sem parada: o mesmo resultado de antes
    let e = cancellable_display(&img, (64, 64), &a, SIDE_MAX_TEXTURE, &|| false).unwrap();
    let reference = to_display_with_limit(&img, &a, SIDE_MAX_TEXTURE);
    assert_eq!(*e.bgra, *reference.bgra);
}

#[test]
fn base_reduced_and_created_a_time_and_reused() {
    let large = Arc::new(DecodedPage { image: RgbaImage::from_pixel(3000, 1000, Rgba([1, 2, 3, 255])), page_count: 1 });
    let p = Page::new(Arc::clone(&large));
    assert!(!p.created_base(), "a copia reduzida so nasce na primeira base");
    let a = p.base() as *const RgbaImage;
    assert_eq!(p.base().dimensions(), (2048, 683));
    assert!(std::ptr::eq(a, p.base()), "a mesma copia e reusada");
    assert!(Arc::ptr_eq(&p.decoded, &large), "a pagina cheia continua compartilhada");

    let small = Page::new(Arc::new(DecodedPage { image: RgbaImage::new(100, 2048), page_count: 1 }));
    assert!(std::ptr::eq(small.base(), &small.decoded.image), "ja cabe: sem copia");
    assert_eq!(SIDE_BASE, 2048);
}

/// Pagina com textura (o filtro com desfoque tem o que mudar), sem gpui.
fn default(w: u32, h: u32) -> RgbaImage {
    RgbaImage::from_fn(w, h, |x, y| {
        let v = ((x * 37 + y * 91 + (x * y) % 23) % 256) as u8;
        Rgba([v, v.wrapping_mul(3), 255 - v, 255])
    })
}

fn page(img: RgbaImage) -> Page {
    Page::new(Arc::new(DecodedPage { image: img, page_count: 1 }))
}

fn highlight() -> Adjustments {
    Adjustments { sharpen: 0.6, highpass: 0.7, ..Adjustments::default() }
}

fn rgba_of(bgra: &RgbaImage) -> RgbaImage {
    let mut out = bgra.clone();
    for px in out.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
    }
    out
}

#[test]
fn window_in_the_screen_uses_the_grid_of_the_screen_no_enlarge() {
    // afastado: a pagina inteira ocupa 1000x500 px fisicos -> grade 1000x500
    let j = ViewWindow::in_screen((4000, 2000), (1000.0, 500.0), [0.25, 0.5, 0.75, 1.0]);
    assert_eq!(j.grid, (1000, 500));
    assert_eq!(j.region, GridRegion { x0: 250, y0: 250, x1: 750, y1: 500 });
    assert_eq!(j.size_px(), (500, 250));
    assert!((j.scale((4000, 2000)) - 0.25).abs() < 1e-6);
    // aproximado alem do nativo: a grade nunca passa da pagina
    let j = ViewWindow::in_screen((4000, 2000), (8000.0, 4000.0), [0.25, 0.5, 0.75, 1.0]);
    assert_eq!(j.grid, (4000, 2000));
    assert_eq!(j.region, GridRegion { x0: 1000, y0: 1000, x1: 3000, y1: 2000 });
    assert_eq!(j.scale((4000, 2000)), 1.0);
    // arredonda para fora e grampeia na grade
    let j = ViewWindow::in_screen((100, 100), (100.0, 100.0), [0.105, -0.5, 0.501, 1.7]);
    assert_eq!(j.region, GridRegion { x0: 10, y0: 0, x1: 51, y1: 100 });
    // area sem tamanho / valores degenerados: grade minima, sem panico
    let j = ViewWindow::in_screen((100, 50), (0.0, f32::NAN), [0.0, 0.0, 1.0, 1.0]);
    assert_eq!(j.grid, (1, 50));
    assert!(!j.empty());
    assert!(ViewWindow::in_screen((100, 50), (100.0, 50.0), [0.5, 0.5, 0.5, 0.9]).empty());
}

#[test]
fn window_of_the_page_whole_fits_in_the_texture() {
    let j = ViewWindow::whole_page((3000, 1000));
    assert_eq!(j.grid, (3000, 1000));
    assert_eq!(j.region, GridRegion { x0: 0, y0: 0, x1: 3000, y1: 1000 });
    let j = ViewWindow::whole_page((20000, 10000));
    assert_eq!(j.grid, (SIDE_MAX_TEXTURE, 8192));
    assert_eq!(j.size_px(), (SIDE_MAX_TEXTURE, 8192));
}

#[test]
fn window_normalized_clamps_in_the_page_and_in_the_grid() {
    let j = ViewWindow { grid: (500, 80), region: GridRegion { x0: 50, y0: 10, x1: 900, y1: 100 } };
    let n = j.normalized((200, 100));
    assert_eq!(n.grid, (200, 80));
    assert_eq!(n.region, GridRegion { x0: 50, y0: 10, x1: 200, y1: 80 });
    let outside = ViewWindow { grid: (100, 100), region: GridRegion { x0: 150, y0: 0, x1: 190, y1: 10 } };
    assert!(outside.normalized((100, 100)).empty());
    assert_eq!(outside.normalized((100, 100)).region, GridRegion::default());
}

#[test]
fn tile_has_exactly_the_size_of_the_window() {
    let p = page(default(300, 200));
    for (grid, r) in [
        ((150, 100), GridRegion { x0: 10, y0: 20, x1: 60, y1: 70 }),
        ((300, 200), GridRegion { x0: 0, y0: 0, x1: 300, y1: 1 }),
        ((97, 61), GridRegion { x0: 90, y0: 55, x1: 97, y1: 61 }),
    ] {
        let j = ViewWindow { grid, region: r };
        let e = cancellable_tile(&p, j, &highlight(), &|| false).expect("sem parada");
        assert_eq!(e.bgra.dimensions(), j.size_px(), "a margem e aparada: {j:?}");
        assert_eq!(e.sz_original, (300, 200));
    }
    // janela vazia: imagem 0x0, sem panico
    let empty = ViewWindow { grid: (150, 100), region: GridRegion { x0: 10, y0: 20, x1: 10, y1: 70 } };
    assert_eq!(cancellable_tile(&p, empty, &highlight(), &|| false).unwrap().bgra.dimensions(), (0, 0));
}

/// Janela cobrindo a pagina inteira na escala 1 = o filtro antigo (final cheia).
#[test]
fn tile_of_the_page_whole_in_scale_1_and_the_filter_old() {
    let img = default(64, 48);
    let p = page(img.clone());
    for a in [highlight(), Adjustments { sharpen: 0.8, ..Adjustments::default() }, Adjustments { invert: true, ..Adjustments::default() }] {
        let old = to_display_with_limit(&img, &a, SIDE_MAX_TEXTURE);
        let tile = cancellable_tile(&p, ViewWindow::whole_page((64, 48)), &a, &|| false).unwrap();
        assert_eq!(*tile.bgra, *old.bgra);
        // um pedaco na escala 1 e o mesmo pedaco do filtro antigo
        let j = ViewWindow { grid: (64, 48), region: GridRegion { x0: 5, y0: 7, x1: 40, y1: 30 } };
        let piece = cancellable_tile(&p, j, &a, &|| false).unwrap();
        assert_eq!(*piece.bgra, imageops::crop_imm(&*old.bgra, 5, 7, 35, 23).to_image());
    }
}

/// Fora da escala 1 o tile e o pedaco da pagina reamostrada para a grade e
/// filtrada com o sigma da escala (o realce tem o mesmo alcance relativo).
#[test]
fn tile_reduced_follows_the_sigma_of_the_scale() {
    let img = default(240, 160);
    let p = page(img.clone());
    let grid = (120, 80);
    let scale = grid_scale((240, 160), grid);
    let whole = imageops::resize(&img, grid.0, grid.1, FilterType::Triangle);
    let filtered = filters::apply_cancellable_in_scale(whole, &highlight(), scale, &|| false).unwrap();
    let j = ViewWindow { grid, region: GridRegion { x0: 30, y0: 20, x1: 90, y1: 70 } };
    let tile = cancellable_tile(&p, j, &highlight(), &|| false).unwrap();
    assert_eq!(rgba_of(&tile.bgra), imageops::crop_imm(&filtered, 30, 20, 60, 50).to_image());
    // e difere do filtro com o sigma cheio (a escala importa)
    let no_scale = filters::apply_cancellable_in(imageops::resize(&img, 120, 80, FilterType::Triangle), &highlight(), &|| false).unwrap();
    assert_ne!(rgba_of(&tile.bgra), imageops::crop_imm(&no_scale, 30, 20, 60, 50).to_image());

    // grade com proporcao diferente da pagina (arredondamentos por eixo): o
    // mesmo contrato, com a escala de `grid_scale` (media dos eixos)
    for (grid, r) in [
        ((120, 100), GridRegion { x0: 7, y0: 3, x1: 77, y1: 99 }),
        ((61, 160), GridRegion { x0: 0, y0: 40, x1: 61, y1: 41 }),
        ((240, 33), GridRegion { x0: 200, y0: 0, x1: 240, y1: 33 }),
    ] {
        let scale = grid_scale((240, 160), grid);
        let whole = imageops::resize(&img, grid.0, grid.1, FilterType::Triangle);
        let filtered = filters::apply_cancellable_in_scale(whole, &highlight(), scale, &|| false).unwrap();
        let j = ViewWindow { grid, region: r };
        let tile = cancellable_tile(&p, j, &highlight(), &|| false).unwrap();
        let (w, h) = j.size_px();
        assert_eq!(tile.bgra.dimensions(), (w, h));
        assert_eq!(rgba_of(&tile.bgra), imageops::crop_imm(&filtered, r.x0, r.y0, w, h).to_image(), "{grid:?}");
    }
}

#[test]
fn source_of_the_tile_and_reused_between_values_and_redone_when_the_window_changes() {
    let p = page(default(200, 120));
    let j1 = ViewWindow { grid: (100, 60), region: GridRegion { x0: 10, y0: 10, x1: 50, y1: 40 } };
    assert_eq!(p.sources_tile_created(), 0);
    cancellable_tile(&p, j1, &highlight(), &|| false).unwrap();
    cancellable_tile(&p, j1, &Adjustments { sharpen: 0.2, ..Adjustments::default() }, &|| false).unwrap();
    cancellable_tile(&p, j1, &Adjustments { invert: true, ..Adjustments::default() }, &|| false).unwrap();
    assert_eq!(p.sources_tile_created(), 1, "so os valores mudaram: o recorte reamostrado e o mesmo");
    // outro retangulo na mesma grade
    let j2 = ViewWindow { region: GridRegion { x0: 11, ..j1.region }, ..j1 };
    cancellable_tile(&p, j2, &highlight(), &|| false).unwrap();
    assert_eq!(p.sources_tile_created(), 2);
    // mesmo retangulo em outra grade (outra escala)
    let j3 = ViewWindow { grid: (120, 72), ..j2 };
    cancellable_tile(&p, j3, &highlight(), &|| false).unwrap();
    assert_eq!(p.sources_tile_created(), 3);
    cancellable_tile(&p, j3, &highlight(), &|| false).unwrap();
    assert_eq!(p.sources_tile_created(), 3);
}

#[test]
fn tile_interrupted_not_delivers_image() {
    let p = page(default(256, 256));
    let j = ViewWindow::whole_page((256, 256));
    assert!(cancellable_tile(&p, j, &highlight(), &|| true).is_none());
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let stop = || calls.fetch_add(1, Ordering::SeqCst) >= 3;
    assert!(cancellable_tile(&p, j, &highlight(), &stop).is_none());
}

#[test]
fn base_filters_the_copy_reduced_with_the_sigma_of_the_scale() {
    let img = default(4096, 1024);
    let p = page(img.clone());
    let e = cancellable_base(&p, &highlight(), &|| false).unwrap();
    assert_eq!((e.bgra.dimensions(), e.sz_original), ((2048, 512), (4096, 1024)));
    let reduced = imageops::resize(&img, 2048, 512, FilterType::Triangle);
    let expected = filters::apply_cancellable_in_scale(reduced, &highlight(), 0.5, &|| false).unwrap();
    assert_eq!(rgba_of(&e.bgra), expected);
}

#[test]
fn adjustments_are_applied_before_of_the_swap_of_channels() {
    let img = RgbaImage::from_pixel(2, 2, Rgba([10, 20, 30, 255]));
    let inv = Adjustments { invert: true, ..Adjustments::default() };
    let e = to_display_with_limit(&img, &inv, 16384);
    assert_eq!(e.bgra.get_pixel(1, 1).0, [225, 235, 245, 255]);
}

/// Ajustes + reducao + BGRA, sem parada (testes de compatibilidade).
pub fn to_display_with_limit(img: &RgbaImage, a: &Adjustments, side_max: u32) -> Display {
    cancellable_display(img, img.dimensions(), a, side_max, &|| false).expect("sem parada")
}

impl Page {
    pub fn created_base(&self) -> bool {
        self.reduced.get().is_some()
    }

    /// Quantos recortes de tile foram reamostrados nesta pagina.
    pub fn sources_tile_created(&self) -> u64 {
        self.tile_sources.load(Ordering::SeqCst)
    }
}
