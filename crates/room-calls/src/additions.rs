//! A chat-service room added in `record-only` mode on the owner's word in the turn (docs/api.md, 「Rooms」;
//! docs/chat-mcp.md, 「The rooms tool」): the receiver's own `POST /rooms`, its rooms and contacts listed by name
//! where the room is not named exactly, and the one mode this road never sets.

use std::collections::BTreeMap;

use receiver_client::{
  AccountContact, AccountRoom, AddRoom, ClientFailure, Provider, ReceiverClient, Room, RoomMode, StartRoom,
};

/// The turn variable the receiver marks every new message of the turn the owner's by.
pub const PRIVILEGED_VARIABLE: &str = "BMO_TURN_PRIVILEGED";

/// The one mode a room added by this road takes.
pub const MODE: RoomMode = RoomMode::RecordOnly;

/// What a call made outside the owner's word answers with: no grant names adding a room.
pub const NOT_OWNER: &str = "A room is added on the owner's word in the turn alone: this turn is not the owner's, and no grant in force names it.";

/// How many rooms or contacts one page asks a provider for.
const PAGE: i64 = 100;

/// Whether the call is made on the owner's word: the receiver marked every new message of the turn the owner's.
pub fn admitted(variables: &BTreeMap<String, String>) -> Result<(), String> {
  let owner = variables.get(PRIVILEGED_VARIABLE).is_some_and(|flag| flag.trim() == "1");

  if owner { Ok(()) } else { Err(NOT_OWNER.to_owned()) }
}

/// The one choice a call makes among the room, a contact and the contacts of a group; none where it names none,
/// which lists the provider's rooms and contacts for the owner to pick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Chosen {
  Room(i64),
  Contact(i64),
  Group { contact_ids: Vec<i64>, title: String },
}

/// The room the call names, or none where it names none; refused where it names more than one, and where a group
/// names no title.
pub fn chosen(
  room_id: Option<i64>,
  contact_id: Option<i64>,
  contact_ids: Option<Vec<i64>>,
  title: Option<&str>,
) -> Result<Option<Chosen>, String> {
  let title = title.map(str::trim).filter(|title| !title.is_empty());
  let named =
    usize::from(room_id.is_some()) + usize::from(contact_id.is_some()) + usize::from(contact_ids.is_some());

  match (named, room_id, contact_id, contact_ids) {
    (0, None, None, None) => Ok(None),
    (1, Some(room), None, None) => Ok(Some(Chosen::Room(room))),
    (1, None, Some(contact), None) => Ok(Some(Chosen::Contact(contact))),
    (1, None, None, Some(contact_ids)) => match title {
      Some(title) => Ok(Some(Chosen::Group {
        contact_ids,
        title: title.to_owned(),
      })),
      None => Err("A group to start needs a title.".to_owned()),
    },
    _ => Err("A room to add names one of roomId, contactId or contactIds.".to_owned()),
  }
}

/// The room added through the receiver's own `POST /rooms`, in the one mode this road sets; its answer is the room
/// as the receiver now holds it, and its refusals are the receiver's own sentences.
pub async fn add(
  client: &ReceiverClient,
  provider: &str,
  chosen: &Chosen,
  title: Option<&str>,
) -> Result<Room, ClientFailure> {
  let title = title.map(str::trim).filter(|title| !title.is_empty()).map(str::to_owned);

  match chosen {
    Chosen::Room(room_id) => {
      client
        .add_room(&AddRoom {
          provider: provider.to_owned(),
          room_id: *room_id,
          mode: MODE,
          title,
        })
        .await
    }
    Chosen::Contact(contact_id) => {
      client
        .start_room(&StartRoom {
          provider: provider.to_owned(),
          mode: MODE,
          contact_id: Some(*contact_id),
          contact_ids: None,
          title,
          account_id: None,
        })
        .await
    }
    Chosen::Group {
      contact_ids,
      title: group,
    } => {
      client
        .start_room(&StartRoom {
          provider: provider.to_owned(),
          mode: MODE,
          contact_id: None,
          contact_ids: Some(contact_ids.clone()),
          title: Some(group.clone()),
          account_id: None,
        })
        .await
    }
  }
}

/// The rooms and contacts a provider's account holds, as the console's room-add dialog reads them.
#[derive(Clone, Debug)]
pub struct Candidates {
  pub provider: Provider,
  pub rooms: Vec<AccountRoom>,
  pub contacts: Vec<AccountContact>,
}

/// Every room and contact the provider lists for its account, page by page; a provider that lists neither answers
/// with both lists empty. A key no chat service stands under is refused in the receiver's own sentence, since
/// `POST /rooms` would refuse it the same way.
pub async fn candidates(client: &ReceiverClient, key: &str) -> Result<Candidates, String> {
  let providers = client.providers().await.map_err(|why| why.to_string())?;
  let provider = providers
    .items
    .into_iter()
    .find(|provider| provider.key == key)
    .ok_or_else(|| format!("No chat service stands under {key}."))?;

  let mut rooms = Vec::new();

  if provider.lists_rooms {
    let mut after = None;

    loop {
      let page = client.account_rooms(key, after, Some(PAGE)).await.map_err(|why| why.to_string())?;
      let last = page.items.last().map(|room| room.id);

      rooms.extend(page.items);

      if !page.more || last.is_none() || last == after {
        break;
      }
      after = last;
    }
  }

  let mut contacts = Vec::new();

  if provider.lists_contacts {
    let mut after = None;

    loop {
      let page = client
        .account_contacts(key, after, Some(PAGE), None)
        .await
        .map_err(|why| why.to_string())?;
      let last = page.items.last().map(|contact| contact.id);

      contacts.extend(page.items);

      if !page.more || last.is_none() || last == after {
        break;
      }
      after = last;
    }
  }

  Ok(Candidates {
    provider,
    rooms,
    contacts,
  })
}

/// The candidates as the lines an agent reads to the owner: each room and contact by the name the provider gives it,
/// with whether the receiver already holds it and the mode it was added in.
pub fn lines(candidates: &Candidates) -> Vec<String> {
  let provider = &candidates.provider;
  let mut lines = vec![format!("{} ({}):", provider.key, provider.label)];

  if provider.lists_rooms {
    lines.push("rooms:".to_owned());

    if candidates.rooms.is_empty() {
      lines.push("- none".to_owned());
    }

    for room in &candidates.rooms {
      let title = match room.title.as_deref() {
        Some(title) => format!("\"{title}\""),
        None => "(no title)".to_owned(),
      };

      lines.push(format!("- {} {title}, {}, {}", room.id, room.kind, addition(room.held, room.mode)));
    }
  } else {
    lines.push("rooms: not listed by this chat service".to_owned());
  }

  if provider.lists_contacts {
    lines.push("contacts:".to_owned());

    if candidates.contacts.is_empty() {
      lines.push("- none".to_owned());
    }

    for contact in &candidates.contacts {
      let room = match contact.room_id {
        Some(room) => format!("room {room}"),
        None => "no room yet".to_owned(),
      };

      lines.push(format!(
        "- {} {}, {}, {room}, {}",
        contact.id,
        contact.name,
        contact.kind,
        addition(contact.held, contact.mode)
      ));
    }
  } else {
    lines.push("contacts: not listed by this chat service".to_owned());
  }

  lines
}

// Whether the receiver already holds a room, and the mode it was added in.
fn addition(held: bool, mode: Option<RoomMode>) -> String {
  match mode {
    Some(mode) => format!("added already in {}", mode.as_str()),
    None if held => "held, never added".to_owned(),
    None => "not added".to_owned(),
  }
}
