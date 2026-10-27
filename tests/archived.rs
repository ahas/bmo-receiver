//! The archived block's cap (docs/bmo-window.md, 「What every turn carries」): a turn carries the newest unread
//! messages in their order, as many as the policy sets, and counts the older ones it left in the archive.

use bmo_receiver::chat::prompt::{archived_head, carried_unread};
use bmo_receiver::receiver::Inbound;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;

fn unread(message_id: i64) -> Inbound {
  Inbound {
    chat_id: 1,
    message_id,
    sender_id: Some(2),
    sender_name: "tester".into(),
    sent_at: 1_760_000_000 + message_id,
    text: format!("words {message_id}"),
    image_path: None,
    transcript: None,
    voice_seconds: None,
    voice_state: None,
    reply_to_message_id: None,
    via: "road-a".into(),
    mode: None,
    via_reader: None,
    album: None,
  }
}

fn ids(messages: &[Inbound]) -> Vec<i64> {
  messages.iter().map(|message| message.message_id).collect()
}

#[test]
fn the_block_carries_a_hundred_unread_messages_by_default() {
  assert_eq!(CHAT_ROUND.archived_messages, 100);
}

#[test]
fn more_unread_than_the_cap_carries_the_newest_in_order_and_counts_the_rest() {
  let (carried, left_out) = carried_unread((1..=7).map(unread).collect(), 4);

  assert_eq!(ids(&carried), vec![4, 5, 6, 7]);
  assert_eq!(left_out, 3);

  let head = archived_head(carried.len(), left_out);

  assert_eq!(head.len(), 2, "{head:?}");
  assert_eq!(head[0], fill_marks(&CHAT_ROUND.blocks.archived, &[("count", "4")]));
  assert_eq!(head[1], fill_marks(&CHAT_ROUND.blocks.archived_left_out, &[("count", "3")]));
  assert!(head[1].starts_with("3 older unread messages"), "{}", head[1]);
}

#[test]
fn unread_within_the_cap_are_carried_whole_with_no_line_left_out() {
  let (carried, left_out) = carried_unread((1..=4).map(unread).collect(), 4);

  assert_eq!(ids(&carried), vec![1, 2, 3, 4]);
  assert_eq!(left_out, 0);
  assert_eq!(
    archived_head(carried.len(), left_out),
    vec![fill_marks(&CHAT_ROUND.blocks.archived, &[("count", "4")])]
  );

  let (none, left_out) = carried_unread(vec![], 4);

  assert!(none.is_empty());
  assert_eq!(left_out, 0);
}
