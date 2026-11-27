# The receiver's fixed API

This document is the receiver's contract: every route it serves, in the one common wire format, grouped by the
resource each acts on. `docs/openapi.json` is the same contract in machine form, generated from the types in
`crates/receiver-client`, and the Rust crate and the TypeScript package beside it are written from that one
document. Where this document and another disagree, this one and the OpenAPI document stand.

The wire format, the API design and the rules between units are bmo-chat's (`docs/conventions.md`); this document
states only what is the receiver's.

## What the receiver is

The receiver owns the rooms and their messages, each room's rights and windows, the envelope a turn goes out
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
- **Auth.** Every route needs `Authorization: Bearer <token>` and answers 401 without it. Two tokens are taken:
  the receiver's own, which every unit calling it and every shell of the machine holds, and the console's, the
  credential the configuration names under `[console]` for the console relay alone (`docs/boundary-api.md`,
  「Configuration」). Either reaches every route but the routes that file a person's words: `POST /owner/messages`,
  `POST /owner/message-files`, `POST /owner/instructions`, `POST /rooms/{roomId}/words`,
  `POST /rooms/{roomId}/word-files`, an edit (`PATCH /rooms/{roomId}/messages/{messageId}`) and a forward
  (`POST /rooms/{roomId}/messages/{messageId}/forwards`). Those take the console's token alone, and a call under the
  receiver's own is refused 403 `forbidden` before anything is read or written, so no shell holding the receiver's
  token speaks in a room as a person; a deployment naming no console token refuses them to every caller. The cache
  warmer's ping is the one route outside the bearer check: its URL carries the receiver's token as `token`, since
  the warmer sends no header.
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
| 413 | `too-large` | a file is past the largest file a chat provider of the room declares it takes |
| 429 | `limit` | a talk's provider limit is reached |
| 502 | `upstream-failed` | a call the route needed on another unit failed; the sentence names it |
| 503 | `not-ready` | the service is up but a part it needs does not stand yet |
| 503 | `outage` | a talk's provider is out |
| 503 | `stopping` | the service was told to stop while the request waited on work that could not end first |
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
`"@bmo/receiver-client": "file:vendor/bmo-receiver-client-<commit>.tgz"`. The package's version field is a
placeholder that never moves; the package is never taken from a registry, and no downstream pins it.

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
| `POST /rooms` | `provider`, a chat service's key; `mode`, `avatar` or `record-only`; and one of `roomId`, the room as that service numbers it, `contactId`, a contact to start a one-to-one with, or `contactIds`, the contacts to start a group with; `title`, optional, and required with `contactIds` as the group's title; `accountId`, optional with `contactId` or `contactIds`, the account the conversation is started in. With no `provider`, `title` and `supervisors`, the agents that answer it, make an agent room on the record alone, with no chat service behind it, and `openedBy`, optional, the account that opens it and is its first member | 201, the `Room` added |
| `GET /rooms/{roomId}` | | one `Room` |
| `GET /rooms/{roomId}/registration` | `by` (the administrator) | the room's registration: `mode`, `providers` and `chosen`, `answerers`, `readOnly`, and `readOnlyBy`, the chat services reading it in the owner's account alone |
| `PUT /rooms/{roomId}/registration` | `by` (the administrator); `mode`, `avatar` or `record-only` | the registration as it now stands |
| `GET /rooms/{roomId}/leaders` | | `room` and `leaders`, the accounts that own a supervisor the room names, each once, in order |
| `GET /rooms/{roomId}/archive` | | `room`, `archivedAt` and `archivedBy`, both null for a room that is not archived |
| `PUT /rooms/{roomId}/archive` | `by` (the administrator, the room's leader or a head of the leader's unit); `archived` | the archive as it now stands; `not-room-leader` (403) for anyone else, `not-found` (404) for a room the receiver does not hold |
| `GET /rooms/{roomId}/title` | | `title`, or null |
| `PUT /rooms/{roomId}/title` | `title`, the name the room is to stand under | the `title` it now stands under; `bad-request` (400) for an empty one, `not-found` (404) for a room the receiver does not hold, `titled-by-service` (409) for a room a chat service carries, whose title is its service's |
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
| `GET /providers` | | a page of `Provider` (`key`, `label`, `can`, `listsRooms`, `listsContacts`, `startsChats`, `startsGroups`, `roomModes`, `fileBytes`): every chat provider a room may be on |
| `GET /providers/{key}/rooms` | `after` (a room number), `limit` | a page of `AccountRoom` (`id`, `title`, `kind`, `privateTo`, `held`, `mode`): the rooms the provider's account is in |
| `GET /providers/{key}/contacts` | `after` (a contact number), `limit`, `accountId` (the account whose contacts are listed) | a page of `AccountContact` (`id`, `name`, `kind`, `roomId`, `held`, `mode`): the contacts of the provider's account, or of the account `accountId` names |
| `GET /rooms/{roomId}/providers` | | `providers`, the keys of the room's chat providers in order, `chosen`, whether the list was set, and `fileLimit`, the largest file the room takes |
| `PUT /rooms/{roomId}/providers` | `providers`, the whole list | `providers`, `chosen` and `fileLimit`, as the list now stands |
| `GET /rooms/{roomId}/members` | | `members`, the people in the room as its own chat service lists them, each `person` (the sender number the room's messages carry, or null where the service gave the person none), `name` and `statusMessage` (or null) |
| `GET /rooms/{roomId}/audience` | `agent`, a supervisor's name | `room`, `agent` and `audience`: `internal` where everyone in the room stands in the company of the supervisor's owner, `external` otherwise (`docs/receiver.md`, 「Whether someone outside the company sits in a room」); `bad-request` (400) for a blank `agent`, `not-found` (404) for a room the receiver does not hold |
| `GET /rooms/{roomId}/accounts` | | `accounts`, the room's members by account, each `account`, `addedBy` and `addedAt` (both null for a private room's own account, which nobody let in) |
| `PUT /rooms/{roomId}/accounts/{account}` | `by`, the account letting it in | the member, `account`, `addedBy` and `addedAt`; the same member where it already is one |
| `POST /rooms/{roomId}/accounts` | `accounts`, the keys of the accounts to let in, at least one; `by`, the account letting them in | `accounts`, each account asked as a member as it now stands, and `letIn`, the keys of those this call let in |
| `POST /rooms/{roomId}/unit-accounts` | `unit`, an organisation unit's number in the web unit; `by`, the account letting its members in | `unit`, `unitName`, `accounts`, each of the unit's members as a member of the room as it now stands, and `letIn`, the keys of those this call let in |
| `DELETE /rooms/{roomId}/accounts/{account}` | `by`, the account taking it out, the member itself where it leaves | 204 |
| `GET /rooms/{roomId}/accounts/{account}/grants` | `by`, the account asking | `account`, its rights `letIn` and `takeOut` over the room's people (each false where none is granted), and `mayWrite`, whether `by` governs the room and so writes such rights; `not-found` (404) for a room the receiver does not hold, `bad-request` (400) for a key that is none |
| `PUT /rooms/{roomId}/accounts/{account}/grants` | `by`, the account granting; `right`, `let-in` or `take-out` | the member's rights as they now stand, `letIn` and `takeOut`; the same rights where it holds the right already; `not-room-leader` (403) where `by` governs the room no way named, `not-found` (404), `bad-request` (400) for no `right` or another one |
| `DELETE /rooms/{roomId}/accounts/{account}/grants` | `by`, the account taking back; `right`, `let-in` or `take-out` | 204; `not-room-leader` (403) where `by` governs the room no way named, `not-found` (404), `bad-request` (400) for no `right` or another one |
| `PUT /accounts/{account}/identity` | `name?`, the name the identity goes by from then | `account` and `sender`, the account's record identity, made where it does not stand |
| `GET /accounts/{account}/rooms` | | `rooms`, the numbers of the rooms the account is a member of in the console, its private rooms among them |
| `GET /accounts/{account}/removal` | `Owner-Key` and `Owner-Role` (the administrator) | `counts`, what the account's removal would take now, by kind; nothing written |
| `DELETE /accounts/{account}` | `Owner-Key` and `Owner-Role` (the administrator) | `removed`, what the removal took, by kind |
| `GET /org-units/{unit}/removal` | `company`; `Owner-Key` and `Owner-Role` (the administrator) | `counts`, empty: the receiver keys nothing by an organisation unit |
| `DELETE /org-units/{unit}` | `company`; `Owner-Key` and `Owner-Role` (the administrator) | `removed`, empty |
| `GET /rooms/{roomId}/figures` | `zone`, an IANA time zone name, `UTC` where none | the room's figures: `room`, `messages`, `fromPeople`, `speakers`, `firstAt`, `lastAt`, `lastAnswerAt`, `top`, `byHour`, `agents` |
| `PUT /pinned-rooms` | `roomIds`, the whole order; `account?` | `roomIds`, the order as it now stands |
| `POST /pinned-rooms` | `room`, one room stood last; `account?` | `roomIds` |
| `DELETE /pinned-rooms/{roomId}` | `account?` | `roomIds` |
| `PUT /room-holders/{holderName}` | `address`, `holdings` (each `name` and `effect`: `kept` or `warns`) | 204 |
| `DELETE /rooms/{roomId}` | `by` (the administrator), `dryRun` | what the removal would do, or what it took |

- **A `Room`** is `id`, `title` (or null), `messageCount`, `lastMessageAt` (or null), `pinned`, `senders`, the
  people that have stood as a sender, `supervisors`, the names of the supervisors the room names to answer it, in
  name order, empty where it names none, `ownerRoom`, `privateOf`, `registered`, `listened`, whether the receiver
  listens to the room: it stands on the console's room list, being the owner's private room, a room added in a mode,
  a room naming supervisors, a pinned room, or a room on the listen list (`allowed_chats`, `docs/store.md`), one the
  owner allowed with his allow command and has not denied since. Every room's messages are kept whether it is
  listened to or not; a room that is not wakes no agent and starts no session (`docs/wake.md`, 「Who reaches the
  session」), and a chat service asking about it is answered that it is not listened to; `workRoom`, where the news,
  questions and answers of work filed from it are told, and `mode`, the mode it was added in:
  - `record-only`: the user alone. The room is kept and its messages carried to its providers as any room's, and
    no agent is woken in it.
  - `avatar`: the room's designated agents answer it as any room's do, and where it names supervisors they are
    the only agents its words can wake there (`docs/wake.md`, 「Who reaches the session」); every line the
    receiver sends there that names no voice of its own goes in the provider's `human-avatar` voice, the account
    of the person the provider reads.
  - null for a room never added: it stands as the roads that handed it messages make it, and every line the
    receiver sends there that names no voice of its own goes in the provider's `human-avatar` voice where
    that is the one mode the provider declares active, and names none otherwise (`docs/receiver.md`, 「A room's
    chat providers」).
- **`privateOf`** is the key of the account whose private room the room is, or null for every other room: a BMO's
  own room on the record is its supervisor's owner account's, and the owner's private room the owner's account's,
  the two private rooms `docs/receiver.md`, 「Console rooms and accounts」, states, where it also states who reads
  one and how the record keeps it. **`sealed`** is true for a private room whose account is not known yet, the
  owner's private room before the web unit links an account to his sender, and false for every other room; its
  `privateOf` is null. A room is written in for account A only where it is not `sealed` and its `privateOf` is null
  or A. The receiver answers both from the room's record, never from a room's members or providers, so a group room
  every other member has left reads null and not sealed. No read asks the agency for them, and the web unit is
  asked only for the owner's private room until its account is kept, the room read as sealed where it does not
  answer.
- **Every write in a room names the account it acts for** where the console acts for a person: `account`, a number,
  in the body or the query; `by` where a route takes one; or the relay's `Owner-Key`. Wherever it is named it is one
  account, and two that differ are refused 400 `bad-request`. Every route that changes a room, a message in it or
  what is kept of it is judged so (`docs/receiver.md`, 「Console rooms and accounts」): the acts on a message, a
  sticker, a forward (its own room and `to` both), a line's reading, the pinned order, a room's picture, its
  registration, archive, providers, supervisors, notices, read marks and file fills, its removal, its members by
  account, an account's words and files (`account` there, required), and the owner's routes, which act for the
  owner's account. In a room whose `privateOf` is set, a write for any other account, the administrator's among
  them, is refused 403 `private-room` (`room`, `account`) before anything is written; the room's own account's go
  through. A sealed private room, `privateOf` null though it is one (the owner's private room while no account is linked
  to his sender), refuses every account's write. The room's removal by the administrator is the one write no
  private room refuses (below). The whole pinned order is a write in each room it pins or unpins. A write naming no account is a unit
  acting as itself and is refused nowhere.
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
  A list that is empty, names a key no provider stands under or names one twice is refused 400 `bad-request`,
  a room the receiver does not hold 404 `not-found`, and an added room 409 `providers-fixed`, its list being the one
  its addition set: the chat service it was chosen on and the record. Every message kept in the room goes to each of its providers, and a
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
  - A `Provider`'s `can` is every chat verb it declares it performs, by bmo-chat's names for them (its
    `docs/contract.md`, 「Capabilities」), the record's own verbs for the record and none for a chat service that
    does not answer its declaration. It is the one read of what a provider performs, whatever room it is asked
    for, so a screen tells two providers of one room apart by it.
  - An `AccountRoom` is the room as the service answers it, `id`, `title` (or null), `kind` (`owner` or
    `group`) and `privateTo` (or null), with `held`, whether the receiver already holds the room (its own, one it
    keeps a title or a message of, or one added), and `mode`, the mode the receiver added it in, or null. A room
    held and never added is `held` with a null `mode`. The page and its `more` are the service's; `after` and
    `limit` are handed to it as they came.
  - An `AccountContact` is the contact as the service answers it, `id`, `name`, `kind` (`person` or `bot`) and
    `roomId`, the one-to-one the service's account already has with it (or null), with `held` and `mode` of that
    room as an `AccountRoom` carries them, false and null where it has none. The page is the service's, as above,
    and `accountId` is handed to it as it came (bmo-chat's `docs/contract.md`, 「The account a call is for」).
  - `POST /rooms` with `roomId` keeps the room under the service's number: its mode, its title (the one given,
    else the one the service gives, else none) and its list of providers set to that service and the record, so
    every mode's messages stand in the archive. With `contactId` or with `contactIds` and `title` it first asks
    the service to start the conversation, in the account `accountId` names where it names one, and keeps the
    room it comes to the same way; a one-to-one the account
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
- **A room's members are asked of the room's own road** (`docs/receiver.md`, 「A room's chat providers」), its first
  provider where its list was chosen, through bmo-chat's `GET /rooms/{roomId}/members`, and answered as that service
  lists them; nothing is written and the room is not changed. A person is named by the number the room's road gives
  that person, the number the room's messages carry as their sender, never by a platform's own identifier, so a
  screen links an account and a kept picture by that number; a sender the road numbered otherwise before is
  matched to the member of its nickname as the service starts (`docs/messages.md`, 「A sender matched to the room's
  members」). A room the receiver holds nothing of is refused 404 `not-found`; a room on the record alone 409
  `record-only-room`; a room whose service does not declare `memberList`, or serves no members route at all, 409 `members-unlisted`; a room whose
  service holds no members of it, or refuses, 409 `members-unheld`, its sentence carrying the service's own;
  and a service that does not answer 502 `upstream-failed`.
- **A body naming no `provider`, a `title` and the `supervisors` that answer it** makes an agent room: the
  receiver numbers it in the record's own range, sets its one provider to the record, writes its title and its
  answering agents, and answers the room as it stands. No chat service stands behind it, and its members wake
  by a call by name or a reply to their words as those of any room do. `openedBy` names the account that opened
  it, which stands as its first member, let in by itself, with no line written.
- **A room's members by account** are the console's (`docs/receiver.md`, 「Console rooms and accounts」), beside
  the members a chat service lists (`GET /rooms/{roomId}/members`), whom the console links to accounts itself. An
  account is a key of the web unit's, as decimal text in a path and a number in a body. `PUT` lets an account in,
  writing a line into the room naming who let whom in (the fixed line `account-let-in`, its fields `account` and
  `by`); `DELETE` takes one out, writing `account-left` (its field `account`) where `by` is the member itself and
  `account-taken-out` (`account` and `by`) otherwise; a member let in again, or one not a member taken out,
  writes nothing and answers as it stands. The receiver judges no right of `by`: the console asks the web unit who
  may before it calls. A room the receiver does not hold is refused 404 `not-found`, a `by` missing 400
  `bad-request`, and a change in a private room 403 `private-room`.
- **Several accounts are let in by one call,** `POST /rooms/{roomId}/accounts`, each as `PUT` lets one in, and the
  room hears one line for all of them: `accounts-let-in` (its fields `accounts`, the keys this call let in, and
  `by`), or `account-let-in` where it let in only one; an account already a member is left as it stands and
  named in no line, and a call that let nobody in writes nothing. A list naming no account, or an account twice,
  is refused 400 `bad-request`.
- **An organisation unit's members are let in by one call,** `POST /rooms/{roomId}/unit-accounts`: the receiver
  asks the web unit for the unit (`GET /units/{unit}`) and lets in, as above, the users whose home unit it is and
  those holding a post in it, as they stand at that moment, and the room hears one line naming the unit:
  `unit-let-in` (its fields `unit`, `unitName`, `accounts`, the keys this call let in, and `by`). An account the
  web unit marks as having left (`GET /users/{user}/node`, its `left`) still stands in its unit there, and is
  left out: it is neither let in nor named in `accounts`, `letIn` or the line, and one that is already a member
  stays as it stands. The room keeps no link to the unit: a person who joins it later is not let in by it, and
  one who leaves it stays a member. A unit the web unit does not hold is refused 404 `not-found`, and a web unit
  that does not answer, for the unit or for whether one of its people has left, 502 `upstream-failed`, with
  nobody let in. `GET /rooms/{roomId}/accounts` of a
  private room answers its own account alone. `GET /accounts/{account}/rooms` answers the rooms the account is a
  member of now with its private rooms: those of the supervisors it owns, and the owner's private room for the
  owner's account; the rooms it reaches through a chat service's identities are the console's to compose.
  `PUT /accounts/{account}/identity` answers the same sender every time it is asked for one account. A `name` in
  its body is the name the identity goes by from then, the same field an account's words carry, and each of the
  account's words, files or forwards that carries a `name` names it again: every reading of the person's name
  (`GET /people`, `GET /rooms/{roomId}/people`, a forward that names none) answers the latest, so a name the console
  changes reaches the record with no words sent. A missing body, or one with no `name`, names nothing; a `name`
  that is blank is refused 400 `bad-request`.
- **A room member's rights over its people** stand beside its membership, kept by the receiver and read by the web
  unit's entry check. `GET /rooms/{roomId}/accounts/{account}/grants` answers the member's `letIn` and `takeOut` (each
  false where none is granted) and `mayWrite`, whether the asking `by` governs the room; `PUT` grants and `DELETE`
  takes back the one `right` named, `let-in` or `take-out`, the `PUT` answering the rights as they now stand and the
  `DELETE` answering 204. Who may write is the same standing that locks a room: its administrator, its leader or a
  head of a leader's unit (`docs/receiver.md`, 「Console rooms and accounts」 and 「An archived room」). Anyone else is
  refused 403 `not-room-leader`; the room is no one's private room, the guard every write in a room meets among them.
  These roads stand outside the published client, the console asking them on the receiver's address.
- **A room's removal** asks the record what it would take, each declared holder and the agency, and refuses
  anyone but the administrator (`by` names who removes it, as the web unit knows him). The administrator removes any
  room, another account's private room and a sealed one among them; any other caller is refused 403
  `not-administrator`, and in another account's private room or a sealed one 403 `private-room` first, as every
  write is. The refusals of `docs/codes.md` (`RoomRemovalRefusal`) stand for the administrator too. `dryRun=true` answers the preview and writes nothing; the answer names
  `goes`, `stays`, `pictures`, `settings`, `folders`, `warned` (each a `RoomRemovalWarning` of `docs/codes.md`, its
  `code` beside its fields), `warns` (each declared holder's answer), `units` and, for the removal itself, `taken`,
  `cleared`, `holders` and `couldNot` (each a `RemovalLeft` of `docs/codes.md`). No field of either answer carries a
  sentence a screen shows; a holder's or a unit's `failure` is the English sentence for the log.
- **An account's removal** is the administrator's alone (`docs/receiver.md`, 「An account's removal」): each of the
  four routes is answered only under the console's token, with `Owner-Role` `administrator` and an `Owner-Key` the
  web unit answers as the administrator (`GET /users/{user}`); any other caller is refused 403 `forbidden`, and a
  web unit that does not answer 502 `upstream-failed`. `{account}`, `{unit}` and `company` are positive decimal
  numbers, anything else refused 400 `bad-request`. `GET /accounts/{account}/removal` answers
  `{"counts": {kind: n}}` and writes nothing; `DELETE /accounts/{account}` answers `{"removed": {kind: n}}`. Both
  name every kind, 0 where there is none, and the count is what the removal would take at that moment:
  - `private-rooms`: the rooms the record keeps as the account's private rooms, each removed as the administrator's
    room removal removes it;
  - `memberships`: the account's rows as a member of every other room, those rooms standing;
  - `identities`: the account's record identity;
  - `names`: the name its record identity goes by;
  - `mutes`: its record identity muted in a room;
  - `pictures`: the pictures set on its record identity, each with its file.

  A second call takes nothing and answers every kind 0. A private room whose removal is refused stops the call 409
  with the refusal's own code, an agency that does not answer for one 502 `upstream-failed`, and a file or folder of
  one that could not be removed 500 `failed`, each sentence naming the room; a picture's file that could not be
  taken down once the identity's rows went answers 500 `failed` naming the file. What was taken before stays taken,
  and calling again goes on from there. Nothing of a room or a row that names no account, or names another, is touched.
  The receiver keys nothing by an organisation unit, so `GET /org-units/{unit}/removal` answers `{"counts": {}}`
  and `DELETE /org-units/{unit}` `{"removed": {}}`.

## Messages

| Route | Body or query | Answers |
|---|---|---|
| `GET /rooms/{roomId}/messages` | `before`, `after`, `limit`, `agentName`, `since`, `marked`, `unread`, `by`, `q`, `holding` | a page of `Message` |
| `GET /supervisors/{supervisorName}/rooms/{roomId}/messages` | `before`, `after`, `limit` | a page of `Message` |
| `GET /rooms/{roomId}/messages/{messageId}` | | one `Message` |
| `GET /rooms/{roomId}/pins` | | `items`, the room's pinned messages, newest pin first, each a `Message` |
| `PUT /rooms/{roomId}/messages/{messageId}/reactions/mine` | `emoji`, `account?` | the `Message` as it now stands, `notCarried` where a chat service did not take the act (`docs/messages.md`, 「What an agent can send back」) |
| `DELETE /rooms/{roomId}/messages/{messageId}/reactions/mine` | `account?` | the `Message` as it now stands, `notCarried` as above |
| `PUT /rooms/{roomId}/messages/{messageId}/pin` | `on`, `account?` | the `Message` as it now stands, `notCarried` as above |
| `PATCH /rooms/{roomId}/messages/{messageId}` | `text`, `account?` | the `Message` as it now stands, `notCarried` as above |
| `PUT /rooms/{roomId}/messages/{messageId}/speaker` | `agentName` | the `Message` as it now stands, filed as that agent's line on the record alone (`docs/messages.md`, 「The archive」); `message-not-agents` where it stands under another sender than the agent's own number in the room |
| `DELETE /rooms/{roomId}/messages/{messageId}` | `account?` | the `Message` as it now stands, `notCarried` as above |
| `GET /stickers` | | `items`, each `emoji`: the stickers the record keeps |
| `POST /rooms/{roomId}/stickers` | `emoji`, `account?` | 201, the `Message` the sticker was kept as |
| `GET /rooms/{roomId}/messages/{messageId}/metadata` | | `metadata`, what the chat service handed over beside it, or null |
| `GET /rooms/{roomId}/messages/{messageId}/sender-is-owner` | | `owner` |
| `GET /rooms/{roomId}/messages/{messageId}/files` | | a page of `FileEntry` |
| `GET /rooms/{roomId}/messages/{messageId}/files/{place}` | `download` | the file's bytes |
| `GET /rooms/{roomId}/messages/{messageId}/picture` | | the picture's bytes |
| `POST /rooms/{roomId}/file-fills` | `FileFill`: `rows`, `dryRun` | `FileFills` |
| `GET /rooms/{roomId}/day-start` | `from` (epoch milliseconds) | `messageId`, the first message a person said at or after it, or null |
| `GET /rooms/{roomId}/pictures` | `at`, `before`, `after`, `limit` | a page of `Picture`, `before` and `total` |
| `GET /avatars` | `after`, `limit` | a page of `Avatar`: the profile pictures kept, oldest taken first |
| `GET /rooms/{roomId}/avatar` | | the room's kept profile picture, as its bytes |
| `GET /people/{personId}/avatar` | | the person's kept profile picture, as its bytes |
| `PUT /avatars/{kind}/{subject}` | a form with one `picture`; `kind` is `room`, `person` or `agent` | `items`, each `kind`, `id` and `takenAt`: the picture set; for an agent, kept under its name and worn under every number it stands under now, none where it stands under no number yet |
| `DELETE /avatars/{kind}/{subject}` | | `items`, each `kind` and `id` whose console-set picture went; for an agent, every number that wore its picture, its own kept picture going with them |

- **`q` searches the room:** the page holds only the messages whose words carry the text, case set aside and
  read as given (no pattern), newest page first and walked by `before`, each in the one message shape; `by`
  narrows it as it narrows any page, and a deleted message's cleared words carry nothing.
- **`holding` lists what the room's messages hold:** the page holds only the messages holding that kind, one of
  five (`docs/room-files.md`, 「A room's files by kind」):
  - `media`, a picture the message stored that is no sticker, a kept file of kind `video` or `animation`, or a
    kept `document` whose name implies a video's type;
  - `file`, a kept `document` whose name implies neither a video's type nor a sound's, a name with no ending among
    them;
  - `voice`, a kept file of kind `audio` on a voice note: a message whose hearing is marked on it (`voiceState`
    not null), whose words, leading whitespace set aside, open with the receiver's own voice mark, or whose sound
    came with no name of its own and stands under the name the receiver gives such a sound;
  - `music`, a kept file of kind `audio`, or a kept `document` whose name implies a sound's type, on a message that
    is no voice note by the same test;
  - `link`, words that carry `http://` or `https://`, case set aside. The receiver filters on the words alone and
    leaves reading the addresses out of them to the reader.

  The type a name implies is the one a message's files are served with, its first guess by the name's ending. A
  message holding several kinds stands under each. The page is walked by `before` and `limit` as the search is,
  newest page first and each page oldest first, with `more` where older ones stand; `by` and the leaving out of
  a client's opening apply as on any page. A `holding` that is none of the five, or one asked together with `q`,
  `after`, `unread` or `marked`, is refused 400 `bad-request`.
- **A supervisor reads a group room's messages through its own route,** `GET
  /supervisors/{supervisorName}/rooms/{roomId}/messages`, newest page first and walked by `before` or `after`.
  The room's answerers read it, and so does a member's own supervisor: one whose `ownerAccount` is a member of
  the room by account, or one where the account model links that account to a sender that has stood in the room,
  its record identity among them; a member's read writes nothing, so it
  opens no window, wakes nobody and makes no answerer of the reader. A room that is the owner's private chat or
  a one-to-one is refused 403 `not-a-group`, and a supervisor that is neither its answerer nor a member's is
  refused 403 `not-a-member`; an unregistered name is 404 `unknown-supervisor`.
- **A `Message`** is `id`, `room`, `senderId` (or null), `senderName`, `sentAt`, `text`, `imagePath` (or null),
  `replyToMessageId` (or null), `answerSeconds`, `round`, `album`, `via`, `agentName` (or null), `voice` (or null),
  `transcript` (the words heard in a voice note, or null), `voiceSeconds` (how long the recording ran, or null),
  `voiceState` (`heard`, `failed`, or null), `kind` (`sticker` for a sticker the record sent and for a message a chat service handed a sticker over on, as its picture or as a file of any type (`docs/room-files.md`, 「What the console reads of them」), `thinking` for a thinking block a record holds as a message, `fixed-line` for a fixed line the receiver said into the room itself (`docs/voiced-notices.md`, 「Lines that stay fixed」) and for a line a chat service handed over (「The chat services」), or null for any other), `line` (on a message of kind `fixed-line`, the line it was said from, its `code` and its `fields`: as `docs/codes.md`, 「Fixed lines」, names them on the receiver's own, and as the chat service handed them over on a provider's, a code the receiver does not know among them, and null on any other), `editedAt`, `deletedAt` and
  `pinnedAt` (epoch milliseconds, or null), `reactions`, each `emoji` with `by`, who put it there (an agent's name
  or a person's number), `forwardedFrom` (`room`, `messageId` and `senderName` of the message it copies, or null),
  `situation` (the room's situation note as it stood when the round that judged this message opened — its `text`,
  and `changed`, which is false since no round writes the note, the owner's private chat's messages included — or
  null on a message no round judged under a note; `docs/instruction.md`, 「The situation note」), and `files`, each a `KeptFile` (`kind`, `source`,
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
  where the form holds no `picture`, and 400 `not-a-picture` where it cannot be read as one. A picture of any size
  is taken: it is kept by the receiver alone and handed to no chat service, so no provider's limit bears on it. An agent's picture is kept under its name whether or not it stands under any
  number yet, and every number it takes later wears it (`docs/pictures.md`, 「Profile pictures」).
- **These picture routes are the console's one source of profile pictures,** every chat service's among them: a
  room's, a person's and the owner's own account's. The receiver takes each chat service's pictures from that
  service's avatar routes and serves them here under the number the service gave the subject, so the console
  never asks a chat service for one and a new chat service adds no route the console reads (`docs/pictures.md`,
  「Profile pictures」). A message's face and a member row ask under the same number, the one a message's sender
  carries: a sender a road numbered otherwise before is matched to the room's member of its nickname, so the
  message moves to the number the picture is kept under and the picture stays where it is (`docs/messages.md`,
  「A sender matched to the room's members」).
- **`agentName` and `voice`** stand on a send the receiver archived itself, naming the agent whose words these are
  and the voice the send went out in (`bot-account`, `human-avatar`, `human-reader`), and on a line a provider
  handed over under the room's own number, which is the default answerer's in the voice the provider handed it
  over in (`docs/messages.md`, 「The archive」). Both are null on any other message that came in from a provider;
  the provider's echo of the receiver's own send keeps them, since that message is the one already kept with its
  fields. So an answer sent in the owner's `human-avatar` account is read as the
  agent's with its voice named, not guessed from the number it went out under.
- **The message page** holds whose messages `by` names, told apart by the record alone: `by=agent` holds the
  messages whose `agentName` names an agent, whatever name they stand under, and `by=person` those whose
  `agentName` is null. With no `by`, it holds both. The opening a client sends for a person (`/start` in a
  one-to-one room) stands on no page but the one `by=any` asks, which holds every message. It is walked by
  `before` or `after`, a message the reader holds, and ends on the side it walked from. With `agentName`,
  `unread=true` answers the messages past that agent's read marks, oldest first, from `since` where given; with
  `marked=true` it answers the messages the agent has marked, after `after`. This is the one message read: there is
  no second route for the same message.
- **The bytes** answer as a file does over HTTP: `Content-Type`, `Content-Length`, `Accept-Ranges`, and `Range`
  answered 206 or 416. `download=true` adds `Content-Disposition: attachment`. Only bytes inside the room's own
  folder are served; anything else is 404 `not-found`.
- **A file fill** asks the room's chat service again for the files of placeholder rows that kept none
  (`docs/room-files.md`, 「Files filled in later」). `FileFill` is `rows` (how many rows the pass may ask, or
  null for the policy's default; more than the policy's ceiling is read as the ceiling) and `dryRun` (list the
  rows, ask nothing). `FileFills` is `room`, `rows`, each a `FilledRow` the pass reached, newest first, and
  `unreached`, the sentence of a road that could not be reached and ended the pass there, or null. A
  `FilledRow` is `message`, `sentAt`, `outcome` (`filled`, `none`, `unavailable`, `refused`, or null on a dry
  run), `reason` (the road's own sentence, or null) and `files`, each `kind` (`image` for a picture, else the
  kind its `collected_items` row names) and `bytes` (what the store holds of it). A room whose road declares no
  `fetchMedia` is refused 409 `room-fetches-no-files`, and one where a pass already runs 409 `room-files-filling`.
- **The metadata is opaque:** it is kept and handed back unread, so what it holds is the chat service's to state;
  only its strings are read, each for secret-shaped runs as a message's words are (`docs/messages.md`, 「Secrets
  taken out」).
- **A `Picture`** is `id`, `album`, `senderName`, `sentAt`, `text` and `file` (`name`, `bytes`, `type`), null where
  the stored picture is gone. `at` answers that picture alone, `before` the stretch ending before it, `after` the
  stretch past it, and none the newest.
- **A sticker is no picture of the room's:** a message of kind `sticker` stands in no page, in no `before` and in
  no `total`, though it stores a picture (`docs/pictures.md`, 「The console's picture viewer」), and `at` on one
  answers an empty page.

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
| `GET /people` | `ids`, comma-separated | a page of `Person` (`id`, `name` or null): each with the name they last went by: an account's record identity by the name it was last given (`PUT /accounts/{account}/identity`, or its words, files or forwards), anyone else by the name their latest words carried |

## Notices

| Route | Body or query | Answers |
|---|---|---|
| `GET /rooms/{roomId}/notices` | `before`, `after`, `limit`, `kind`, `from`, `to` | a page of `Notice` |
| `POST /rooms/{roomId}/notices` | `fact`, `intent`, `followUp?`, `optional`, `kind?`, `events?`, `ridesUntil?` | 201, `{id}`: the instruction it was queued as |

- **A `Notice`** is `id`, `kind`, `summary`, `text` and `createdAt`. The list is walked by `before` or
  `after`, an instruction id, and narrowed by `kind` and by the span it was queued in, both ends included.

| Route | Body | Answers |
|---|---|---|
| `GET /notice-switches` | | `restart` and `rest`, the owner's private chat's two notice switches as they stand |
| `PUT /notice-switches/{kind}` | `on`, a boolean | `restart` and `rest`, as they now stand |

- **The notice switches** are the owner's private chat's (`docs/voiced-notices.md`, 「Operating notices per
  chat」): `restart`, the going-down line and the back notice, and `rest`, the line a rest begins with and the
  notice of its lifting. Each is on unless it was set off. A kind that is neither is refused 404 `not-found`,
  and a body whose `on` is not a boolean 400 `bad-request`.

## Instructions

| Route | Body or query | Answers |
|---|---|---|
| `POST /instructions` | `agentName?`, `room?`, `said` (`fact` or `order`), `sort`, `summary?`, `kind?`, `cause?` (`reply` or `housekeeping`), `events?`, `agentOnly?`, `ridesUntil?`, `ridesForGood?`, `via?`, `run?`, `publisher?`, `line?` | 201, `{id}` |
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
  delegate's or a worker's answer, a job's ending and report, the end or loss of background work an agent's
  program launched in the room (`docs/instruction.md`, 「Long work in a round」), a reminder or a watch come due —
  and opens the agent's turn as soon as it is queued, on a session past its cache lifetime too, which that turn
  replaces.
  `housekeeping` is a record for the agent alone that nobody waits on, and waits for a round something else opens
  where the session is past its cache lifetime; one marked `agentOnly` is housekeeping whatever it names.
- **A quiet instruction** is one with `sort` `watch` or `reminder` and a `said.order`: it is queued under its
  `kind` and opens the agent's turn there.
- **A rider** waits for the next round its room opens for something else and goes in with it, never opening a turn
  of its own while it rides (`docs/bmo-window.md`, 「Rows that ride」): `ridesUntil` names the moment after which it
  opens one itself, and `ridesForGood` (`true`) has it ride its room's rounds with no such moment.
- **Its `line`** is a fixed line (`docs/codes.md`, 「Fixed lines」), its `code` beside its fields, that the room the
  row names hears at once, said by the receiver as its own and kept as a `fixed-line` message
  (`docs/voiced-notices.md`, 「Lines that stay fixed」): for a fact a person must not wait for a round to hear, such
  as a run of theirs finding no supply left while no turn can go out in the room. It is said once, as the row is
  written: an instruction repeated says nothing again, and a row for a room whose registration is gone says no line.
  Only a line a unit composes is taken, `supply-closed` alone; any other, and one that does not read as its code's
  fields, is refused 400 `bad-request`.
- **An instruction repeated is not queued again** (`docs/bmo-window.md`, 「Quiet instruction」): one whose agent,
  room, publisher, kind, sort, events, run and words are those of a row still waiting, or of a row let go as
  failed within the instruction policy's `repeat_minutes`, writes nothing and is answered 201 with that row's id.
- **A withdrawn instruction** that was handed or is gone is no refusal; it is simply no longer pending.

## Sends

| Route | Body or query | Answers |
|---|---|---|
| `POST /sends` | `asker` (`room?`, `via?`, `agentName?`, `began?`, `by?`, `job`, `from?`), `tool`, `input`; the `say`, `photo`, `file`, `voice`, `sticker` and `talk` acts take an optional `replyTo`, the message in the room the send replies to | `lines`, what the act said, and `refusal` where it ended in one; `handed`, the instruction's number, where it was handed to the room's supervisor |
| `GET /rooms/{roomId}/send-verbs` | `via` | a page of the verbs of the road a send naming `via` goes out on in the room |
| `POST /owner/messages` | `chatId`, `text`, `replyTo?` | `id`, the message kept or sent |
| `POST /rooms/{roomId}/words` | `account`, `name` (the account's shown name), `text`, `replyTo?` | 201, `id`, the message kept |
| `POST /rooms/{roomId}/word-files` | a form as `POST /owner/message-files` takes but for `chatId`, which the path gives, with `account` and `name` | the answer `POST /owner/message-files` gives |
| `POST /owner/message-files` | a form: `chatId`, `text?` (the caption), `replyTo?`, up to ten `file` fields, and `voice?` (`true` to send the one file as a voice note) | `id`, `sent` (each `name`, `id`), `checking`, `failed` (each `name` and `reason`, a `FileUnsent` of `docs/codes.md`) |
| `POST /rooms/{roomId}/messages/{messageId}/forwards` | `to`, the room it is forwarded into; `account?`, the account forwarding it, and `name?`, its shown name | 201, `room` and `id`, the copy kept there, its stored picture kept again under it |

- **A route that takes files sets no ceiling of its own** on a file's size, on how many files a form carries or
  on the body. `POST /owner/message-files` and `POST /rooms/{roomId}/word-files` read their form as it arrives,
  each file written to the disk as its bytes come rather than held whole in memory (`docs/store.md`, 「The data
  root's folders」); `PUT /avatars/{kind}/{subject}` reads its one picture whole, since the picture is drawn anew.
  The one limit is the room's: **the largest file a room takes is the smallest `fileBytes` any chat service on its
  list of providers declares** (bmo-chat's `docs/contract.md`, the dialect's `limits`), and a room none of whose
  chat services declares one, a room on the record alone among them, takes a file of any size. A file past the
  room's limit is turned away by its name, and refused 413 `too-large` where no file of the form stood, its
  sentence naming the file, its size, the largest the room takes and the provider that declares it
  (`docs/codes.md`, `OwnerSendRefusal`). Every other route takes a body of words and reads at most two mebibytes of it; the chat services'
  hand-overs the base library serves are bound by no ceiling of the receiver's.
- **The console reads the limit before it sends:** each `Provider` on `GET /providers` carries the `fileBytes` its
  declaration gives, null where it declares none, and `GET /rooms/{roomId}/providers` answers the room's
  `fileLimit`, `{provider, bytes}`, the chat service whose limit is the smallest and that limit, or null where the
  room takes a file of any size.
- **`send-verbs`** answers what a send could do in the room, never what one provider declares: the road it reads is
  the one `POST /sends` chooses for the same `via`, the road `via` names where the room answers on it and the
  room's own road otherwise, and the road a send naming none goes out on where `via` is absent
  (`docs/receiver.md`, 「A room's chat providers」). So every `via` the room does not answer on, a provider it is
  not on among them, answers the room's own road's verbs. A room no road answers is refused 409 `conflict`. One
  provider's own verbs are its `can` on `GET /providers`.
- **`tool` and `input`:** `say`, `photo`, `file`, `voice`, `speak`, `react`, `pin`, `sticker` and `random_draw`,
  each the arguments its tool takes (`docs/chat-mcp.md`), and `talk`, a spoken talk asked by its `voice`, its
  written `text` and its `spoken?` words. Any other tool is refused 400 `bad-request`. An act that holds no words (a
  picture or a file with no caption, a reaction, a pin, a sticker, a draw) goes out as the asker's agent, on the
  road the turn names, only where that agent's round is working in the room `by` names and no job asks. An act
  that holds words (`say`, `talk`, `voice`, a `photo` or `file` with a `caption`) never goes out: asked in that
  round into the room `by` names, it is refused, its `refusal` code `words-in-answer` (409), and anywhere else it
  is handed. `speak` marks that round's answer to go out as a voice note and is refused anywhere else
  (`docs/voiced-notices.md`, 「An answer said aloud」). Every send not carried or refused is handed to the room's supervisor as a quiet instruction and
  nothing goes out (`docs/messages.md`, 「Words handed to the room's supervisor」). `from` names the command or
  unit the words came through, which the handed words carry.
- **`began`** is the moment the asking turn took its words, in epoch milliseconds. A send whose
  `began` is at or before its agent's reset point, the agency's as the receiver reads it then, is refused before
  anything else, its `refusal` (code `overtaken`, 409) telling the turn to end (`docs/wake.md`,
  「An agent's reset」).
- **A voice briefing turn sends nothing.** While the receiver carries a notice turn's summary or a line's full
  reading for an agent in a room, a send that agent asks into that room, of any act and from no job, is refused
  `briefing-turn`, its `refusal` saying that the turn's answer is the script itself and nothing is sent into the
  room from it (`docs/voiced-notices.md`, 「The summary of a notice turn」).
- **A `say`** is handed, or refused in its own round's room. With `reply`, asked from a round of another room,
  the room it is handed to is armed to relay its answer back once the round carrying it has sent its message
  (`docs/messages.md`, 「Words handed to the room's supervisor」).
- **A photo send** is one picture or several as one album: each `url` is a web address or a file under the data
  root, with an optional `label` drawn into it, and an optional `caption`. A web address goes as it stands for the
  road to fetch; a file goes from the disk it stands on, read only as the send carries it, and one with a label
  drawn into it goes as the drawn bytes. An address that is neither is refused before anything
  goes, and every picture that did not go is named with why. A picture kept in the room is filed under its
  message (`docs/pictures.md`, 「A picture an agent sent」).
- **A file send** is one document, or several files as one album: each file a `path` on this machine and an
  optional `name`, with an optional `caption`. The files go to the verb the room's road declares, `document` for
  one file and `album` for several, each from the disk it stands on and read only as the send carries it
  (bmo-chat's `docs/protocol.md`, 「Files carried as parts」), never whole in the receiver's memory; a file a room's
  folder holds goes only into that room, and a job sends none.
  A file past the largest the room takes (「A route that takes files」 above) is refused before anything goes, as
  is a voice note's. Each file that went is filed under its message (`docs/room-files.md`, 「Message files」).
- **A voice note** is an OGG/Opus file at `path`, with its `caption`, what it `said` and the `transcript` read out,
  sent as the chat protocol's `voice` kind from the disk it stands on; only its first bytes are read here, to tell
  it is OGG.
- An owner's file send is the form above.
- **An owner's voice note** is the same form with `voice` true: one `file` that is a sound. The room keeps it
  as one message under the policy's voice word and the caption beside it, its file the recording, and hands it to
  the room's other providers as the chat protocol's `voice` kind with the caption. A call with no file, more than
  one, or one that is no sound is refused 400 `bad-request`; a file past the largest the room takes is refused 413
  `too-large`.
- **An account's words** (`POST /rooms/{roomId}/words`, `POST /rooms/{roomId}/word-files`) are filed under the
  owner's sender for the owner's account and under the account's record identity for any other, and from there are
  the words the owner's routes below take, kept, woken on and carried as any message, whether they are the owner's
  read off the sender (`docs/receiver.md`, 「Console rooms and accounts」). Their refusals are those of the owner's
  routes, and beside them: words from any account but its own in a private room 403 `private-room`, an `account`
  or `name` missing 400 `bad-request`, and a web unit that cannot say which account is the owner's 502
  `upstream-failed`. `POST /owner/messages`, `POST /owner/message-files` and `POST /owner/instructions` are the
  owner's road, which the console takes for the owner's account alone. Every one of these routes is the console
  relay's alone, under the console's token (「Auth」 above), and refused 403 `forbidden` under the receiver's own.
- **The owner's words** from the console are archived in the room under a console number, the owner's number and
  the owner's own name, handed to the room's conversations as any of the owner's words are, and carried to the
  room's other chat providers as every kept message is. A file past the largest the room takes is refused 413
  `too-large`; a `replyTo` the archive does not hold is refused 409.
- **Words the room kept are answered with their number.** `POST /owner/messages` and `POST /rooms/{roomId}/words`
  answer the `id` the words stand under once the room has kept them, whatever follows: a wake of the agents they
  call for, or their carry to the room's other providers, that then fails is written to the service's log and never
  into the answer, and is not tried again. So a send the console repeats on a failure never puts the words into the
  room twice. A setting command among the owner's words is settled with the keep, and a failure in settling it is
  answered as any failure is.
- **A file the room kept is answered as sent.** `POST /owner/message-files` and `POST /rooms/{roomId}/word-files`
  keep each file as its own message, and a file that stands in the room is answered under `sent` whatever follows
  it: a wake of the agents it calls for, or its carry to the room's other providers, that then fails is written to
  the service's log and never into the answer, and is not tried again. So a send the console repeats on a failure
  never puts a second copy of a file into the room. A file the room could not keep is answered under `failed`, its
  `name` and the `reason`, a `FileUnsent` naming its kind and, for one refused, the refusal's code, beside the files it
  did keep; a form of which the room kept none is refused as the first
  of them failed, and carries nothing.
- **The signed-in person's acts on a message** come through the console relay, which names the caller by
  `Owner-Key`, the login's user id, and `Owner-Role`, as a picture's setting does (`docs/pictures.md`, 「Profile
  pictures」), so a member acts only as themselves. The account an act is for is the `account` the call names, else
  the one `Owner-Key` names, and a call naming an account `Owner-Key` does not is refused 400 `bad-request`; in
  another account's private room every act is refused 403 `private-room`, the administrator's delete among them. The
  caller's senders are those the web unit links to the user, and the caller acts under the one of them the room
  already holds, else the first. The record keeps each act as the agent-side acts are kept (`docs/messages.md`,
  「What an agent can send back」) under that number and tells `message.changed`. Where the caller is the owner, the
  act goes on in his own voice to the room's other providers wherever the message stands under a number of theirs,
  and a provider that refuses the act in his own voice is handed it again in the voice it answers the room by,
  unless it reads the room in the owner's account alone, so a reaction or its removal reaches the room's chat
  service even where the owner's account sends only words and files; a member's act stands on the record alone,
  since no road speaks in a member's account. Where the act was carried, every provider that did not take it is
  named in the answer under `notCarried` and said in the service's log, and the act stands on the record all the
  same: the answer is still the message as it now stands, and the screen reads that row as it does for any act. A
  reaction is the caller's own, one to a reactor; a pin anyone signed in sets or takes off; an edit is the caller's
  on a message one of their senders said, and a delete the same or the administrator's on any; a sticker is a
  message of the caller's own, its kind `sticker`, carried as the owner's words are where the caller is the owner. A
  caller the relay names no user for, or whose user holds no sender, is refused 403 `no-caller`; a message the room
  does not keep 404 `message-unknown`; a deleted one 409 `message-deleted`; another person's message to edit or
  delete 403 `message-forbidden`; an emoji the record's set does not hold 400 `unknown-sticker`; a reaction with no
  emoji or an edit with no words 400 `bad-request`. The record's set is the policy's (`roads.toml`, `[record]`,
  `stickers`), and a road the sticker goes on picks its own sticker for the emoji.
- **A room's pinned messages** are the ones whose pin stands on the record, a deleted message none of them, newest
  pin first.
- **A forward** copies a message into another room: kept there under a console number as the forwarding
  account's, under the sender it speaks as (the owner's for the owner's account, its record identity for any other)
  and the `name` given, else the name that identity last went by, and as the owner's, his number and the name that
  room shows him by, where the call names no account, its words the message's own, marked `forwardedFrom`
  with the room, the message and who said it, and carried to that room's providers as every kept message is, so
  it goes out on the room's own road. It wakes nobody. A message or a room the receiver does not hold is refused
  404 `not-found`, a forward into the room it stands in 400 `bad-request`, a deleted message 409 `conflict`, and a
  forward out of or into another account's private room 403 `private-room`.

## Talks

| Route | Body | Answers |
|---|---|---|
| `POST /talks` | `topic`, `turns` (each `owner`, `text`), `said` | `answer`, or null where the agent said nothing |

- **A topic is the caller's:** `kind` names what it is about, `heading` is the paragraph the turn opens on, and
  `fields` are the values that head carries. The receiver enumerates no kind; `item` and `document` are read as the
  talks they name, and any other kind is read from the heading and fields as given.
- **A talk that comes back with nothing is refused:** `conflict` with `busy` where the talk is being answered,
  `not-ready` where the default answerer's conversations do not stand, `conflict` with `resting` where the rest
  closes the default answerer's road — before any turn goes out, or as the reason a turn that went out failed —
  429 `limit` where its provider's limit is reached, and 503 `outage` where its provider is out. A turn that was
  stopped, or that failed with the road open, comes back `unanswered`.

## Events and the live board

| Route | Body or query | Answers |
|---|---|---|
| `GET /events` | `after`, `run`, `kinds`, `roomId` | a server-sent stream |
| `POST /events` | `kind`, `data` | 204, the event published |
| `GET /live` | | the live board: `at`, `turns`, `compacting`, `queued` |

- **The stream** opens with `event: run`, replays the ring of the last 500 events after the cursor, a reader
  naming no run being handed none and starting at the present, then the picture and each working turn's present
  `run-progress`, then the typing marks that stand and the presence known in the rooms, then sends each as an event
  named for its kind, a `: ping` comment every fifteen seconds, and `event: stopping` when the service stops on
  purpose.
- **What goes into the stream:** a message the receiver kept or wrote again — its words edited, or the file a
  chat service's `gathering` promised kept under it — as `message.created`, a room's window
  opened or restarted as `window.opened`, a room's window for an agent closed as `window.closed`, carrying the room,
  the agent and the moment it closed, a verdict the receiver wrote of a round as `judgement.created`, carrying the
  room, the message it stands under or null, the stage, the verdict and its detail, a room whose registration was
  removed as `room.deleted`, an act the record kept on a message, a reaction, a pin, an edit or a delete, as
  `message.changed`, carrying the room and the message, the picture
  whenever it changes as `live.changed`, a typing mark as `typing` and a presence as `presence` (「Typing and
  presence」 below), and what `POST /events` says: `message.created`, `briefing.created` or `avatars.changed`.
- **A turn's events name whose turn it is.** Every event a conversation tells of its room's turn — `run-started`,
  `run-progress`, `sent`, `draft`, `run-failed` and `waited` among them — and the turn as the picture holds it
  carry `speaker`, `{id, name}`: `name` is the registration name of the agent whose turn it is, the default
  answerer's as much as any other agent's, and `id` is the number that agent's words are filed under in the room —
  the room's own identity for the default answerer, the number the room's road or the record gave any other
  agent — or null where the room gives it none. The speaker is read as each turn begins, so an event told of a
  room no turn has begun in yet carries none.
- **A turn ends as one of `sent`, `skipped`, `waited`, `run-stopped`, `run-failed` or `run-cut`.** `run-cut` is a
  turn the agency's restart or the rest cut before it ended, which the room's next round continues
  (`docs/bmo-window.md`, 「Quiet instruction」); it carries `cut`, `agency` or `rest`, and `detail`, what the round
  read of it. `run-failed` is a turn that broke.
- **`run-progress` carries the working turn's picture** as `progress`, with `chatId` and `messageId`, sent only
  where the picture changed (`docs/receiver.md`, 「The event stream」): its
  `phase`, `tool`, token counts and `spent`, its last `actions` with `droppedActions` and `droppedContext`, and
  `said`, the turn's last three progress lines, oldest first, each `{text, before}` — the words it wrote and then
  went on working, redacted, run into one line and cut at 240 characters, never its answer
  (`docs/receiver.md`, 「The event stream」) — and `thinking`, the last thinking block the run wrote, redacted and
  cut at the policy's ceiling, on a turn working in the owner's private chat and null on every other
  (`docs/messages.md`, 「A working turn's thinking」).

## Typing and presence

| Route | Body | Answers |
|---|---|---|
| `PUT /accounts/{account}/presence` | `state` (`online`, `away`, `offline`, or null), `lastSeenAt` (epoch milliseconds, or null) | 204 |
| `POST /rooms/{roomId}/typing` | `account`, `on` | 204; `private-room` (403) in another account's private room, `not-found` (404) for a room the receiver does not hold |

- **Nothing of either is filed,** wakes anybody, raises an unread count, moves a read mark or stands in a turn
  (`docs/receiver.md`, 「Typing and presence」).
- **`PUT /accounts/{account}/presence`** holds the account's presence as the console tells it, one source among the
  account's; a `state` of null takes the console's word back.
- **`POST /rooms/{roomId}/typing`** holds the account's mark in the room for the base's six seconds unless told
  again, tells it on the stream, and hands it to the room's chat services as a `typing` send in the writer's own
  voice; a service that refuses is let go.
- **`typing` on the stream** carries `chatId`, `who`, `on` and `lastsMs` (the mark's life in milliseconds, null for
  the answerer's mark, which stands until told off); **`presence`** carries `chatId`, `who`, `state` (null where
  unknown) and `lastSeenAt`. Both carry `id` null: neither enters the ring nor is replayed, and what stands is
  handed after the replay as the stream opens.
- **`who`** is `{kind: "account", account}`, `{kind: "person", via, personId}` for a chat service's person linked to
  no account, or `{kind: "answerer"}` for the room's answerer, whose event carries the turn's `speaker`.

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
| `POST /rooms/{roomId}/messages/{messageId}/reading` | `account?` | `state` (`making` or `ready`) and `briefingId`, the engine's briefing of the line's full reading, null while the line's agent is still writing its script |

- **The switches** are the whole briefing's and whether a summary plays by itself (`docs/voiced-notices.md`, 「The
  voice briefing」); each is on unless it was set off. A body naming neither switch, or both, or a value that is not
  a boolean, is refused 400 `one-switch`. `on` governs only the summaries the receiver speaks unasked: setting
  it to false asks the voice engine to stop every summary it holds, and no full reading; the answer waits on none
  of it, and a stop that failed is asked again until it is made.
- **A line's full reading** is launched in the voice engine where none stands, or answered as it stands there,
  whichever way `on` stands. It is refused 409 `voiceless` where the line's agent has no voice, 404 `not-found`
  where the message is no agent's line with words, and 502 `unreachable` where no voice engine is configured or
  the engine did not answer. The sound is the engine's: the console reads it there by the
  briefing's id. Launching one writes in the line's room, a turn of the line's agent there writing the script and
  a briefing the engine keeps for the room, so another account's ask in a private room is refused 403
  `private-room` before either.

## Handoffs

| Route | Body | Answers |
|---|---|---|
| `POST /handoffs/{agentName}` | `reason`, `room?`, `cause?`, `by?`, `clean?`, `ask?` | 200 `path`, `bytes`: the handoff file the agent's standing session wrote; 204 where it has no session to ask, wrote nothing, was asked to start clean, or was not asked |

- **The agency asks this before it opens a fresh session for an agent that had one** (`docs/unified-bmo.md`,
  「Replacing the session」). The receiver asks the agent's standing session — its one session, or the session of
  `room` where it keeps one in each room — for its handoff file as a replacement does, waited on until its turn
  ends however long it takes, and keeps the file for the next fresh session's first turn, which opens on it. The
  asking stands behind the agent's round: a round doing its own work between two of its turns is waited for,
  while one that itself waits on the agency for a turn is not, since that turn cannot end before the handoff is
  answered; and the agent's turn is held while the handoff is asked, so words that arrive meanwhile, and words a failed
  round gave back, go to the fresh session's first turn in their room. A session already in a turn the receiver
  did not send is waited for: the receiver waits for that turn's end on the agency's event stream and asks again,
  however long it takes. The handoff turn answers `room`, carrying its block, and the owner's private
  chat where none is named. `reason` names why a fresh session is coming, for the receiver's journal.
- **`clean: true` starts the fresh session clean.** The standing session is asked one turn, marked `CLEAN CLOSE.`,
  in which it consolidates its brain and tells the brain it folds exactly as the handoff turn does, and writes
  no handoff file and nothing else; it writes no owed situation note either. Nothing is kept for the fresh
  session: a note kept for it before is let go, and its first turn opens on no handoff, no transcript and no word
  that nothing was recovered, even where the agency's state names the handoff lost, and the owner is told of no
  lost memory (`docs/unified-bmo.md`, 「Replacing the session」). The asking waits, stands in line and answers
  `stopping` as a handoff's does, and is answered 204. `clean` absent or false asks for the handoff.
- **`ask: false` asks no session at all.** The agency sends it where no standing program is left to ask, or where it
  made the handover itself, as it does for a room's own session: the receiver writes the room's line alone, at
  once, waiting on no round and no asking in line, and answers 204; with `clean: true` it also lets go whatever
  was kept for the fresh session and has its first turn open on no note, as a clean asking does. `ask` absent
  or true asks.
- **A room is told of it in one line,** `session-handed-over`, kept in the record alone of one room and sent to no
  chat service (`docs/unified-bmo.md`, 「The room's line」). `cause`, one of the `HandoverCause` kinds
  (`docs/codes.md`), is the cause the line names, and a word that is none, or none given, names `other`; `by`,
  the account key of the person who asked, is named by the line where given. An asking of the agent's one
  session across rooms tells it once the asking is done, whether or not a standing session was there to ask
  and whether or not it wrote anything: in `room`, else the room of the round under way the asking stood behind,
  else the owner's private chat. An asking with `ask: false` tells it for either kind of session: in `room`,
  else the owner's private chat. An asking of a room's own session with `ask` true tells nothing, since the
  agency tells that handover with `ask: false` once it has made it. An asking answered by the handoff kept within
  `again_minutes`, and one made from inside the handoff under way, tell nothing. Nothing of the room's record is
  deleted or hidden.
- **The agency lets its record of the session go before it asks,** so the session's state names no generation
  while its program still stands. The receiver reads no generation to decide whether to ask: it asks the standing
  session, and the asking itself tells whether there was one to ask.
- **An asking never outlasts the service's stop.** However long it waits — in line, behind a round, on the
  handoff turn — it is answered 503 `stopping` as the receiver is told to stop, so a stop never waits on it;
  the agency opens its fresh session without the handoff.
- **An asking while a handoff of the same agent is under way waits in line** behind it, and is then answered as
  an asking again within `again_minutes` is. One made while that handoff's own turn waits on the agency comes
  from inside that turn, which cannot end before it is answered, so it is answered 204 at once, and the
  handoff under way writes the note the fresh session opens on.
- **An asking within `again_minutes` of the last one asked** (`handover.toml`) does not reach the standing
  session: it answers 200 with the file that one kept where it is still kept, else 204, and the journal counts
  the askings in a row. A clean one answers 204 and keeps nothing for the fresh session all the same.
- **A failure to reach the agency** answers 502 `upstream-failed`, and tells no room.

## Handovers

| Route | Body | Answers |
|---|---|---|
| `POST /handovers/{agentName}` | `reason`, `room?`, `by?` | 202 `agentName`, `sessionReason` (`asked`): the agent's one session hands over once the agent is quiet |

- **A handover is asked for the agent's one session across its rooms** (`docs/unified-bmo.md`, 「Replacing the
  session」). The route writes the word to start that session afresh with the reason `asked` and answers at once;
  it waits on no turn, so an agent may ask it of its own session from inside its own turn. The session is
  replaced once the agent is quiet — at once where it is in no round, else as the round it is in ends — as every
  replacement is: the old session writes its handoff file, is ended, and the new session's first turn opens on
  the note.
- **`reason`** is a non-empty sentence saying why, in the asker's words; an empty one is refused 400
  `bad-request`. The journal holds one line naming the agent and the reason, and the room's line names it.
- **The room is told once the replacement is made,** in one `session-handed-over` line with the cause `asked`, kept
  in the room's record alone and sent to no chat service (`docs/unified-bmo.md`, 「The room's line」): in `room`, a
  room number, where the asking names one, else the room of the round behind which the replacement is made, else
  the owner's private chat. `by`, the account key of the person who asked, is the line's `by`; an asking that
  names none, as the `handoff` command's, names no one. A `room` the receiver holds no road for is refused 404
  `not-found`. A second asking before the replacement replaces the first's words, room and asker.
- **Only the session is replaced.** No message, file or minutes of any room is deleted or hidden.
- **An agent with no opened session** is given the word all the same, and its next turn spends it in the one
  replacement. A second asking before that turn writes the same word again, so one replacement answers both.
- **An agent the agency does not register** is refused 404 `not-found`, **one that keeps a session in each room**
  409 `conflict`, since a room's own session is the agency's to resume, and **a failure to reach the agency** 502
  `upstream-failed`.

## Kept lines

| Route | Body | Answers |
|---|---|---|
| `POST /lines/{agentName}` | `line` (a fixed line: its `code` and its fields) | 201 `room`, `messageId` (its number in the room's record, null where no sentence could be made of it) |

- **A line another unit composes, kept in a room's record alone.** The line is one a unit asks to be kept (the
  fixed lines that say so in `docs/codes.md`, `slept` alone today); any other, or one that does not read, is refused
  400 `bad-request`. It is kept as every record-only line is (`docs/voiced-notices.md`, 「Lines that stay fixed」): in
  the road's own voice, with the kind `fixed-line` and the line it was said from, the screens reading the room told,
  and nothing sent to any chat service, so no person is messaged and no session is woken.
- **The room is the agent's owner's private room:** the agent's own room on the record, where one was made for it
  (「Supervisors」), else the owner's private room. An archived room, or one no road holds, keeps nothing and is
  answered 409 `conflict`; a road that fails is answered 502 `upstream-failed`.

## Rounds

| Route | Query | Answers |
|---|---|---|
| `GET /rounds/openings` | `room`, `rounds` (comma-separated) | a page of `{round, instructions}`, each instruction `id`, `sort`, `summary`, `createdAt` |
| `GET /rounds/openings/{instructionId}` | `room` | `id`, `text`: one quiet instruction whole |
| `GET /rounds/pipeline` | `room`, `rounds` (comma-separated), `limit` | a page of `{round, rows}`, each row a `PipelineRound` |

- **An `InstructionChain`** and an `InstructionText` are described in 「The routes」 of `docs/instruction.md`: the chain a round runs, and each stage's editable text.
- **A `PipelineRound`** is one turn of a round's pipeline, or its end (`docs/instruction.md`, 「The round log」):
  `id`, `agentName`, `number` (from 1, the round's own turn first and then the check of the words that arrived meanwhile, the audit and each instruction stage in the order asked; 0 for the agent's own
  judgement before a round), `question` (the row's name: the stage it ran, `arrived` for the closing check, `message` for the round's own turn), `kind` (`supervisor`, the agent's own judgement whether to answer words
  its open window alone let in; `turn`, the round's own turn; `arrived`, the closing check; `audit`, the audit of the people the message answers; `stage`, an instruction stage; or `end`), `answerChars`,
  `changed`, `outcome` (`answered`, `send`, `no-verdict`, `skip` or `failed` for a turn or check, and `pass` for a check whose empty answer kept the message; `pass`, `revised` or `failed` for the audit and a stage, each row carrying its answer; `message` or `skip` for the end), `detail` (or null: a row ending `skip` names there why the
  round sent nothing, or `none given`), `answer` (or null), `ms`, `at` and `reads`:
  on the end, what the round read from the brain, each `path`, `holds` (or null) and `how` (`attached`, `held` or
  `read`; `docs/instruction.md`, 「The round log」), and empty on every other row; `session` and `loaded`: on the end,
  the agency session the round's turn ran on (or null) and how many pieces that session's earlier rounds in the room
  read in full (`docs/instruction.md`, 「The round log」), and null on every other row; `versions`: on a turn's row
  whose turn spanned more than one turn of the agent's program, each of those turns' answers in order, `text` and
  `opened` (`handed` or `program`), the row's `answer` built from them (`docs/instruction.md`, 「The round log」), and
  empty on every other row. The rows of a round stand in the order they were written.

## The instruction stages

| Route | Body | Answers |
|---|---|---|
| `GET /instruction-stages` | query `agentName?` | a page of `InstructionChain`: the policy's chain and each supervisor's own; with `agentName`, the one chain that supervisor follows |
| `PUT /instruction-stages/{agentName}`, the agent a supervisor's name | `stages` | the `InstructionChain` as it now stands |
| `DELETE /instruction-stages/{agentName}` | | 204; `not-found` where the console set no chain of that name |
| `GET /instruction-texts` | query `agentName?` | a page of `InstructionText`: that supervisor's catalogue, or the policy's alone where none is named |
| `PUT /instruction-texts/{agentName}/{name}` | `text` | the `InstructionText` as it now stands |
| `DELETE /instruction-texts/{agentName}/{name}` | | 204; `not-found` where the console set that supervisor no text of the name, `conflict` where a chain of its supervisor still names a stage the console added |

- **An `InstructionChain`** is `agentName` (the supervisor's name, or `*` for the policy's default chain every
  supervisor follows until it sets one), `stages` (the chain's stage names, in the order they are asked once the
  message stands; empty runs no stage and the message goes out unchecked) and `source` (`set` where the console set
  the chain, `default` where the policy's stands). The page opens with the policy's chain, then each supervisor's own
  by name.
- **An `InstructionText`** is `name`, `text`, `source` and `policyText`: the policy's text of the stage wherever the
  policy holds it, so a screen can set a supervisor's text back to it, and null for a stage the console added. The page carries the policy's
  stages, each text replaced by the console's for that supervisor, then the stages the console added for it.
- **A chain set is refused `bad-request`,** with nothing written, where it names a stage whose text the catalogue
  does not hold; an empty chain is allowed and runs no stage. A stage's name that is not a lowercase word of letters,
  digits and dashes opening on a letter, of at most 40 characters, or one of the fixed turns' names `message`,
  `arrived` and `audit`, or a text that is empty or past 4,000 characters, is refused the same way.
- **A stage's text taken away** leaves the catalogue where the console added the stage, and stands again in the
  policy's text where the policy holds it. A stage the console added that a chain of its supervisor still names is
  refused `conflict`, since the chain would name nothing: it is taken out of the chain first.
- **A set or a removal reaches the supervisor's next round** and replaces no session: a stage's words ride its own
  turn, and nothing of the room's block moves with them (`docs/instruction.md`, 「The stage chain and its texts」).

## Supervisors

| Route | Body or query | Answers |
|---|---|---|
| `POST /supervisors` | `name`, `label`, `repository` (optional), `room`, and the road: `provider`, `model`, and `effort`, `thinking` and `answerTokens` (each optional), and `ownerAccount` (optional), the account it belongs to | 201, `supervisor`, the agency's read of it, `brain`, what became of its brain, and `room` (`id`, `takenBack`, `said`) |
| `DELETE /supervisors/{supervisorName}` | `by`, `forgetBrain`, `force` | 204; `unknown-supervisor` (404), `seeded` (409), `default-answerer` (409), `running` (409), `action` (400) |
| `POST /supervisors/{supervisorName}/resets` | `conversation`, `person`, `dryRun`, `by` | the agency's answer to the reset as it came; `unknown-supervisor` (404) |
| `GET /supervisors/{supervisorName}/rooms` | | a page of `SupervisorRoom` (`id`, `title`, `named`, `alone`): the rooms it answers |
| `GET /default-answerer` | | `supervisor`, the name the setting holds |
| `PUT /default-answerer` | `supervisor` | `supervisor`, as the setting now stands; `unknown-supervisor` (404) |

- **Every supervisor is alike to the receiver.** Its rounds are the same rounds, its judgement of window words
  first and its pipeline among them (`docs/wake.md`); what tells one from another is its own row at the
  agency and the rooms that name it here.
- **A room names the supervisors that answer it,** the room's `supervisors`. A room that names none is answered by
  the supervisor the user it is a one-to-one with owns, where one stands, and otherwise by the default answerer;
  a room is added naming none, so the default answerer is also the supervisor a new room gets. `PUT
  /rooms/{roomId}/supervisors` writes the whole list, each name one the agency registers; an empty list hands
  the room back to the supervisor its user owns where one stands, and otherwise to the default answerer. In a
  room added `avatar` the list is also the whole of who its words may wake there: the member's call of their own
  supervisor reaches no supervisor outside the list, and an empty list is that setting left unset
  (`docs/wake.md`, 「Who reaches the session」).
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
  `label`, `provider`, `model`, `repository`; a name the agency holds already as `name-taken`; any other as
  `upstream-failed`). A name the agency registers already is refused `name-taken` before anything is written, and
  so is a name whose room the record keeps as another account's private room, which stays that account's. With
  `ownerAccount` the room is kept on its record as that account's private room from the start.
- **Its road is added with it, in one ask.** A supervisor stands on one road: `provider` and `model`, with
  `effort` (a level of the reasoning policy), `thinking` (`on` or `off`) and `answerTokens` (the answer ceiling,
  on the registry's provider) where the console names them, each left to the provider's own where it does not.
  The receiver hands the road to the agency's add as it came, and the agency judges it as a road is judged and
  writes the row with it; a road it refuses, an effort the model does not take among them, comes back as
  `provider`. A supervisor holds no provider order: the add reads no `order` and no `quotaShare`, and a
  body naming either is added without it.
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
  instruction stage chain and its question texts (`docs/instruction.md`, 「The stage chain and its texts」); a room left naming none is
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
| `PUT /chat/rooms/{roomId}/messages/{messageId}` | hands over a message, live, edited or read back; one handed over with a `line` (bmo-chat's `docs/contract.md`, 「Lines」), live or read back, is kept as kind `fixed-line` with the line's `code` and `fields` as they came, beside its words (「Messages」) |
| `PATCH /chat/rooms/{roomId}/messages/{messageId}` | hands over the files a gathering message promised |
| `GET /chat/rooms/{roomId}` | asks the room as the receiver holds it; `read` is true for a room added on the asking provider and still on its list, whatever its mode, and false for every other room (`docs/receiver.md`, 「A read-only room」); `listened` is the room's own, as a `Room` answers it (「Rooms」); the answer is read off what the receiver holds, and no chat service is asked for it (`docs/receiver.md`, 「What is kept for a moment」) |
| `GET /chat/rooms` | asks the rooms whose past it may read back |
| `POST /chat/avatar-changes` | tells of kept profile pictures that changed; the receiver takes each subject named from the provider's avatar routes before it answers (`docs/pictures.md`, 「Profile pictures」) |
| `POST /chat/reaction-changes` | tells one message's reactions as the platform shows them now |
| `POST /chat/deletions` | tells of messages of one room deleted on the platform |
| `POST /chat/typing-marks` | tells that a person is writing in a room, or has stopped; the receiver holds the mark and tells it on the stream, and passes over one about a room it holds nothing of or whose chosen list does not name the service, answering each as taken (「Typing and presence」) |
| `POST /chat/presence-changes` | tells whether people are reachable; the receiver holds each person's presence and tells it on the stream in the rooms the person stands in, and passes over a person it holds in no room, answering the change as taken (「Typing and presence」) |

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
| the web unit | the account model: a sender's user, a user's senders, a fixed line rendered |
| the cache warmer | the consumer and its one target, the chat touch |
| each chat service | every provider route of the protocol, through `bmo-chat-client`, its avatar routes among them for the profile pictures the receiver keeps and serves |

A call is offered again only where the connection was refused, every two seconds until the unit listens; a call
that reached a listener is never offered again, and nothing bounds how long a unit takes to answer. A call on the
brain is offered once, a refused connection ending it (`docs/boundary-api.md`, 「The brain」).
