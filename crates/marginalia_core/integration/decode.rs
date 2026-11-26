use marginalia_core::decode::{decode_page, DecodeError};
use marginalia_core::Error;

fn fx(name: &str) -> Vec<u8> {
    let p = format!("{}/integration/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("fixture {p}: {e}"))
}

#[test]
fn jpg_and_png_has_a_page() {
    let j = decode_page("a.jpg", &fx("amostra.jpg"), 1).unwrap();
    assert_eq!(j.page_count, 1);
    assert_eq!(j.image.dimensions(), (40, 30));
    let p = decode_page("a.png", &fx("amostra.png"), 1).unwrap();
    assert_eq!(p.page_count, 1);
    assert_eq!(p.image.dimensions(), (40, 30));
    assert!(matches!(
        decode_page("a.png", &fx("amostra.png"), 2),
        Err(DecodeError::MissingPage { requested: 2, total: 1 })
    ));
}

#[test]
fn tiff_two_pages_of_sizes_different() {
    let b = fx("duas.tif");
    let p1 = decode_page("duas.tif", &b, 1).unwrap();
    assert_eq!(p1.page_count, 2);
    assert_eq!(p1.image.dimensions(), (20, 10));
    let p2 = decode_page("duas.tif", &b, 2).unwrap();
    assert_eq!(p2.page_count, 2);
    assert_eq!(p2.image.dimensions(), (30, 15));
    assert_ne!(p1.image.dimensions(), p2.image.dimensions());
    // pixels corretos: pagina 1 vermelha, pagina 2 verde
    assert_eq!(p1.image.get_pixel(0, 0).0, [200, 0, 0, 255]);
    assert_eq!(p2.image.get_pixel(0, 0).0, [0, 200, 0, 255]);
}

#[test]
fn tiff_page_outside_of_the_range() {
    let b = fx("duas.tif");
    assert!(matches!(
        decode_page("duas.tif", &b, 3),
        Err(DecodeError::MissingPage { requested: 3, total: 2 })
    ));
    assert!(matches!(
        decode_page("duas.tif", &b, 0),
        Err(DecodeError::MissingPage { requested: 0, .. })
    ));
}

#[test]
fn pdf_three_pages() {
    let b = fx("tres.pdf");
    let p1 = decode_page("tres.pdf", &b, 1).unwrap();
    assert_eq!(p1.page_count, 3);
    let p2 = decode_page("tres.pdf", &b, 2).unwrap();
    let p3 = decode_page("tres.pdf", &b, 3).unwrap();
    // paginas de tamanhos diferentes (200x100, 100x200, 300x300 pt; escala x2)
    assert_eq!(p1.image.dimensions(), (400, 200));
    assert_eq!(p2.image.dimensions(), (200, 400));
    assert_eq!(p3.image.dimensions(), (600, 600));
    // fundo branco opaco
    assert_eq!(p1.image.get_pixel(5, 5).0, [255, 255, 255, 255]);
}

#[test]
fn pdf_page_missing() {
    let b = fx("tres.pdf");
    assert!(matches!(
        decode_page("tres.pdf", &b, 4),
        Err(DecodeError::MissingPage { requested: 4, total: 3 })
    ));
    assert!(matches!(
        decode_page("tres.pdf", &b, 0),
        Err(DecodeError::MissingPage { requested: 0, .. })
    ));
}

#[test]
fn pdf_large_limited_the_2400() {
    let b = fx("grande.pdf");
    let p = decode_page("grande.pdf", &b, 1).unwrap();
    let (w, h) = p.image.dimensions();
    assert_eq!(w.max(h), 2400);
    assert!(w < h); // 1500x3000 pt, retrato
}

#[test]
fn corrupt_and_invalid_no_panic() {
    let b = fx("corrompido.jpg");
    assert!(matches!(decode_page("corrompido.jpg", &b, 1), Err(DecodeError::Invalid(_))));
}

#[test]
fn uppercase_extensions() {
    assert_eq!(decode_page("X.TIF", &fx("duas.tif"), 2).unwrap().page_count, 2);
    assert_eq!(decode_page("X.PDF", &fx("tres.pdf"), 1).unwrap().page_count, 3);
    assert_eq!(decode_page("X.JPG", &fx("amostra.jpg"), 1).unwrap().page_count, 1);
    assert_eq!(decode_page("X.Jpeg", &fx("amostra.jpg"), 1).unwrap().page_count, 1);
    assert_eq!(decode_page("X.tiff", &fx("duas.tif"), 1).unwrap().page_count, 2);
}

#[test]
fn truncated_are_invalid() {
    let t = fx("duas.tif");
    let t = &t[..t.len() / 2];
    assert!(matches!(decode_page("a.tif", t, 1), Err(DecodeError::Invalid(_))));
    assert!(matches!(decode_page("a.tif", t, 2), Err(DecodeError::Invalid(_))));
    let p = fx("tres.pdf");
    let p = &p[..p.len() / 2];
    assert!(matches!(decode_page("a.pdf", p, 1), Err(DecodeError::Invalid(_))));
}

#[test]
fn empty_and_invalid() {
    for name in ["a.jpg", "a.png", "a.tif", "a.pdf"] {
        assert!(matches!(decode_page(name, &[], 1), Err(DecodeError::Invalid(_))), "{name}");
    }
}

#[test]
fn unknown_extension() {
    assert!(matches!(decode_page("a.bmp", &fx("amostra.png"), 1), Err(DecodeError::NotSupported(_))));
    assert!(matches!(decode_page("semext", &fx("amostra.png"), 1), Err(DecodeError::NotSupported(_))));
}

#[test]
fn converts_to_error_of_the_crate() {
    let e: Error = DecodeError::Invalid("x".into()).into();
    assert!(e.to_string().contains("x"));
}

#[test]
fn pdf_of_several_threads() {
    let b = std::sync::Arc::new(fx("tres.pdf"));
    let hs: Vec<_> = (0..4u32)
        .map(|i| {
            let b = b.clone();
            std::thread::spawn(move || {
                for k in 0..5 {
                    let pg = 1 + (i + k) % 3;
                    let d = decode_page("tres.pdf", &b, pg).unwrap();
                    assert_eq!(d.page_count, 3);
                }
            })
        })
        .collect();
    for h in hs {
        h.join().unwrap();
    }
}


#[test]
fn tiff_16_bits_gray_reduces_to_8() {
    let d = decode_page("g16.tif", &fx("cinza16.tif"), 1).unwrap();
    assert_eq!(d.image.dimensions(), (4, 2));
    let v: Vec<u8> = d.image.pixels().map(|p| p.0[0]).collect();
    assert_eq!(v, vec![255, 128, 0, 255, 1, 2, 127, 64]);
    assert_eq!(d.image.get_pixel(1, 0).0, [128, 128, 128, 255]);
}

#[test]
fn tiff_1_bit_width_not_multiple_of_8() {
    for name in ["bilevel13.tif", "bilevel13_packbits.tif", "bilevel13_lzw.tif", "bilevel13_deflate.tif"] {
        let d = decode_page(name, &fx(name), 1).unwrap();
        assert_eq!(d.image.dimensions(), (13, 2), "{name}");
        let line = |y: u32| -> String {
            (0..13).map(|x| if d.image.get_pixel(x, y).0 == [255, 255, 255, 255] { '1' } else { '0' }).collect()
        };
        assert_eq!(line(0), "1010101010101", "{name}");
        assert_eq!(line(1), "1100110011001", "{name}");
        assert_eq!(d.image.get_pixel(1, 0).0, [0, 0, 0, 255]);
    }
}

#[test]
fn tiff_1_bit_white_and_zero_and_inverted() {
    let d = decode_page("w.tif", &fx("bilevel13_whiteiszero.tif"), 1).unwrap();
    // mesmos bits gravados, mas 0 = branco: o padrao visual se inverte
    assert_eq!(d.image.get_pixel(0, 0).0, [0, 0, 0, 255]);
    assert_eq!(d.image.get_pixel(1, 0).0, [255, 255, 255, 255]);
}

#[test]
fn tiff_1_bit_buffer_short_not_enters_in_panic() {
    // fixture com altura declarada maior que os dados: sem indice fora dos limites
    let r = decode_page("c.tif", &fx("bilevel_curto.tif"), 1);
    assert!(matches!(r, Err(DecodeError::Invalid(_))), "{r:?}");
}

#[test]
fn tiff_compression_ccitt_and_not_supported() {
    let r = decode_page("g4.tif", &fx("ccitt_g4.tif"), 1);
    assert!(matches!(r, Err(DecodeError::NotSupported(_))), "{r:?}");
}

#[test]
fn tiff_palette_and_not_supported() {
    let r = decode_page("p.tif", &fx("paleta.tif"), 1);
    assert!(matches!(r, Err(DecodeError::NotSupported(_))), "{r:?}");
}

fn lines(d: &marginalia_core::decode::DecodedPage) -> Vec<String> {
    let (w, h) = d.image.dimensions();
    (0..h)
        .map(|y| {
            (0..w)
                .map(|x| match d.image.get_pixel(x, y).0 {
                    [255, 255, 255, 255] => '1',
                    [0, 0, 0, 255] => '0',
                    o => panic!("pixel inesperado {o:?}"),
                })
                .collect()
        })
        .collect()
}

#[test]
fn packbits_with_runs_replicated() {
    let d = decode_page("r.tif", &fx("bilevel24_replicado.tif"), 1).unwrap();
    assert_eq!(
        lines(&d),
        vec!["111111111111111111111111", "000011110000111110101010"]
    );
}

#[test]
fn tiff_1_bit_several_strips() {
    let expected = vec!["1010101010101", "1100110011001", "1111000011110", "0000111100001", "1000000000001"];
    for name in [
        "multi5_none.tif",
        "multi5_packbits.tif",
        "multi5_lzw.tif",
        "multi5_deflate.tif",
    ] {
        let d = decode_page(name, &fx(name), 1).unwrap();
        assert_eq!(d.image.dimensions(), (13, 5), "{name}");
        assert_eq!(lines(&d), expected, "{name}");
    }
}

#[test]
fn tiff_huge_and_refused_no_abort() {
    let r = decode_page("h.tif", &fx("enorme.tif"), 1);
    assert!(matches!(r, Err(DecodeError::NotSupported(_))), "{r:?}");
}

#[test]
fn strips_that_expand_beyond_of_the_declared_are_invalid() {
    for name in ["expande_deflate.tif", "expande_lzw.tif", "expande_packbits.tif"] {
        let r = decode_page(name, &fx(name), 1);
        assert!(matches!(r, Err(DecodeError::Invalid(_))), "{name}: {r:?}");
    }
}

#[test]
fn fillorder_2_and_predictor_are_not_supported() {
    for name in ["fillorder2.tif", "predictor2.tif", "tiles.tif"] {
        let r = decode_page(name, &fx(name), 1);
        assert!(matches!(r, Err(DecodeError::NotSupported(_))), "{name}: {r:?}");
    }
}

#[test]
fn strips_beyond_of_the_file_are_invalid() {
    let r = decode_page("o.tif", &fx("strip_fora.tif"), 1);
    assert!(matches!(r, Err(DecodeError::Invalid(_))), "{r:?}");
}

/// Gera os fixtures: `cargo test -p marginalia_core --test decode -- --ignored gerar_fixtures`
#[test]
#[ignore]
fn generate_fixtures() {
    use image::{Rgb, RgbImage};
    use tiff::encoder::{colortype, TiffEncoder};
    let dir = format!("{}/integration/fixtures", env!("CARGO_MANIFEST_DIR"));
    let img = RgbImage::from_fn(40, 30, |x, y| Rgb([(x * 6) as u8, (y * 8) as u8, 120]));
    img.save(format!("{dir}/amostra.jpg")).unwrap();
    img.save(format!("{dir}/amostra.png")).unwrap();

    let mut f = std::fs::File::create(format!("{dir}/duas.tif")).unwrap();
    let mut enc = TiffEncoder::new(&mut f).unwrap();
    let p1: Vec<u8> = (0..20 * 10).flat_map(|_| [200u8, 0, 0]).collect();
    enc.write_image::<colortype::RGB8>(20, 10, &p1).unwrap();
    let p2: Vec<u8> = (0..30 * 15).flat_map(|_| [0u8, 200, 0]).collect();
    enc.write_image::<colortype::RGB8>(30, 15, &p2).unwrap();
    drop(enc);

    std::fs::write(format!("{dir}/tres.pdf"), pdf(&[(200, 100), (100, 200), (300, 300)])).unwrap();
    std::fs::write(format!("{dir}/grande.pdf"), pdf(&[(1500, 3000)])).unwrap();


    // 16 bits
    let mut f = std::fs::File::create(format!("{dir}/cinza16.tif")).unwrap();
    let mut enc = TiffEncoder::new(&mut f).unwrap();
    enc.write_image::<colortype::Gray16>(4, 2, &[0xFF00, 0x8000, 0, 0xFFFF, 0x0100, 0x0200, 0x7F80, 0x4000])
        .unwrap();
    drop(enc);
    // 1 bit, largura 13
    let bits = [0xAA, 0xA8, 0xCC, 0xC8];
    std::fs::write(format!("{dir}/bilevel13.tif"), tiff_raw(13, 2, 1, 1, 1, 1, &bits, None)).unwrap();
    std::fs::write(format!("{dir}/bilevel13_whiteiszero.tif"), tiff_raw(13, 2, 1, 0, 1, 1, &bits, None)).unwrap();
    // PackBits: cada linha de 2 bytes como corrida literal (n=1, 2 bytes)
    let pb = [1, 0xAA, 0xA8, 1, 0xCC, 0xC8];
    std::fs::write(format!("{dir}/bilevel13_packbits.tif"), tiff_raw(13, 2, 1, 1, 32773, 1, &pb, None)).unwrap();
    let lzw = weezl::encode::Encoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8).encode(&bits).unwrap();
    std::fs::write(format!("{dir}/bilevel13_lzw.tif"), tiff_raw(13, 2, 1, 1, 5, 1, &lzw, None)).unwrap();
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    std::io::Write::write_all(&mut z, &bits).unwrap();
    std::fs::write(format!("{dir}/bilevel13_deflate.tif"), tiff_raw(13, 2, 1, 1, 8, 1, &z.finish().unwrap(), None)).unwrap();
    // altura declarada 5, dados so de 2 linhas
    std::fs::write(format!("{dir}/bilevel_curto.tif"), tiff_raw(13, 5, 1, 1, 1, 1, &bits, None)).unwrap();
    std::fs::write(format!("{dir}/ccitt_g4.tif"), tiff_raw(13, 2, 1, 0, 4, 1, &bits, None)).unwrap();
    let map: Vec<u16> = (0..768u32).map(|i| (i * 85) as u16).collect();
    std::fs::write(format!("{dir}/paleta.tif"), tiff_raw(4, 2, 8, 3, 1, 1, &[0, 1, 2, 3, 4, 5, 6, 7], Some(&map))).unwrap();

    // --- endurecimento do 1 bit ---
    let ex = |tags: &[(u16, u16, u32, u32)]| tags.to_vec();
    // corridas replicadas (w=24): linha 0 = FF FF FF (n=-2), linha 1 = 0F 0F AA (n=-1 e literal)
    std::fs::write(
        format!("{dir}/bilevel24_replicado.tif"),
        tiff_gen(24, 2, 1, 32773, 2, &[vec![0xFE, 0xFF, 0xFF, 0x0F, 0x00, 0xAA]], &ex(&[])),
    )
    .unwrap();
    // 13x5, RowsPerStrip=2 (strips de 2, 2 e 1 linhas), linhas distintas
    let l: [[u8; 2]; 5] = [[0xAA, 0xA8], [0xCC, 0xC8], [0xF0, 0xF0], [0x0F, 0x08], [0x80, 0x08]];
    let strips: Vec<Vec<u8>> = vec![
        [l[0], l[1]].concat(),
        [l[2], l[3]].concat(),
        l[4].to_vec(),
    ];
    let pb = |s: &[u8]| -> Vec<u8> { let mut o = vec![(s.len() - 1) as u8]; o.extend_from_slice(s); o };
    let lz = |s: &[u8]| weezl::encode::Encoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8).encode(s).unwrap();
    let df = |s: &[u8]| {
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        std::io::Write::write_all(&mut z, s).unwrap();
        z.finish().unwrap()
    };
    let maps = |f: &dyn Fn(&[u8]) -> Vec<u8>| -> Vec<Vec<u8>> { strips.iter().map(|s| f(s)).collect() };
    std::fs::write(format!("{dir}/multi5_none.tif"), tiff_gen(13, 5, 1, 1, 2, &strips, &ex(&[]))).unwrap();
    std::fs::write(format!("{dir}/multi5_packbits.tif"), tiff_gen(13, 5, 1, 32773, 2, &maps(&pb), &ex(&[]))).unwrap();
    std::fs::write(format!("{dir}/multi5_lzw.tif"), tiff_gen(13, 5, 1, 5, 2, &maps(&lz), &ex(&[]))).unwrap();
    std::fs::write(format!("{dir}/multi5_deflate.tif"), tiff_gen(13, 5, 1, 8, 2, &maps(&df), &ex(&[]))).unwrap();
    // declara 100000x100000
    std::fs::write(format!("{dir}/enorme.tif"), tiff_gen(100_000, 100_000, 1, 1, 100_000, &[vec![0xFF; 4]], &ex(&[]))).unwrap();
    // expansao alem do declarado (13x2 = 4 bytes esperados)
    let zeros = vec![0u8; 5000];
    std::fs::write(format!("{dir}/expande_deflate.tif"), tiff_gen(13, 2, 1, 8, 2, &[df(&zeros)], &ex(&[]))).unwrap();
    std::fs::write(format!("{dir}/expande_lzw.tif"), tiff_gen(13, 2, 1, 5, 2, &[lz(&zeros)], &ex(&[]))).unwrap();
    let mut rep = Vec::new();
    for _ in 0..40 {
        rep.extend_from_slice(&[0x81, 0x00]); // 128 bytes cada
    }
    std::fs::write(format!("{dir}/expande_packbits.tif"), tiff_gen(13, 2, 1, 32773, 2, &[rep], &ex(&[]))).unwrap();
    std::fs::write(format!("{dir}/fillorder2.tif"), tiff_gen(13, 2, 1, 1, 2, &[bits.to_vec()], &ex(&[(266, 3, 1, 2)]))).unwrap();
    std::fs::write(format!("{dir}/predictor2.tif"), tiff_gen(13, 2, 1, 1, 2, &[bits.to_vec()], &ex(&[(317, 3, 1, 2)]))).unwrap();
    std::fs::write(format!("{dir}/tiles.tif"), tiff_gen(13, 2, 1, 1, 2, &[bits.to_vec()], &ex(&[(322, 3, 1, 16)]))).unwrap();
    // StripByteCounts apontando alem do arquivo
    let mut outside = tiff_gen(13, 2, 1, 1, 2, &[bits.to_vec()], &ex(&[]));
    let n = outside.len();
    for i in 0..n - 12 {
        // acha a entrada 279 (StripByteCounts, SHORT/LONG) e troca o valor por 100000
        if outside[i..i + 2] == 279u16.to_le_bytes() && outside[i + 2..i + 4] == 4u16.to_le_bytes() {
            outside[i + 8..i + 12].copy_from_slice(&100_000u32.to_le_bytes());
        }
    }
    std::fs::write(format!("{dir}/strip_fora.tif"), outside).unwrap();

    let mut x: u32 = 12345;
    let junk: Vec<u8> = (0..300)
        .map(|_| {
            x = x.wrapping_mul(1664525).wrapping_add(1013904223);
            (x >> 24) as u8
        })
        .collect();
    std::fs::write(format!("{dir}/corrompido.jpg"), junk).unwrap();
}

fn pdf(pages: &[(u32, u32)]) -> Vec<u8> {
    let n = pages.len();
    let mut objs: Vec<String> = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".into(),
        format!(
            "<< /Type /Pages /Count {n} /Kids [{}] >>",
            (0..n).map(|i| format!("{} 0 R", 3 + i)).collect::<Vec<_>>().join(" ")
        ),
    ];
    for (w, h) in pages {
        objs.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w} {h}] >>"));
    }
    let mut out = String::from("%PDF-1.4\n");
    let mut offs = vec![];
    for (i, o) in objs.iter().enumerate() {
        offs.push(out.len());
        out.push_str(&format!("{} 0 obj\n{}\nendobj\n", i + 1, o));
    }
    let xref = out.len();
    out.push_str(&format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1));
    for o in offs {
        out.push_str(&format!("{o:010} 00000 n \n"));
    }
    out.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objs.len() + 1
    ));
    out.into_bytes()
}

/// TIFF little-endian de uma strip, escrito a mao (o encoder nao faz 1 bit, paleta nem CCITT).
#[allow(clippy::too_many_arguments)]
fn tiff_raw(w: u32, h: u32, bits: u16, photo: u16, compr: u16, spp: u16, data: &[u8], map: Option<&[u16]>) -> Vec<u8> {
    let mut out = vec![b'I', b'I', 42, 0, 0, 0, 0, 0];
    let off_data = out.len() as u32;
    out.extend_from_slice(data);
    if out.len() % 2 == 1 {
        out.push(0);
    }
    let off_ifd = out.len() as u32;
    let n_tags: u16 = if map.is_some() { 10 } else { 9 };
    let off_map = off_ifd + 2 + 12 * n_tags as u32 + 4;
    let mut tags: Vec<(u16, u16, u32, u32)> = vec![
        (256, 4, 1, w),
        (257, 4, 1, h),
        (258, 3, 1, bits as u32),
        (259, 3, 1, compr as u32),
        (262, 3, 1, photo as u32),
        (273, 4, 1, off_data),
        (277, 3, 1, spp as u32),
        (278, 4, 1, h),
        (279, 4, 1, data.len() as u32),
    ];
    if let Some(m) = map {
        tags.push((320, 3, m.len() as u32, off_map));
    }
    out.extend_from_slice(&(tags.len() as u16).to_le_bytes());
    for (t, ty, c, v) in tags {
        out.extend_from_slice(&t.to_le_bytes());
        out.extend_from_slice(&ty.to_le_bytes());
        out.extend_from_slice(&c.to_le_bytes());
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&0u32.to_le_bytes());
    if let Some(m) = map {
        for v in m {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out[4..8].copy_from_slice(&off_ifd.to_le_bytes());
    out
}

/// TIFF 1 amostra, LE, com varias strips (offsets/contagens em arrays LONG).
/// `extras`: tags adicionais (tag, tipo, contagem, valor inline).
fn tiff_gen(w: u32, h: u32, bits: u16, compr: u16, rps: u32, strips: &[Vec<u8>], extras: &[(u16, u16, u32, u32)]) -> Vec<u8> {
    let mut out = vec![b'I', b'I', 42, 0, 0, 0, 0, 0];
    let mut offs = vec![];
    for s in strips {
        offs.push(out.len() as u32);
        out.extend_from_slice(s);
        if out.len() % 2 == 1 {
            out.push(0);
        }
    }
    let n = strips.len() as u32;
    let (v_off, v_cnt) = if n == 1 {
        (offs[0], strips[0].len() as u32)
    } else {
        let a = out.len() as u32;
        for o in &offs {
            out.extend_from_slice(&o.to_le_bytes());
        }
        let b = out.len() as u32;
        for s in strips {
            out.extend_from_slice(&(s.len() as u32).to_le_bytes());
        }
        (a, b)
    };
    let mut tags: Vec<(u16, u16, u32, u32)> = vec![
        (256, 4, 1, w),
        (257, 4, 1, h),
        (258, 3, 1, bits as u32),
        (259, 3, 1, compr as u32),
        (262, 3, 1, 1),
        (273, 4, n, v_off),
        (277, 3, 1, 1),
        (278, 4, 1, rps),
        (279, 4, n, v_cnt),
    ];
    tags.extend_from_slice(extras);
    tags.sort_by_key(|t| t.0);
    let off_ifd = out.len() as u32;
    out.extend_from_slice(&(tags.len() as u16).to_le_bytes());
    for (t, ty, c, v) in tags {
        out.extend_from_slice(&t.to_le_bytes());
        out.extend_from_slice(&ty.to_le_bytes());
        out.extend_from_slice(&c.to_le_bytes());
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&0u32.to_le_bytes());
    out[4..8].copy_from_slice(&off_ifd.to_le_bytes());
    out
}
