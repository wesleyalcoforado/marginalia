//! Rotacao de imagem em passos de 90 graus (sentido horario), so como
//! visualizacao: grifos e enquadramentos continuam nas coordenadas da imagem
//! decodificada ("imagem"); a pagina girada que aparece na tela e o espaco de
//! "exibicao". Tudo aqui e puro, em coordenadas relativas 0..1.

use image::RgbaImage;
use serde::{Deserialize, Serialize};
use texts::t;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub enum Rotation {
    #[default]
    R0,
    R90,
    R180,
    R270,
}

impl From<Rotation> for u16 {
    fn from(r: Rotation) -> u16 {
        r.degrees()
    }
}

impl TryFrom<u16> for Rotation {
    type Error = String;
    fn try_from(g: u16) -> Result<Self, String> {
        match g {
            0 => Ok(Rotation::R0),
            90 => Ok(Rotation::R90),
            180 => Ok(Rotation::R180),
            270 => Ok(Rotation::R270),
            other => Err(t!("error.invalid_rotation", other)),
        }
    }
}

impl Rotation {
    pub fn degrees(self) -> u16 {
        match self {
            Rotation::R0 => 0,
            Rotation::R90 => 90,
            Rotation::R180 => 180,
            Rotation::R270 => 270,
        }
    }

    pub fn clockwise(self) -> Rotation {
        match self {
            Rotation::R0 => Rotation::R90,
            Rotation::R90 => Rotation::R180,
            Rotation::R180 => Rotation::R270,
            Rotation::R270 => Rotation::R0,
        }
    }

    pub fn anti_clockwise(self) -> Rotation {
        match self {
            Rotation::R0 => Rotation::R270,
            Rotation::R90 => Rotation::R0,
            Rotation::R180 => Rotation::R90,
            Rotation::R270 => Rotation::R180,
        }
    }

    pub fn sides_swap(self) -> bool {
        matches!(self, Rotation::R90 | Rotation::R270)
    }

    /// Tamanho da pagina na exibicao: os lados trocam em 90 e 270.
    pub fn display_sz(self, (w, h): (u32, u32)) -> (u32, u32) {
        if self.sides_swap() {
            (h, w)
        } else {
            (w, h)
        }
    }

    /// Ponto da imagem -> ponto da exibicao.
    pub fn display_to(self, (x, y): (f32, f32)) -> (f32, f32) {
        match self {
            Rotation::R0 => (x, y),
            Rotation::R90 => (1.0 - y, x),
            Rotation::R180 => (1.0 - x, 1.0 - y),
            Rotation::R270 => (y, 1.0 - x),
        }
    }

    /// Ponto da exibicao -> ponto da imagem (inverso de `display_to`).
    pub fn from_display(self, (u, v): (f32, f32)) -> (f32, f32) {
        match self {
            Rotation::R0 => (u, v),
            Rotation::R90 => (v, 1.0 - u),
            Rotation::R180 => (1.0 - u, 1.0 - v),
            Rotation::R270 => (1.0 - v, u),
        }
    }

    /// Retangulo `[x0, y0, x1, y1]` da imagem -> da exibicao, com min/max
    /// normalizados (a rotacao troca qual canto e o de cima, a esquerda).
    pub fn rectangle_to_display(self, r: [f32; 4]) -> [f32; 4] {
        normalizes(self.display_to((r[0], r[1])), self.display_to((r[2], r[3])))
    }

    /// Inverso de `rectangle_to_display`.
    pub fn display_rectangle(self, r: [f32; 4]) -> [f32; 4] {
        normalizes(self.from_display((r[0], r[1])), self.from_display((r[2], r[3])))
    }

    /// Gira os pixels (sentido horario).
    pub fn rotate_rgba(self, img: &RgbaImage) -> RgbaImage {
        use image::imageops::{rotate180, rotate270, rotate90};
        match self {
            Rotation::R0 => img.clone(),
            Rotation::R90 => rotate90(img),
            Rotation::R180 => rotate180(img),
            Rotation::R270 => rotate270(img),
        }
    }
}

fn normalizes(a: (f32, f32), b: (f32, f32)) -> [f32; 4] {
    [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)]
}

#[cfg(test)]
#[path = "tests/rotation.rs"]
mod tests;
