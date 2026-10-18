# The repository checks

`tests/repository.rs` holds the checks that span the repository. Each runs with the root package's tests.
Beside them the testbed's declaration enables four sweeps that read the tree before anything runs, a line per
fault each: the wording check holds every file that is not a test file to the terms `docs/wording.md` bans; the
Korean check holds each code file to the lines `korean-ledger.json` allows it, a file the ledger does not name
being allowed none; the layout check reads the README's folder table against the folders that stand; the
dependencies check reads every manifest against the base libraries the declaration names.

- **Dependencies.** No package of the workspace depends on a `bmo-*` package but the base libraries,
  `bmo-chat`, `bmo-chat-client` and `bmo-ui`, nor on a crate named after a shared crate of the single
  repository this one was separated from (`archive-pool`, `archive-read`, `bmo-lang`, `bmo-markup`,
  `bmo-policy`, `bmo-tool`, `room-acts`, `seoul-clock`, `harness-providers`, `agent-host`, `test-support`).
  No package of the workspace takes one of those names either. What such a crate held that the receiver
  uses lives in a crate of this repository under a name of its own.
- **Manifests.** Only Cargo manifests stand at the root; no other build tool's manifest does.
- **Layout.** No tracked path holds a directory named after the directory it stands in.
- **The version.** `VERSION` holds the workspace's version.
- **The testbed's declaration.** `testbed.toml` names only keys of the testbed's format, its major version,
  the Cargo suite and the three base libraries.
- **Hangul.** No tracked Rust file holds a Hangul character. Korean words the code matches on are data,
  kept under `policy/` and `tests/fixtures/`.
- **Home paths and private addresses.** No tracked file holds a path under a home directory or an address
  of a private network range.
- **Names.** No tracked file holds an agent's, a person's or a machine's name. No repository keeps the list
  of those names, so the check reads it from the file the variable `BMO_NAMES_LIST` names, outside the
  checkout: a TOML file with `words` (matched as whole words, ignoring case), `exact_words` (whole words,
  case kept), `anywhere` (matched anywhere, as a Korean name takes its particles with no space) and
  `allowed` (terms of another unit's protocol that hold a name, set aside first). Where the variable names
  no file, the names are not checked and the run says so.
