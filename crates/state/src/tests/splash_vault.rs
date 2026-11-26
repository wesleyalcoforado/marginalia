use super::*;

#[test]
fn no_vault_in_the_line_of_command_nor_in_the_environment_shows_the_screen() {
    assert!(screen_needs(None, None));
}

#[test]
fn vault_empty_counts_as_not_given() {
    assert!(screen_needs(Some(Path::new("")), Some(Path::new(""))));
}

#[test]
fn vault_in_the_line_of_command_or_in_the_environment_opens_direct() {
    assert!(!screen_needs(Some(Path::new("/v")), None));
    assert!(!screen_needs(None, Some(Path::new("/v"))));
}

#[test]
fn items_follow_the_order_of_the_data_of_the_app_and_mark_the_that_not_exists_more() {
    let known_all = vec![PathBuf::from("/nas/genealogia"), PathBuf::from("/local/outro")];
    let v = items(&known_all, |p| p == Path::new("/local/outro"));
    assert_eq!(v.len(), 2);
    assert_eq!(v[0].label, "genealogia");
    assert_eq!(v[0].file_path, PathBuf::from("/nas/genealogia"));
    assert!(!v[0].exists);
    assert_eq!(v[1].label, "outro");
    assert!(v[1].exists);
}

#[test]
fn no_vaults_known_the_list_and_empty() {
    assert!(items(&[], |_| true).is_empty());
}

#[test]
fn so_chooses_the_that_exists() {
    let v = items(&[PathBuf::from("/a"), PathBuf::from("/b")], |p| p == Path::new("/b"));
    assert_eq!(choose(&v, 0), None);
    assert_eq!(choose(&v, 1), Some(PathBuf::from("/b")));
    assert_eq!(choose(&v, 7), None);
}
