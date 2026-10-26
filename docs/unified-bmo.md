# One chat agent

## One agent, one session

- **One row.** The chat agent is the agency's registration whose settings make the one session: the `agent`
  scope, the `window` shape and the default answerer. The receiver reads it from `GET /agents` like every
  conversational agent; nothing of this document is read off its name.
  - The chat agent in the owner's private chat and in the group rooms is the same agent.
  - The job, search, editor and gate agents are separate.
- **Who carries it:** the agency, on the one session its row keeps across rooms (`POST /sessions` with the
  `agent` scope), as the resident program standing in the agent's one pane. Every round, fold and touch of every
  room is a turn put into it (`POST /sessions/{sessionId}/turns`), and a person watching the pane sees it working
  and can type into it.
- **Every room but one a companion answers alone.** A room made for a companion names who answers in it. Where
  the chat agent is not among them, its words never reach this session, the room is never recorded among the
  agent's rooms, and nothing of it is in the transcript; where it is, the room is one more group room of this
  session's, with the companion's words in its line.
- **One queue:** chat runs for every room are handled one at a time from a common queue.
- **One window:** every turn put into the session, whatever brought it, is composed by one module and handed the
  same memories and reference (`docs/bmo-window.md`).
- **Words that arrive while a round works** go into the working turn at its next look where the program takes
  words mid-turn, but only the running round's own room's words, and only where they keep the standing and the
  tools the round's prompt declared. Another room's words wait for that room's own round, because a round's
  answer goes to its room alone.
- **What stays per room:** a room's message batch, read position, conversation window and reply target remain that
  room's own. Just before handling, the latest state of the common session is read.
- **During a rest and a session fold,** waiting rooms are shown no typing.

## The system prompt and the room switch

- **The fixed system prompt** is the chat line's, which the agency composes and keeps (the room-switch contract,
  the absolute rules, the common manual, the answer rules, the base persona). It is the same for every room.
- **The base persona is the agent's own,** the base layer of its brain, which the agency reads itself when it
  launches the chat agent and puts after the line's documents. The receiver hands no base persona with a turn.
- **The persona a room adds is the receiver's:** the room's tone layer, `prompts/persona/<tone>.md` under the data
  root, and the room's own persona, `prompts/persona/rooms/<chat id>.md`, each under its header in the turn's block.
- **What a turn's scaffolding means is said there once**, not on every turn: that a quiet instruction comes from
  the system, is acted on as the owner's word and may be answered with `<<SKIP>>`; that a reference block outranks
  the session's memory and what its already-loaded marks are; what a room context mark is; that the system lines
  are English and the answer language line names the language of each person the turn answers; and what the owed-answer line withholds. The turn
  carries only what changes from turn to turn: the room and its facts, the blocks themselves and the one-line
  states (answer language, owed answer, the waiting window).
- **Kept out of the system prompt:** each room's tone and traits, the group-room rules, the private chat's
  instructions and agent list, and the rules for what can be done in the room (a picture, a file, a reaction, a
  sticker, a voice briefing, pinning, an answer aimed at a message), which the room's chat service declares and
  which reach the turn marked by capability, with one line naming what that service cannot do.
- **The room-switch block is the receiver's,** handed to the agency as the turn's `room` (`chatId`, `text`,
  `mark`):
  - When the room changes in the actual run order, the room's context stands at the very front of the request
    prompt: in full once per session per version, and as a one-line mark after that.
  - The same room is not repeated.
  - **The mark** names the room (its chat id, its kind and its name), says that this room's context, in this same
    version, was given in full earlier in this session, and says that it is in force again and that every other
    room's context given since no longer applies. So the room a turn is in is never left to be inferred from what
    came before it.
  - **What a version is:** the whole block as composed for the turn (the room's rules or the owner's
    instructions, the roster, the road it answers on, the room's tone and traits). Any change to any of them is
    a new version and goes in full.
  - **After a new session, a session fold** or anything that changes the block, it goes in full again; a fold
    empties the record of what the session holds, and a new session has none.
  - Two records stand in the receiver's database, both written only after the answer succeeds under the session
    that answered, so they survive restarts and a failed call is never recorded as having delivered the settings:
    the room whose block the session read last, and each room's version the session was sent in full. A session
    that answered under a new id is recorded as holding the block only where the block went in full.
- **Reference documents:**
  - Every reference document the gate hands is loaded in full once per session per version, and recorded the
    same way only after success, in the agency's record of the session's loads (`PUT /session-loads/{sessionId}`).
  - That record is not emptied by a room switch; only a new session and a session fold empty it
    (`DELETE /session-loads/{sessionId}`).
- **A road with no session** needs the context on every request; the agency carries it as that road's line
  declares.

### Checked on every request

- **The request's facts:** the room identifier, the requester's identity, the run-privilege mark, whether it is
  delegated, and the answer's destination.
- **No leaking between rooms:** even if one session remembers several rooms' conversations, a group room is never
  shown information from another room or the private chat.
- **The switch block covers only the current room.** The previous room's tone, rights and instructions do not
  carry over to the next room, whether the room is restated in full or by its mark.

## Console conversation in the private chat

- **Two places to speak.** The owner's private chat is one room two roads both reach: the owner's everyday chat
  service and the web console's own, the chat's record.
- **Each road is a window onto the owner's chat, and neither is its default.** What the agent says there on its
  own (a job's report, a deployment's or a restart's notice, a queue notice, a voice briefing, a question a job
  puts to the owner) is kept in the chat's own record, bound to no road, and the console's screen shows it, since
  it reads the chat's whole line.
- **Which messengers see the chat is the room's list of providers** (`docs/receiver.md`, 「A room's chat
  providers」), the same setting every room has: every message kept in the chat goes to each provider on it,
  wherever it was said. A chat on the record alone is seen in the console alone; with the everyday chat service
  on its list, everything said in it stands there too.
- **Where an answer goes** is decided by where the words came from: the road carried on the messages of the batch
  itself, not a name anything compares. The answer is kept and then goes to the chat's other providers like any
  message.

### The cases

- **Words from another chat service** are answered on that service, in the words' form its dialect reads.
- **Words from the web console** are the console's own road.
  - Console input goes straight into the archive as the owner's message and on to the agent, and from the archive
    to the room's other providers, in the owner's own voice.
  - The console has no name of its own for the owner, so the owner's row stands under the name the room's own
    service last showed the owner by in that room, else under the owner's registered name, the user the web
    unit's account model links the owner's number to, and under the owner's number where neither stands. The
    owner's console rows are never read for it, and a name another room calls the owner by never reaches this
    one. The chat agent's name never stands on the owner's row.
  - The answer stays in the archive as markdown, as written, and shows in the console; it goes to the room's other
    providers as every kept message does.
  - A long answer stands as the same run of messages it would have stood as on the other road: the room's line
    holds the rows of both roads together, so one answer is one shape wherever it is read.
  - What the owner types there is read for what the machine keeps, whichever road it was typed on.
- **Numbering console messages:**
  - Words from the console have no number of another service, so the archive gives them its own number
    (negative), unique within that room.
  - A number once given is never given again in that room, even after the message holding it is taken out of the
    record.
  - A message with such a number does not exist on another service under it, and is never sent to one as a reply
    target; where the message went to that service, the number the service gave it there is the target.
  - Places that read a room's messages in order (the conversation window, the console screen, the brain's
    readers) order them by send time and arrival order, not message number, so words from both places stand in
    one line by their own time.
  - The room's read position is also a place in that line: whether a new message was already read is decided by
    whether it stands before the read-position message in the line, not by the size of its number.
  - A message a road hands over that already stands at or before the read position wakes nobody, even one that
    calls the agent, and the receiver says so in its journal, naming the room, the message and the read position.
- **Files uploaded from the console** are the console's own road too.
  - Each file is kept on this machine and archived as the owner's message under a console number, one message a
    file: a JPEG or a PNG filed as a picture, the pictures of one send drawn as one album, and any other file kept
    in the message-file store under its message.
  - The words typed beside them stand on the first file's message, however long, with the reply the send names.
  - They go on to the agent as the owner's console words do, and to the room's other providers as one send of
    files.
- **A turn with words from both places** is answered on the other service, and the console sees its answer.
- **A turn with only quiet instructions and no message** is answered on the road its instructions were asked on
  where they carry one and agree (`via` on bmo-job's instructions). An instruction that answers nobody's words (a
  job's report, a deployment's notice, a failure order) carries none and leaves the others to decide. Where none
  carries one, or those that do name different roads, the turn is opened by the system's lines alone and answers
  into the owner's chat's record, whichever road the owner last spoke on.
- **A line nobody's turn answers is kept in the record too.** The receiver's own lines in the owner's chat and a
  send into that chat that names no road are kept in its record, and go to the chat's other providers from
  there. In a room whose whole line no road's screen shows, such a line goes on the room's own road.
- **An instruction pushed into a working turn counts with the round's own.** Its road joins the roads the round
  was opened with, and the answer's road is decided again over all of them, as it is for a message pushed in.
- **The turn is taught its road, not the room's.** Its room context carries the dialect and the capabilities of
  the road it answers on and names that road's key, and every send and launch it makes is given that key as
  `via`.

### Group rooms

- **The same rule:** the owner's words from the console are kept in the room's archive as the owner's and go to
  every other provider of the room, in the owner's own voice. The turn they wake answers on the room's own road
  where no road shows the room's whole line, and the copy its answer leaves in the archive goes to the room's
  providers other than that road.

## Replacing the session

- **The session is replaced, not burned again,** when its head moved, its road moved (another provider, model or
  effort, or another login), the cache lifetime its provider declares passed, or the owner asked for a fresh
  start. The agency carries the replacement as the session's provider declares.
  A full session whose provider hands it over by its own note (`providers.toml`, `handover = "handoff"`) is not
  folded: the owner's word to start it afresh is written for it, with the reason `full`, and its next turn
  replaces it.
- **The head is what the session's cache stands on:** the common system prompt, which the agency composes and
  keeps. A turn the receiver composes is appended after the session's own history and moves nothing ahead of
  it, so no part of a turn is the head: not the room-switch block or its mark, the reference blocks, the turn's
  scope line, the queue line, the room's facts, the words and instructions it carries or its closing lines, and
  not a question, an item or a check of the draft process. A turn of any room on an unmoved head resumes the
  session.
- **The agency says whether the head moved.** The session's state names why its prefix changed where the
  agency names it (`prefixChangeReason`, the agency's `docs/api.md`, 「Sessions」); each turn the receiver
  carries keeps the reason the state named, and a later turn whose state names another replaces the session.
  Where the agency names none, the receiver's own measure stands in: the fingerprint of the fixed lines every
  round's turn is written in (the round policy's header, block, closing, queue and push lines), which a
  deployment that rewrites them moves and nothing a room or a turn fills in does.
- **A session with no record of its head** is resumed where the agency's state says it was never opened (its
  generation is 0 or not named), the record starting with its first turn, and replaced otherwise.
- **Before the old session is ended it writes its handoff,** whatever the reason. The receiver asks it one more
  turn, the handover policy's `ask` instruction (`handover.toml`), naming the file it writes with its file tool:
  `handoffs/<agent>/<epoch-ms>.md` under the data root (`docs/store.md`, 「The data root's folders」). The file
  holds what the session was in the middle of and how far it got, the promises it made and what wakes them,
  the open threads of each room, what the next session must do first, and what it learned was wrong. The turn
  is bounded by the policy's `ask_seconds`; a run that fails or passes the bound ends the session all the same.
  A file that stands and is not empty is copied to `handoffs/<agent>/latest.md`, the newest handoff.
- **The new session's first turn carries the note at its head:** the handoff file's path and its whole content,
  so the session can read it again with its file tool.
- **Where the old session wrote nothing,** the note is the transcript the receiver keeps of what the session
  answered in each room, the tail of it kept on the road the session stands on, carried under its own heading.
- **Only where neither stands** does the first turn open on the `lost` lines, saying memory could not be
  restored; the whole old context is never read again. The owner's private chat is then queued one notice
  (`handoff-lost`, `docs/codes.md`) that the handoff could not be recovered.
- **The owner's word to start afresh is spent** by the replacement it asked for: one word, one replacement.
- **Behind the note, the first turn is whole:** it is composed for the new session, which holds nothing yet, so
  the room's block and every reference document go in full, and it carries everything the turn would have
  carried on the old session, the room's messages archived since the agent last answered there among them.
  What arrived around the handover that the agent never read reaches the new session in that turn, up to the
  archived block's cap: the newest of them, with a line counting the older ones left in the room's archive
  (`docs/bmo-window.md`, 「What every turn carries」).
- **Only the one session is replaced this way.** The receiver judges a replacement only for an agent whose row
  keeps one session across its rooms; a room's own session is the agency's to resume.
- **Moving between rooms carries no summary of a room:** the room's block and its archived messages since the
  agent last answered there, up to the archived block's cap, are what a turn in another room reads, as every
  turn does.

## Default access and per-room delegation

### Everyone is welcome

- **Defaults:** every room is allowed and every room's run holds the same tools; there is no room or tool opt-in
  command.
- **No friend registration:** every participant is treated warmly without one.

### Owner-only work is the agent's judgement

- **The owner comes first.** The owner's direct request, in any room, takes precedence over the agent's own
  owner-only rooms and ordinary-member restrictions.
- **Who is speaking:** the sender is checked by the real message's sender. Quotes and links create no rights.
- **No code lock on owner-only work.** This covers system changes, launching jobs, item approval and order,
  work-order stage changes, pins, watches, immediate cleanup and saving delegations. The agent decides by the
  standard of the absolute rules: is it the owner's own request, or a current delegation that entrusted that
  action to that sender?
- **The run-privilege mark** at the head of the request (the turn's `privileged`) only says whether all new
  messages this turn are the owner's; it is not a lock.
- **The same absolute rules also cover:** not carrying room talk elsewhere; refusing questions about privacy and
  about the system itself; and none of those refusals reaching the owner's own request, by the owner's recorded
  sender, in any room.

### Delegations

- **Where they live:** the room's rules are `rule` memories in the agent's brain, read for a turn by the gate, and once the
  account model is in force (`GET /accounts/model` on the web unit), the grants the web unit keeps, a room rule
  being a standing-rule grant to the subject's user with the owner's words beside it.
- **Commands,** each on the basis of an owner message in that room (`--message <id>`):
  - read with `bmo room rules --chat <id> --list`;
  - add a room rule with `--set <rule-id> --subject <userId> --text <work and target project>`;
  - revoke one with `--delete <rule-id> --subject <userId>`;
  - on the account model, grant one relation alone with
    `--grant <relation> --subject <userId | members> [--until <time>] [--text <words>]`, and take it back with
    `--revoke <relation> --subject …`; off it the verb refuses these.
- **Only current entries:** neither place keeps a revocation history or inactive entries. The original chat
  record is separate.
- **Delivery:**
  - The receiver includes what is in force in the room-switch block.
  - When it changes, even in the same room, it is replaced whole with a new block.
  - A delegation left in earlier conversation or a summary is no basis for rights.
- **Scope:** a rule applies only to its named user, work and project.
- **Rights, not a wake:** a rule gives its user rights once the agent is awake; it does not wake the agent. That
  user's text reaches the session only when it calls the agent, replies to the agent or falls in the room's open
  window, and their file only on those same grounds, read off its caption (`docs/room-files.md`, 「Who a file
  wakes」).
