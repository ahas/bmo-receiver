//! The cache warmer's port against a stand-in of the warmer's routes (the warmer's `docs/api.md`).

use receiver_ports::warmer::{HttpWarmer, Target, Used, Warmer};
use serde_json::json;

use crate::stand_in::{StandIn, call};

#[tokio::test]
async fn the_consumer_its_target_and_a_use_go_out_with_no_token() {
  let unit = StandIn::start().await;
  let warmer = HttpWarmer::new(&unit.peer(""));

  unit.answer(
    "PUT /consumers/receiver",
    201,
    json!({ "consumer": "receiver", "hook_url": "http://r/warmer/ping" }),
  );
  unit.answer("PUT /consumers/receiver/targets/chat-touch", 201, json!({ "target": "chat-touch" }));
  unit.answer_empty("POST /consumers/receiver/targets/chat-touch/used", 204);

  let target = Target {
    kind: "chat_touch".to_owned(),
    label: "the chat agent's one session".to_owned(),
    subject: Some("agent-a".to_owned()),
    rank: 90,
    idle_ms: 2_400_000,
    lifetime_ms: Some(3_600_000),
    margin_ms: 0,
    ping_bound_ms: 1_800_000,
    group: None,
    warm_by_default: Some(true),
    failure_touches: false,
    honours_idle_override: true,
    last_used_at: None,
  };

  warmer.register("receiver", "http://r/warmer/ping?token=t").await.unwrap();
  warmer.target("receiver", "chat-touch", &target).await.unwrap();
  warmer
    .used(
      "receiver",
      "chat-touch",
      Used {
        at: Some(2_000),
        started_at: Some(1_000),
      },
    )
    .await
    .unwrap();

  assert_eq!(
    unit.took(),
    [
      call(
        "PUT /consumers/receiver",
        "",
        Some(json!({ "hook_url": "http://r/warmer/ping?token=t" }))
      ),
      call(
        "PUT /consumers/receiver/targets/chat-touch",
        "",
        Some(json!({
          "kind": "chat_touch",
          "label": "the chat agent's one session",
          "subject": "agent-a",
          "rank": 90,
          "idle_ms": 2_400_000,
          "lifetime_ms": 3_600_000,
          "margin_ms": 0,
          "ping_bound_ms": 1_800_000,
          "warm_by_default": true,
          "failure_touches": false,
          "honours_idle_override": true,
        }))
      ),
      call(
        "POST /consumers/receiver/targets/chat-touch/used",
        "",
        Some(json!({ "at": 2000, "started_at": 1000 }))
      ),
    ]
  );
  assert!(unit.calls().iter().all(|call| call.authorization.is_none()));
}
