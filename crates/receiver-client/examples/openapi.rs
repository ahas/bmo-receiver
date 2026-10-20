//! Prints the OpenAPI document this crate generates, as `docs/openapi.json` keeps it.

fn main() {
  print!("{}", receiver_client::openapi_json());
}
