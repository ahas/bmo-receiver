# The send tool

The agents' tool for speaking into rooms: `send`, under `tools/send/`, with its `tool.toml`. The tool host places
it and serves its entries to every session whose role holds them; this document is the tool's part of the
catalogue and how its calls reach the receiver. The receiver's other tool, `wait`, holds no send: its reminders
and watches are `docs/reminders.md`'s. The other tools of the catalogue are their units'.

## Its entries

- **`say`**: words into the turn's room, or the room `chat` names, on the turn's road or the one `via` names,
  with an optional `replyTo` naming the message in the room the line replies to;
  `reply` asks for an answer back.
- **`consult`**: one line in the consult room under the calling agent's own name (`docs/consult.md`).
- **`photo`**: one picture or an album, each `url` with an optional `label`, a `caption` (`docs/pictures.md`) and
  an optional `replyTo` naming the message it replies to.
- **`file`**: one file from this machine, or several as one album, each a `path` and an optional `name`, with a
  `caption` and an optional `replyTo` naming the message it replies to. A file a room's folder holds goes only into that room, and a job sends none of them
  (`docs/room-files.md`).
- **`voice`**: an OGG/Opus voice note at `path`, with its `caption`, what it `said`, the `transcript` read out and
  an optional `replyTo` naming the message it replies to.
- **`fixed_line`**: a fixed line's `code` and its `fields`, which the web unit's language packs render, the
  receiver's policy sentence standing in where they cannot (`docs/voiced-notices.md`, 「Lines that stay fixed」);
  the receiver says it without the output gate or any session, and refuses a code no sentence can be made of.
- **`react`**: an emoji reaction on a message, or none to clear it.
- **`pin`**: a message pinned, or unpinned.
- **`sticker`**: one of the deployment's stickers, picked by its emoji, with an optional `replyTo` naming the
  message it replies to.
- **`random_draw`**: a fair draw (`docs/random-draw.md`).

Every entry is marked open-world. Its arguments are its schema, one JSON schema file beside the manifest for each
entry, and an argument the schema does not name is malformed. Each entry declares in the manifest the chat verb it
needs its road to perform, so the room's block names the sending entries a room's road carries.

## How a call runs

- **The tool runs its own executable,** as every placed tool does. It reads the turn from the call's session
  record (the run its place names, or the run's own variables) and refuses an overtaken turn there before it
  reads anything else. The record's place is its `place` where it names one, else its `labels.place`, which is
  where the tool host's record carries it; the turn's variables are then read off that place's `run.json`.
- **Every entry but one asks the receiver for the send:** `POST /sends` with the entry's name as `tool`, its
  arguments as `input`, and the asker (the room, the road the turn names for it, the turn's agent, the chat that
  set it off as `by`, and whether a job asks). The receiver carries the act as that agent, on its own lines, and
  answers the lines it said, or those lines with the refusal it ended in.
- **`consult`** asks `POST /consult-lines` with the turn's agent, the words and the consult room's message it
  answers; a call made outside any agent's turn is refused here, since there is no agent to speak as.
- **Where each unit is found:** the receiver's address and token from the call's environment
  (`RECEIVER_ADDRESS`, `RECEIVER_TOKEN`), else from `env/receiver.env` under the data root, which the receiver's
  `tool-env` verb writes as it is deployed. The data root is the record's `dataRoot` where it names one, else the
  call's `BMO_DATA_DIR`, the variable every session the agency issues carries; the receiver's token is never a
  session variable. Agency's server, which answers the reset point a turn is refused on,
  from `AGENCY_CONTROL_PORT` and `AGENCY_CONTROL_TOKEN`, else from `env/agency.env`, the file agency's server takes
  its own environment from, at the port it answers on where none is named.
- **A send's room is the one the call names or the turn's own,** its road the turn's own for the turn's own room
  alone (a call addressed to another room goes out on that room's own road), and its sender the turn's agent, who
  sends only where it answers the room and its role gives sending.
- **Where the receiver cannot be reached,** the call fails saying so; nothing is sent twice, since a call that
  reached a listener is never offered again.
- **A call that has not answered within its manifest's limit** is ended and answered as failed. Every send returns
  once it is on its way, a draw once it is drawn and its line is out.

## Turn context

- **What the tool fills.** The run name, the room the turn answers, the road it answers on and who set a send off
  are never typed by the agent: each call reads them for itself.
  - The room is the default for every entry that takes one, `say` included: an unaddressed `say` goes to the
    turn's room, never to the owner's chat by default. An entry still takes an explicit room for the calls that
    address another: the owner's private chat, a room a watch reports to.
- **When it is read.** Every call reads the context afresh, never once when the tool starts.
- **Outside a round.** A call made while no run is going (a person typing in the pane, a keepalive touch) reads no
  room. An entry that needs a room then refuses unless it is handed one; it never falls back to the room that
  launched last or to the owner's chat.

## One list in every room

- **The tool shows the same entries in every room and on every road,** since one session serves every room and the
  tools it is shown are the front of that session's cached prefix.
- **What differs per room is said in the room's block,** where the prefix has already ended: its capability section
  names the sending entries this room's road can carry (read off each entry's declared verb and the road's
  declared capabilities, `POST /sends/can`) and leaves out the rest.
- **Where a turn answers on a road other than the room's own,** the section also names the sending entries the
  room's own road carries instead, and a sending entry takes `via` naming that road. It is the one place a call
  names a road.
- **A room on the record performs the same entries** a messenger does where they take a room: `react`, `pin` and
  `sticker` are kept by the record on its own messages, as an edit and a delete the receiver makes are
  (`docs/messages.md`, 「What an agent can send back」), so a call names no other entry for a record room.
- **A sending entry called for a road that cannot carry its verb** answers with that road's own one-sentence
  refusal and sends nothing.

## The verbs beside it

The receiver's command line carries the same sends for a person, a job and a turn whose tool call failed:
`bmo room say`, `bmo photo`, `bmo react`, `bmo pin`, `bmo sticker`, `bmo consult say` and
`bmo draw draw` (docs/boundary-api.md, 「The unit command」). Each asks the receiver the same way, from its own
shell's turn, so a tool and a command cannot send differently.
