//! The secret shapes (docs/messages.md, 「Secrets taken out」): every shape the policy holds takes its run out of
//! the words, a sentence keeps its other words, a `name=value` keeps its name, the marker is read as no word, and
//! ordinary words are left as they were written. Each secret-shaped value is assembled at run time, so no tracked
//! file holds one (docs/PERSONAL-DATA.md).

use bmo_receiver::secrets::{marker, names, only_marked, redact, redact_value, redacted, unmarked};
use serde_json::json;

// One value of every shape, by the shape's name, each assembled from parts no check reads as a token.
fn samples() -> Vec<(&'static str, String)> {
  vec![
    (
      "pem-private-key",
      format!(
        "-----BEGIN {0} KEY-----\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASC\n-----END {0} KEY-----",
        "RSA PRIVATE"
      ),
    ),
    (
      "jwt",
      format!(
        "{0}J{1}.{0}J{2}.{3}",
        "ey", "hbGciOiJIUzI1NiJ9", "zdWIiOiIxMjM0NTY3ODkwIn0", "dozjgNryP4J3jVmNHl0w5N"
      ),
    ),
    ("sk-key", format!("{}{}", "sk", "-proj-abcdEFGH1234ijklMNOP5678")),
    ("github-token", format!("gh{}_{}", "p", "A1b2".repeat(9))),
    ("slack-token", format!("xox{}-{}", "b", "1234567890-abcdefghij")),
    ("aws-access-key", format!("AKI{}{}", "A", "IOSFODNN7EXAMPLE")),
    (
      "google-api-key",
      format!("AI{}{}", "za", "Sy".to_owned() + &"a1B2c3D4e5".repeat(4)[..33]),
    ),
    ("hf-token", format!("hf{}{}", "_", "aB3".repeat(12))),
    ("telegram-bot-token", format!("{}:{}", "123456789", "AA".to_owned() + &"x9Y".repeat(11))),
    ("url-password", format!("postgres://app:{}@db.example.com/app", "hunter2")),
    ("basic-auth", format!("Authorization: Basic {}{}", "dGVzdGVyOmh1", "bnRlcjI=")),
    ("bearer-token", format!("Bearer {}", "q7Wm".repeat(6))),
    ("session-string", format!("1{}", "BVtsOK4Bu1".repeat(35))),
    ("hex-64", "0123456789abcdef".repeat(4)),
    ("named-value", format!("{}={}", "DB_PASSWORD", "correcthorsebattery")),
  ]
}

#[test]
fn every_shape_takes_its_run_out_and_every_shape_has_a_sample() {
  let samples = samples();
  let mut sampled: Vec<&str> = samples.iter().map(|(name, _)| *name).collect();
  let mut declared = names();

  sampled.sort_unstable();
  declared.sort_unstable();
  assert_eq!(sampled, declared, "every shape the policy holds is sampled here, and no other");

  for (name, value) in &samples {
    let read = redact(value);

    assert!(read.runs >= 1, "the {name} shape took nothing out");
    assert!(read.text.contains(marker()), "the {name} shape left no marker");
    assert!(!read.text.contains(value.as_str()), "the {name} shape left its value");
  }
}

#[test]
fn a_sentence_keeps_its_other_words() {
  let key = format!("{}{}", "sk", "-or-v1-".to_owned() + &"0123456789abcdef".repeat(4));
  let hf = format!("hf{}{}", "_", "Zx9".repeat(11));
  let read = redact(&format!("use {key} for the road, then hf {hf} too, thanks"));

  assert_eq!(read.text, format!("use {0} for the road, then hf {0} too, thanks", marker()));
  assert_eq!(read.runs, 2);
}

#[test]
fn a_named_value_keeps_its_name() {
  assert_eq!(
    redacted(&format!("{}={} is the setting", "OPENROUTER_API_KEY", "abcdefghijklmnop")),
    format!("OPENROUTER_API_KEY={} is the setting", marker())
  );
  assert_eq!(
    redacted(&format!("{{\"password\": \"{}\"}}", "correct-horse-battery")),
    format!("{{\"password\": \"{}\"}}", marker())
  );
  assert_eq!(
    redacted(&format!("access_token: {}", "zyxwvutsrqponm")),
    format!("access_token: {}", marker())
  );

  // A value another shape took first is one run, and its name still stands.
  let read = redact(&format!("{}={}{}", "OPENAI_API_KEY", "sk", "-abcdefghijklmnopqrstuvwx"));

  assert_eq!((read.text, read.runs), (format!("OPENAI_API_KEY={}", marker()), 1));
}

#[test]
fn a_short_value_and_a_word_ending_in_a_name_are_no_secret() {
  for words in [
    "password: hunter2",
    "the monkey: a long animal sentence",
    "keyboard=mechanicalswitches",
    "the key to it is patience and practice",
  ] {
    assert_eq!(redacted(words), words);
  }
}

#[test]
fn ordinary_words_a_commit_hash_and_a_long_korean_sentence_stand() {
  let korean =
    std::fs::read_to_string("tests/fixtures/secret-words-korean.txt").expect("the fixture reads");
  let sentence = korean.lines().next().expect("the fixture's sentence");

  assert!(
    sentence.chars().count() >= 64,
    "the fixture's sentence runs sixty-four characters or more"
  );
  assert_eq!(redact(sentence).runs, 0);

  for words in [
    "dc77f56a1b2c3d4e5f60718293a4b5c6d7e8f901",
    "merged as dc77f56a1b2c3d4e5f60718293a4b5c6d7e8f901 into main",
    "see https://example.com/a/very/long/path/that/goes/on/and/on/for/a/while/yet",
    "sk-learn is a library",
    "a plain sentence with nothing to hide",
  ] {
    assert_eq!(redacted(words), words, "{words}");
  }
}

#[test]
fn a_run_glued_to_korean_words_is_still_taken_out() {
  let korean =
    std::fs::read_to_string("tests/fixtures/secret-words-korean.txt").expect("the fixture reads");
  let template = korean.lines().nth(1).expect("the fixture's glued line");
  let key = format!("{}{}", "sk", "-abcdefghijklmnopqrstuvwx");

  assert_eq!(redacted(&template.replace("{token}", &key)), template.replace("{token}", marker()));
}

#[test]
fn a_private_key_block_the_paste_cut_short_is_taken_out_to_the_end() {
  let cut = format!(
    "here it is\n-----BEGIN {} KEY-----\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASC",
    "OPENSSH PRIVATE"
  );

  assert_eq!(redacted(&cut), format!("here it is\n{}", marker()));
}

#[test]
fn words_read_twice_lose_nothing_more() {
  let once =
    redacted(&format!("{}={} and {}", "API_KEY", "abcdefghijklmnop", "0123456789abcdef".repeat(4)));
  let twice = redact(&once);

  assert_eq!(twice.text, once);
  assert_eq!(twice.runs, 0);
}

#[test]
fn the_marker_is_no_word() {
  let only = format!("  {0}\n{0} ", marker());

  assert!(only_marked(&only));
  assert!(!only_marked(&format!("{} here it is", marker())));
  assert!(!only_marked("   "));
  assert!(!unmarked(&format!("a{}b", marker())).contains(marker()));
}

#[test]
fn every_string_of_a_payload_is_read_and_its_keys_and_numbers_stand() {
  let token = "0123456789abcdef".repeat(4);
  let mut payload = json!({
    "text": format!("the token is {token}"),
    "entities": [{ "type": "code", "offset": 13, "length": 64, "note": token }],
    "id": 7,
  });

  assert_eq!(redact_value(&mut payload), 2);
  assert_eq!(
    payload,
    json!({
      "text": format!("the token is {}", marker()),
      "entities": [{ "type": "code", "offset": 13, "length": 64, "note": marker() }],
      "id": 7,
    })
  );
}

// A console secret-request link as the supervisor's procedure writes it: the console's page of a room, then the
// file's name, its fields or kind, what it is for and whose space it goes into, each value percent-encoded.
fn request_link(query: &str) -> String {
  format!("https://console.example.com/bmo/rooms/4200?{query}")
}

// "a console secret-request link, which carries no secret but names one, stands whole"
#[test]
fn a_console_secret_request_link_stands_whole() {
  for link in [
    request_link("secret=app-prod-user.secret&fields=username&for=the%20app%20database&user=7"),
    request_link("secret=app-prod.pass&fields=password%2A&for=the%20app%20database"),
    request_link("secret=mail-relay&kind=smtp&for=mail"),
  ] {
    let words = format!("Save it here: {link} and tell me once it is saved.");

    assert_eq!(redact(&words).runs, 0, "{link}");
    assert_eq!(redacted(&words), words);
  }
}

// "a key, token or password value written in a link's query or as `KEY=VALUE` anywhere else is taken out as
// before", a request link carrying another parameter among them.
#[test]
fn a_secret_in_a_link_s_query_or_set_as_a_value_is_still_taken_out() {
  let password = format!("{}={}", "password", "hunter2hunter2");
  let token = format!("https://console.example.com/bmo/rooms/4200?{}={}", "token", "abcdef123456789");
  let smuggled = request_link(&format!("secret=app.pass&{}={}", "token", "abcdef123456789"));
  let elsewhere =
    format!("https://console.example.com/bmo/settings?{}={}", "secret", "app-prod-user.secret");

  assert_eq!(redacted(&password), format!("password={}", marker()));
  assert_eq!(
    redacted(&token),
    format!("https://console.example.com/bmo/rooms/4200?token={}", marker())
  );
  assert!(!redacted(&smuggled).contains("abcdef123456789"), "{}", redacted(&smuggled));
  assert!(
    !redacted(&elsewhere).contains("app-prod-user.secret"),
    "only a room's page is a request link"
  );
}

// A base64 run as long as a session string, written the way a link carries a redirect target: the base64 of
// `https://ads.example.com/`, again and again.
fn base64_run() -> String {
  "aHR0cHM6Ly9hZHMuZXhhbXBsZS5jb20v".repeat(15)
}

// "A run of three hundred base64 letters inside a link is the link's own ... It stands"
#[test]
fn a_long_base64_run_inside_a_link_stands_whole() {
  let run = base64_run();

  for link in [
    format!("https://shop.example.com/bridge/go.php?sLink={run}&minPrice=1130000&adKeyword=NAS+HDD+8TB"),
    format!("https://ads.example.com/v1/{}?c=shop.ch9&t=0", &run[..320]),
    format!("http://parts.example.com/filter/739?s={run}%2BlRLOC"),
  ] {
    for words in [format!("here it is: {link}"), format!("[the offer]({link})\nask me again")] {
      assert_eq!(redact(&words).runs, 0, "{words}");
      assert_eq!(redacted(&words), words);
    }
  }
}

// "a marker an earlier shape left inside it included": the spare spells the policy's marker, so a link a narrower
// shape already took a run out of keeps its long base64 run, and this fails where the two part.
#[test]
fn a_long_base64_run_stands_in_a_link_an_earlier_shape_took_a_run_out_of() {
  let run = base64_run();
  let link = format!("https://tester:{}@shop.example.com/bridge?sLink={run}", "hunter2pw");

  assert_eq!(
    redacted(&link),
    format!("https://tester:{}@shop.example.com/bridge?sLink={run}", marker())
  );
}

// "a run of three hundred or more base64 letters" outside a link is still a session string, a link just
// before it included.
#[test]
fn a_long_base64_run_beside_a_link_is_still_taken_out() {
  let run = base64_run();

  assert_eq!(
    redacted(&format!("see https://shop.example.com/a then {run}")),
    format!("see https://shop.example.com/a then {}", marker())
  );
}

// "Every other shape still reads the link": a signed link's signature, its session token and a token set by its
// name are taken out of a link a long base64 run stands in.
#[test]
fn a_secret_in_a_link_is_taken_out_while_its_long_base64_run_stands() {
  let run = base64_run();
  let hex = "0123456789abcdef";

  for (name, value) in [
    ("access_token", "zyxwvutsrqponm1234".to_owned()),
    ("sig", "kP3e".repeat(11)),
    ("Signature", "Zm9vYmFy".repeat(4)),
    ("X-Amz-Signature", hex.repeat(4)),
    ("X-Goog-Signature", hex.repeat(32)),
    ("X-Amz-Security-Token", "FwoGZXIvYXdzE".repeat(25)),
  ] {
    let link = format!("https://store.example.com/o?go={run}&{name}={value}");
    let read = redacted(&link);

    assert!(!read.contains(&value), "{name}: {read}");
    assert!(read.contains(&format!("go={run}&{name}=")), "{name}: {read}");
  }
}

#[test]
fn a_password_in_a_link_s_user_part_is_taken_out_and_the_link_stands() {
  let words = format!("connect to {}://app:{}@db.example.com:5432/app", "postgres", "s3cr3t!pw");

  assert_eq!(
    redacted(&words),
    format!("connect to postgres://app:{}@db.example.com:5432/app", marker())
  );

  for words in [
    "https://example.com:8443/a/b?x=1",
    "write to tester@example.com",
    "https://example.com/@tester/posts",
  ] {
    assert_eq!(redacted(words), words);
  }
}

#[test]
fn a_bearer_token_and_basic_credentials_are_taken_out_and_a_placeholder_stands() {
  let token = "q7Wm.x_2-".repeat(4);

  assert_eq!(
    redacted(&format!("curl -H 'Authorization: Bearer {token}' https://api.example.com")),
    format!("curl -H 'Authorization: Bearer {}' https://api.example.com", marker())
  );
  assert_eq!(
    redacted(&format!("\"authorization\": \"Basic {}{}\"", "dGVzdGVyOmh1", "bnRlcjI=")),
    format!("\"authorization\": \"Basic {}\"", marker())
  );

  for words in [
    "Authorization: Bearer <token>",
    "Authorization: Bearer $TOKEN",
    "the bearer of bad news came early",
    "a basic authorization scheme",
  ] {
    assert_eq!(redacted(words), words);
  }
}
