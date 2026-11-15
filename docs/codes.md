# The codes

Every refusal the receiver answers, every fact it hands a room's agent and every fixed line a room hears is a code
with structured fields, held in the crate `receiver-codes`, one Rust module to an area and in it one enum to a
family. No code carries a sentence in a room's language.

- **Four families:**
  - **Refusals** are answered as the route's family answers a refusal (`docs/boundary-api.md`, 「Wire
    conventions」): the status, the code and one English sentence for the caller. `ReceiverError` gathers every
    area's refusals, and each answers its own status.
  - **Facts** are what the receiver hands a room's agent as a quiet instruction, to say in its own words: the code
    and its fields, with the English the agent reads. `Fact` gathers every area's facts.
  - **Fixed lines** are the few lines a room hears that no session says (`docs/voiced-notices.md`, 「Lines that
    stay fixed」): the code and its fields, which the web unit's language packs render in the room's language and
    the receiver's policy sentence stands in for where they cannot; a person never reads the code or its fields.
  - **Kinds** are the values a field takes where it names a sort of thing a screen or a record shows: a
    kebab-case word, or, in a list where any kind carries fields of its own, an object holding the word under
    `kind` beside the fields.
- **A code is kebab-case and unique across refusals, facts and fixed lines,** so a reader of `code` needs nothing
  else to know what it is. A kind is unique within its own list. `tests/listed.rs` in the crate holds every code
  and kind to this document.
- **What has no code:** a prompt line an agent reads, which is English and stands in the policy
  (`docs/policy.md`); a run's log line, which is English; a command line's own usage.

## Refusals

### MessageActRefusal

| Code | Status | Fields | When |
|---|---|---|---|
| `no-caller` | 403 | | the relay names no user, or the user holds no sender |
| `message-unknown` | 404 | `room`, `message` | the room keeps no such message |
| `message-deleted` | 409 | `room`, `message` | the message was deleted |
| `message-forbidden` | 403 | `room`, `message` | another person's message to edit or delete |
| `unknown-sticker` | 400 | `emoji` | the record's sticker set holds no such emoji |

### AvatarRefusal

| Code | Status | Fields | When |
|---|---|---|---|
| `picture-forbidden` | 403 | `kind` | the caller may not set or let go this subject's picture |
| `unknown-kind` | 400 | `kind` | the subject's kind is none of `room`, `person` and `agent` |
| `no-picture-given` | 400 | | the form holds no `picture` |
| `not-a-picture` | 400 | | the picture cannot be read as one |
| `picture-too-large` | 413 | `size_bytes`, `largest_bytes` | the picture is past the ceiling one file may be |
| `no-number` | 404 | `agent` | the agent stands under no number in any room |

### OwnerSendRefusal

| Code | Status | Fields | When |
|---|---|---|---|
| `reply-missing` | 409 | | the message to reply to is not in the record |
| `words-only` | 409 | | the room takes words only |
| `too-large` | 413 | `name`, `size_bytes`, `largest_bytes` | a file past the ceiling one file may be |
| `none-sent` | 409 | `unsent` (each a `FileUnsent`) | no file of a send went |
| `room-archived` | 409 | `room` | the room is archived and locked (`docs/receiver.md`, 「An archived room」) |

### TalkRefusal

| Code | Status | Fields | When |
|---|---|---|---|
| `busy` | 409 | | the agent is still answering the talk's last words |
| `resting` | 409 | | the agent rests, or the owner's chat has fallen silent |
| `limit` | 429 | | the provider's usage limit is reached |
| `outage` | 503 | | the provider is out |
| `silent` | 502 | | the agent said nothing |
| `unanswered` | 502 | | no answer came, any other way |

### VoiceRefusal

| Code | Status | Fields | When |
|---|---|---|---|
| `switched-off` | 409 | | the voice briefing is switched off |
| `voiceless` | 409 | `label` | the agent has no voice |
| `unsigned` | 400 | | a briefing that says nobody launched it |
| `wordless` | 400 | | a briefing with nothing to say |
| `not-up` | 502 | `unit` | the briefing's unit did not come up |
| `one-switch` | 400 | | a switch asked with no switch or no value |
| `unreachable` | 502 | `reason` | no voice engine is configured, or the engine did not take the briefing |

### RoomRemovalRefusal

| Code | Status | Fields | When |
|---|---|---|---|
| `seeded-room` | 409 | | a room the agent room seeds declare |
| `infrastructure` | 409 | | the infrastructure room the configuration names |
| `owners-chat` | 409 | | the administrator's own private chat |
| `not-administrator` | 403 | | anyone but the administrator asked |
| `chat` | 400 | | the room's number is not a room's number |

### SendRefusal

| Code | Status | Fields | When |
|---|---|---|---|
| `verb-refused` | 409 | `road`, `verb` | the road the room is answered on does not perform the verb |
| `another-agents-room` | 403 | `agent` | another agent answers the room |
| `read-only-room` | 403 | `room` | the room is read-only, read in the owner's account alone (`docs/receiver.md`, 「A read-only room」) |
| `archived-room` | 409 | `room` | the room is archived and locked (`docs/receiver.md`, 「An archived room」) |
| `role-without-sends` | 403 | `agent` | the asker's role gives no sending |
| `sticker` | 409 | `refusal` | the road refused a sticker |
| `no-picture` | 409 | `road` | not one picture of a send went |
| `speaker-refused` | 409 | `road`, `agent` | the road cannot send under the agent's name |
| `unnumbered` | 409 | `road`, `agent` | the road gave the agent no number |
| `not-an-address` | 400 | `url` | a picture named by neither a web address nor a file of the data root |
| `outside-data-root` | 403 | `path`, `agent` | a file named outside the asker's data root |
| `missing-file` | 400 | `path` | a file of the data root that is not there |
| `picture-unfetched` | 502 | `url` | a picture at a web address that could not be fetched |
| `file-not-kept` | 502 | `name` | a file the send went with could not be kept beside its message |
| `other-rooms-file` | 403 | `room` | a file of another room's folder |
| `job-sends-room-file` | 403 | | a job asked to send a room's file |
| `briefing-turn` | 409 | `agent`, `room` | the agent's session asked a send into the room while the receiver carries its voice briefing turn there, whose answer is the script itself |

### RoomGrantRefusal

| Code | Status | Fields | When |
|---|---|---|---|
| `not-owners-message` | 403 | | a room's rules changed on a message its owner did not send there |
| `no-account-model` | 409 | | a grant of one relation where the account model is not in force |

### RoomAdditionRefusal

| Code | Status | Fields | When |
|---|---|---|---|
| `rooms-unlisted` | 409 | `label` | the provider does not declare that it lists the rooms of its account |
| `room-unknown` | 409 | `label`, `room` | the provider does not say the room is its own |
| `avatar-unsupported` | 409 | `label` | the room is to be answered as the user where the provider's `human-avatar` mode does not stand active, or the room's `modes` leave it out |
| `room-added` | 409 | `room` | the room was already added |
| `contacts-unlisted` | 409 | `label` | the provider does not declare that it lists the contacts of its account |
| `chats-unstarted` | 409 | `label` | the provider does not declare that it starts a conversation of the shape asked |
| `contact-unknown` | 409 | `label` | the provider does not know a contact the start names |
| `start-refused` | 409 | `label`, `detail` | the provider refused to start the conversation, for its own reason |

### RoomAccountRefusal

| Code | Status | Fields | When |
|---|---|---|---|
| `private-room` | 403 | `room`, `account` | the room is one account's private room with its BMO, or the owner's private room: words or files from any other account, or a member let in or taken out by any account (`docs/receiver.md`, 「Console rooms and accounts」) |

### RoomMembersRefusal

| Code | Status | Fields | When |
|---|---|---|---|
| `record-only-room` | 409 | `room` | the room stands on the record alone, so no chat service lists its members |
| `members-unlisted` | 409 | `label` | the room's chat service does not declare that it lists the members of a room, or serves no route for them |
| `members-unheld` | 409 | `label`, `detail` | the room's chat service holds no members of the room, or refused them, for its own reason |

### RoomModeRefusal

| Code | Status | Fields | When |
|---|---|---|---|
| `room-not-added` | 409 | `room` | the room was never added, so it has no mode to change |
| `no-chat-service` | 409 | | the room is on no chat service, so nothing there would answer as the person |
| `avatar-unoffered` | 409 | `label` | a chat service on the room's list does not offer the avatar mode |
| `room-takes-no-avatar` | 409 | `label`, `room` | a chat service says the room is never answered in, so it takes no avatar |

### FileFillRefusal

| Code | Status | Fields | When |
|---|---|---|---|
| `room-fetches-no-files` | 409 | `room` | no road holds the room, or its road declares no `fetchMedia`, so no row of it can be filled |
| `room-files-filling` | 409 | `room` | a pass already fills the room's rows |

### RoomReadRefusal

| Code | Status | Fields | When |
|---|---|---|---|
| `not-a-group` | 403 | `room` | the room is no group room, so no supervisor but its own answerers reads it |
| `not-a-member` | 403 | `agent`, `room` | the supervisor's account is no member of the room |

### SupervisorRefusal

The refusals of the supervisor routes and of a room's list of supervisors (`docs/api.md`, 「Supervisors」).

| Code | Status | Fields | When |
|---|---|---|---|
| `unknown-supervisor` | 404 | `supervisor` | the agency registers no supervisor of the name |
| `name` | 400 | | a name that is no name |
| `name-taken` | 409 | `name` | the name is another supervisor's |
| `label` | 400 | | no label |
| `provider` | 400 | `provider` | a road the agency refuses, on the provider named: one it does not carry or that cannot carry the chat line, a model the provider does not hold, an effort the model does not take |
| `model` | 400 | | a model that is no registered model with an address |
| `repository` | 400 | `repository` | a repository the agency does not hold |
| `room` | 400 | | no room title |
| `seeded` | 409 | `label` | a supervisor the deployment seeds asked to be removed |
| `default-answerer` | 409 | `label` | the supervisor the default answerer setting names asked to be removed |
| `running` | 409 | `label` | a supervisor a turn holds now asked to be removed without `force` |
| `action` | 400 | | an action the route does not take |

## Facts

### HealthFact

| Code | Fields | When |
|---|---|---|
| `handoff-lost` | `agent`, `reason` | an agent's session was replaced and neither its handoff nor a transcript of it could be carried to the new one; the owner hears it once |

### VoiceFact

| Code | Fields | When |
|---|---|---|
| `briefing-on` | | the owner switched the voice briefing on from the console |
| `briefing-off` | | the owner switched the voice briefing off from the console |

### SettingFact

Each is the acknowledgement of one of the owner's in-room setting commands (`docs/rooms.md`), queued for the room's
session once the setting has changed, one waiting per setting.

| Code | Fields | When |
|---|---|---|
| `chat-allowed` | | the owner allowed the room |
| `photos-switched` | `on` | the owner switched opening the room's pictures, for this room alone |
| `restart-notice-switched` | `on` | the owner switched the room's restart notice |
| `member-muted` | `muted` | the owner muted or unmuted a member of the room |
| `mute-target-missing` | | a mute or an unmute named nobody who can be: neither an answered person nor a number, or the owner or the agent |

### SupervisorFact

| Code | Fields | When |
|---|---|---|
| `room-taken-back` | `label`, `title` | a supervisor added on the console took back a room already standing under its name |

### RoomRemovalWarning

| Code | Fields | When |
|---|---|---|
| `answerers` | `names` | other agents answer the room |
| `road` | `road` | a road can still reach the room |
| `private` | `user` | the room is another user's private chat |

## Fixed lines

### FixedLine

| Code | Fields | When |
|---|---|---|
| `rest-begins` | `until` (epoch seconds, or null) | a usage limit begins |
| `provider-down` | | the provider stopped answering |
| `run-broken` | `agent`, `cause`, `record` | a run broke and no session can answer the order about it |
| `chat-denied` | | the owner shut the room |
| `going-down-for-update` | `items` | the service stops for a deployment: a system notice |
| `going-down` | | the service restarts with no deployment behind it: a system notice |
| `memory-tidying` | | the agent's one session is being replaced and its memory tidied; a room whose words arrive meanwhile is told once that the answer comes when it is done |
| `draw` | `label`, `asked`, `result`, `number`, `seed`, `method`, `answered`, `next` | a fair draw's room line (`docs/random-draw.md`) |
| `account-let-in` | `account`, `by` | an account let into a room in the console by another (`docs/receiver.md`, 「Console rooms and accounts」) |
| `account-left` | `account` | an account went out of a room in the console by itself |
| `account-taken-out` | `account`, `by` | an account taken out of a room in the console by another |

## Kinds

### FileUnsent

| Kind | Fields | Meaning |
|---|---|---|
| `empty` | `name` | the file holds no bytes |
| `too-large` | `name`, `size_bytes`, `largest_bytes` | the file is past the ceiling |
| `refused` | `name`, `reason` | the road refused it |

### RunTitle

| Kind | Fields | Meaning |
|---|---|---|
| `chat` | | a round of a room's conversation |
| `fold` | | a room's session folded |
| `voice-briefing` | `room` | a voice briefing for a room |
| `say` | `room` | a line said into a room by a run of its own |

### SessionReason

| Kind | Meaning |
|---|---|
| `route` | the provider or model changed |
| `account` | the account changed |
| `placement` | the node the session ran on is gone |
| `prefix` | the system prompt or the head of the run changed |
| `expired` | the cache's lifetime passed; no replacement is made for it any more, and the code stands for records that name it |
| `no-prefix` | the session holds no record of its prefix |
| `downgrade` | a downgrade was noticed |
| `downgrade-forced` | the model was replaced on its own |
| `full` | the session filled its window and its provider hands it over by its own note rather than a fold |
| `asked` | a handover was asked for the session (`POST /handovers/{agentName}`) |
| `process` | the draft process's instructions the session was handed changed (`docs/drafting.md`, 「The questions」) |

### TurnOpening

| Kind | Meaning |
|---|---|
| `fold` | the words before a room's fold |
| `fold-deferred` | a fold put off until the turn running ends |

### SkipReason

| Kind | Meaning |
|---|---|
| `provider-off` | the provider is switched off |
| `per-room` | the agent keeps a session in each room, so it has no one session to keep warm |
| `no-session` | the session never stood in a room |
| `used-recently` | the session was used not long enough ago for a touch |
| `expired` | the session's last turn is older than its provider's cache lifetime, so a touch would pay its whole prefix again |
| `no-conversation` | no conversations stand to be touched |
| `not-chat-touch` | the ping names a target other than the chat touch |
