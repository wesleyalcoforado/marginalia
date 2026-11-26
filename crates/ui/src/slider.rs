//! Barras deslizantes do painel de ajustes, no modelo do painel Basic do
//! Camera Raw, sem gpui (regra R4). A view (`main_view`) so desenha e traduz
//! eventos; aqui ficam as contas:
//! - As escalas na tela, em inteiros: Brilho e Contraste de -100 a +100 (0 no
//!   meio, o neutro), Nitidez e Passa-alta de 0 a 100, Limiar de 0 a 255.
//! - O mapeamento tela <-> `Adjustments` ([`Bar::in_screen`], [`Bar::apply`]).
//!   O contraste continua multiplicador por dentro: tela 0 = 1,0; -100 = 0,0;
//!   +100 = 4,0 (linear em cada metade). Brilho e tela/100; Nitidez e
//!   Passa-alta tambem. Limiar desligado aparece em [`THRESHOLD_DEFAULT`] (marcador
//!   apagado); mexer na barra dele o liga.
//! - Posicao <-> valor ([`Bar::fraction`], [`Bar::point_value`]): a trilha
//!   vai do minimo (esquerda) ao maximo (direita); nos bipolares o 0 fica no
//!   centro.
//! - O texto da caixa numerica ([`Bar::text`], [`interpret_text`]): vazio,
//!   letras ou NaN nao valem (`None`: o valor anterior fica); um numero fora da
//!   faixa e saneado ao limite; decimal (com `.` ou `,`) arredonda; `+`, `-` e
//!   `−` na frente valem.
//! - ↑/↓ na caixa ([`key_step`]): 1 em 1; com Shift, 10 em 10.
//! - Duplo clique na trilha (o 1o clique ja levou o marcador ao ponteiro) ou
//!   no rotulo: o neutro ([`Bar::restore`]):
//!   0 nos bipolares e em Nitidez/Passa-alta; o Limiar desliga.

use marginalia_core::filters::Adjustments;
use texts::t;

use crate::app_state::Control;

/// Onde o marcador de um Limiar desligado fica (o valor que `t` liga).
pub const THRESHOLD_DEFAULT: i32 = 128;

/// Uma barra do painel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Bar {
    Brightness,
    Contrast,
    Sharpness,
    Highpass,
    Threshold,
}

/// As barras, na ordem do painel (e do indice das caixas numericas).
pub const ALL: [Bar; 5] = [Bar::Brightness, Bar::Contrast, Bar::Sharpness, Bar::Highpass, Bar::Threshold];

impl Bar {
    pub fn index(self) -> usize {
        match self {
            Bar::Brightness => 0,
            Bar::Contrast => 1,
            Bar::Sharpness => 2,
            Bar::Highpass => 3,
            Bar::Threshold => 4,
        }
    }

    pub fn label(self) -> String {
        match self {
            Bar::Brightness => t!("adjustment.bar.brightness"),
            Bar::Contrast => t!("adjustment.bar.contrast"),
            Bar::Sharpness => t!("adjustment.bar.sharpness"),
            Bar::Highpass => t!("adjustment.bar.highpass"),
            Bar::Threshold => t!("adjustment.bar.threshold"),
        }
    }

    /// O controle de `app_state::step` (teclas e menus) desta barra.
    pub fn control(self) -> Control {
        match self {
            Bar::Brightness => Control::Brightness,
            Bar::Contrast => Control::Contrast,
            Bar::Sharpness => Control::Sharpness,
            Bar::Highpass => Control::Highpass,
            Bar::Threshold => Control::Threshold,
        }
    }

    /// (minimo, maximo) na tela.
    pub fn strip(self) -> (i32, i32) {
        match self {
            Bar::Brightness | Bar::Contrast => (-100, 100),
            Bar::Sharpness | Bar::Highpass => (0, 100),
            Bar::Threshold => (0, 255),
        }
    }

    /// 0 no meio da trilha (o marcador comeca centrado).
    pub fn bipolar(self) -> bool {
        self.strip().0 < 0
    }

    /// O valor de `a` na tela (NaN e fora da faixa saneados).
    pub fn in_screen(self, a: &Adjustments) -> i32 {
        let finite = |x: f32, neutral: f32| if x.is_finite() { x } else { neutral };
        let v = match self {
            Bar::Brightness => (finite(a.brightness, 0.0) * 100.0).round() as i32,
            Bar::Contrast => {
                let c = finite(a.contrast, 1.0);
                let t = if c < 1.0 { (c - 1.0) * 100.0 } else { (c - 1.0) * 100.0 / 3.0 };
                t.round() as i32
            }
            Bar::Sharpness => (finite(a.sharpen, 0.0) * 100.0).round() as i32,
            Bar::Highpass => (finite(a.highpass, 0.0) * 100.0).round() as i32,
            Bar::Threshold => a.threshold.map_or(THRESHOLD_DEFAULT, i32::from),
        };
        self.sanitized(v)
    }

    fn sanitized(self, v: i32) -> i32 {
        let (min, max) = self.strip();
        v.clamp(min, max)
    }

    /// O neutro na tela (Limiar: onde o marcador desligado fica).
    fn neutral(self) -> i32 {
        match self {
            Bar::Threshold => THRESHOLD_DEFAULT,
            _ => 0,
        }
    }

    /// A barra vale agora (so o Limiar desliga: o marcador fica apagado).
    pub fn active(self, a: &Adjustments) -> bool {
        self != Bar::Threshold || a.threshold.is_some()
    }

    /// `a` com esta barra em `v` (na tela; saneado a faixa). No Limiar, liga.
    pub fn apply(self, a: &Adjustments, v: i32) -> Adjustments {
        let v = self.sanitized(v);
        let f = v as f32 / 100.0;
        let mut r = a.clone();
        match self {
            Bar::Brightness => r.brightness = f,
            Bar::Contrast => r.contrast = if v < 0 { 1.0 + f } else { 1.0 + 3.0 * f },
            Bar::Sharpness => r.sharpen = f,
            Bar::Highpass => r.highpass = f,
            Bar::Threshold => r.threshold = Some(v as u8),
        }
        r
    }

    /// `a` com esta barra no neutro (duplo clique): 0, contraste 1,0, Limiar desligado.
    pub fn restore(self, a: &Adjustments) -> Adjustments {
        let mut r = a.clone();
        match self {
            Bar::Brightness => r.brightness = 0.0,
            Bar::Contrast => r.contrast = 1.0,
            Bar::Sharpness => r.sharpen = 0.0,
            Bar::Highpass => r.highpass = 0.0,
            Bar::Threshold => r.threshold = None,
        }
        r
    }

    /// `a` com esta barra `delta` passos de tela adiante (↑/↓ na caixa).
    pub fn step(self, a: &Adjustments, delta: i32) -> Adjustments {
        self.apply(a, self.in_screen(a).saturating_add(delta))
    }

    /// Posicao do marcador na trilha, de 0 (esquerda) a 1 (direita).
    pub fn fraction(self, v: i32) -> f32 {
        let (min, max) = self.strip();
        (self.sanitized(v) - min) as f32 / (max - min) as f32
    }

    /// O valor no ponto `f` da trilha (0..1, saneado; NaN: o neutro).
    pub fn fraction_value(self, f: f32) -> i32 {
        if f.is_nan() {
            return self.neutral();
        }
        let (min, max) = self.strip();
        let v = min as f32 + f.clamp(0.0, 1.0) * (max - min) as f32;
        self.sanitized(v.round() as i32)
    }

    /// O valor sob o ponteiro em `x` (coordenada da janela) numa trilha que
    /// comeca em `left` e tem `width` px. Trilha sem largura: `None`.
    pub fn point_value(self, x: f32, left: f32, width: f32) -> Option<i32> {
        if !(width > 0.0) || !x.is_finite() || !left.is_finite() {
            return None;
        }
        Some(self.fraction_value((x - left) / width))
    }

    /// O texto da caixa numerica: `+12`, `0`, `-12` nos bipolares, `40` nos
    /// outros; Limiar desligado: vazio (a caixa mostra a dica).
    pub fn text(self, a: &Adjustments) -> String {
        if !self.active(a) {
            return String::new();
        }
        let v = self.in_screen(a);
        if self.bipolar() && v > 0 { format!("+{v}") } else { v.to_string() }
    }
}

/// O numero digitado na caixa de `b`, ou `None` se nao vale (o valor anterior fica).
pub fn interpret_text(text: &str, b: Bar) -> Option<i32> {
    let t = text.trim().replace('−', "-").replace(',', ".");
    let (negative, note_body) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(&t)),
    };
    // So digitos e um ponto (nada de "inf", "nan", "1e3", "0x10" que o f64 aceitaria).
    let points = note_body.matches('.').count();
    if note_body.is_empty() || points > 1 || !note_body.chars().all(|c| c.is_ascii_digit() || c == '.') || note_body == "." {
        return None;
    }
    // So digitos: nunca NaN. Digitos demais viram infinito, que satura no limite do sinal.
    let v: f64 = note_body.parse().ok()?;
    let v = if negative { -v } else { v };
    let (min, max) = b.strip();
    Some(v.round().clamp(min as f64, max as f64) as i32)
}

/// Quais caixas numericas tem texto digitado ainda nao confirmado (suja: da
/// primeira tecla ate Enter/Esc ou outra imagem). Uma caixa limpa mostra sempre
/// o valor atual, focada ou nao; so a suja guarda o que o usuario digita.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DirtyTextboxes([bool; 5]);

impl DirtyTextboxes {
    /// O usuario mudou o texto da caixa de `b`.
    pub fn changed(&mut self, b: Bar) {
        self.0[b.index()] = true;
    }

    /// Enter, Esc ou o valor reescrito na caixa de `b`.
    pub fn clear(&mut self, b: Bar) {
        self.0[b.index()] = false;
    }

    /// Outra imagem/pagina: o que se digitava nao vale para ela.
    pub fn clear_all(&mut self) {
        self.0 = [false; 5];
    }

    /// A caixa de `b` (com `text` agora) deve passar a mostrar `value`.
    pub fn rewrite(&self, b: Bar, text: &str, value: &str) -> bool {
        !self.0[b.index()] && text != value
    }
}

/// ↑/↓ na caixa numerica: `+1`/`-1`, com Shift `+10`/`-10`. Outra tecla: `None`.
pub fn key_step(key: &str, shift: bool) -> Option<i32> {
    let n = if shift { 10 } else { 1 };
    match key {
        "up" => Some(n),
        "down" => Some(-n),
        _ => None,
    }
}

#[cfg(test)]
#[path = "tests/slider.rs"]
mod tests;
