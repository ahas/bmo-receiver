# The receiver's tools

The agents' tools bound to the receiver: `send`, under `tools/send/`, for acting in rooms, and `rooms`,
under `tools/rooms/`, for opening an agent room and adding a chat-service room (「The rooms tool」 below). The
tool host places each and serves its entries to every session whose role holds them; this document is their part
of the catalogue and how their calls reach the receiver. The receiver's other tool, `wait`, holds no send: its
reminders and watches are `docs/reminders.md`'s. The other tools of the catalogue are their units'.

## Its entries

No entry puts words in a room. An agent's words reach a room only as its round's message, which its message turn
writes and its instruction stages check (`docs/instruction.md`); an entry that holds words is refused in the round's own room and handed to
the supervisor of any other (`docs/messages.md`, 「Words handed to the room's supervisor」).

- **`say`**: words for another room, the room `chat` names, handed to that room's supervisor, whose round there
  writes and reviews them, on the road `via` names where given, with an optional `replyTo` naming the message
  there the line is to reply to; `reply` asks for that room's answer back. Asked for the round's own room it is
  refused: the words go into the round's answer.
- **`photo`**: one picture or an album, each `url` with an optional `label` (`docs/pictures.md`), and an optional
  `replyTo` naming the message it replies to. A `caption` is words, refused in the round's own room.
- **`file`**: one file from this machine, or several as one album, each a `path` and an optional `name`, with an
  optional `replyTo` naming the message it replies to; a `caption` is words, as for `photo`. A file a room's folder holds goes only
  into that room, and a job sends none of them (`docs/room-files.md`). It is for a file a room saves or opens;
  a picture a room is to look at goes through `photo`, and the entry's description says so.
- **`voice`**: an OGG/Opus voice note at `path`, with its `caption`, what it `said`, the `transcript` read out and
  an optional `replyTo` naming the message it replies to. A voice note is words, so it is refused in the round's
  own room and handed anywhere else.
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
  record (the run its place names, or the run's own variables). The record's place is its `place` where it names one, else its `labels.place`, which is
  where the tool host's record carries it; the turn's variables are then read off that place's `run.json`.
- **Every entry asks the receiver for the send:** `POST /sends` with the entry's name as `tool`, its arguments as
  `input`, and the asker (the room, the road the turn names for it, the turn's agent, the moment the turn took its
  words as `began`, the chat that set it off as `by`, and whether a job asks). Where that agent's round is working
  in the room that set it off and the act holds no words, the receiver carries the act as that agent, on its own
  lines, and answers the lines it said, or those lines with the refusal it ended in; an act holding words asked
  there into that room is refused `words-in-answer`; anything else (words for another room, a job's call, a
  worker's, a call made outside any round) it hands to the room's supervisor and answers the line saying so
  (`docs/messages.md`, 「Words handed to the room's supervisor」).
- **An overtaken turn sends nothing.** The receiver reads the agent's reset point from the agency's reset-point read as
  each send asks, beside the one it keeps from the agency's cut-off (`docs/wake.md`, 「An agent's
  reset」), and refuses a send whose `began` is at or before that point, before anything else is
  read, the refusal telling the turn to end without another call.
- **Where each unit is found:** the receiver's address and token from the call's environment
  (`RECEIVER_ADDRESS`, `RECEIVER_TOKEN`), else from `env/receiver.env` under the data root, which the receiver's
  `tool-env` verb writes as it is deployed. The data root is the record's `dataRoot` where it names one, else the
  call's `BMO_DATA_DIR`, the variable every session the agency issues carries; the receiver's token is never a
  session variable. The tool reaches no other unit.
- **A send's room is the one the call names or the turn's own,** its road the turn's own for the turn's own room
  alone (a call addressed to another room goes out on that room's own road), and its sender the turn's agent, who
  sends only where it answers the room and its role gives sending.
- **Where the receiver cannot be reached,** the call fails saying so; nothing is sent twice, since a call that
  reached a listener is never offered again.
- **A call waits for the receiver's answer within its act's bound.** Every entry is declared unbounded, so the tool
  host sets no limit on a call; the call to the receiver carries a bound its act gives it, and ends with the
  answer, the connection failing or the bound passing, never waiting on past it. A connection is given five
  seconds to open. A line, a spoken talk and a question of what a road performs are handed on or refused and
  answered at once, so the receiver is given a minute for them; a reaction, a pin,
  a sticker and a fair draw go to the road within the call, three minutes; a picture, a file and a
  voice note carry their bytes there, ten minutes. A call whose bound passed says so, and that what it asked may
  still be done, so the asker looks before asking it again. Every send returns once it is on its way, a draw once
  it is drawn and its line is out.

## Turn context

- **What the tool fills.** The run name, the room the turn answers, the road it answers on and who set a send off
  are never typed by the agent: each call reads them for itself.
  - The room is the default for every entry that takes one: an unaddressed entry acts in the turn's room, never
    in the owner's chat by default, and an unaddressed `say` is refused as words in the round's own room. An entry
    still takes an explicit room for the calls that address another: the owner's private chat, a room a watch
    reports to.
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

The receiver's command line carries the same sends: `bmo room say`, `bmo talk`, `bmo photo`, `bmo react`,
`bmo pin`, `bmo sticker` and `bmo draw draw` (docs/boundary-api.md, 「The unit command」).
Each asks the receiver the same way, from its own shell's turn, so a tool and a command cannot send differently:
in a shell of the supervisor's own working round a command carries what holds no words as that supervisor and is
refused words for that round's room, and for words for another room, a person, a job, a script or a turn whose
tool call failed it hands the words to the room's supervisor, who writes them as its own round's message.

## The rooms tool

- **`rooms`, under `tools/rooms/`, holds `room_open`, `room_list`, `room_supervisors` and `room_add`,** every
  call asking the running receiver as the `bmo` commands doing the same ask ask it. The first three open an agent
  room on the record alone, read the rooms a supervisor answers, and name a room's answerers.
- **`room_add` adds a chat-service room in `record-only` mode on the owner's word in the turn.** It answers where
  the receiver marked every new message of the turn the owner's, and refuses any other turn with one sentence,
  since no grant in force names adding a room. It takes the provider's key and one of `roomId`, `contactId` or
  `contactIds` under `title`; a call naming none answers the provider's rooms and contacts, each by its name and
  with whether the receiver already holds it, so the owner picks.
- **The addition is the receiver's own `POST /rooms`** (`docs/api.md`, 「Rooms」), and its refusals are the
  receiver's own sentences. The road never sets `avatar` and changes no existing room's mode: the console's
  registration screen alone changes a mode. `bmo room add` carries the same call for a shell.
