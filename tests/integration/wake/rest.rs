//! The rest's gate as the receiver reads it (docs/receiver.md): the moment a run's usage calls a rest for, and
//! the lengths the policy declares.

use receiver_policy::rest::REST;

use bmo_receiver::receiver::wake::rest::{Usage, outage_rest_until, rest_until_after, rest_until_for};

const NOW: i64 = 1_760_000_000;

#[test]
fn a_window_past_its_ceiling_rests_its_login_until_it_resets() {
  let usage = Usage {
    utilization: REST.threshold,
    resets_at: (NOW + 100) as f64,
    weekly_utilization: None,
    weekly_resets_at: None,
  };

  assert_eq!(rest_until_after(&usage, NOW), Some((NOW + 100) as f64));
}

#[test]
fn a_weekly_window_past_its_ceiling_rests_until_its_reset_or_the_fallback_after_now() {
  let usage = Usage {
    utilization: 0.5,
    resets_at: 0.0,
    weekly_utilization: Some(REST.weekly_threshold),
    weekly_resets_at: Some((NOW + 500) as f64),
  };

  assert_eq!(rest_until_after(&usage, NOW), Some((NOW + 500) as f64));

  let unnamed = Usage {
    utilization: 0.5,
    resets_at: 0.0,
    weekly_utilization: Some(REST.weekly_threshold),
    weekly_resets_at: None,
  };

  assert_eq!(rest_until_after(&unnamed, NOW), Some((NOW + REST.fallback_seconds) as f64));
}

#[test]
fn neither_window_past_its_ceiling_rests_nobody() {
  let usage = Usage {
    utilization: REST.threshold - 0.01,
    resets_at: (NOW + 100) as f64,
    weekly_utilization: Some(REST.weekly_threshold - 0.01),
    weekly_resets_at: None,
  };

  assert_eq!(rest_until_after(&usage, NOW), None);
}

#[test]
fn an_outage_rest_lasts_its_declared_length() {
  let since = NOW * 1000;

  assert_eq!(outage_rest_until(since), since.div_euclid(1000) + REST.outage_seconds);
}

#[test]
fn a_limit_that_named_no_reset_uses_the_fallback_after_now() {
  let since = NOW * 1000;

  assert_eq!(rest_until_for(Some(NOW + 7), since), NOW + 7);
  assert_eq!(rest_until_for(None, since), (since + REST.fallback_seconds * 1000).div_euclid(1000));
}
