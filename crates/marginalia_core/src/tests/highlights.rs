use super::*;
use crate::model::Framing;
use chrono::NaiveDate;

fn img() -> ImageRef {
    ImageRef { source: "Inv".into(), file: "p1.jpg".into(), page: 1 }
}
fn vault() -> (tempfile::TempDir, Vault) {
    let d = tempfile::tempdir().unwrap();
    let v = Vault::open_or_create(d.path()).unwrap();
    (d, v)
}
fn ret(id: &str) -> Highlight {
    Highlight::Rectangle { id: id.into(), x: 0.1, y: 0.2, w: 0.3, h: 0.4, color: None }
}

const ROTATIONS: [Rotation; 4] = [Rotation::R0, Rotation::R90, Rotation::R180, Rotation::R270];

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

#[test]
fn rectangle_goes_to_the_display_and_back() {
    for rot in ROTATIONS {
        let g = ret("a");
        let e = g.display_to(rot);
        e.validate().unwrap();
        let Highlight::Rectangle { x, y, w, h, .. } = e.from_display(rot) else { panic!() };
        assert!(near(x, 0.1) && near(y, 0.2) && near(w, 0.3) && near(h, 0.4), "{rot:?}");
    }
    // R0 nao mexe em nada, nem no ultimo bit.
    assert_eq!(ret("a").display_to(Rotation::R0), ret("a"));
}

#[test]
fn rectangle_that_touches_the_border_continues_valid_in_the_four_rotations() {
    // Um grifo desenhado na tela encosta nas bordas (`x + w == 1`): a conta de volta
    // nao pode passar de 1 por um bit, senao o vault recusa o grifo.
    let values = [0.0f32, 0.1, 0.3, 1.0 / 3.0, 0.7, 0.9];
    for rot in ROTATIONS {
        for &x in &values {
            for &y in &values {
                let g = Highlight::Rectangle { id: "g".into(), x, y, w: 1.0 - x, h: 1.0 - y, color: None };
                if x >= 1.0 || y >= 1.0 {
                    continue;
                }
                g.from_display(rot).validate().unwrap_or_else(|e| panic!("{rot:?} {x} {y}: {e}"));
                g.display_to(rot).validate().unwrap_or_else(|e| panic!("{rot:?} {x} {y}: {e}"));
            }
        }
    }
}

#[test]
fn arrow_takes_tip_and_tail() {
    let g = Highlight::Pointer { id: "s".into(), x: 0.2, y: 0.3, tail: Some((0.6, 0.9)), color: Some("#e53935".into()) };
    for rot in ROTATIONS {
        let e = g.display_to(rot);
        e.validate().unwrap();
        let Highlight::Pointer { x, y, tail, color, .. } = e.from_display(rot) else { panic!() };
        let c = tail.unwrap();
        assert!(near(x, 0.2) && near(y, 0.3) && near(c.0, 0.6) && near(c.1, 0.9), "{rot:?}");
        assert_eq!(color.as_deref(), Some("#e53935"));
    }
    // A ponta continua sendo a ponta: em 90 graus horarios (0.2, 0.3) vira (0.7, 0.2).
    let Highlight::Pointer { x, y, .. } = g.display_to(Rotation::R90) else { panic!() };
    assert!(near(x, 0.7) && near(y, 0.2));
}

/// Falha ao trocar o arquivo: o temporario nao fica para tras.
#[test]
fn failure_to_the_save_not_leaves_temporary() {
    let (d, v) = vault();
    let target = d.path().join("grifos/Inv.json");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("x"), "ocupado").unwrap();
    let mut f = HighlightsFile::default();
    f.add(&img(), ret("g1")).unwrap();
    assert!(f.save(&v, "Inv").is_err());
    let names: Vec<String> =
        std::fs::read_dir(d.path().join("grifos")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["Inv.json"], "sobrou temporario");
}

#[test]
fn round_trip_and_format() {
    let (d, v) = vault();
    let mut f = HighlightsFile::default();
    f.add(&img(), ret("g1")).unwrap();
    f.add(&img(), Highlight::Pointer { id: "g2".into(), x: 0.5, y: 0.6, tail: None, color: None }).unwrap();
    f.save(&v, "Inv").unwrap();
    let raw = fs::read_to_string(d.path().join("grifos/Inv.json")).unwrap();
    let j: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(j["version"], 1);
    assert_eq!(j["images"]["p1.jpg#1"][0]["kind"], "rectangle");
    assert_eq!(j["images"]["p1.jpg#1"][1]["kind"], "pointer");
    assert_eq!(HighlightsFile::load(&v, "Inv").unwrap(), f);
}

#[test]
fn absent_and_empty_and_image_no_highlights() {
    let (_d, v) = vault();
    let f = HighlightsFile::load(&v, "Nada").unwrap();
    assert!(f.for_image(&img()).is_empty());
}

#[test]
fn outside_of_0_1_rejected() {
    let mut f = HighlightsFile::default();
    let cases = [
        Highlight::Pointer { id: "a".into(), x: 1.1, y: 0.0, tail: None, color: None },
        Highlight::Pointer { id: "a".into(), x: -0.1, y: 0.0, tail: None, color: None },
        Highlight::Pointer { id: "a".into(), x: f32::NAN, y: 0.0, tail: None, color: None },
        Highlight::Rectangle { id: "a".into(), x: 0.8, y: 0.0, w: 0.3, h: 0.1, color: None },
        Highlight::Rectangle { id: "a".into(), x: 0.0, y: 0.0, w: 0.0, h: 0.1, color: None },
    ];
    for g in cases {
        assert!(matches!(f.add(&img(), g), Err(Error::OutsideHighlight(_))));
    }
    assert!(f.for_image(&img()).is_empty());
}

#[test]
fn load_rejects_file_invalid() {
    let (d, v) = vault();
    fs::write(
        d.path().join("grifos/Ruim.json"),
        r#"{"versao":1,"imagens":{"a.jpg#1":[{"id":"g","tipo":"ponteiro","x":2.0,"y":0.1}]}}"#,
    )
    .unwrap();
    assert!(matches!(HighlightsFile::load(&v, "Ruim"), Err(Error::OutsideHighlight(_))));
}

#[test]
fn source_not_escapes_of_highlights() {
    let (d, v) = vault();
    HighlightsFile::default().save(&v, "../x").unwrap();
    assert!(!d.path().join("x.json").exists());
    assert!(d.path().join("grifos").read_dir().unwrap().count() == 1);
}

#[test]
fn id_by_milliseconds() {
    let t = NaiveDate::from_ymd_opt(2026, 11, 12).unwrap().and_hms_opt(10, 0, 0).unwrap();
    assert_eq!(new_highlight_id(t), format!("g{}", t.and_utc().timestamp_millis()));
    assert!(new_highlight_id(t).starts_with("g17"));
}

#[test]
fn note_survives_the_highlight_removed() {
    let (_d, v) = vault();
    let now = NaiveDate::from_ymd_opt(2026, 11, 12).unwrap().and_hms_opt(10, 0, 0).unwrap();
    let mut f = HighlightsFile::default();
    f.add(&img(), ret("g1")).unwrap();
    f.save(&v, "Inv").unwrap();
    let fr = Framing { zoom: 1.0, cx: 0.5, cy: 0.5 };
    v.create_note("achei", img(), fr, Some("g1".into()), now).unwrap();

    let mut f = HighlightsFile::load(&v, "Inv").unwrap();
    assert!(f.find(&img(), "g1").is_some());
    f.remove(&img(), "g1");
    f.save(&v, "Inv").unwrap();
    let f = HighlightsFile::load(&v, "Inv").unwrap();
    assert!(f.find(&img(), "g1").is_none());

    let notes = v.list_notes().unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].0.highlight.as_deref(), Some("g1"));
}

fn point(id: &str, tail: Option<(f32, f32)>) -> Highlight {
    Highlight::Pointer { id: id.into(), x: 0.5, y: 0.5, tail, color: None }
}

#[test]
fn json_old_loads_with_color_default() {
    let (d, v) = vault();
    fs::write(
        d.path().join("grifos/Velho.json"),
        r#"{"versao":1,"imagens":{"a.jpg#1":[{"id":"r","tipo":"retangulo","x":0.1,"y":0.1,"w":0.2,"h":0.2},{"id":"p","tipo":"ponteiro","x":0.5,"y":0.5}]}}"#,
    )
    .unwrap();
    let f = HighlightsFile::load(&v, "Velho").unwrap();
    let i = ImageRef { source: "Velho".into(), file: "a.jpg".into(), page: 1 };
    assert_eq!(f.find(&i, "r").unwrap().color(), DEFAULT_COLOR);
    assert_eq!(f.find(&i, "p").unwrap().color(), "#ffc400");
    assert!(matches!(f.find(&i, "p"), Some(Highlight::Pointer { tail: None, .. })));
}

#[test]
fn no_fields_new_not_serializes_color_nor_tail() {
    let mut f = HighlightsFile::default();
    f.add(&img(), ret("g1")).unwrap();
    f.add(&img(), point("g2", None)).unwrap();
    let s = serde_json::to_string(&f).unwrap();
    assert!(!s.contains("cor") && !s.contains("cauda"), "{s}");
}

#[test]
fn round_trip_with_color_and_tail() {
    let (_d, v) = vault();
    let mut f = HighlightsFile::default();
    f.add(&img(), ret("g1").with_color("#2e7d32")).unwrap();
    f.add(&img(), point("g2", Some((0.1, 0.9))).with_color(PALETTE[1])).unwrap();
    f.save(&v, "Inv").unwrap();
    let c = HighlightsFile::load(&v, "Inv").unwrap();
    assert_eq!(c, f);
    assert_eq!(c.find(&img(), "g1").unwrap().color(), "#2e7d32");
    assert!(matches!(c.find(&img(), "g2"), Some(Highlight::Pointer { tail: Some((a, b)), .. }) if *a == 0.1 && *b == 0.9));
}

#[test]
fn tail_invalid_rejected() {
    let mut f = HighlightsFile::default();
    for c in [(1.1, 0.5), (0.5, -0.1), (f32::NAN, 0.5), (0.5, 0.5)] {
        assert!(matches!(f.add(&img(), point("a", Some(c))), Err(Error::OutsideHighlight(_))), "{c:?}");
    }
    assert!(f.for_image(&img()).is_empty());
    f.add(&img(), point("b", Some((0.0, 1.0)))).unwrap();
}

#[test]
fn recolor_changes_so_the_right_and_persists() {
    let (_d, v) = vault();
    let mut f = HighlightsFile::default();
    f.add(&img(), ret("g1")).unwrap();
    f.add(&img(), point("g2", None)).unwrap();
    assert!(f.recolor(&img(), "g2", PALETTE[2]));
    assert!(!f.recolor(&img(), "nada", PALETTE[2]));
    f.save(&v, "Inv").unwrap();
    let c = HighlightsFile::load(&v, "Inv").unwrap();
    assert_eq!(c.find(&img(), "g1").unwrap().color(), DEFAULT_COLOR);
    assert_eq!(c.find(&img(), "g2").unwrap().color(), PALETTE[2]);
}

#[test]
fn merge_in_keeps_color_and_tail() {
    let (d, v) = vault();
    let mut a = HighlightsFile::default();
    a.add(&img(), ret("g1").with_color(PALETTE[3])).unwrap();
    a.add(&img(), point("g2", Some((0.1, 0.9))).with_color(PALETTE[1])).unwrap();
    a.save(&v, "Antiga").unwrap();
    let mut b = HighlightsFile::default();
    b.add(&img(), ret("g3")).unwrap();
    b.save(&v, "Nova").unwrap();
    merge_at(&d.path().join("grifos/Antiga.json"), &d.path().join("grifos/Nova.json")).unwrap();
    let m = HighlightsFile::load(&v, "Nova").unwrap();
    assert_eq!(m.find(&img(), "g1").unwrap().color(), PALETTE[3]);
    assert_eq!(m.find(&img(), "g2").unwrap().color(), PALETTE[1]);
    assert!(matches!(m.find(&img(), "g2"), Some(Highlight::Pointer { tail: Some((a, b)), .. }) if *a == 0.1 && *b == 0.9));
    assert!(m.find(&img(), "g3").is_some());
    assert!(!d.path().join("grifos/Antiga.json").exists());
}

#[test]
fn recolor_to_the_default_keeps_none() {
    let (_d, v) = vault();
    let mut f = HighlightsFile::default();
    f.add(&img(), ret("g1").with_color(PALETTE[1])).unwrap();
    assert!(f.recolor(&img(), "g1", DEFAULT_COLOR));
    assert_eq!(f.find(&img(), "g1"), Some(&ret("g1")));
    f.save(&v, "Inv").unwrap();
    let s = std::fs::read_to_string(_d.path().join("grifos/Inv.json")).unwrap();
    assert!(!s.contains("cor"), "{s}");
}

#[test]
fn with_id_swap_so_the_id() {
    let g = point("a", Some((0.1, 0.2))).with_color(PALETTE[2]).with_id("b".into());
    assert_eq!(g.id(), "b");
    assert_eq!(g.color(), PALETTE[2]);
    assert!(matches!(g, Highlight::Pointer { tail: Some(_), .. }));
}

#[test]
fn palette_starts_in_the_yellow() {
    assert_eq!(PALETTE[0], "#ffc400");
    assert_eq!(PALETTE[0], DEFAULT_COLOR);
}
