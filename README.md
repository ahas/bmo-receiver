# receiver

The chat receiver, `bmo-receiver`: what becomes of a message whichever chat service hands it over. It keeps
the rooms and their messages, each room's rules, rights and windows, and the queue of words and quiet
instructions a room's agents answer. It builds the envelope a turn goes out in, hands the turn to the
agency's sessions, sends what an agent answers back into the room, and wakes the agents a room's words, a
quiet instruction or a job's end calls for. Every chat service is reached the same way, through the base
library `bmo-chat-client`; every other unit over HTTP, as `docs/boundary-api.md` states.

## Layout

| Folder | Holds |
|---|---|
| `src/` | the service's own package |
| `tests/` | the checks that span the repository, and the service's integration tests as one binary under `tests/integration/` |
| `docs/` | the canonical statements of what the service does |

At the root stand the Cargo workspace manifest, which is also the service's package, the version in
`VERSION`, `testbed.toml` (the testbed's declaration of how this repository is tested, checked, built and
deployed), `rustfmt.toml` and `CLAUDE.md`.

## Documents

| Document | States |
|---|---|
| `docs/boundary-api.md` | The boundary: every route the receiver serves, every call it makes on another unit, and every code it answers with |
| `docs/receiver.md` | What the receiver carries, the one hard rule, the owner's words, the event stream, removing a room's registration, identity and reaching the owner |
| `docs/wake.md` | Who an agent's session is woken for, the window a room keeps, waiting, replies, and the room's queue |
| `docs/bmo-window.md` | The one window every turn passes: the queue, every road in, quiet instructions, rows that ride, what a turn carries, the openings, the touch, a typed line, document reloads, events |
| `docs/unified-bmo.md` | One chat agent on one session: the room switch, the console conversation in the private chat, replacing the session, default access and delegations |
| `docs/answer-checks.md` | The checks an answer passes before it goes out |
| `docs/voiced-notices.md` | How a room is told something on the system's own account, and the few lines that stay fixed |
| `docs/rooms.md` | The per-room switches, the infrastructure room, delegations and the reply budget |
| `docs/messages.md` | Messages in and out: the archive, every kind, sends, typing, the read marker, the backlog |
| `docs/room-files.md` | A room's files: the stores, message files, who a file wakes, the tools that put a file back |
| `docs/pictures.md` | Pictures sent, received and kept, profile pictures, and whether an agent looks |
| `docs/consult.md` | Agents settling overlapping work in the consult room |
| `docs/random-draw.md` | Fair draws and their method |
| `docs/reminders.md` | Reminders as the receiver delivers them |
| `docs/chat-mcp.md` | The send tool: its entries, how a call runs, its turn context |
| `docs/policy.md` | The policy files and the keys the receiver reads |
| `docs/checks.md` | The repository checks: dependencies, manifests, layout, the version, the testbed's declaration, Hangul, home paths, private addresses and names |

## Version

- **One version for the whole repository,** the workspace's, in semver. `VERSION` holds it and the
  workspace's version equals it.
- **A major change is a change to a contract a consumer reads;** an addition no existing consumer reads is
  a minor change, anything else a patch.

## Build and test

Cargo runs through the testbed's Cargo verb, which builds in the one shared build directory:

```sh
bmo-testbed cargo check --workspace --all-targets
bmo-testbed cargo test --workspace
bmo-testbed cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```
