# The receiver's boundary

The receiver's fixed API — every route it serves, in the one common wire format — is `docs/api.md`, and its machine
form is `docs/openapi.json`. This document states the rest of the receiver's boundary: what it is configured with,
the commands its binary answers, every call it makes on another unit, and the statements a sibling's document does
not yet match.

Rooms are the receiver's: the chats it receives through the chat services, their owner, the messages said in them,
the agents that answer in them, each room's rules, rights and windows, the instructions it hands an agent's session
there, and the envelope a turn goes out in. What a turn is told, whom an agent answers and whether an answer may go
into the room are decided here; the agency carries the turn, the gate chooses its reference and reads it on the way
in and out, and the brain keeps what the agents remember.

## Configuration

The receiver reads one configuration file, `settings.toml`. The file `--config` names is read first; with none
named, the file stands in the folder `BMO_RECEIVER_SETTINGS` names, else in the folder above the binary's own, a
release's root, else in the working directory. Each release links its own to the data root's `env/receiver.toml`.
A deployment that finds no such file is refused, and the unit does not start where the data root holds none.
`settings.example.toml` at the repository root is the checked-in example, naming a database and neutral names,
and it loads as it stands.

Beside its own listen address, its token and its database it names, for each unit it calls, that unit's base
address and the token that unit takes. A chat service's base address names the receiver's `/chat` prefix, where
the protocol's receiving-side routes stand.

The deployment's names are read from the same file: the owner; the chat agent's registration name, whose label
and wake words (`GET /agents`) are the words a room calls it by; the delegate's registration name, whose label a
delegation notice names; the infrastructure room, where one is named; the language the rooms are spoken in, which
a fixed line is rendered in; the attachment size cap, the largest file a message may carry to be kept; the brain
root, where the brain's documents stand; the document tree, where the documents that ship with the agents' code
stand; the deploy roots, the folder every unit's deploy root stands in as `bmo-<unit>`, the account's
`.local/share` where it names none; and each chat service's road name with its base address and token.

### The service and its commands

- **The service runs as the user unit `bmo-receiver.service`,** this repository's `deploy/bmo-receiver.service`,
  which the deployment links from the deploy tree under `~/.local/share/bmo-receiver`. It starts the deploy
  tree's `bin/bmo-receiver` with `serve`, and does not start where the data root holds no configuration.
- **`serve`** runs the migrations, stands the receiver on the configuration, settles the start (the data root's
  folders made, the cache warmer told of the consumer and its one target, the agency told of the three hooks it
  answers), keeps the clocks and answers the routes on the configured address until the process is told to stop.
- **`routes`** prints every route the receiver answers, one method and path to a line, sorted; the table is what
  the router is built from.
- **`deliver-waiting`** runs one pass over the instruction rows a job's end left waiting, with no clock of the
  receiver's.
- **`tool-env`** writes where the receiver's tools find it and ends.
- **`health`** asks the service the configuration names whether it is ready and ends.
- **`nouns`** prints the nouns `bmo` dispatches to this command, one a line (「The unit command」).
- **`import`** moves the old archive's exported rows into this store. It reads one export directory — a manifest
  carrying each source table's exact count and key, and one NDJSON file a table — and writes only this store,
  refusing one that already holds rows of its tables. Two path columns are rewritten on the way: `--stale-root`
  names the prefix the old archive's paths carry, and `--data-root` names the data root those files stand under
  here, whose relative layout the old paths keep. Only that prefix is rewritten, and a relative path is written
  as it stands. The import reads its arguments and never the deployment's configuration, so both roots are
  always given. It runs whole inside one transaction, so a dry run or a failed run leaves the store as it stood;
  `--dry-run` reads, checks and counts everything and writes nothing.
- **The consumer's name** the receiver registers with the cache warmer is `receiver`, unless the configuration
  names another.

### The unit command

`bmo-receiver` is the unit command bmo-cli finds (bmo-cli's `docs/api.md`, 「The unit command」). Asked `nouns`,
it declares seven, each marked `home` after a tab, since the receiver, its store and the rooms it answers are
the operations machine's:

| Noun | Words | What it asks |
|---|---|---|
| `room say` | `[--chat <id>] [--by <who>] [--reply] [--text <words>]` | `POST /sends`, the act `say`; the words come from standard input where `--text` names none, and the room is the owner's chat where `--chat` names none |
| `photo` | `[--chat <id>] --url <url> [--url <url> …] [--label <word> …] [--caption <words>]` | `POST /sends`, the act `photo`; the nth label is drawn into the nth picture |
| `react` | `--chat <id> --message <id> --emoji <emoji>` | `POST /sends`, the act `react`; `none` clears the agent's own |
| `pin` | `--chat <id> --message <id> [--unpin]` | `POST /sends`, the act `pin` |
| `sticker` | `--chat <id> --emoji <emoji>`, or `--list` | `POST /sends`, the act `sticker`; `--list` prints the road's own sentence about its set |
| `consult` | `room [<room>]`, `state`, or `say <words> [--reply <id>]`, each with `[--agent <name>]` | `room` and `state` read the store; `say` asks `POST /consult-lines` under the agent's own name |
| `draw` | `draw --chat <id> --spec '<json>'`, or `verify <id>`, or `verify --seed <hex> --draw <id> --spec '<json>' [--commitment <hex>] [--result <words>]` | `draw` asks `POST /sends`, the act `random_draw`; `verify` reads the store by number, or checks the published claim without one |

- **A send is asked as the shell's turn:** the room, the agent whose turn the process works in, the chat that set
  it off and whether a job asks are read from the environment, and the running receiver carries it on its own
  lines. The address and token come from the process's environment, else from the data root's `env/receiver.env`
  (「Its tools」).
- **`--via <road>`** is taken after the noun's words as bmo-cli hands it on, and names the road a send goes out
  on where that road answers in the room.
- **Every noun ends 0** where it did what it was asked, 1 where it refused, and 2 where its words or the
  configuration could not be read. The lines it said of what it did go to standard output; a refusal goes to
  standard error.

### Its tools

- **The receiver's tools are `send` (`docs/chat-mcp.md`) and `wait` (`docs/reminders.md`),** each a folder under
  `tools/` with its `tool.toml`, its schemas and its executable, which a release's `bin/` holds.
- **Every deployment places and registers both,** before the release goes live: each tool's executable, manifest,
  schemas and manuals are copied into the release's own placed folder for it, and that folder is put with the
  tool host's `bmo-pocket put --service receiver --placed`, allowing the capabilities its entries and their bundles
  ask for. So the tool host serves the entries of the release that is live, and an entry a release no longer
  holds is gone from the catalogue with it.
- **The send tool finds the receiver** as `docs/chat-mcp.md`, 「How a call runs」, states; the wait tool calls no unit.

## Calls the receiver makes

Each call goes to the serving unit's configured base address, carrying that unit's token as a bearer, by the
method, path and body the serving unit's own boundary document gives it. Every unit the receiver calls takes a
token, the cache warmer included, and the receiver is configured with each. Each unit is a port of the crate
`receiver-ports`: a trait the receiver's code is written against and the HTTP client that answers it. A call is
retried only where the connection was refused, within two minutes, offered every two seconds.

### The agency

Its `docs/api.md`. The agency supplies, carries and controls; the receiver composes every turn and decides every
room's matter.

| Call | Route | Sends | Answers |
|---|---|---|---|
| the agents | `GET /agents` | | `items`, the agents each with its row's fields, its roads and whether it is awake, `roles` and `more`; kept for a short span (`docs/receiver.md`, 「What is kept for a moment」) |
| a stand-in | `GET /agents/{agent}` | | `standInAvailable`, whether a stand-in answers the agent while the rest closes its road |
| the rest | `GET /rest` | | `resting`, `until`, `kind` |
| a rest written | `PATCH /rest` | `until`, `kind`, `account` | `silenced` |
| a rest passed | `PATCH /rest` | `clearPassed` | `kind` |
| start a session | `POST /sessions` | `agentName`, `scope`, `roomId` | the session's `id`, `agentName`, `scope`, `roomId` |
| send a turn | `POST /sessions/{sessionId}/turns` | `prompt`, the context: the room's block, the answers, the privilege and `round`, the name of the round the turn belongs to; no base persona, which the agency takes from the agent's brain | `id`, `delivery` |
| read a session | `GET /sessions/{sessionId}` | | `id`, `agentName`, `scope`, `provider`, `model`, `place`, `generation`, `foldRatio`, `tier` where the agency names one, which the draft process table is matched by (`docs/drafting.md`), and `prefixChangeReason` where it names why the session's prefix changed, which tells the receiver whether the session's head moved (`docs/unified-bmo.md`, 「Replacing the session」) |
| push words | `POST /sessions/{sessionId}/turns/{turnId}/inputs` | `words` | `said` |
| interrupt a turn | `PATCH /sessions/{sessionId}/turns/{turnId}` | `state`, the turn is `interrupted` | the turn |
| fold a session | `POST /sessions/{sessionId}/folds` | | `sessionId`, `at` |
| end a session | `DELETE /sessions/{sessionId}` | | nothing |
| list sessions | `GET /sessions` | | `items` |
| forget a room | `DELETE /room-holdings/{roomId}?dryRun=` | | `gone`, `stayed` for a dry run, nothing otherwise |
| register a hook | `PUT /hooks/{point}` | `url`, the receiver's own `POST /hooks/{point}` at the address it listens on; `token`, the receiver's | the registration |
| a companion registered or taken out | `POST /companions`, `DELETE /companions/{companionName}` | the companion's fields; nothing | `ok`; nothing |
| a run's log | `GET /log-lines?run=&after=` | | `items` |
| an agent's reset point | `GET /agents/{agent}` | | `resetAt` |
| a session's loads | `GET /session-loads/{sessionId}` | | `sessionId`, `documents`, each `path` and `version` |
| a session's loads kept | `PUT /session-loads/{sessionId}` | `documents`, every load the session now holds | `sessionId`, `documents` |
| a session's loads forgotten | `DELETE /session-loads/{sessionId}` | | nothing |
| a run | `GET /runs/{id}` | | the run's row, its `startedAt` in epoch milliseconds |
| a round's runs | `GET /runs?round=` | | `items`, the runs the round's turns opened, each its `id`, oldest first |
| an answer attached to its run | `PATCH /runs/{runId}` | `messageId` alone | the run |
| the events | `GET /events?kinds=` | | a stream of the kinds named |

The events read, each by the part of the receiver its effect belongs to, on one stream that names every kind read
and is opened again when it drops: `turn.created`, `turn.ended`, `companion-reset.created`,
`document-move.created`, `fallback.changed`, `rest.changed`, `account.changed`, `worker.changed`,
`overtime-request.created` and `overtime-request.changed`.

A turn of a round names the round in its body, and the agency opens the turn's run in its ledger under that name.
Once the round's answer is filed in the room, the receiver asks for the round's runs and attaches the answer, as
the room keeps it (on the record its number there, on a chat service the copy the record keeps beside that
service's own send), to the newest of them, so the message reads back the run that wrote it. A round that opened
no run is attached to nothing, and one whose runs cannot be read or whose attach is refused says so in the
journal.

A session's loads are the documents it was handed in full, each the `path` and the `version` it read. A keep
replaces the whole list, so the receiver writes the session's loads it reads back with the turn's new ones laid
over them by path.

### The brain

The receiver makes no call on the brain and holds no port to it. What the brain reads of the receiver it reads
over this service's own routes (`docs/api.md`), and its documents stand under the brain root a document reload
reads.

### bmo-job

Its `docs/api.md`, snake_case. The receiver reads a room's holdings and the items a turn filed, reads and answers
the questions of a job the owner answers in his chat, and tells the queue it moved.

### The gate

Its `docs/api.md` and `docs/boundary-api.md`, in the common format: camelCase keys and epoch milliseconds. The
receiver hands it the turn's envelope and the words on their way in and out, every body in the gate's own shape:

| Call | Route | Sends | Answers |
|---|---|---|---|
| a turn's reference | `POST /reference/selected` | `roomId`, `messageId?`, `lines`, `speakerIds`, `situation?` (`agentName`, `roomKind`, `events` each `kind` and `itemId?`, `nowAt`), `holds?`, `round?`, `peopleSpoke`, `answered?`, `offGates`, `records` | `entries`, each `path`, `text` and `source` |
| the sanitiser | `POST /sanitised` | `text`, `label`, `roomId?`, `messageId?` | `text`, `changed` |
| a judgement | `POST /judgements` | `roomId`, `messageId?`, `gateName`, `verdict`, `detail?`, `ms?`, `round?` | 201, the judgement as written |
| a round's judgements attached | `POST /judgements/attached` | `roomId`, `messageId`, `since` in epoch milliseconds | 204 |

A round's verdicts stand under the answer that ends it. Once the answer is sent the receiver asks the attach with
`since` the moment the round began, before it took the room's words and asked for their reference, and with
`messageId` the answer as the room keeps it: on the record its number there, and on a chat service the copy the
record keeps beside that service's own send. The verdict of the send itself is written under the same message.
| a companion's options copied | `POST /options/copied` | `agentName`, `fromAgentName` | 204 |

### The deployments

The receiver calls no unit to learn what was deployed: it reads each unit's deployment record, `deployed.json`
under the unit's deploy root (the testbed's `docs/deploy.md`, 「Deployment record」), for the deployment briefing
(`docs/wake.md`, 「Quiet instructions」). A unit is named by its deploy root's folder past `bmo-`. Of a record it
reads `at`, the moment it went live, as an RFC 3339 time or epoch milliseconds; the deploy message, `message`
where the record carries one, else `summary`, else `subject`; the commit, its first twelve characters; and the
version, `version` where the record carries one, else the `VERSION` file of the release the deploy root's `deploy`
link stands on, else that commit. A folder with no record, or a record with no time that reads, is no deployment.

### The web unit

Its `docs/boundary.md` and `docs/openapi.json`, in the common format: the account model, a sender's user, a user
and its senders, the grants in a room, a sender's relation in a room, a message's sender's relation, a user's
relation on an object, making and revoking a grant, and a fixed line's words. The port reads the model as
`inForce`, a grant's holder as `subjectKey` and a grant's lapse as `expiresAt`, and sends a grant's lapse as
`expiresAt`.

### The cache warmer

Its `docs/api.md`, in camelCase and behind its token: `PUT /consumers/{consumer}` with `url`, the consumer's
target-ping hook; `PUT /consumers/{consumer}/targets/chat-touch` with the target's clocks, `ttlMs` among them; and
`POST /consumers/{consumer}/targets/chat-touch/uses`, a use of the target reported with `at?` and `startedAt?`.

### The chat services

Every provider route of bmo-chat's `docs/protocol.md`, through `bmo-chat-client`: `GET /declaration`,
`GET /health`, `POST /sends`, `POST /rooms/{roomId}/messages/{messageId}/files`, `POST /dialect/readings`,
`POST /dialect/checks`, the room routes, `GET /rooms` for the rooms of a service's account, `GET /contacts` for
its contacts, `POST /rooms` to start a conversation on it, and `GET /owner`. The
receiver holds no per-provider code: what differs between providers is read from what each declares. The
declaration is asked once and kept, except where a room is added: there it is asked afresh, since a mode's
standing moves while the service runs. A room's facts are read off `GET /rooms/{roomId}`, its answer kept by road
and room for a short span (`docs/receiver.md`, 「What is kept for a moment」); a room added reads its title there
afresh. Whether a service reads a room in the owner's account alone is read off that answer's `modes`, and where
the answer states none, off the service's declared modes and the answer's `identity` (`docs/receiver.md`,
「A read-only room」).

A message kept in a room goes to each other chat service on the room's list of providers as a `POST /sends` of
its words or its files (`docs/receiver.md`, 「A room's chat providers」): `mode` `human-reader` where the owner said
it and none otherwise, a `tag` naming the delivery, and `replyToMessageId` only where the message it answers stands
on that service under the service's own number. A hand-over whose `context.tag` names a delivery to that service,
or whose number is the one that service gave a delivery, is answered as held and kept no second time.

## Contract gaps

Statements the receiver makes that a sibling's document does not yet match, each for that sibling to take up:

- **The agency's `Agent`** carries no registration field for the receiver's roster: the receiver reads `awake`,
  `kind`, `wake_words`, `window_seconds` and `default_answerer` from an agent's row, and the agency's fixed `Agent`
  states only `name`, `label`, `role`, `gates`, `reasoning`, `voice`, `resetAt` and `standInAvailable`. Until the
  agency states the roster fields, the receiver's agent port keeps its own shape.
- **The agency's session loads** state `documents` as items of no fixed shape; the receiver writes each as `path`
  and `version` and reads back only those, until the agency states the item.
- **The agency** answers no route that tallies an agent's runs by room: how many it ran in a room, when it last
  did, and the tokens they spent. The console's room figures answer `runs`, `lastRunAt`, `weekTokens` and
  `totalTokens` null until the agency serves a tally by agent and room.
- **bmo-chat's `GET /rooms/{roomId}`** answers a room's title beside its facts, and a provider may look the title
  up on its platform on every ask. The receiver reads the facts alone outside a room's addition and keeps the
  answer for a short span, until the protocol offers the facts without the title.
- **bmo-chat's message file** does not say whether a sound was a spoken note or a video, a round note or an
  animation in every case; the receiver keeps the kind where the provider gives one.
- **The word chain** reads the receiver's message stream for its moves, and no document states how the receiver
  learns a message was taken as a move: until one does, a move is woken on as any message is.
- **bmo-chat's `POST /sends`** carries no word of who said a message the receiver carries to another provider
  beyond the voice: a message of a person or of an agent other than the room's voice goes in the provider's own
  voice, under no name of its own, until the protocol states one.
- **A room the console made** is no room any chat service numbers, so a chat service put on its list refuses
  what is carried to it until that service holds the room; each refusal is said in the journal.
- **The agency's `docs/api.md`** names `turn-submitting` as its one hook point, while the agency asks the
  receiver at `room-title` (a room's title for its number, answered under `answer`, null for none), and the
  receiver registers `room-title`, `typed-line` and `road-verbs` at its own `POST /hooks/{point}`. Until the
  agency states those points, the receiver answers them as `docs/api.md`, 「Hooks」, states.
- **No unit's document states how a deployment records its purpose** before the receiver stops; the receiver keeps
  what was recorded as its `restart_reason` setting.
- **The agency's session state** names no `tier`. The receiver reads one where the state carries it and matches
  the draft process table by it (`docs/drafting.md`, 「The process table」); until the agency states it, every
  session reads as no tier and matches only rows of any tier.
- **The gate's `draft` stage** reads the receiver's event stream (`GET /events`) and takes every frame whose
  data's `type` is `draft` as one row of a round's draft process, with the fields the receiver's
  `docs/receiver.md`, 「The event stream」, names: `chatId`, `round`, `agentName`, `number` (from 1), `question`,
  `kind`, `answerChars`, `changed`, `outcome`, `detail`, `ms` and `at`. A `draft` event of kind `start` is no row:
  it tells a question as it starts, with its `number`, the process's `count` and the question's name, for the
  console's running line, and carries no `answerChars`, `changed`, `outcome` or `ms`. The gate is to let a
  `start` frame go by its `kind` before it reads a row's fields; its port reads every `draft` frame as a row and
  refuses each `start` as missing `answerChars` until it does. The receiver writes no `draft` judgement at the
  gate itself.
- **The gate's room facts** (the gate's `docs/boundary-api.md`, 「bmo-receiver」) are read through the receiver's
  fixed API in the common format, each at the route `docs/api.md` states: every room's title and senders off
  `GET /rooms` (each `Room`'s `id`, `title` and `senders`); a room's kind off `GET /rooms/{roomId}/roads`;
  `GET /rooms/{roomId}/spoke-between?person=&from=&to=`; `GET /rooms/{roomId}/owner-spoke?person=&before=&windowMs=`;
  the names people went under off `GET /people?ids=`, a page of `Person`, `id` and `name`; whether a text waits on
  a happening off `POST /promise-readings` with `text`; an agent's own number off
  `GET /rooms/{roomId}/own-number?agentName=`; the owner notice at `POST /owner/notices`, 201 with the queued row's
  `id`; and one stored message off `GET /rooms/{roomId}/messages/{messageId}`, a `Message` whose sender, name and
  moment are `senderId`, `senderName` and `sentAt`, 404 `not-found` where the receiver keeps none. The receiver
  serves no second family of these: `POST /rooms/titles`, `POST /rooms/roads`, `POST /rooms/spoke-between`,
  `POST /rooms/owner-spoke`, `POST /people/names`, `POST /promises/waits`, `POST /rooms/own-number` and
  `POST /owner/notice` answer 404 `no-route`, and the message read carries no snake_case `sent_at`. The gate's
  port asks those routes and reads those fields until it moves onto the ones above.
- **The testbed's deployment record** names its commit, subject, time and backup, and no version or deploy
  message. The receiver reads `message` and `version` where a record carries them and stands in the subject and
  the release's `VERSION` file where it does not (「The deployments」).
- **The agency's usage by turn** is read by round: every turn of the draft process goes out naming its round, as
  the round's own turn does, so the agency's ledger opens each under the round's name. No document of the agency
  states a per-turn usage record the console reads beside a round's draft rows yet.
