use budget_core::hlc::{Clock, Hlc};

const DEV_A: &str = "00000000-0000-4000-8000-00000000000a";
const DEV_B: &str = "00000000-0000-4000-8000-00000000000b";

#[test]
fn successive_local_ticks_only_move_forward() {
    let mut clock = Clock::new(DEV_A);
    let first = clock.tick(1_000);
    let second = clock.tick(2_000);
    assert!(second > first);
    assert_eq!(second.wall_ms(), 2_000);
}

#[test]
fn ticks_keep_moving_forward_when_the_wall_clock_goes_backwards() {
    let mut clock = Clock::new(DEV_A);
    let before = clock.tick(5_000);
    let after_jump_back = clock.tick(1_000);
    let later_still = clock.tick(1_001);
    assert!(after_jump_back > before);
    assert!(later_still > after_jump_back);
    assert_eq!(
        after_jump_back.wall_ms(),
        5_000,
        "wall time never goes back"
    );
}

#[test]
fn observing_a_stamp_from_a_device_whose_clock_is_ahead_moves_the_clock_past_it() {
    let mut phone = Clock::new(DEV_B);
    let from_phone = phone.tick(90_000);

    let mut pc = Clock::new(DEV_A);
    pc.tick(10_000);
    pc.observe(&from_phone);
    let next_on_pc = pc.tick(10_001);
    assert!(next_on_pc > from_phone);
}

#[test]
fn stamps_with_the_same_time_and_counter_are_ordered_by_device_id() {
    let a = Clock::new(DEV_A).tick(7_000);
    let b = Clock::new(DEV_B).tick(7_000);
    assert_ne!(a, b);
    assert!(a < b, "DEV_A sorts before DEV_B");
    assert!(a.to_string() < b.to_string());
}

#[test]
fn the_string_form_is_zero_padded_and_round_trips() {
    let stamp = Clock::new(DEV_A).tick(1_731_900_000_123);
    let text = stamp.to_string();
    assert_eq!(text, format!("0001731900000123-0000-{DEV_A}"));
    assert_eq!(text.parse::<Hlc>().unwrap(), stamp);
}

#[test]
fn string_order_matches_clock_order_even_after_many_writes_in_one_millisecond() {
    let mut clock = Clock::new(DEV_A);
    let mut previous = clock.tick(1_000);
    for _ in 0..20_000 {
        let next = clock.tick(1_000);
        assert!(next > previous);
        assert!(
            next.to_string() > previous.to_string(),
            "{next} vs {previous}"
        );
        previous = next;
    }
}

#[test]
fn malformed_stamps_are_rejected() {
    assert!("".parse::<Hlc>().is_err());
    assert!("12-0000-x".parse::<Hlc>().is_err());
    assert!("000000000000abcd-0000-dev".parse::<Hlc>().is_err());
}
