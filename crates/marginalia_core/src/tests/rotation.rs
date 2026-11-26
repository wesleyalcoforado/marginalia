use super::*;
use image::Rgba;

const ALL: [Rotation; 4] = [Rotation::R0, Rotation::R90, Rotation::R180, Rotation::R270];

fn near(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6
}

#[test]
fn forward_and_back_of_points() {
    for r in ALL {
        for p in [(0.0, 0.0), (1.0, 0.0), (0.25, 0.75), (0.5, 0.5), (1.0, 1.0)] {
            assert!(near(r.from_display(r.display_to(p)), p), "{r:?} {p:?}");
            assert!(near(r.display_to(r.from_display(p)), p), "{r:?} {p:?}");
        }
    }
}

#[test]
fn four_rotations_clockwise_return_and_the_anti_clockwise_undoes() {
    let mut r = Rotation::R0;
    for _ in 0..4 {
        r = r.clockwise();
    }
    assert_eq!(r, Rotation::R0);
    for r in ALL {
        assert_eq!(r.clockwise().anti_clockwise(), r);
        assert_eq!(r.anti_clockwise().clockwise(), r);
    }
}

#[test]
fn corners_in_the_direction_clockwise() {
    // Em 90 graus horarios o canto de cima, a esquerda, vai para o de cima, a direita.
    assert!(near(Rotation::R90.display_to((0.0, 0.0)), (1.0, 0.0)));
    assert!(near(Rotation::R90.display_to((1.0, 0.0)), (1.0, 1.0)));
    assert!(near(Rotation::R180.display_to((0.0, 0.0)), (1.0, 1.0)));
    assert!(near(Rotation::R270.display_to((0.0, 0.0)), (0.0, 1.0)));
}

#[test]
fn rotations_successive_compose() {
    let p = (0.2, 0.7);
    assert!(near(Rotation::R90.display_to(Rotation::R90.display_to(p)), Rotation::R180.display_to(p)));
    assert!(near(
        Rotation::R180.display_to(Rotation::R90.display_to(p)),
        Rotation::R270.display_to(p)
    ));
}

#[test]
fn rectangle_leaves_normalized_and_back() {
    let r = [0.1, 0.2, 0.4, 0.9];
    for rot in ALL {
        let e = rot.rectangle_to_display(r);
        assert!(e[0] <= e[2] && e[1] <= e[3], "{rot:?} {e:?}");
        let back = rot.display_rectangle(e);
        for i in 0..4 {
            assert!((back[i] - r[i]).abs() < 1e-6, "{rot:?} {back:?}");
        }
    }
    // 90 graus: a largura vira a altura.
    let e = Rotation::R90.rectangle_to_display(r);
    assert!(((e[2] - e[0]) - 0.7).abs() < 1e-6 && ((e[3] - e[1]) - 0.3).abs() < 1e-6);
}

#[test]
fn size_swap_os_sides_so_in_90_and_270() {
    assert_eq!(Rotation::R0.display_sz((40, 30)), (40, 30));
    assert_eq!(Rotation::R90.display_sz((40, 30)), (30, 40));
    assert_eq!(Rotation::R180.display_sz((40, 30)), (40, 30));
    assert_eq!(Rotation::R270.display_sz((40, 30)), (30, 40));
}

#[test]
fn pixels_fall_where_the_point_sends() {
    // 4x2 com um pixel marcado no canto de cima, a esquerda, e outro no de baixo, a direita.
    let mut img = RgbaImage::new(4, 2);
    img.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
    img.put_pixel(3, 1, Rgba([0, 255, 0, 255]));
    for r in ALL {
        let g = r.rotate_rgba(&img);
        let (w, h) = r.display_sz((4, 2));
        assert_eq!(g.dimensions(), (w, h), "{r:?}");
        for (mark, p) in [([255, 0, 0, 255], (0.125, 0.25)), ([0, 255, 0, 255], (0.875, 0.75))] {
            // Centro do pixel marcado, em relativo, passado pela rotacao.
            let (u, v) = r.display_to(p);
            let (px, py) = ((u * w as f32) as u32, (v * h as f32) as u32);
            assert_eq!(g.get_pixel(px, py).0, mark, "{r:?} {p:?}");
        }
    }
}

#[test]
fn serde_in_degrees() {
    assert_eq!(serde_json::to_string(&Rotation::R270).unwrap(), "270");
    assert_eq!(serde_json::from_str::<Rotation>("90").unwrap(), Rotation::R90);
    assert!(serde_json::from_str::<Rotation>("45").is_err());
}
