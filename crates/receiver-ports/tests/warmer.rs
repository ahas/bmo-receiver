//! The cache warmer's port against a stand-in of the warmer's published routes (the warmer's `docs/api.md`).

use receiver_ports::warmer::{HttpWarmer, Target, Used, Warmer};
use serde_json::json;

use crate::stand_in::{StandIn, call};

#[tokio::test]
async fn the_consumer_its_target_and_a_use_go_out_as_the_warmer_publishes_them_behind_its_token() {
  let unit = StandIn::start().await;
  let warmer = HttpWarmer::new(&unit.peer("t"));

  unit.answer(
    "PUT /consumers/receiver",
    201,
    json!({ "id": 1, "name": "receiver", "url": "http://r/warmer/ping", "createdAt": 1 }),
  );
  unit.answer("PUT /consumers/receiver/targets/chat-touch", 201, json!({ "id": 2 }));
  unit.answer("POST /consumers/receiver/targets/chat-touch/uses", 201, json!({ "at": 2000 }));

  let target = Target {
    kind: "chat-touch".to_owned(),
    label: "the chat agent's one session".to_owned(),
    subject_name: Some("agent-a".to_owned()),
    rank: 90,
    idle_ms: 2_400_000,
    ttl_ms: Some(3_600_000),
    margin_ms: 0,
    ping_bound_ms: 1_800_000,
    group_name: None,
    warm_by_default: Some(true),
    failure_touches: false,
    honours_idle_override: true,
    used_at: None,
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
      call("PUT /consumers/receiver", "", Some(json!({ "url": "http://r/warmer/ping?token=t" }))),
      call(
        "PUT /consumers/receiver/targets/chat-touch",
        "",
        Some(json!({
          "kind": "chat-touch",
          "label": "the chat agent's one session",
          "subjectName": "agent-a",
          "rank": 90,
          "idleMs": 2_400_000,
          "ttlMs": 3_600_000,
          "marginMs": 0,
          "pingBoundMs": 1_800_000,
          "warmByDefault": true,
          "failureTouches": false,
          "honoursIdleOverride": true,
        }))
      ),
      call(
        "POST /consumers/receiver/targets/chat-touch/uses",
        "",
        Some(json!({ "at": 2000, "startedAt": 1000 }))
      ),
    ]
  );
  assert!(unit.calls().iter().all(|call| call.authorization.as_deref() == Some("Bearer t")));
}
