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
    stay fixed」): the code and its fields, which the web unit's language packs render in the room's language.
  - **Kinds** are the values a field takes where it names a sort of thing a screen or a record shows: a
    kebab-case word, or, in a list where any kind carries fields of its own, an object holding the word under
    `kind` beside the fields.
- **A code is kebab-case and unique across refusals, facts and fixed lines,** so a reader of `code` needs nothing
  else to know what it is. A kind is unique within its own list. `tests/listed.rs` in the crate holds every code
  and kind to this document.
- **What has no code:** a prompt line an agent reads, which is English and stands in the policy
  (`docs/policy.md`); a run's log line, which is English; a command line's own usage.

## Refusals

### ConsultRefusal

| Code | Status | Fields | When |
|---|---|---|---|
| `empty` | 400 | | the words for the consult room are empty |
| `no-room` | 409 | | no consult room is set |
| `not-answerer` | 409 | `agent`, `room` | the calling agent does not answer in the consult room |
| `no-agent` | 400 | | the call was made outside any agent's turn |

### OwnerSendRefusal

| Code | Status | Fields | When |
|---|---|---|---|
| `reply-missing` | 409 | | the message to reply to is not in the record |
| `words-only` | 409 | | the room takes words only |
| `too-large` | 413 | `name`, `size_bytes`, `largest_bytes` | a file past the ceiling one file may be |
| `none-sent` | 409 | `unsent` (each a `FileUnsent`) | no file of a send went |

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

### RoomRemovalRefusal

| Code | Status | Fields | When |
|---|---|---|---|
| `seeded-room` | 409 | | a room the companion seeds declare |
| `infrastructure` | 409 | | the infrastructure room the configuration names |
| `owners-chat` | 409 | | the administrator's own private chat |
| `not-administrator` | 403 | | anyone but the administrator asked |
| `chat` | 400 | | the room's number is not a room's number |

### SendRefusal

| Code | Status | Fields | When |
|---|---|---|---|
| `verb-refused` | 409 | `road`, `verb` | the road the room is answered on does not perform the verb |
| `another-agents-room` | 403 | `agent` | another agent answers the room |
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
| `avatar-unsupported` | 409 | `label` | the room is to be answered as the user where the provider's `human-avatar` mode does not stand active |
| `room-added` | 409 | `room` | the room was already added |

### CompanionRefusal

| Code | Status | Fields | When |
|---|---|---|---|
| `not-companion` | 404 | `agent` | the agent named is not a companion |
| `name` | 400 | | a name that is no name |
| `name-taken` | 409 | `name` | the name is another agent's |
| `label` | 400 | | no label |
| `provider` | 400 | `provider` | a provider that cannot carry a companion |
| `model` | 400 | | a model that is no registered model with an address |
| `worker` | 400 | `worker` | a worker that is no worker |
| `room` | 400 | | no room title |
| `seeded-companion` | 409 | `label` | a seeded companion asked to be removed |
| `action` | 400 | | an action the route does not take |

## Facts

### HealthFact

| Code | Fields | When |
|---|---|---|
| `fold-failed` | `room`, `reason` | a room's fold did not happen; the owner hears why |

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
| `tools-switched` | `on` | the owner switched tools in the room |
| `photos-switched` | `on` | the owner switched opening the room's pictures, for this room alone |
| `delegation-switched` | `on`, `delegate` (the delegate's registered label, or null) | the owner switched delegation in the room |
| `rest-notice-switched` | `on` | the owner switched the room's rest notice |
| `rest-notice-kept` | | the owner asked to switch the rest notice off in the owner's own private chat, where it stays on |
| `restart-notice-switched` | `on` | the owner switched the room's restart notice |
| `member-muted` | `muted` | the owner muted or unmuted a member of the room |
| `mute-target-missing` | | a mute or an unmute named nobody who can be: neither an answered person nor a number, or the owner or the agent |
| `mode-set` | `everyone` | the owner set whose words wake the agents: everyone's, or the owner's alone |

### CompanionFact

| Code | Fields | When |
|---|---|---|
| `room-taken-back` | `label`, `title` | a companion took back a room already standing under its name |

### RoomRemovalWarning

| Code | Fields | When |
|---|---|---|
| `answerers` | `names` | companions answer the room |
| `road` | `road` | a road can still reach the room |
| `private` | `user` | the room is another user's private chat |

## Fixed lines

### FixedLine

| Code | Fields | When |
|---|---|---|
| `rest-begins` | `until` (epoch seconds, or null) | a usage limit begins |
| `provider-down` | | the provider stopped answering |
| `compacting` | | a room's session is being folded and could not say so itself |
| `compact-failed` | | a room's fold did not happen |
| `run-broken` | `agent`, `cause`, `record` | a run broke and no session can answer the order about it |
| `chat-denied` | | the owner shut the room |
| `going-down-for-update` | `items` | the service stops for a deployment: a system notice |
| `going-down` | | the service restarts with no deployment behind it: a system notice |
| `companion-engine-error` | `kind` (an `EngineError`) | a companion's engine ended its turn with an error |
| `companion-cut` | `agent`, `reason` | a companion's turn its guard cut short; the next turn opens a new session |
| `draw` | `label`, `asked`, `result`, `number`, `seed`, `method`, `answered`, `next` | a fair draw's room line (`docs/random-draw.md`) |

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
| `expired` | the cache's lifetime passed |
| `no-prefix` | the session holds no record of its prefix |
| `downgrade` | a downgrade was noticed |
| `downgrade-forced` | the model was replaced on its own |

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
| `no-conversation` | no conversations stand to be touched |
| `not-chat-touch` | the ping names a target other than the chat touch |

### EngineError

| Kind | Meaning |
|---|---|
| `busy` | the engine was busy |
| `refused` | the engine refused the request |
| `too-long` | the answer was longer than the engine allows |
| `failed` | any other failure |
