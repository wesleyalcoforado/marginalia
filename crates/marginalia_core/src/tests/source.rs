use super::*;

#[test]
fn input_above_of_the_limit_and_error_and_size_declared_not_allocates() {
    let data = vec![7u8; 100];
    // Cabe: le tudo, mesmo com tamanho declarado absurdo (cabecalho forjado).
    let v = input_read(&data[..], u64::MAX, 100, "a.jpg").unwrap();
    assert_eq!(v.len(), 100);
    assert!(v.capacity() <= RESERVE_MAX);
    // Passa do limite: erro, sem ler o resto.
    let e = input_read(&data[..], 10, 50, "a.jpg").unwrap_err();
    assert!(matches!(e, Error::InputLarge(ref p) if p == "a.jpg"), "{e}");
}
