# Working in this repository

This repository is the chat receiver, `bmo-receiver`: what becomes of a message whichever chat service hands
it over. It owns the rooms and their messages, the rules, rights and windows of each room, the envelope a turn
goes out in (the room's context, the turn's scope, the queue and the quiet instructions), and the waking of the
agents a room's words, a quiet instruction or a job's end calls for. It reaches every chat service through the
base library `bmo-chat-client`, hands turns to the agency's sessions, sends the envelope to the gate, and is
read by the brain, bmo-job and the console over HTTP. The tools bound to it, `send` and `wait`, are this
repository's, under `tools/`, each with its `tool.toml`.

## Rules

- **`docs/` is canonical.** Read the document covering a change before changing code. A change that would
  make it wrong or incomplete repairs it first, stating what is true now, and the code is then written to
  match. `docs/api.md` and the OpenAPI document `docs/openapi.json` state every route this service serves, in
  the one fixed wire format; `docs/boundary-api.md` states its configuration, its commands, every call it
  makes on another unit and the gaps a sibling's document has not yet matched.
- **No `bmo-*` dependency but the base libraries** `bmo-chat`, `bmo-chat-client` and `bmo-ui`. Another unit
  is reached through a port in this repository, over HTTP; a helper an old shared crate held is copied into
  a crate here. `tests/repository.rs` fails on any other `bmo-*` dependency and on the old shared crate
  names (`docs/checks.md`).
- **A base never knows its consumers.** A port, a crate or a route supplies the mechanism and a safe
  default; no code here names the unit or agent that calls it, and nothing branches on a chat service's
  name: what differs between them is asked of what each declares.
- **English only** in code, comments, strings, tests and documents, and in commit messages. Messages the
  service answers with are English sentences; what a machine reads, and every fact a room is to be told, is
  a code with parameters, which the web unit's language packs render or the agent puts in its own words.
  Korean words the code reads or matches on are data, kept in `policy/*.toml` files and under
  `tests/fixtures/`, never in `.rs` files.
- **No personal data in a tracked file.** A mail address, a home path, a telephone number, a token or a
  private network address stands in no file and no commit; `docs/PERSONAL-DATA.md` states the rule, and the
  suite's `personal_data` check fails on one.
- **No agent names, owner names or host names** in code, documents or commits. An agent is named by its
  registration and the service's configuration; tests use neutral names such as `agent-a` and `agent-b`. No
  list of forbidden names is kept in the repository; the names check reads one from outside it
  (`docs/checks.md`).
- **Tests live in `tests/`.** A crate's tests are in its own `tests/`; checks that span the repository and
  the service's own tests are in the root `tests/`, the integration tests one binary under
  `tests/integration/`. No test module sits in `src/`; a private function a test needs becomes public in
  its own module.
- **No directory named after its parent** (no `x/x`), and **only Cargo manifests at the root**: the root
  `Cargo.toml` is the workspace and also the service's package.
- **The code graph in `.graph/` is written only with `bmo graph`**: one node per feature, with its files, its
  exported names, the features it needs and its exposed or called API. `bmo graph check` says nothing where the
  graph is whole, and the graph is brought up to date in the checkout before the commit. Its area files are
  generated and untracked, so a fresh checkout builds them again, while `settings.toml` is the repository's
  configuration and stays tracked.
- **Documents state what is true now**: no change history, no quoted source and no file-and-line pins.

## Checks

A change moves through three stages in order:

1. Writing: `cargo check --workspace --all-targets`.
2. Review: read the whole diff against what the change is for and the document governing it, then
   `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all --check` (108 columns,
   2-space indentation, as `rustfmt.toml` sets).
3. Verification: the test targets the change touched, then `cargo test --workspace` once, the Cargo suite
   `testbed.toml` declares.

Cargo runs through the testbed's Cargo verb, `bmo-testbed cargo <cargo arguments…>`, which builds in the
one shared build directory; no checkout keeps a `target/` of its own.

The version is the workspace's, and `VERSION` holds it. It moves only at a release the owner declares, as
bmo-chat's `docs/conventions.md` 「Versions」 states; a commit, a deploy, a fix or a contract change moves none, and
a deployment is told apart by its commit. The testbed's declaration, `testbed.toml`, carries none; it states how the testbed tests, checks, builds and
deploys the repository.
