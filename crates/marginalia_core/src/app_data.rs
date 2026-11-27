//! Dados por maquina, guardados FORA do vault (caminhos das fontes, ajustes de
//! imagem, ultimo vault). Persistidos em `<base>/app_data.json`.

use crate::error::Error;
use crate::vault::Result;
use crate::filters::Adjustments;
use crate::highlights::{DEFAULT_COLOR, PALETTE};
use crate::model::ImageRef;
use crate::rotation::Rotation;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const VERSION: u32 = 1;
const FILE: &str = "appdata.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceStatus {
    Ok(PathBuf),
    NoPath,
    NotFound(PathBuf),
}

/// Quantas fontes recentes cada vault lembra.
const MAX_RECENT: usize = 10;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Data {
    #[serde(rename = "version", alias = "versao")]
    version: u32,
    /// vault -> nome da fonte -> pasta no disco.
    #[serde(default)]
    #[serde(rename = "sources", alias = "fontes")]
    sources: BTreeMap<String, BTreeMap<String, PathBuf>>,
    /// Chave `"{fonte}\u{1f}{arquivo}#{pagina}"` (veja `image_key`).
    #[serde(default)]
    #[serde(rename = "adjustments", alias = "ajustes")]
    image_adjustments: BTreeMap<String, Adjustments>,
    /// Rotacao de exibicao por imagem (mesma chave dos ajustes); ausente = 0.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(rename = "rotations", alias = "rotacoes")]
    rotations: BTreeMap<String, Rotation>,
    #[serde(default)]
    #[serde(rename = "last_vault", alias = "ultimo_vault")]
    last_vault: Option<PathBuf>,
    /// Mais recente primeiro, sem duplicatas.
    #[serde(default)]
    #[serde(rename = "vaults")]
    vaults: Vec<PathBuf>,
    /// vault -> nome da fonte -> ultima posicao aberta (onde reabrir a fonte).
    #[serde(default)]
    #[serde(rename = "positions", alias = "posicoes")]
    positions: BTreeMap<String, BTreeMap<String, Position>>,
    /// vault -> nomes das fontes abertas, a mais recente primeiro, sem repetir.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(rename = "recent", alias = "recentes")]
    recent: BTreeMap<String, Vec<String>>,
    /// Cor escolhida para novos grifos; ausente = padrao.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "highlight_color", alias = "cor_grifo")]
    highlight_color: Option<String>,
    /// A barra de ferramentas esta oculta (menu Exibir); ausente = visivel.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[serde(rename = "hide_tools", alias = "ocultar_ferramentas")]
    hide_tools: bool,
    /// Largura que o usuario deu ao painel das gavetas (px); ausente = padrao.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "drawers_width", alias = "largura_gavetas")]
    drawers_width: Option<f32>,
    /// Ao abrir o app sem fonte na linha de comando, reabre a ultima fonte do vault; ausente = nao.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[serde(rename = "reopen_last_source", alias = "reabrir_ultima_fonte")]
    reopen_last_source: bool,
    /// Ritmo do salvamento automatico da nota rapida; ausente = normal.
    #[serde(default, skip_serializing_if = "QuickNoteRhythm::is_default")]
    #[serde(rename = "note_rhythm", alias = "ritmo_nota")]
    quick_note_rhythm: QuickNoteRhythm,
    /// Tamanho da letra da nota rapida; ausente = media.
    #[serde(default, skip_serializing_if = "FontSize::is_default")]
    #[serde(rename = "note_font", alias = "letra_nota")]
    quick_note_font: FontSize,
    /// Tema de cores; ausente = escuro.
    #[serde(default, skip_serializing_if = "Theme::is_default")]
    #[serde(rename = "theme", alias = "tema")]
    theme: Theme,
    /// Avisar de versao nova ao abrir; ausente = sim.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    auto_update_check: bool,
    /// Instante (segundos Unix) da ultima checagem de versao bem-sucedida.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_update_check: Option<i64>,
    /// Versao nova que a pessoa pediu para nao avisar mais ("Pular esta versao").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    skipped_version: Option<String>,
}

fn default_true() -> bool {
    true
}

fn is_true(b: &bool) -> bool {
    *b
}

/// Quanto a nota rapida espera, depois da ultima tecla, para ir ao disco (tela de Configuracoes).
/// As duracoes de cada ritmo ficam em `state::quick_note_session`; aqui so a escolha do usuario.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuickNoteRhythm {
    #[serde(rename = "fast", alias = "rapido")]
    Fast,
    #[default]
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "calm", alias = "calmo")]
    Calm,
}

impl QuickNoteRhythm {
    pub const ALL: [QuickNoteRhythm; 3] = [QuickNoteRhythm::Fast, QuickNoteRhythm::Normal, QuickNoteRhythm::Calm];

    fn is_default(&self) -> bool {
        *self == QuickNoteRhythm::default()
    }
}

/// Tema de cores da janela (tela de Configuracoes). As cores de cada tema ficam na `interface`;
/// aqui so a escolha do usuario. O escuro e o visual de sempre.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    #[serde(rename = "dark", alias = "escuro")]
    Dark,
    #[serde(rename = "light", alias = "claro")]
    Light,
    #[serde(rename = "sepia")]
    Sepia,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::Dark, Theme::Light, Theme::Sepia];

    fn is_default(&self) -> bool {
        *self == Theme::default()
    }
}

/// Tamanho da letra da caixa da nota rapida (tela de Configuracoes). A media e a de sempre.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontSize {
    #[serde(rename = "small", alias = "pequena")]
    Small,
    #[default]
    #[serde(rename = "medium", alias = "media")]
    Media,
    #[serde(rename = "large", alias = "grande")]
    Large,
}

impl FontSize {
    pub const ALL: [FontSize; 3] = [FontSize::Small, FontSize::Media, FontSize::Large];

    /// O tamanho da fonte em pixels.
    pub fn pixels(self) -> f32 {
        match self {
            FontSize::Small => 12.0,
            FontSize::Media => 14.0,
            FontSize::Large => 16.0,
        }
    }

    fn is_default(&self) -> bool {
        *self == FontSize::default()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Position {
    #[serde(rename = "file", alias = "arquivo")]
    file: String,
    #[serde(rename = "page", alias = "pagina")]
    page: u32,
}

impl Default for Data {
    fn default() -> Self {
        Data {
            version: VERSION,
            sources: BTreeMap::new(),
            image_adjustments: BTreeMap::new(),
            rotations: BTreeMap::new(),
            last_vault: None,
            vaults: Vec::new(),
            positions: BTreeMap::new(),
            recent: BTreeMap::new(),
            highlight_color: None,
            hide_tools: false,
            drawers_width: None,
            reopen_last_source: false,
            quick_note_rhythm: QuickNoteRhythm::Normal,
            quick_note_font: FontSize::Media,
            theme: Theme::Dark,
            auto_update_check: true,
            last_update_check: None,
            skipped_version: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppData {
    base: PathBuf,
    data: Data,
}

/// Limpeza lexical, sem tocar o disco (o vault pode estar num NAS desmontado,
/// entao nada de `canonicalize`): tira barras finais e `.`, resolve `..`.
fn normalizes(p: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                let last_normal = matches!(out.components().next_back(), Some(Component::Normal(_)));
                if last_normal {
                    out.pop();
                } else if !out.has_root() {
                    out.push("..");
                }
                // `/..` == `/`: nada a fazer.
            }
            other => out.push(other.as_os_str()),
        }
    }
    if out.as_os_str().is_empty() {
        out.push(".");
    }
    out
}

/// Chave persistida do vault: caminho normalizado como string. Se nao for
/// UTF-8 valido, usa `\u{1}hex:` + bytes em hexadecimal (sem perda e sem colidir
/// com nenhum caminho UTF-8, que nunca contem U+0001 nessa posicao).
fn vault_key(vault: &Path) -> String {
    let n = normalizes(vault);
    match n.to_str() {
        Some(s) => s.to_string(),
        None => {
            #[cfg(unix)]
            let bytes: Vec<u8> = {
                use std::os::unix::ffi::OsStrExt;
                n.as_os_str().as_bytes().to_vec()
            };
            #[cfg(not(unix))]
            let bytes: Vec<u8> = n.to_string_lossy().into_owned().into_bytes();
            let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
            format!("\u{1}hex:{hex}")
        }
    }
}

/// Ajustes sao por imagem (`source` + `file` + `page`), sem o vault: dois
/// vaults que usem o mesmo nome de fonte compartilham ajustes. Aceitavel, pois
/// sao so preferencias de visualizacao. O separador U+001F nao ocorre em nomes.
fn image_key(i: &ImageRef) -> String {
    format!("{}\u{1f}{}#{}", i.source, i.file, i.page)
}

impl AppData {
    /// Pasta padrao por maquina: `<data_dir>/Marginalia`.
    pub fn default_base() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Marginalia")
    }

    pub fn new(base: PathBuf) -> AppData {
        AppData { base, data: Data::default() }
    }

    /// Pasta dos dados do app (`MARGINALIA_APPDATA` ou a padrao): outros
    /// arquivos do app (ex.: o registro de recuperacao da nota rapida) ficam aqui.
    pub fn base(&self) -> &Path {
        &self.base
    }

    /// Arquivo ausente vira `AppData` vazio; JSON corrompido ou de versao
    /// desconhecida e erro.
    pub fn load(base: PathBuf) -> Result<AppData> {
        let file = base.join(FILE);
        let text = match std::fs::read_to_string(&file) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(AppData::new(base))
            }
            Err(e) => return Err(e.into()),
        };
        let data: Data = serde_json::from_str(&text)?;
        if data.version != VERSION {
            return Err(Error::VersionNotSupported(data.version));
        }
        Ok(AppData { base, data })
    }

    /// Escrita atomica: arquivo temporario na mesma pasta e `rename`. Duas
    /// gravacoes do mesmo processo (a interface e o fundo) nunca escrevem o
    /// temporario ao mesmo tempo.
    pub fn save(&self) -> Result<()> {
        static WRITING: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _time = WRITING.lock().unwrap_or_else(|e| e.into_inner());
        std::fs::create_dir_all(&self.base)?;
        let json = serde_json::to_string_pretty(&self.data)?;
        let tmp = self.base.join(format!("{FILE}.tmp"));
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, self.base.join(FILE))?;
        Ok(())
    }

    pub fn set_source_path(&mut self, vault: &Path, name: &str, file_path: PathBuf) {
        self.data
            .sources
            .entry(vault_key(vault))
            .or_default()
            .insert(name.to_string(), file_path);
    }

    pub fn source_path(&self, vault: &Path, name: &str) -> Option<PathBuf> {
        self.data.sources.get(&vault_key(vault))?.get(name).cloned()
    }

    /// A fonte `old` deste vault passou a se chamar `new` (o vault ja foi
    /// reescrito por `Vault::rename_source`): o caminho nesta maquina vai para o
    /// nome novo e os ajustes por imagem sao COPIADOS (ajustes nao sao por vault:
    /// outro vault com uma fonte `old` continua com os seus). Nao salva.
    pub fn rename_source(&mut self, vault: &Path, old: &str, new: &str) {
        if let Some(m) = self.data.sources.get_mut(&vault_key(vault)) {
            if let Some(p) = m.remove(old) {
                m.insert(new.to_string(), p);
            }
        }
        if let Some(m) = self.data.positions.get_mut(&vault_key(vault)) {
            if let Some(p) = m.remove(old) {
                m.insert(new.to_string(), p);
            }
        }
        let prefix = format!("{old}\u{1f}");
        let copies: Vec<(String, Adjustments)> = self
            .data
            .image_adjustments
            .iter()
            .filter_map(|(k, a)| k.strip_prefix(&prefix).map(|rest| (format!("{new}\u{1f}{rest}"), a.clone())))
            .collect();
        self.data.image_adjustments.extend(copies);
        let rotations: Vec<(String, Rotation)> = self
            .data
            .rotations
            .iter()
            .filter_map(|(k, r)| k.strip_prefix(&prefix).map(|rest| (format!("{new}\u{1f}{rest}"), *r)))
            .collect();
        self.data.rotations.extend(rotations);
    }

    /// Ultima posicao aberta da fonte `name` deste vault: (arquivo, pagina).
    pub fn position(&self, vault: &Path, name: &str) -> Option<(String, u32)> {
        let p = self.data.positions.get(&vault_key(vault))?.get(name)?;
        Some((p.file.clone(), p.page))
    }

    /// Lembra a posicao aberta da fonte `name` deste vault. Nao salva.
    pub fn set_position(&mut self, vault: &Path, name: &str, file: &str, page: u32) {
        let p = Position { file: file.to_string(), page };
        self.data.positions.entry(vault_key(vault)).or_default().insert(name.to_string(), p);
    }

    /// Marca `name` como a fonte aberta mais recentemente neste vault (guarda
    /// so as `MAX_RECENT` ultimas). Devolve se a lista mudou. Nao salva.
    pub fn mark_recent(&mut self, vault: &Path, name: &str) -> bool {
        let list = self.data.recent.entry(vault_key(vault)).or_default();
        if list.first().is_some_and(|n| n == name) {
            return false;
        }
        list.retain(|n| n != name);
        list.insert(0, name.to_string());
        list.truncate(MAX_RECENT);
        true
    }

    /// Fontes abertas neste vault, a mais recente primeiro.
    pub fn recent(&self, vault: &Path) -> &[String] {
        self.data.recent.get(&vault_key(vault)).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn resolve_source(&self, vault: &Path, name: &str) -> SourceStatus {
        match self.source_path(vault, name) {
            None => SourceStatus::NoPath,
            Some(p) if p.exists() => SourceStatus::Ok(p),
            Some(p) => SourceStatus::NotFound(p),
        }
    }

    /// Campos nao finitos (NaN, infinito) viram o valor neutro do campo antes
    /// de tudo, pois `serde_json` os grava como `null` e o proximo `load`
    /// falharia. Ajuste neutro remove a entrada, para o arquivo nao crescer.
    pub fn set_adjustments(&mut self, image: &ImageRef, mut a: Adjustments) {
        let fin = |v: f32, neutral: f32| if v.is_finite() { v } else { neutral };
        a.brightness = fin(a.brightness, 0.0);
        a.contrast = fin(a.contrast, 1.0);
        a.sharpen = fin(a.sharpen, 0.0);
        a.highpass = fin(a.highpass, 0.0);
        let k = image_key(image);
        if a.is_neutral() {
            self.data.image_adjustments.remove(&k);
        } else {
            self.data.image_adjustments.insert(k, a);
        }
    }

    pub fn adjustments(&self, image: &ImageRef) -> Adjustments {
        self.data
            .image_adjustments
            .get(&image_key(image))
            .cloned()
            .unwrap_or_default()
    }

    /// Rotacao de exibicao da imagem (0 se nunca girada). Nao e por vault.
    pub fn rotation(&self, image: &ImageRef) -> Rotation {
        self.data.rotations.get(&image_key(image)).copied().unwrap_or_default()
    }

    /// Guarda a rotacao da imagem; `R0` remove a entrada. Nao salva.
    pub fn set_rotation(&mut self, image: &ImageRef, rot: Rotation) {
        let k = image_key(image);
        if rot == Rotation::R0 {
            self.data.rotations.remove(&k);
        } else {
            self.data.rotations.insert(k, rot);
        }
    }

    /// A barra de ferramentas esta oculta? (padrao: visivel)
    pub fn hidden_tools(&self) -> bool {
        self.data.hide_tools
    }

    /// Guarda se a barra de ferramentas esta oculta. Nao salva.
    pub fn set_tools_hidden(&mut self, v: bool) {
        self.data.hide_tools = v;
    }

    /// Largura escolhida para o painel das gavetas (`None`: a padrao).
    pub fn drawers_width(&self) -> Option<f32> {
        self.data.drawers_width
    }

    /// Guarda a largura do painel das gavetas (`None` volta ao padrao). Nao salva.
    pub fn set_width_drawers(&mut self, v: Option<f32>) {
        self.data.drawers_width = v;
    }

    /// Reabrir a ultima fonte ao abrir o app? (padrao: nao)
    pub fn reopen_last_source(&self) -> bool {
        self.data.reopen_last_source
    }

    /// Guarda se a ultima fonte reabre ao abrir o app. Nao salva.
    pub fn set_reopen_last_source(&mut self, v: bool) {
        self.data.reopen_last_source = v;
    }

    /// Ritmo do salvamento automatico da nota rapida (padrao: normal).
    pub fn quick_note_rhythm(&self) -> QuickNoteRhythm {
        self.data.quick_note_rhythm
    }

    /// Guarda o ritmo do salvamento da nota. Nao salva.
    pub fn set_rhythm_quick_note(&mut self, r: QuickNoteRhythm) {
        self.data.quick_note_rhythm = r;
    }

    /// Tema de cores da janela (padrao: escuro).
    pub fn theme(&self) -> Theme {
        self.data.theme
    }

    /// Guarda o tema de cores. Nao salva.
    pub fn set_theme(&mut self, t: Theme) {
        self.data.theme = t;
    }

    /// Avisar de versao nova ao abrir o app? (padrao: sim)
    pub fn auto_update_check(&self) -> bool {
        self.data.auto_update_check
    }

    /// Guarda se o app avisa de versao nova ao abrir. Nao salva.
    pub fn set_auto_update_check(&mut self, v: bool) {
        self.data.auto_update_check = v;
    }

    /// Instante (segundos Unix) da ultima checagem de versao bem-sucedida.
    pub fn last_update_check(&self) -> Option<i64> {
        self.data.last_update_check
    }

    /// Guarda o instante da ultima checagem bem-sucedida. Nao salva.
    pub fn set_last_update_check(&mut self, t: i64) {
        self.data.last_update_check = Some(t);
    }

    /// A versao que a pessoa pulou, se alguma.
    pub fn skipped_version(&self) -> Option<crate::update::Version> {
        self.data.skipped_version.as_deref().and_then(crate::update::Version::parse)
    }

    /// Guarda (ou limpa) a versao pulada. Nao salva.
    pub fn set_skipped_version(&mut self, v: Option<crate::update::Version>) {
        self.data.skipped_version = v.map(|v| v.to_string());
    }

    /// Tamanho da letra da nota rapida (padrao: media).
    pub fn quick_note_font(&self) -> FontSize {
        self.data.quick_note_font
    }

    /// Guarda o tamanho da letra da nota rapida. Nao salva.
    pub fn set_font_quick_note(&mut self, t: FontSize) {
        self.data.quick_note_font = t;
    }

    /// A fonte mais recente deste vault que ainda existe no disco (a que "reabrir a ultima
    /// fonte" abre): o nome e o caminho. Pula as que perderam o caminho ou sumiram.
    pub fn last_source_existing(&self, vault: &Path) -> Option<(String, PathBuf)> {
        self.recent(vault).iter().find_map(|name| match self.resolve_source(vault, name) {
            SourceStatus::Ok(p) => Some((name.clone(), p)),
            _ => None,
        })
    }

    /// Volta as configuracoes da tela de Configuracoes aos padroes. Nao mexe nos dados de
    /// trabalho (fontes, ajustes, rotacoes, vaults, posicoes), na cor dos grifos nem na barra de ferramentas. Nao salva.
    pub fn restore_settings(&mut self) {
        self.data.reopen_last_source = false;
        self.data.quick_note_rhythm = QuickNoteRhythm::default();
        self.data.quick_note_font = FontSize::default();
        self.data.drawers_width = None;
        self.data.theme = Theme::default();
        self.data.auto_update_check = true;
    }

    pub fn last_vault(&self) -> Option<&Path> {
        self.data.last_vault.as_deref()
    }

    pub fn set_last_vault(&mut self, p: PathBuf) {
        let p = normalizes(&p);
        self.data.vaults.retain(|v| v != &p);
        self.data.vaults.insert(0, p.clone());
        self.data.last_vault = Some(p);
    }

    /// Cor dos novos grifos (a padrao se nunca escolhida). Nao e por vault.
    pub fn highlight_color(&self) -> &str {
        self.data.highlight_color.as_deref().unwrap_or(DEFAULT_COLOR)
    }

    /// Escolhe a cor dos novos grifos. Valor fora de `PALETTE` e RECUSADO
    /// (`Error::ColorOutsideOfThePalette`), nunca normalizado. Nao salva.
    pub fn set_color_highlight(&mut self, color: &str) -> Result<()> {
        if !PALETTE.contains(&color) {
            return Err(Error::ColorOutsideOfThePalette(color.to_string()));
        }
        self.data.highlight_color = (color != DEFAULT_COLOR).then(|| color.to_string());
        Ok(())
    }

    pub fn known_vaults(&self) -> &[PathBuf] {
        &self.data.vaults
    }
}

#[cfg(test)]
#[path = "tests/app_data.rs"]
mod tests;
