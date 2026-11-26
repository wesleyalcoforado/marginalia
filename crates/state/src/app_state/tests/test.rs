//! Estado de teste com vault e fonte em memoria (para os testes de outros modulos).

use super::*;
use marginalia_core::source::ImageEntry;

/// Fonte em memoria: PNGs 3x2 chamados `p01.png`, `p02.png`...
pub struct FakeSource {
    entries: Vec<ImageEntry>,
    png: Vec<u8>,
}

impl ImageSource for FakeSource {
    fn entries(&self) -> &[ImageEntry] {
        &self.entries
    }
    fn read(&self, path: &str) -> marginalia_core::vault::Result<Vec<u8>> {
        if self.entries.iter().any(|e| e.path == path) {
            Ok(self.png.clone())
        } else {
            Err(marginalia_core::Error::InputNotFound(path.into()))
        }
    }
}

pub fn source(n: usize) -> Arc<FakeSource> {
    let img = image::RgbaImage::from_pixel(3, 2, image::Rgba([10, 20, 30, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    let entries = (1..=n).map(|i| ImageEntry { path: format!("p{i:02}.png") }).collect();
    Arc::new(FakeSource { entries, png: out.into_inner() })
}

/// Vault em `t/<folder>` e a fonte "Livro" (3 quadros) aberta e exibida no 1o.
pub fn with_vault(t: &Path, folder: &str) -> AppState {
    let vault = Vault::open_or_create(&t.join(folder)).unwrap();
    let mut st = AppState::new(Some(vault), AppData::new(t.join("appdata")), t.join("cache"));
    let p = st.adopt_source("Livro", source(3)).unwrap();
    let _ = st.load_now(p);
    st
}

/// Resolvedor de atalhos dos testes (a `interface` registra o de verdade): os
/// mesmos simbolos que o app mostra no macOS para os ids que os textos do
/// `state` citam. Idempotente; todo teste que compara um texto com atalho o chama.
pub fn register_shortcuts_false() {
    texts::register_shortcuts(false_shortcut);
}

fn false_shortcut(id: &str) -> Option<String> {
    let t = match id {
        "open_source" => "⌘O",
        "open_vault" => "⇧⌘V",
        "notes" => "⌘1",
        "sources" => "⌘2",
        "tasks" => "⌘3",
        "finish_quick_note" => "⌘↵",
        _ => return None,
    };
    Some(t.to_string())
}
