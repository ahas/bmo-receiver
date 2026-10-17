//! A JSON value that keeps the order its object keys were written in, and writes itself back the way
//! JavaScript's `JSON.stringify` does. A run log line carries a tool's arguments and what it handed back as the
//! program wrote them, so a reader comparing a line written here with one the run log wrote reads the same text.

use std::fmt;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

/// One JSON value, its objects in the order their keys were written.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
  Null,
  Bool(bool),
  Number(serde_json::Number),
  String(String),
  Array(Vec<Json>),
  Object(Vec<(String, Json)>),
}

impl Json {
  /// A line of text read as JSON, or none where it is not.
  pub fn parse(text: &str) -> Option<Self> {
    serde_json::from_str(text).ok()
  }

  /// The field of an object named so, the last written where a name stands twice.
  pub fn get(&self, name: &str) -> Option<&Json> {
    match self {
      Self::Object(fields) => fields.iter().rev().find(|(held, _)| held == name).map(|(_, value)| value),
      _ => None,
    }
  }

  pub fn as_str(&self) -> Option<&str> {
    match self {
      Self::String(text) => Some(text),
      _ => None,
    }
  }

  pub fn as_array(&self) -> Option<&[Json]> {
    match self {
      Self::Array(items) => Some(items),
      _ => None,
    }
  }

  pub fn as_bool(&self) -> Option<bool> {
    match self {
      Self::Bool(value) => Some(*value),
      _ => None,
    }
  }

  pub fn is_object(&self) -> bool {
    matches!(self, Self::Object(_))
  }

  /// The text JavaScript's `JSON.stringify` writes for this value.
  pub fn stringify(&self) -> String {
    let mut out = String::new();

    self.write_into(&mut out);
    out
  }

  fn write_into(&self, out: &mut String) {
    match self {
      Self::Null => out.push_str("null"),
      Self::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
      Self::Number(number) => out.push_str(&number_text(number)),
      Self::String(text) => out.push_str(&quoted(text)),
      Self::Array(items) => {
        out.push('[');
        for (place, item) in items.iter().enumerate() {
          if place > 0 {
            out.push(',');
          }
          item.write_into(out);
        }
        out.push(']');
      }
      Self::Object(fields) => {
        out.push('{');
        for (place, (name, value)) in fields.iter().enumerate() {
          if place > 0 {
            out.push(',');
          }
          out.push_str(&quoted(name));
          out.push(':');
          value.write_into(out);
        }
        out.push('}');
      }
    }
  }
}

/// A string as JSON writes it.
pub fn quoted(text: &str) -> String {
  serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_owned())
}

/// A number as JavaScript writes it: whole numbers without a point, others as the shortest text that
/// reads back to the same value.
pub fn number_text(number: &serde_json::Number) -> String {
  match number.as_f64() {
    Some(value) if number.is_f64() && value.fract() == 0.0 && value.abs() < 1e21 => format!("{value:.0}"),
    _ => number.to_string(),
  }
}

impl<'de> Deserialize<'de> for Json {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    deserializer.deserialize_any(JsonVisitor)
  }
}

struct JsonVisitor;

impl<'de> Visitor<'de> for JsonVisitor {
  type Value = Json;

  fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str("any JSON value")
  }

  fn visit_bool<E: de::Error>(self, value: bool) -> Result<Json, E> {
    Ok(Json::Bool(value))
  }

  fn visit_i64<E: de::Error>(self, value: i64) -> Result<Json, E> {
    Ok(Json::Number(value.into()))
  }

  fn visit_u64<E: de::Error>(self, value: u64) -> Result<Json, E> {
    Ok(Json::Number(value.into()))
  }

  fn visit_f64<E: de::Error>(self, value: f64) -> Result<Json, E> {
    Ok(serde_json::Number::from_f64(value).map_or(Json::Null, Json::Number))
  }

  fn visit_str<E: de::Error>(self, value: &str) -> Result<Json, E> {
    Ok(Json::String(value.to_owned()))
  }

  fn visit_string<E: de::Error>(self, value: String) -> Result<Json, E> {
    Ok(Json::String(value))
  }

  fn visit_unit<E: de::Error>(self) -> Result<Json, E> {
    Ok(Json::Null)
  }

  fn visit_none<E: de::Error>(self) -> Result<Json, E> {
    Ok(Json::Null)
  }

  fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Json, A::Error> {
    let mut items = Vec::new();

    while let Some(item) = seq.next_element()? {
      items.push(item);
    }

    Ok(Json::Array(items))
  }

  fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Json, A::Error> {
    let mut fields: Vec<(String, Json)> = Vec::new();

    while let Some((name, value)) = map.next_entry::<String, Json>()? {
      // A name written twice keeps its first place and its last value, as `JSON.parse` reads it.
      match fields.iter_mut().find(|(held, _)| *held == name) {
        Some((_, held)) => *held = value,
        None => fields.push((name, value)),
      }
    }

    Ok(Json::Object(fields))
  }
}
