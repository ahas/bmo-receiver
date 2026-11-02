# Reminders and watches

A reminder wakes an agent in its room at a moment; a watch wakes it once a condition holds. Neither the receiver
nor the brain holds one. Each is a call of the wait tool, `wait`, under `tools/wait/`, that bmo-cli's `bmo use`
runs on its own (bmo-cli's `docs/api.md`, 「The tool verb」): the call waits for its moment or its condition, and
its end wakes the agent through the receiver with what the call answered and the memory the agent gave it to
carry. The receiver keeps no reminder table, serves no reminder route, and its send tool holds no reminder entry.

## The wait tool

The tool host places it and serves its one entry to every session whose role holds `room.tools`, beside the
send tool's (`docs/chat-mcp.md`). A session reaches it through the toolbook and `bmo use`, not by name.

- **`bmo/wait`**: waits `seconds`, or until the moment `until` names (a date and time with its offset from UTC,
  in RFC 3339), and answers with the `note` it was given: the note on its first line, the moment it waited until
  after it. Exactly one of `seconds` and `until` is given; a moment already past is answered at once.
- **A watch is the same entry with `check`:** it runs the shell command at once and then `every` seconds after
  each run ends (60 where none is given), and answers once a run of it ends with 0: the `note` on its first line,
  then how many runs it took and what the last run wrote, cut to its last 2,000 characters. Each run is waited on
  until it exits, however long it takes, and nothing but the check holding or the call being stopped ends a
  watch. An agent that wants to hear at a moment whether the check has held by then sets a reminder for that
  moment beside the watch. A call naming `check` with `seconds` or `until` is refused, and so is one naming
  `every` without it.
- **It is reached by `act`,** since its check is a command of the caller's where one is named. It is declared
  unbounded: the host sets no limit on a call of it. The unit `bmo use --async` runs the call in ends it as
  bmo-cli's document states. It carries a manual, which `bmo use <tool> --describe` answers whole.
- **A check runs as a call of the tool does:** in the tool's placed folder, with the tool host's clean
  environment, so it names what it reads by absolute paths.
- **The tool reads nothing of the turn.** The room and the agent the end wakes are the ones `bmo use` was given or
  read from the turn's variables, and the memory is the `--memory` it was given.

## Setting one

- **A reminder** is `bmo use bmo/wait --async` with `seconds` or `until`, the `note`, and `--memory` saying what
  the agent meant to do then. A moment further away than a day needs `--max-seconds` past it, since the unit is
  stopped at its longest run, one day unless that option names another.
- **A watch** is `bmo use bmo/wait --async` with the `check`, the `note`, and `every` as it needs.
- **A repeating reminder** is one reminder at a time: the turn its end opens sets the next.
- **Cancelling one** is stopping its unit, `bmo-use-<id>` as `bmo use --async` answered it; the stop is that
  unit's end, and its last word wakes the agent with the failure, so the agent learns the wait is gone.

## Its end wakes the agent in its room

- **The end is an instruction,** `POST /instructions` with the sort `use-ended` and the call's answer as a fact,
  for the agent in the room `bmo use` names. Queued with no kind, it opens the agent's turn there at the
  instruction pass's next tick (`docs/bmo-window.md`, 「Quiet instruction」). The agent looks at how things stand
  and writes what the room hears then, in that room's manner, or says nothing where nothing needs saying.
- **The wake is kept until it is taken:** once the instruction stands in the receiver's instruction table it
  survives a restart until the room's session takes it.
