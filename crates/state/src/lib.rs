//! Estado e logica do app, sem gpui (regra R4): navegacao, carga de imagens,
//! ajustes, nota rapida, gavetas, grifos. A `interface` so desenha e repassa
//! eventos; nada aqui conhece janela, menu ou textura.

pub mod schedule_adjustment;
pub mod update_check;
pub mod annotate;
pub mod app_state;
pub mod tree;
pub mod load;
pub mod drawers;
pub mod layout;
pub mod quick_note_session;
pub mod open_source;
pub mod image_frame;
pub mod quick_note;
pub mod relink;
pub mod splash_vault;
pub mod vault_picker;
pub mod viewer_math;
