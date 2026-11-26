use super::*;
use crate::load::GridRegion;

fn img(file: &str) -> ImageRef {
    ImageRef { source: "Livro".into(), file: file.into(), page: 1 }
}

fn brightness(b: f32) -> Adjustments {
    Adjustments { brightness: b, ..Adjustments::default() }
}

fn ms(t0: Instant, n: u64) -> Instant {
    t0 + Duration::from_millis(n)
}

/// Janela de 300x200 na grade 1000x800, a partir de `x0`.
fn jan(x0: u32) -> ViewWindow {
    ViewWindow { grid: (1000, 800), region: GridRegion { x0, y0: 0, x1: x0 + 300, y1: 200 } }
}

/// Agenda na imagem `p01.png`, sem ajuste, com a janela `jan(500)` ja informada
/// (sem janela o tile filtrado espera a primeira: teste proprio).
fn schedule() -> ScheduleAdjustment {
    let mut a = ScheduleAdjustment::new();
    assert!(a.switched_image(img("p01.png"), Adjustments::default()).is_empty());
    assert!(!a.window_changed(jan(500), Instant::now(), false));
    a
}

fn tiles(actions: &[MenuAction]) -> Vec<(u64, Adjustments, Option<ViewWindow>)> {
    actions
        .iter()
        .filter_map(|a| match a {
            MenuAction::RequestTile { work, image_adjustments, view_window } => Some((*work, image_adjustments.clone(), *view_window)),
            _ => None,
        })
        .collect()
}

fn bases(actions: &[MenuAction]) -> Vec<(u64, Adjustments)> {
    actions
        .iter()
        .filter_map(|a| match a {
            MenuAction::RequestBase { work, image_adjustments } => Some((*work, image_adjustments.clone())),
            _ => None,
        })
        .collect()
}

fn values<T>(v: &[(u64, Adjustments, T)]) -> Vec<Adjustments> {
    v.iter().map(|x| x.1.clone()).collect()
}

fn writes(actions: &[MenuAction]) -> Vec<(ImageRef, Adjustments)> {
    actions
        .iter()
        .filter_map(|a| match a {
            MenuAction::Write { image, image_adjustments } => Some((image.clone(), image_adjustments.clone())),
            _ => None,
        })
        .collect()
}

fn interruptions(actions: &[MenuAction]) -> Vec<u64> {
    actions
        .iter()
        .filter_map(|a| match a {
            MenuAction::Interrupt { work } => Some(*work),
            _ => None,
        })
        .collect()
}

/// Agenda com o tile de `value` pronto e a base dele em voo. Devolve o trabalho da base.
fn with_base_in_flight(a: &mut ScheduleAdjustment, value: Adjustments, t: Instant) -> u64 {
    assert!(a.changed(value.clone(), t));
    a.dropped(t);
    let ti = tiles(&a.tick(t));
    a.finished(ti[0].0);
    let ba = bases(&a.tick(t));
    assert_eq!(ba.iter().map(|b| b.1.clone()).collect::<Vec<_>>(), vec![value]);
    ba[0].0
}

#[test]
fn ten_changes_one_tile_in_flight_the_last_value_a_base_after_of_the_tile_and_a_write() {
    let t0 = Instant::now();
    let mut a = schedule();
    let mut all = Vec::new();
    for i in 1..=10u64 {
        assert!(a.changed(brightness(i as f32 / 100.0), ms(t0, i * 10)));
        all.extend(a.tick(ms(t0, i * 10)));
    }
    // o 1o tile saiu na hora; as outras 9 mudancas NAO o interrompem (R2: o
    // arrasto congelaria): so trocam o pendente
    let t1 = tiles(&all);
    assert_eq!(values(&t1), vec![brightness(0.01)], "um tile em voo: {all:?}");
    assert!(interruptions(&all).is_empty(), "valor novo nao interrompe o tile: {all:?}");
    assert!(bases(&all).is_empty() && writes(&all).is_empty());
    assert!(a.tile_in_flight(), "o tile continua em voo");
    assert!(!a.idle());
    // mesmo vencida a base (200 ms), nada sai com o tile ainda em voo
    assert!(a.tick(ms(t0, 350)).is_empty());
    assert_eq!(a.next_tick(ms(t0, 350)), Some(Duration::from_millis(250)), "so a gravacao tem prazo");

    // o tile (de 0.01) terminou: nao conta para a base de 0.10; o proximo leva
    // so o ULTIMO valor e a base espera ESTE tile
    a.finished(t1[0].0);
    let actions = a.tick(ms(t0, 360));
    let t2 = tiles(&actions);
    assert_eq!(values(&t2), vec![brightness(0.10)]);
    assert!(t2[0].0 > t1[0].0, "numero de trabalho novo");
    assert!(bases(&actions).is_empty(), "a base espera o tile do valor");
    assert!(a.tile_in_flight());
    assert!(a.tick(ms(t0, 370)).is_empty());

    a.finished(t2[0].0);
    assert_eq!(a.next_tick(ms(t0, 380)), Some(Duration::ZERO), "a base esta vencida");
    let actions = a.tick(ms(t0, 380));
    assert_eq!(bases(&actions).iter().map(|b| b.1.clone()).collect::<Vec<_>>(), vec![brightness(0.10)], "{actions:?}");
    assert!(tiles(&actions).is_empty(), "o ultimo valor ja teve tile");
    let ba = bases(&actions)[0].0;
    assert!(!a.tile_in_flight(), "a base em voo nao e tile");
    assert!(a.tick(ms(t0, 390)).is_empty(), "nada repetido");

    // gravacao: 500 ms depois da ULTIMA mudanca (100 ms), uma so vez
    assert!(writes(&a.tick(ms(t0, 599))).is_empty());
    assert_eq!(writes(&a.tick(ms(t0, 600))), vec![(img("p01.png"), brightness(0.10))]);
    assert!(a.tick(ms(t0, 2000)).is_empty(), "exatamente uma gravacao, uma base");
    assert!(!a.idle(), "a base ainda em voo");
    a.finished(ba);
    assert!(a.idle());
    assert_eq!(a.next_tick(ms(t0, 2000)), None);
    assert!(a.tick(ms(t0, 5000)).is_empty());
}

/// A ordem que impede a base borrada de piscar no arrasto: a base de um valor
/// so sai depois de um tile DESSE valor terminar sem interrupcao, mesmo com
/// a base vencida (soltou) e o lugar livre.
#[test]
fn base_so_leaves_after_of_the_tile_of_the_value_finish() {
    let t0 = Instant::now();
    let mut a = schedule();
    // uma janela ja informada (a primeira da imagem nao espera os 60 ms)
    assert!(!a.window_changed(jan(0), t0, false));
    assert!(a.changed(brightness(0.1), t0));
    a.dropped(t0);
    let actions = a.tick(t0);
    let t1 = tiles(&actions);
    assert_eq!(values(&t1), vec![brightness(0.1)], "o tile vem primeiro");
    assert!(bases(&actions).is_empty());
    assert_eq!(writes(&actions).len(), 1);
    assert_eq!(a.next_tick(t0), None, "a base espera o tile em voo");
    // valor novo antes do tile terminar: o tile de 0.1 segue, mas nao conta para 0.2
    assert!(a.changed(brightness(0.2), ms(t0, 1)));
    a.dropped(ms(t0, 1));
    assert!(interruptions(&a.tick(ms(t0, 1))).is_empty());
    a.finished(t1[0].0);
    let actions = a.tick(ms(t0, 2));
    assert!(bases(&actions).is_empty(), "o tile de outro valor nao libera a base: {actions:?}");
    let t2 = tiles(&actions);
    assert_eq!(values(&t2), vec![brightness(0.2)]);
    // janela nova com o tile em voo: interrompido, a base continua esperando
    assert!(a.window_changed(jan(10), ms(t0, 3), true));
    assert_eq!(interruptions(&a.tick(ms(t0, 3))), vec![t2[0].0]);
    a.finished(t2[0].0);
    assert!(a.tick(ms(t0, 4)).is_empty(), "nem base nem tile antes dos 60 ms da janela");
    let actions = a.tick(ms(t0, 63));
    let t3 = tiles(&actions);
    assert_eq!(t3.iter().map(|t| (t.1.clone(), t.2)).collect::<Vec<_>>(), vec![(brightness(0.2), Some(jan(10)))]);
    assert!(bases(&actions).is_empty());
    a.finished(t3[0].0);
    assert_eq!(bases(&a.tick(ms(t0, 64))).iter().map(|b| b.1.clone()).collect::<Vec<_>>(), vec![brightness(0.2)]);
}

#[test]
fn base_leaves_200_ms_after_of_the_last_change() {
    let t0 = Instant::now();
    let mut a = schedule();
    a.changed(brightness(0.2), t0);
    let ti = tiles(&a.tick(t0));
    a.finished(ti[0].0);
    assert!(a.tick(ms(t0, 50)).is_empty());
    assert_eq!(a.next_tick(ms(t0, 50)), Some(Duration::from_millis(150)));
    assert!(bases(&a.tick(ms(t0, 199))).is_empty());
    assert_eq!(bases(&a.tick(ms(t0, 200))), vec![(ti[0].0 + 1, brightness(0.2))]);
}

#[test]
fn dropped_anticipates_base_and_write() {
    let t0 = Instant::now();
    let mut a = schedule();
    a.changed(brightness(0.3), t0);
    let ti = tiles(&a.tick(t0));
    a.dropped(ms(t0, 20));
    // gravacao sai ja; a base espera so o tile em voo
    let actions = a.tick(ms(t0, 20));
    assert_eq!(writes(&actions), vec![(img("p01.png"), brightness(0.3))]);
    assert!(bases(&actions).is_empty());
    a.finished(ti[0].0);
    assert_eq!(a.next_tick(ms(t0, 30)), Some(Duration::ZERO), "a base esta vencida");
    let actions = a.tick(ms(t0, 30));
    assert_eq!(bases(&actions).len(), 1);
    assert!(writes(&actions).is_empty(), "gravou uma vez so");
}

#[test]
fn step_of_key_and_changed_and_dropped_tile_base_and_write_no_wait() {
    let t0 = Instant::now();
    let mut a = schedule();
    assert!(a.changed(brightness(0.05), t0));
    a.dropped(t0);
    let actions = a.tick(t0);
    assert_eq!(tiles(&actions).len(), 1, "o tile vem primeiro (resposta rapida)");
    assert_eq!(writes(&actions).len(), 1);
    a.finished(tiles(&actions)[0].0);
    assert_eq!(bases(&a.tick(t0)).len(), 1, "a base logo depois, sem esperar 200 ms");
}

#[test]
fn adjustment_neutral_asks_so_the_tile() {
    let t0 = Instant::now();
    let mut a = ScheduleAdjustment::new();
    a.switched_image(img("p01.png"), brightness(0.4));
    assert!(a.changed(Adjustments::default(), t0));
    let actions = a.tick(t0);
    assert_eq!(values(&tiles(&actions)), vec![Adjustments::default()], "{actions:?}");
    a.finished(tiles(&actions)[0].0);
    a.dropped(ms(t0, 1));
    let actions = a.tick(ms(t0, 1000));
    assert!(bases(&actions).is_empty(), "sem ajuste nao ha base: {actions:?}");
    assert_eq!(writes(&actions).len(), 1);
    assert!(a.idle());
}

/// Pan/zoom continuos: 20 janelas seguidas nao enfileiram pedidos nem gravam;
/// o tile sai 60 ms depois da ultima, com ela.
#[test]
fn twenty_windows_fast_one_tile_of_the_last_after_of_60_ms() {
    let t0 = Instant::now();
    let mut a = ScheduleAdjustment::new();
    a.switched_image(img("p01.png"), brightness(0.3));
    // a imagem ja tinha uma janela (a primeira sai sem espera: teste proprio)
    assert!(!a.window_changed(jan(500), t0, false));
    let mut all = Vec::new();
    for i in 0..20u64 {
        assert!(a.window_changed(jan(i as u32 * 5), ms(t0, i * 2), true));
        all.extend(a.tick(ms(t0, i * 2)));
    }
    assert!(all.is_empty(), "nada antes de a janela parar: {all:?}");
    assert!(a.tick(ms(t0, 59)).is_empty(), "o primeiro so depois de 60 ms");
    assert_eq!(a.next_tick(ms(t0, 38)), Some(Duration::from_millis(60)));
    let actions = a.tick(ms(t0, 98));
    let t1 = tiles(&actions);
    assert_eq!(t1.iter().map(|t| (t.1.clone(), t.2)).collect::<Vec<_>>(), vec![(brightness(0.3), Some(jan(95)))]);
    // mais 20 com o tile em voo: uma interrupcao, nenhum pedido ate ele terminar
    let mut all = Vec::new();
    for i in 0..20u64 {
        assert!(a.window_changed(jan(200 + i as u32), ms(t0, 100 + i), true));
        all.extend(a.tick(ms(t0, 100 + i)));
    }
    assert_eq!(interruptions(&all), vec![t1[0].0], "{all:?}");
    assert!(tiles(&all).is_empty());
    a.finished(t1[0].0);
    assert!(a.tick(ms(t0, 178)).is_empty(), "60 ms depois da ULTIMA janela (119 ms)");
    let t2 = tiles(&a.tick(ms(t0, 179)));
    assert_eq!(t2.iter().map(|t| t.2).collect::<Vec<_>>(), vec![Some(jan(219))]);
    a.finished(t2[0].0);
    // a imagem ja tinha o ajuste: a base dela vem depois do primeiro tile pronto
    let actions = a.tick(ms(t0, 180));
    assert_eq!(bases(&actions).len(), 1, "{actions:?}");
    a.finished(bases(&actions)[0].0);
    assert!(a.tick(ms(t0, 10_000)).is_empty(), "janelas nunca gravam");
    assert!(a.idle());
}

#[test]
fn window_equal_the_current_not_generates_work() {
    let t0 = Instant::now();
    let mut a = schedule();
    a.changed(brightness(0.2), t0);
    a.dropped(t0);
    let ti = tiles(&a.tick(t0));
    assert_eq!(ti[0].2, Some(jan(500)), "a janela ja informada");
    a.finished(ti[0].0);
    let ba = bases(&a.tick(t0))[0].0;
    a.finished(ba);
    assert!(a.window_changed(jan(0), ms(t0, 10), true));
    let ti = tiles(&a.tick(ms(t0, 70)));
    assert_eq!(ti[0].2, Some(jan(0)));
    assert!(!a.window_changed(jan(0), ms(t0, 71), true), "a mesma janela: nada");
    assert!(a.tick(ms(t0, 71)).is_empty(), "o tile em voo nao e interrompido");
    a.finished(ti[0].0);
    assert!(!a.window_changed(jan(0), ms(t0, 80), true));
    assert!(a.tick(ms(t0, 1000)).is_empty());
    assert!(a.idle());
}

#[test]
fn tile_of_the_window_old_and_interrupted_and_request_of_new_but_the_base_in_flight_follows() {
    let t0 = Instant::now();
    let mut a = schedule();
    // tile em voo da janela 0 (guardada antes, sem ajuste na tela)
    assert!(!a.window_changed(jan(0), t0, false));
    a.changed(brightness(0.2), t0);
    let ti = tiles(&a.tick(t0));
    assert_eq!(ti[0].2, Some(jan(0)), "o tile do valor ja usa a janela guardada");
    assert!(a.window_changed(jan(50), ms(t0, 5), true));
    assert_eq!(interruptions(&a.tick(ms(t0, 5))), vec![ti[0].0]);
    a.finished(ti[0].0 + 7);
    assert!(a.tick(ms(t0, 100)).is_empty(), "o lugar so vaga pelo terminou do trabalho certo");
    a.finished(ti[0].0);
    let t2 = tiles(&a.tick(ms(t0, 100)));
    assert_eq!(t2.iter().map(|t| t.2).collect::<Vec<_>>(), vec![Some(jan(50))]);
    a.finished(t2[0].0);
    // base em voo: a janela nova nao a interrompe; o tile sai quando ela terminar
    let ba = bases(&a.tick(ms(t0, 300)))[0].0;
    assert!(a.window_changed(jan(80), ms(t0, 301), true));
    assert!(interruptions(&a.tick(ms(t0, 301))).is_empty(), "a base nao depende da janela");
    assert!(a.tick(ms(t0, 400)).is_empty(), "um filtro em voo por vez");
    a.finished(ba);
    assert_eq!(tiles(&a.tick(ms(t0, 400))).iter().map(|t| t.2).collect::<Vec<_>>(), vec![Some(jan(80))]);
}

#[test]
fn window_no_adjustment_in_the_screen_so_and_stored() {
    let t0 = Instant::now();
    let mut a = schedule();
    assert!(!a.window_changed(jan(0), t0, true), "ajuste neutro: nada a filtrar");
    assert!(a.tick(ms(t0, 1000)).is_empty());
    let mut b = ScheduleAdjustment::new();
    b.switched_image(img("p01.png"), brightness(0.3));
    assert!(!b.window_changed(jan(10), t0, false), "olho desligado: nada a filtrar");
    assert!(b.tick(ms(t0, 1000)).is_empty() && b.idle());
    // o olho volta: o tile usa a janela guardada
    assert!(b.refilter(ms(t0, 1000), true));
    assert_eq!(tiles(&b.tick(ms(t0, 1000)))[0].2, Some(jan(10)));
}

#[test]
fn tile_that_fell_by_the_window_and_request_of_new_already() {
    let t0 = Instant::now();
    let mut a = schedule();
    a.window_changed(jan(0), t0, false);
    a.changed(brightness(0.2), t0);
    a.dropped(t0);
    let ti = tiles(&a.tick(t0))[0].0;
    // o estado derrubou o tile (janela velha); o fim dele vem logo depois
    a.fell_tile(ms(t0, 1), Some(ti));
    a.finished(ti);
    let actions = a.tick(ms(t0, 1));
    assert_eq!(tiles(&actions).iter().map(|t| t.2).collect::<Vec<_>>(), vec![Some(jan(0))], "sem esperar 60 ms");
    assert!(bases(&actions).is_empty(), "o tile que caiu nao libera a base");
    let t2 = tiles(&actions)[0].0;
    // o aviso de outro trabalho (velho) nao derruba o tile em voo
    a.fell_tile(ms(t0, 2), Some(ti));
    a.finished(t2);
    let actions = a.tick(ms(t0, 2));
    assert!(tiles(&actions).is_empty(), "o tile pronto do valor e da janela atuais basta: {actions:?}");
    assert_eq!(bases(&actions).len(), 1);
}

/// A carga do quadro (navegacao, que a agenda nao pediu) tambem volta como
/// janela velha: ela so adianta o pedido do tile, nunca marca o tile em voo
/// da agenda como derrubado.
#[test]
fn frame_of_the_navigation_that_falls_not_breaks_the_tile_in_flight_of_the_schedule() {
    let t0 = Instant::now();
    let mut a = schedule();
    a.window_changed(jan(0), t0, false);
    a.changed(brightness(0.2), t0);
    a.dropped(t0);
    let ti = tiles(&a.tick(t0))[0].0;
    a.fell_tile(ms(t0, 1), None);
    assert!(a.tile_in_flight(), "o tile da agenda segue valendo");
    a.finished(ti);
    let actions = a.tick(ms(t0, 2));
    assert!(tiles(&actions).is_empty(), "o tile da agenda chegou com o valor e a janela atuais: {actions:?}");
    assert_eq!(bases(&actions).len(), 1, "e libera a base");
    // sem nada em voo, a carga que caiu pede o tile da janela vigente ja
    a.finished(bases(&actions)[0].0);
    a.fell_tile(ms(t0, 3), None);
    assert_eq!(tiles(&a.tick(ms(t0, 3))).iter().map(|t| t.2).collect::<Vec<_>>(), vec![Some(jan(0))]);
}

/// A -> B -> A com o tile de A em voo: quando ele termina, o valor e a janela
/// sao os dele; nao se refiltra o mesmo tile.
#[test]
fn the_b_the_with_the_tile_of_the_in_flight_not_refilters_the_same_tile() {
    let t0 = Instant::now();
    let mut a = schedule();
    a.changed(brightness(0.1), t0);
    let ti = tiles(&a.tick(t0))[0].0;
    assert!(a.changed(brightness(0.2), ms(t0, 1)));
    assert!(a.changed(brightness(0.1), ms(t0, 2)));
    a.finished(ti);
    let actions = a.tick(ms(t0, 3));
    assert!(actions.is_empty(), "nada de tile repetido: {actions:?}");
    assert_eq!(a.next_tick(ms(t0, 3)), Some(Duration::from_millis(199)), "so a base (200 ms depois do ultimo valor)");
    assert_eq!(bases(&a.tick(ms(t0, 202))).len(), 1);
    // com a janela mudada no meio (sem ajuste na tela nao interrompe), o tile sai de novo
    let mut b = schedule();
    b.changed(brightness(0.1), t0);
    let ti = tiles(&b.tick(t0))[0].0;
    b.changed(brightness(0.2), ms(t0, 1));
    b.changed(brightness(0.1), ms(t0, 2));
    assert!(!b.window_changed(jan(40), ms(t0, 2), false));
    b.finished(ti);
    assert_eq!(tiles(&b.tick(ms(t0, 3))).iter().map(|t| t.2).collect::<Vec<_>>(), vec![Some(jan(40))]);
}

/// O olho religado pelo `changed` (mexer num ajuste com o olho desligado): o
/// que esta em voo e a imagem sem filtro (textura cheia), entao o valor novo o
/// interrompe, mesmo sem passar pelo neutro. Com o olho ligado, um valor novo
/// nao interrompe o tile filtrado (R2).
#[test]
fn eye_relinked_by_the_value_new_interrupts_the_full_in_flight() {
    let t0 = Instant::now();
    let mut a = ScheduleAdjustment::new();
    a.switched_image(img("p01.png"), brightness(0.3));
    a.set_eye(false);
    assert!(!a.window_changed(jan(0), t0, false));
    assert!(a.refilter(t0, false));
    let full = tiles(&a.tick(t0))[0].0;
    a.set_eye(true);
    assert!(a.changed(brightness(0.4), ms(t0, 1)));
    assert_eq!(interruptions(&a.tick(ms(t0, 1))), vec![full], "a cheia sem filtro nao fica na tela");
    a.finished(full);
    let ti = tiles(&a.tick(ms(t0, 2)))[0].0;
    assert!(a.changed(brightness(0.5), ms(t0, 3)));
    assert!(interruptions(&a.tick(ms(t0, 3))).is_empty(), "tile filtrado em voo: R2");
    a.finished(ti);
    // olho desligado e valor novo: a tela segue sem filtro, nada a interromper
    let mut b = ScheduleAdjustment::new();
    b.switched_image(img("p01.png"), brightness(0.3));
    b.set_eye(false);
    b.refilter(t0, false);
    b.tick(t0);
    b.changed(brightness(0.4), ms(t0, 1));
    assert!(interruptions(&b.tick(ms(t0, 1))).is_empty());
}

/// A janela que a view guardou antes de a pagina decodificar e grampeada pelo
/// estado quando ela chega: a agenda passa a guardar a normalizada, e a mesma
/// janela informada de novo nao gera um tile a mais.
#[test]
fn window_normalized_by_the_state_replaces_the_stored() {
    let t0 = Instant::now();
    let mut a = ScheduleAdjustment::new();
    a.switched_image(img("p01.png"), brightness(0.3));
    let raw = ViewWindow { grid: (2000, 800), region: GridRegion { x0: 0, y0: 0, x1: 1500, y1: 200 } };
    assert!(a.window_changed(raw, t0, true));
    let ti = tiles(&a.tick(ms(t0, 60)));
    assert_eq!(ti[0].2, Some(raw));
    let normal = ViewWindow { grid: (1000, 800), region: GridRegion { x0: 0, y0: 0, x1: 1000, y1: 200 } };
    a.normalized_window(Some(normal));
    assert!(!a.window_changed(normal, ms(t0, 61), true), "a mesma janela, agora normalizada: nada");
    a.finished(ti[0].0);
    let actions = a.tick(ms(t0, 62));
    assert!(tiles(&actions).is_empty(), "o tile em voo era desta janela: {actions:?}");
    assert_eq!(bases(&actions).len(), 1);
}

/// O olho do painel (antes/depois): refiltrar o valor atual sem mudar nada,
/// pelas mesmas regras (um filtro em voo; tile e base; nunca grava).
#[test]
fn refilter_asks_tile_and_base_of_the_value_current_no_write() {
    let t0 = Instant::now();
    let mut no_image = ScheduleAdjustment::new();
    assert!(!no_image.refilter(t0, true), "sem imagem: nada");
    let mut a = ScheduleAdjustment::new();
    a.switched_image(img("p01.png"), brightness(0.3));
    assert!(!a.window_changed(jan(0), t0, false));
    assert!(a.refilter(t0, true));
    let actions = a.tick(t0);
    assert_eq!(values(&tiles(&actions)), vec![brightness(0.3)]);
    assert!(bases(&actions).is_empty(), "a base espera o tile em voo");
    a.finished(tiles(&actions)[0].0);
    let actions = a.tick(ms(t0, 1));
    assert_eq!(bases(&actions).iter().map(|b| b.1.clone()).collect::<Vec<_>>(), vec![brightness(0.3)], "a base logo depois");
    a.finished(bases(&actions)[0].0);
    assert!(a.idle());
    assert!(writes(&a.tick(ms(t0, 5000))).is_empty(), "o olho nunca grava");
    // olho desligado: so o tile (a imagem sem ajustes), sem base
    assert!(a.refilter(ms(t0, 6000), false));
    let actions = a.tick(ms(t0, 6000));
    assert_eq!(tiles(&actions).len(), 1);
    let ti = tiles(&actions)[0].0;
    a.finished(ti);
    assert!(a.tick(ms(t0, 7000)).is_empty() && a.idle());
    // com um filtro em voo, ele e interrompido e o pedido espera ele terminar
    assert!(a.refilter(ms(t0, 7001), true));
    let ti = tiles(&a.tick(ms(t0, 7001)))[0].0;
    assert!(a.refilter(ms(t0, 7002), true));
    assert_eq!(a.tick(ms(t0, 7002)), vec![MenuAction::Interrupt { work: ti }], "um filtro em voo por vez");
    a.finished(ti);
    assert_eq!(tiles(&a.tick(ms(t0, 7003))).len(), 1);
}

#[test]
fn swap_of_image_in_the_middle_discards_pending_and_writes_in_the_image_right() {
    let t0 = Instant::now();
    let mut a = schedule();
    a.changed(brightness(0.1), t0);
    let ti = tiles(&a.tick(t0));
    a.changed(brightness(0.2), ms(t0, 10)); // pendente: tile e base de 0.2
    a.window_changed(jan(30), ms(t0, 11), true);
    let actions = a.switched_image(img("p02.png"), Adjustments::default());
    // a gravacao pendente sai ja, com a chave da p01; nada vai para a p02
    assert_eq!(writes(&actions), vec![(img("p01.png"), brightness(0.2))]);
    assert!(tiles(&actions).is_empty() && bases(&actions).is_empty());
    assert_eq!(a.image(), Some(&img("p02.png")));
    // o filtro da p01 em voo termina (obsoleto); depois nada pendente sai
    a.finished(ti[0].0);
    assert!(a.tick(ms(t0, 5000)).is_empty(), "nenhum tile/base/gravacao da p01 na p02");
    assert!(a.idle());
    // o valor guardado da p02 e o ponto de partida: igual a ele nao e trabalho
    assert!(!a.changed(Adjustments::default(), ms(t0, 5000)));
    // a janela da p01 nao vale na p02 (o tile espera a da p02); a gravacao
    // seguinte e da p02
    assert!(a.changed(brightness(0.8), ms(t0, 5000)));
    a.dropped(ms(t0, 5000));
    let actions = a.tick(ms(t0, 5000));
    assert_eq!(writes(&actions), vec![(img("p02.png"), brightness(0.8))]);
    assert!(tiles(&actions).is_empty(), "a janela da imagem anterior caiu: {actions:?}");
    assert!(a.window_changed(jan(30), ms(t0, 5001), true));
    assert_eq!(tiles(&a.tick(ms(t0, 5001))).iter().map(|t| t.2).collect::<Vec<_>>(), vec![Some(jan(30))]);
}

#[test]
fn swap_of_image_wait_the_filter_old_finish() {
    let t0 = Instant::now();
    let mut a = schedule();
    a.changed(brightness(0.1), t0);
    let ti = tiles(&a.tick(t0));
    let actions = a.switched_image(img("p02.png"), Adjustments::default());
    assert!(interruptions(&actions).is_empty(), "a troca de imagem ja invalida o filtro antigo (geracao nova)");
    a.changed(brightness(0.5), ms(t0, 10));
    let actions = a.tick(ms(t0, 10));
    assert!(tiles(&actions).is_empty() && interruptions(&actions).is_empty(), "um filtro em voo, mesmo de outra imagem: {actions:?}");
    a.finished(ti[0].0);
    assert!(a.window_changed(jan(0), ms(t0, 20), true));
    assert_eq!(values(&tiles(&a.tick(ms(t0, 20)))), vec![brightness(0.5)]);
}

#[test]
fn same_image_not_and_swap() {
    let t0 = Instant::now();
    let mut a = schedule();
    a.changed(brightness(0.1), t0);
    assert!(a.switched_image(img("p01.png"), Adjustments::default()).is_empty());
    assert_eq!(tiles(&a.tick(t0)).len(), 1, "o pendente continua");
}

#[test]
fn value_equal_to_the_last_not_generates_work() {
    let t0 = Instant::now();
    let mut a = schedule();
    assert!(!a.changed(Adjustments::default(), t0), "igual ao guardado");
    assert!(a.tick(t0).is_empty());
    assert!(a.idle());
    a.changed(brightness(0.1), t0);
    a.dropped(t0);
    let ti = tiles(&a.tick(t0));
    a.finished(ti[0].0);
    let actions = a.tick(t0);
    assert_eq!(bases(&actions).len(), 1);
    a.finished(bases(&actions)[0].0);
    // de novo o mesmo valor (tecla repetida no limite, barra parada): nada
    assert!(!a.changed(brightness(0.1), ms(t0, 10)));
    a.dropped(ms(t0, 10));
    assert!(a.tick(ms(t0, 1000)).is_empty());
    assert!(a.idle());
}

#[test]
fn back_to_the_value_already_shown_during_the_drag_still_asks_the_value_final() {
    // 0.1 -> 0.2 -> 0.1: o ultimo valor e o que fica, mesmo igual a um anterior
    let t0 = Instant::now();
    let mut a = schedule();
    a.changed(brightness(0.1), t0);
    let ti = tiles(&a.tick(t0));
    a.changed(brightness(0.2), ms(t0, 10));
    assert!(a.changed(brightness(0.1), ms(t0, 20)));
    a.dropped(ms(t0, 20));
    a.finished(ti[0].0);
    // o tile em voo ja era de 0.1 (e da janela atual): ele vale, sem refiltrar
    let actions = a.tick(ms(t0, 30));
    assert!(tiles(&actions).is_empty(), "{actions:?}");
    assert_eq!(bases(&actions).iter().map(|b| b.1.clone()).collect::<Vec<_>>(), vec![brightness(0.1)]);
    assert_eq!(writes(&actions), vec![(img("p01.png"), brightness(0.1))]);
}

#[test]
fn finished_of_other_work_not_releases_the_in_flight() {
    let t0 = Instant::now();
    let mut a = schedule();
    a.changed(brightness(0.1), t0);
    let ti = tiles(&a.tick(t0));
    a.changed(brightness(0.2), ms(t0, 10));
    a.finished(ti[0].0 + 99);
    assert!(tiles(&a.tick(ms(t0, 20))).is_empty(), "o filtro em voo continua");
    a.finished(ti[0].0);
    a.finished(ti[0].0); // repetido: nada
    assert_eq!(tiles(&a.tick(ms(t0, 20))).len(), 1);
}

#[test]
fn unload_returns_the_write_pending_a_time() {
    let t0 = Instant::now();
    let mut a = schedule();
    assert_eq!(a.unload(), None);
    a.changed(brightness(0.3), t0);
    let ti = tiles(&a.tick(t0));
    a.window_changed(jan(0), ms(t0, 1), true);
    assert_eq!(a.unload(), Some(MenuAction::Write { image: img("p01.png"), image_adjustments: brightness(0.3) }));
    assert_eq!(a.unload(), None);
    a.finished(ti[0].0);
    let actions = a.tick(ms(t0, 1000));
    assert!(tiles(&actions).is_empty() && bases(&actions).is_empty() && writes(&actions).is_empty(), "tile/base pendentes cairam: {actions:?}");
    assert!(a.idle());
}

#[test]
fn value_new_not_interrupts_the_tile_but_interrupts_the_base() {
    let t0 = Instant::now();
    let mut a = schedule();
    a.changed(brightness(0.1), t0);
    let ti = tiles(&a.tick(t0))[0].0;
    assert!(a.tile_in_flight());
    assert!(a.changed(brightness(0.2), ms(t0, 10)));
    assert!(a.changed(brightness(0.3), ms(t0, 11)));
    assert_eq!(a.next_tick(ms(t0, 11)), Some(Duration::from_millis(500)), "nada a interromper; so a gravacao tem prazo");
    let actions = a.tick(ms(t0, 11));
    assert!(actions.is_empty(), "o tile em voo segue (R2): {actions:?}");
    assert!(a.tile_in_flight());
    // o lugar so vaga quando o tile termina; o proximo leva so o ultimo valor
    a.finished(ti);
    let actions = a.tick(ms(t0, 30));
    assert_eq!(values(&tiles(&actions)), vec![brightness(0.3)]);
    assert!(interruptions(&actions).is_empty());
    // a base de um valor superado para
    let mut b = schedule();
    let ba = with_base_in_flight(&mut b, brightness(0.4), t0);
    assert!(b.changed(brightness(0.5), ms(t0, 1)));
    let actions = b.tick(ms(t0, 1));
    assert_eq!(interruptions(&actions), vec![ba]);
    assert!(tiles(&actions).is_empty());
    b.finished(ba);
    assert_eq!(values(&tiles(&b.tick(ms(t0, 2)))), vec![brightness(0.5)]);
}

/// Passar a neutro (ou sair do neutro) muda o tipo do que a tela pede (textura
/// cheia ou tile filtrado): o tile em voo e interrompido.
#[test]
fn transition_of_neutral_interrupts_the_tile_in_flight() {
    let t0 = Instant::now();
    let mut a = schedule();
    a.changed(brightness(0.1), t0);
    let ti = tiles(&a.tick(t0))[0].0;
    assert!(a.changed(Adjustments::default(), ms(t0, 1)));
    assert_eq!(interruptions(&a.tick(ms(t0, 1))), vec![ti]);
    a.finished(ti);
    let full = tiles(&a.tick(ms(t0, 2)));
    assert_eq!(values(&full), vec![Adjustments::default()]);
    assert!(a.changed(brightness(0.2), ms(t0, 3)));
    assert_eq!(interruptions(&a.tick(ms(t0, 3))), vec![full[0].0], "saindo do neutro tambem");
}

#[test]
fn value_equal_during_the_tile_not_interrupts() {
    let t0 = Instant::now();
    let mut a = schedule();
    a.changed(brightness(0.1), t0);
    a.tick(t0);
    assert!(!a.changed(brightness(0.1), ms(t0, 10)));
    assert!(interruptions(&a.tick(ms(t0, 10))).is_empty());
    assert!(a.tile_in_flight());
}

/// O olho muda o que a tela pede: o filtro em voo (tile ou base) leva o
/// estado velho do olho e e interrompido, uma vez; depois sai o pedido novo.
#[test]
fn eye_with_filter_in_flight_interrupts_it_and_refilters() {
    let t0 = Instant::now();
    let mut a = schedule();
    let ba = with_base_in_flight(&mut a, brightness(0.1), t0);
    assert!(a.refilter(ms(t0, 1), false));
    assert!(a.refilter(ms(t0, 2), true));
    let mut all = a.tick(ms(t0, 2));
    all.extend(a.tick(ms(t0, 3)));
    assert_eq!(interruptions(&all), vec![ba], "{all:?}");
    assert!(tiles(&all).is_empty(), "o lugar so vaga no terminou");
    a.finished(ba);
    let ti = tiles(&a.tick(ms(t0, 4)));
    assert_eq!(ti.len(), 1, "o pedido novo sai depois");
    a.finished(ti[0].0);
    assert_eq!(bases(&a.tick(ms(t0, 5))).len(), 1, "o ultimo estado do olho (ligado): a base");
    // nada em voo: o olho nao interrompe nada
    let mut b = schedule();
    assert!(b.refilter(t0, true));
    assert!(interruptions(&b.tick(t0)).is_empty());
}

fn renamed(file: &str) -> ImageRef {
    ImageRef { source: "Livro novo".into(), file: file.into(), page: 1 }
}

/// Renomear a fonte muda so a chave da imagem: a mesma imagem continua na
/// tela, entao a base pendente (e o filtro em voo) seguem valendo.
#[test]
fn rename_the_source_in_the_middle_keeps_base_and_write_pending() {
    let t0 = Instant::now();
    let mut a = schedule();
    assert!(!a.window_changed(jan(0), t0, true), "sem ajuste: so guardada");
    assert!(a.changed(brightness(0.2), t0));
    let ti = tiles(&a.tick(t0))[0].0;
    a.renamed_source("Outro", "Nada");
    assert_eq!(a.image(), Some(&img("p01.png")), "outra fonte: nada muda");
    a.renamed_source("Livro", "Livro novo");
    assert_eq!(a.image(), Some(&renamed("p01.png")));
    // a view segue o quadro (agora com a chave nova): nao e troca de imagem
    let actions = a.switched_image(renamed("p01.png"), brightness(0.2));
    assert!(actions.is_empty(), "nada cai nem sai antes da hora: {actions:?}");
    a.finished(ti);
    assert!(bases(&a.tick(ms(t0, 100))).is_empty());
    assert!(!a.tile_in_flight());
    assert!(bases(&a.tick(ms(t0, 199))).is_empty());
    let actions = a.tick(ms(t0, 200));
    assert_eq!(bases(&actions).iter().map(|b| b.1.clone()).collect::<Vec<_>>(), vec![brightness(0.2)], "{actions:?}");
    let ba = bases(&actions)[0].0;
    assert_eq!(writes(&a.tick(ms(t0, 500))), vec![(renamed("p01.png"), brightness(0.2))]);
    // a base em voo continua interrompivel por um valor novo
    assert!(a.changed(brightness(0.3), ms(t0, 600)));
    assert_eq!(interruptions(&a.tick(ms(t0, 600))), vec![ba]);
    // outra imagem de verdade continua descartando os pendentes
    let actions = a.switched_image(renamed("p02.png"), Adjustments::default());
    assert_eq!(writes(&actions), vec![(renamed("p01.png"), brightness(0.3))]);
    a.finished(ba);
    assert!(a.tick(ms(t0, 5000)).is_empty());
}

#[test]
fn ten_changes_during_the_tile_no_interruption_and_one_tile_and_a_base_of_the_last() {
    let t0 = Instant::now();
    let mut a = schedule();
    a.changed(brightness(0.1), t0);
    let ti = tiles(&a.tick(t0))[0].0;
    let mut all = Vec::new();
    for i in 1..=10u64 {
        assert!(a.changed(brightness(0.2 + i as f32 / 100.0), ms(t0, i)));
        all.extend(a.tick(ms(t0, i)));
    }
    assert!(interruptions(&all).is_empty(), "o tile em voo segue: {all:?}");
    assert!(tiles(&all).is_empty() && bases(&all).is_empty());
    a.finished(ti);
    a.dropped(ms(t0, 20));
    let actions = a.tick(ms(t0, 20));
    let t2 = tiles(&actions);
    assert_eq!(values(&t2), vec![brightness(0.30)]);
    a.finished(t2[0].0);
    let actions = a.tick(ms(t0, 21));
    assert_eq!(bases(&actions).iter().map(|b| b.1.clone()).collect::<Vec<_>>(), vec![brightness(0.30)]);
    a.finished(bases(&actions)[0].0);
    assert!(a.tick(ms(t0, 5000)).is_empty());
    assert!(a.idle());
}

/// A primeira janela informada numa imagem nova pede o tile ja (nenhum tile
/// dela na tela ainda: esperar 60 ms so atrasaria a imagem nitida); as
/// seguintes esperam a janela parar.
#[test]
fn first_tile_of_the_image_leaves_no_wait_60_ms() {
    let t0 = Instant::now();
    let mut a = ScheduleAdjustment::new();
    a.switched_image(img("p01.png"), brightness(0.3));
    assert!(a.window_changed(jan(0), t0, true));
    assert_eq!(a.next_tick(t0), Some(Duration::ZERO));
    let t1 = tiles(&a.tick(t0));
    assert_eq!(t1.iter().map(|t| t.2).collect::<Vec<_>>(), vec![Some(jan(0))], "sem esperar 60 ms");
    a.finished(t1[0].0);
    let ba = bases(&a.tick(ms(t0, 1)));
    assert_eq!(ba.len(), 1, "a base da imagem vem depois do primeiro tile");
    a.finished(ba[0].0);
    // a segunda janela da mesma imagem espera a janela parar
    assert!(a.window_changed(jan(10), ms(t0, 2), true));
    assert!(a.tick(ms(t0, 61)).is_empty());
    let t2 = tiles(&a.tick(ms(t0, 62)));
    assert_eq!(t2.iter().map(|t| t.2).collect::<Vec<_>>(), vec![Some(jan(10))]);
    a.finished(t2[0].0);
    // imagem nova: de novo a primeira janela
    a.switched_image(img("p02.png"), brightness(0.3));
    assert!(a.window_changed(jan(20), ms(t0, 100), true));
    let t3 = tiles(&a.tick(ms(t0, 100)));
    assert_eq!(t3.iter().map(|t| t.2).collect::<Vec<_>>(), vec![Some(jan(20))]);
    // mesmo com o filtro da imagem anterior em voo, sai assim que ele terminar
    let mut b = schedule();
    b.changed(brightness(0.2), t0);
    let old = tiles(&b.tick(t0))[0].0;
    b.switched_image(img("p02.png"), brightness(0.2));
    assert!(b.window_changed(jan(0), ms(t0, 1), true));
    assert!(tiles(&b.tick(ms(t0, 1))).is_empty(), "um filtro em voo por vez");
    b.finished(old);
    assert_eq!(tiles(&b.tick(ms(t0, 2))).len(), 1, "sem esperar os 60 ms");
}

/// A base que a carga do quadro trouxe ja e a do valor guardado: o primeiro
/// tile da imagem nao pede outra igual (base duplicada na abertura).
#[test]
fn base_of_the_opening_of_the_same_value_not_and_requested_of_new() {
    let t0 = Instant::now();
    let mut a = ScheduleAdjustment::new();
    a.switched_image(img("p01.png"), brightness(0.3));
    a.screen_base(&brightness(0.3));
    assert!(a.window_changed(jan(0), t0, true));
    let ti = tiles(&a.tick(t0));
    a.finished(ti[0].0);
    let actions = a.tick(ms(t0, 1));
    assert!(actions.is_empty(), "a base do valor ja esta na tela: {actions:?}");
    assert!(a.idle());
    // uma base de outro valor nao conta
    let mut b = ScheduleAdjustment::new();
    b.switched_image(img("p01.png"), brightness(0.3));
    b.screen_base(&brightness(0.4));
    assert!(b.window_changed(jan(0), t0, true));
    let ti = tiles(&b.tick(t0));
    b.finished(ti[0].0);
    assert_eq!(bases(&b.tick(ms(t0, 1))).len(), 1);
}

/// Pan logo depois de mudar o valor: o tile novo tirou a base do valor velho e
/// a area exposta ficaria escura ate a base nova (200 ms). Com o tile sem base
/// na tela (a view avisa [`ScheduleAdjustment::base_missing`] depois de `window_changed`),
/// a base sai ja, antes do tile da janela nova.
#[test]
fn window_new_with_the_tile_no_base_asks_the_base_already() {
    let t0 = Instant::now();
    let mut a = schedule();
    assert!(!a.window_changed(jan(0), t0, false));
    a.changed(brightness(0.2), t0);
    let t1 = tiles(&a.tick(t0));
    a.finished(t1[0].0);
    assert!(a.tick(ms(t0, 10)).is_empty(), "sem pan a base espera os 200 ms");
    assert!(a.window_changed(jan(40), ms(t0, 50), true));
    a.base_missing(ms(t0, 50));
    assert_eq!(a.next_tick(ms(t0, 50)), Some(Duration::ZERO));
    let actions = a.tick(ms(t0, 50));
    assert_eq!(bases(&actions).iter().map(|b| b.1.clone()).collect::<Vec<_>>(), vec![brightness(0.2)], "{actions:?}");
    assert!(tiles(&actions).is_empty());
    let ba = bases(&actions)[0].0;
    // o tile da janela nova sai depois da base, 60 ms depois da janela
    a.finished(ba);
    assert!(a.tick(ms(t0, 109)).is_empty());
    let t2 = tiles(&a.tick(ms(t0, 110)));
    assert_eq!(t2.iter().map(|t| t.2).collect::<Vec<_>>(), vec![Some(jan(40))]);
    a.finished(t2[0].0);
    assert!(bases(&a.tick(ms(t0, 400))).is_empty(), "uma base so");
}

/// A regra exata da base adiantada: ela nunca sai antes de o filtro em voo
/// terminar (um filtro por vez). Sem janela nova, a base continua esperando um
/// tile do valor terminar sem interrupcao (a ordem tile-antes-da-base). Com
/// janela nova, o tile em voo era da janela velha e foi interrompido: a base
/// sai assim que ele terminar, antes do tile da janela nova, sem esperar um
/// tile do valor ficar pronto.
#[test]
fn base_early_wait_the_tile_in_flight_finish() {
    let t0 = Instant::now();
    let mut a = schedule();
    assert!(!a.window_changed(jan(0), t0, false));
    a.changed(brightness(0.2), t0);
    let t1 = tiles(&a.tick(t0))[0].0;
    assert!(a.window_changed(jan(40), ms(t0, 5), true));
    a.base_missing(ms(t0, 5));
    let actions = a.tick(ms(t0, 5));
    assert_eq!(interruptions(&actions), vec![t1]);
    assert!(bases(&actions).is_empty(), "o tile em voo ainda nao terminou");
    assert!(a.tick(ms(t0, 100)).is_empty(), "um filtro em voo por vez");
    a.finished(t1);
    let actions = a.tick(ms(t0, 101));
    assert_eq!(bases(&actions).iter().map(|b| b.1.clone()).collect::<Vec<_>>(), vec![brightness(0.2)], "{actions:?}");
    assert!(tiles(&actions).is_empty(), "a base antes do tile da janela nova");
    a.finished(bases(&actions)[0].0);
    assert_eq!(tiles(&a.tick(ms(t0, 102))).iter().map(|t| t.2).collect::<Vec<_>>(), vec![Some(jan(40))]);

    // uma base do valor ja em voo: nada de outra
    let mut b = schedule();
    assert!(!b.window_changed(jan(0), t0, false));
    let ba = with_base_in_flight(&mut b, brightness(0.4), t0);
    assert!(b.window_changed(jan(40), ms(t0, 1), true));
    b.base_missing(ms(t0, 1));
    assert!(b.tick(ms(t0, 1)).is_empty(), "a base em voo nao depende da janela");
    b.finished(ba);
    assert!(bases(&b.tick(ms(t0, 2))).is_empty(), "a base do valor ja veio");
    let ti = tiles(&b.tick(ms(t0, 61)));
    assert_eq!(ti.len(), 1);
    b.finished(ti[0].0);
    assert!(bases(&b.tick(ms(t0, 1000))).is_empty());

    // um valor novo depois volta a ordem tile-antes-da-base
    let mut c = schedule();
    assert!(!c.window_changed(jan(0), t0, false));
    c.changed(brightness(0.2), t0);
    let ti = tiles(&c.tick(t0))[0].0;
    c.finished(ti);
    assert!(c.window_changed(jan(40), ms(t0, 10), true));
    c.base_missing(ms(t0, 10));
    assert!(c.changed(brightness(0.3), ms(t0, 10)));
    let actions = c.tick(ms(t0, 10));
    assert_eq!(values(&tiles(&actions)), vec![brightness(0.3)], "{actions:?}");
    assert!(bases(&actions).is_empty());

    // sem ajuste na tela, nada a adiantar
    let mut d = schedule();
    d.base_missing(t0);
    assert!(d.tick(t0).is_empty() && d.idle());
}

/// Sem janela informada (a imagem nova ainda carrega: o visualizador so informa
/// a regiao quando uma camada dela esta na tela), um valor novo ou o olho NAO
/// pedem um tile filtrado (seria a pagina inteira em resolucao cheia e abriria
/// uma geracao nova, derrubando a base da carga em voo): o tile espera a
/// primeira janela, sem tick nem trabalho pendente que segure o ocioso. A
/// textura sem filtro (neutro, olho desligado) nao precisa de janela.
#[test]
fn no_window_the_tile_filtered_wait_the_first_window() {
    let t0 = Instant::now();
    let mut a = ScheduleAdjustment::new();
    a.switched_image(img("p01.png"), brightness(0.3));
    assert!(a.changed(brightness(0.4), t0));
    a.dropped(t0);
    let actions = a.tick(t0);
    assert!(tiles(&actions).is_empty() && bases(&actions).is_empty(), "nada filtrado sem janela: {actions:?}");
    assert_eq!(writes(&actions).len(), 1, "a gravacao nao espera a janela");
    assert_eq!(a.next_tick(ms(t0, 1)), None, "sem tick em vao");
    assert!(a.idle(), "o tile que espera a janela nao segura o ocioso");
    assert!(a.refilter(ms(t0, 2), true));
    assert!(tiles(&a.tick(ms(t0, 2))).is_empty(), "o olho tambem espera a janela");
    // a primeira janela: o tile sai ja, com ela; a base depois dele
    assert!(a.window_changed(jan(0), ms(t0, 10), true));
    let ti = tiles(&a.tick(ms(t0, 10)));
    assert_eq!(ti.iter().map(|t| (t.1.clone(), t.2)).collect::<Vec<_>>(), vec![(brightness(0.4), Some(jan(0)))]);
    a.finished(ti[0].0);
    assert_eq!(bases(&a.tick(ms(t0, 11))).len(), 1);
    // neutro (a textura cheia sem filtro) e olho desligado saem sem janela
    let mut b = ScheduleAdjustment::new();
    b.switched_image(img("p01.png"), brightness(0.3));
    assert!(b.changed(Adjustments::default(), t0));
    assert_eq!(tiles(&b.tick(t0)).iter().map(|t| t.2).collect::<Vec<_>>(), vec![None]);
    let mut c = ScheduleAdjustment::new();
    c.switched_image(img("p01.png"), brightness(0.3));
    assert!(c.refilter(t0, false));
    assert_eq!(tiles(&c.tick(t0)).len(), 1);
}

#[test]
fn no_image_nothing_happens() {
    let t0 = Instant::now();
    let mut a = ScheduleAdjustment::new();
    assert!(!a.changed(brightness(0.3), t0));
    assert!(!a.window_changed(jan(0), t0, true));
    a.dropped(t0);
    a.fell_tile(t0, None);
    assert!(a.tick(t0).is_empty());
    assert!(a.idle());
}
