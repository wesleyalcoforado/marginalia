use super::*;

#[test]
fn starts_in_zero_and_each_new_sum_one() {
    let mut g = Generation::default();
    assert_eq!(g.current(), 0);
    assert_eq!(g.fresh(), 1);
    assert_eq!(g.fresh(), 2);
    assert_eq!(g.current(), 2);
}

#[test]
fn the_marker_of_outside_follows_the_generation_and_mark_the_old_as_stale() {
    let mut g = Generation::default();
    let old = g.fresh();
    // O que a tarefa de fundo guardou ao ser pedida: o marcador e o numero do pedido.
    let marker = g.effective();
    assert_eq!(marker.load(Ordering::SeqCst), old);

    let fresh = g.fresh();
    assert_ne!(marker.load(Ordering::SeqCst), old, "a tarefa velha deve ver que perdeu a vez");
    assert_eq!(marker.load(Ordering::SeqCst), fresh);
}
