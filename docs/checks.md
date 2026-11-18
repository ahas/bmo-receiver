# The repository checks

`tests/repository.rs` holds the checks that span the repository. It runs with the
root package's tests. Beside it the testbed's declaration enables five sweeps that read the tree before
anything runs, a line per fault each: the wording check holds every file that is not a test file to the terms
`docs/wording.md` bans; the Korean check holds each code file to the lines `korean-ledger.json` allows it, a
file the ledger does not name being allowed none; the layout check reads the README's folder table against the
folders that stand; the dependencies check reads every manifest against the base libraries the declaration
names; the personal-data check reads every tracked file for the five kinds `docs/PERSONAL-DATA.md` names.

- **Dependencies.** No package of the workspace depends on a `bmo-*` package but the base libraries,
  `bmo-chat`, `bmo-chat-client` and `bmo-ui`, nor on a crate named after a shared crate of the single
  repository this one was separated from (`archive-pool`, `archive-read`, `bmo-lang`, `bmo-markup`,
  `bmo-policy`, `bmo-tool`, `room-acts`, `seoul-clock`, `harness-providers`, `agent-host`, `test-support`).
  No package of the workspace takes one of those names either. What such a crate held that the receiver
  uses lives in a crate of this repository under a name of its own.
- **Manifests.** Only Cargo manifests stand at the root; no other build tool's manifest does.
- **Layout.** No tracked path holds a directory named after the directory it stands in.
- **The version.** `VERSION` holds the workspace's version.
- **Hangul.** No tracked code file holds a Hangul character. Korean words the code matches on, and the
  script ranges it tells them by, are data, kept under `policy/` and `tests/fixtures/`.
- **Personal data.** No tracked file holds a mail address, a home path, a telephone number, a token or a
  private network address. A finding names the file, the line and the kind, never the value; an address at a
  domain RFC 2606 and RFC 6761 reserve for documentation and the loopback address are let stand
  (`docs/PERSONAL-DATA.md`).
- **Names.** No tracked file holds an agent's, a person's or a machine's name. No repository keeps the list
  of those names, so the check reads it from the file the variable `BMO_NAMES_LIST` names, outside the
  checkout: a TOML file with `words` (matched as whole words, ignoring case), `exact_words` (whole words,
  case kept), `anywhere` (matched anywhere, as a Korean name takes its particles with no space) and
  `allowed` (terms of another unit's protocol that hold a name, set aside first). Where the variable names
  no file, the names are not checked and the run says so.

## The deployment gate

- **The testbed's deployment gate reads the checkout's own tree, as it stands, not the tree of the commit
  being deployed** (the testbed's `docs/deploy.md`, 「What the deploy run does」): its declared setup, the
  static check and the suite run in the checkout over the range from the commit live before, and only then is
  the release prepared at the commit. A checkout holding a change not yet whole therefore fails a deployment
  even where the commit's own tree passes; a deployment is handed over from a checkout that stands whole.
- **What the gate runs is the declaration's quick sweeps and suite and the testbed's own Cargo workspace
  check** (`testbed.toml`; the testbed's `docs/checks.md`, 「The static check」): the quick sweeps, then, where
  the change since the commit live before marks a crate, the workspace's format check and its lints over every
  target through the Cargo verb in the shared build directory, then the workspace suite and the client's check.
  The declaration names no Cargo step of its own, so a change that marks no crate compiles no Rust.
