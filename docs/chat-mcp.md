# The send tool

The agents' tool for speaking into rooms: `send`, under `tools/send/`, with its `tool.toml`. The tool host places
it and serves its entries to every session whose role holds them; this document is the tool's part of the
catalogue and how its calls reach the receiver. The other tools of the catalogue are their units'.

## Its entries

- **`say`**: words into the turn's room, or the room `chat` names, on the turn's road or the one `via` names;
  `reply` asks for an answer back.
- **`consult`**: one line in the consult room under the calling agent's own name (`docs/consult.md`).
- **`photo`**: one picture or an album, each `url` with an optional `label`, and a `caption` (`docs/pictures.md`).
- **`remind`**: a reminder set for the turn's agent in its room, at a moment (`at`) or on a repeating rule
  (`every`), with its `reason`, and optionally the `memory` it serves and the `user` it is for
  (`docs/reminders.md`).
- **`react`**: an emoji reaction on a message, or none to clear it.
- **`pin`**: a message pinned, or unpinned.
- **`sticker`**: one of the deployment's stickers, picked by its emoji.
- **`random_draw`**: a fair draw (`docs/random-draw.md`).

Every entry is marked open-world. Its arguments are its schema, one JSON schema file beside the manifest for each
entry, and an argument the schema does not name is malformed. Each entry declares in the manifest the chat verb it
needs its road to perform, so the room's block names the sending entries a room's road carries.

## How a call runs

- **The tool runs its own executable,** as every placed tool does. It reads the turn from the call's session
  record (the run its place names, or the run's own variables) and refuses an overtaken turn there before it
  reads anything else.
- **Every entry but two asks the receiver for the send:** `POST /sends` with the entry's name as `tool`, its
  arguments as `input`, and the asker (the room, the road the turn names for it, the turn's agent, the chat that
  set it off as `by`, and whether a job asks). The receiver carries the act as that agent, on its own lines, and
  answers the lines it said, or those lines with the refusal it ended in.
- **`consult`** asks `POST /consult/say` with the turn's agent, the words and the consult room's message it
  answers; a call made outside any agent's turn is refused here, since there is no agent to speak as.
- **`remind`** posts to the brain directly: `POST /reminders/set` with the turn's agent and room, `at` or `every`,
  `reason`, `memory` and `user` as the call gives them, and answers the reading the brain gives. The brain's
  `POST /reminders/cancel` and `POST /reminders/list` are what the shell's `bmo reminder` verbs call the same way.
- **Where each unit is found:** the receiver's address and token from the call's environment
  (`RECEIVER_ADDRESS`, `RECEIVER_TOKEN`), else from `env/receiver.env` under the data root, which the receiver's
  `tool-env` verb writes as it is deployed; the brain's from `BRAIN_ADDRESS` and `BRAIN_TOKEN`, else from
  `env/brain.env`, which the brain writes the same way; agency's server, which answers the reset point a turn is
  refused on, from `AGENCY_CONTROL_PORT` and `AGENCY_CONTROL_TOKEN`, else from `env/agency.env`, the file agency's
  server takes its own environment from, at the port it answers on where none is named.
- **A send's room is the one the call names or the turn's own,** its road the turn's own for the turn's own room
  alone (a call addressed to another room goes out on that room's own road), and its sender the turn's agent, who
  sends only where it answers the room and its role gives sending.
- **Where the receiver cannot be reached,** the call fails saying so; nothing is sent twice, since a call that
  reached a listener is never offered again.
- **A call that has not answered within its manifest's limit** is ended and answered as failed. Every send returns
  once it is on its way; the one wait inside a call is a sync draw's.

## Turn context

- **What the tool fills.** The run name, the room the turn answers, the road it answers on and who set a send off
  are never typed by the agent: each call reads them for itself.
  - What the agent does type for an entry that acts is the message it acts on, `message`: the person's message in
    this turn whose words asked for it, which the tool host checks the grant of.
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
- **A sending entry called for a road that cannot carry its verb** answers with that road's own one-sentence
  refusal and sends nothing.

## The verbs beside it

The receiver's command line carries the same sends for a person, a job and a turn whose tool call failed:
`bmo room say`, `bmo room photo`, `bmo room react`, `bmo room pin`, `bmo room sticker`, `bmo consult say` and
`bmo draw draw`. Each asks the receiver the same way, from its own shell's turn, so a tool and a command cannot
send differently.
