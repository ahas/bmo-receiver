//! The console's measure of a text and its writing of a bound.

use bmo_receiver::text::{grouped, length};

#[test]
fn a_length_counts_utf16_units_and_a_bound_sets_its_thousands_apart() {
  assert_eq!(length("ab"), 2);
  assert_eq!(length("😀"), 2);
  assert_eq!(grouped(200_000), "200,000");
  assert_eq!(grouped(60), "60");
}
