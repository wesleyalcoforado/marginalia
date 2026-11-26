use crate::model::ImageRef;
use crate::rotation::Rotation;
use crate::vault::{safe_name, Result, Vault};
use crate::Error;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;

/// Marca sobre uma imagem; coordenadas relativas (0..=1).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind")]
pub enum Highlight {
    #[serde(rename = "rectangle", alias = "retangulo")]
    Rectangle {
        id: String,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[serde(rename = "color", alias = "cor")]
        color: Option<String>,
    },
    /// Seta: `x`,`y` e a PONTA; `tail` (opcional) e a outra extremidade.
    #[serde(rename = "pointer", alias = "ponteiro")]
    Pointer {
        id: String,
        x: f32,
        y: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[serde(rename = "tail", alias = "cauda")]
        tail: Option<(f32, f32)>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[serde(rename = "color", alias = "cor")]
        color: Option<String>,
    },
}

/// Espelho so para ler: o `tag` do serde nao aceita `alias`, entao `Deserialize`
/// de `Highlight` renomeia a chave antiga `tipo` para `kind` antes de ler.
#[derive(Deserialize)]
#[serde(tag = "kind", remote = "Highlight")]
enum HighlightWire {
    #[serde(rename = "rectangle", alias = "retangulo")]
    Rectangle {
        id: String,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[serde(rename = "color", alias = "cor")]
        color: Option<String>,
    },
    /// Seta: `x`,`y` e a PONTA; `tail` (opcional) e a outra extremidade.
    #[serde(rename = "pointer", alias = "ponteiro")]
    Pointer {
        id: String,
        x: f32,
        y: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[serde(rename = "tail", alias = "cauda")]
        tail: Option<(f32, f32)>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[serde(rename = "color", alias = "cor")]
        color: Option<String>,
    },
}

impl<'de> Deserialize<'de> for Highlight {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        use serde::de::Error as _;
        let mut v = serde_json::Value::deserialize(d)?;
        if let Some(o) = v.as_object_mut() {
            if !o.contains_key("kind") {
                if let Some(t) = o.remove("tipo") {
                    o.insert("kind".into(), t);
                }
            }
        }
        HighlightWire::deserialize(v).map_err(D::Error::custom)
    }
}

/// Cor usada quando o grifo nao guarda uma (amarelo).
pub const DEFAULT_COLOR: &str = "#ffc400";

/// Cores oferecidas para grifos; a primeira e a padrao.
pub const PALETTE: [&str; 6] = ["#ffc400", "#e53935", "#43a047", "#1e88e5", "#d81b60", "#ffffff"];

fn unit(v: f32) -> bool {
    (0.0..=1.0).contains(&v)
}

impl Highlight {
    pub fn id(&self) -> &str {
        match self {
            Highlight::Rectangle { id, .. } | Highlight::Pointer { id, .. } => id,
        }
    }

    /// Cor guardada ou `DEFAULT_COLOR`.
    pub fn color(&self) -> &str {
        match self {
            Highlight::Rectangle { color, .. } | Highlight::Pointer { color, .. } => color.as_deref().unwrap_or(DEFAULT_COLOR),
        }
    }

    pub fn with_color(mut self, fresh: &str) -> Highlight {
        match &mut self {
            Highlight::Rectangle { color, .. } | Highlight::Pointer { color, .. } => *color = Some(fresh.to_string()),
        }
        self
    }

    /// Troca a cor guardando `None` para `DEFAULT_COLOR` (o JSON fica igual ao de um
    /// grifo novo na cor padrao, sem `color` explicita).
    pub fn recolored(mut self, fresh: &str) -> Highlight {
        match &mut self {
            Highlight::Rectangle { color, .. } | Highlight::Pointer { color, .. } => {
                *color = (fresh != DEFAULT_COLOR).then(|| fresh.to_string())
            }
        }
        self
    }

    /// O mesmo grifo com outro id (quem cria a forma, ex.: `annotate::arrow`, nao sabe o id).
    pub fn with_id(mut self, new: String) -> Highlight {
        match &mut self {
            Highlight::Rectangle { id, .. } | Highlight::Pointer { id, .. } => *id = new,
        }
        self
    }

    /// O grifo levado da imagem para a exibicao (a pagina girada em `rot`): as
    /// coordenadas relativas passam por `Rotation::display_to`; um retangulo
    /// continua retangulo (cantos reordenados). `R0` devolve o grifo igual.
    pub fn display_to(&self, rot: Rotation) -> Highlight {
        self.converted(rot, Rotation::display_to)
    }

    /// Inverso de [`Highlight::display_to`]: um grifo desenhado sobre a pagina
    /// girada volta para as coordenadas da imagem, que e o que o vault guarda.
    pub fn from_display(&self, rot: Rotation) -> Highlight {
        self.converted(rot, Rotation::from_display)
    }

    fn converted(&self, rot: Rotation, f: fn(Rotation, (f32, f32)) -> (f32, f32)) -> Highlight {
        if rot == Rotation::R0 {
            return self.clone();
        }
        match self {
            Highlight::Rectangle { id, x, y, w, h, color } => {
                let a = f(rot, (*x, *y));
                let b = f(rot, (x + w, y + h));
                let (x0, y0) = (a.0.min(b.0).clamp(0.0, 1.0), a.1.min(b.1).clamp(0.0, 1.0));
                let (x1, y1) = (a.0.max(b.0).clamp(0.0, 1.0), a.1.max(b.1).clamp(0.0, 1.0));
                // `x + w <= 1` tem de valer apos a conta em ponto flutuante (`validate`).
                Highlight::Rectangle {
                    id: id.clone(),
                    x: x0,
                    y: y0,
                    w: (x1 - x0).min(1.0 - x0),
                    h: (y1 - y0).min(1.0 - y0),
                    color: color.clone(),
                }
            }
            Highlight::Pointer { id, x, y, tail, color } => {
                let (nx, ny) = f(rot, (*x, *y));
                Highlight::Pointer { id: id.clone(), x: nx, y: ny, tail: tail.map(|c| f(rot, c)), color: color.clone() }
            }
        }
    }

    fn validate(&self) -> Result<()> {
        let ok = match *self {
            Highlight::Rectangle { x, y, w, h, .. } => {
                unit(x) && unit(y) && unit(w) && unit(h) && w > 0.0 && h > 0.0 && x + w <= 1.0 && y + h <= 1.0
            }
            Highlight::Pointer { x, y, tail, .. } => {
                unit(x) && unit(y) && tail.is_none_or(|(cx, cy)| unit(cx) && unit(cy) && (cx, cy) != (x, y))
            }
        };
        if ok {
            Ok(())
        } else {
            Err(Error::OutsideHighlight(self.id().to_string()))
        }
    }
}

fn key(image: &ImageRef) -> String {
    format!("{}#{}", image.file, image.page)
}

fn default_version() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HighlightsFile {
    #[serde(default = "default_version")]
    #[serde(rename = "version", alias = "versao")]
    pub version: u32,
    #[serde(default)]
    #[serde(rename = "images", alias = "imagens")]
    pub images: BTreeMap<String, Vec<Highlight>>,
}

impl Default for HighlightsFile {
    fn default() -> Self {
        HighlightsFile { version: 1, images: BTreeMap::new() }
    }
}

impl HighlightsFile {
    fn path(vault: &Vault, source: &str) -> Result<std::path::PathBuf> {
        Ok(vault.root.join("grifos").join(format!("{}.json", safe_name(source)?)))
    }

    pub fn load(vault: &Vault, source: &str) -> Result<HighlightsFile> {
        let path = Self::path(vault, source)?;
        if !path.exists() {
            return Ok(HighlightsFile::default());
        }
        let f: HighlightsFile = serde_json::from_str(&fs::read_to_string(path)?)?;
        for g in f.images.values().flatten() {
            g.validate()?;
        }
        Ok(f)
    }

    pub fn save(&self, vault: &Vault, source: &str) -> Result<()> {
        let path = Self::path(vault, source)?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        crate::vault::write_atomic(&path, &serde_json::to_string_pretty(self)?)
    }

    pub fn add(&mut self, image: &ImageRef, g: Highlight) -> Result<()> {
        g.validate()?;
        self.images.entry(key(image)).or_default().push(g);
        Ok(())
    }

    pub fn remove(&mut self, image: &ImageRef, id: &str) {
        let k = key(image);
        if let Some(v) = self.images.get_mut(&k) {
            v.retain(|g| g.id() != id);
            if v.is_empty() {
                self.images.remove(&k);
            }
        }
    }

    pub fn for_image(&self, image: &ImageRef) -> &[Highlight] {
        self.images.get(&key(image)).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn find(&self, image: &ImageRef, id: &str) -> Option<&Highlight> {
        self.for_image(image).iter().find(|g| g.id() == id)
    }

    /// Troca a cor do grifo `id` nesta imagem; `false` se nao existe. Nao salva.
    pub fn recolor(&mut self, image: &ImageRef, id: &str, color: &str) -> bool {
        let Some(v) = self.images.get_mut(&key(image)) else { return false };
        let Some(g) = v.iter_mut().find(|g| g.id() == id) else { return false };
        *g = g.clone().recolored(color);
        true
    }
}

/// Junta os grifos de `de` em `to` (uniao por imagem e id; num id repetido
/// vale o de `to`) e apaga `de`. Conteudos identicos: so apaga `de`. Algum
/// dos dois ilegivel: erro citando o arquivo e nada muda (nenhum grifo se perde).
pub(crate) fn merge_at(de: &std::path::Path, to: &std::path::Path) -> Result<()> {
    let read_file = |p: &std::path::Path| -> Result<String> {
        fs::read_to_string(p).map_err(|e| Error::Document(p.to_path_buf(), Box::new(e.into())))
    };
    let parse = |p: &std::path::Path, t: &str| -> Result<HighlightsFile> {
        serde_json::from_str(t).map_err(|e| Error::Document(p.to_path_buf(), Box::new(e.into())))
    };
    let (t_of, t_to) = (read_file(de)?, read_file(to)?);
    if t_of != t_to {
        let old = parse(de, &t_of)?;
        let mut together = parse(to, &t_to)?;
        for (k, gs) in old.images {
            let target = together.images.entry(k).or_default();
            for g in gs {
                if !target.iter().any(|x| x.id() == g.id()) {
                    target.push(g);
                }
            }
        }
        crate::vault::write_atomic(to, &serde_json::to_string_pretty(&together)?)?;
    }
    fs::remove_file(de)?;
    Ok(())
}

/// `g` + milissegundos desde a epoca Unix (`now` tratado como UTC).
pub fn new_highlight_id(now: NaiveDateTime) -> String {
    format!("g{}", now.and_utc().timestamp_millis())
}

#[cfg(test)]
#[path = "tests/highlights.rs"]
mod tests;
