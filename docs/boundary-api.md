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

The deployment's names are read from the same file: the owner; `agent`, the registration name of the supervisor
the default answerer setting is seeded with where the store holds none (`docs/api.md`, 「Supervisors」),
whose label and wake words (`GET /supervisors`) are the words a room calls it by; the infrastructure room, where one is named; the owner's private room, `owner_room`, where
one is named in place of the room numbered as the owner's own number, a room `owner-room` below stands up on the
record alone among what it may name (`docs/receiver.md`, 「Identity, and reaching the owner」); the owner's notices room, `notices_room`, the room the system's other notices to the owner go to, never a briefing's, which goes to his private room;
the owner's private room where none is named; the language the rooms are spoken in, which
a fixed line is rendered in; the attachment size cap, the largest file a message may carry to be kept; the brain
root, where the brain's documents stand; the document tree, where the documents that ship with the agents' code
stand; the deploy roots, the folder every unit's deploy root stands in as `bmo-<unit>`, the account's
`.local/share` where it names none; the tool host's control socket, `tool_host_socket`, where it stands apart
from the one 「The tool host」 below finds; the voice engine's base address and token under `voice`, where the
deployment speaks briefings, a configuration without it speaking none; the brain's base address and token under
`brain`, where the deployment names it apart from the brain's own `env/brain.env` under the data root
(`BRAIN_ADDRESS` and `BRAIN_TOKEN`), which stands in where the configuration names none, a deployment with
neither telling no turn of a due consolidation and reporting no fold (「The brain」 below); and each chat service's
road name with its base address and token.

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
- **`quiet`** says whether work stands under way that a restart of the receiver would cut, for the testbed's wait
  before a deployment's flip (`testbed.toml`, `quiet`): it asks the agency for the runs still open and names each
  that counts, a run whose name carries one of the long-work words of `quiet.toml` however long it has run, and
  a chat turn's run, named as the receiver names its turns' runs, however long it has run too. A job counts
  only with `--with-jobs`, for a unit whose restart can cut a worker's tool calls mid-turn: then every job the
  agency answers as starting or running counts, however long it has run, each named by its role and its
  repository, with the checkout it runs in beside it. It ends 0 where none counts and 1 where some does or the
  agency did not answer, each one named on its own line.
- **`owner-room --title <words>`** stands up the owner's private room once, on the record alone, under that
  title, and prints its number for the configuration's `owner_room`: numbered as a console room is, below every room the receiver has numbered, with
  the record as its one provider, the owner and the default answerer its members and the default answerer's number there one
  of the console's own. Run again, it prints the room it stood up before and stands up nothing. It writes the
  store and never the configuration, and the room is the owner's private room once the configuration names it
  and the service starts on it.
- **`redact-kept`** reads the secret shapes (`docs/messages.md`, 「Secrets taken out」) over the rows the store
  the configuration names already holds and replaces each run in place: a message's words, what was heard in its
  voice note and the room's situation note its round read, every string of the payload kept beside it, and each
  room's own situation note. It prints, for each of those, how many rows changed and how many runs they lost, then each changed message as its
  room and number, and never a word of a row; `--dry-run` counts and lists the same and writes nothing. Run again,
  it finds nothing, since the marker is no secret's shape. It needs no running receiver, and the operator runs it
  once a receiver that reads the shapes is deployed.
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
it declares ten, each marked `home` after a tab, since the receiver, its store and the rooms it answers are
the operations machine's:

| Noun | Words | What it asks |
|---|---|---|
| `room say` | `[--chat <id>] [--by <who>] [--reply] [--reply-to <id>] [--text <words>]` | `POST /sends`, the act `say`; the words come from standard input where `--text` names none, and the room is the owner's private room where `--chat` names none; `--reply-to` names the message the line replies to; `--by` names the chat that set it off, the turn's own room where none is named |
| `room registration` | `--room <id> --by <administrator> [--mode record-only\|avatar]` | `GET /rooms/{roomId}/registration` where no mode is named, printing the mode, the providers and whether the list was chosen, whether the room is read-only and by which chat services, and the answerers; a mode named goes through `PUT /rooms/{roomId}/registration`, and the registration as it now stands is printed the same way |
| `room add` | `--provider <key> [--room-id <id> \| --contact-id <id> \| --contact-ids <id,…>] [--title <words>]` | `POST /rooms` in `record-only` mode, carrying `provider` and the one of `roomId`, `contactId` or `contactIds` named, with `title` where one is; a call naming none prints the provider's rooms and contacts by name, each with whether the receiver holds it, through `GET /providers`, `GET /providers/{key}/rooms` and `GET /providers/{key}/contacts`; the room's refusals are the receiver's own. Only the owner's word in the turn is served, and no grant names it |
| `talk` | the words of `room say`, and `[--voice <name>] [--spoken <words>]` | `POST /sends`, the act `say` where no voice is named; a voice names a trained voice, and the line is first asked as the act `speak`: outside the agent's own round that hands the voice note over as its words, and inside it the line is synthesised by the voice engine the configuration names and sent as the room's voice note, the act `voice`, the words what the note says, which a road that folds a long reading (Telegram's expandable quote) shows folded under it; `--spoken`, taken only with `--voice`, names the spoken words the voice synthesises in their place |
| `photo` | `[--chat <id>] --url <url> [--url <url> …] [--label <word> …] [--caption <words>]` | `POST /sends`, the act `photo`; the nth label is drawn into the nth picture |
| `react` | `--chat <id> --message <id> --emoji <emoji>` | `POST /sends`, the act `react`; `none` clears the agent's own |
| `pin` | `--chat <id> --message <id> [--unpin]` | `POST /sends`, the act `pin` |
| `sticker` | `--chat <id> --emoji <emoji>`, or `--list` | `POST /sends`, the act `sticker`; `--list` prints the road's own sentence about its set |
| `draw` | `draw --chat <id> --spec '<json>'`, or `verify <id>`, or `verify --seed <hex> --draw <id> --spec '<json>' [--commitment <hex>] [--result <words>]` | `draw` asks `POST /sends`, the act `random_draw`; `verify` reads the store by number, or checks the published claim without one |
| `handoff` | `handoff <agent> --reason <words>` | `POST /handovers/{agentName}`: the agent's one session hands over at its next turn, writing its handoff note and a fresh session opening on it; prints one line that the handover is asked and happens at the next turn; a refusal is the route's own sentence |

- **`handoff` puts no words into a room.** It asks for the agent's handover (`docs/api.md`, 「Handovers」), answered
  at once and running no turn, so an agent may run it on itself inside its own turn; it changes nothing of a
  room's conversation, its logs or its runs. The agent's next turn asks the old session for its handoff note,
  ends it, and opens the new session on the note (`docs/unified-bmo.md`, 「Replacing the session」).
- **A send is asked as the shell's turn:** the room, the agent whose turn the process works in, the chat that set
  it off (the turn's own room where `--by` names none), whether a job asks and the noun it came through are read
  from the environment and the words, and the running receiver carries it on its own lines.
- **A noun sends only inside its agent's own round.** Run in a shell of the room's supervisor's working round, a
  noun sends as that supervisor; run anywhere else (the operator's shell, a job, a worker, a script, a turn whose
  tool call failed over to the shell) it hands its words to the room's supervisor and sends nothing
  (`docs/messages.md`, 「Words handed to the room's supervisor」). It prints the line the receiver answered with,
  naming the agent it was handed to, and ends 0. The address and token come from the process's environment, else from the data root's `env/receiver.env`
  (「Its tools」). A spoken `talk` asks the voice engine at the address and token the configuration names.
- **`room add` is the owner's own, wherever it runs.** It answers only where the turn is the owner's, and refuses a
  job, a worker or any turn not the owner's in one sentence, since no grant in force names adding a room. A call
  naming no room, contact or contacts prints the provider's rooms and contacts instead, each by name, and writes
  nothing.
- **A spoken `talk` is two texts.** The words are the written words, what the room reads: the voice note's
  `said`, and the synthesis's `text`, which the engine keeps as its transcript; nothing changes them. The
  spoken words are what the voice says, the same words as a Korean reader says them aloud. `--spoken` hands
  them to the engine as the synthesis's `spoken`; with none given the synthesis carries the words alone and
  the engine derives the spoken words itself. Either way the engine checks them before it speaks.
- **A spoken `talk` is handed before it is made** where the shell is not in its agent's own round: the voice, the
  written words and the spoken words go to the room's supervisor as they were given, and nothing is synthesised.
- **A refusal of the engine is carried back with its sentence.** A synthesis the engine does not take in, a
  422 `unconverted-run` among them for spoken words that still hold a Latin letter, a digit, an address, a
  path or markup, is the verb's refusal, naming the status, the engine's code and its sentence; nothing is
  sent to the room.
- **A spoken `talk` follows its synthesis until it is `done` or `failed`,** however long the synthesis waits in
  the line for a card: nothing on the verb's side ends the wait or bounds a request to the engine. While it waits,
  each `waitingOn` sentence the engine gives is said once on standard error as it changes, under the synthesis's
  number. A synthesis that failed is a refusal; one done is sent as the room's voice note.
- **`--via <road>`** is taken after the noun's words as bmo-cli hands it on, and names the road a send goes out
  on where that road answers in the room.
- **Every noun ends 0** where it did what it was asked, 1 where it refused, and 2 where its words or the
  configuration could not be read. The lines it said of what it did go to standard output; a refusal goes to
  standard error.

### Its tools

- **The receiver's tools are `send` (`docs/chat-mcp.md`), `rooms` (`docs/chat-mcp.md`, 「The rooms tool」) and
  `wait` (`docs/reminders.md`),** each a folder under `tools/` with its `tool.toml`, its schemas and its
  executable, which a release's `bin/` holds.
- **Every deployment places and registers all three,** before the release goes live: each tool's executable,
  manifest, schemas and manuals are copied into the release's own placed folder for it, and that folder is put
  with the tool host's `bmo-pocket put --service receiver --placed`, allowing the capabilities its entries and
  their bundles ask for. So the tool host serves the entries of the release that is live, and an entry a release
  no longer holds is gone from the catalogue with it.
- **The send tool finds the receiver** as `docs/chat-mcp.md`, 「How a call runs」, states, and the rooms tool asks
  it the same way; the wait tool calls no unit.

## Calls the receiver makes

Each call goes to the serving unit's configured base address, carrying that unit's token as a bearer, by the
method, path and body the serving unit's own boundary document gives it. Every unit the receiver calls takes a
token, the cache warmer included, and the receiver is configured with each. Each unit is a port of the crate
`receiver-ports`: a trait the receiver's code is written against and the HTTP client that answers it. A call is
offered again only where the connection was refused, every two seconds until the unit listens, since a refusal is
the unit down or coming up; a call that reached a listener is never offered again. No request timeout cuts a call
short: it ends with its answer, a broken connection, or the peer gone, which the connection's TCP keep-alive
tells.

### The agency

Its `docs/api.md`. The agency supplies, carries and controls; the receiver composes every turn and decides every
room's matter.

| Call | Route | Sends | Answers |
|---|---|---|---|
| the supervisors | `GET /supervisors` | | `items`, every supervisor with its row's fields — its `label`, `aliases`, `sessionScope`, `turnShape`, `defaultAnswerer`, `repository`, `voice`, and the `provider`, `model` and `effort` its last turn went out on — and `more`; every supervisor listed stands, and is read as awake; kept for a short span (`docs/receiver.md`, 「What is kept for a moment」) |
| a supervisor | `GET /supervisors/{supervisorName}` | | the supervisor's row as the list answers it, of which the receiver reads `seeded`, whether the deployment seeds it; 404 where no supervisor has the name, read as none. Asked afresh, never kept, before a supervisor is taken out (`docs/api.md`, 「Supervisors」) |
| a supervisor's reset point | `GET /supervisors/{supervisorName}/reset-point` | | `supervisor` and `resetPoint`, the moment in epoch milliseconds its last reset cut its turns off at, or null where it was never reset; 404 where no supervisor has the name, read as none. Asked afresh, never kept, by every send for its agent's reset point (`docs/wake.md`, 「An agent's reset」) |
| the roles | `GET /roles` | | `roles`, each its `name`, `label`, `description` and `bundles` (each its `name`), which the owner's roster and a role's send read |
| the repositories | `GET /repositories` | | `items`, each its `name` and `label`, which a supervisor added on the console is checked against |
| a stand-in | `GET /stand-in?agent=` | | `answers`, whether a stand-in answers the agent while the rest closes its road |
| the rest | `GET /rest` | | `resting`, `until`, `kind` |
| a rest written | `PATCH /rest` | `until`, `kind`, `account` | `silenced` |
| a rest passed | `PATCH /rest` | `clearPassed` | `kind` |
| start a session | `POST /sessions` | `agentName`, `scope`, `agent` or `room`, and `roomId` for a room's | the session's `id`, `agentName`, `scope`, `roomId` |
| send a turn | `POST /sessions/{sessionId}/turns` | `prompt`, the context: the room's block, the answers, the privilege and `round`, the name of the round the turn belongs to; no base persona, which the agency takes from the agent's brain | `id`, `delivery`; 409 `session-busy` where the session already carries a turn, which the receiver waits out on the events and asks again |
| read a session | `GET /sessions/{sessionId}` | | `id`, `agentName`, `scope`, `provider`, `model`, `place`, `generation`, `context` (what the session carries: `carried` against its `window`, which the fold is judged from, and the turn's token counts), `foldRatio`, `foldsItself`, whether the session's program carries its own fold, in which case the receiver folds it never (`docs/wake.md`), `supervisor` or `role`, the row the session runs for, the other null, and `prefixChangeReason` where it names why the session's prefix changed, which tells the receiver whether the session's head moved (`docs/unified-bmo.md`, 「Replacing the session」) |
| push words | `POST /sessions/{sessionId}/turns/{turnId}/inputs` | `words` | `said` |
| interrupt a turn | `PATCH /sessions/{sessionId}/turns/{turnId}` | `state`, the turn is `interrupted` | the turn |
| fold a session | `POST /sessions/{sessionId}/folds` | | `sessionId`, `at` |
| end a session | `DELETE /sessions/{sessionId}` | | nothing |
| ask a slot of the double's role | `POST /slot-requests` | `requestKey`, `owner`, `source`, `title`, `askedBy`, the supervisor whose round it is, and `path`, that supervisor's scratch workspace | the request: its `token`, `state` and `role` |
| a slot's admission | `PATCH /slot-requests/{token}` | nothing, asked again at a steady pace until the request is `active` | `request`, the request as it stands |
| start the double's run | `POST /sessions` | `scope` `role`, `agentName`, the slot's role, `slot`, `cwd` and `supervisor` | the session's `id` |
| bind a slot to its run | `PATCH /slot-requests/{token}` | `state` `bound`, `childId` | the request |
| a turn of the double | `POST /sessions/{sessionId}/turns` | `prompt`, `resume` `auto`, `name`, `title`, `chat`, `hearsWords` and the turn's `variables` | answered once the run ends: `answer`, `outcome`, `reason` |
| give a slot back | `PATCH /slot-requests/{token}` | `state` `finished`, `outcome`, `cancel` | the request |
| list sessions | `GET /sessions` | | `items`, each a session's `id`, `agentName`, `scope` — `agent`, `room`, `role` or `conversation` — and `roomId` |
| forget a room | `DELETE /room-holdings/{roomId}?dryRun=` | | `gone`, `stayed` for a dry run, nothing otherwise |
| register a hook | `PUT /hooks/{point}` | `url`, the receiver's own `POST /hooks/{point}` at the address it listens on; `token`, the receiver's | the registration |
| a supervisor registered | `POST /supervisors` | `name`, `label`, `repository` where the console names one, and its order as the console hands it — `order` whole with `quotaShare`, or `provider` and `model`, or both | 201, `supervisor` and `brain`. A field the agency refuses names itself under `field`, an order's member as `order[<n>]`; a name it holds already is 409 `name-taken` |
| a supervisor's registration ended | `DELETE /supervisors/{supervisorName}?rooms=&forgetBrain=&force=` | `rooms`, the room numbers that name it, joined by commas | 204; 409 `seeded` and `running`. A seeded supervisor, and the one the default answerer setting names, is never asked to be taken out: the receiver reads `seeded` off the supervisor first and refuses the console itself |
| a supervisor started afresh | `POST /supervisors/{supervisorName}/resets` | `conversation`, `person`, `dryRun`, `by`, and `alone` and `shared`, the rooms that name it alone and beside another | the reset's counts as the agency answers them, handed to the console as they came |
| a run's log | `GET /log-lines?run=&after=` | | `items` |
| a session's loads | `GET /session-loads/{sessionId}` | | `sessionId`, `documents`, each `path` and `version` |
| a session's loads kept | `PUT /session-loads/{sessionId}` | `documents`, every load the session now holds | `sessionId`, `documents` |
| a session's loads forgotten | `DELETE /session-loads/{sessionId}` | | nothing |
| a run | `GET /runs/{id}` | | the run's row, its `startedAt` in epoch milliseconds |
| the runs under way | `GET /runs?live=true&limit=500` | | `items`, the runs still open, newest first, each its `id`, `name` and `startedAt`, which `quiet` reads |
| the jobs under way | `GET /jobs?running=true&limit=500` | | `items`, the jobs starting or running, newest first, each its `id`, `roleName`, the role it runs as, `repositoryName`, the repository it runs in, or null for a checkout no repository holds any longer, `workerName`, the checkout of it the job runs in, `itemId` and `startedAt`, which `quiet --with-jobs` reads |
| a round's runs | `GET /runs?round=` | | `items`, the runs the round's turns opened, each its `id`, newest first; the round's last run is the one with the highest id, whatever the order |
| an answer attached to its run | `PATCH /runs/{runId}` | `messageId` alone | the run |
| the events | `GET /events?kinds=` | | a stream of the kinds named, each event its number (`id`), `kind`, the moment it was published (`at`) and its `data` |
| the events kept | `GET /events?after=&limit=1000` with `Accept: application/json` | | `items`, the events the agency still keeps after that number, each as the stream carries it, oldest first, and `more` |
| a turn read | `GET /sessions/{sessionId}/turns/{turnId}` | | `state` (`running` or `ended`), `outcome`, `answer`; `unknown-turn` where the session holds no such turn |

The events read, each by the part of the receiver its effect belongs to, on one stream that names every kind read
and is opened again when it drops: `turn.created`, `turn.ended`, `supervisor-reset.created`,
`document-move.created`, `fallback.changed`, `rest.changed`, `account.changed`, `worker.changed`,
`overtime-request.created`, `overtime-request.changed`, `road.changed` and `session.deleted`. Each time it opens
the receiver reads the events kept since the last one it heard and hands those it has not (`docs/wake.md`, 「What
the agency's events wake」); a round whose own stream ended before its turn did reads the turn instead. The
agency's numbers only rise, across its restarts too, and it keeps its newest events in its store; a page asked
after a number it no longer keeps answers `missed`, and the receiver then hands every event it keeps as new.

Of the events whose shape the receiver reads:

- **`road.changed`** names its row as `supervisor` or `role`, the other null, beside its `fact` and what the fact
  carries.
- **`worker.changed`** names `repository`, the repository the job's run ran in, or null for a checkout no
  repository holds any longer, `worker`, the checkout of it the run ran in, and `role`, the role it ran as,
  beside the models and the job.
- **`supervisor-reset.created`** names `agent` and either `point`, the moment the reset cut the agent off, which the
  receiver keeps as the agent's reset point (`docs/store.md`, 「The settings」), or the ranges the reset took. The
  point it names is the live signal; the agency's reset-point read is the point itself, read again by every ask
  it judges, so a cut-off the receiver never heard still counts.

A turn of a round names the round in its body, and the agency opens the turn's run in its ledger under that name.
Once the round's answer is filed in the room, the receiver asks for the round's runs and attaches the answer, as
the room keeps it (under the number its road gave it, the record's own where the road gave none), to the newest
of them, so the message reads back the run that wrote it. A round that opened no run is attached to nothing, and
one whose runs cannot be read or whose attach is refused says so in the journal.

A session's loads are the documents it was handed in full, each the `path` and the `version` it read. A keep
replaces the whole list, so the receiver writes the session's loads it reads back with the turn's new ones laid
over them by path.

### The brain

Its `docs/boundary-api.md`, in camelCase and behind its token, at the `brain` the configuration names or the
address its `env/brain.env` gives. The receiver reaches it for the two moments the brain's consolidation answers
to (the brain's `docs/design.md`, 「Consolidation」) and for nothing the agent reads or writes itself: what the
agent remembers it writes with its own brain tool, and what a turn is handed of the brain is the gate's choice
(「The gate」 below).

| Call | Route | Sends | Answers |
|---|---|---|---|
| the agent's brain | `GET /brains?agentKey=&limit=2` | the turn's agent | `items`, of which the receiver reads `id`: the one brain registered for the agent; none, or more than one, is read as no brain, and the calls below are not made |
| whether a consolidation is due | `GET /brains/{brainId}/consolidation` | | `unconsolidatedImportance`, `consolidationDue`, `threshold` and `unconsolidatedCount`, of which the receiver reads `consolidationDue`; a reading with no effect on the brain |
| a fold reported | `POST /brains/{brainId}/folds` | `sessionKey`, the agency's id of the session folded | `clearedCount` |

- **The agent's brain is kept for the process,** by the agent's name, once a lookup has found it: the next
  call for the agent uses the kept id and asks no lookup. A call on a kept id that the brain answers 404 lets
  the id go, so the next call for the agent looks it up again; a lookup that finds no brain keeps nothing.
- **Due:** when the room's block goes in full at a session's opening on that room, the window asks it for the
  agent, and where the brain answers `consolidationDue` true the block's index carries the policy's due line
  (`docs/bmo-window.md`, 「What every turn carries」). A call that fails leaves the line out and says why in the
  journal.
- **The fold:** once the agency answers a fold the receiver asked of a session (`docs/wake.md`, 「The room's queue
  and the round」), the receiver reports it for the session's agent, so the marks the brain keeps of what it
  handed that session clear. The receiver reports the same fold when it releases a generation the agency opened
  under a session (`docs/unified-bmo.md`, 「Replacing the session」): the marks of the generation that ended clear
  with it. A report that fails is said in the journal and changes nothing of the fold.
- **The fold's key is the agency's id of the session.** A session's turns run under names of their own: the
  agency names each turn of a chat session's run `chat:<room>:<moment>`, new with every turn, and hands it to
  the turn's tools as `BMO_RUN_NAME`, which the brain's tool reports its own `brain_fold` under. No run's name
  outlives its turn, so none names the session a fold ends; the session's id is the one key that stays the same
  across its turns.
- **Offered once.** A call on the brain is never offered again, a refused connection included: neither the due
  line nor a fold's report is worth holding a turn or a fold for.

What the brain reads of the receiver it reads over this service's own routes (`docs/api.md`), and its documents
stand under the brain root a document reload reads.

### bmo-job

Its `docs/api.md`, snake_case. The receiver reads a room's holdings and the items a turn filed, reads and answers
the questions of a job the owner answers in his chat, and tells the queue it moved.

### The gate

Its `docs/api.md` and `docs/boundary-api.md`, in the common format: camelCase keys and epoch milliseconds. The
receiver hands it the turn's envelope and the words on their way in and out, every body in the gate's own shape:

| Call | Route | Sends | Answers |
|---|---|---|---|
| a turn's reference | `POST /reference/selected` | `roomId`, `messageId?`, `lines`, `speakerIds`, `situation?` (`agentName`, `roomKind`, `events` each `kind` and `itemId?`, `nowAt`), `holds?`, `round?`, `peopleSpoke`, `answered?`, `records` | `entries`, each `path`, `text`, `source` and `holds?`: what attaches with no pick, which the gate hands the brain to count; the receiver puts each in the turn whole, as the turn's core |
| a room's index | `GET /reference/index?room=&session=&agent=` | | `groups`, each `group` (`system-documents`, `room-rules`, `knowledge`, `people`, `room-memories`), `unread`, the entries the session's loads do not hold, `hint` or null, and `entries`, each `slug`, `holds`, `path` and `memoryId?`; the receiver reads `group`, `unread` and `hint` into the index the room's block carries when the session opens, and an entry's `slug`, `holds`, `path` and `memoryId` into a round's reads (`docs/drafting.md`, 「The round log」) |
| a judgement | `POST /judgements` | `roomId`, `messageId?`, `gateName`, `verdict`, `detail?`, `ms?`, `round?` | 201, the judgement as written |
| a round's judgements attached | `POST /judgements/attached` | `roomId`, `messageId`, `since` in epoch milliseconds | 204 |

A round's verdicts stand under the answer that ends it. Once the answer is sent the receiver asks the attach with
`since` the moment the round began, before it took the room's words and asked for their reference, and with
`messageId` the answer as the room keeps it: on the record its number there, and on a chat service the copy the
record keeps beside that service's own send. The verdict of the send itself is written under the same message.
| a new supervisor's options copied from the default answerer's | `POST /options/copied` | `agentName`, `fromAgentName` | 204 |

### The deployments

The receiver calls no unit to learn what was deployed: it reads each unit's deployment record, `deployed.json`
under the unit's deploy root (the testbed's `docs/deploy.md`, 「Deployment record」), for the deployment briefing
(`docs/wake.md`, 「Quiet instructions」). A unit is named by its deploy root's folder past `bmo-`. Of a record it
reads `at`, the moment it went live, as an RFC 3339 time or epoch milliseconds; the deploy message, `message`
where the record carries one, else `summary`, else `subject`; the commit, its first twelve characters; and the
version, `version` where the record carries one, else the `VERSION` file of the release the deploy root's `deploy`
link stands on, else that commit. A folder with no record, or a record with no time that reads, is no deployment.

### The tool host

bmo-pocket's `docs/api.md`, 「The control socket」. The receiver reaches it over HTTP on its control socket — the
path the configuration's `tool_host_socket` names, else `BMO_MCP_HOST_SOCKET`, else `bmo/mcp-host.sock` under the
user's runtime directory — with the issuer token the host keeps in `tool-host/issuer-token` under the data root,
read again at every connection. Where no socket can be named, nothing is heard of it.

| What | Route | Read |
|---|---|---|
| the catalogue's changes | `GET /events?kinds=tool.placed,tool.removed`, resumed with `after` | each event's `id`, `kind` and `data`: `tool`, `namespaces`, `entries` (`tool`, `reachedBy`, `summary`) and `replaced` |
| the tool sessions standing | `GET /tool-sessions`, paged by `after` | each session's `holderName` and `namespaces` |

- **The stream is heard for as long as the service runs.** It is resumed after the last event read; one answered
  `410 gone` is opened again with no cursor, since the host keeps no event of an earlier run; one that drops, or
  hears neither a frame nor the host's ping for a minute, is opened again after the policy's wait
  (`policy/tool-host.toml`). What the host published while nothing listened is heard by nobody, as the host's own
  document says.
- **A voice note is heard through bmo-ear.** The receiver issues a tool session for itself with the issuer token
  (`POST /tool-sessions`; the `bmo` namespace, the `room.tools` capability, `BMO_DATA_DIR`) and calls
  `ear/listen` with the saved recording's path (`POST /tool-calls`), keeping the session for the next call. The
  answer's `structuredContent` carries `transcript`, `language` and `duration`; a tool failure is kept as the
  message's failed hearing, never as silence.

- **Whose tools an event changes** is read off the tool sessions standing when it is heard: a supervisor of the
  agency's (`GET /supervisors`) is told of an event whose namespaces meet those of a
  session the host holds for it (`docs/wake.md`, 「Quiet instructions」). An agent with no session standing is told
  nothing; its next session's record shows it the host's tools as they are.

### The web unit

Its `docs/boundary.md` and `docs/openapi.json`, in the common format: the account model, a sender's user, a user
and its senders, the grants in a room, a sender's relation in a room, a message's sender's relation, a user's
relation on an object, making and revoking a grant, a fixed line's words, and the rooms the console has open:
`GET /watching` answers `rooms` and the moment the console last reported, which the restart's backlog reads before
it serves an open room. The port reads the model as
`inForce`, a grant's holder as `subjectKey` and a grant's lapse as `expiresAt`, and sends a grant's lapse as
`expiresAt`.

### The voice engine

Its `docs/boundary-api.md` and `docs/openapi.json`, in camelCase and behind its token, at the `voice` the
configuration names: `POST /briefings` with `voiceName`, `text`, `chatId`, `messageId`, `kind`, `title?`,
`emotion?` and `plays`, a notice turn's summary or a line's full reading, its `text` the written words and no
`spoken`, so the engine derives the spoken words; `POST /syntheses` with `voiceName`, `text` and `spoken?`,
then `GET /syntheses/{id}` and its `sound`, a spoken `talk`; and `GET /briefings` with `chatId`,
`messageId` and `kind`, the page a line's full reading is looked up in. The voice an agent speaks in is the
`voice` its agency registration names. The receiver reads `id`, `state` and `kind` of a briefing, and nothing of
its sound: listeners read that, and the briefing's events, from the engine.

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
and room for a short span (`docs/receiver.md`, 「What is kept for a moment」), except a room the record holds
alone, of which no service is asked (`docs/receiver.md`, 「A room's chat providers」); a room added reads its
title there afresh. Whether a service reads a room in the owner's account alone is read off that answer's `modes`, and where
the answer states none, off the service's declared modes and the answer's `identity` (`docs/receiver.md`,
「A read-only room」).

A message kept in a room goes to each other chat service on the room's list of providers as a `POST /sends` of
its words or its files (`docs/receiver.md`, 「A room's chat providers」): `mode` `human-reader` where the owner said
it and none otherwise, a `tag` naming the delivery, and `replyToMessageId` only where the message it answers stands
on that service under the service's own number. A hand-over whose `context.tag` names a delivery to that service,
or whose number is the one that service gave a delivery, is answered as held and kept no second time.

## Contract gaps

Statements the receiver makes that a sibling's document does not yet match, each for that sibling to take up:

- **The web unit's console** adds a supervisor in two asks, the receiver's add on the order's first entry and then
  the whole order on the agency's supervisor, and its language pack holds no wording for the add's refusals
  `order` and `quota-share`, until it hands the order whole to the receiver's one add (`docs/api.md`,
  「Supervisors」).
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
  what was recorded as its `restart_reason` setting, and where none was, reads its own deployment record
  (「The deployments」) at start-up for the purpose and the commit.
- **The draft process table** is keyed by provider, tier and agent, and the agency names no tier: a session
  runs for a supervisor or a role. A turn matches only rows of any tier (`docs/drafting.md`, 「The process
  table」), and the console's routes keep the tier in the key until the web unit takes it out with the receiver.
- **The gate's `draft` stage** reads the receiver's event stream (`GET /events`) and takes every frame whose
  data's `type` is `draft` as one row of a round's draft process, with the fields the receiver's
  `docs/receiver.md`, 「The event stream」, names: `chatId`, `round`, `agentName`, `number` (from 1), `question`,
  `kind`, `answerChars`, `changed`, `outcome`, `detail`, `ms`, `at`, `reads`, `session` and `loaded`, the last
  three of which the gate does not read. A `draft` event of kind `start` is no row:
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
- **The gate's reference entries** name no `holds` on a piece of the turn's core and no `memoryId` on an entry of
  its index. The receiver reads either where the gate gives it; until the gate states them, a core piece the index
  does not list is named by the receiver's policy word for its `source`, and a piece's memory is read off the two
  path forms the gate builds for one memory, `memories/<id>` and `memories/<room>/memory-<id>`, so a knowledge
  piece or a rule filed under a path of its own names no memory (`docs/drafting.md`, 「The round log」).
- **The testbed's deployment record** names its commit, subject, time and backup, and no version or deploy
  message. The receiver reads `message` and `version` where a record carries them and stands in the subject and
  the release's `VERSION` file where it does not (「The deployments」).
- **The brain's `docs/boundary-api.md`** does not yet state `GET /brains/{brainId}/consolidation`; the receiver
  reads it as 「The brain」 above states, `consolidationDue` among `unconsolidatedImportance`, `threshold` and
  `unconsolidatedCount`, until it does.
- **The brain's tool** reports its `brain_fold` under the turn's `BMO_RUN_NAME`, a name new with every turn of a
  chat session, while the receiver reports a fold under the agency's id of the session (「The brain」 above): the
  fold the agent reports at a hand-off clears the marks of no key that outlives its own turn, until the brain's
  tool reports under a name the session keeps across its turns.
- **The agency's usage by turn** is read by round: every turn of the draft process goes out naming its round, as
  the round's own turn does, so the agency's ledger opens each under the round's name. No document of the agency
  states a per-turn usage record the console reads beside a round's draft rows yet.
