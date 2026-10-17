//! A text measured and a bound written as the console does, so a refusal reads the same whichever side
//! said it.

use crate::js::units;

/// A text's length as the console counts it, in UTF-16 units.
pub fn length(text: &str) -> usize {
  units(text)
}

/// A bound as the console writes a number, its thousands set apart.
pub fn grouped(number: usize) -> String {
  let digits = number.to_string();
  let mut grouped = String::new();

  for (at, digit) in digits.chars().enumerate() {
    if at > 0 && (digits.len() - at).is_multiple_of(3) {
      grouped.push(',');
    }

    grouped.push(digit);
  }

  grouped
}
