# One session across rooms

## One agent, one session

- **One row.** What this document states holds for a supervisor whose row makes the one session: the `agent`
  scope and the `window` shape. The receiver reads it from `GET /supervisors` like every other supervisor; nothing
  of this document is read off its name, and whether it is the default answerer is the receiver's setting, not its
  row (`docs/api.md`, 「Supervisors」).
  - The supervisor in the owner's private chat and in the group rooms is the same agent.
  - The work it hands off runs as the agency's roles — a job, a search, a delegation — each apart from it.
- **Who carries it:** the agency, on the one session its row keeps across rooms (`POST /sessions` with the
  `agent` scope), as the resident program standing in the agent's one pane. Every round, fold and touch of every
  room is a turn put into it (`POST /sessions/{sessionId}/turns`), and a person watching the pane sees it working
  and can type into it.
- **A room answered by other supervisors alone stays out of it.** A room names the supervisors that answer in
  it. Where this supervisor is not among them, nor the default answerer of a room naming none, its words never
  reach this session, the room is never recorded among the
  agent's rooms, and nothing of it is in the transcript; where it is, the room is one more group room of this
  session's, with the other agents' words in its line.
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
  launches the supervisor and puts after the line's documents. The receiver hands no base persona with a turn.
- **The persona a room adds is the receiver's:** the room's tone layer, `prompts/persona/<tone>.md` under the data
  root, and the room's own persona, `prompts/persona/rooms/<chat id>.md`, each under its header in the turn's block.
- **What a turn's scaffolding means is said there once**, not on every turn: that a quiet instruction comes from
  the system, is acted on as the owner's word and may be answered with `<<SKIP>>`; that a reference block outranks
  the session's memory and what its already-loaded marks are; what a room context mark is; that the system lines
  are English while the language an answer is written in is the agent's own judgement, never a line of the turn;
  and what the owed-answer line withholds. The draft process's questions and the stage each asks stand there,
  and so do what a round's one-word send permission means, the review's instruction and verdict wording, the
  situation-note instruction with its ceiling, the closing rules (how a turn opens and closes, when a `<<WAIT>>`
  is right, and that the owner's own words in his own chat are always answered), the queue rule, the brain's
  rule and the memories rule, so a round's turn names only which round it is and whether it may send, a note turn
  carries only its mark, and the review turn carries only its mark and the material it reads. The turn carries
  only what changes from turn to turn: the room and its facts, the words themselves, the round's mark and the
  material the gate attaches.
- **Kept out of the system prompt:** each room's tone and traits, the group-room rules, the private chat's
  instructions and agent list, and the rules for what can be done in the room (a picture, a file, a reaction, a
  sticker, a voice briefing, pinning, an answer aimed at a message, a folded answer), which the room's chat
  service declares and which reach the turn marked by capability, with one line naming what that service cannot
  do. A section stands where the road performs its verb, and the one on folding where the road's dialect folds.
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
  - **After a new session, a session fold, a new generation the agency opens under the session** or anything
    that changes the block, it goes in full again; a fold, a new generation and a new session empty the record
    of what the session holds.
  - Two records stand in the receiver's database, both written only after the answer succeeds under the session
    that answered, so they survive restarts and a failed call is never recorded as having delivered the settings:
    the room whose block the session read last, and each room's version the session was sent in full. A session
    that answered under a new id is recorded as holding the block only where the block went in full.
- **Reference documents:**
  - Every reference document the gate hands is loaded in full once per session per version, and recorded the
    same way only after success, in the agency's record of the session's loads (`PUT /session-loads/{sessionId}`).
  - That record is not emptied by a room switch; only a new session, a session fold and a new generation the
    agency opens under the session empty it (`DELETE /session-loads/{sessionId}`).
  - **A person's own thread** is the receiver's own document, not the gate's: a session is handed it whole on its
    first turn in the person's private room, then only the lines added since the last line it was handed, under a
    heading saying so; a turn with no new line attaches nothing and its draft record lists the piece as `held`. The
    last line handed is kept for that session, not the room: a session continued across a restart keeps it and
    gets only the added lines, while a new or replaced session, a fold's included, and a new generation the agency
    opens under it start again from the whole thread. The mark is let go when that session is replaced or folds,
    and when a new generation opens under it.
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

- **Two places to speak.** The owner's private chat is one room (`docs/receiver.md`, 「Identity, and reaching the
  owner」), which the web console's own road, the chat's record, always reaches, and each messenger on its list of
  providers reaches as well. On the record alone it is spoken in the console alone.
- **Each road is a window onto the owner's chat, and neither is its default.** What the agent says there on its
  own (a job's report, a deployment's or a restart's notice, a queue notice, a voice briefing, a question a job
  puts to the owner) is kept in the chat's own record, bound to no road, and the console's screen shows it, since
  it reads the chat's whole line.
- **Which messengers see the chat, and whose words it takes, is the room's list of providers** (`docs/receiver.md`,
  「A room's chat providers」), the same setting every room has: every message kept in the chat goes to each
  provider on it, wherever it was said, and a messenger off the list neither hears it nor is heard in it. A chat
  on the record alone is seen in the console alone; with the everyday chat service on its list, everything said
  in it stands there too.
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
    one. The supervisor's name never stands on the owner's row.
  - The answer stays in the archive as markdown, as written, and shows in the console; it goes to the room's other
    providers as every kept message does.
  - A long answer stands as the same run of messages it would have stood as on the other road: the room's line
    holds the rows of both roads together, so one answer is one shape wherever it is read.
  - What the owner types there is read for what the machine keeps, whichever road it was typed on.
- **One number for each message:**
  - A message stands in the archive under the one number the road it came in or went out on gave it: a chat
    service's own number for the words it handed over and for every line the receiver sent there, an agent's
    answer, a line of its tools or `bmo talk` and a line the receiver says itself alike. A sent line is kept once
    the send has answered, under the number the answer names, so the transcript shows it under that number,
    `<<REPLY:n>>` pins by it and a reply on that service names it.
  - Only a message no road numbered takes the archive's own number (negative), unique within that room: words
    from the console, every message of a room on the record alone, and a send on a road that gave no number.
  - A number once given is never given again in that room, even after the message holding it is taken out of the
    record.
  - A message with such a number does not exist on another service under it, and is never sent to one as a reply
    target; where the message went to that service, the number the service gave it there is the target.
  - A reply that service hands over to the number it gave the message answers the message under the archive's own
    number.
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
  was opened with, and the answer's road is decided again over all of them.
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
  effort, or another login), the cache lifetime its provider declares passed (`providers.toml`, `cache_minutes`,
  else `keepalive_idle_minutes`), or the owner asked for a fresh
  start. The agency carries the replacement as the session's provider declares.
  A full session whose provider hands it over by its own note (`providers.toml`, `handover = "handoff"`) is not
  folded: the owner's word to start it afresh is written for it, with the reason `full`, and its next turn
  replaces it.
- **A session past its cache lifetime is woken only by what someone waits on.** A room's own words and a room
  reply (`docs/wake.md`, 「Quiet instructions」: a research result, a delegate's or a worker's answer, a job's
  report, a reminder come due) open a round on it at once, and that round replaces it: the fresh session reads
  the handoff document, then the room's block with the brain's index at session opening, then the unread
  messages of the turn. Nothing else opens a turn on
  it: no keepalive touch, no fold, no housekeeping instruction, no push and no spoken script
  (`docs/bmo-window.md`, 「The touch」; `docs/voiced-notices.md`), since waking it for those would have the
  provider read its whole prefix again for nothing. The lifetime is the one the agency's warm and cold
  criterion reads (its `docs/sessions.md`, 「The warm/cold criterion」), the same minutes in
  `providers.toml` here.
- **The head is what the session's cache stands on:** the common system prompt, which the agency composes and
  keeps. A turn the receiver composes is appended after the session's own history and moves nothing ahead of
  it, so no part of a turn is the head: not the room-switch block or its mark, its opening index, the turn's
  scope line, the room's facts, or the words and instructions it carries, and not a question, an item or a
  check of the draft process. A turn of any room on an unmoved head resumes the session.
- **The agency says whether the head moved.** The session's state names why its prefix changed where the
  agency names it (`prefixChangeReason`, the agency's `docs/api.md`, 「Sessions」); each turn the receiver
  carries keeps the reason the state named, and a later turn whose state names another replaces the session.
  Where the agency names none, the receiver's own measure stands in: the fingerprint of the fixed lines every
  round's turn is written in (the round policy's header, block, closing, queue and push lines), which a
  deployment that rewrites them moves and nothing a room or a turn fills in does.
- **A session with no record of its head** is resumed where the agency's state says it was never opened (its
  generation is 0 or not named), the record starting with its first turn, and replaced otherwise.
- **A generation the agency opens under the session is a handover the receiver did not make.** The session's
  state names the generation its turns stand in, and the agency announces the handover on the event stream as
  `system-prompt-changed` for the one session across rooms, whose state names no higher generation. A turn that
  finds the state past the generation this session last answered on, the session gone where one was answered on,
  or the agency's announcement of the session on its stream releases what the ended generation held before its
  prompt is composed: the loads, the room block, the note versions and the cache record, and the fold reported to
  the brain so the marks it handed that session clear (`docs/boundary-api.md`, 「The brain」). A plain
  turn releases nothing, and neither does a session resumed after the agency's restart or a deploy, nor a return
  to another room of the one session across rooms.
- **Before the old session is ended it consolidates and writes its handoff,** whatever the reason. The receiver
  asks it one more turn marked `HANDOFF ASK.` and naming, as data, the file it writes with its file tool:
  `handoffs/<agent>/<epoch-ms>.md` under the data root (`docs/store.md`, 「The data root's folders」); the rule
  for the turn stands in the room's block (`handover.toml`, `ask_rule`). The turn goes in one order, none of it
  said in any room: the brain's consolidation first (「The brain's consolidation」 below), then the fold told to
  the brain with `brain_fold`, then the file, then the exact skip. The file
  holds what the session was in the middle of and how far it got, the promises it made and what wakes them,
  the open threads of each room, what the next session must do first, and what it learned was wrong. The turn
  is waited on until it ends, however long it takes; a run that fails ends the session all the same. The turn
  answers a room, as every turn of a session across rooms does, carrying that room's block: the room the
  replacement is made in, or the room the agency's asking named, and the owner's private chat where neither
  names one.
  A file that stands and is not empty is copied to `handoffs/<agent>/latest.md`, the newest handoff.
- **The new session's first turn carries the note at its head:** `HANDOFF.`, then the handoff file's path and its
  whole content in its block, so the session can read it again with its file tool; the rule the marker stands for
  is in the room's block.
- **Where the old session wrote nothing,** the note is `WHAT YOU ANSWERED BEFORE.` with the transcript the
  receiver keeps of what the session answered in each room, the tail of it kept on the road the session stands
  on, in its own block.
- **Only where neither stands** does the first turn open on `NEW SESSION.` saying memory could not be restored;
  the whole old context is never read again. The owner's private chat is then queued one notice
  (`handoff-lost`, `docs/codes.md`) that the handoff could not be recovered.
- **The owner's word to start afresh is spent** by the replacement it asked for: one word, one replacement.
- **No fresh session opens without its handoff, whoever opens it.** Where the agency is about to open a fresh
  session for an agent that had one — the kept session gone, a program stood anew that cannot resume it, its own
  restart — it asks the receiver first (`POST /handoffs/{agentName}`, `docs/api.md`, 「Handoffs」), and the receiver
  asks the agent's standing session, usually still alive in its pane, for its handoff file exactly as a
  replacement does, waited on until its turn ends. A second asking for the same agent while one is under way
  waits in line behind it. The file it writes is kept for the agent
  (`<agent>_handoff_pending`, `docs/store.md`), and the next turn on a session the agency's state says was never
  opened carries it as its note, in the order below; a turn on a session the agency resumed lets it go. Where the
  agent has no opened session to ask, or it wrote nothing, the transcript the receiver keeps stands in at the next
  replacement as above.
- **A handoff is asked at most once in a span** (`handover.toml`, `again_minutes`). The agency asking again for
  the same agent's handoff within that span of the last one the standing session was asked for is answered with
  the file that one kept, where it is still kept, else with nothing, and the standing session is not asked. Each
  such asking is logged with how many have come in a row, and so is a replacement for the same reason within
  the span of the last one, so a road whose program cannot keep its session shows in the journal instead of
  asking turn after turn.
- **The first turn goes in one order: the handoff, then the room, then the brain.** First the note above; then
  the room's block, in full since the new session holds nothing yet, and the room's state note in full, since the
  new session holds no version of it (`docs/bmo-window.md`, 「What every turn carries」), and the turn as it would
  have gone to the old session, the room's messages archived since the agent last answered there among them, so what arrived
  while the handover was under way and the old session never read stands right behind the handoff, up to the
  archived block's cap: the newest of them, with a line counting the older ones left in the room's archive
  (`docs/bmo-window.md`, 「What every turn carries」); the room's block carries the brain's index at that opening.
- **The brain's index rides the room's block, not the turn.** The block goes in full when the session opens on
  a room, and that opening block carries the room's groups with their unread counts and hints as the gate's
  index gives them for the new session (`GET /reference/index`), and the index's one hint; where the brain says
  a consolidation is due at that opening, the due line follows. No cross-room walk rides the first turn: the
  handoff note carries what the old session was doing, and the agent opens the brain's index with its tools as
  later rounds need it. It is a hint and no obligation.
- **Only the one session is replaced this way.** The receiver judges a replacement only for an agent whose row
  keeps one session across its rooms; a room's own session is the agency's to resume.
- **Moving between rooms carries no summary of a room:** the room's block and its archived messages since the
  agent last answered there, up to the archived block's cap, are what a turn that returns to a room reads. An
  ordinary round in a room the session already holds carries no archived block.

## The brain's consolidation

The agent's brain consolidates at the two moments its design names and no other (the brain's `docs/design.md`,
「Consolidation」): its unconsolidated episodes' importance reaching its threshold, and a session folded or handed
off. The brain runs no pass of its own; the agent consolidates with its brain tool, and the receiver asks it to
at those moments and tells the brain of a fold it asks of the session itself.

- **A consolidation** goes in four steps, in one turn: the unconsolidated episodes read, most important first
  (`brain_unconsolidated`); what each one worth keeping comes to drafted, a fact, a reflection or a correction,
  with the standing fact it would confirm, strengthen or close; each draft written (`brain_learn`), a
  consequential one only once it passes the review below; and every episode read marked consolidated
  (`brain_consolidated`), those that came to nothing among them, so none is read again, except the episodes of a
  draft the review stopped for a contradiction, which stay unconsolidated.
- **The review is for consequential drafts alone:** a draft that would close a live fact (`replaces`) or that
  carries importance 8 or more. The agent reads again the episodes it rests on and the live fact it would close,
  and asks itself three things: whether those episodes actually say or show the statement; whether it is the same
  as a live fact, in which case it confirms or strengthens that fact rather than writing anew; and whether it
  contradicts a live fact the episodes do not overturn, in which case it writes nothing and leaves those episodes
  unconsolidated. Only a draft that passes is written; a draft below the bar is written as it stands. The review
  is the agent's own second reading in the same turn: it costs reading, never a second model or a turn of its
  own.
- **At a hand-off** the closing session consolidates what it holds, then tells the brain it folds
  (`brain_fold`), then writes its handoff (「Replacing the session」 above).
- **When the brain says it is due,** the room's block carries the due line at session opening
  (`docs/bmo-window.md`, 「What every turn carries」), and the round's `memory` question consolidates in that
  turn, bounded to what the turn can afford: the first episodes the brain answers, never all of them, and no `brain_fold` (`docs/drafting.md`,
  「The questions」).
- **At a fold the receiver asks of the session,** in this order: one quiet turn on the session, tools on, carrying
  the `CONSOLIDATE.` marker with the room's block, whose rule (`handover.toml`, `consolidate_rule`) says what the
  turn is: the session consolidates the first of its unconsolidated episodes, as many as the turn can afford and
  never all,
  calls no `brain_fold`, says nothing in any room and answers exactly the skip; then the fold, the agency's
  command; then, once the agency answers it, the fold reported to the brain by the receiver itself
  (`docs/wake.md`, 「The room's queue and the round」). The fold never waits on the consolidation: where the
  session cannot take the turn or the turn fails, the journal says the consolidation was skipped and the fold
  goes ahead. A session whose program carries its own fold is asked none of this.

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
  messages this turn are the owner's; it is not a lock. It is read off the words each round itself takes, in
  any room, his private room or not, and whatever an earlier round of the same conversation read, and every
  question of that round's draft process carries it. Only people's words count: a line an agent said, any
  supervisor's or the receiver's own, neither grants it nor takes it away, so a round whose people's
  words are all the owner's is his. A round that takes no people's words, a quiet instruction's or agents' lines
  alone, is the owner's in his private room alone.
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
- **Rights, not a wake:** a rule gives its user rights once the agent is awake. Every member's words wake the
  agent in a room it answers, on the grounds in `docs/wake.md` (「Who reaches the session」); a rule neither wakes
  it nor keeps it asleep, and a muted member's words do not wake it.
