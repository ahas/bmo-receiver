//! The web unit's port against a stand-in of the routes the receiver states for it (`docs/boundary-api.md`,
//! 「The web unit」).

use receiver_ports::web::{
  Checked, Grant, GrantMessage, Grantee, HttpWeb, MessageCheck, ObjectCheck, Render, RoomCheck, User, Web,
};
use serde_json::{Map, json};

use crate::stand_in::{StandIn, call};

fn checked(user: Option<&str>) -> Checked {
  Checked {
    allowed: true,
    grant: "room:-7#talk".to_owned(),
    user: user.map(str::to_owned),
    sender: None,
  }
}

#[tokio::test]
async fn the_account_model_the_checks_the_grants_and_a_fixed_line_go_out_as_the_receiver_states_them() {
  let unit = StandIn::start().await;
  let web = HttpWeb::new(&unit.peer("t"));
  let allowed = json!({ "allowed": true, "grant": "room:-7#talk" });

  unit.answer("GET /accounts/model", 200, json!({ "in_force": true }));
  unit.answer("GET /senders/42/user", 200, json!({ "user": "user-a" }));
  unit.answer(
    "GET /senders/43/user",
    404,
    json!({ "code": "not_found", "error": "No user is linked." }),
  );
  unit.answer("GET /users/user-a", 200, json!({ "user": "user-a", "administrator": true }));
  unit.answer(
    "POST /checks/room",
    200,
    json!({ "allowed": true, "grant": "room:-7#talk", "user": "user-a" }),
  );
  unit.answer("POST /checks/message", 200, allowed.clone());
  unit.answer("POST /checks/object", 200, allowed);
  unit.answer("POST /grants", 200, json!({ "grant": "room:-7#talk" }));
  unit.answer("DELETE /grants", 200, json!({ "grant": "room:-7#talk" }));
  unit.answer("POST /words/render", 200, json!({ "text": "Resting until nine." }));

  assert!(web.model_in_force().await.unwrap());
  assert_eq!(web.user_of_sender(42).await.unwrap().as_deref(), Some("user-a"));
  assert_eq!(
    web.user_of_sender(43).await.unwrap(),
    None,
    "a sender linked to no user is found as none"
  );
  assert_eq!(
    web.user("user-a").await.unwrap(),
    User {
      user: "user-a".to_owned(),
      administrator: true,
    }
  );

  let room = RoomCheck {
    room: -7,
    sender: Some(42),
    relation: "talk".to_owned(),
  };
  let message = MessageCheck {
    room: -7,
    message: 11,
    relation: "talk".to_owned(),
  };
  let object = ObjectCheck {
    kind: "bmo_worker".to_owned(),
    object: "w-1".to_owned(),
    relation: "run".to_owned(),
    user: "user-a".to_owned(),
  };

  assert_eq!(web.check_room(&room).await.unwrap(), checked(Some("user-a")));
  assert_eq!(web.check_message(&message).await.unwrap(), checked(None));
  assert_eq!(web.check_object(&object).await.unwrap(), checked(None));

  let mut grant = Grant {
    room: Some(-7),
    kind: None,
    object: None,
    relation: "talk".to_owned(),
    to: Grantee {
      sender: Some(43),
      user: None,
      members: false,
    },
    words: Some("you may talk".to_owned()),
    expires_at: None,
    message: GrantMessage { room: -7, id: 11 },
  };

  assert_eq!(web.grant(&grant).await.unwrap(), "room:-7#talk");
  grant.words = None;
  assert_eq!(web.revoke(&grant).await.unwrap(), "room:-7#talk");

  let mut fields = Map::new();

  fields.insert("until".to_owned(), json!(32_400));

  let line = Render {
    code: "rest-begins".to_owned(),
    fields,
    language: "en".to_owned(),
  };

  assert_eq!(web.render(&line).await.unwrap(), "Resting until nine.");

  let granted = json!({
    "room": -7,
    "relation": "talk",
    "to": { "sender": 43, "members": false },
    "words": "you may talk",
    "message": { "room": -7, "id": 11 },
  });
  let revoked = json!({
    "room": -7,
    "relation": "talk",
    "to": { "sender": 43, "members": false },
    "message": { "room": -7, "id": 11 },
  });

  assert_eq!(
    unit.took(),
    [
      call("GET /accounts/model", "", None),
      call("GET /senders/42/user", "", None),
      call("GET /senders/43/user", "", None),
      call("GET /users/user-a", "", None),
      call("POST /checks/room", "", Some(json!({ "room": -7, "sender": 42, "relation": "talk" }))),
      call(
        "POST /checks/message",
        "",
        Some(json!({ "room": -7, "message": 11, "relation": "talk" }))
      ),
      call(
        "POST /checks/object",
        "",
        Some(json!({ "kind": "bmo_worker", "object": "w-1", "relation": "run", "user": "user-a" }))
      ),
      call("POST /grants", "", Some(granted)),
      call("DELETE /grants", "", Some(revoked)),
      call(
        "POST /words/render",
        "",
        Some(json!({ "code": "rest-begins", "until": 32400, "language": "en" }))
      ),
    ]
  );
}
