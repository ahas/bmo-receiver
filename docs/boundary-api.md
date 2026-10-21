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
stand; and each chat service's road name with its base address and token.

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
- **The consumer's name** the receiver registers with the cache warmer is `receiver`, unless the configuration
  names another.

### The unit command

`bmo-receiver` is the unit command bmo-cli finds (bmo-cli's `docs/api.md`, 「The unit command」). Asked `nouns`,
it declares eight, each marked `home` after a tab, since the receiver, its store and the rooms it answers are
the operations machine's:

| Noun | Words | What it asks |
|---|---|---|
| `room say` | `[--chat <id>] [--by <who>] [--reply] [--text <words>]` | `POST /sends`, the act `say`; the words come from standard input where `--text` names none, and the room is the owner's chat where `--chat` names none |
| `photo` | `[--chat <id>] --url <url> [--url <url> …] [--label <word> …] [--caption <words>]` | `POST /sends`, the act `photo`; the nth label is drawn into the nth picture |
| `react` | `--chat <id> --message <id> --emoji <emoji>` | `POST /sends`, the act `react`; `none` clears the agent's own |
| `pin` | `--chat <id> --message <id> [--unpin]` | `POST /sends`, the act `pin` |
| `sticker` | `--chat <id> --emoji <emoji>`, or `--list` | `POST /sends`, the act `sticker`; `--list` prints the road's own sentence about its set |
| `reminder set` | `--chat <id> (--at <moment> \| --every <rule>) --reason <words> [--memory <id>] [--user <id>] [--agent <name>]` | `POST /sends`, the act `remind`; the agent is the one the turn names where `--agent` names none |
| `consult` | `room [<room>]`, `state`, or `say <words> [--reply <id>]`, each with `[--agent <name>]` | `room` and `state` read the store; `say` asks `POST /consult-lines` under the agent's own name |
| `draw` | `draw --chat <id> --mode sync\|async --spec '<json>'`, or `verify <id>`, or `verify --seed <hex> --draw <id> --spec '<json>' [--commitment <hex>] [--result <words>]` | `draw` asks `POST /sends`, the act `random_draw`; `verify` reads the store by number, or checks the published claim without one |

- **A send is asked as the shell's turn:** the room, the agent whose turn the process works in, the chat that set
  it off and whether a job asks are read from the environment, and the running receiver carries it on its own
  lines. The address and token come from the process's environment, else from the data root's `env/receiver.env`
  (「Its tools」).
- **`--via <road>`** is taken after the noun's words as bmo-cli hands it on, and names the road a send goes out
  on where that road answers in the room.
- **Every noun ends 0** where it did what it was asked, 1 where it refused, and 2 where its words or the
  configuration could not be read. The lines it said of what it did go to standard output; a refusal goes to
  standard error.

## Calls the receiver makes

Each call goes to the serving unit's configured base address, carrying that unit's token as a bearer (a unit that
takes none, as the cache warmer, is configured with none and called with no `Authorization` header), by the
method, path and body the serving unit's own boundary document gives it. Each unit is a port of the crate
`receiver-ports`: a trait the receiver's code is written against and the HTTP client that answers it. A call is
retried only where the connection was refused, within two minutes, offered every two seconds.

### The agency

Its `docs/api.md`. The agency supplies, carries and controls; the receiver composes every turn and decides every
room's matter.

| Call | Route | Sends | Answers |
|---|---|---|---|
| the agents | `GET /agents` | | `items`, the agents each with its row's fields, its roads and whether it is awake, `roles` and `more` |
| a stand-in | `GET /agents/{agent}` | | `standInAvailable`, whether a stand-in answers the agent while the rest closes its road |
| the rest | `GET /rest` | | `resting`, `until`, `kind` |
| a rest written | `PATCH /rest` | `until`, `kind`, `account` | `silenced` |
| a rest passed | `PATCH /rest` | `clearPassed` | `kind` |
| start a session | `POST /sessions` | `agentName`, `scope`, `roomId` | the session's `id`, `agentName`, `scope`, `roomId` |
| send a turn | `POST /sessions/{sessionId}/turns` | `prompt`, the context | `id`, `delivery` |
| read a session | `GET /sessions/{sessionId}` | | `id`, `agentName`, `scope`, `provider`, `model`, `place`, `generation` |
| push words | `POST /sessions/{sessionId}/turns/{turnId}/inputs` | `words` | `said` |
| interrupt a turn | `PATCH /sessions/{sessionId}/turns/{turnId}` | `state`, the turn is `interrupted` | the turn |
| fold a session | `POST /sessions/{sessionId}/folds` | | `sessionId`, `at` |
| end a session | `DELETE /sessions/{sessionId}` | | nothing |
| list sessions | `GET /sessions` | | `items` |
| forget a room | `DELETE /room-holdings/{roomId}?dryRun=` | | `gone`, `stayed` for a dry run, nothing otherwise |
| register a hook | `PUT /hooks/{point}` | `url`, `token` | the registration |
| a companion registered or taken out | `POST /companions`, `DELETE /companions/{companionName}` | the companion's fields; nothing | `ok`; nothing |
| a run's log | `GET /log-lines?run=&after=` | | `items` |
| an agent's reset point | `GET /agents/{agent}` | | `resetAt` |
| a run | `GET /runs/{id}` | | the run's row, its `startedAt` in epoch milliseconds |
| the events | `GET /events?kinds=` | | a stream of the kinds named |

The events read, each by the part of the receiver its effect belongs to, on one stream that names every kind read
and is opened again when it drops: `turn.created`, `turn.ended`, `companion-reset.created`,
`document-move.created`, `fallback.changed`, `rest.changed`, `account.changed`, `worker.changed`,
`overtime-request.created` and `overtime-request.changed`.

### The brain

Its `docs/boundary-api.md`, camelCase `POST`s. The receiver reads and writes a room's rules, the reminders the send
tool sets, a session's loaded documents, a chat session's handoff note, a room's person book before a fold, the
rows a turn wrote, and a room forgotten or removed.

### bmo-job

Its `docs/api.md`, snake_case. The receiver reads a room's holdings and the items a turn filed, reads and answers
the questions of a job the owner answers in his chat, and tells the queue it moved.

### The gate

Its `docs/boundary-api.md`. The receiver hands it the turn's envelope and the words on their way in and out: a
turn's reference, the sanitiser, a room's judgements, the judgements a round's answer is attached to, and a
companion's gate options copied from the chat agent's.

### The web unit

Its `docs/boundary.md` and `docs/openapi.json`, in the common format: the account model, a sender's user, a user
and its senders, the grants in a room, a sender's relation in a room, a message's sender's relation, a user's
relation on an object, making and revoking a grant, and a fixed line's words. The port reads the model as
`inForce`, a grant's holder as `subjectKey` and a grant's lapse as `expiresAt`, and sends a grant's lapse as
`expiresAt`.

### The cache warmer

Its `docs/api.md`: `PUT /consumers/{consumer}`, `PUT /consumers/{consumer}/targets/chat-touch` and
`POST /consumers/{consumer}/targets/chat-touch/used`.

### The chat services

Every provider route of bmo-chat's `docs/protocol.md`, through `bmo-chat-client`: `GET /declaration`,
`GET /health`, `POST /sends`, `POST /rooms/{roomId}/messages/{messageId}/files`, `POST /dialect/readings`,
`POST /dialect/checks`, the room routes and `GET /owner`. The receiver holds no per-provider code: what differs
between providers is read from what each declares.

## Contract gaps

Statements the receiver makes that a sibling's document does not yet match, each for that sibling to take up:

- **The agency's `Agent`** carries no registration field for the receiver's roster: the receiver reads `awake`,
  `kind`, `wake_words`, `window_seconds` and `default_answerer` from an agent's row, and the agency's fixed `Agent`
  states only `name`, `label`, `role`, `gates`, `reasoning`, `voice`, `resetAt` and `standInAvailable`. Until the
  agency states the roster fields, the receiver's agent port keeps its own shape.
- **The brain's fixed API** is generic memory, fragments and scope holdings; it serves no room rule, session load,
  handoff note or person-book route as the receiver's calls state them. Those calls stand as the receiver makes
  them until the brain states the shapes, or until the receiver's use of them is restated in the generic terms.
- **The agency** answers no route that tallies an agent's runs by room: how many it ran in a room, when it last
  did, and the tokens they spent. The console's room figures answer `runs`, `lastRunAt`, `weekTokens` and
  `totalTokens` null until the agency serves a tally by agent and room.
- **bmo-chat's message file** does not say whether a sound was a spoken note or a video, a round note or an
  animation in every case; the receiver keeps the kind where the provider gives one.
- **The word chain** reads the receiver's message stream for its moves, and no document states how the receiver
  learns a message was taken as a move: until one does, a move is woken on as any message is.
- **No unit's document states how a deployment records its purpose** before the receiver stops; the receiver keeps
  what was recorded as its `restart_reason` setting.
