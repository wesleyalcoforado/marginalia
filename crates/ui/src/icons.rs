//! Icones Unicons (Iconscout, Apache-2.0; ver `assets/icons/LICENSE-unicons.txt`)
//! embutidos no binario e o `AssetSource` que os serve ao `svg()` do gpui.
//!
//! Os SVGs usam `currentColor`; o gpui os pinta como mascara com a cor do texto
//! do elemento, entao `icon` so precisa de tamanho e cor (o SVG diz `1em`:
//! o tamanho e sempre explicito).

use std::borrow::Cow;

use gpui::{img, px, svg, AssetSource, Hsla, IntoElement, Pixels, Result, SharedString, Styled, Window};

/// Um icone do conjunto embutido.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Brush,
    FileSearch,
    Files,
    OpenFolder,
    Close,
    Marked,
    Square,
    AdjustmentsPanel,
    Trash,
    Undo,
    Previous,
    Next,
    More,
    Less,
    ZoomUp,
    ZoomDown,
    Vault,
    Eye,
    ClosedEye,
    Textbox,
    Arrow,
    RotateLeft,
    RotateRight,
    Adjust,
    Help,
    Edit,
    Image,
    Folder,
    Pdf,
    Zip,
    RecordStart,
    RecordEnd,
    ExtendRecord,
    Above,
    Below,
    Palette,
}

/// Todas as variantes (a lista que os testes cruzam com os arquivos).
pub const ALL: [Icon; 36] = [
    Icon::Brush,
    Icon::FileSearch,
    Icon::Files,
    Icon::OpenFolder,
    Icon::Close,
    Icon::Marked,
    Icon::Square,
    Icon::AdjustmentsPanel,
    Icon::Trash,
    Icon::Undo,
    Icon::Previous,
    Icon::Next,
    Icon::More,
    Icon::Less,
    Icon::ZoomUp,
    Icon::ZoomDown,
    Icon::Vault,
    Icon::Eye,
    Icon::ClosedEye,
    Icon::Textbox,
    Icon::Arrow,
    Icon::RotateLeft,
    Icon::RotateRight,
    Icon::Adjust,
    Icon::Help,
    Icon::Edit,
    Icon::Image,
    Icon::Folder,
    Icon::Pdf,
    Icon::Zip,
    Icon::RecordStart,
    Icon::RecordEnd,
    Icon::ExtendRecord,
    Icon::Above,
    Icon::Below,
    Icon::Palette,
];

impl Icon {
    /// Caminho do icone no `AssetSource` (o que `svg().path` recebe).
    pub fn file(self) -> &'static str {
        match self {
            Icon::Brush => "icons/paint-tool.svg",
            Icon::FileSearch => "icons/file-search-alt.svg",
            Icon::Files => "icons/files-landscapes.svg",
            Icon::OpenFolder => "icons/folder-open.svg",
            Icon::Close => "icons/times.svg",
            Icon::Marked => "icons/check-square.svg",
            Icon::Square => "icons/square.svg",
            Icon::AdjustmentsPanel => "icons/setting.svg",
            Icon::Trash => "icons/trash-alt.svg",
            Icon::Undo => "icons/corner-up-left.svg",
            Icon::Previous => "icons/angle-left.svg",
            Icon::Next => "icons/angle-right.svg",
            Icon::More => "icons/plus.svg",
            Icon::Less => "icons/minus.svg",
            Icon::ZoomUp => "icons/search-plus.svg",
            Icon::ZoomDown => "icons/search-minus.svg",
            Icon::Vault => "icons/database.svg",
            Icon::Eye => "icons/eye.svg",
            Icon::ClosedEye => "icons/eye-slash.svg",
            Icon::Textbox => "icons/square-shape.svg",
            Icon::Arrow => "icons/arrow-up-right.svg",
            Icon::RotateLeft => "icons/redo-esquerda.svg",
            Icon::RotateRight => "icons/redo.svg",
            Icon::Adjust => "icons/focus.svg",
            Icon::Help => "icons/question-circle.svg",
            Icon::Edit => "icons/edit-alt.svg",
            Icon::Image => "icons/image.svg",
            Icon::Folder => "icons/folder.svg",
            Icon::Pdf => "icons/file-alt.svg",
            Icon::Zip => "icons/compress.svg",
            Icon::RecordStart => "icons/arrow-from-right.svg",
            Icon::RecordEnd => "icons/left-arrow-from-left.svg",
            Icon::ExtendRecord => "icons/arrow-to-right.svg",
            Icon::Above => "icons/angle-up.svg",
            Icon::Below => "icons/angle-down.svg",
            Icon::Palette => "icons/palette.svg",
        }
    }

    /// Os bytes embutidos (nada de caminho no disco em tempo de execucao).
    fn bytes(self) -> &'static [u8] {
        match self {
            Icon::Brush => include_bytes!("../assets/icons/paint-tool.svg"),
            Icon::FileSearch => include_bytes!("../assets/icons/file-search-alt.svg"),
            Icon::Files => include_bytes!("../assets/icons/files-landscapes.svg"),
            Icon::OpenFolder => include_bytes!("../assets/icons/folder-open.svg"),
            Icon::Close => include_bytes!("../assets/icons/times.svg"),
            Icon::Marked => include_bytes!("../assets/icons/check-square.svg"),
            Icon::Square => include_bytes!("../assets/icons/square.svg"),
            Icon::AdjustmentsPanel => include_bytes!("../assets/icons/setting.svg"),
            Icon::Trash => include_bytes!("../assets/icons/trash-alt.svg"),
            Icon::Undo => include_bytes!("../assets/icons/corner-up-left.svg"),
            Icon::Previous => include_bytes!("../assets/icons/angle-left.svg"),
            Icon::Next => include_bytes!("../assets/icons/angle-right.svg"),
            Icon::More => include_bytes!("../assets/icons/plus.svg"),
            Icon::Less => include_bytes!("../assets/icons/minus.svg"),
            Icon::ZoomUp => include_bytes!("../assets/icons/search-plus.svg"),
            Icon::ZoomDown => include_bytes!("../assets/icons/search-minus.svg"),
            Icon::Vault => include_bytes!("../assets/icons/database.svg"),
            Icon::Eye => include_bytes!("../assets/icons/eye.svg"),
            Icon::ClosedEye => include_bytes!("../assets/icons/eye-slash.svg"),
            Icon::Textbox => include_bytes!("../assets/icons/square-shape.svg"),
            Icon::Arrow => include_bytes!("../assets/icons/arrow-up-right.svg"),
            Icon::RotateLeft => include_bytes!("../assets/icons/redo-esquerda.svg"),
            Icon::RotateRight => include_bytes!("../assets/icons/redo.svg"),
            Icon::Adjust => include_bytes!("../assets/icons/focus.svg"),
            Icon::Help => include_bytes!("../assets/icons/question-circle.svg"),
            Icon::Edit => include_bytes!("../assets/icons/edit-alt.svg"),
            Icon::Image => include_bytes!("../assets/icons/image.svg"),
            Icon::Folder => include_bytes!("../assets/icons/folder.svg"),
            Icon::Pdf => include_bytes!("../assets/icons/file-alt.svg"),
            Icon::Zip => include_bytes!("../assets/icons/compress.svg"),
            Icon::RecordStart => include_bytes!("../assets/icons/arrow-from-right.svg"),
            Icon::RecordEnd => include_bytes!("../assets/icons/left-arrow-from-left.svg"),
            Icon::ExtendRecord => include_bytes!("../assets/icons/arrow-to-right.svg"),
            Icon::Above => include_bytes!("../assets/icons/angle-up.svg"),
            Icon::Below => include_bytes!("../assets/icons/angle-down.svg"),
            Icon::Palette => include_bytes!("../assets/icons/palette.svg"),
        }
    }
}

/// Um icone quadrado de `size` pixels, pintado com `color`.
pub fn icon(i: Icon, size: f32, color: impl Into<Hsla>) -> impl IntoElement {
    let side: Pixels = px(size);
    svg().path(i.file()).size(side).flex_none().text_color(color)
}

/// O icone do app em varios tamanhos (lado em pixels fisicos, caminho no `AssetSource`, bytes). Sao 96 px logicos nas
/// escalas 1, 1.25, 1.5, 2 e 3: o gpui reduz textura sem filtro de qualidade (1024 -> 96 fica craquelado no Windows),
/// entao a imagem sai 1:1. Gerados por `scripts/gerar_icones.py` a partir de `assets/app/marginalia.png`.
const APP_ICONS: [(u32, &str, &[u8]); 5] = [
    (96, "app/marginalia-96.png", include_bytes!("../assets/app/marginalia-96.png")),
    (120, "app/marginalia-120.png", include_bytes!("../assets/app/marginalia-120.png")),
    (144, "app/marginalia-144.png", include_bytes!("../assets/app/marginalia-144.png")),
    (192, "app/marginalia-192.png", include_bytes!("../assets/app/marginalia-192.png")),
    (288, "app/marginalia-288.png", include_bytes!("../assets/app/marginalia-288.png")),
];

/// Caminho do menor tamanho do icone do app que cobre `side` pixels logicos na escala `scale` (o maior, se nenhum cobre).
pub fn app_icon_path(side: f32, scale: f32) -> &'static str {
    let needed = (side * scale).ceil() as u32;
    APP_ICONS.iter().find(|(l, _, _)| *l >= needed).unwrap_or(&APP_ICONS[APP_ICONS.len() - 1]).1
}

/// O icone do app com `side` pixels logicos, no tamanho certo para a escala da janela.
pub fn app_icon(window: &Window, side: f32) -> impl IntoElement {
    img(app_icon_path(side, window.scale_factor())).size(px(side)).flex_none()
}

/// Fonte de assets do app: serve os icones embutidos e o icone do app.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some((_, _, bytes)) = APP_ICONS.iter().find(|(_, p, _)| *p == path) {
            return Ok(Some(Cow::Borrowed(*bytes)));
        }
        Ok(ALL.iter().find(|i| i.file() == path).map(|i| Cow::Borrowed(i.bytes())))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let folder = path.trim_end_matches('/');
        Ok(ALL
            .iter()
            .filter(|i| i.file().rsplit_once('/').is_some_and(|(p, _)| p == folder))
            .map(|i| SharedString::from(i.file()))
            .collect())
    }
}

#[cfg(test)]
#[path = "tests/icons.rs"]
mod tests;
