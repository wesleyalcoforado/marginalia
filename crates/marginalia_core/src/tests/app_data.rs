use super::*;
use tempfile::tempdir;

fn img(f: &str, a: &str, p: u32) -> ImageRef {
    ImageRef { source: f.into(), file: a.into(), page: p }
}

#[test]
fn source_never_mapped_no_path() {
    let d = AppData::new(PathBuf::from("/x"));
    assert_eq!(d.resolve_source(Path::new("/v"), "Livro"), SourceStatus::NoPath);
}

#[test]
fn path_missing_not_found_and_existing_ok() {
    let t = tempdir().unwrap();
    let mut d = AppData::new(t.path().join("base"));
    let absent = t.path().join("nao-existe");
    d.set_source_path(Path::new("/v"), "Livro", absent.clone());
    assert_eq!(d.resolve_source(Path::new("/v"), "Livro"), SourceStatus::NotFound(absent));
    let real = t.path().join("real");
    std::fs::create_dir(&real).unwrap();
    d.set_source_path(Path::new("/v"), "Livro", real.clone());
    assert_eq!(d.resolve_source(Path::new("/v"), "Livro"), SourceStatus::Ok(real));
    assert_eq!(d.source_path(Path::new("/outro"), "Livro"), None);
}

#[test]
fn rotation_survives_the_save_and_load_and_r_0_remove_the_input() {
    let t = tempdir().unwrap();
    let base = t.path().join("base");
    let mut d = AppData::new(base.clone());
    let i = img("Livro", "p.jpg", 1);
    assert_eq!(d.rotation(&i), Rotation::R0);
    d.set_rotation(&i, Rotation::R270);
    d.save().unwrap();
    let mut c = AppData::load(base.clone()).unwrap();
    assert_eq!(c.rotation(&i), Rotation::R270);
    assert_eq!(c.rotation(&img("Livro", "p.jpg", 2)), Rotation::R0);
    c.set_rotation(&i, Rotation::R0);
    c.save().unwrap();
    let text = std::fs::read_to_string(base.join(FILE)).unwrap();
    assert!(!text.contains("rotacoes"), "{text}");
}

#[test]
fn tools_visible_by_default() {
    assert!(!AppData::new(PathBuf::from("/x")).hidden_tools());
}

#[test]
fn hide_tools_persists() {
    let t = tempdir().unwrap();
    let base = t.path().join("base");
    let mut d = AppData::new(base.clone());
    d.set_tools_hidden(true);
    d.save().unwrap();
    let mut c = AppData::load(base.clone()).unwrap();
    assert!(c.hidden_tools());
    c.set_tools_hidden(false);
    c.save().unwrap();
    let text = std::fs::read_to_string(base.join(FILE)).unwrap();
    assert!(!text.contains("ocultar_ferramentas"), "{text}");
}

#[test]
fn app_data_old_no_the_field_opens_visible() {
    let t = tempdir().unwrap();
    std::fs::write(t.path().join(FILE), r#"{"versao":1}"#).unwrap();
    assert!(!AppData::load(t.path().to_path_buf()).unwrap().hidden_tools());
}

#[test]
fn file_old_no_rotations_continues_opening() {
    let t = tempdir().unwrap();
    std::fs::write(t.path().join(FILE), r#"{"versao":1}"#).unwrap();
    let d = AppData::load(t.path().to_path_buf()).unwrap();
    assert_eq!(d.rotation(&img("L", "a.jpg", 0)), Rotation::R0);
}

#[test]
fn rename_source_copy_the_rotation() {
    let mut d = AppData::new(PathBuf::from("/x"));
    d.set_rotation(&img("Velho", "a.jpg", 0), Rotation::R90);
    d.rename_source(Path::new("/v"), "Velho", "Novo");
    assert_eq!(d.rotation(&img("Novo", "a.jpg", 0)), Rotation::R90);
    assert_eq!(d.rotation(&img("Velho", "a.jpg", 0)), Rotation::R90);
}

#[test]
fn survives_the_save_and_load() {
    let t = tempdir().unwrap();
    let base = t.path().join("sub/base");
    let mut d = AppData::new(base.clone());
    d.set_source_path(Path::new("/v"), "Livro", PathBuf::from("/a/b"));
    let adj = Adjustments { brightness: 0.2, contrast: 1.5, threshold: Some(128), ..Adjustments::default() };
    let i = img("Livro", "p.jpg", 3);
    d.set_adjustments(&i, adj.clone());
    d.set_last_vault(PathBuf::from("/v"));
    d.save().unwrap();
    let c = AppData::load(base).unwrap();
    assert_eq!(c.source_path(Path::new("/v"), "Livro"), Some(PathBuf::from("/a/b")));
    assert_eq!(c.adjustments(&i), adj);
    assert_eq!(c.last_vault(), Some(Path::new("/v")));
    assert_eq!(c.adjustments(&img("Livro", "p.jpg", 4)), Adjustments::default());
}

#[test]
fn last_position_by_vault_and_source_survives_and_follows_the_renaming() {
    let t = tempdir().unwrap();
    let mut d = AppData::new(t.path().to_path_buf());
    assert_eq!(d.position(Path::new("/v"), "Livro"), None);
    d.set_position(Path::new("/v"), "Livro", "b/p.tif", 3);
    d.set_position(Path::new("/w"), "Livro", "a.jpg", 1);
    d.save().unwrap();
    let mut c = AppData::load(t.path().to_path_buf()).unwrap();
    assert_eq!(c.position(Path::new("/v/"), "Livro"), Some(("b/p.tif".to_string(), 3)));
    assert_eq!(c.position(Path::new("/w"), "Livro"), Some(("a.jpg".to_string(), 1)));
    c.rename_source(Path::new("/v"), "Livro", "Livro 2");
    assert_eq!(c.position(Path::new("/v"), "Livro"), None);
    assert_eq!(c.position(Path::new("/v"), "Livro 2"), Some(("b/p.tif".to_string(), 3)));
    assert_eq!(c.position(Path::new("/w"), "Livro"), Some(("a.jpg".to_string(), 1)), "outro vault nao muda");
}

#[test]
fn file_old_no_positions_loads() {
    let t = tempdir().unwrap();
    std::fs::write(t.path().join("appdata.json"), r#"{"versao": 1, "fontes": {}}"#).unwrap();
    let d = AppData::load(t.path().to_path_buf()).unwrap();
    assert_eq!(d.position(Path::new("/v"), "Livro"), None);
}

#[test]
fn adjustment_neutral_not_leaves_input() {
    let t = tempdir().unwrap();
    let mut d = AppData::new(t.path().to_path_buf());
    let i = img("Livro", "unico-arquivo.jpg", 1);
    d.set_adjustments(&i, Adjustments { invert: true, ..Adjustments::default() });
    d.set_adjustments(&i, Adjustments::default());
    d.save().unwrap();
    let json = std::fs::read_to_string(t.path().join("appdata.json")).unwrap();
    assert!(!json.contains("unico-arquivo"));
}

#[test]
fn load_of_folder_empty_of_the_empty() {
    let t = tempdir().unwrap();
    let d = AppData::load(t.path().to_path_buf()).unwrap();
    assert!(d.known_vaults().is_empty());
    assert_eq!(d.last_vault(), None);
}

#[test]
fn json_corrupt_and_error() {
    let t = tempdir().unwrap();
    std::fs::write(t.path().join("appdata.json"), "{ nao e json").unwrap();
    assert!(matches!(AppData::load(t.path().to_path_buf()), Err(Error::Json(_))));
}

#[test]
fn version_unknown_and_error() {
    let t = tempdir().unwrap();
    std::fs::write(t.path().join("appdata.json"), r#"{"versao": 2}"#).unwrap();
    assert!(matches!(AppData::load(t.path().to_path_buf()), Err(Error::VersionNotSupported(2))));
}

#[test]
fn known_vaults_order_and_no_duplicates() {
    let mut d = AppData::new(PathBuf::from("/x"));
    d.set_last_vault(PathBuf::from("/a"));
    d.set_last_vault(PathBuf::from("/b"));
    d.set_last_vault(PathBuf::from("/a"));
    assert_eq!(d.known_vaults(), &[PathBuf::from("/a"), PathBuf::from("/b")]);
    assert_eq!(d.last_vault(), Some(Path::new("/a")));
}

#[test]
fn name_with_accent_space_and_two_points() {
    let t = tempdir().unwrap();
    let mut d = AppData::new(t.path().to_path_buf());
    let name = "Registro Paroquial: Óbitos 1850";
    d.set_source_path(Path::new("/v"), name, PathBuf::from("/a"));
    let i = img(name, "pág 1.jpg", 2);
    d.set_adjustments(&i, Adjustments { invert: true, ..Adjustments::default() });
    d.save().unwrap();
    let c = AppData::load(t.path().to_path_buf()).unwrap();
    assert_eq!(c.source_path(Path::new("/v"), name), Some(PathBuf::from("/a")));
    assert!(c.adjustments(&i).invert);
}

#[test]
fn vault_normalized_one_map_and_a_input() {
    let mut d = AppData::new(PathBuf::from("/x"));
    d.set_source_path(Path::new("/a/b/"), "L", PathBuf::from("/1"));
    for v in ["/a/b", "/a/./b", "/a/c/../b", "/a/b//"] {
        assert_eq!(d.source_path(Path::new(v), "L"), Some(PathBuf::from("/1")), "{v}");
        d.set_last_vault(PathBuf::from(v));
    }
    assert_eq!(d.known_vaults(), &[PathBuf::from("/a/b")]);
    assert_eq!(d.source_path(Path::new("/a/bb"), "L"), None);
    assert_eq!(d.source_path(Path::new("/a"), "L"), None);
}

#[cfg(unix)]
#[test]
fn vault_not_utf_8_not_collides() {
    use std::os::unix::ffi::OsStrExt;
    let a = PathBuf::from(std::ffi::OsStr::from_bytes(b"/v/\xff"));
    let b = PathBuf::from(std::ffi::OsStr::from_bytes(b"/v/\xfe"));
    let mut d = AppData::new(PathBuf::from("/x"));
    d.set_source_path(&a, "L", PathBuf::from("/1"));
    assert_eq!(d.source_path(&a, "L"), Some(PathBuf::from("/1")));
    assert_eq!(d.source_path(&b, "L"), None);
}

#[test]
fn rename_source_takes_path_and_adjustments() {
    let t = tempdir().unwrap();
    let mut d = AppData::new(t.path().to_path_buf());
    let v = Path::new("/v");
    d.set_source_path(v, "Livro", PathBuf::from("/a"));
    d.set_source_path(Path::new("/outro"), "Livro", PathBuf::from("/b"));
    let i = img("Livro", "p.jpg", 1);
    d.set_adjustments(&i, Adjustments { invert: true, ..Adjustments::default() });
    d.rename_source(v, "Livro", "Livro 3");
    assert_eq!(d.source_path(v, "Livro"), None);
    assert_eq!(d.source_path(v, "Livro 3"), Some(PathBuf::from("/a")));
    assert_eq!(d.source_path(Path::new("/outro"), "Livro"), Some(PathBuf::from("/b")), "outro vault intocado");
    assert!(d.adjustments(&img("Livro 3", "p.jpg", 1)).invert, "ajustes seguem o nome novo");
    assert!(d.adjustments(&i).invert, "copiados: outro vault com o mesmo nome continua com os seus");
    // fonte sem caminho nesta maquina: nada muda e nao entra em panico
    d.rename_source(v, "Nada", "Outra");
    assert_eq!(d.source_path(v, "Outra"), None);
}

#[test]
fn nan_with_rest_neutral_not_leaves_input() {
    let mut d = AppData::new(PathBuf::from("/x"));
    let i = img("L", "a.jpg", 1);
    d.set_adjustments(&i, Adjustments { brightness: f32::NAN, ..Adjustments::default() });
    d.set_adjustments(&i, Adjustments { contrast: f32::INFINITY, ..Adjustments::default() });
    assert!(d.data.image_adjustments.is_empty());
}

#[test]
fn nan_with_brightness_real_keeps_brightness_and_loads() {
    let t = tempdir().unwrap();
    let mut d = AppData::new(t.path().to_path_buf());
    let i = img("L", "a.jpg", 1);
    d.set_adjustments(&i, Adjustments { sharpen: f32::NAN, brightness: 0.2, ..Adjustments::default() });
    d.save().unwrap();
    let c = AppData::load(t.path().to_path_buf()).unwrap();
    let a = c.adjustments(&i);
    assert_eq!(a.brightness, 0.2);
    assert_eq!(a.sharpen, 0.0);
}

#[test]
fn app_data_old_no_color_highlight_uses_default() {
    let t = tempdir().unwrap();
    std::fs::write(t.path().join("appdata.json"), r#"{"versao": 1, "fontes": {}}"#).unwrap();
    let d = AppData::load(t.path().to_path_buf()).unwrap();
    assert_eq!(d.highlight_color(), "#ffc400");
}

#[test]
fn color_default_not_and_written_and_chosen_persists() {
    let t = tempdir().unwrap();
    let mut d = AppData::new(t.path().to_path_buf());
    d.save().unwrap();
    assert!(!std::fs::read_to_string(t.path().join("appdata.json")).unwrap().contains("cor_grifo"));
    d.set_color_highlight(PALETTE[3]).unwrap();
    d.save().unwrap();
    assert_eq!(AppData::load(t.path().to_path_buf()).unwrap().highlight_color(), PALETTE[3]);
}

#[test]
fn color_outside_of_the_palette_and_refused() {
    let mut d = AppData::new(PathBuf::from("/x"));
    assert!(d.set_color_highlight("#123456").is_err());
    assert_eq!(d.highlight_color(), "#ffc400");
}

#[test]
fn width_of_the_drawers_persists_and_file_old_opens_no_ela() {
    let t = tempdir().unwrap();
    std::fs::write(t.path().join(FILE), r#"{"versao":1}"#).unwrap();
    let mut d = AppData::load(t.path().to_path_buf()).unwrap();
    assert_eq!(d.drawers_width(), None);
    d.set_width_drawers(Some(410.0));
    d.save().unwrap();
    assert_eq!(AppData::load(t.path().to_path_buf()).unwrap().drawers_width(), Some(410.0));
    d.set_width_drawers(None);
    d.save().unwrap();
    let text = std::fs::read_to_string(t.path().join(FILE)).unwrap();
    assert!(!text.contains("largura_gavetas"), "{text}");
}

#[test]
fn recent_more_new_first_no_repeat_and_limited() {
    let mut d = AppData::new(PathBuf::from("/x"));
    let v = Path::new("/v");
    assert!(d.recent(v).is_empty());
    assert!(d.mark_recent(v, "A"));
    assert!(d.mark_recent(v, "B"));
    assert!(d.mark_recent(v, "A"));
    assert!(!d.mark_recent(v, "A"), "ja e a primeira: nada muda");
    assert_eq!(d.recent(v), ["A", "B"]);
    assert!(d.recent(Path::new("/outro")).is_empty());
    for i in 0..20 {
        d.mark_recent(v, &format!("F{i}"));
    }
    assert_eq!(d.recent(v).len(), 10);
    assert_eq!(d.recent(v)[0], "F19");
}

#[test]
fn reopen_last_source_starts_disabled_and_persists() {
    let t = tempdir().unwrap();
    let base = t.path().join("base");
    let mut d = AppData::new(base.clone());
    assert!(!d.reopen_last_source());
    d.set_reopen_last_source(true);
    d.save().unwrap();
    let mut c = AppData::load(base.clone()).unwrap();
    assert!(c.reopen_last_source());
    c.restore_settings();
    c.save().unwrap();
    let text = std::fs::read_to_string(base.join(FILE)).unwrap();
    assert!(!text.contains("reabrir_ultima_fonte"), "{text}");
}

#[test]
fn last_source_existing_skips_as_that_vanished() {
    let t = tempdir().unwrap();
    let vault = t.path().join("vault");
    let good = t.path().join("boa");
    std::fs::create_dir_all(&good).unwrap();
    let mut d = AppData::new(t.path().join("base"));
    d.set_source_path(&vault, "boa", good.clone());
    d.set_source_path(&vault, "sumiu", t.path().join("nao-existe"));
    d.mark_recent(&vault, "boa");
    d.mark_recent(&vault, "sumiu");
    d.mark_recent(&vault, "sem-caminho");
    assert_eq!(d.last_source_existing(&vault), Some(("boa".to_string(), good)));
    assert_eq!(d.last_source_existing(&t.path().join("outro-vault")), None);
}

#[test]
fn rhythm_of_the_quick_note_starts_normal_persists_and_file_old_opens_no_it() {
    let t = tempdir().unwrap();
    let base = t.path().join("base");
    let mut d = AppData::new(base.clone());
    assert_eq!(d.quick_note_rhythm(), QuickNoteRhythm::Normal);
    d.set_rhythm_quick_note(QuickNoteRhythm::Calm);
    d.save().unwrap();
    let text = std::fs::read_to_string(base.join(FILE)).unwrap();
    assert!(text.contains("\"note_rhythm\": \"calm\""), "{text}");
    let mut c = AppData::load(base.clone()).unwrap();
    assert_eq!(c.quick_note_rhythm(), QuickNoteRhythm::Calm);
    c.restore_settings();
    assert_eq!(c.quick_note_rhythm(), QuickNoteRhythm::Normal);
    c.save().unwrap();
    assert!(!std::fs::read_to_string(base.join(FILE)).unwrap().contains("ritmo_nota"));
    // Arquivo de antes do campo: abre no padrao.
    let old = t.path().join("velho");
    std::fs::create_dir_all(&old).unwrap();
    std::fs::write(old.join(FILE), "{\"versao\": 1}").unwrap();
    assert_eq!(AppData::load(old).unwrap().quick_note_rhythm(), QuickNoteRhythm::Normal);
}

#[test]
fn font_of_the_quick_note_starts_media_persists_and_restore_back_font_and_width() {
    let t = tempdir().unwrap();
    let base = t.path().join("base");
    let mut d = AppData::new(base.clone());
    assert_eq!(d.quick_note_font(), FontSize::Media);
    assert_eq!(FontSize::Media.pixels(), 14.0);
    assert!(FontSize::Small.pixels() < FontSize::Media.pixels());
    assert!(FontSize::Media.pixels() < FontSize::Large.pixels());
    d.set_font_quick_note(FontSize::Large);
    d.set_width_drawers(Some(460.0));
    d.set_tools_hidden(true);
    d.save().unwrap();
    let mut c = AppData::load(base.clone()).unwrap();
    assert_eq!(c.quick_note_font(), FontSize::Large);
    c.restore_settings();
    assert_eq!(c.quick_note_font(), FontSize::Media);
    assert_eq!(c.drawers_width(), None);
    assert!(c.hidden_tools(), "restaurar padroes nao mexe na barra de ferramentas");
    c.set_tools_hidden(false);
    c.save().unwrap();
    assert!(!std::fs::read_to_string(base.join(FILE)).unwrap().contains("letra_nota"));
}

#[test]
fn theme_starts_dark_persists_and_restore_back() {
    let t = tempdir().unwrap();
    let base = t.path().join("base");
    let mut d = AppData::new(base.clone());
    assert_eq!(d.theme(), Theme::Dark);
    d.set_theme(Theme::Sepia);
    d.save().unwrap();
    assert!(std::fs::read_to_string(base.join(FILE)).unwrap().contains("\"theme\": \"sepia\""));
    let mut c = AppData::load(base.clone()).unwrap();
    assert_eq!(c.theme(), Theme::Sepia);
    c.restore_settings();
    assert_eq!(c.theme(), Theme::Dark);
    c.save().unwrap();
    assert!(!std::fs::read_to_string(base.join(FILE)).unwrap().contains("\"tema\""));
}

#[test]
fn update_fields_default_and_roundtrip() {
    let t = tempdir().unwrap();
    let mut d = AppData::new(t.path().to_path_buf());
    assert!(d.auto_update_check());
    assert_eq!(d.last_update_check(), None);
    assert_eq!(d.skipped_version(), None);
    d.set_auto_update_check(false);
    d.set_last_update_check(1_700_000_000);
    let skipped = crate::update::Version::parse("v0.2.0").unwrap();
    d.set_skipped_version(Some(skipped));
    d.save().unwrap();
    let again = AppData::load(t.path().to_path_buf()).unwrap();
    assert!(!again.auto_update_check());
    assert_eq!(again.last_update_check(), Some(1_700_000_000));
    assert_eq!(again.skipped_version(), Some(skipped));
}

#[test]
fn old_file_without_update_fields_opens_with_check_on() {
    let t = tempdir().unwrap();
    std::fs::write(t.path().join("appdata.json"), r#"{"version":1}"#).unwrap();
    let d = AppData::load(t.path().to_path_buf()).unwrap();
    assert!(d.auto_update_check());
    assert_eq!(d.skipped_version(), None);
}

#[test]
fn restore_settings_turns_check_on_but_keeps_skipped() {
    let t = tempdir().unwrap();
    let mut d = AppData::new(t.path().to_path_buf());
    let skipped = crate::update::Version::parse("0.3.0").unwrap();
    d.set_auto_update_check(false);
    d.set_skipped_version(Some(skipped));
    d.set_last_update_check(5);
    d.restore_settings();
    assert!(d.auto_update_check());
    assert_eq!(d.skipped_version(), Some(skipped));
    assert_eq!(d.last_update_check(), Some(5));
}
