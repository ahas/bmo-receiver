//! The web unit's port against a stand-in of the routes the receiver states for it (`docs/boundary-api.md`,
//! 「The web unit」).

use receiver_ports::web::{Checked, HttpWeb, ObjectCheck, OrganisationUnit, Render, User, Web};
use serde_json::{Map, json};

use crate::stand_in::{StandIn, call};

#[tokio::test]
async fn the_account_model_an_object_check_and_a_fixed_line_go_out_as_the_receiver_states_them() {
  let unit = StandIn::start().await;
  let web = HttpWeb::new(&unit.peer("t"));

  unit.answer("GET /accounts/model", 200, json!({ "inForce": true }));
  unit.answer("GET /senders/42/user", 200, json!({ "user": "user-a" }));
  unit.answer(
    "GET /senders/43/user",
    404,
    json!({ "code": "not_found", "error": "No user is linked." }),
  );
  unit.answer("GET /users/user-a", 200, json!({ "user": "user-a", "administrator": true }));
  unit.answer(
    "POST /checks/object",
    200,
    json!({ "allowed": true, "grant": "bmo_worker:w-1#run", "user": "user-a" }),
  );
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

  let object = ObjectCheck {
    kind: "bmo_worker".to_owned(),
    object: "w-1".to_owned(),
    relation: "run".to_owned(),
    user: "user-a".to_owned(),
  };

  assert_eq!(
    web.check_object(&object).await.unwrap(),
    Checked {
      allowed: true,
      grant: "bmo_worker:w-1#run".to_owned(),
      user: Some("user-a".to_owned()),
    }
  );

  let mut fields = Map::new();

  fields.insert("until".to_owned(), json!(32_400));

  let line = Render {
    code: "rest-begins".to_owned(),
    fields,
    language: "en".to_owned(),
  };

  assert_eq!(web.render(&line).await.unwrap(), "Resting until nine.");

  assert_eq!(
    unit.took(),
    [
      call("GET /accounts/model", "", None),
      call("GET /senders/42/user", "", None),
      call("GET /senders/43/user", "", None),
      call("GET /users/user-a", "", None),
      call(
        "POST /checks/object",
        "",
        Some(json!({ "kind": "bmo_worker", "object": "w-1", "relation": "run", "user": "user-a" }))
      ),
      call(
        "POST /words/render",
        "",
        Some(json!({ "code": "rest-begins", "until": 32400, "language": "en" }))
      ),
    ]
  );
}

#[tokio::test]
async fn an_organisation_unit_is_read_as_its_home_members_then_its_post_holders_each_once() {
  let unit = StandIn::start().await;
  let web = HttpWeb::new(&unit.peer("t"));

  unit.answer(
    "GET /units/5",
    200,
    json!({
      "id": 5, "name": "Design", "company": { "id": 1, "name": "Co" }, "path": [], "children": [],
      "heads": ["7"], "members": ["7", "8"], "roles": [], "acting": null,
      "concurrent": [{ "user": "9", "head": false }, { "user": "8", "head": true }],
    }),
  );
  unit.answer("GET /units/6", 404, json!({ "code": "not-found", "error": "No such unit." }));

  assert_eq!(
    web.organisation_unit(5).await.unwrap(),
    Some(OrganisationUnit {
      id: 5,
      name: "Design".to_owned(),
      people: vec![7, 8, 9],
    })
  );
  assert_eq!(web.organisation_unit(6).await.unwrap(), None, "a unit the web unit does not hold");
}

#[tokio::test]
async fn a_users_company_is_the_company_its_unit_names_and_none_for_a_user_placed_in_no_unit() {
  let unit = StandIn::start().await;
  let web = HttpWeb::new(&unit.peer("t"));

  unit.answer(
    "GET /users/7/unit",
    200,
    json!({
      "user": "7", "unit": { "id": 5, "name": "Design" }, "company": { "id": 1, "name": "Co" },
      "path": [{ "id": 1, "name": "Co" }, { "id": 5, "name": "Design" }],
    }),
  );
  unit.answer("GET /users/8/unit", 404, json!({ "code": "not-found", "error": "No such user." }));

  assert_eq!(web.company_of("7").await.unwrap(), Some(1));
  assert_eq!(web.company_of("8").await.unwrap(), None, "a user placed in no unit");
}
