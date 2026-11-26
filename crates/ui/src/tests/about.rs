use super::*;

#[test]
fn shows_name_version_author_and_right() {
    let c = content();
    assert_eq!(c.name, "Marginalia");
    assert_eq!(c.version, format!("Versão {}", env!("CARGO_PKG_VERSION")));
    assert_eq!(c.developer, "Desenvolvido por Wesley Alcoforado");
    assert_eq!(c.right, "© 2026 Wesley Alcoforado. Todos os direitos reservados.");
    assert_eq!(c.link_label, "Genealogia do Ceará");
    assert!(!c.description.is_empty() && !c.title.is_empty());
}

#[test]
fn the_site_and_the_of_the_genealogy_of_the_ceara_in_https() {
    assert_eq!(SITE, "https://www.genealogiadoceara.net");
    assert!(SITE.starts_with("https://") && SITE.ends_with("genealogiadoceara.net"));
}

#[test]
fn none_marker_left_over_in_the_texts() {
    let c = content();
    for text in [&c.title, &c.name, &c.version, &c.description, &c.developer, &c.link_label, &c.right] {
        assert!(!text.contains('{') && !text.contains('}'), "{text:?}");
    }
}
