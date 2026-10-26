//! The talks' composition (docs/boundary-api.md, 「For bmo-job」, `POST /talks`): a turn restates only the last
//! few turns, each clipped, and a subject's head names what it is about.

use bmo_receiver::chat::talk::{
  DocumentTopic, ItemTopic, SaidTurn, clip, document_request, item_request, prompt,
};
use receiver_policy::talk::TALK;

#[test]
fn a_turn_restates_only_the_last_few_turns_each_clipped() {
  let turns: Vec<SaidTurn> = (0..TALK.turns_given + 2)
    .map(|at| SaidTurn {
      owner: at % 2 == 0,
      text: format!("turn {at}"),
    })
    .collect();
  let written = prompt(&["HEAD".to_owned()], &turns, "now", "Owner", "Agent A");

  assert!(!written.contains("turn 0") && !written.contains("turn 1\n"));
  assert!(written.ends_with(&format!("{}\nnow", TALK.says_now.replace("{owner}", "Owner"))));
  assert_eq!(clip("abcdef", 3), "abc…");
}

#[test]
fn an_item_turn_opens_on_the_item_by_its_number_title_and_state() {
  let request = item_request(
    &ItemTopic {
      id: 900,
      title: "Talk about items".into(),
      done: false,
      merged_into: None,
      held: true,
      hold: Some("pause".into()),
      stopping: true,
      approved: true,
      status: "waiting".into(),
    },
    vec![],
    "Where is it?".into(),
  );

  assert!(
    request
      .head
      .iter()
      .any(|line| line.contains("item #900") && line.contains("winding down"))
  );
}

#[test]
fn a_document_turn_carries_the_passage_selected_under_its_heading() {
  let request = document_request(
    &DocumentTopic {
      document: "docs/wake.md".into(),
      label: "docs".into(),
      writer: "the jobs".into(),
      file: "/repo/docs/wake.md".into(),
      version: "a".repeat(64),
      heading: Some("The window".into()),
      passage: Some("A room that woke the agent keeps a window.".into()),
      translated: true,
    },
    vec![],
    "Why three minutes?".into(),
  );
  let written = &request.head;

  assert!(written.iter().any(|line| line.contains("docs/wake.md")));
  assert!(written.join("\n").contains("A room that woke the agent keeps a window."));
  assert!(written.join("\n").contains("The window"));
}
