# The receiver's fixed API

**고정됨 0.11.1.**

This document is the receiver's contract: every route it serves, in the one common wire format, grouped by the
resource each acts on. `docs/openapi.json` is the same contract in machine form, generated from the types in
`crates/receiver-client`, and the Rust crate and the TypeScript package beside it are written from that one
document. Where this document and another disagree, this one and the OpenAPI document stand.

The wire format, the API design and the rules between units are bmo-chat's (`docs/conventions.md`); this document
states only what is the receiver's.

## What the receiver is

The receiver owns the rooms and their messages, each room's rules, rights and windows, the envelope a turn goes out
in, and the waking of the agents a room's words, a quiet instruction or a job's end calls for. It reaches every chat
service through `bmo-chat-client`, hands turns to the agency's sessions, sends the envelope to the gate, and is read
by the brain, bmo-job and the console over this API.

It serves one API to every caller. No route, field, section or list here names a caller, and no caller has a shape
of its own.

## The wire

- **Keys** are camelCase. **Times** are integers of epoch milliseconds. **Rooms, messages, senders, instructions and
  every other id** are signed 64-bit integers.
- **A list is a page**, `{"items": [...], "more": boolean}`, walked by `before` or `after` (an id) and `limit`. A
  member has one shape wherever it is answered.
- **A failure is a non-2xx status** whose body is `{"code": ..., "error": ...}`. A refusal is answered by status
  too; no success carries a refusal flag.
- **Auth.** Every route needs `Authorization: Bearer <token>` with the receiver's token and answers 401 without it.
  The cache warmer's ping is the one route outside that check: its URL carries the token as `token`, since the
  warmer sends no header.
- **No sentence a person reads goes out of the receiver.** What a room is told is a code and its fields, which the
  agent speaking for the room puts in its own words. Error sentences are English and are for a machine's reader.

### Codes

| Status | `code` | When |
|---|---|---|
| 400 | `bad-request` | the body or the query is not the shape the route takes; the sentence names what is wrong |
| 401 | `unauthorized` | the token is missing or wrong |
| 403 | `forbidden` | the asker may not do this, as the account model says |
| 404 | `no-route` | no route answers this method and path |
| 404 | `not-found` | the room, message, instruction or run named does not exist |
| 409 | `conflict` | the request contradicts what stands; the answer carries the refusal's own code |
| 413 | `too-large` | a file is past the ceiling a road takes |
| 429 | `limit` | a talk's provider limit is reached |
| 502 | `upstream-failed` | a call the route needed on another unit failed; the sentence names it |
| 503 | `not-ready` | the service is up but a part it needs does not stand yet |
| 503 | `outage` | a talk's provider is out |
| 500 | `failed` | the service failed while answering |

A route's own refusals, the facts the receiver hands a room's agent and the fixed lines a room hears are the codes
of `docs/codes.md`, each with its status and its fields.

## Health

| Route | Answers |
|---|---|
| `GET /health` | `ready`, `reason` (null when ready) and `version` |

## The owner

| Route | Answers |
|---|---|
| `GET /owner` | `senderId`, the owner as a sender, and `room`, the owner's private room |

## Rooms

| Route | Body or query | Answers |
|---|---|---|
| `GET /rooms` | | a page of `Room`, the pinned ones first |
| `GET /rooms/{roomId}` | | one `Room` |
| `GET /rooms/{roomId}/title` | | `title`, or null |
| `GET /rooms/{roomId}/answerers` | | `default`, `voice`, and a page of `Answerer` (`agentName`, `companion`) |
| `GET /answerers` | `room` (optional) | a page of `{room, agents}`: every room's agents answering it, or one room's |
| `GET /rooms/{roomId}/roads` | | `kind` (`owner` or `group`) and `oneToOne` |
| `GET /rooms/{roomId}/own-number` | `agentName` | `number` (or null), the sender an agent's own words stand under |
| `GET /rooms/{roomId}/spoke-between` | `person`, `from`, `to` (epoch milliseconds) | `spoke`, whether the person spoke in that span |
| `GET /rooms/{roomId}/people` | `ids`, comma-separated | a page of `Person`: those among `ids` who have stood as a sender in the room |
| `GET /rooms/{roomId}/owner-spoke` | `person`, `before`, `windowMs` | `owner` and `spoke`: whether the person is the owner, and whether the owner spoke in the span |
| `GET /rooms/{roomId}/window` | | `openForMs`, the chat agent's window left, and `closesAt` in epoch milliseconds, or null |
| `GET /rooms/{roomId}/figures` | `zone`, an IANA time zone name, `UTC` where none | the room's figures: `room`, `messages`, `fromPeople`, `speakers`, `firstAt`, `lastAt`, `lastAnswerAt`, `top`, `byHour`, `agents` |
| `PUT /pinned-rooms` | `roomIds`, the whole order | `roomIds`, the order as it now stands |
| `POST /pinned-rooms` | `room`, one room stood last | `roomIds` |
| `DELETE /pinned-rooms/{roomId}` | | `roomIds` |
| `PUT /room-holders/{holderName}` | `address`, `holdings` (each `name` and `effect`: `kept` or `warns`) | 204 |
| `DELETE /rooms/{roomId}` | `by` (the administrator), `dryRun` | what the removal would do, or what it took |

- **A `Room`** is `id`, `title` (or null), `messageCount`, `lastMessageAt` (or null), `pinned`, `senders`, the
  people that have stood as a sender, `companions`, the companions answering there, `ownerRoom`, `registered`, and
  `workRoom`, where the news, questions and answers of work filed from it are told.
- **Any chat reads as a room.** One never registered reads as one whose registration is gone, and work filed from
  it reaches the owner's private room instead.
- **The answerers** are the awake agents the room names, or the default answerer where it names none awake; `voice`
  is the agent that speaks for the room.
- **The figures' `agents`** hold each answering agent's `agentName`, messages, `lastAnswerAt`, the session it keeps
  for the room, the agency's row for it and the state of that session; `runs`, `lastRunAt`, `weekTokens` and
  `totalTokens` stand null until the agency answers a tally by agent and room.
- **A room's removal** asks the record what it would take, each declared holder, the agency and the brain, and
  refuses anyone but the administrator. `dryRun=true` answers the preview and writes nothing; the answer names
  `goes`, `stays`, `pictures`, `settings`, `folders`, `warned`, `units` and, for the removal itself, `taken`,
  `cleared`, `holders` and `couldNot`.

## Messages

| Route | Body or query | Answers |
|---|---|---|
| `GET /rooms/{roomId}/messages` | `before`, `after`, `limit`, `agentName`, `since`, `marked`, `unread`, `by` | a page of `Message` |
| `GET /rooms/{roomId}/messages/{messageId}` | | one `Message` |
| `GET /rooms/{roomId}/messages/{messageId}/metadata` | | `metadata`, what the chat service handed over beside it, or null |
| `GET /rooms/{roomId}/messages/{messageId}/sender-is-owner` | | `owner` |
| `GET /rooms/{roomId}/messages/{messageId}/files` | | a page of `FileEntry` |
| `GET /rooms/{roomId}/messages/{messageId}/files/{place}` | `download` | the file's bytes |
| `GET /rooms/{roomId}/messages/{messageId}/picture` | | the picture's bytes |
| `GET /rooms/{roomId}/day-start` | `from` (epoch milliseconds) | `messageId`, the first message a person said at or after it, or null |
| `GET /rooms/{roomId}/pictures` | `at`, `before`, `after`, `limit` | a page of `Picture`, `before` and `total` |

- **A `Message`** is `id`, `room`, `senderId` (or null), `senderName`, `sentAt`, `text`, `imagePath` (or null),
  `replyToMessageId` (or null), `answerSeconds`, `round`, `album`, `via` and `files`, each a `KeptFile` (`kind`,
  `source`, `storedPath`, `summary`, `collectedAt`).
- **The message page** holds only what a person said unless `by=agent` or `by=any` says otherwise. It is walked by
  `before` or `after`, a message the reader holds, and ends on the side it walked from. With `agentName`,
  `unread=true` answers the messages past that agent's read marks, oldest first, from `since` where given; with
  `marked=true` it answers the messages the agent has marked, after `after`. This is the one message read: there is
  no second route for the same message.
- **The bytes** answer as a file does over HTTP: `Content-Type`, `Content-Length`, `Accept-Ranges`, and `Range`
  answered 206 or 416. `download=true` adds `Content-Disposition: attachment`. Only bytes inside the room's own
  folder are served; anything else is 404 `not-found`.
- **The metadata is opaque:** it is kept and handed back unread, so what it holds is the chat service's to state.
- **A `Picture`** is `id`, `album`, `senderName`, `sentAt`, `text` and `file` (`name`, `bytes`, `type`), null where
  the stored picture is gone. `at` answers that picture alone, `before` the stretch ending before it, `after` the
  stretch past it, and none the newest.

## Read marks

| Route | Body or query | Answers |
|---|---|---|
| `GET /rooms/{roomId}/read-marks` | `agentName` | `count`, the agent's marks on the room's messages that still stand |
| `POST /rooms/{roomId}/read-marks` | `agentName`, `messageIds` | 204 |
| `DELETE /rooms/{roomId}/read-marks` | `agentName` | `forgotten`, the count of marks let go |
| `POST /read-mark-counts` | `agentName`, `since` (each `room` and `since`, epoch milliseconds) | a page of `{room, count}` |

## People

| Route | Body or query | Answers |
|---|---|---|
| `GET /people` | `ids`, comma-separated | a page of `Person` (`id`, `name` or null) |

## Notices

| Route | Body or query | Answers |
|---|---|---|
| `GET /rooms/{roomId}/notices` | `before`, `after`, `limit`, `kind`, `from`, `to` | a page of `Notice` |
| `POST /rooms/{roomId}/notices` | `fact`, `intent`, `followUp?`, `optional`, `kind?`, `expiresAt?`, `events?`, `ridesUntil?` | 201, `{id}`: the instruction it was queued as |
| `POST /owner/notices` | `code` and its parameters beside it | 201, `{id}` |

- **A `Notice`** is `id`, `kind`, `summary`, `text`, `createdAt` and `expiresAt`. The list is walked by `before` or
  `after`, an instruction id, and narrowed by `kind` and by the span it was queued in, both ends included.
- **An owner notice** is queued for the chat agent in the owner's private room, carrying the code and its
  parameters as a fact.

## Instructions

| Route | Body or query | Answers |
|---|---|---|
| `POST /instructions` | `agentName?`, `room?`, `said` (`fact` or `order`), `sort`, `summary?`, `kind?`, `expiresAt?`, `events?`, `ridesUntil?`, `via?`, `run?` | 201, `{id}` |
| `GET /instructions/{instructionId}` | | `id`, `state` (`pending`, `handed` or `delivered`) |
| `DELETE /instructions/{instructionId}` | | 204 |
| `POST /owner/instructions` | `chatId`, `text` | 204 |

- **An instruction** is queued for the agent's session in the room, and a `kind` replaces the pending instruction
  of the same kind there. An instruction for a room whose registration is gone goes to the owner's private room
  instead, naming the room, and never into the platform room; one whose agent no longer answers its room goes to
  the room's voice.
- **A quiet instruction** is one with `kind` `watch` or `reminder` and a `said.order`: it is queued under that
  kind and opens the agent's turn there.
- **A withdrawn instruction** that was handed or is gone is no refusal; it is simply no longer pending.

## Sends

| Route | Body or query | Answers |
|---|---|---|
| `POST /sends` | `asker` (`room?`, `via?`, `agentName?`, `by?`, `job`), `tool`, `input` | `lines`, what the act said, and `refusal` where it ended in one |
| `GET /rooms/{roomId}/send-verbs` | `via` | a page of the verbs the road a room is answered on performs |
| `POST /owner/messages` | `chatId`, `text`, `replyTo?` | `id`, the message kept or sent |
| `POST /owner/message-files` | a form: `chatId`, `text?` (the caption), `replyTo?`, and up to ten `file` fields | `id`, `sent` (each `name`, `id`), `checking`, `failed` |

- **`tool` and `input`:** `say`, `photo`, `remind`, `react`, `pin`, `sticker` and `random_draw`, each the arguments
  its tool takes (`docs/chat-mcp.md`). A file send is `document`, `audio`, `voice` or `album` in the chat protocol,
  and an owner's file send is the form above. The act goes out as the asker's agent, on the road the turn names.
- **`remind`** is carried to the brain's reminder route and answered with the reading the brain gives. Asked
  outside any agent's turn, it is refused, since a reminder is one agent's.
- **The owner's words** in the owner's private room are archived under a console number and handed to that chat's
  conversation; in another room they go as the owner, on the road the room declares for the owner's voice. A file
  past the road's ceiling is refused 413 `too-large`; a `replyTo` the archive does not hold is refused 409.

## Consult lines

| Route | Body or query | Answers |
|---|---|---|
| `POST /consult-lines` | `agent`, `text`, `replyTo?` | `room`, `id`, `lines`: each agent's window in the consult room |
| `GET /consult-lines` | `agentName` | `block`, what the agent is shown of the other agents' work, or null |

## Talks

| Route | Body | Answers |
|---|---|---|
| `POST /talks` | `topic`, `turns` (each `owner`, `text`), `said` | `answer`, or null where the agent said nothing |

- **A topic is the caller's:** `kind` names what it is about, `heading` is the paragraph the turn opens on, and
  `fields` are the values that head carries. The receiver enumerates no kind; `item` and `document` are read as the
  talks they name, and any other kind is read from the heading and fields as given.
- **A talk that comes back with nothing is refused:** `conflict` with `busy` where the talk is being answered,
  `not-ready` where the chat agent's conversations do not stand, `conflict` with `resting` where the agent rests,
  429 `limit` where its provider's limit is reached, and 503 `outage` where its provider is out.

## Events and the live board

| Route | Body or query | Answers |
|---|---|---|
| `GET /events` | `after`, `run`, `kinds`, `roomId` | a server-sent stream |
| `POST /events` | `kind`, `data` | 204, the event published |
| `GET /live` | | the live board: `at`, `turns`, `compacting`, `queued` |

- **The stream** opens with `event: run`, replays the ring of the last 500 events after the cursor, then sends
  each as an event named for its kind, a `: ping` comment every fifteen seconds, and `event: stopping` when the
  service stops on purpose.
- **What goes into the stream:** a message the receiver kept or wrote again as `message.created`, a room's window
  opened or restarted as `window.opened`, a room whose registration was removed as `room.deleted`, the picture
  whenever it changes as `live.changed`, and what `POST /events` says: `message.created`, `briefing.created` or
  `avatars.changed`.

## Hooks

| Route | Body | Answers |
|---|---|---|
| `POST /hooks/{point}` | the agency's `{point, ask}` | `answer` |
| `POST /warmer/ping?token=` | `consumer`, `target`, `kind`, `subject`, `boundMs` | `outcome` (`warmed`, `busy`, `skipped` or `failed`), `code?`, `reason?` |

- **The points** are `room-title` (the room's title, or null), `typed-line` (the context a typed line goes on
  with, or null) and `road-verbs` (the verbs the road performs).
- **The warmer's ping** is the one route outside the bearer check; the token stands in the URL the receiver
  registered.

## Rounds

| Route | Query | Answers |
|---|---|---|
| `GET /rounds/openings` | `room`, `rounds` (comma-separated) | a page of `{round, instructions}`, each instruction `id`, `sort`, `summary`, `createdAt` |
| `GET /rounds/openings/{instructionId}` | `room` | `id`, `text`: one quiet instruction whole |

## Companions

| Route | Body or query | Answers |
|---|---|---|
| `POST /companions` | `name`, `label`, `provider`, `model`, `worker`, `room` | a page of companions, and `room` (`id`, `takenBack`, `said`) |
| `DELETE /companions/{companionName}` | `by` | a page of companions |

- The receiver writes the companion's console room, its number there and the room's answerers; the agent's half is
  the agency's, and its gate options the gate's. A field the agency refuses comes back as the refusal of that
  field (`name`, `label`, `provider`, `model`, `worker`; `agent` as `not-companion`; any other as `upstream-failed`).

## The chat services

The receiving side of bmo-chat's protocol (`docs/protocol.md`), served through `bmo-chat-client` under `/chat`, so
a provider's base address names that prefix. Every route here is the protocol's and its `openapi/receiver.json`
states it; it is repeated here only so one document holds every route the receiver serves.

| Route | What a provider does |
|---|---|
| `PUT /chat/rooms/{roomId}/messages/{messageId}` | hands over a message, live, edited or read back |
| `PATCH /chat/rooms/{roomId}/messages/{messageId}` | hands over the files a gathering message promised |
| `GET /chat/rooms/{roomId}` | asks the room as the receiver holds it |
| `GET /chat/rooms` | asks the rooms whose past it may read back |
| `POST /chat/avatar-changes` | tells of kept profile pictures that changed |

The receiver serves the protocol's shapes as `bmo-chat-client` defines them: the client is the published one, and
the receiver holds no hand-written copy of them. What differs between providers is read from what each declares.

## Calls the receiver makes

Each call goes to the serving unit's published client, at the configured base address with that unit's token. The
receiver's ports are its own vocabulary; where an upstream publishes a client, that client is the one the port
calls, and where an upstream has not fixed a part, the gap is stated in `docs/boundary-api.md`, 「Contract gaps」,
and in this repository's delivery report.

| Unit | What the receiver reaches it for |
|---|---|
| the agency | agents, sessions and their turns, events and hooks, the rest and its stand-in, a room's holdings, companions, a run's log and reset point, a room forgotten, the deployment's documents |
| the brain | room rules, reminders, a session's loaded documents, a chat session's handoff note, a room's person book, what a turn wrote, a room forgotten or removed |
| bmo-job | a room's holdings, the questions the owner answers in his chat, the items a turn filed, the queue moved |
| the gate | a turn's reference, the sanitiser, a room's judgements, a companion's gate options |
| the web unit | the account model: a sender's user, the checks, the grants, a fixed line rendered |
| the cache warmer | the consumer and its one target, the chat touch |
| each chat service | every provider route of the protocol, through `bmo-chat-client` |

A call is retried only where the connection was refused, within two minutes, offered every two seconds.
