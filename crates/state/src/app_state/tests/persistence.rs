use super::*;

#[test]
fn queue_numbers_and_so_leaves_of_be_pending_when_writes() {
    let t = tempfile::tempdir().unwrap();
    let mut queue = WriteQueue::default();
    assert_eq!((queue.assembled(), queue.pending()), (0, false));

    let old = queue.build(AppData::new(t.path().join("appdata")));
    let fresh = queue.build(AppData::new(t.path().join("appdata")));
    assert_eq!((queue.assembled(), queue.pending()), (2, true));

    // A mais nova grava e quita a fila; a mais velha, que termina depois, nao regrava.
    fresh.execute().unwrap();
    assert!(!queue.pending());
    std::fs::remove_file(t.path().join("appdata/appdata.json")).unwrap();
    old.execute().unwrap();
    assert!(!t.path().join("appdata/appdata.json").exists(), "a gravacao velha nao devia regravar");
    assert!(!queue.pending());
}
