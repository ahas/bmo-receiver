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
