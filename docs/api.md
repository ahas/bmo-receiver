# The receiver's fixed API

**Fixed at 0.1.0-rev.1.**

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
| 404 | `not-found` | the room, message, instruction, run or agent named does not exist |
| 409 | `conflict` | the request contradicts what stands; the answer carries the refusal's own code |
| 413 | `too-large` | a file is past the ceiling a road takes |
| 429 | `limit` | a talk's provider limit is reached |
| 502 | `upstream-failed` | a call the route needed on another unit failed; the sentence names it |
| 503 | `not-ready` | the service is up but a part it needs does not stand yet |
| 503 | `outage` | a talk's provider is out |
| 500 | `failed` | the service failed while answering |

A route's own refusals, the facts the receiver hands a room's agent and the fixed lines a room hears are the codes
of `docs/codes.md`, each with its status and its fields.

## The clients

`docs/openapi.json` is one document, and both clients are written from it.

- **The Rust crate** `receiver-client`, in this workspace, holds every body and member as a type and one call for
  each route.
- **The TypeScript package** `@bmo/receiver-client`, in `crates/receiver-client/ts`, holds the same types and one
  call for each route. `receiverClient({ baseUrl, token })` makes a client; a call answers the route's type, and
  a refusal throws `ReceiverError` carrying its status and code. The generated types stand at
  `@bmo/receiver-client/schema`.

A downstream installs the package from a tarball: `bun pm pack` in `crates/receiver-client/ts` writes it, and
the downstream keeps it under `vendor/` named by the commit it was packed from,
`bmo-receiver-client-<commit>.tgz`, depending on it as
`"@bmo/receiver-client": "file:vendor/bmo-receiver-client-<commit>.tgz"`. The package's version is the
repository's, in `VERSION`; it is never taken from a registry, and no downstream pins it.

## Health

| Route | Answers |
|---|---|
| `GET /health` | `ready`, `reason` (null when ready) and `version` |

## The owner

| Route | Answers |
|---|---|
| `GET /owner` | `senderId`, the owner as a sender, and `room`, the owner's private room: the configuration's `owner_room` where it names one, else the room numbered as the owner's own number |

## Rooms

| Route | Body or query | Answers |
|---|---|---|
| `GET /rooms` | | a page of `Room`, the pinned ones first |
| `POST /rooms` | `provider`, a chat service's key; `mode`, `avatar` or `record-only`; and one of `roomId`, the room as that service numbers it, `contactId`, a contact to start a one-to-one with, or `contactIds`, the contacts to start a group with; `title`, optional, and required with `contactIds` as the group's title. With no `provider`, `title` and `supervisors`, the agents that answer it, make an agent room on the record alone, with no chat service behind it | 201, the `Room` added |
| `GET /rooms/{roomId}` | | one `Room` |
| `GET /rooms/{roomId}/registration` | `by` (the administrator) | the room's registration: `mode`, `providers` and `chosen`, `answerers`, `readOnly`, and `readOnlyBy`, the chat services reading it in the owner's account alone |
| `PUT /rooms/{roomId}/registration` | `by` (the administrator); `mode`, `avatar` or `record-only` | the registration as it now stands |
| `GET /rooms/{roomId}/leaders` | | `room` and `leaders`, the accounts that own a supervisor the room names, each once, in order |
| `GET /rooms/{roomId}/archive` | | `room`, `archivedAt` and `archivedBy`, both null for a room that is not archived |
| `PUT /rooms/{roomId}/archive` | `by` (the administrator, the room's leader or a head of the leader's unit); `archived` | the archive as it now stands; `not-room-leader` (403) for anyone else, `not-found` (404) for a room the receiver does not hold |
| `GET /rooms/{roomId}/title` | | `title`, or null |
| `GET /rooms/{roomId}/answerers` | | `default`, `voice`, and a page of `Answerer` (`agentName`, `defaultAnswerer`, whether it is the supervisor the receiver's default answerer setting names) |
| `PUT /rooms/{roomId}/supervisors` | `supervisors`, the whole list, each a supervisor's name | `supervisors`, the list as it now stands; `unknown-supervisor` (404) for a name the agency does not register |
| `GET /answerers` | `room` (optional) | a page of `{room, agents}`: every room's agents answering it, or one room's |
| `GET /rooms/{roomId}/roads` | | `kind` (`owner` or `group`) and `oneToOne` |
| `GET /rooms/{roomId}/own-number` | `agentName` | `number` (or null), the sender an agent's own words stand under: the number the room's road gave it, else the sender its newest own words in the room's record are archived under |
| `GET /rooms/{roomId}/spoke-between` | `person`, `from`, `to` (epoch milliseconds) | `spoke`, whether the person spoke in that span |
| `GET /rooms/{roomId}/people` | `ids`, comma-separated | a page of `Person`: those among `ids` who have stood as a sender in the room |
| `GET /rooms/{roomId}/owner-spoke` | `person`, `before`, `windowMs` | `owner` and `spoke`: whether the person is the owner, and whether the owner spoke in the span |
| `GET /rooms/{roomId}/situation` | | `text`, the room's situation note as it stands, and `updatedAt`, when it last changed in epoch milliseconds, both null where the room has none |
| `GET /rooms/{roomId}/window` | | `openForMs`, the default answerer's window left, and `closesAt` in epoch milliseconds, or null |
| `GET /providers` | | a page of `Provider` (`key`, `label`, `listsRooms`, `listsContacts`, `startsChats`, `startsGroups`, `roomModes`): every chat provider a room may be on |
| `GET /providers/{key}/rooms` | `after` (a room number), `limit` | a page of `AccountRoom` (`id`, `title`, `kind`, `privateTo`, `held`, `mode`): the rooms the provider's account is in |
| `GET /providers/{key}/contacts` | `after` (a contact number), `limit` | a page of `AccountContact` (`id`, `name`, `kind`, `roomId`, `held`, `mode`): the contacts of the provider's account |
| `GET /rooms/{roomId}/providers` | | `providers`, the keys of the room's chat providers in order, and `chosen`, whether the list was set |
| `PUT /rooms/{roomId}/providers` | `providers`, the whole list | `providers` and `chosen`, as the list now stands |
| `GET /rooms/{roomId}/figures` | `zone`, an IANA time zone name, `UTC` where none | the room's figures: `room`, `messages`, `fromPeople`, `speakers`, `firstAt`, `lastAt`, `lastAnswerAt`, `top`, `byHour`, `agents` |
| `PUT /pinned-rooms` | `roomIds`, the whole order | `roomIds`, the order as it now stands |
| `POST /pinned-rooms` | `room`, one room stood last | `roomIds` |
| `DELETE /pinned-rooms/{roomId}` | | `roomIds` |
| `PUT /room-holders/{holderName}` | `address`, `holdings` (each `name` and `effect`: `kept` or `warns`) | 204 |
| `DELETE /rooms/{roomId}` | `by` (the administrator), `dryRun` | what the removal would do, or what it took |

- **A `Room`** is `id`, `title` (or null), `messageCount`, `lastMessageAt` (or null), `pinned`, `senders`, the
  people that have stood as a sender, `supervisors`, the names of the supervisors the room names to answer it, in
  name order, empty where it names none,
  `ownerRoom`, `registered`, `workRoom`, where the news, questions and answers of work filed from it are told, and `mode`, the mode it was
  added in:
  - `record-only`: the user alone. The room is kept and its messages carried to its providers as any room's, and
    no agent is woken in it.
  - `avatar`: the room's designated agents answer it as any room's do, and every line the receiver sends there
    that names no voice of its own goes in the provider's `human-avatar` voice, the account of the person the
    provider reads.
  - null for a room never added: it stands as the roads that handed it messages make it.
- **A room's registration is read and its mode changed by the administrator alone,** `by` naming him as the web
  unit knows him. `GET /rooms/{roomId}/registration` answers `room`, `mode`, `providers`, `chosen`,
  `answerers`, `readOnly`, whether every chat service on the list reads the room in the owner's account alone,
  and `readOnlyBy`, those services. `PUT /rooms/{roomId}/registration` sets `mode` between `record-only` and
  `avatar` and answers the registration as it now stands, leaving the room's record, title, providers and
  messages as they stand. A room never added is refused 409 `room-not-added`; `avatar` for a room on no chat
  service 409 `no-chat-service`, for a chat service that does not offer it 409 `avatar-unoffered`, and for one
  whose answer for the room is never answered in 409 `room-takes-no-avatar`; a room the receiver does not hold
  404 `not-found`; and anyone but the administrator 403 `not-administrator`. A read-only room is never answered
  in, so no mode makes it answer; what does is its chat service's own account standing in it.
- **Any chat reads as a room.** One never registered reads as one whose registration is gone, and work filed from
  it reaches the owner's private room instead.
- **The answerers** listed are the awake agents the room names; `default` is the default answerer setting, and
  `voice` is the agent that speaks for the room (`docs/voiced-notices.md`, 「The rule」).
- **The figures' `agents`** hold each answering agent's `agentName`, messages, `lastAnswerAt`, the session it keeps
  for the room, the agency's row for it and the state of that session; `runs`, `lastRunAt`, `weekTokens` and
  `totalTokens` stand null until the agency answers a tally by agent and room.
- **A room's chat providers** are one setting of its registration: the providers whose rooms it is on, the web
  console's among them as any other's. A `Provider` is the room's record, the receiver's own road that every
  screen reading the archive shows, and each chat service the configuration lists, each named by its key and
  called by its label. A room whose list was never set is on every provider that has handed it a message or kept
  one in it and on the chat service that holds it, and `chosen` is false; a list set is the room's from then on.
  A list that is empty, names a key no provider stands under or names one twice is refused 400 `bad-request`, and
  a room the receiver does not hold 404 `not-found`. Every message kept in the room goes to each of its providers, and a
  set list rules both ways: a chat service it does not name has its messages for the room not taken and is sent
  nothing of it (`docs/receiver.md`, 「A room's chat providers」).
- **Adding a room** is three choices made in turn, each offered by what the chat service declares and never by
  its name (`docs/receiver.md`, 「Adding a room」): a provider, a room among those `GET /providers/{key}/rooms`
  relays from that service's own list or a conversation started with contacts among those
  `GET /providers/{key}/contacts` relays, and a mode among its `roomModes`.
  - A `Provider`'s `listsRooms` is whether it declares `roomList` (bmo-chat's `docs/contract.md`, 「The rooms of
    the account」), `listsContacts` whether it declares `contactList`, `startsChats` whether it declares
    `startChat` and `startsGroups` whether it declares `startGroup` (「Starting a conversation」 there), and its
    `roomModes` holds `record-only` for every chat service and `avatar` beside it where the service's
    `human-avatar` mode stands active. The record lists nothing, starts nothing and takes no mode.
  - An `AccountRoom` is the room as the service answers it, `id`, `title` (or null), `kind` (`owner` or
    `group`) and `privateTo` (or null), with `held`, whether the receiver already holds the room (its own, one it
    keeps a title or a message of, or one added), and `mode`, the mode the receiver added it in, or null. A room
    held and never added is `held` with a null `mode`. The page and its `more` are the service's; `after` and
    `limit` are handed to it as they came.
  - An `AccountContact` is the contact as the service answers it, `id`, `name`, `kind` (`person` or `bot`) and
    `roomId`, the one-to-one the service's account already has with it (or null), with `held` and `mode` of that
    room as an `AccountRoom` carries them, false and null where it has none. The page is the service's, as above.
  - `POST /rooms` with `roomId` keeps the room under the service's number: its mode, its title (the one given,
    else the one the service gives, else none) and its list of providers set to that service and the record, so
    every mode's messages stand in the archive. With `contactId` or with `contactIds` and `title` it first asks
    the service to start the conversation and keeps the room it comes to the same way; a one-to-one the account
    already has comes to that room. It answers 201 with the room as `GET /rooms/{roomId}` reads it.
  - `GET /providers/{key}/rooms` and `GET /providers/{key}/contacts` for a key no provider stands under are
    refused 404 `not-found`; the first for one that does not declare `roomList` 409 `rooms-unlisted`, the second
    for one that does not declare `contactList` 409 `contacts-unlisted`. `POST /rooms` with a body not of one of
    its three shapes or a key no chat service stands under is refused 400 `bad-request`; `avatar` where the
    service's `human-avatar` mode does not stand active, or the room's `modes` leave it out, 409
    `avatar-unsupported`, asked before any conversation is started; a start of a shape the service does not
    declare 409 `chats-unstarted`; a contact the service does not know 409 `contact-unknown`; a start the service
    refuses 409 `start-refused` with its sentence; a room the service does not say is its own 409 `room-unknown`;
    and a room already added 409 `room-added`. A service that does not answer is 502 `upstream-failed` on each
    route.
- **A body naming no `provider`, a `title` and the `supervisors` that answer it** makes an agent room: the
  receiver numbers it in the record's own range, sets its one provider to the record, writes its title and its
  answering agents, and answers the room as it stands. No chat service stands behind it, and its members wake
  by a call by name or a reply to their words as those of any room do.
- **A room's removal** asks the record what it would take, each declared holder and the agency, and refuses
  anyone but the administrator. `dryRun=true` answers the preview and writes nothing; the answer names
  `goes`, `stays`, `pictures`, `settings`, `folders`, `warned`, `units` and, for the removal itself, `taken`,
  `cleared`, `holders` and `couldNot`.

## Messages

| Route | Body or query | Answers |
|---|---|---|
| `GET /rooms/{roomId}/messages` | `before`, `after`, `limit`, `agentName`, `since`, `marked`, `unread`, `by`, `q` | a page of `Message` |
| `GET /supervisors/{supervisorName}/rooms/{roomId}/messages` | `before`, `after`, `limit` | a page of `Message` |
| `GET /rooms/{roomId}/messages/{messageId}` | | one `Message` |
| `GET /rooms/{roomId}/pins` | | `items`, the room's pinned messages, newest pin first, each a `Message` |
| `PUT /rooms/{roomId}/messages/{messageId}/reactions/mine` | `emoji` | the `Message` as it now stands, `notCarried` where a chat service did not take the act (`docs/messages.md`, 「What an agent can send back」) |
| `DELETE /rooms/{roomId}/messages/{messageId}/reactions/mine` | | the `Message` as it now stands, `notCarried` as above |
| `PUT /rooms/{roomId}/messages/{messageId}/pin` | `on` | the `Message` as it now stands, `notCarried` as above |
| `PATCH /rooms/{roomId}/messages/{messageId}` | `text` | the `Message` as it now stands, `notCarried` as above |
| `DELETE /rooms/{roomId}/messages/{messageId}` | | the `Message` as it now stands, `notCarried` as above |
| `GET /stickers` | | `items`, each `emoji`: the stickers the record keeps |
| `POST /rooms/{roomId}/stickers` | `emoji` | 201, the `Message` the sticker was kept as |
| `GET /rooms/{roomId}/messages/{messageId}/metadata` | | `metadata`, what the chat service handed over beside it, or null |
| `GET /rooms/{roomId}/messages/{messageId}/sender-is-owner` | | `owner` |
| `GET /rooms/{roomId}/messages/{messageId}/files` | | a page of `FileEntry` |
| `GET /rooms/{roomId}/messages/{messageId}/files/{place}` | `download` | the file's bytes |
| `GET /rooms/{roomId}/messages/{messageId}/picture` | | the picture's bytes |
| `GET /rooms/{roomId}/day-start` | `from` (epoch milliseconds) | `messageId`, the first message a person said at or after it, or null |
| `GET /rooms/{roomId}/pictures` | `at`, `before`, `after`, `limit` | a page of `Picture`, `before` and `total` |
| `GET /avatars` | `after`, `limit` | a page of `Avatar`: the profile pictures kept, oldest taken first |
| `GET /rooms/{roomId}/avatar` | | the room's kept profile picture, as its bytes |
| `GET /people/{personId}/avatar` | | the person's kept profile picture, as its bytes |
| `PUT /avatars/{kind}/{subject}` | a form with one `picture`; `kind` is `room`, `person` or `agent` | `items`, each `kind`, `id` and `takenAt`: the picture set, under every number the agent stands under for an agent |
| `DELETE /avatars/{kind}/{subject}` | | `items`, each `kind` and `id` whose console-set picture went |

- **`q` searches the room:** the page holds only the messages whose words carry the text, case set aside and
  read as given (no pattern), newest page first and walked by `before`, each in the one message shape; `by`
  narrows it as it narrows any page, and a deleted message's cleared words carry nothing.
- **A supervisor reads a group room's messages through its own route,** `GET
  /supervisors/{supervisorName}/rooms/{roomId}/messages`, newest page first and walked by `before` or `after`.
  The room's answerers read it, and so does a member's own supervisor where the account model links the
  supervisor's `ownerAccount` to a sender that has stood in the room; a member's read writes nothing, so it
  opens no window, wakes nobody and makes no answerer of the reader. A room that is the owner's private chat or
  a one-to-one is refused 403 `not-a-group`, and a supervisor that is neither its answerer nor a member's is
  refused 403 `not-a-member`; an unregistered name is 404 `unknown-supervisor`.
- **A `Message`** is `id`, `room`, `senderId` (or null), `senderName`, `sentAt`, `text`, `imagePath` (or null),
  `replyToMessageId` (or null), `answerSeconds`, `round`, `album`, `via`, `agentName` (or null), `voice` (or null),
  `transcript` (the words heard in a voice note, or null), `voiceSeconds` (how long the recording ran, or null),
  `voiceState` (`heard`, `failed`, or null), `kind` (`sticker`, `thinking` for a thinking block a record holds as a message, or null for any other), `editedAt`, `deletedAt` and
  `pinnedAt` (epoch milliseconds, or null), `reactions`, each `emoji` with `by`, who put it there (an agent's name
  or a person's number), `forwardedFrom` (`room`, `messageId` and `senderName` of the message it copies, or null),
  `situation` (the room's situation note as it stood when the round that judged this message opened — its `text`,
  and `changed`, which is false since no round writes the note, the owner's private chat's messages included — or
  null on a message no round judged under a note; `docs/drafting.md`, 「The situation note」), and `files`, each a `KeptFile` (`kind`, `source`,
  `storedPath`, `summary`, `collectedAt`). `notCarried` stands on the answer of an act on a message alone, and only where one of the
  room's chat services did not take that act: each entry its `provider` key, its `reason` (`no-copy` where the service holds no number
  for the message, `refused` where the service turned the act down in the voice named and in the room's own voice where that was tried,
  `no-service` where the room's list names a provider no chat service stands under) and the service's own `detail` sentence where it
  gave one. The record keeps the act all the same, and where every provider it was carried to took it, the answer names no `notCarried`.
- **An `Avatar`** is `kind` (`room` or `person`), `id` and `takenAt` (epoch milliseconds); the page is walked by
  `after`, a `takenAt`, and `limit`. The bytes routes answer 404 `not-found` where no picture is kept for the number
  or its file is gone (`docs/pictures.md`, 「Profile pictures」). A picture set or let go is refused 403 `picture-forbidden`
  to a caller the relay's `Owner-Key` and `Owner-Role` do not let set it (`docs/pictures.md`), 400 `unknown-kind`
  for a kind other than the three, 400 `no-picture-given`
  where the form holds no `picture`, 400 `not-a-picture` where it cannot be read as one, 413 `picture-too-large`
  past `files.toml`'s `largest_bytes`, and an agent standing under no number 404 `no-number`.
- **`agentName` and `voice`** stand on a send the receiver archived itself, naming the agent whose words these are
  and the voice the send went out in (`bot-account`, `human-avatar`, `human-reader`); both are null on a message
  that came in from a provider, which the provider's echo of the receiver's own send keeps, since that message is
  the one already kept with its fields. So an answer sent in the owner's `human-avatar` account is read as the
  agent's with its voice named, not guessed from the number it went out under.
- **The message page** holds only what a person said unless `by=agent` or `by=any` says otherwise. It is walked by
  `before` or `after`, a message the reader holds, and ends on the side it walked from. With `agentName`,
  `unread=true` answers the messages past that agent's read marks, oldest first, from `since` where given; with
  `marked=true` it answers the messages the agent has marked, after `after`. This is the one message read: there is
  no second route for the same message.
- **The bytes** answer as a file does over HTTP: `Content-Type`, `Content-Length`, `Accept-Ranges`, and `Range`
  answered 206 or 416. `download=true` adds `Content-Disposition: attachment`. Only bytes inside the room's own
  folder are served; anything else is 404 `not-found`.
- **The metadata is opaque:** it is kept and handed back unread, so what it holds is the chat service's to state;
  only its strings are read, each for secret-shaped runs as a message's words are (`docs/messages.md`, 「Secrets
  taken out」).
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
| `POST /rooms/{roomId}/notices` | `fact`, `intent`, `followUp?`, `optional`, `kind?`, `events?`, `ridesUntil?` | 201, `{id}`: the instruction it was queued as |
| `POST /owner/notices` | `code` and its parameters beside it | 201, `{id}` |

- **A `Notice`** is `id`, `kind`, `summary`, `text` and `createdAt`. The list is walked by `before` or
  `after`, an instruction id, and narrowed by `kind` and by the span it was queued in, both ends included.
- **An owner notice** is queued for the default answerer in the owner's private room, carrying the code and its
  parameters as a fact.

## Instructions

| Route | Body or query | Answers |
|---|---|---|
| `POST /instructions` | `agentName?`, `room?`, `said` (`fact` or `order`), `sort`, `summary?`, `kind?`, `cause?` (`reply` or `housekeeping`), `events?`, `agentOnly?`, `ridesUntil?`, `via?`, `run?`, `publisher?` | 201, `{id}` |
| `GET /instructions/{instructionId}` | | `id`, `state` (`pending`, `handed` or `delivered`); 404 where the row was let go — withdrawn, replaced, or ended without a turn carrying it (`docs/bmo-window.md`, 「Quiet instruction」) — though the row stays kept |
| `DELETE /instructions/{instructionId}` | | 204 |
| `POST /owner/instructions` | `chatId`, `text` | 204 |

- **An instruction** is queued for the agent's session in the room, and a `kind` replaces the pending instruction
  of the same kind there. **A job's status notice** — an order under the sort `job` — goes to the room's
  answerer; where the room names no answerer, to the requester's private room; where that names none, to the
  default answerer. An `agentName` that answers the room is kept; one that does not is replaced by this order. Every
  other instruction keeps the room it names, and its voice is the room's answerer or the default answerer. One marked
  `agentOnly` is a record the agent reads and no room hears: a round carrying only such rows sends no room
  message (`docs/bmo-window.md`, 「Quiet instruction」). An instruction for a room whose registration is gone
  goes to the owner's private room instead, naming the room, and never into the platform room.
- **Its `publisher`** names the unit or source that published the row, kept with it and carried into every turn
  it rides. A row that names none is read by the first word of its `kind`; one that names neither stands as
  `the system` (`docs/bmo-window.md`, 「Quiet instruction」).
- **Its `cause`** says whether somebody waits on it (`docs/wake.md`, 「Quiet instructions」). `reply`, the default,
  is a room reply: it answers something a room or the owner asked or set going — a research run's result, a
  delegate's or a worker's answer, a job's ending and report, a reminder or a watch come due — and opens the
  agent's turn as soon as it is queued, on a session past its cache lifetime too, which that turn replaces.
  `housekeeping` is a record for the agent alone that nobody waits on, and waits for a round something else opens
  where the session is past its cache lifetime; one marked `agentOnly` is housekeeping whatever it names.
- **A quiet instruction** is one with `kind` `watch` or `reminder` and a `said.order`: it is queued under that
  kind and opens the agent's turn there.
- **A withdrawn instruction** that was handed or is gone is no refusal; it is simply no longer pending.

## Sends

| Route | Body or query | Answers |
|---|---|---|
| `POST /sends` | `asker` (`room?`, `via?`, `agentName?`, `began?`, `by?`, `job`, `from?`), `tool`, `input`; the `say`, `photo`, `file`, `voice`, `sticker` and `speak` acts take an optional `replyTo`, the message in the room the send replies to | `lines`, what the act said, and `refusal` where it ended in one; `handed`, the instruction's number, where it was handed to the room's supervisor; `own` true for a `speak` asked in the asker's own round |
| `GET /rooms/{roomId}/send-verbs` | `via` | a page of the verbs the road a room is answered on performs |
| `POST /owner/messages` | `chatId`, `text`, `replyTo?` | `id`, the message kept or sent |
| `POST /owner/message-files` | a form: `chatId`, `text?` (the caption), `replyTo?`, up to ten `file` fields, and `voice?` (`true` to send the one file as a voice note) | `id`, `sent` (each `name`, `id`), `checking`, `failed` |
| `POST /rooms/{roomId}/messages/{messageId}/forwards` | `to`, the room it is forwarded into | 201, `room` and `id`, the copy kept there, its stored picture kept again under it |

- **`tool` and `input`:** `say`, `photo`, `file`, `voice`, `fixed_line`, `react`, `pin`, `sticker` and
  `random_draw`, each the arguments its tool takes (`docs/chat-mcp.md`), and `speak`, a spoken talk asked by its
  `voice`, its written `text` and its `spoken?` words. The act goes out as the asker's agent, on the road the turn
  names, only where that agent's round is working in the room `by` names and no job asks; every other send is
  handed to the room's supervisor as a quiet instruction and nothing goes out (`docs/messages.md`, 「Words handed
  to the room's supervisor」). `from` names the command or unit the words came through, which the handed words
  carry. A `speak` sends nothing either way: in the asker's own round it answers `own` true and no lines, and the
  command sends the note it makes as `voice`.
- **`began`** is the moment the asking turn took its words, in epoch milliseconds. A send whose
  `began` is at or before its agent's reset point, the agency's as the receiver reads it then, is refused before
  anything else, its `refusal` (code `overtaken`, 409) telling the turn to end (`docs/wake.md`,
  「An agent's reset」).
- **A voice briefing turn sends nothing.** While the receiver carries a notice turn's summary or a line's full
  reading for an agent in a room, a send that agent asks into that room, of any act and from no job, is refused
  `briefing-turn`, its `refusal` saying that the turn's answer is the script itself and nothing is sent into the
  room from it (`docs/voiced-notices.md`, 「The summary of a notice turn」).
- **A `say`** answers once the line is on its way: the receiver puts the words through the room's gates and into
  the room apart from the call, opens the room's window for the agent that said it, and tells the chat that set
  it off what became of them. A launcher the binary stands carries it as a run of its own where one stands.
- **A photo send** is one picture or several as one album: each `url` is a web address or a file under the data
  root, with an optional `label` drawn into it, and an optional `caption`. A web address goes as it stands for the
  road to fetch; a file is read here and goes as its bytes. An address that is neither is refused before anything
  goes, and every picture that did not go is named with why. A picture kept in the room is filed under its
  message (`docs/pictures.md`, 「A picture an agent sent」).
- **A file send** is one document, or several files as one album: each file a `path` on this machine and an
  optional `name`, with an optional `caption`. The bytes go to the verb the room's road declares, `document` for
  one file and `album` for several; a file a room's folder holds goes only into that room, and a job sends none.
  Each file that went is filed under its message (`docs/room-files.md`, 「Message files」).
- **A voice note** is an OGG/Opus file at `path`, with its `caption`, what it `said` and the `transcript` read out,
  sent as the chat protocol's `voice` kind.
- **A fixed line** is a `code` and its `fields`, rendered by the web unit's `POST /words/render` in the
  rooms' language and said into the room without the output gate or any session, since the line is already read.
  Where the web unit cannot render it, the line is the policy's plain sentence for the code in the rooms'
  language; a code with no sentence anywhere is refused and nothing is said (`docs/voiced-notices.md`, 「Lines that
  stay fixed」).
- An owner's file send is the form above.
- **An owner's voice note** is the same form with `voice` true: one `file` that is a sound. The room keeps it
  as one message under the policy's voice word and the caption beside it, its file the recording, and hands it to
  the room's other providers as the chat protocol's `voice` kind with the caption. A call with no file, more than
  one, or one that is no sound is refused 400 `bad-request`; a file past the ceiling a road takes is refused 413
  `too-large`.
- **The owner's words** from the console are archived in the room under a console number, the owner's number and
  the owner's own name, handed to the room's conversations as any of the owner's words are, and carried to the
  room's other chat providers as every kept message is. A file past the ceiling a road takes is refused 413
  `too-large`; a `replyTo` the archive does not hold is refused 409.
- **The signed-in person's acts on a message** come through the console relay, which names the caller by
  `Owner-Key`, the login's user id, and `Owner-Role`, as a picture's setting does (`docs/pictures.md`, 「Profile
  pictures」); no body names who acts, so a member acts only as themselves. The caller's senders are those the web
  unit links to the user, and the caller acts under the one of them the room already holds, else the first. The
  record keeps each act as the agent-side acts are kept (`docs/messages.md`, 「What an agent can send back」) under
  that number and tells `message.changed`. Where the caller is the owner, the act goes on in his own voice to the
  room's other providers wherever the message stands under a number of theirs, and a provider that refuses the
  act in his own voice is handed it again in the voice it answers the room by, unless it reads the room in the
  owner's account alone, so a reaction or its removal reaches the room's chat service even where the owner's
  account sends only words and files; a member's act stands on the record alone, since no road speaks in a
  member's account. Where the act was carried, every provider that did not take it is named in the answer under
  `notCarried` and said in the service's log, and the act stands on the record all the same: the answer is still
  the message as it now stands, and the screen reads that row as it does for any act. A reaction is the caller's
  own, one to a reactor; a pin anyone
  signed in sets or takes off; an edit is the caller's on a message one of their senders said, and a delete the
  same or the administrator's on any; a sticker is a message of the caller's own, its kind `sticker`, carried as
  the owner's words are where the caller is the owner. A caller the relay names no user for, or whose user holds no
  sender, is refused 403 `no-caller`; a message the room does not keep 404 `message-unknown`; a deleted one 409
  `message-deleted`; another person's message to edit or delete 403 `message-forbidden`; an emoji the record's set
  does not hold 400 `unknown-sticker`; a reaction with no emoji or an edit with no words 400 `bad-request`. The
  record's set is the policy's (`roads.toml`, `[record]`, `stickers`), and a road the sticker goes on picks its own
  sticker for the emoji.
- **A room's pinned messages** are the ones whose pin stands on the record, a deleted message none of them, newest
  pin first.
- **A forward** copies a message the owner picks into another room: kept there under a console number as the
  owner's, his number and the name that room shows him by, its words the message's own, marked `forwardedFrom`
  with the room, the message and who said it, and carried to that room's providers as every kept message is, so
  it goes out on the room's own road. It wakes nobody. A message or a room the receiver does not hold is refused
  404 `not-found`, a forward into the room it stands in 400 `bad-request`, and a deleted message 409 `conflict`.

## Consult lines

| Route | Body or query | Answers |
|---|---|---|

## Talks

| Route | Body | Answers |
|---|---|---|
| `POST /talks` | `topic`, `turns` (each `owner`, `text`), `said` | `answer`, or null where the agent said nothing |

- **A topic is the caller's:** `kind` names what it is about, `heading` is the paragraph the turn opens on, and
  `fields` are the values that head carries. The receiver enumerates no kind; `item` and `document` are read as the
  talks they name, and any other kind is read from the heading and fields as given.
- **A talk that comes back with nothing is refused:** `conflict` with `busy` where the talk is being answered,
  `not-ready` where the default answerer's conversations do not stand, `conflict` with `resting` where the agent rests,
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
  opened or restarted as `window.opened`, a room's window for an agent closed as `window.closed`, carrying the room,
  the agent and the moment it closed, a verdict the receiver wrote of a round as `judgement.created`, carrying the
  room, the message it stands under or null, the stage, the verdict and its detail, a room whose registration was
  removed as `room.deleted`, an act the record kept on a message, a reaction, a pin, an edit or a delete, as
  `message.changed`, carrying the room and the message, the picture
  whenever it changes as `live.changed`, and what `POST /events` says: `message.created`, `briefing.created` or
  `avatars.changed`.
- **A turn's events name whose turn it is.** Every event a conversation tells of its room's turn — `run-started`,
  `run-progress`, `sent`, `draft`, `run-failed` and `waited` among them — and the turn as the picture holds it
  carry `speaker`, `{id, name}`: `name` is the registration name of the agent whose turn it is, the default
  answerer's as much as any other agent's, and `id` is the number that agent's words are filed under in the room —
  the room's own identity for the default answerer, the number the room's road or the record gave any other
  agent — or null where the room gives it none. The speaker is read as each turn begins, so an event told of a
  room no turn has begun in yet carries none.
- **`run-progress` carries the working turn's picture** as `progress`, with `chatId` and `messageId`: its
  `phase`, `tool`, token counts and `spent`, its last `actions` with `droppedActions` and `droppedContext`, and
  `said`, the turn's last three progress lines, oldest first, each `{text, before}` — the words it wrote and then
  went on working, redacted, run into one line and cut at 240 characters, never its answer
  (`docs/receiver.md`, 「The event stream」) — and `thinking`, the last thinking block the run wrote, redacted and
  cut at the policy's ceiling, on a turn working in the owner's private chat and null on every other
  (`docs/messages.md`, 「A working turn's thinking」).

## Hooks

| Route | Body | Answers |
|---|---|---|
| `POST /hooks/{point}` | the agency's `{point, ask}` | `answer` |
| `POST /warmer/ping?token=` | `consumerName`, `targetName`, `subjectName`, `kind?`, `dueAt`, `boundMs` | `outcome` (`warmed`, `busy`, `skipped` or `failed`), `code?`, `reason?` |

- **The points** are `room-title` (the room's title, or null), `typed-line` (the context a typed line goes on
  with, or null) and `road-verbs` (the verbs the road performs).
- **The warmer's ping** is the one route outside the bearer check; the token stands in the URL the receiver
  registered. Its body is the warmer's own published body, `kind` among it optional, and a `skipped` answer
  carries its code (`docs/codes.md`, 「SkipReason」), which the warmer requires of every skip.

## The voice briefing

| Route | Body | Answers |
|---|---|---|
| `GET /voice-briefing` | | `on` and `autoplay`, the two switches as they stand |
| `PUT /voice-briefing` | `on` or `autoplay`, exactly one, a boolean | `on` and `autoplay`, as they now stand |
| `POST /rooms/{roomId}/messages/{messageId}/reading` | | `state` (`making` or `ready`) and `briefingId`, the engine's briefing of the line's full reading, null while the line's agent is still writing its script |

- **The switches** are the whole briefing's and whether a summary plays by itself (`docs/voiced-notices.md`, 「The
  voice briefing」); each is on unless it was set off. A body naming neither switch, or both, or a value that is not
  a boolean, is refused 400 `one-switch`. Setting `on` to false asks the voice engine to stop every briefing it
  holds; the answer waits on none of it, and a stop that failed is asked again until it is made.
- **A line's full reading** is launched in the voice engine where none stands, or answered as it stands there. It
  is refused 409 `switched-off` while the whole briefing is off, 409 `voiceless` where the line's agent has no
  voice, 404 `not-found` where the message is no agent's line with words, and 502 `unreachable` where no voice
  engine is configured or the engine did not answer. The sound is the engine's: the console reads it there by the
  briefing's id.

## Handoffs

| Route | Body | Answers |
|---|---|---|
| `POST /handoffs/{agentName}` | `reason`, `room?` | 200 `path`, `bytes`: the handoff file the agent's standing session wrote; 204 where it has no session to ask or wrote nothing |

- **The agency asks this before it opens a fresh session for an agent that had one** (`docs/unified-bmo.md`,
  「Replacing the session」). The receiver asks the agent's standing session — its one session, or the session of
  `room` where it keeps one in each room — for its handoff file as a replacement does, waited on until its turn
  ends however long it takes, and keeps the file for the next fresh session's first turn, which opens on it. A
  session already in a turn is waited for: the receiver waits for that turn's end on the agency's event stream and
  asks again, however long it takes. The handoff turn answers `room`, carrying its block, and the owner's private
  chat where none is named. `reason` names why a fresh session is coming, for the receiver's journal.
- **The agency lets its record of the session go before it asks,** so the session's state names no generation
  while its program still stands. The receiver reads no generation to decide whether to ask: it asks the standing
  session, and the asking itself tells whether there was one to ask.
- **An asking while a handoff of the same agent is under way waits in line** behind it, and is then answered as
  an asking again within `again_minutes` is.
- **An asking within `again_minutes` of the last one asked** (`handover.toml`) does not reach the standing
  session: it answers 200 with the file that one kept where it is still kept, else 204, and the journal counts
  the askings in a row.
- **A failure to reach the agency** answers 502 `upstream-failed`.

## Handovers

| Route | Body | Answers |
|---|---|---|
| `POST /handovers/{agentName}` | `reason` | 202 `agentName`, `sessionReason` (`asked`): the agent's one session hands over once the agent is quiet |

- **A handover is asked for the agent's one session across its rooms** (`docs/unified-bmo.md`, 「Replacing the
  session」). The route writes the word to start that session afresh with the reason `asked` and answers at once;
  it waits on no turn, so an agent may ask it of its own session from inside its own turn. The session is
  replaced once the agent is quiet — at once where it is in no round, else as the round it is in ends — as every
  replacement is: the old session writes its handoff file, is ended, and the new session's first turn opens on
  the note.
- **`reason`** is a non-empty English sentence saying why, for the receiver's journal; an empty one is refused
  400 `bad-request`. The journal holds one line naming the agent and the reason.
- **An agent with no opened session** is given the word all the same, and its next turn spends it in the one
  replacement. A second asking before that turn writes the same word again, so one replacement answers both.
- **An agent the agency does not register** is refused 404 `not-found`, **one that keeps a session in each room**
  409 `conflict`, since a room's own session is the agency's to resume, and **a failure to reach the agency** 502
  `upstream-failed`.

## Rounds

| Route | Query | Answers |
|---|---|---|
| `GET /rounds/openings` | `room`, `rounds` (comma-separated) | a page of `{round, instructions}`, each instruction `id`, `sort`, `summary`, `createdAt` |
| `GET /rounds/openings/{instructionId}` | `room` | `id`, `text`: one quiet instruction whole |
| `GET /rounds/drafts` | `room`, `rounds` (comma-separated) | a page of `{round, rows}`, each row a `DraftRound` |

- **A `DraftRound`** is one turn of a round's draft process, or its end (`docs/drafting.md`, 「The round log」):
  `id`, `agentName`, `number` (from 1, the round's own turn being the first question's row; 0 for the agent's own
  judgement before a round), `question`, `kind` (`supervisor`, the agent's own judgement whether to answer words
  its open window alone let in, `turn`, `question`, `item`, `check` or `end`; `turn` only where the row's list is
  empty), `answerChars`,
  `changed`, `outcome` (`answered`, `send`, `no-verdict`, `skip` or `failed` for a turn; `PASS` or `REJECT` for a
  review; `message`, `skip` or `withheld` for the end), `detail` (or null: a row ending `skip` names there why the
  round sent nothing, or `none given`), `answer` (or null), `ms`, `at` and `reads`:
  on the end, what the round read from the brain, each `path`, `holds` (or null) and `how` (`attached`, `held` or
  `read`; `docs/drafting.md`, 「The round log」), and empty on every other row; `session` and `loaded`: on the end,
  the agency session the round's turn ran on (or null) and how many pieces that session's earlier rounds in the room
  read in full (`docs/drafting.md`, 「The round log」), and null on every other row. The rows of a round stand in the
  order they were written.

## The draft process

| Route | Body | Answers |
|---|---|---|
| `GET /draft-processes` | query `agentName?` | a page of `DraftProcess`: the policy's row and each supervisor's own; with `agentName`, the one row that supervisor reads |
| `PUT /draft-processes/{provider}/{tier}/{agentName}`, the provider and the tier `*` and the agent a supervisor's name | `rounds`, `off?`; `light?`, `lightChars?`, `sendFrom?`, `turnSeconds?`, `ceilingSeconds?` and `pastCeiling?` are read and ignored | the `DraftProcess` as it now stands |
| `DELETE /draft-processes/{provider}/{tier}/{agentName}` | | 204; `not-found` where the console set no row of the key |
| `GET /draft-questions` | query `agentName?` | a page of `DraftQuestion`: that supervisor's catalogue, or the policy's alone where none is named |
| `PUT /draft-questions/{agentName}/{name}` | `text` | the `DraftQuestion` as it now stands |
| `DELETE /draft-questions/{agentName}/{name}` | | 204; `not-found` where the console set that supervisor no text of the name, `conflict` where a list of its row still names a question the console added |

- **A `DraftProcess`** is `provider`, `tier` and `agentName`, each a name or `*` for any; `rounds`, the
  names of the questions every turn's round follows, in order; `off`, the questions of that list that are switched
  off, each keeping its place and not followed; no turn bound stands; and `source`,
  `set` where the console set the row and `default` where the policy's stands (`docs/drafting.md`, 「The process
  table」). A
  row that still carries `sendFrom`, `ceilingSeconds` or `pastCeiling` is read with them ignored, and they stand
  in its answer as they came. The page holds the policy's rows in their order,
  each replaced by the console's of the same key, then the console's own keys.
- **A `DraftQuestion`** is `name`, `text` and `source`. The page opens on `supervisor`, the text the agent's own
  judgement of whether to answer opens on (`docs/drafting.md`, 「The routes」), set like any question; a list that
  names it is refused `bad-request`.
- **A set is refused `bad-request`,** with nothing written, where a list names a question the catalogue does not
  hold, where a list that holds questions does not end in `final` or holds it before its end, where `off` names
  `final`, and where a question's name is not a lowercase word of letters, digits
  and dashes of at most 40 characters or its text is empty or past 4,000 characters. A name `off` holds that
  the list does not hold is dropped, and the answer carries `off` as it now stands.
- **A question's text taken away** leaves the catalogue where the console added the question, and stands again
  in the policy's text where the policy holds it.
- **A set or a removal that changes the instructions a session reads for the process** has the agent's one
  session replaced through its handoff before its next turn (`docs/drafting.md`, 「The questions」).

## Supervisors

| Route | Body or query | Answers |
|---|---|---|
| `POST /supervisors` | `name`, `label`, `repository` (optional), `room`, and the order: `order` and `quotaShare`, or `provider` and `model`, or both | 201, `supervisor`, the agency's read of it, `brain`, what became of its brain, and `room` (`id`, `takenBack`, `said`) |
| `DELETE /supervisors/{supervisorName}` | `by`, `forgetBrain`, `force` | 204; `unknown-supervisor` (404), `seeded` (409), `default-answerer` (409), `running` (409), `action` (400) |
| `POST /supervisors/{supervisorName}/resets` | `conversation`, `person`, `dryRun`, `by` | the agency's answer to the reset as it came; `unknown-supervisor` (404) |
| `GET /supervisors/{supervisorName}/rooms` | | a page of `SupervisorRoom` (`id`, `title`, `named`, `alone`): the rooms it answers |
| `GET /default-answerer` | | `supervisor`, the name the setting holds |
| `PUT /default-answerer` | `supervisor` | `supervisor`, as the setting now stands; `unknown-supervisor` (404) |

- **Every supervisor is alike to the receiver.** Its rounds are the same rounds, its judgement of window words
  first and its draft process among them (`docs/wake.md`); what tells one from another is its own row at the
  agency and the rooms that name it here.
- **A room names the supervisors that answer it,** the room's `supervisors`. A room that names none is answered by
  the supervisor the user it is a one-to-one with owns, where one stands, and otherwise by the default answerer;
  a room is added naming none, so the default answerer is also the supervisor a new room gets. `PUT
  /rooms/{roomId}/supervisors` writes the whole list, each name one the agency registers; an empty list hands
  the room back to the supervisor its user owns where one stands, and otherwise to the default answerer.
- **The default answerer is a setting of the receiver's,** naming one supervisor: the one that answers every room
  naming none that is no user's own private chat, and every such chat whose user owns no awake supervisor; it
  says the system's own lines there, takes the console's talks, the warmer's touch and a line typed into its
  program, and is told the owner's facts in his private room (`docs/wake.md`, 「Who reaches the
  session」). The store's settings hold it (`docs/store.md`, 「The settings」); a store holding none is seeded
  with the configuration's `agent` as the receiver starts. `PUT /default-answerer` names a supervisor the agency registers and is read by
  the next thing that asks who answers; the clocks the conversations keep (the health check and the restart's
  backlog) stay on the supervisor the receiver started with until it starts again.
- **Adding a supervisor writes both halves at once.** The receiver writes its console room, its number there, the
  room's one provider, the record, and the room's `supervisors`, naming it alone; the supervisor's half is the
  agency's `POST /supervisors`, and its gate options are copied from the default answerer's as they stand. What a
  name may be is the agency's to say. A field the agency refuses comes back as the refusal of that field (`name`,
  `label`, `provider`, `model`, `repository`; an order's member, `order` or `order[<n>]`, as `order` naming it;
  `quotaShare` as `quota-share`; a name the agency holds already as `name-taken`; any other as
  `upstream-failed`). A name the agency registers already is refused `name-taken` before anything is written.
- **The order is added with it, in one ask.** `order` is its provider order whole, each entry a `provider`, a
  `model`, and the `effort`, `thinking` and `answerTokens` the agency's order takes, and `quotaShare` the share
  beside it; the receiver hands both to the agency's add as they came, and the agency judges them as a
  supervisor's order is judged and writes the row with them. Where the console names `provider` and `model`
  alone, the order is that one entry at the agency's default share; where it names both, the order's first entry
  must be that provider and model, or the agency refuses it as `order`.
- **`repository` names the repository** the supervisor's items go to, where it names one: a repository the agency
  does not hold (`GET /repositories`) is refused as `repository` before anything is written.
- **Removing a supervisor asks the agency for its half and takes the receiver's.** Before it asks the agency
  anything, the receiver reads the supervisor (`GET /supervisors/{supervisorName}`): one the agency does not
  register is refused `unknown-supervisor`, one the deployment seeds `seeded` with its `label`, and the one the
  default answerer setting names `default-answerer` with its `label`. It then asks the agency's
  `DELETE /supervisors/{supervisorName}` with `rooms`, every room that names it, read there as rooms it shares so
  each keeps its record, and `forgetBrain` and `force` as the console gave them; the agency's `running` comes back
  as `running` with the `label`. Once the agency has taken its half, the receiver lets go of every number it gave
  the supervisor that no kept message is filed under, with its picture, of every room's naming of it, and of its
  draft process row and its question texts (`docs/drafting.md`, 「The process table」); a room left naming none is
  the default answerer's.
- **A reset is the agency's, with the rooms the receiver names.** `POST /supervisors/{supervisorName}/resets`
  hands the agency's reset (the agency's `docs/supervisors.md`, 「Starting it afresh」) `conversation`, `person`,
  `dryRun` and `by` as they came, with `alone`, the rooms that name the supervisor alone, and `shared`, those that
  name it beside another; a room that names none is handed in neither list, so its record stays whole. The
  agency's answer is the route's, and a body it refuses comes back as `bad-request` with the agency's sentence.
- **A `SupervisorRoom`** is a room the supervisor answers: `id`, `title` (or null), `named`, true where the room
  names it and false where it answers as the default answerer of a room naming none, and `alone`, whether it
  answers there alone.

## The chat services

The receiving side of bmo-chat's protocol (`docs/protocol.md`), served through `bmo-chat-client` under `/chat`, so
a provider's base address names that prefix. Every route here is the protocol's and its `openapi/receiver.json`
states it; it is repeated here only so one document holds every route the receiver serves.

| Route | What a provider does |
|---|---|
| `PUT /chat/rooms/{roomId}/messages/{messageId}` | hands over a message, live, edited or read back |
| `PATCH /chat/rooms/{roomId}/messages/{messageId}` | hands over the files a gathering message promised |
| `GET /chat/rooms/{roomId}` | asks the room as the receiver holds it; `read` is true only for a read-only room added on the asking provider and still on its list (`docs/receiver.md`, 「A read-only room」) |
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
| the agency | supervisors, roles and repositories, sessions and their turns, events and hooks, the rest and whether a supervisor's road answers through it, a room's holdings, the supervisors added and removed, a supervisor's reset, a run's log, a room forgotten, a session's loaded documents, the deployment's documents |
| bmo-job | a room's holdings, the questions the owner answers in his chat, the items a turn filed, the queue moved |
| the gate | a turn's reference, the brain's index, a room's judgements, a new supervisor's gate options |
| the brain | the agent's brain, whether a consolidation is due for a turn, a session's fold reported |
| the web unit | the account model: a sender's user, the checks, the grants, a fixed line rendered |
| the cache warmer | the consumer and its one target, the chat touch |
| each chat service | every provider route of the protocol, through `bmo-chat-client` |

A call is offered again only where the connection was refused, every two seconds until the unit listens; a call
that reached a listener is never offered again, and nothing bounds how long a unit takes to answer. A call on the
brain is offered once, a refused connection ending it (`docs/boundary-api.md`, 「The brain」).
