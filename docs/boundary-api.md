# The receiver's boundary

This document is the receiver's boundary: every route it serves, every call it makes on another unit, and every
code it answers with. A unit reaches the receiver only across it, and the receiver reaches another unit only by
the calls below. Rooms are the receiver's: the chats it receives through the chat services, their owner, the
messages said in them, the agents that answer in them, each room's rules, rights and windows, the instructions it
hands an agent's session there, and the envelope a turn goes out in. What a turn is told, whom an agent answers
and whether an answer may go into the room are decided here; the agency carries the turn, the gate chooses its
reference and reads it on the way in and out, and the brain keeps what the agents remember.

## Configuration

The receiver reads one configuration file, `settings.toml` in its working directory — the release's own, which
each release links to the data root's `env/receiver.toml`. A deployment that finds no such file is refused, and
the unit does not start where the data root holds none. `settings.example.toml` at the repository root is the
checked-in example, naming a database and neutral names, and it loads as it stands.

Beside its own listen address, its token and its database it names, for each unit it calls, that unit's base
address and the token that unit takes:

| Unit | What the receiver reaches it for |
|---|---|
| the agency | the agents' registrations, their sessions and the turns carried in them, the events of those turns, the hooks it answers, a room forgotten, a run's log, the deployment's rest and whether a stand-in answers while it closes a road |
| the brain | room rules, reminders, a session's loaded documents, a chat session's handoff note, what a turn wrote into memory, and a room's rows when the room goes |
| bmo-job | a room's holdings, the questions of its jobs the owner answers in his chat, and the items a turn filed |
| the gate | a turn's reference, the sanitiser, a room's judgements |
| the web unit | the account model: a sender's user, the checks, the grants |
| the voice unit | a voice briefing launched and its state |
| the cache warmer | the chat touch target, registered and reported |
| each chat service | everything a room's road does, through bmo-chat-client |

The deployment's names are read from the same file: the owner; the chat agent's registration name, whose label
and wake words (`GET /agents`) are the words a room calls it by; the delegate's registration name, whose label a
delegation notice names; the infrastructure room, where one is named; the language the rooms are spoken in, which
a fixed line is rendered in; the attachment size cap, the largest file a message may carry to be kept; the brain
root, where the brain's documents stand, `brain` under the data root where none is named, which a document
reload reads the brain's documents from; the document tree, where the documents that ship with the agents' code
stand (the deploy tree of the unit that ships them), which a reload reads such a document's current text from and
which, where none is named, leaves such a document named without its text; and each chat service's road name with
its base address and token.

### The service and its commands

- **The service runs as the user unit `bmo-receiver.service`,** this repository's `deploy/bmo-receiver.service`,
  which the deployment links from the deploy tree under `~/.local/share/bmo-receiver`. It starts the deploy
  tree's `bin/bmo-receiver` with `serve`, and does not start where the data root holds no configuration.
- **`serve`** runs the migrations, stands the receiver on the configuration — its news heard by the event stream,
  its messages judged by the one rule, its turns handed to the one runner, its rows handed over by the one
  instruction pass — settles the start (the data root's folders made, the cache warmer told of the consumer and
  its one target, the agency told of the three hooks it answers), keeps the clocks and answers the routes on the
  configured address until the process is told to stop; on a stop it tells every stream first and says the
  going-down line to the owner after.
- **`routes`** prints every route the receiver answers, one method and path to a line, sorted; the table is what
  the router is built from, so the printed names are the routes that stand.
- **`deliver-waiting`** runs one pass over the instruction rows a job's end left waiting, told as the receiver's
  sweep tells them, with no clock of the receiver's: what a check stands in for the sweep with once a runner has
  ended.
- **`tool-env`** writes where the receiver's tools find it (「Its tools」 below) and ends.
- **`health`** asks the service the configuration names whether it is ready and ends (「Health」 below).
- **The consumer's name** the receiver registers with the cache warmer is `receiver`, unless the configuration
  names another.

## Wire conventions

The receiver serves route families its callers' documents state, and each family keeps its caller's
conventions, so a caller reads the receiver as its own document says:

| Family | Bodies | Error shape | Stated by |
|---|---|---|---|
| the brain's | JSON, camelCase, all `POST` | the chat wire protocol's: `code` (kebab-case) and `error` | the brain's `docs/boundary-api.md`, 「Wire conventions」 |
| bmo-job's | JSON, snake_case | `error` and `code` (snake_case) | bmo-job's `docs/api.md`, 「A refused request」 and its calls on bmo-agency |
| the chat protocol's | as `bmo-chat-client` serves them | the protocol's | bmo-chat's `docs/protocol.md` |
| the cache warmer's hook | JSON, snake_case | none: every answer is 200 with an outcome | the warmer's `docs/api.md`, 「The ping hook」 |
| the agency's hooks | `{ "point", "ask" }` in, `{ "answer" }` out | as the receiver's own | the agency's `docs/api.md`, 「Hooks」 |
| the receiver's own | JSON, camelCase | `code` (kebab-case) and `error` | this document |

- **Auth.** Every route takes `Authorization: Bearer <token>` with the receiver's token, and answers 401
  without it. The cache warmer posts its hook with no header, so the hook's URL the receiver registers carries
  the token as its `token` query parameter, which the hook route alone reads.
- **Numbers.** A room is named by its chat, a signed 64-bit integer that may be negative; so are senders and
  messages. Times are what the family's document says: Unix seconds for the brain's and the chat protocol's,
  epoch milliseconds for bmo-job's and the warmer's.
- **No sentence a person reads goes out of the receiver.** Every answer is a code with its fields, an English
  message only where an error needs one. What a room is told is a fact handed to the agent that speaks for it,
  which says it in the room's language.

### The receiver's own error shape

| Status | `code` | When |
|---|---|---|
| 400 | `bad-request` | the body is not JSON or not the shape the route takes; the sentence names what is missing |
| 401 | `unauthorized` | the token is missing or wrong |
| 403 | `forbidden` | the asker may not do this, as the account model says |
| 404 | `no-route` | no route at this method and path |
| 404 | `not-found` | the room, message, instruction or run named does not exist |
| 409 | `not-ready` | the service is up but cannot answer yet: its database or its conversations are not standing |
| 409 | `conflict` | the request contradicts what stands; the answer carries the refusal's own code |
| 413 | `too-large` | a file past the ceiling a road takes |
| 502 | `unit-failed` | a call the route needed on another unit failed; `unit` names which |
| 503 | `unreachable` | a chat service the route needed does not answer |
| 500 | `failed` | the service failed while answering |

bmo-job's routes answer the same cases with the same status and the code in snake_case (`bad_request`,
`not_found`, `unit_failed` and so on).

A route's own refusals, the facts the receiver hands a room's agent and the fixed lines a room hears are the codes
of `docs/codes.md`, each with its status and its fields.

## Routes served

### For the brain

The brain's side of these routes is stated in its `docs/boundary-api.md`, 「The receiver」; the receiver serves
them as stated there. Every one is a `POST`. A message is answered as `chatId`, `messageId`, `senderId` (or
null), `senderName`, `sentAt`, `text` and `replyToMessageId` (or null).

| Operation | Route | Body | Answer |
|---|---|---|---|
| unread messages | `POST /messages/unread` | `agent`, `chatId`, `limit`, `since` or null | `messages` past the agent's read marks, sent at or after `since` where it is given, oldest first |
| unread counts | `POST /messages/unread-counts` | `agent`, `since`: each `chatId` and `since`, the moment the agent's book of that room starts reading | `rooms`: each room holding messages past the agent's read marks, its `chatId` and `count`; a room named in `since` counts only the messages sent at or after its moment |
| read messages | `POST /messages/read` | `agent`, `chatId`, `from`, `limit` | `messages` the agent has marked, after the message `from` in the room's order (or, where that message is gone, with an id above `from`), oldest first |
| read count | `POST /read-marks/count` | `agent`, `chatId` | `count`: the agent's marks on the room's messages that still stand |
| keep read marks | `POST /read-marks/keep` | `agent`, `chatId`, `messageIds` | `ok` |
| forget read marks | `POST /read-marks/forget` | `agent`, `chatId` or null for every room | `forgotten`: the count of marks let go |
| room titles | `POST /rooms/titles` | none | `rooms`: each room's `chatId`, `title` (or null) and `senders`, the people that have stood as a sender in it |
| room answerers | `POST /rooms/answerers` | `chatId` or null | `rooms`: each room's `chatId` and `agents`, the agents answering it: the awake agents it names, or the default answerer where it names none awake |
| spoke between | `POST /rooms/spoke-between` | `chatId`, `person`, `from`, `to` | `spoke` (boolean): the person sent a message in the room between the two moments, both included |
| own number | `POST /rooms/own-number` | `agent`, `chatId` | `number` (or null): the sender the agent's own words stand under in the room: the number the room's road gave the agent, or for the default answerer where it gave none, who the road says the agent is there |
| spoken in | `POST /rooms/spoken-in` | `chatId`, `people` | `people`: those who have stood as a sender in the room |
| display names | `POST /people/names` | `people` | `names`: each person's `person` and `name` or null: the name their latest message went under, a room where the owner goes by a name of that room alone read last |
| waits on a happening | `POST /promises/waits` | `text` | `waits` (boolean): whether the text waits for something to happen first, by the receiver's reading of promises |
| roads | `POST /rooms/roads` | `chatId` | `kind` (`owner` or `group`) and `oneToOne` (boolean) |
| sender is owner | `POST /messages/sender-is-owner` | `chatId`, `messageId` | `owner` (boolean) |
| owner spoke | `POST /rooms/owner-spoke` | `chatId`, `person`, `before`, `seconds` | `owner` (boolean): the person is the owner; `spoke` (boolean): the owner sent a message in the room in that span |
| owner notice | `POST /owner/notice` | `code` and its parameters beside it | `ok` |
| quiet instruction | `POST /instructions/quiet` | `agent`, `chatId`, `text`, `kind` (`watch` or `reminder`) | `ok`: the instruction is queued for the agent in the room, whose turn it wakes there |

- **An owner notice** is queued as an instruction for the chat agent in the owner's private chat, carrying the
  code and its parameters as a fact; the agent says it in its own words. The codes are the brain's (its
  「Owner notices」); the receiver hands on a code it does not know as it came.
- **A quiet instruction** is queued for the agent in the room under the sort its `kind` names (a watch's memo or
  a reminder's wake) and wakes the agent's turn there through the instruction pass (`docs/bmo-window.md`,
  「Quiet instruction」). It is answered once it is queued, which is when the brain counts it taken.

### For bmo-job

bmo-job's side of these routes is stated in its `docs/api.md`, 「bmo-receiver」, and its `job-receiver` crate;
the receiver serves them as stated there, with snake_case bodies.

| Call | Route | Request | Answer |
|---|---|---|---|
| Queue an instruction | `POST /instructions` | an instruction | 201, `id` |
| Read its state | `GET /instructions/{id}` | | `state`: `pending`, `handed` or `delivered` |
| Withdraw it | `DELETE /instructions/{id}` | | `withdrawn`: false where it was handed already or is gone |
| Voice a notice | `POST /rooms/{chat}/notices` | a notice | 201, `id`: the instruction it was queued as |
| Read the owner | `GET /owner` | | `sender` (the owner as a sender), `chat` (the owner's private chat) |
| Read a room | `GET /rooms/{chat}` | | a room |
| Read a room's answerers | `GET /rooms/{chat}/answerers` | | the answerers |
| Read a message | `GET /rooms/{chat}/messages/{message}` | | a message |
| Talk | `POST /talks` | `topic`, `[turns]`, `said` | `answer?`: null where the agent said nothing |
| Declare a holder | `PUT /room-holders/{unit}` | `address`, `[holdings]` | 204 |

- **An instruction:** `agent?`, `chat?`, `said`, `kind?`, `sort`, `summary?`, `expires_at?`, `[events]`,
  `rides_until?`, `via?`, `run?`.
  - `agent` is the agent whose session takes it, the deployment's agent where null; `chat` is the room, the
    owner's private chat where null.
  - `said` is an object of one key: `fact`, holding a fact as bmo-job's 「Codes」 gives it, or `order`, holding
    the words of an order.
  - `kind` names what it is about: queueing one replaces the pending instruction of the same kind in the room.
  - `sort` names its family, such as `job`, `notice`, `overtime` or `run_failure`; `summary` is the line it is
    listed under, made from the fact where null.
  - `expires_at` is when it lapses; `events` are the events its delivery carries; `rides_until` makes it ride
    the next turn rather than open one, until that time; `via` is the road the session was read on, which it is
    delivered on; `run` is the run whose result it is.
  - An instruction withdrawn, replaced or lapsed reads as nothing found.
  - **An instruction for a room whose registration is gone** (`docs/receiver.md`, 「Removing a room's
    registration」) goes to the owner's private chat instead, never into the platform room: it is queued for
    the voice of the owner's chat, whatever `agent` named, and opens with the line the policy keeps for it
    (`chat-round.toml`, `[instructions]`), which names the room by its number. It goes on no road and of no
    kind, since the removed room's road is not the owner's and its kind must not replace a pending row of the
    owner's own; its `run`, `events`, `sort`, `summary`, `expires_at` and `rides_until` are kept.
  - **An instruction whose agent no longer answers its room**, such as the asker of a run taken off the room's
    answerers or resting since it asked, is queued for the room's voice instead (`docs/voiced-notices.md`, 「The
    rule」), with everything else it carries kept as given, so what it holds still reaches the room. An agent
    answers a room that names it among its awake answerers, and the default answerer every room that names
    nobody awake. An instruction whose `agent` is null is the deployment's agent's as it stands.
- **A notice:** `fact` (a fact), `intent` (what the line is for, in English for the agent), `follow_up?` (what
  follows it), `optional` (true where the voice may leave it unsaid when the fact leaves the room nothing to
  hear), `kind?`, `expires_at?`, `[events]` and `rides_until?`, the last four read as an instruction's.
- **A room:** `chat`, `owner_room` (true where it is the owner's private chat with the deployment's agent),
  `registered` (true where its registration stands), and `work_chat`: where the news, questions and answers of
  work filed from it are told, the room itself while its registration stands and the owner's private chat once
  it is gone. Any chat reads as a room; one never registered reads as one whose registration is gone.
- **The answerers:** `default` (the agent that answers where a room names none), `voice` (the agent that speaks
  for the room), and `answerers`: a list of `agent` and `companion` (true where it answers beside the voice
  rather than as it), every agent named to answer in the room that is not resting.
- **A message:** `sender?` (null where no sender is known), `sender_name?`, `sent_at`, `text`. A message the
  receiver does not keep reads as nothing found.
- **A talk's topic** is an object holding `kind`: `item`, with `id`, `title`, `done`, `merged_into?`, `held`,
  `hold?`, `stopping`, `approved` and `status`; or `document`, with `document`, `label`, `writer`, `file`,
  `version`, `heading?`, `passage?` and `translated`. A turn is `owner` and `text`. The talk is a round of the
  owner's private chat on the chat agent's session (`docs/unified-bmo.md`). A talk that comes back with nothing
  is refused: `resting` with 409 where the agent rests or the owner's chat has fallen silent, `limit` with 429
  where its provider's limit is reached, `outage` with 503 where its provider is out, and any other code for any
  other failure.
- **A holding:** `name` and `effect`: `kept` where the rows stay and keep the room's chat after it is removed,
  `warns` where the receiver shows what the unit answers before it removes the room. Declaring again replaces
  the unit's declaration. The receiver asks every declared holder `GET <address>/rooms/{chat}/holdings` before it
  removes a room (「Removing a room」 below).

### For bmo-artifact

A search's, a browse's or a remote browser's run hands what it came to back to the session of the room that
asked, as a quiet instruction. bmo-artifact queues it on the same route bmo-job does, `POST /instructions`, with
the same body and the same answers as 「For bmo-job」 states them, under its `docs/api.md`, 「What this repository
calls」.

- **What a run's ending carries:** `agent` (the agent whose turn set the run going, the room's voice taking it
  where that agent no longer answers the room), `chat` (the room it was asked in), `said` holding an `order` (the
  words the session is handed), `sort` (the kind of ending), `via` (the road the question came in on) and `run`
  (the run in the agency's ledger).
- **A room whose registration is gone** takes nothing: the ending goes to the owner's private chat, naming the
  room, as 「For bmo-job」 states for every instruction.
- **The run is kept on the row,** so a companion's round can tell an ending that is no longer its to answer: a
  row whose run the agency answers as begun at or before the companion's reset point reaches no turn, as the
  words said before that point reach none (`GET /runs/{id}` under 「The agency」 below; `docs/wake.md`, 「A
  companion's round」); a row whose run the agency cannot answer for is kept.

### For the chat services

The receiver routes of bmo-chat's `docs/protocol.md`, 「Receiver routes」, served through `bmo-chat-client`'s
server exactly as the protocol and its `openapi.json` state them:

| Route | What a provider hands over |
|---|---|
| `POST /message` | a message a room said |
| `POST /attachment` | a message carrying a file, with the file or the promise of it |
| `POST /edited` | a message already handed over, said again with different words |
| `POST /gathered` | the file a message's `gathering` promised, or why it was not kept |

The receiver archives what it is handed whatever its origin, and decides what becomes of it by what the
provider declares, never by which provider it is (`docs/receiver.md`).

### For the cache warmer: the chat touch

The receiver is a consumer of the warmer (its `docs/api.md`, 「Consumers and their targets」). It registers itself
with `PUT /consumers/{consumer}` and its one target, the chat touch, with
`PUT /consumers/{consumer}/targets/chat-touch`:

| Field | Value |
|---|---|
| `kind` | `chat_touch` |
| `label` | the chat agent's one session, in the room it last stood in |
| `subject` | the chat agent's registration name |
| `rank` | 90 |
| `idle_ms` | 2 400 000 (40 minutes) |
| `lifetime_ms` | 3 600 000 (60 minutes) |
| `margin_ms` | 0 |
| `ping_bound_ms` | 1 800 000 (30 minutes) |
| `warm_by_default` | true |
| `failure_touches` | false |
| `honours_idle_override` | true |

The values are the receiver's policy's (`docs/policy.md`). Each turn of the chat agent's session reports
`POST /consumers/{consumer}/targets/chat-touch/used` with `at` and `started_at`. The consumer's name is the
deployment's, `receiver` unless it names another.

| Route | Body | Answer |
|---|---|---|
| `POST /warmer/ping?token=<token>` | `consumer`, `target`, `kind`, `subject`, `bound_ms` | 200: `outcome` (`warmed`, `busy`, `skipped` or `failed`), `code?`, `reason?` |

- The touch is a turn of the room the chat agent's session last stood in, handed the keepalive prompt
  (`docs/bmo-window.md`, 「The touch」).
- `busy` while any room is in a conversation or a compaction; `skipped` with a code while the chat agent keeps a
  session per room, or its provider is off, resting or down; `failed` with an English reason where the touch went
  and did not warm.

### For the agency: its hooks

The receiver registers at three of the agency's points with `POST /hooks` (its `docs/api.md`, 「Hooks」), each
with the URL below and the receiver's token. The agency posts `{ "point", "ask" }` and reads `answer`.

| Point | Route | `ask` | `answer` |
|---|---|---|---|
| `room-title` | `POST /agency/hooks/room-title` | the chat id | the room's title, or null |
| `typed-line` | `POST /agency/hooks/typed-line` | `text`, `sessionId` | the context a line typed into the chat agent's program goes on with, or null (`docs/bmo-window.md`, 「A line typed into the program」) |
| `road-verbs` | `POST /agency/hooks/road-verbs` | `chat`, `via` or null | every verb the road the room is answered on performs |

### The receiver's own routes

These are the routes the console, the receiver's tool and the `bmo` command line reach it on.

#### Sends

| Route | Body | Answer |
|---|---|---|
| `POST /sends` | `asker` (`chat?`, `via?`, `agent?`, `by?`, `job`), `tool` and `input` | `lines`, what the act said, and `refusal` where it ended in one |
| `POST /sends/can` | `chat`, `via?` | `can`: the verbs the road that room is answered on performs |
| `POST /consult/say` | `agent`, `text`, `replyTo?` | `ok`, `chatId`, `messageId`, `lines`: each agent's window in the consult room |
| `POST /consult/block` | `agent` | `ok`, `block`: what the agent is shown of the other agents' work, or null (`docs/consult.md`) |

- **`tool` and `input`:** `say` (`chat?`, `via?`, `text`, `reply`), `photo` (`chat?`, `via?`, `pictures`: each
  `url` and `label?`, `caption?`), `remind` (`chat?`, `at?`, `every?`, `reason`, `memory?`, `user?`,
  `message?`), `react` (`chat?`, `via?`, `message`, `emoji`), `pin` (`chat?`, `via?`, `message`, `unpin`),
  `sticker` (`chat?`, `via?`, `emoji`) and `random_draw` (`chat?`, `via?`, `mode`, `shape` and the shape's
  fields, `docs/random-draw.md`). The act goes out as the asker's agent, on the road the turn names
  (`docs/messages.md`).
- **`remind`** is carried to the brain's `POST /reminders/set` as the asker's agent in its room (「The brain」 below),
  and answered with the reading the brain gives; the brain keeps a reminder by its memory row alone, so the message
  a call names is not carried. Asked outside any agent's turn it is refused, since a reminder is one agent's.
- **A line asked to be said** goes in a run of its own, whose launcher the receiver's binary fills; asked of a
  service standing without one, it is turned away before its words are written anywhere.

#### The owner's words from the console

| Route | Body | Answer |
|---|---|---|
| `POST /say` | `chatId`, `text`, `replyTo?` | `ok`, `messageId` |
| `POST /say-file` | a form: `chatId`, `text?` (the caption), `replyTo?`, and up to ten `file` fields | `ok`, `messageId` (the first send's), `sent` (each `name`, `messageId`), `checking` (each file the road is still drawing on), `failed` (each `name`, `reason`) |
| `POST /instruct` | `chatId`, `text` | `ok`: the owner's words queued for the room's session as a quiet instruction |

`POST /say` and `POST /say-file` in the owner's private chat archive the words under a console number and hand
them to that chat's conversation; in any other room they go as the owner, on the road the room declares for the
owner's voice (`docs/receiver.md`, 「The owner's words」). A file past the road's ceiling is refused 413 `too-large`
with a sentence naming the file and the ceiling; a `replyTo` the archive does not hold is refused 409.

#### The event stream and the live board

| Route | Answer |
|---|---|
| `GET /events?after=&run=` | a server-sent stream: `event: run` first, then each event as `id: <n>` and `event: bot` with its JSON out of a ring of the last 500 replayed after the cursor, then `event: live` with the picture, a `: ping` comment every fifteen seconds, and `event: stopping` when the service stops on purpose |
| `POST /event` | `type`: `message` (`chatId`, `messageId`), `briefing` (its fields) or `avatars-updated`; put into the stream once checked; `ok` |
| `GET /live` | the picture: the chat turns of the conversations and companions, the rooms whose session is folding and the message each room queued for a turn |
| `GET /health` | `ok`, `ownerId`, `bot` (`pid`, `since`), `version` |

What goes into the stream: a message the receiver kept or wrote again, a room's window opened or restarted as
`window`, a room whose registration was removed as `room-removed`, the picture whenever it changes, and what
`POST /event` says (a message or a briefing checked against the room's record first, and the profile pictures a
chat service took).

#### Health

`bmo-receiver --config <file> health` asks `GET /health` of the service the file names, at its listen address
(an unspecified host asked at the loopback one) with its token, waiting at most five seconds: it ends 0 where the
service answers with `ok` standing, and 1 otherwise, saying on its error stream why: the status it answered with,
or that nothing answered.

#### Its tools

The receiver's own tool reads the receiver's base address and token from `RECEIVER_ADDRESS` and `RECEIVER_TOKEN`
in its own environment, else from `env/receiver.env` under the record's data root; a tool placed by the host has
a clean environment. That file is written from the configuration by `bmo-receiver --config <file> tool-env`, which
every deployment runs before its flip: `RECEIVER_ADDRESS` is `http://` and the listen address, an unspecified
host answered as the loopback one, and `RECEIVER_TOKEN` the configured token. It is written under the configured
data root, readable by the user alone, and only where it would change; a value an env file cannot hold bare
(empty, or carrying whitespace, a quote, `#`, `$` or `\`) refuses it with exit code 1 and nothing written.

The tool answers the brain directly for a reminder, so it reads the brain's address and token the same way, from
`BRAIN_ADDRESS` and `BRAIN_TOKEN` or from `env/brain.env` under the record's data root; it asks agency's server for
the reset point a turn is refused on, from `AGENCY_CONTROL_PORT` and `AGENCY_CONTROL_TOKEN` or from `env/agency.env`
there, at the port agency's server answers on by default where none is named. The call protocol, the answer shape,
the session record and the manifest held to the executable are this repository's `crates/tool-call`; the sends and
the turn a call reads are shared with the `bmo` commands in `crates/room-calls`.

#### Removing a room

| Route | Body | Answer |
|---|---|---|
| `GET /rooms/remove?chat=&by=` | | what the removal would do: `goes` (each `table`, `rows`, `why` it gives up), `stays` (each kept or cleared among them), `pictures` (the files the rows point at), `settings` (`taken`, the keys a declaration matches; `undeclared`, the keys holding the room's number that none does; and `trimmed`, the declared lists of rooms it is taken out of, the console's pinned order among them), `folders` (each `what`, `path` and its `files`), `warnings` (what the receiver's own record warns of beside the removal), `warns` (every declared holder's answer), `units` (the agency's and the brain's answer, or why it did not come), `refused` (the code of a refusal that applies, where one does) |
| `POST /rooms/remove` | `chat`, `by` | what it took: `taken` (as the preview's `goes`), `cleared` (as its `stays`, with the columns named cleared), `pictures`, `settings`, `folders`, `holders`, `units`, and `couldNot` (what it could not remove, each with why) |

Both are refused 403 unless `by` names the administrator. The removal asks each declared holder, the agency
(`POST /rooms/forget`) and the brain (`POST /rooms/removed`) for their part (`docs/receiver.md`, 「Removing a
room's registration」).

#### What opened a turn

| Route | Answer |
|---|---|
| `GET /rounds/openings?chat=&rounds=` | `rounds`: for each round named, the quiet instructions it settled (sort, summary, when each was queued) without their text |
| `GET /rounds/instruction?chat=&id=` | `id`, `text`: one of those instructions whole, asked by the room it was queued for or the room whose round settled it |

#### The voice briefing

| Route | Body | Answer |
|---|---|---|
| `GET /voice-briefing` | | `on`, `autoplay`: how the switch stands, read without moving it |
| `POST /voice-briefing` | `on?`, `autoplay?` | `ok`, `on`, `autoplay` |
| `POST /voice-briefing/line` | `chatId`, `messageId` | `ok`, `state` (`making` or `ready`) |

#### Companions with a room of their own

| Route | Body | Answer |
|---|---|---|
| `POST /companions/add` | `name`, `label`, `provider`, `model`, `worker`, `room` | `companions`, `room` (`chatId`, `takenBack`), `said` |
| `POST /companions/remove` | `name`, `by?` | `companions`, `said` |

The receiver writes the companion's console room, its number there and the room's answerers; the agent's half is
the agency's, and its gate options the gate's. Adding checks the name, the label, the room and the worker here,
asks the agency to write the row (`POST /companions/add`), then the gate to copy the chat agent's gate options
for it (`POST /options/copied`), then writes the room half. Removing reads the rooms the companion answers before
it gives any up, asks the agency to take its half with those rooms and `by`, who asked where the console names
one, and then gives up the room half. A field the agency refuses comes back as the `CompanionRefusal` of that
field (`name`, `label`, `provider`, `model`, `worker`; `agent` as `not-companion`; any other as `unit-failed`),
and nothing of the receiver's is written. A gate that does not copy refuses nothing: the companion stands on the gate's defaults,
and the failure is logged.

### For the web unit's console: its rooms

#### Reading a room

| Route | Answer |
|---|---|
| `GET /console/rooms` | `rooms`: each `chatId`, `title`, `messageCount`, `lastMessageAt`, `pinned`, and `companions` where it is a room made for companions |
| `GET /console/rooms/{chat}/title` | `title`: the room's name as the room list gives it |
| `GET /console/rooms/{chat}/agent-message/{message}` | a message an agent filed, or 404 for a person's or one the archive does not hold |
| `GET /console/rooms/{chat}/messages?before=&limit=` | `messages`, oldest first, ending just before `before`, and `hasMore` |
| `GET /console/rooms/{chat}/day-start?from=` | `messageId` of the first message a person said at or after `from`, or null |
| `GET /console/rooms/{chat}/messages/after?after=&limit=` | `messages` past `after`, oldest first, and `hasMore` |
| `GET /console/rooms/{chat}/messages/{message}` | the one message as a page of one |
| `GET /console/rooms/{chat}/messages/{message}/metadata` | `metadata`: what the chat service handed over beside the message as its context's `metadata`, as it arrived, or null where it handed over none |
| `GET /console/rooms/{chat}/window` | `openFor`, the seconds the chat agent's window in the room has left, 0 where it stands closed, and `closesAt`, when it closes in Unix seconds, or null where the room has never opened one |

The owner's private chat is read by `GET /owner`. A page holds only what a person said themselves; a cursor is a
message the reader holds, and a room's line is walked by when each message was sent and the order the archive
took it in, never by its number. A message carries its files and its audio as the archive keeps them. Times on a
page and in the room's figures are epoch milliseconds, as the store keeps a message's. The judgements beside a
page are the gate's, read there.

- **The metadata is opaque to the receiver:** it keeps what the chat service handed over and hands it back
  unread, so what it holds (the custom emoji among the words, a video's shape) is the chat service's to state
  (bmo-chat's `docs/contract.md`, the context's `metadata`). A message handed over again as edited with metadata
  of its own replaces what was kept; one handed over with none leaves it.
- **The window is the chat agent's,** the agent the configuration names (「Configuration」), read off the room's
  `room_windows` row for it (`docs/wake.md`, 「The names and the window」).

#### A message's files and picture

| Route | Answer |
|---|---|
| `GET /console/rooms/{chat}/messages/{message}/files` | `files`, in the order a page's files stand: each `place` (counted from 0), `kind`, `name`, `type` (the content type its stored name implies, or null), `size` (in bytes, or null) and `kept` (whether its bytes stand in the store) |
| `GET /console/rooms/{chat}/messages/{message}/files/{place}?download=` | the bytes of the file at that place |
| `GET /console/rooms/{chat}/messages/{message}/picture` | the bytes of the picture the message stored |

- **The bytes answer as a file does over HTTP:** `Content-Type` as the file's name implies, `Content-Length`,
  `Accept-Ranges: bytes`, and a `Range` asked for answered 206 with its `Content-Range`, or 416 where it lies
  outside the file. `download=true` adds `Content-Disposition: attachment` with the file's name, so a browser
  saves it rather than opening it.
- **Only what the store keeps is served:** a file is read only where its stored path resolves inside the room's
  folder of the message-file store, and a picture inside the room's folder of the picture store, links followed
  first (`docs/room-files.md`). A message the record does not hold, a place past its files, a file whose bytes
  were not kept, a path that resolves anywhere else and a stored file gone from the disk are each answered 404
  `not-found`.

#### A room's pictures

| Route | Answer |
|---|---|
| `GET /console/rooms/{chat}/pictures?at=&before=&after=&limit=` | `pictures`, oldest first: each `messageId`, `album`, `senderName`, `sentAt`, `text` and `file` (`name`, `bytes`, `type`), null where the stored picture is gone; `hasMore`; `before`, how many of the room's pictures stand before the first one answered; `total`, how many it holds |

- **The pictures are the room's messages that stored one,** walked as its line is (by when each was sent and the
  order the archive took it in) and held to what a person said, as a page is. A stretch is asked for by a picture
  the reader holds: `at` answers that picture alone, `before` the stretch ending just before it, `after` the
  stretch just past it, and none of the three the room's newest. `limit` is taken as a page of messages takes it.
- **`hasMore`** says more pictures stand beyond the stretch on the side it was walked away from the cursor, or
  before the newest where none was named.

#### A room's figures

| Route | Answer |
|---|---|
| `GET /console/rooms/{chat}/figures?zone=` | `chatId`; `messages`, every message the room holds; `fromPeople`, those a person said; `speakers`, how many people said them; `firstAt`, `lastAt` and `lastAnswerAt`, its first and last message and an agent's last, each null where it holds none; `top`, the three people who said most, each `name` and `messages`; `byHour`, 24 counts of its messages by the hour of the day they were sent in `zone`; `agents` (below) |

- **`zone`** is an IANA time zone name, `UTC` where none is given; the database's own zone data reads it, and a
  name it does not know is refused 400 `bad-request`.
- **`agents`** holds the room's agents: those answering in it (`room_answerers`), and every agent the agency
  lists a session of for the room (`GET /session/list`): a session of the room's own scope, or one session across
  rooms that the receiver's record says has been given the room (`bmo_session_rooms`). Each is:
  - from the receiver's record: `agent`, its name; `messages`, the messages it filed in the room, those under the
    numbers its roads gave it and, for the chat agent, those under the name its own words are filed under;
    `lastAnswerAt`, when it filed the last;
  - `session`, the session it keeps for the room: its session of the room's own scope where it has one, its
    session across rooms where that has been given the room otherwise, or null;
  - from the agency's agent list (`GET /agents`), its row as the agency holds it across every room: `label`,
    `lastMode`, `lastModel`, `lastEffort`;
  - from the agency's reading of that session (`GET /session/state`): `sessionMode` (its `provider`), `model`
    and `contextTokens` (its context's `carried`), each null where no session is held;
  - `runs`, `lastRunAt`, `weekTokens` and `totalTokens`, the room's runs and what they spent, which no route of the
    agency's answers by room (「Contract gaps」), so each stands null.
- **An agency that does not answer** leaves the fields read from it null, and the figures are still answered.

#### The pinned order

| Route | Body | Answer |
|---|---|---|
| `PUT /console/pinned-rooms` | `rooms`, the room numbers in the order they stand over the rest | `rooms`, the order as it now stands |
| `POST /console/pinned-rooms/add` | `chat` | `rooms` |
| `POST /console/pinned-rooms/remove` | `chat` | `rooms` |

- **The order is the `console_pinned_rooms` setting** (`docs/store.md`), which `GET /console/rooms` stands its
  pinned rooms first by. Setting it replaces the whole list, a number named twice standing once at its first
  place. Adding stands a room last unless it stands already; removing takes it out, and a room not in the list
  is no refusal.
- **A room the record does not hold is refused** 404 `not-found`, on setting and on adding alike, and nothing is
  written. Removing a room's registration takes it out of the order (`docs/receiver.md`, 「Removing a room's
  registration」).

## Calls the receiver makes

Each call goes to the serving unit's configured base address, carrying that unit's token as a bearer (a unit that
takes none, as the cache warmer, is configured with none and called with no `Authorization` header), by the
method, path and body the serving unit's own boundary document gives it; the tables repeat them as that document
states them. Each unit is a port of the crate `receiver-ports`: a trait the receiver's code is written against and
the HTTP client that answers it.
Where the serving unit states no boundary yet, the call is stated here as the receiver makes it, for that unit's
document to match. A call is retried only where the connection was refused, within two minutes, offered every
two seconds.

### The agency

Its `docs/api.md`. The agency supplies, carries and controls; the receiver composes every turn and decides
every room's matter.

| Call | Route | Sends | Answers |
|---|---|---|---|
| the agents | `GET /agents` | | `agents`, each with its row's fields, its roads, `awake`, `brainFolder`; `roles`; the console's room figures read an agent's `label`, `lastMode`, `lastModel` and `lastEffort` from it |
| a stand-in | `GET /stand-in?agent=` | | `answers` (boolean): whether a stand-in answers the agent while the rest closes its road |
| the rest | `GET /rest` | | `resting` (boolean), `until` in epoch seconds or null, `kind` (`limit` or `outage`) or null |
| a rest written | `POST /rest` | `until` in epoch seconds, `kind` (`limit` or `outage`), `account` or null for the service's own | `silenced` (boolean): the rest is the one that closed every road that answers |
| a rest passed | `GET /rest/passed` | | `kind` (`limit` or `outage`) or null: why the deployment rested where its rest has just passed, with the passed rests cleared |
| start a session | `POST /session/start` | `agent`, `scope` (`{ "kind": "agent" }` or `{ "kind": "room", "room": <id> }`) | `id`, `agent`, `scope` |
| send a turn | `POST /session/turn` | `session`, `prompt`, and the context: `room` (`chatId`, `text`, `mark`), `via`, `basePersona`, `privileged`, `answers`, `began` | `delivery`: `taken` or `blocked` |
| read a session | `GET /session/state?session=` | | `session`, `provider`, `model`, `place`, `generation`, `context` (`carried`, the tokens its last request carried, and `window`, the registration's window), which a round's fold reads, and the console's room figures for the session a room holds |
| push words | `POST /session/push` | `session`, `words` | `said`: `delivered`, `waiting` or `refused` |
| interrupt, fold, end | `POST /session/interrupt`, `POST /session/fold`, `POST /session/end` | `session` | `ok` |
| list sessions | `GET /session/list` | | `sessions`, each `id`, `agent` and `scope`; the console's room figures read the sessions an agent keeps for a room from it |
| forget a room | `POST /rooms/forget` | `chatId`, `dryRun` | `gone`, `stayed` (each `table`, `rows`, `why`) |
| register a hook | `POST /hooks` | `point`, `url`, `token` | |
| a companion registered | `POST /companions/add` | `name`, `label`, `provider`, `model`, `worker` | `ok`; 400 `field`, `error` for a field that does not pass |
| a companion taken out | `POST /companions/remove` | `agent`, `rooms` (the chat ids of the rooms it answers), `by?` | `ok`; 400 `field`, `error` for an agent that is no companion or is seeded |
| a run's log | `GET /run-log?run=&after=` | | `lines` |
| an agent's reset point | `GET /reset-point?agent=` | | `point` in epoch milliseconds or null: the moment the agent was last reset at, which overtakes a turn begun at or before it |
| a run | `GET /runs/{id}` | | the run's row, of which the receiver reads `started_at` in epoch seconds: whether a quiet instruction handing back its result comes from before a companion's reset; 404 for a run it holds no row of |
| the events | `GET /events?kinds=` | | a stream of the kinds named |

A round's turn takes words while it works: on the round's push look, while the turn's run has started, the
words that came for the round's room since it froze are composed (`push` in `docs/bmo-window.md`) and handed to
the session's push. `delivered` joins them to the round; `waiting` gives them back to wait for the next round and
offers that turn nothing more; `refused`, or a push that failed, gives them back, and nothing more is offered
until the policy's gap after a refusal has passed (`chat-round.toml`, `[pushing]`).

The receiver reads these of an agent's row, beside the `name`, `label` and `awake` the agency states of every
agent: `kind`, `aliases` and `builtin` (how the room's roster groups and names it), `wake_words` and
`default_answerer` (the words a room calls it by, and whether it answers the rooms that name nobody),
`window_seconds` (how long a room it answered keeps it listening), `session_scope` (`unified` for the one
session shared across the rooms), `voice` (whether its answers may be spoken), `item_worker` (the item it is
working, on the queue) and `gates_off` (the gate switches turned off for it, each its name). A row holding
none of a field answers it as nothing: no window, no aliases, not unified, not spoken, no gates off.

The events read, each by the part of the receiver its effect belongs to, on one stream that names every kind
read and is opened again when it drops:

- `turn-started` and `turn-ended`: a turn's answer, taken into the room by the receiver's checks
  (`docs/answer-checks.md`); a failed companion turn's `engineError`, `{ "kind": <the engine's error kind or
  null> }` where its engine ended it with an error (`docs/wake.md`, 「A companion's round」). A round reads
  these on a stream of its own while its turn works.
- `companion-cut-off`: what waited for the companion's turns let go unread and its quiet instructions settled
  (`docs/wake.md`, 「A companion's round」).
- `companion-reset`: the receiver's own rows of the ranges it names taken (`docs/wake.md`, 「A companion's
  reset」).
- `documents-moved`: the sessions told to read documents again (「Document reload」 in `docs/bmo-window.md`).
- `fallback-answering` and `fallback-ended`: the stand-in answering kept in the `fallback_since` and
  `fallback_provider` settings and cleared, and the owner told by the chat agent which stand-in answers now, or
  that it answers on its own model again (`docs/voiced-notices.md`, 「Rest and fallback」).
- `deployment-resting`: the owner's chat hears the fixed line `rest-begins`, with `until`, for a limit's rest,
  and `provider-down` for an outage.
- `account-login-notice`: its `notice` said in the owner's chat as the agency's policy words it.
- `run-recovered`: its `fact`, `intent`, `kind` and `ttlMs` voiced in the owner's chat.
- `worker-downgraded`: the owner told by the chat agent which worker fell to which model, and whether its
  session was dropped or kept.
- `herdr-leftovers` and `machine-pressure`: told to the chat agent in the owner's chat as a fact the system
  recorded (`docs/wake.md`, 「What the agency's events wake」).
- `clock-asked` and `clock-ask-closed`: the overtime order queued for the chat agent in the owner's chat, and
  withdrawn where it was not yet handed over (`docs/voiced-notices.md`, 「A turn past its clock」).

`companion-run-going` is read by none of it: the agency stands the cancellation over a reset companion's runs
itself (its `docs/companions.md`, 「What the reset cuts off」). The agency waits on no reader, so a stream that
drops is opened again from the moment it is read again, and what it published meanwhile is heard by nobody.

### The brain

Its `docs/boundary-api.md`, camelCase `POST`s.

| Call | Route | Sends | Answers |
|---|---|---|---|
| a room's rules | `POST /room-rules/read` | `chatId` | `rules`: each `id`, `subject`, `text` |
| a rule set or revoked | `POST /room-rules/set` | `chatId`, `messageId`, `id`, `rule` (`subject`, `text`) or null | `rules` |
| set a reminder | `POST /reminders/set` | `agent`, `chatId`, `at` or `every`, `reason`, `memory?`, `user?` | a reading |
| cancel a reminder | `POST /reminders/cancel` | `id` | a reading |
| a room's reminders | `POST /reminders/list` | `chatId`, `agent` or null, `all` | `reminders`, `lines` |
| a session's loaded documents | `POST /sessions/loads`, `POST /sessions/loads/keep`, `POST /sessions/loads/forget` | `session` (and `loads`, `at` to keep) | `loads` / `ok` |
| a chat session's handoff note | `POST /session-summary/read`, `POST /session-summary/clear` | `agent` | `note` / `ok` |
| a room's person book before a fold | `POST /people/before-fold` | `chatId`, `agent` | `outcome` |
| what a turn wrote | `GET /memories?agent=&chat=`, `POST /notes/list`, `POST /watches/list` | as stated there | the rows, read for the turn's answer checks (`docs/answer-checks.md`) |
| a room forgotten or removed | `POST /rooms/forgotten`, `POST /rooms/removed` | `chatId` (and `preview`) | `ok` / `took` |

### bmo-job

Its `docs/api.md`, snake_case.

| Call | Route | Sends | Answers |
|---|---|---|---|
| a room's holdings | `GET /rooms/{chat}/holdings` | | `[holdings]` (`RoomHoldings`) |
| a room's open questions | `GET /questions/open?chat=` | | a list, the one asked last first: each `id`, `options` (in the order they were asked), `talks` (boolean: the owner may talk it over) |
| the open items | `GET /items?open=1` | | `items`: each `id`, `approved` (boolean), `status`, `title`, `worker?` (the agent assigned where one is), oldest first |
| a question settled | `POST /question` | `id`, `chosen` (the option's number, counted from zero) | a question answer |
| a question talked over | `POST /question-talk` | `id`, `say` | a question answer |
| the queue moved | `POST /queue/moved` | `detail?` | 204 |

The items a turn filed, read for its answer checks, are read through the same API (its 「Reading the queue」).

- **The owner's answer to a job's question** is read here, not in bmo-job: the receiver sees the owner's words,
  matches them against the question's options (`docs/wake.md`, and the suffixes `policy/commands.toml` names
  after an option's number) and hands bmo-job the option it chose, or the line of talk with its clock stopped.
  A talk bmo-job refuses because of the question itself, `question_absent`, `question_takes_no_talk` or
  `question_talk_settled`, is no failure to deliver: the words are taken as an ordinary message instead. Any
  other refusal, or no answer at all, fails the message as a unit's failure does.

### The gate

The gate states no boundary yet. The receiver hands it the turn's envelope and the words on their way in and
out; the gate reads its own material from the brain and from the receiver's routes for the brain above. Bodies
are snake_case, as bmo-job's calls on the gate are.

| Call | Route | Sends | Answers |
|---|---|---|---|
| a turn's reference | `POST /reference/selected` | `chat`, `message?`, `lines` (the room's recent messages in the room shape with every line the turn brought, each a string), `speakers` (the people whose words the turn answers, by number), `situation?` (`agent`, `room`: `owner` or `group`, `events`: each `kind` and `item?`, `now` in Unix seconds), `holds?` (the holds' names), `round?`, `people_spoke` (boolean), `answered?` (what the turn answered already, where it is asked again), `off` (the gates switched off for the agent), `records` (boolean: whether the turn's road records its gates) | `entries`: each `path`, `text`, `source` (`document`, `person`, `rule`, `situation`, `knowledge`, `memory`, `room` or `archive`) and `fragment?` |
| sanitise words | `POST /sanitised` | `text`, `label`, `chat?`, `message?` | `text`, `changed`; 409 `stopped` where its run was stopped |
| a room's judgement | `POST /judgements` | `chat`, `message?`, `stage`, `verdict`, `detail?`, `ms?`, `round?` | 204 |
| a companion's gate options | `POST /options/copied` | `agent` (the companion), `from` (the deployment's chat agent) | 204 |

The sanitiser is bmo-job's call as its `docs/api.md` states it, with `message` beside it so a verdict is kept
under the message it was made on. A judgement is a decision written into a room's log: a round's skip, a re-ask,
an answer sent. The road a turn walked and what it spent are published by the agency as `road-walked` and
`usage-spent`, for the units that keep them; the receiver writes neither. A companion's gate options are the
gate's: as a companion stands they are copied from the chat agent's as they are then, so the gates run for it
from its first turn and are tuned apart afterwards; the receiver, which stands the companion up, asks for the
copy.

### The web unit

The web unit states no boundary yet. Its calls are stated as bmo-job's `docs/api.md` states its own calls on
it, with the same conventions; the receiver adds the room checks and the grants.

| Call | Route | Sends | Answers |
|---|---|---|---|
| whether the model is in force | `GET /accounts/model` | | `in_force` |
| a sender's user | `GET /senders/{sender}/user` | | `user`; nothing found where none is linked |
| a user | `GET /users/{user}` | | `user`, `administrator` (boolean) |
| a user's senders | `GET /users/{user}/senders` | | `senders`: the numbers linked to the user |
| the grants in a room | `GET /grants` | `room` | `grants`: each `relation`, `subject_key` (`user:{user}` or another key), `expires_at?` in epoch milliseconds, `words?` |
| a sender's relation in a room | `POST /checks/room` | `room`, `sender?`, `relation` | `allowed`, `grant`, `user?` |
| a message's sender's relation | `POST /checks/message` | `room`, `message`, `relation` | `allowed`, `grant`, `user?`, `sender?` |
| a user's relation on an object | `POST /checks/object` | `kind`, `object`, `relation`, `user` | `allowed`, `grant` |
| make a grant | `POST /grants` | `room?` or `kind` and `object`, `relation`, `to` (`sender?`, `user?`, `members`: boolean, every member of the room), `words?`, `expires_at?`, `message` (`room`, `id`) | `grant` |
| revoke a grant | `DELETE /grants` | as a grant, without `words` and `expires_at` | `grant` |
| a fixed line's words | `POST /words/render` | `code`, its fields beside it, `language` | `text` |

The granter is the sender of the message a grant is made on, and the web unit checks that they may grant what
they give (`docs/rooms.md`, 「Delegations」). A fixed line is a code the web unit's language packs render in the
room's language, for the few lines no session can say (`docs/voiced-notices.md`, 「Lines that stay fixed」);
where the web unit cannot be reached the line goes as its code and fields, so the room is never silent about a
rest or a stop.

### The voice unit

The voice unit states no boundary yet.

| Call | Route | Sends | Answers |
|---|---|---|---|
| launch a briefing | `POST /briefings` | `chat`, `message?`, `kind` (`notice` or `summary`), `title`, `awaits_line`, `text` | `run` |
| a room's briefings | `GET /briefings?chat=&awaiting=` | | `briefings`: each `run`, `chat`, `message?` (the line the reading was asked for), `voice_message?`, `kind`, `title`, `finished` |
| a briefing's line | `POST /briefings/{run}/line` | `message` | 204 |

A briefing is spoken where the voice briefing's switch stands on (`docs/voiced-notices.md`).

### The cache warmer

Its `docs/api.md`, as 「For the cache warmer」 above states: `PUT /consumers/{consumer}`,
`PUT /consumers/{consumer}/targets/chat-touch`, `POST /consumers/{consumer}/targets/chat-touch/used`.

### The chat services

Every provider route of bmo-chat's `docs/protocol.md`, through `bmo-chat-client`: `GET /declaration`,
`GET /health`, `POST /send`, `POST /media`, `POST /dialect/read`, `POST /dialect/check`, the room routes
(`POST /room/owns`, `POST /room/identity`, `POST /room/is-owner`, `POST /room/private-of`, `POST /room/title`)
and `GET /owner-room`. The receiver holds no per-provider code: what differs between providers is read from what
each declares.

## Contract gaps

Statements the receiver makes that a sibling's document does not yet match, each for that sibling to take up:

- **The voice unit** gives no time beside a briefing it keeps, so the receiver reads a room's briefings at large
  where it wants the ones this round launched, for the voice unit to state.
- **The agency** carries no sign on a failed turn's `turn-ended` that a companion's engine ended it with an
  error, nor the error's kind, which its companion turn reads and drops; the receiver reads `engineError`, for the
  agency to publish, and tells the room nothing of an engine's error until it does.
- **The agency's `companion-reset`** names the ranges and the rooms but no run, so the receiver's run records
  (`run_records`), which name a run alone, are not taken with a companion's conversation.
- **The agency** publishes `road-walked` and `usage-spent`; the receiver writes no road and no usage rows of its
  own, and a run of a second asking that failed is written as a judgement of the round with no figures.
- **The agency's `room_windows`** and the receiver's are the same table: agency's `docs/store.md` lists it
  among the companions' sessions, and the receiver's `docs/store.md` keeps it as its own, since the window a
  room holds for an agent is what the wake reads. One of the two is to be moved before both stand in one
  database.
- **The gate, the web unit and the voice unit** state no boundary yet; their calls above are the receiver's side.
  The gate's copy of a companion's gate options, which the agency's `docs/companions.md` names the gate's, is
  among them.
- **bmo-chat's file item** carries a name, a type and the bytes, and no identifier of the file's own, so a file
  another replaced under the same name in an edited message is not told apart from it and the first bytes stand;
  nor does a hand-over say whether a sound was a spoken note or a video a round note or an animation.
- **The word chain** reads the receiver's message stream for its moves, and no document states how the receiver
  learns a message was taken as a move: until one does, a move is woken on as any message is.
- **No unit's document states how a deployment records its purpose** before the receiver stops; the receiver
  keeps what was recorded as its `restart_reason` setting (`docs/store.md`) and needs a route for it.
- **The brain's** 「The receiver」 names the operations by path alone; every one is a `POST`, as its port calls
  them.
- **The agency** answers no route that tallies an agent's runs by room: how many it ran in a room, when it last
  did, and the tokens they spent in the last seven days and in all. Its `GET /agents` row carries `runs`,
  `lastRunAt` and `contextTokens` across every room, and its run ledger lists runs by search alone, so the
  console's room figures answer `runs`, `lastRunAt`, `weekTokens` and `totalTokens` null until the agency serves a
  tally by agent and room.
