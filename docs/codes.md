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
| `message-not-agents` | 409 | `room`, `message`, `agent` | a message to be filed as an agent's line stands under another sender than that agent's own number in the room |
| `unknown-sticker` | 400 | `emoji` | the record's sticker set holds no such emoji |

### AvatarRefusal

| Code | Status | Fields | When |
|---|---|---|---|
| `picture-forbidden` | 403 | `kind` | the caller may not set or let go this subject's picture |
| `unknown-kind` | 400 | `kind` | the subject's kind is none of `room`, `person` and `agent` |
| `no-picture-given` | 400 | | the form holds no `picture` |
| `not-a-picture` | 400 | | the picture cannot be read as one |

### OwnerSendRefusal

| Code | Status | Fields | When |
|---|---|---|---|
| `reply-missing` | 409 | | the message to reply to is not in the record |
| `words-only` | 409 | | the room takes words only |
| `too-large` | 413 | `name`, `size_bytes`, `largest_bytes`, `provider` | a file past the largest the room takes, which `provider` declares (`docs/api.md`, 「Sends」) |
| `none-sent` | 409 | `unsent` (each a `FileUnsent`) | no file of a send went |
| `room-archived` | 409 | `room` | the room is archived and locked (`docs/receiver.md`, 「An archived room」) |

### TalkRefusal

| Code | Status | Fields | When |
|---|---|---|---|
| `busy` | 409 | | the agent is still answering the talk's last words |
| `resting` | 409 | | the rest closes the default answerer's road |
| `limit` | 429 | | the provider's usage limit is reached |
| `outage` | 503 | | the provider is out |
| `silent` | 502 | | the agent said nothing |
| `unanswered` | 502 | | no answer came, any other way |

### VoiceRefusal

| Code | Status | Fields | When |
|---|---|---|---|
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
| `words-in-answer` | 409 | `agent`, `room` | the agent's own round asked a line, a spoken talk, a voice note or a caption into the room it answers, whose words go only into the round's answer, which the instruction pipeline reviews (`docs/messages.md`, 「Words handed to the room's supervisor」) |
| `speak-outside-round` | 409 | `agent`, `room` | `speak` was asked outside a round of the agent's own in the room that round answers; only that round's answer is said aloud (`docs/voiced-notices.md`, 「An answer said aloud」) |
| `speak-emotion` | 400 | `emotion`, `emotions` | `speak` named an emotion outside the set an answer is said with |
| `speak-voiceless` | 409 | `agent` | `speak` was asked by an agent whose registration names no voice |
| `speak-no-engine` | 409 | | `speak` was asked where no voice engine is named |

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
| `providers-fixed` | 409 | `room` | the room was added, so its chat service stays the one it was added on and its list of providers is not set |

### RoomAccountRefusal

| Code | Status | Fields | When |
|---|---|---|---|
| `private-room` | 403 | `room`, `account` | the room is one account's private room with its BMO, or the owner's private room: any write in it for another account, every route that changes a room, a message in it or what is kept of it judged alike, or for any account where the room is sealed; and a member let in or taken out by any account (`docs/receiver.md`, 「Console rooms and accounts」) |

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
| `name-taken` | 409 | `name` | the name is another supervisor's, or its room stands already and the record keeps it as no private room of the account the addition names |
| `label` | 400 | | no label |
| `provider` | 400 | `provider` | a road the agency refuses, on the provider named: one it does not carry or that cannot carry the chat line, a model the provider does not hold, an effort the model does not take |
| `model` | 400 | | a model that is no registered model with an address |
| `repository` | 400 | `repository` | a repository the agency does not hold |
| `room` | 400 | | no room title |
| `seeded` | 409 | `label` | a supervisor the deployment seeds asked to be removed |
| `default-answerer` | 409 | `label` | the supervisor the default answerer setting names asked to be removed |
| `running` | 409 | `label` | a supervisor a turn holds now asked to be removed without `force` |
| `not-owners-account` | 403 | `account` | an addition naming `ownerAccount` for another account than the one `Owner-Key` names, not on the administrator's `Owner-Role` |
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
| `road` | `road` (the road's key) | a road can still reach the room |
| `private` | `user` | the room is another user's private chat |

### RemovalLeft

| Kind | Fields | Meaning |
|---|---|---|
| `outside-data-root` | `path` | a file or folder of the room stands outside this data root, so the removal leaves it |
| `not-removed` | `path`, `reason` | the file or folder could not be removed; `reason` is the English sentence for the log, which a screen never shows |

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
| `accounts-let-in` | `accounts` (a list of account keys), `by` | several accounts let into a room in the console by another in one call, the list naming those it let in |
| `unit-let-in` | `unit` (the unit's number), `unitName` (its name as the web unit answered it then), `accounts` (a list of account keys), `by` | the members of an organisation unit, as they stood at that moment, let into a room in the console by another, the list naming those it let in |
| `session-handed-over` | `agent` (the agent's registered name), `cause` (a `HandoverCause`), `reason` (the asker's words, present only where an asker gave them), `by` (the account key of the person who asked, present only where the receiver knows one) | an agent's session was handed over to a fresh one or started afresh; kept in one room's record alone and sent to no chat service (`docs/unified-bmo.md`, 「The room's line」) |
| `supply-closed` | `why` (the reason as the agency names it, a kebab-case word), `until` (epoch seconds, or null) | a person's run found no supply source left; the agency asks for it beside the supply fact it queues in the run's room, so the person hears why while no turn can go out there (the agency's `docs/supply.md`, 「The facts told」) |

## Kinds

### FileUnsent

| Kind | Fields | Meaning |
|---|---|---|
| `empty` | `name` | the file holds no bytes |
| `too-large` | `name`, `size_bytes`, `largest_bytes`, `provider` | the file is past the largest the room takes, which `provider` declares |
| `refused` | `name`, `code`, `reason` | the file was not kept: `code` is the road's refusal code (bmo-chat's `docs/protocol.md`, 「Refusals」) where a chat road refused it, and otherwise the receiver's own refusal code (`docs/api.md`, 「Codes」); `reason` is the English sentence for the log, which a screen never shows |

### RunTitle

| Kind | Fields | Meaning |
|---|---|---|
| `chat` | | a round of a room's conversation |
| `fold` | | a room's session folded |
| `voice-briefing` | `room` | a voice briefing for a room |

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
| `process` | the room block's rules for reading and answering a turn the session was handed changed (`docs/instruction.md`, 「The stage chain and its texts」) |

### HandoverCause

| Kind | Meaning |
|---|---|
| `asked` | a person or the agent itself asked a handover (`POST /handovers/{agentName}`) |
| `reset` | a reset of the session was asked through the agency |
| `full` | the session filled its window |
| `road` | the road the session stood on moved: another provider or model |
| `documents` | the documents or the instructions the session was handed moved |
| `other` | anything else, or a cause nothing names |

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
