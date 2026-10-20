# Working in this repository

This repository is the chat receiver, `bmo-receiver`: what becomes of a message whichever chat service hands
it over. It owns the rooms and their messages, the rules, rights and windows of each room, the envelope a turn
goes out in (the room's context, the turn's scope, the queue and the quiet instructions), and the waking of the
agents a room's words, a quiet instruction or a job's end calls for. It reaches every chat service through the
base library `bmo-chat-client`, hands turns to the agency's sessions, sends the envelope to the gate, and is
read by the brain, bmo-job and the console over HTTP. The tool bound to it, `send`, is this repository's,
under `tools/`, with its `tool.toml`.

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
- **No agent names, owner names, host names, home paths or private network addresses** in code, documents
  or commits. An agent is named by its registration and the service's configuration; tests use neutral
  names such as `agent-a` and `agent-b`. No list of forbidden names is kept in the repository; the names
  check reads one from outside it (`docs/checks.md`).
- **Tests live in `tests/`.** A crate's tests are in its own `tests/`; checks that span the repository and
  the service's own tests are in the root `tests/`, the integration tests one binary under
  `tests/integration/`. No test module sits in `src/`; a private function a test needs becomes public in
  its own module.
- **No directory named after its parent** (no `x/x`), and **only Cargo manifests at the root**: the root
  `Cargo.toml` is the workspace and also the service's package.
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

The version is the workspace's, in semver, and `VERSION` holds it: a change to a contract a consumer reads
moves the major version, an addition no existing consumer reads the minor, anything else the patch. The
testbed's declaration, `testbed.toml`, carries none; it states how the testbed tests, checks, builds and
deploys the repository.
