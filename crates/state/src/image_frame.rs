//! Uma posicao de imagem (fonte, arquivo, pagina e, se houver, o enquadramento):
//! o que as notas, as notas e a navegacao trocam entre si. So dados.

use marginalia_core::model::{Framing, ImageRef};
use marginalia_core::prefetch;

/// Identifica uma imagem (fonte + arquivo + pagina) e, opcionalmente, o
/// enquadramento com que deve abrir (de uma nota).
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub image: ImageRef,
    pub framing: Option<Framing>,
}

impl Frame {
    pub fn new(source: &str, file: &str, page: u32) -> Frame {
        Frame {
            image: ImageRef { source: source.into(), file: file.into(), page },
            framing: None,
        }
    }

    pub(crate) fn target(&self) -> prefetch::Frame {
        prefetch::Frame { path: self.image.file.clone(), page: self.image.page }
    }
}
