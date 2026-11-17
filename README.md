# receiver

The chat receiver, `bmo-receiver`: what becomes of a message whichever chat service hands it over. It keeps
the rooms and their messages, each room's rules, rights and windows, and the queue of words and quiet
instructions a room's agents answer. It builds the envelope a turn goes out in, hands the turn to the
agency's sessions, sends what an agent answers back into the room, and wakes the agents a room's words, a
quiet instruction or a job's end calls for. Every chat service is reached the same way, through the base
library `bmo-chat-client`; every other unit over HTTP, as `docs/api.md` and `docs/openapi.json` state.

## Layout

| Folder | Holds |
|---|---|
| `src/` | the service's own package: the receiver's judgement of a message and the wake it hands it to (`receiver/`, with the wake's own rule, its agents, the owner's answers to a job's questions, the rest's gate and who is heard under `receiver/wake/`), the round a woken turn goes through and what it is composed of (`chat/`, with the conversations, the window, the instruction pass, the room's block, the turn's prompt, the answer's shape, the thread of a person's own rooms, the records a turn wrote down and the voice of a spoken notice, the touch of a quiet room, the fold, the brain's consolidation as the receiver answers to it, the handoff a replaced session leaves, the health of the rooms, the restart and the draft process an answer is made in, with its items and checks), the roads a room is answered on (`roads/`), a room's record as its removal reads it (`room_record/`), the event stream, the live board and every screen listening to them (`events/`, with the relay's own line for a caller driving the news hard under `events/relay/`), its store, with the migrations under `src/store/migration/`, its folders under the data root, the routes the console, the send tool and the `bmo` command reach it on (`routes/`), the nouns `bmo` dispatches to it (`command`), the verbs the `bmo` command runs over its own record (`verbs/`, including the reload's telling of the chats whose documents moved), the one stream the agency's events are heard on and handed to the parts their effects belong to (`hearing`), the machine's pressure and the workers' facts told to the owner's session (`machine`, `workers`), and the readings a text is cut by (`dashes`, `web_address`, `text`, `form`, `json`, `run_lines`, `language`, `rounds`), the secret shapes a message's words are read for and the kept rows cleaned of them (`secrets`), with the configuration the service reads at start (`config`), the standing and the start's work (`service_start`), where its tools find it (`tool_env`) and the service's own binary under `src/bin/` |
| `.cargo/` | the Cargo settings of this workspace: the test thread cap the suite's runs keep to so each test's minutes stay within the limit the gate watches them by |
| `deploy/` | the service's user unit, which a deployment links from the deploy tree and restarts as the repository's one service |
| `crates/` | `receiver-client`, the fixed API's bodies as Rust types, one call for each route, and the OpenAPI document generated from them, with the TypeScript package generated from the same document in `crates/receiver-client/ts/`; `receiver-codes`, every refusal, fact and fixed line the receiver gives, as codes with structured fields; `receiver-policy`, the loader of `policy/`; `receiver-ports`, a trait and an HTTP client for each unit the receiver calls; `room-calls`, what the receiver's send tool and the `bmo` commands doing the same sends ask of the running receiver — the acts and their arguments, who asks one, the turn a call reads, a procedure read in a room, a reading held under its ceiling, and where the receiver is found from a data root; `tool-call`, the receiver's side of the tool host's call protocol, which the tools bound to it share |
| `tools/` | the tools bound to this service, each with its own `tool.toml`: `send`, the entries the agents speak rooms through, and `wait`, the reminders and watches `bmo use` runs on their own |
| `policy/` | the receiver's fixed numbers and words, the Korean words its commands are matched by among them |
| `tests/` | the package's one test harness, `main.rs`: the checks that span the repository and the service's own tests a module per file, and the integration tests the module `tests/integration/`; the words a test asks in another unit's language, kept as data under `tests/fixtures/` |
| `docs/` | the canonical statements of what the service does |

At the root stand the Cargo workspace manifest, which is also the service's package, the version in
`VERSION`, `testbed.toml` (the testbed's declaration of how this repository is tested, checked, built and
deployed), `rustfmt.toml`, `CLAUDE.md`, `settings.example.toml` (the checked-in example of the
configuration, which the deployment's own file stands under the data root) and `korean-ledger.json` (the Korean
left in a code file, which the testbed's Korean sweep holds every file to; no code file of this repository
holds any).

## Documents

| Document | States |
|---|---|
| `docs/boundary-api.md` | The boundary: every route the receiver serves, every call it makes on another unit, and every code it answers with |
| `docs/receiver.md` | What the receiver carries, the one hard rule, the owner's words, the event stream, removing a room's registration, identity and reaching the owner |
| `docs/wake.md` | Who an agent's session is woken for, the window a room keeps, waiting, replies, and the room's queue |
| `docs/bmo-window.md` | The one window every turn passes: the queue, every road in, quiet instructions, rows that ride, what a turn carries, the openings, the touch, a typed line, document reloads, events |
| `docs/unified-bmo.md` | One supervisor on one session across rooms: the room switch, the console conversation in the private chat, replacing the session, default access and delegations |
| `docs/drafting.md` | The draft process: the questions a round's turn is followed by, the table that says which, the items and checks on the final text, the round log |
| `docs/answer-checks.md` | The checks an answer passes before it goes out |
| `docs/voiced-notices.md` | How a room is told something on the system's own account, and the few lines that stay fixed |
| `docs/rooms.md` | The per-room switches, the infrastructure room, delegations and the reply budget |
| `docs/messages.md` | Messages in and out: the archive, every kind, sends, typing, the read marker, the backlog |
| `docs/room-files.md` | A room's files: the stores, message files, who a file wakes, the tools that put a file back |
| `docs/pictures.md` | Pictures sent, received and kept, profile pictures, and whether an agent looks |
| `docs/consult.md` | Agents settling overlapping work in the consult room |
| `docs/random-draw.md` | Fair draws and their method |
| `docs/reminders.md` | Reminders and watches: the wait tool, and how `bmo use` runs it and wakes the agent |
| `docs/chat-mcp.md` | The receiver's tools: the send and rooms entries, how a call runs, its turn context |
| `docs/store.md` | The receiver's database, its tables and settings keys, and its folders under the data root |
| `docs/codes.md` | Every refusal, fact, fixed line and kind the receiver gives, with its fields and status |
| `docs/policy.md` | The policy files and the keys the receiver reads |
| `docs/PERSONAL-DATA.md` | The personal data no tracked file may hold, and the check that holds it |
| `docs/checks.md` | The repository checks: dependencies, manifests, layout, the version, the testbed's declaration, Hangul, personal data and names |
| `docs/wording.md` | The terms this repository's English text bans, held to by the testbed's wording check |
| `docs/inventory.md` | Every member of the single repository this one was separated from: where it stands here, the task that carried it, and the unit that owns the ones that did not come |

## Personal data

No tracked file holds a mail address, a home path, a telephone number, a token or a private network address.
`docs/PERSONAL-DATA.md` states the rule, and the suite's `personal_data` check reads every tracked file for
it, reporting the file, the line and the kind but never the value.

## Version

- **One version for the whole repository,** the workspace's. `VERSION` holds it and the workspace's version
  equals it.
- **It moves only at a release the owner declares,** as bmo-chat's `docs/conventions.md` 「Versions」 states; a
  commit, a deploy, a fix or a contract change moves none, and a deployment is told apart by its commit.

## Build and test

Cargo runs through the testbed's Cargo verb, which builds in the one shared build directory:

```sh
bmo-testbed check quick
bmo-testbed cargo check --workspace --all-targets
bmo-testbed cargo test --workspace
bmo-testbed cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

`bmo-testbed check quick` runs the four sweeps that read the tree before anything runs: the glossary's terms,
Korean in the code, the README's folder table and the dependencies the manifests name. The names a person, an
agent or a machine goes by are checked by the repository's own test, which reads the list from outside the
checkout. The `personal_data` check reads every tracked file for a mail address, a home path, a telephone
number, a token and a private network address (`docs/PERSONAL-DATA.md`).

The tests that use a database make their own on the PostgreSQL server `BMO_TEST_POSTGRES` names, which the
testbed's suite sets; with nothing named, the machine's own socket. Each test process migrates one template,
`receiver_rs_tmpl_<process>`, and hands each test a copy of it, `receiver_rs_copy_<process>_<count>`, dropped
when the test ends: the role's own sessions on it are ended and the database dropped without forcing, so the
suite's role needs no right to end another role's process, and a test whose copy is not dropped fails. Before
a process makes its first database it drops the templates and copies its role owns whose process no longer
runs on the machine and that no client's session is on; no other name is touched.
