use super::*;
use image::Rgba;

fn sample() -> RgbaImage {
    RgbaImage::from_fn(5, 4, |x, y| {
        Rgba([(x * 50) as u8, (y * 60) as u8, (x * y * 10) as u8, 77])
    })
}

fn all_filters() -> Vec<Adjustments> {
    let d = Adjustments::default();
    vec![
        Adjustments { brightness: 0.3, ..d.clone() },
        Adjustments { contrast: 2.0, ..d.clone() },
        Adjustments { invert: true, ..d.clone() },
        Adjustments { grayscale: true, ..d.clone() },
        Adjustments { sharpen: 1.0, ..d.clone() },
        Adjustments { highpass: 1.0, ..d.clone() },
        Adjustments { threshold: Some(100), ..d.clone() },
    ]
}

#[test]
fn neutral_returns_identical_pixels() {
    let img = sample();
    let a = Adjustments::default();
    assert!(a.is_neutral());
    assert_eq!(apply(&img, &a), img);
}

#[test]
fn non_neutral_is_detected() {
    for a in all_filters() {
        assert!(!a.is_neutral(), "{a:?}");
    }
}

#[test]
fn invert_flips_channels() {
    let img = RgbaImage::from_pixel(1, 1, Rgba([10, 20, 30, 255]));
    let out = apply(&img, &Adjustments { invert: true, ..Default::default() });
    assert_eq!(out.get_pixel(0, 0), &Rgba([245, 235, 225, 255]));
}

#[test]
fn grayscale_makes_channels_equal() {
    let out = apply(&sample(), &Adjustments { grayscale: true, ..Default::default() });
    for p in out.pixels() {
        assert!(p[0] == p[1] && p[1] == p[2]);
    }
}

#[test]
fn brightness_brightens_without_overflow() {
    let img = RgbaImage::from_pixel(1, 1, Rgba([100, 250, 0, 255]));
    let out = apply(&img, &Adjustments { brightness: 0.2, ..Default::default() });
    let p = out.get_pixel(0, 0);
    assert_eq!(p[0], 151);
    assert_eq!(p[1], 255);
    assert_eq!(p[2], 51);
}

#[test]
fn threshold_yields_only_black_or_white() {
    let out = apply(&sample(), &Adjustments { threshold: Some(128), ..Default::default() });
    for p in out.pixels() {
        for c in &p.0[..3] {
            assert!(*c == 0 || *c == 255);
        }
    }
    let img = RgbaImage::from_pixel(1, 1, Rgba([128, 128, 128, 255]));
    let out = apply(&img, &Adjustments { threshold: Some(128), ..Default::default() });
    assert_eq!(out.get_pixel(0, 0), &Rgba([255, 255, 255, 255]));
}

#[test]
fn apply_does_not_modify_input() {
    let img = sample();
    let copy = img.clone();
    for a in all_filters() {
        let _ = apply(&img, &a);
        assert_eq!(img, copy);
    }
}

#[test]
fn alpha_is_preserved_under_every_filter() {
    let img = sample();
    for a in all_filters() {
        let out = apply(&img, &a);
        assert!(out.pixels().all(|p| p[3] == 77), "{a:?}");
    }
}

#[test]
fn tiny_and_empty_images_are_safe() {
    let one = RgbaImage::from_pixel(1, 1, Rgba([1, 2, 3, 4]));
    let empty = RgbaImage::new(0, 0);
    for a in all_filters() {
        assert_eq!(apply(&one, &a).dimensions(), (1, 1));
        assert_eq!(apply(&empty, &a).dimensions(), (0, 0));
    }
}

#[test]
fn nan_behaves_like_neutral() {
    let img = sample();
    let a = Adjustments {
        brightness: f32::NAN,
        contrast: f32::NAN,
        sharpen: f32::NAN,
        highpass: f32::NAN,
        ..Default::default()
    };
    assert_eq!(apply(&img, &a), img);
}

#[test]
fn out_of_range_is_clamped_not_panicking() {
    let a = Adjustments { brightness: 9.0, contrast: -3.0, sharpen: 50.0, highpass: -2.0, ..Default::default() };
    let out = apply(&sample(), &a);
    assert_eq!(out.dimensions(), (5, 4));
}

/// Copia fiel da implementacao de `apply` anterior a versao cancelavel/paralela.
/// Serve de referencia: o resultado novo tem de ser identico byte a byte.
fn apply_reference(img: &RgbaImage, a: &Adjustments) -> RgbaImage {
    apply_reference_sigma(img, a, BLUR_SIGMA)
}

/// Como [`apply_reference`], com o sigma do desfoque explicito (imageproc inteiro).
fn apply_reference_sigma(img: &RgbaImage, a: &Adjustments, sigma: f32) -> RgbaImage {
    fn map_rgb_ref(img: &mut RgbaImage, f: impl Fn(f32) -> f32) {
        for p in img.pixels_mut() {
            for c in &mut p.0[..3] {
                *c = to_u8(f(*c as f32));
            }
        }
    }
    fn blend_ref(img: &mut RgbaImage, sigma: f32, f: impl Fn(f32, f32) -> f32) {
        let blurred = imageproc::filter::gaussian_blur_f32(img, sigma);
        for (p, b) in img.pixels_mut().zip(blurred.pixels()) {
            for i in 0..3 {
                p.0[i] = to_u8(f(p.0[i] as f32, b.0[i] as f32));
            }
        }
    }
    let mut out = img.clone();
    if out.width() == 0 || out.height() == 0 || a.is_neutral() {
        return out;
    }
    let c = a.clean();
    if a.grayscale {
        for p in out.pixels_mut() {
            let l = to_u8(luma(p[0], p[1], p[2]));
            p.0[..3].fill(l);
        }
    }
    if c.brightness != 0.0 || c.contrast != 1.0 {
        let shift = c.brightness * 255.0;
        map_rgb_ref(&mut out, |v| (v - 128.0) * c.contrast + 128.0 + shift);
    }
    if c.sharpen != 0.0 {
        blend_ref(&mut out, sigma, |v, b| v + c.sharpen * (v - b));
    }
    if c.highpass != 0.0 {
        blend_ref(&mut out, sigma, |v, b| {
            let high = (v - b + 128.0).clamp(0.0, 255.0);
            v * (1.0 - c.highpass) + high * c.highpass
        });
    }
    if a.invert {
        map_rgb_ref(&mut out, |v| 255.0 - v);
    }
    if let Some(t) = a.threshold {
        for p in out.pixels_mut() {
            let v = if luma(p[0], p[1], p[2]) >= t as f32 { 255 } else { 0 };
            p.0[..3].fill(v);
        }
    }
    out
}

/// Imagem pseudoaleatoria deterministica (LCG), com alfa variado.
fn noisy(w: u32, h: u32) -> RgbaImage {
    let mut s: u32 = 0x1234_5678 ^ (w * 31 + h);
    let mut next = move || {
        s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (s >> 24) as u8
    };
    RgbaImage::from_fn(w, h, |_, _| Rgba([next(), next(), next(), next()]))
}

fn sizes() -> Vec<(u32, u32)> {
    vec![(1, 1), (5, 4), (9, 9), (2, 300), (130, 7), (37, 150), (64, 129), (257, 203)]
}

fn cases() -> Vec<Adjustments> {
    let mut v = all_filters();
    v.push(Adjustments {
        brightness: -0.25,
        contrast: 1.7,
        invert: true,
        grayscale: true,
        sharpen: 0.37,
        highpass: 0.6,
        threshold: Some(90),
    });
    v.push(Adjustments {
        brightness: 0.1,
        contrast: 0.6,
        sharpen: 0.8,
        highpass: 0.25,
        ..Default::default()
    });
    v.push(Adjustments { sharpen: 0.5, highpass: 0.5, invert: true, ..Default::default() });
    v.push(Adjustments { brightness: 9.0, contrast: -3.0, sharpen: 50.0, highpass: -2.0, ..Default::default() });
    v.push(Adjustments { brightness: f32::NAN, contrast: f32::NAN, sharpen: 0.4, highpass: f32::NAN, ..Default::default() });
    v.push(Adjustments { contrast: f32::INFINITY, brightness: f32::NEG_INFINITY, ..Default::default() });
    v
}

#[test]
fn cancellable_matches_reference_pixel_for_pixel() {
    for (w, h) in sizes() {
        let img = noisy(w, h);
        for a in cases() {
            let expected = apply_reference(&img, &a);
            let got = apply_cancellable(&img, &a, &|| false).expect("sem parada");
            assert!(got == expected, "{w}x{h} {a:?}");
            let got_at = apply_cancellable_in(img.clone(), &a, &|| false).expect("sem parada");
            assert!(got_at == expected, "_em {w}x{h} {a:?}");
            assert!(apply(&img, &a) == expected, "apply {w}x{h} {a:?}");
        }
    }
}

#[test]
fn stop_at_first_check_returns_none() {
    let img = noisy(37, 150);
    for a in cases() {
        let calls = std::cell::Cell::new(0u32);
        let r = apply_cancellable(&img, &a, &|| {
            calls.set(calls.get() + 1);
            true
        });
        assert!(r.is_none(), "{a:?}");
        assert_eq!(calls.get(), 1, "{a:?}");
        assert!(apply_cancellable_in(img.clone(), &a, &|| true).is_none(), "{a:?}");
    }
}

#[test]
fn stop_midway_returns_none_without_panic() {
    let img = noisy(40, 300);
    for a in cases() {
        // Conta as checagens de uma execucao completa.
        let total = std::cell::Cell::new(0u32);
        let r = apply_cancellable(&img, &a, &|| {
            total.set(total.get() + 1);
            false
        });
        assert!(r.is_some());
        assert!(total.get() > 1, "deve checar mais de uma vez: {a:?}");
        // Parar em cada checagem possivel devolve None.
        for n in 1..=total.get() {
            let calls = std::cell::Cell::new(0u32);
            let r = apply_cancellable(&img, &a, &|| {
                calls.set(calls.get() + 1);
                calls.get() >= n
            });
            assert!(r.is_none(), "parada na checagem {n}: {a:?}");
            assert_eq!(calls.get(), n);
        }
    }
}

#[test]
fn blur_checks_stop_per_block_of_rows() {
    // Um blur isolado numa imagem alta precisa checar varias vezes
    // (por bloco de linhas), nao so antes e depois.
    let img = noisy(16, 1000);
    let a = Adjustments { sharpen: 1.0, ..Default::default() };
    let total = std::cell::Cell::new(0u32);
    let _ = apply_cancellable(&img, &a, &|| {
        total.set(total.get() + 1);
        false
    });
    assert!(total.get() >= 10, "checagens: {}", total.get());
}

#[test]
fn neutral_returns_image_without_work() {
    let img = noisy(37, 150);
    let nan = Adjustments {
        brightness: f32::NAN,
        contrast: f32::NAN,
        sharpen: f32::NAN,
        highpass: f32::NAN,
        ..Default::default()
    };
    for a in [Adjustments::default(), nan] {
        let calls = std::cell::Cell::new(0u32);
        let stop = || {
            calls.set(calls.get() + 1);
            true
        };
        assert_eq!(apply_cancellable(&img, &a, &stop), Some(img.clone()));
        assert_eq!(apply_cancellable_in(img.clone(), &a, &stop), Some(img.clone()));
        assert_eq!(calls.get(), 0, "ajuste neutro nao trabalha");
    }
}

#[test]
fn empty_image_is_safe_cancellable() {
    let empty = RgbaImage::new(0, 0);
    let wide = RgbaImage::new(10, 0);
    for a in cases() {
        assert_eq!(apply_cancellable(&empty, &a, &|| false).unwrap().dimensions(), (0, 0));
        assert_eq!(apply_cancellable_in(wide.clone(), &a, &|| false).unwrap().dimensions(), (10, 0));
    }
}

/// Medicao (nao roda por padrao): `cargo test --release -p nucleo -- --ignored --nocapture`.
/// Numa imagem sintetica 6000x4000 imprime, por combinacao de ajustes, o tempo do
/// `apply` antigo (`apply_reference`), da final (`apply_cancellable_in`, resolucao
/// cheia), da previa (copia com lado maior <= 2048, como o app faz) e a latencia da
/// interrupcao (de `stop` virar verdadeiro ate a funcao devolver `None`).
#[test]
#[ignore]
fn measure_times_of_the_filters() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    fn best<T>(n: usize, mut f: impl FnMut() -> T) -> (Duration, T) {
        let mut r = None;
        let mut t = Duration::MAX;
        for _ in 0..n {
            let t0 = Instant::now();
            let v = f();
            t = t.min(t0.elapsed());
            r = Some(v);
        }
        (t, r.unwrap())
    }
    let ms = |d: Duration| d.as_secs_f64() * 1e3;

    let (w, h) = (6000u32, 4000u32);
    let img = noisy(w, h);
    // Mesma reducao da base do app (carga.rs: LADO_BASE = 2048, Triangle).
    let (pw, ph) = (2048u32, (h as u64 * 2048 / w as u64) as u32);
    let (t_red, preview) = best(1, || image::imageops::resize(&img, pw, ph, image::imageops::FilterType::Triangle));
    println!(
        "medicao: {w}x{h}, {} threads rayon; previa {pw}x{ph} (reducao, uma vez por pagina: {:.0} ms)",
        rayon::current_num_threads(),
        ms(t_red)
    );

    let d = Adjustments::default();
    let combos = [
        ("contraste", Adjustments { contrast: 1.8, ..d.clone() }),
        ("nitidez", Adjustments { sharpen: 0.6, ..d.clone() }),
        ("passa-alta", Adjustments { highpass: 0.7, ..d.clone() }),
        ("nitidez+passa-alta", Adjustments { sharpen: 0.6, highpass: 0.7, ..d.clone() }),
        ("nitidez+passa-alta+contraste", Adjustments { contrast: 1.8, sharpen: 0.6, highpass: 0.7, ..d.clone() }),
    ];
    for (name, a) in combos {
        let (t_ref, expected) = best(2, || apply_reference(&img, &a));
        let (t_final, got) = best(3, || apply_cancellable_in(img.clone(), &a, &|| false).unwrap());
        assert!(got == expected, "final difere da referencia: {name}");
        let (t_clone, _) = best(3, || img.clone());
        let (t_preview, _) = best(5, || apply_cancellable_in(preview.clone(), &a, &|| false).unwrap());

        // Interrupcao: `stop` vira verdadeiro na metade (aprox.) da final.
        // Um filtro muito rapido pode terminar antes: conta so as que pararam.
        let (mut lat, mut stopped) = (Duration::ZERO, 0);
        for _ in 0..3 {
            let stop = Arc::new(AtomicBool::new(false));
            let started = Arc::new(AtomicBool::new(false));
            let (p, c, base, a2) = (Arc::clone(&stop), Arc::clone(&started), img.clone(), a.clone());
            let job = std::thread::spawn(move || {
                c.store(true, Ordering::SeqCst);
                apply_cancellable_in(base, &a2, &|| p.load(Ordering::Relaxed)).is_none()
            });
            while !started.load(Ordering::SeqCst) {
                std::hint::spin_loop();
            }
            std::thread::sleep(t_final / 2);
            let t0 = Instant::now();
            stop.store(true, Ordering::Relaxed);
            if job.join().unwrap() {
                lat = lat.max(t0.elapsed());
                stopped += 1;
            }
        }
        println!(
            "medicao: {name:<29} antigo {:>6.0} ms | final {:>6.0} ms (copia {:.0} ms inclusa) | previa {:>5.1} ms | interrupcao <= {:.1} ms ({stopped}/3 paradas)",
            ms(t_ref),
            ms(t_final),
            ms(t_clone),
            ms(t_preview),
            ms(lat)
        );
        assert!(stopped > 0 || t_final < Duration::from_millis(50), "nunca parou: {name}");
    }
}

#[test]
fn scale_one_and_identical_the_reference() {
    for (w, h) in sizes() {
        let img = noisy(w, h);
        for a in cases() {
            let expected = apply_reference(&img, &a);
            let got = apply_cancellable_in_scale(img.clone(), &a, 1.0, &|| false).expect("sem parada");
            assert!(got == expected, "escala 1 {w}x{h} {a:?}");
        }
    }
}

#[test]
fn scale_follows_the_blur_with_sigma_scaled() {
    // As faixas refiltram `radius` linhas extras: com sigma escalado o raio muda e a
    // sobreposicao tem de acompanhar (imagens altas cruzam varias faixas e blocos).
    for scale in [0.5f32, 0.3, 0.05] {
        for (w, h) in [(9, 9), (37, 150), (64, 129), (2, 300)] {
            let img = noisy(w, h);
            for a in cases() {
                let expected = apply_reference_sigma(&img, &a, BLUR_SIGMA * scale);
                let got = apply_cancellable_in_scale(img.clone(), &a, scale, &|| false).expect("sem parada");
                assert!(got == expected, "escala {scale} {w}x{h} {a:?}");
            }
        }
    }
}

#[test]
fn scale_sanitized_never_enlarges_and_invalid_is_worth_1() {
    assert_eq!(sanitized_scale(0.5), 0.5);
    assert_eq!(sanitized_scale(1.0), 1.0);
    for e in [1.7f32, f32::INFINITY, 0.0, -2.0, f32::NAN, f32::NEG_INFINITY] {
        assert_eq!(sanitized_scale(e), 1.0, "{e}");
    }
    // O filtro usa a mesma escala saneada: acima de 1 vale 1.
    let img = noisy(37, 150);
    for a in cases() {
        let um = apply_cancellable_in_scale(img.clone(), &a, 1.0, &|| false);
        assert!(apply_cancellable_in_scale(img.clone(), &a, 1.7, &|| false) == um, "{a:?}");
    }
    assert_eq!(marginalia_core_radius(1.7), 4);
}

#[test]
fn radius_of_the_marginalia_core_follows_the_scale_with_min_1() {
    assert_eq!(marginalia_core_radius(1.0), 4);
    assert_eq!(marginalia_core_radius(0.5), 2);
    assert_eq!(marginalia_core_radius(0.05), 1);
    assert_eq!(marginalia_core_radius(0.0), 4, "escala invalida vale 1");
    assert_eq!(marginalia_core_radius(f32::NAN), 4, "escala invalida vale 1");
    assert_eq!(marginalia_core_radius(-3.0), 4, "escala invalida vale 1");
}

#[test]
fn scales_extreme_not_enter_in_panic() {
    let img = noisy(37, 40);
    let a = Adjustments { sharpen: 0.7, highpass: 0.5, ..Default::default() };
    for scale in [0.05f32, 1e-6, 0.0, -1.0, f32::NAN, f32::INFINITY] {
        let got = apply_cancellable_in_scale(img.clone(), &a, scale, &|| false).expect("sem parada");
        assert_eq!(got.dimensions(), (37, 40), "escala {scale}");
        assert!(got.pixels().zip(img.pixels()).all(|(g, o)| g[3] == o[3]), "alfa {scale}");
    }
    for (w, h) in [(0, 0), (1, 1), (10, 0)] {
        let got = apply_cancellable_in_scale(RgbaImage::new(w, h), &a, 0.05, &|| false).unwrap();
        assert_eq!(got.dimensions(), (w, h));
    }
}

#[test]
fn scale_neutral_returns_no_work() {
    let img = noisy(37, 150);
    let calls = std::cell::Cell::new(0u32);
    let stop = || {
        calls.set(calls.get() + 1);
        true
    };
    assert_eq!(apply_cancellable_in_scale(img.clone(), &Adjustments::default(), 0.5, &stop), Some(img));
    assert_eq!(calls.get(), 0);
}

#[test]
fn scale_with_stopped_returns_none() {
    let img = noisy(40, 300);
    for a in cases() {
        assert!(apply_cancellable_in_scale(img.clone(), &a, 0.5, &|| true).is_none(), "{a:?}");
        let total = std::cell::Cell::new(0u32);
        let _ = apply_cancellable_in_scale(img.clone(), &a, 0.5, &|| {
            total.set(total.get() + 1);
            false
        });
        for n in 1..=total.get() {
            let calls = std::cell::Cell::new(0u32);
            let r = apply_cancellable_in_scale(img.clone(), &a, 0.5, &|| {
                calls.set(calls.get() + 1);
                calls.get() >= n
            });
            assert!(r.is_none(), "parada na checagem {n}: {a:?}");
        }
    }
}

#[test]
fn highpass_on_flat_image_stays_flat() {
    let img = RgbaImage::from_pixel(8, 8, Rgba([90, 120, 200, 255]));
    let out = apply(&img, &Adjustments { highpass: 1.0, ..Default::default() });
    let first = *out.get_pixel(0, 0);
    assert!(out.pixels().all(|p| *p == first));
}
