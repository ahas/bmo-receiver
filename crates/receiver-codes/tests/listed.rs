//! Every code and every kind is listed in the codes document under its enum, every code is kebab-case and unique
//! across refusals, facts and fixed lines, and the wire shapes are the document's (docs/codes.md).

use std::collections::{BTreeMap, BTreeSet};

use bmo_chat_client::ChatCapability;
use http::StatusCode;
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use receiver_codes::draws::{DrawAsked, DrawLine, DrawResult};
use receiver_codes::{
  AvatarRefusal, Code, Fact, FileFillRefusal, FileUnsent, FixedLine, HealthFact, MessageActRefusal,
  OwnerSendRefusal, ReceiverError, Refused, RoomAdditionRefusal, RoomGrantRefusal, RoomModeRefusal,
  RoomReadRefusal, RoomRemovalRefusal, RoomRemovalWarning, RunTitle, SendRefusal, SessionReason,
  SettingFact, SkipReason, SupervisorFact, SupervisorRefusal, TalkRefusal, TurnOpening, VoiceFact,
  VoiceRefusal,
};
use serde_json::json;
use strum::VariantNames;

const DOCUMENT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/codes.md");

/// Whether an enum's names are codes, unique across the receiver, or kinds, unique within their list.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Names {
  Codes,
  Kinds,
}

fn enums() -> Vec<(&'static str, Names, &'static [&'static str])> {
  use Names::{Codes, Kinds};

  vec![
    ("AvatarRefusal", Codes, AvatarRefusal::VARIANTS),
    ("MessageActRefusal", Codes, MessageActRefusal::VARIANTS),
    ("FileFillRefusal", Codes, FileFillRefusal::VARIANTS),
    ("OwnerSendRefusal", Codes, OwnerSendRefusal::VARIANTS),
    ("TalkRefusal", Codes, TalkRefusal::VARIANTS),
    ("VoiceRefusal", Codes, VoiceRefusal::VARIANTS),
    ("RoomRemovalRefusal", Codes, RoomRemovalRefusal::VARIANTS),
    ("SendRefusal", Codes, SendRefusal::VARIANTS),
    ("RoomGrantRefusal", Codes, RoomGrantRefusal::VARIANTS),
    ("RoomAdditionRefusal", Codes, RoomAdditionRefusal::VARIANTS),
    ("RoomModeRefusal", Codes, RoomModeRefusal::VARIANTS),
    ("RoomReadRefusal", Codes, RoomReadRefusal::VARIANTS),
    ("SupervisorRefusal", Codes, SupervisorRefusal::VARIANTS),
    ("HealthFact", Codes, HealthFact::VARIANTS),
    ("VoiceFact", Codes, VoiceFact::VARIANTS),
    ("SettingFact", Codes, SettingFact::VARIANTS),
    ("SupervisorFact", Codes, SupervisorFact::VARIANTS),
    ("RoomRemovalWarning", Codes, RoomRemovalWarning::VARIANTS),
    ("FixedLine", Codes, FixedLine::VARIANTS),
    ("FileUnsent", Kinds, FileUnsent::VARIANTS),
    ("RunTitle", Kinds, RunTitle::VARIANTS),
    ("SessionReason", Kinds, SessionReason::VARIANTS),
    ("TurnOpening", Kinds, TurnOpening::VARIANTS),
    ("SkipReason", Kinds, SkipReason::VARIANTS),
  ]
}

/// Each `###` heading of the document with the first cells of the tables under it, read as code spans.
fn listed() -> BTreeMap<String, BTreeSet<String>> {
  let text = std::fs::read_to_string(DOCUMENT).expect("the codes document is readable");
  let mut listed: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
  let mut heading: Option<String> = None;
  let mut in_heading = false;
  let mut first_cell = false;
  let mut cell = 0;

  for event in Parser::new_ext(&text, pulldown_cmark::Options::ENABLE_TABLES) {
    match event {
      Event::Start(Tag::Heading {
        level: HeadingLevel::H3,
        ..
      }) => {
        in_heading = true;
        heading = Some(String::new());
      }
      Event::End(TagEnd::Heading(HeadingLevel::H3)) => in_heading = false,
      Event::Start(Tag::Heading { .. }) => heading = None,
      Event::Text(words) if in_heading => heading.get_or_insert_with(String::new).push_str(&words),
      Event::Start(Tag::TableRow) => cell = 0,
      Event::Start(Tag::TableCell) => {
        cell += 1;
        first_cell = cell == 1;
      }
      Event::End(TagEnd::TableCell) => first_cell = false,
      Event::Code(code) if first_cell => {
        if let Some(heading) = &heading {
          listed.entry(heading.clone()).or_default().insert(code.to_string());
        }
      }
      _ => {}
    }
  }

  listed
}

fn is_kebab(name: &str) -> bool {
  !name.is_empty()
    && name.split('-').all(|part| {
      part.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && part.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    })
}

#[test]
fn every_code_and_kind_is_listed_under_its_enum_and_nothing_else_is() {
  let listed = listed();

  for (name, _, variants) in enums() {
    let held: BTreeSet<String> = variants.iter().map(|variant| (*variant).to_owned()).collect();
    let document = listed.get(name).unwrap_or_else(|| panic!("docs/codes.md lists no `{name}`"));

    assert_eq!(&held, document, "`{name}` and its list in docs/codes.md differ");
  }
  for heading in listed.keys() {
    assert!(
      enums().iter().any(|(name, _, _)| name == heading),
      "docs/codes.md lists `{heading}`, no enum"
    );
  }
}

#[test]
fn every_code_is_kebab_case_and_unique_across_the_families() {
  let mut seen = BTreeMap::new();

  for (name, names, variants) in enums() {
    let mut within = BTreeSet::new();

    for variant in variants {
      assert!(is_kebab(variant), "`{variant}` of `{name}` is not kebab-case");
      assert!(within.insert(*variant), "`{variant}` stands twice in `{name}`");
      if names == Names::Codes
        && let Some(other) = seen.insert(*variant, name)
      {
        panic!("`{variant}` is a code of both `{other}` and `{name}`");
      }
    }
  }
}

#[test]
fn a_refusal_answers_its_code_its_status_and_an_english_sentence() {
  let refusal = ReceiverError::from(SendRefusal::VerbRefused {
    road: "road-a".to_owned(),
    verb: ChatCapability::Sticker,
  });

  assert_eq!(refusal.code(), "verb-refused");
  assert_eq!(refusal.status(), StatusCode::CONFLICT);
  assert_eq!(refusal.to_string(), "the road road-a does not perform `sticker`");
  assert_eq!(ReceiverError::from(TalkRefusal::Limit).status(), StatusCode::TOO_MANY_REQUESTS);
  let unlisted = ReceiverError::from(RoomAdditionRefusal::RoomsUnlisted {
    label: "the road-a road".to_owned(),
  });
  assert_eq!(
    (unlisted.code(), unlisted.status(), unlisted.to_string()),
    (
      "rooms-unlisted",
      StatusCode::CONFLICT,
      "the road-a road does not list the rooms of its account".to_owned()
    )
  );
  assert_eq!(ReceiverError::from(TalkRefusal::Outage).code(), "outage");
  assert_eq!(
    OwnerSendRefusal::NoneSent {
      unsent: vec![FileUnsent::Empty {
        name: "a.txt".to_owned()
      }]
    }
    .to_string(),
    "no file went: a.txt: the file is empty"
  );
}

#[test]
fn a_fact_is_its_code_and_fields_on_the_wire_and_english_for_its_agent() {
  let fact = Fact::Health(HealthFact::HandoffLost {
    agent: "agent-a".to_owned(),
    reason: "the window filled".to_owned(),
  });

  assert_eq!(fact.code(), "handoff-lost");
  assert_eq!(
    serde_json::to_value(&fact).unwrap(),
    json!({ "code": "handoff-lost", "agent": "agent-a", "reason": "the window filled" })
  );
  assert_eq!(
    fact.to_string(),
    "The session of agent-a was replaced (the window filled), and neither its handoff nor a transcript of it \
     could be carried to the new session, which starts remembering none of the rooms' recent conversation"
  );
  assert_eq!(Fact::Voice(VoiceFact::BriefingOff).code(), "briefing-off");
  assert_eq!(
    serde_json::from_value::<Fact>(json!({ "code": "briefing-on" })).unwrap(),
    Fact::Voice(VoiceFact::BriefingOn)
  );
  assert_eq!(
    RoomRemovalWarning::Answerers {
      names: vec!["agent-a".to_owned(), "agent-b".to_owned()]
    }
    .to_string(),
    "other agents answer this room: agent-a, agent-b; they stay, and no longer answer in it"
  );
}

#[test]
fn a_fixed_line_is_its_code_and_fields_and_says_whether_it_is_a_system_notice() {
  let stop = FixedLine::GoingDownForUpdate {
    items: vec![12, 14],
  };
  let draw = FixedLine::Draw(DrawLine {
    label: Some("turns".to_owned()),
    asked: DrawAsked::Pick { items: 5, take: 2 },
    result: DrawResult::Pick {
      picked: vec!["c".to_owned(), "b".to_owned()],
    },
    number: 1,
    seed: "00".repeat(32),
    method: "bmo-draw-v1".to_owned(),
    answered: None,
    next: "11".repeat(32),
  });

  assert_eq!(stop.code(), "going-down-for-update");
  assert!(stop.is_system_notice());
  assert!(!FixedLine::ProviderDown.is_system_notice());
  assert_eq!(
    serde_json::to_value(&stop).unwrap(),
    json!({ "code": "going-down-for-update", "items": [12, 14] })
  );
  assert_eq!(draw.code(), "draw");
  assert_eq!(
    serde_json::to_value(&draw).unwrap()["asked"],
    json!({ "shape": "pick", "items": 5, "take": 2 })
  );
}

#[test]
fn a_kind_is_its_kebab_case_word() {
  assert_eq!(serde_json::to_value(SessionReason::DowngradeForced).unwrap(), json!("downgrade-forced"));
  assert_eq!(SessionReason::NoPrefix.to_string(), "no-prefix");
  assert_eq!(SessionReason::Asked.to_string(), "asked");
  assert_eq!(
    serde_json::to_value(RunTitle::VoiceBriefing {
      room: "room-a".to_owned()
    })
    .unwrap(),
    json!({ "kind": "voice-briefing", "room": "room-a" })
  );
  assert_eq!(TurnOpening::FoldDeferred.to_string(), "fold-deferred");
  assert_eq!(SkipReason::ProviderOff.to_string(), "provider-off");
  assert_eq!(
    serde_json::to_value(SupervisorFact::RoomTakenBack {
      label: "agent-a".to_owned(),
      title: "room-a".to_owned()
    })
    .unwrap()["code"],
    json!("room-taken-back")
  );
  assert_eq!(
    VoiceRefusal::Voiceless {
      label: "agent-a".to_owned()
    }
    .code(),
    "voiceless"
  );
  assert_eq!(RoomGrantRefusal::NotOwnersMessage.code(), "not-owners-message");
  assert_eq!(RoomRemovalRefusal::OwnersChat.code(), "owners-chat");
  assert_eq!(
    SupervisorRefusal::Seeded {
      label: "agent-a".to_owned()
    }
    .code(),
    "seeded"
  );
  assert_eq!(
    SupervisorRefusal::UnknownSupervisor {
      supervisor: "agent-a".to_owned()
    }
    .status(),
    StatusCode::NOT_FOUND
  );
}
