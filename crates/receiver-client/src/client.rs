//! One call for each route of the receiver's fixed API, behind its token (`docs/api.md`).

use reqwest::{Client, Method, RequestBuilder, StatusCode, Url};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::types::*;
use crate::wire::{Answer, ClientFailure, Page, WireError};

/// The receiver at one base address, called with one bearer token.
#[derive(Debug, Clone)]
pub struct ReceiverClient {
  http: Client,
  base: Url,
  token: String,
}

impl ReceiverClient {
  /// The receiver whose routes stand at `base`, called with `token`; `None` where the address does not read.
  pub fn new(base: &str, token: impl Into<String>) -> Result<Self, String> {
    let base = Url::parse(base).map_err(|why| format!("{base} is no address: {why}"))?;
    if base.cannot_be_a_base() {
      return Err(format!("{base} is no base address"));
    }

    Ok(Self {
      http: Client::new(),
      base,
      token: token.into(),
    })
  }

  /// The same receiver called through another HTTP client, such as one with a timeout of its own.
  pub fn with_http(mut self, http: Client) -> Self {
    self.http = http;
    self
  }

  fn url(&self, segments: &[&str]) -> Url {
    let mut url = self.base.clone();
    {
      let mut path = url.path_segments_mut().expect("the base address takes segments");
      path.pop_if_empty();
      path.extend(segments);
    }
    url
  }

  fn request(&self, method: Method, segments: &[&str]) -> RequestBuilder {
    self.http.request(method, self.url(segments)).bearer_auth(&self.token)
  }

  async fn answered<T: DeserializeOwned>(request: RequestBuilder) -> Answer<T> {
    let answer = Self::sent(request).await?;
    let bytes = answer.bytes().await.map_err(|why| ClientFailure::Unreadable(why.to_string()))?;
    serde_json::from_slice(&bytes).map_err(|why| ClientFailure::Unreadable(why.to_string()))
  }

  async fn sent(request: RequestBuilder) -> Answer<reqwest::Response> {
    let answer = request.send().await.map_err(|why| ClientFailure::Unreachable(why.to_string()))?;
    let status = answer.status();
    if status.is_success() {
      return Ok(answer);
    }
    let body = answer.bytes().await.unwrap_or_default();

    Err(ClientFailure::Answered {
      status: status.as_u16(),
      answer: serde_json::from_slice(&body).ok(),
    })
  }

  async fn nothing(request: RequestBuilder) -> Answer<()> {
    let answer = Self::sent(request).await?;

    match answer.status() {
      StatusCode::NO_CONTENT => Ok(()),
      status => Err(ClientFailure::Unreadable(format!("{status} where 204 was due"))),
    }
  }

  async fn bytes(request: RequestBuilder) -> Answer<Vec<u8>> {
    let answer = Self::sent(request).await?;

    answer
      .bytes()
      .await
      .map(|bytes| bytes.to_vec())
      .map_err(|why| ClientFailure::Unreadable(why.to_string()))
  }

  fn get<Q: Serialize>(&self, segments: &[&str], query: &Q) -> RequestBuilder {
    self.request(Method::GET, segments).query(query)
  }

  fn send<B: Serialize>(&self, method: Method, segments: &[&str], body: &B) -> RequestBuilder {
    self.request(method, segments).json(body)
  }

  /// `GET /health`.
  pub async fn health(&self) -> Answer<Health> {
    Self::answered(self.request(Method::GET, &["health"])).await
  }

  /// `GET /owner`.
  pub async fn owner(&self) -> Answer<Owner> {
    Self::answered(self.request(Method::GET, &["owner"])).await
  }

  /// `GET /rooms`.
  pub async fn rooms(&self) -> Answer<Page<Room>> {
    Self::answered(self.request(Method::GET, &["rooms"])).await
  }

  /// `POST /rooms`.
  pub async fn add_room(&self, room: &AddRoom) -> Answer<Room> {
    Self::answered(self.send(Method::POST, &["rooms"], room)).await
  }

  /// `POST /rooms`, starting a conversation on the chat service and adding the room it comes to.
  pub async fn start_room(&self, start: &StartRoom) -> Answer<Room> {
    Self::answered(self.send(Method::POST, &["rooms"], start)).await
  }

  /// `GET /rooms/{roomId}`.
  pub async fn room(&self, room_id: i64) -> Answer<Room> {
    Self::answered(self.request(Method::GET, &["rooms", &room_id.to_string()])).await
  }

  /// `GET /rooms/{roomId}/registration`.
  pub async fn room_registration(&self, room_id: i64, by: &str) -> Answer<RoomRegistration> {
    #[derive(Serialize)]
    struct Asked<'a> {
      by: &'a str,
    }

    let room = room_id.to_string();

    Self::answered(self.get(&["rooms", &room, "registration"], &Asked { by })).await
  }

  /// `PUT /rooms/{roomId}/registration`.
  pub async fn change_room_mode(&self, room_id: i64, by: &str, mode: RoomMode) -> Answer<RoomRegistration> {
    #[derive(Serialize)]
    struct Asked<'a> {
      by: &'a str,
    }

    let room = room_id.to_string();
    let changing = ChangeRoomMode { mode };

    Self::answered(
      self
        .request(Method::PUT, &["rooms", &room, "registration"])
        .query(&Asked { by })
        .json(&changing),
    )
    .await
  }

  /// `GET /rooms/{roomId}/leaders`.
  pub async fn room_leaders(&self, room_id: i64) -> Answer<RoomLeaders> {
    Self::answered(self.request(Method::GET, &["rooms", &room_id.to_string(), "leaders"])).await
  }

  /// `GET /rooms/{roomId}/archive`.
  pub async fn room_archive(&self, room_id: i64) -> Answer<RoomArchive> {
    Self::answered(self.request(Method::GET, &["rooms", &room_id.to_string(), "archive"])).await
  }

  /// `PUT /rooms/{roomId}/archive`.
  pub async fn change_room_archive(&self, room_id: i64, by: &str, archived: bool) -> Answer<RoomArchive> {
    #[derive(Serialize)]
    struct Asked<'a> {
      by: &'a str,
    }

    let room = room_id.to_string();

    Self::answered(
      self
        .request(Method::PUT, &["rooms", &room, "archive"])
        .query(&Asked { by })
        .json(&ChangeRoomArchive { archived }),
    )
    .await
  }

  /// `GET /rooms/{roomId}/title`.
  pub async fn room_title(&self, room_id: i64) -> Answer<RoomTitle> {
    Self::answered(self.request(Method::GET, &["rooms", &room_id.to_string(), "title"])).await
  }

  /// `PUT /rooms/{roomId}/title`, for a room on the record alone.
  pub async fn change_room_title(&self, room_id: i64, title: &str) -> Answer<RoomTitle> {
    let room = room_id.to_string();

    Self::answered(self.request(Method::PUT, &["rooms", &room, "title"]).json(&ChangeRoomTitle {
      title: title.to_owned(),
    }))
    .await
  }

  /// `GET /rooms/{roomId}/answerers`.
  pub async fn answerers(&self, room_id: i64) -> Answer<Answerers> {
    Self::answered(self.request(Method::GET, &["rooms", &room_id.to_string(), "answerers"])).await
  }

  /// `GET /answerers`, narrowed to one room where one is named.
  pub async fn all_answerers(&self, room_id: Option<i64>) -> Answer<Page<RoomAnswerers>> {
    let request = match room_id {
      Some(room_id) => self.get(&["answerers"], &[("room", room_id.to_string())]),
      None => self.request(Method::GET, &["answerers"]),
    };

    Self::answered(request).await
  }

  /// `GET /rooms/{roomId}/messages`.
  pub async fn messages(&self, room_id: i64, query: &MessageQuery) -> Answer<Page<Message>> {
    Self::answered(self.get(&["rooms", &room_id.to_string(), "messages"], query)).await
  }

  /// `GET /supervisors/{supervisorName}/rooms/{roomId}/messages`.
  pub async fn supervisor_room_messages(
    &self,
    supervisor: &str,
    room_id: i64,
    query: &MemberRead,
  ) -> Answer<Page<Message>> {
    Self::answered(self.get(&["supervisors", supervisor, "rooms", &room_id.to_string(), "messages"], query))
      .await
  }

  /// `GET /rooms/{roomId}/messages/{messageId}`.
  pub async fn message(&self, room_id: i64, message_id: i64) -> Answer<Message> {
    Self::answered(
      self.request(Method::GET, &["rooms", &room_id.to_string(), "messages", &message_id.to_string()]),
    )
    .await
  }

  /// `PUT /rooms/{roomId}/messages/{messageId}/speaker`: a line under an agent's own number filed as that agent's.
  pub async fn change_speaker(&self, room_id: i64, message_id: i64, agent_name: &str) -> Answer<Message> {
    let path = ["rooms", &room_id.to_string(), "messages", &message_id.to_string(), "speaker"];

    Self::answered(self.request(Method::PUT, &path).json(&ChangeSpeaker {
      agent_name: agent_name.to_owned(),
    }))
    .await
  }

  /// `GET /rooms/{roomId}/messages/{messageId}/metadata`.
  pub async fn message_metadata(&self, room_id: i64, message_id: i64) -> Answer<MessageMetadata> {
    Self::answered(self.request(
      Method::GET,
      &["rooms", &room_id.to_string(), "messages", &message_id.to_string(), "metadata"],
    ))
    .await
  }

  /// `GET /rooms/{roomId}/messages/{messageId}/files`.
  pub async fn message_files(&self, room_id: i64, message_id: i64) -> Answer<Page<FileEntry>> {
    Self::answered(self.request(
      Method::GET,
      &["rooms", &room_id.to_string(), "messages", &message_id.to_string(), "files"],
    ))
    .await
  }

  /// `POST /rooms/{roomId}/file-fills`.
  pub async fn fill_files(&self, room_id: i64, fill: &FileFill) -> Answer<FileFills> {
    Self::answered(self.send(Method::POST, &["rooms", &room_id.to_string(), "file-fills"], fill)).await
  }

  /// `GET /rooms/{roomId}/messages/{messageId}/files/{place}`.
  pub async fn message_file(
    &self,
    room_id: i64,
    message_id: i64,
    place: usize,
    download: bool,
  ) -> Answer<Vec<u8>> {
    let mut request = self.request(
      Method::GET,
      &[
        "rooms",
        &room_id.to_string(),
        "messages",
        &message_id.to_string(),
        "files",
        &place.to_string(),
      ],
    );
    if download {
      request = request.query(&[("download", "true")]);
    }

    Self::bytes(request).await
  }

  /// `GET /rooms/{roomId}/messages/{messageId}/picture`.
  pub async fn message_picture(&self, room_id: i64, message_id: i64) -> Answer<Vec<u8>> {
    Self::bytes(self.request(
      Method::GET,
      &["rooms", &room_id.to_string(), "messages", &message_id.to_string(), "picture"],
    ))
    .await
  }

  /// `GET /rooms/{roomId}/messages/{messageId}/sender-is-owner`.
  pub async fn sender_is_owner(&self, room_id: i64, message_id: i64) -> Answer<Value> {
    Self::answered(self.request(
      Method::GET,
      &["rooms", &room_id.to_string(), "messages", &message_id.to_string(), "sender-is-owner"],
    ))
    .await
  }

  /// `GET /rooms/{roomId}/day-start`.
  pub async fn day_start(&self, room_id: i64, from: i64) -> Answer<Value> {
    Self::answered(self.get(&["rooms", &room_id.to_string(), "day-start"], &[("from", from.to_string())]))
      .await
  }

  /// `GET /rooms/{roomId}/pictures`.
  pub async fn pictures(&self, room_id: i64, query: &PictureQuery) -> Answer<Pictures> {
    Self::answered(self.get(&["rooms", &room_id.to_string(), "pictures"], query)).await
  }

  /// `GET /avatars`.
  pub async fn avatars(&self, query: &AvatarQuery) -> Answer<Avatars> {
    Self::answered(self.get(&["avatars"], query)).await
  }

  /// `GET /rooms/{roomId}/avatar`.
  pub async fn room_avatar(&self, room_id: i64) -> Answer<Vec<u8>> {
    Self::bytes(self.request(Method::GET, &["rooms", &room_id.to_string(), "avatar"])).await
  }

  /// `GET /people/{personId}/avatar`.
  pub async fn person_avatar(&self, person_id: i64) -> Answer<Vec<u8>> {
    Self::bytes(self.request(Method::GET, &["people", &person_id.to_string(), "avatar"])).await
  }

  /// `GET /rooms/{roomId}/figures`.
  pub async fn figures(&self, room_id: i64, zone: Option<&str>) -> Answer<Figures> {
    let request = match zone {
      Some(zone) => self.get(&["rooms", &room_id.to_string(), "figures"], &[("zone", zone)]),
      None => self.request(Method::GET, &["rooms", &room_id.to_string(), "figures"]),
    };

    Self::answered(request).await
  }

  /// `GET /rooms/{roomId}/window`.
  pub async fn window(&self, room_id: i64) -> Answer<Window> {
    Self::answered(self.request(Method::GET, &["rooms", &room_id.to_string(), "window"])).await
  }

  /// `GET /rooms/{roomId}/situation`.
  pub async fn situation(&self, room_id: i64) -> Answer<RoomSituation> {
    Self::answered(self.request(Method::GET, &["rooms", &room_id.to_string(), "situation"])).await
  }

  /// `GET /providers`.
  pub async fn providers(&self) -> Answer<Page<Provider>> {
    Self::answered(self.request(Method::GET, &["providers"])).await
  }

  /// `GET /providers/{key}/rooms`.
  pub async fn account_rooms(
    &self,
    key: &str,
    after: Option<i64>,
    limit: Option<i64>,
  ) -> Answer<Page<AccountRoom>> {
    let query = [("after", after), ("limit", limit)];
    let given: Vec<(&str, i64)> =
      query.iter().filter_map(|(name, value)| value.map(|value| (*name, value))).collect();

    Self::answered(self.get(&["providers", key, "rooms"], &given)).await
  }

  /// `GET /providers/{key}/contacts`: the contacts of the service's own account, or of the account `account_id`
  /// names.
  pub async fn account_contacts(
    &self,
    key: &str,
    after: Option<i64>,
    limit: Option<i64>,
    account_id: Option<i64>,
  ) -> Answer<Page<AccountContact>> {
    let query = [("after", after), ("limit", limit), ("accountId", account_id)];
    let given: Vec<(&str, i64)> =
      query.iter().filter_map(|(name, value)| value.map(|value| (*name, value))).collect();

    Self::answered(self.get(&["providers", key, "contacts"], &given)).await
  }

  /// `GET /rooms/{roomId}/providers`.
  pub async fn room_providers(&self, room_id: i64) -> Answer<RoomProviders> {
    Self::answered(self.request(Method::GET, &["rooms", &room_id.to_string(), "providers"])).await
  }

  /// `GET /rooms/{roomId}/members`.
  pub async fn room_members(&self, room_id: i64) -> Answer<RoomMembers> {
    Self::answered(self.request(Method::GET, &["rooms", &room_id.to_string(), "members"])).await
  }

  /// `GET /rooms/{roomId}/accounts`.
  pub async fn room_accounts(&self, room_id: i64) -> Answer<RoomAccounts> {
    Self::answered(self.request(Method::GET, &["rooms", &room_id.to_string(), "accounts"])).await
  }

  /// `PUT /rooms/{roomId}/accounts/{account}`.
  pub async fn let_in(&self, room_id: i64, account: i64, by: i64) -> Answer<RoomAccount> {
    let path = ["rooms", &room_id.to_string(), "accounts", &account.to_string()];

    Self::answered(self.send(Method::PUT, &path, &LetIn { by })).await
  }

  /// `DELETE /rooms/{roomId}/accounts/{account}`, the member itself as `by` where it leaves.
  pub async fn take_out(&self, room_id: i64, account: i64, by: i64) -> Answer<()> {
    let path = ["rooms", &room_id.to_string(), "accounts", &account.to_string()];

    Self::nothing(self.request(Method::DELETE, &path).query(&[("by", by.to_string())])).await
  }

  /// `PUT /accounts/{account}/identity`.
  pub async fn account_identity(&self, account: i64) -> Answer<AccountIdentity> {
    Self::answered(self.request(Method::PUT, &["accounts", &account.to_string(), "identity"])).await
  }

  /// `GET /accounts/{account}/rooms`.
  pub async fn account_memberships(&self, account: i64) -> Answer<AccountMemberships> {
    Self::answered(self.request(Method::GET, &["accounts", &account.to_string(), "rooms"])).await
  }

  /// `POST /rooms/{roomId}/words`.
  pub async fn account_words(&self, room_id: i64, words: &AccountWords) -> Answer<Created> {
    Self::answered(self.send(Method::POST, &["rooms", &room_id.to_string(), "words"], words)).await
  }

  /// `PUT /rooms/{roomId}/providers`.
  pub async fn choose_providers(&self, room_id: i64, chosen: &ChooseProviders) -> Answer<RoomProviders> {
    Self::answered(self.send(Method::PUT, &["rooms", &room_id.to_string(), "providers"], chosen)).await
  }

  /// `GET /rooms/{roomId}/roads`.
  pub async fn road(&self, room_id: i64) -> Answer<Road> {
    Self::answered(self.request(Method::GET, &["rooms", &room_id.to_string(), "roads"])).await
  }

  /// `GET /rooms/{roomId}/own-number`.
  pub async fn own_number(&self, room_id: i64, agent_name: &str) -> Answer<OwnNumber> {
    Self::answered(
      self.get(&["rooms", &room_id.to_string(), "own-number"], &[("agentName", agent_name.to_owned())]),
    )
    .await
  }

  /// `GET /rooms/{roomId}/spoke-between`.
  pub async fn spoke_between(&self, room_id: i64, person: i64, from: i64, to: i64) -> Answer<Spoke> {
    Self::answered(self.get(
      &["rooms", &room_id.to_string(), "spoke-between"],
      &[("person", person.to_string()), ("from", from.to_string()), ("to", to.to_string())],
    ))
    .await
  }

  /// `GET /rooms/{roomId}/people`.
  pub async fn room_people(&self, room_id: i64, ids: &[i64]) -> Answer<Page<Person>> {
    Self::answered(self.get(&["rooms", &room_id.to_string(), "people"], &[("ids", joined(ids))])).await
  }

  /// `GET /rooms/{roomId}/owner-spoke`.
  pub async fn owner_spoke(
    &self,
    room_id: i64,
    person: i64,
    before: i64,
    window_ms: i64,
  ) -> Answer<OwnerSpoke> {
    Self::answered(self.get(
      &["rooms", &room_id.to_string(), "owner-spoke"],
      &[
        ("person", person.to_string()),
        ("before", before.to_string()),
        ("windowMs", window_ms.to_string()),
      ],
    ))
    .await
  }

  /// `GET /rooms/{roomId}/read-marks`.
  pub async fn read_marks(&self, room_id: i64, agent_name: &str) -> Answer<ReadMarkCount> {
    Self::answered(
      self.get(&["rooms", &room_id.to_string(), "read-marks"], &[("agentName", agent_name.to_owned())]),
    )
    .await
  }

  /// `POST /rooms/{roomId}/read-marks`.
  pub async fn keep_marks(&self, room_id: i64, agent_name: &str, message_ids: &[i64]) -> Answer<()> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Keep<'a> {
      agent_name: &'a str,
      message_ids: &'a [i64],
    }

    Self::nothing(self.send(
      Method::POST,
      &["rooms", &room_id.to_string(), "read-marks"],
      &Keep {
        agent_name,
        message_ids,
      },
    ))
    .await
  }

  /// `DELETE /rooms/{roomId}/read-marks`.
  pub async fn forget_marks(&self, room_id: i64, agent_name: &str) -> Answer<Forgotten> {
    Self::answered(
      self
        .request(Method::DELETE, &["rooms", &room_id.to_string(), "read-marks"])
        .query(&[("agentName", agent_name.to_owned())]),
    )
    .await
  }

  /// `POST /read-mark-counts`.
  pub async fn read_mark_counts(
    &self,
    agent_name: &str,
    since: &[(i64, i64)],
  ) -> Answer<Page<RoomReadCount>> {
    #[derive(Serialize)]
    struct RoomSince {
      room: i64,
      since: i64,
    }
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Counts<'a> {
      agent_name: &'a str,
      since: Vec<RoomSince>,
    }

    Self::answered(
      self.send(
        Method::POST,
        &["read-mark-counts"],
        &Counts {
          agent_name,
          since: since
            .iter()
            .map(|(room, at)| RoomSince {
              room: *room,
              since: *at,
            })
            .collect(),
        },
      ),
    )
    .await
  }

  /// `GET /rooms/{roomId}/notices`.
  pub async fn notices(&self, room_id: i64, query: &NoticeQuery) -> Answer<Page<Notice>> {
    Self::answered(self.get(&["rooms", &room_id.to_string(), "notices"], query)).await
  }

  /// `POST /rooms/{roomId}/notices`.
  pub async fn notice(&self, room_id: i64, fact: Value) -> Answer<Created> {
    Self::answered(self.send(Method::POST, &["rooms", &room_id.to_string(), "notices"], &fact)).await
  }

  /// `GET /rooms/{roomId}/send-verbs`.
  pub async fn send_verbs(&self, room_id: i64, via: Option<&str>) -> Answer<SendVerbs> {
    let request = match via {
      Some(via) => self.get(&["rooms", &room_id.to_string(), "send-verbs"], &[("via", via)]),
      None => self.request(Method::GET, &["rooms", &room_id.to_string(), "send-verbs"]),
    };

    Self::answered(request).await
  }

  /// `GET /people`.
  pub async fn people(&self, ids: &[i64]) -> Answer<Page<Person>> {
    Self::answered(self.get(&["people"], &[("ids", joined(ids))])).await
  }

  /// `POST /promise-readings`.
  pub async fn promise_reading(&self, text: &str) -> Answer<PromiseReading> {
    Self::answered(self.send(Method::POST, &["promise-readings"], &serde_json::json!({ "text": text })))
      .await
  }

  /// `POST /instructions`.
  pub async fn queue_instruction(&self, instruction: &InstructionRequest) -> Answer<Created> {
    Self::answered(self.send(Method::POST, &["instructions"], instruction)).await
  }

  /// `GET /instructions/{instructionId}`.
  pub async fn instruction(&self, instruction_id: i64) -> Answer<Instruction> {
    Self::answered(self.request(Method::GET, &["instructions", &instruction_id.to_string()])).await
  }

  /// `DELETE /instructions/{instructionId}`.
  pub async fn withdraw_instruction(&self, instruction_id: i64) -> Answer<()> {
    Self::nothing(self.request(Method::DELETE, &["instructions", &instruction_id.to_string()])).await
  }

  /// `POST /owner/notices`.
  pub async fn owner_notice(&self, notice: &OwnerNotice) -> Answer<Created> {
    Self::answered(self.send(Method::POST, &["owner", "notices"], notice)).await
  }

  /// `POST /owner/instructions`.
  pub async fn owner_instruction(&self, instruction: &OwnerInstruction) -> Answer<()> {
    Self::nothing(self.send(Method::POST, &["owner", "instructions"], instruction)).await
  }

  /// `PUT /room-holders/{holderName}`.
  pub async fn declare_holder(&self, holder_name: &str, holder: &RoomHolder) -> Answer<()> {
    Self::nothing(self.send(Method::PUT, &["room-holders", holder_name], holder)).await
  }

  /// `POST /sends`.
  pub async fn send_tool(&self, request: &SendRequest) -> Answer<SendAnswer> {
    Self::answered(self.send(Method::POST, &["sends"], request)).await
  }

  /// `POST /owner/messages`.
  pub async fn owner_message(&self, message: &OwnerMessage) -> Answer<Created> {
    Self::answered(self.send(Method::POST, &["owner", "messages"], message)).await
  }

  /// `POST /consult-lines`.
  pub async fn consult_line(&self, line: &ConsultLine) -> Answer<ConsultSaid> {
    Self::answered(self.send(Method::POST, &["consult-lines"], line)).await
  }

  /// `GET /consult-lines`.
  pub async fn consult_block(&self, agent_name: &str) -> Answer<ConsultBlock> {
    Self::answered(self.get(&["consult-lines"], &[("agentName", agent_name.to_owned())])).await
  }

  /// `POST /talks`.
  pub async fn talk(&self, talk: &Talk) -> Answer<TalkAnswer> {
    Self::answered(self.send(Method::POST, &["talks"], talk)).await
  }

  /// `POST /handoffs/{agentName}`: the file the agent's standing session wrote, or none where it wrote nothing.
  pub async fn hand_off(&self, agent_name: &str, asked: &HandoffAsked) -> Answer<Option<HandoffWritten>> {
    let answer = Self::sent(self.send(Method::POST, &["handoffs", agent_name], asked)).await?;

    if answer.status() == StatusCode::NO_CONTENT {
      return Ok(None);
    }

    let bytes = answer.bytes().await.map_err(|why| ClientFailure::Unreadable(why.to_string()))?;

    serde_json::from_slice(&bytes)
      .map(Some)
      .map_err(|why| ClientFailure::Unreadable(why.to_string()))
  }

  /// `POST /handovers/{agentName}`: the agent's one session hands over at its next turn.
  pub async fn hand_over(&self, agent_name: &str, asked: &HandoverAsked) -> Answer<HandoverPending> {
    Self::answered(self.send(Method::POST, &["handovers", agent_name], asked)).await
  }

  /// `POST /events`.
  pub async fn publish_event(&self, event: &PublishedEvent) -> Answer<Created> {
    Self::answered(self.send(Method::POST, &["events"], event)).await
  }

  /// `GET /live`.
  pub async fn live(&self) -> Answer<Live> {
    Self::answered(self.request(Method::GET, &["live"])).await
  }

  /// `POST /hooks/{point}`.
  pub async fn hook(&self, point: &str, ask: Value) -> Answer<HookAnswer> {
    Self::answered(self.send(Method::POST, &["hooks", point], &ask)).await
  }

  /// `DELETE /rooms/{roomId}`, its preview where `dry_run`.
  pub async fn remove_room(&self, room_id: i64, by: &str, dry_run: bool) -> Answer<Value> {
    let query = [("by", by.to_owned()), ("dryRun", dry_run.to_string())];

    Self::answered(self.request(Method::DELETE, &["rooms", &room_id.to_string()]).query(&query)).await
  }

  /// `GET /rounds/openings`.
  pub async fn openings(&self, room_id: i64, rounds: &[String]) -> Answer<Page<RoundOpening>> {
    Self::answered(
      self.get(&["rounds", "openings"], &[("room", room_id.to_string()), ("rounds", rounds.join(","))]),
    )
    .await
  }

  /// `GET /rounds/openings/{instructionId}`.
  pub async fn opening(&self, room_id: i64, instruction_id: i64) -> Answer<OpeningText> {
    Self::answered(
      self.get(&["rounds", "openings", &instruction_id.to_string()], &[("room", room_id.to_string())]),
    )
    .await
  }

  /// `GET /rounds/drafts`.
  pub async fn drafts(&self, room_id: i64, rounds: &[String]) -> Answer<Page<RoundDrafts>> {
    Self::answered(
      self.get(&["rounds", "drafts"], &[("room", room_id.to_string()), ("rounds", rounds.join(","))]),
    )
    .await
  }

  /// `GET /draft-processes`: every row, or with a supervisor named the one row it reads.
  pub async fn draft_processes(&self, agent_name: Option<&str>) -> Answer<Page<DraftProcess>> {
    let whose: Vec<(&str, &str)> = agent_name.map(|name| ("agentName", name)).into_iter().collect();

    Self::answered(self.get(&["draft-processes"], &whose)).await
  }

  /// `PUT /draft-processes/{provider}/{tier}/{agentName}`.
  pub async fn set_draft_process(
    &self,
    provider: &str,
    tier: &str,
    agent_name: &str,
    process: &DraftProcessSet,
  ) -> Answer<DraftProcess> {
    Self::answered(self.send(Method::PUT, &["draft-processes", provider, tier, agent_name], process)).await
  }

  /// `DELETE /draft-processes/{provider}/{tier}/{agentName}`.
  pub async fn remove_draft_process(&self, provider: &str, tier: &str, agent_name: &str) -> Answer<()> {
    Self::nothing(self.request(Method::DELETE, &["draft-processes", provider, tier, agent_name])).await
  }

  /// `GET /draft-questions`: a supervisor's catalogue, or the policy's alone where none is named.
  pub async fn draft_questions(&self, agent_name: Option<&str>) -> Answer<Page<DraftQuestion>> {
    let whose: Vec<(&str, &str)> = agent_name.map(|name| ("agentName", name)).into_iter().collect();

    Self::answered(self.get(&["draft-questions"], &whose)).await
  }

  /// `PUT /draft-questions/{agentName}/{name}`.
  pub async fn set_draft_question(
    &self,
    agent_name: &str,
    name: &str,
    question: &DraftQuestionSet,
  ) -> Answer<DraftQuestion> {
    Self::answered(self.send(Method::PUT, &["draft-questions", agent_name, name], question)).await
  }

  /// `DELETE /draft-questions/{agentName}/{name}`.
  pub async fn remove_draft_question(&self, agent_name: &str, name: &str) -> Answer<()> {
    Self::nothing(self.request(Method::DELETE, &["draft-questions", agent_name, name])).await
  }

  /// `POST /supervisors`.
  pub async fn add_supervisor(&self, supervisor: &SupervisorRequest) -> Answer<SupervisorAdded> {
    Self::answered(self.send(Method::POST, &["supervisors"], supervisor)).await
  }

  /// `DELETE /supervisors/{supervisorName}`.
  pub async fn remove_supervisor(
    &self,
    name: &str,
    by: Option<&str>,
    forget_brain: bool,
    force: bool,
  ) -> Answer<()> {
    let mut request = self
      .request(Method::DELETE, &["supervisors", name])
      .query(&[("forgetBrain", forget_brain), ("force", force)]);
    if let Some(by) = by {
      request = request.query(&[("by", by)]);
    }

    Self::nothing(request).await
  }

  /// `POST /supervisors/{supervisorName}/resets`.
  pub async fn reset_supervisor(&self, name: &str, reset: &SupervisorReset) -> Answer<Value> {
    Self::answered(self.send(Method::POST, &["supervisors", name, "resets"], reset)).await
  }

  /// `GET /supervisors/{supervisorName}/rooms`.
  pub async fn supervisor_rooms(&self, name: &str) -> Answer<Page<SupervisorRoom>> {
    Self::answered(self.request(Method::GET, &["supervisors", name, "rooms"])).await
  }

  /// `GET /default-answerer`.
  pub async fn default_answerer(&self) -> Answer<DefaultAnswerer> {
    Self::answered(self.request(Method::GET, &["default-answerer"])).await
  }

  /// `PUT /default-answerer`.
  pub async fn set_default_answerer(&self, answerer: &DefaultAnswerer) -> Answer<DefaultAnswerer> {
    Self::answered(self.send(Method::PUT, &["default-answerer"], answerer)).await
  }

  /// `PUT /rooms/{roomId}/supervisors`.
  pub async fn set_room_supervisors(
    &self,
    room_id: i64,
    list: &RoomSupervisors,
  ) -> Answer<RoomSupervisors> {
    Self::answered(self.send(Method::PUT, &["rooms", &room_id.to_string(), "supervisors"], list)).await
  }

  /// `PUT /pinned-rooms`.
  pub async fn pin_rooms(&self, rooms: &PinRooms) -> Answer<PinnedRooms> {
    Self::answered(self.send(Method::PUT, &["pinned-rooms"], rooms)).await
  }

  /// `POST /pinned-rooms`.
  pub async fn pin_room(&self, room: &PinRoom) -> Answer<PinnedRooms> {
    Self::answered(self.send(Method::POST, &["pinned-rooms"], room)).await
  }

  /// `DELETE /pinned-rooms/{roomId}`.
  pub async fn unpin_room(&self, room_id: i64) -> Answer<PinnedRooms> {
    Self::answered(self.request(Method::DELETE, &["pinned-rooms", &room_id.to_string()])).await
  }
}

fn joined(ids: &[i64]) -> String {
  ids.iter().map(i64::to_string).collect::<Vec<String>>().join(",")
}

/// The error shape a refusal carries, for a caller matching on its code.
pub fn wire_error(answer: &ClientFailure) -> Option<&WireError> {
  match answer {
    ClientFailure::Answered { answer, .. } => answer.as_ref(),
    _ => None,
  }
}
