# The draft process

How a round's message is made: the one draft turn the round's session is asked, the questions that turn follows
and the table that says which, the markers that end the process early, the items, the review and the checks the
message passes, and the record each turn of it leaves.

## What it is

- **A round is the agent's judgement where one is asked, and then one draft turn.** Where the round is the
  agent's to judge first (`docs/wake.md`, 「The room's queue and the round」), its first turn is that judgement,
  which asks no reference, reads no index and runs no gate; a skip ends the round there. The room's situation
  note is no step of a round: it is written when the session leaves a room, before a handoff and on a keepalive
  touch, and a session reads a room's note once (「The situation note」 below).
- **The draft is one turn, and its answer is the message.** A round's turn goes to the agent as every round's
  does (`docs/wake.md`, 「The room's queue and the round」), after that judgement where one is asked, and after
  the window's pregate: the core the gate attaches, where the supervisor's brain is on (「A supervisor whose brain
  is off」 below). The brain's index rides the room's block at session opening, never the turn. The turn carries only the line naming the questions its round follows; each question's text
  stands in the room's block, which the session is handed as its system prompt. In that one turn the agent
  follows the questions in the order the line names them — it reads, finds the facts the message rests on,
  engraves what it establishes in the brain with its tools and does what it promised — and then writes the
  message, which is its answer. No question is asked as a turn of its own, and nothing stands as a message
  before that answer.
- **Confidence comes from a fact's dates, and hedging is no option.** The brain's facts carry when they hold from
  and when they were last confirmed: a fact found or confirmed recently is said with confidence, and one the
  agent is not confident of is searched for quietly before it answers. The message never tells the room that
  something may be out of date; the search is the agent's own and the room hears only its result.
- **A fact about the system's live state has one source.** A login, a usage window, a model, what is running,
  what is deployed, a setting: such a fact counts only when read now from the service that owns it, by its route
  or its tool. A file, a cache or an earlier answer that mirrors that state is not a source. What has no source
  is read now, and what cannot be read now is said as not confirmed, never as fact. The process's fixed rules
  and the `facts` question both say so.
- **Every tool the round's turn has is open in the draft turn:** reading and writing memories, searching, filing
  a job, `say`. A question to read more is followed by reading; a question whether a promise was kept is
  followed by keeping it.
- **Every later turn of the process is a turn of the same session,** opened as `question` (`docs/bmo-window.md`,
  「The openings」), so it holds the round's intent, its reference and its cache: the closing check, an item, the
  review, a revision the review sent back and a check.
- **The receiver reads three things of an answer and nothing else:** a skip standing in it outside a code span or
  fence, which ends the process with nothing sent; the marks `<<DONE #<number>>>` and `<<SEPARATE #<number>>>` on
  a message shown to the round (「What came in meanwhile」 below); and the answer itself. `<<SEND>>` opening an
  answer is taken out of it, the answer being the message with the marker or without, and a marker with nothing
  after it names no reason. A skip anywhere in an answer wins, and where an answer carries skips naming reasons and
  not, one that names a reason wins.
- **A later turn of the program that says nothing leaves the answer written before it.** One turn of the process
  can span several turns of the agent's program: words pushed after its last tool call, or a background task's
  notice, each open one, and the agency hands each one's answer with what opened it (its `docs/agent-control.md`,
  「The round's answer」). The receiver builds the answer from them as the agency does — a turn opened by words
  handed over replaces it, a turn the program opened on its own adds to it — save that a turn whose answer says
  nothing, a skip of either shape, a wait or no words, never displaces an answer that says something: the answer
  written before it stands and is read as above. So a turn opened by a quiet instruction alone that skips does
  not take away the message written before it. A cancellation pushed into the turn is answered with a reply
  saying so (`chat-round.toml`, `[push]`), never with a skip. Where the agency hands no turns, the answer it built
  stands.
- **A turn of a round skips in its form: `<<SKIP: <reason>>>`,** the reason one non-empty line in English saying
  why the round sends nothing. A skip that names none is not a skip: the bare marker, or the form with nothing
  after its colon, is read as no verdict, and the same turn is asked once more under one line saying that a skip
  names its reason in that form (`drafting.toml`, `skip_again`), the session holding the round already. A second
  answer naming none ends the round as a skip whose reason is recorded as `none given`, so a round log that ends
  in a skip always says why, or says that no reason was given. The asking is the only repair a bare skip gets: a
  turn that answers anything else is read as it answers.
- **The bare marker stands where a rule gives it whole.** The handoff turn, a consolidation turn and the
  situation-note turn are each told to output exactly `<<SKIP>>` and ask no reason, since nothing of them goes to
  a room and what they leave unsaid is settled by their own rule; the reader of those turns takes a skip naming a
  reason as the same nothing to say.
- **A draft is not a message.** The room's messages are the only record of what the room has heard. A round
  log's answer, a note, or an earlier turn of the session that was not sent is not something said, and no round
  skips a repeated request because it wrote the answer in one; the duplicate rule reads the room's own record and
  nothing else.
- **A round whose draft turn answers `<<WAIT>>` runs no further process:** the wait stands as `docs/wake.md`,
  「Waiting」, says, and the round that takes the waited words opens the process again.
- **A round runs what its row holds,** every supervisor's alike: no agent's turns are exempt from the process,
  and what a round leaves out is what its row of the process table switches off — a question, the review, an
  item after the message or a check on the final text (「The process table」 below). A row that switches nothing
  off runs the whole process. A console talk, a typed line, the touch and the line before a fold are no rounds of
  a room and run none.

## A round handed to the double

- **The draft turn may hand its round over.** Where the round calls for work longer than moments, the agent
  answers `<<HANDOFF>>` and nothing else, at once and before doing any of that work; the process's fixed rules
  say so. An answer that is nothing but the sentinel is the hand-over, and only of the round's own draft
  turn; the sentinel standing among other words is words.
- **The same turn then goes to the agent's double.** The receiver carries the round's turn again, unchanged, as
  a run of the role the agency names for a supervisor's delegated work, started as that agent's double at the
  agent's scratch workspace (the agency's `docs/tools.md`, 「The delegate」): the room's block whole with the
  brain's index, every document of the turn opened in full, and the turn, under one line saying that the round
  was handed over and that the double answers it as the agent itself, followed by every paragraph pushed into
  the agent's own turn before it answered the sentinel. It is no instruction the agent writes: the double reads
  what the agent read and follows the same process. Its turn names the round, so its run is the round's, and
  it is followed as the agent's own turn is, re-attached across a cut (`docs/bmo-window.md`, 「Quiet
  instruction」).
- **The double's answer is the round's draft,** and everything after it is the same process: the closing
  check, the items, the review, a revision and the checks are turns of the double's run, each sent as its own
  words alone, since the double's kept session holds the round. The message goes out as the agent's, by the
  round's own send.
- **The double's session is the one its last run left.** Each turn takes a slot of the role and resumes the
  session the role keeps at that folder on that road, so a round handed over after another goes on in the same
  session, as a delegated task does, until the agency lets that session go. The agent's own session is free
  from the moment it answered the sentinel: it takes its other rooms' rounds while the double works.
- **The round's own turn asks its slot as a short ask.** Its slot request carries `short`, the mark that vouches
  for one short turn someone is waiting on now, so where the role's delegated tasks hold every slot the agency
  lets it in on the one seat beyond the role's cap (the agency's `docs/role-slots.md`, 「A short ask」). No other
  ask of the receiver's carries the mark: a later turn of the process asks as a delegated task does.
- **The round's own turn waits only a short while for its slot.** A room's round is live conversation and never
  waits behind the role's delegated work, so where the slot its own turn asks for is not let in within the
  policy's limit (`chat-round.toml`, `[double]`, `admit_ms`), as where another short run or a run that is no
  delegated task holds the seat, the asking stops there: the slot's request is
  cancelled by its key and the double has not taken the round, whose reason says the double's role had no free
  slot within that limit. A slot let in within the limit goes on to start the double's session however long the
  start takes, so no session is ever cut half started. The receiver logs the wait as it begins and
  the hand-back once the limit passes. A later turn of the process waits for its slot however long the role's
  line takes, as a delegated task does.
- **Every slot the double holds is the receiver's own record until it is given back.** The slot's key is kept
  in the store before the slot is asked for and dropped once the agency took it back (`docs/store.md`,
  `double_slots`), so `quiet` counts a double's round as under way, and the receiver's start gives back every
  slot a stopped process left, with the role session bound to it, by the key it kept: no double's run outlives
  the receiver that followed it, and the rows its turn carried wait again as a restart leaves them.
- **Words that come meanwhile go to the double's turn,** pushed as they are pushed into the agent's own once
  that turn runs, and the turn's picture is drawn from the double's run. While the round's own turn still waits
  for its slot there is no turn to push into, so the room's words and the quiet instructions wait in the room's
  queue and reach whichever session takes the round: the double's turn once it runs, or the agent's own once
  the round is handed back.
- **A double that does not take the round hands it back.** Where its slot or its run is refused, its slot is
  not let in within the limit above, its turn fails, or it answers the sentinel itself, the agent's own session
  is asked once more, under one line saying the double did not take the round and why, to answer the round
  itself; that session holds the round already, so the line is the whole of the turn. The process then goes on
  in the agent's own session.
- **A hand-back stops the double and returns its pushes.** The double's turn, where it may still run, is
  interrupted and its slot given back as cancelled, so no run of the double goes on beside the agent. What was
  pushed into the double's turn — the room's words and the quiet instructions — was never received by the
  session that goes on, so it is not the round's: the words go back to wait at the front of the room's queue
  and the rows back to the conversation with their carrying cleared, to be pushed into the agent's own turn or
  shown at its next question boundary.
- **The sentinel is never a message.** An answer of nothing but the sentinel from that second asking fails the
  round as a failed draft turn does, and one from any later turn of the process is read as a skip whose reason is
  that the answer carried the hand-off sentinel.

## A voice note

- **The turn reads a voice note as text.** The receiver transcribes the saved recording before the turn
  (`docs/messages.md`, 「Every kind of message」), and the line stands as a voice message with its length and what
  was said. The room block tells the agent voice arrives as text. A recording that could not be transcribed is one
  line saying so, never nothing.
- **The transcript is not the message's words.** The archived text stays the service's voice mark and its caption;
  the transcript stands on its own column and the console reads it under the player.

## The questions

- **The questions are a catalogue, by name,** each a text the draft turn follows. The policy holds six, in
  English as every line an agent reads, each a rule with no sentence for the agent to copy. **A supervisor's
  catalogue is its own:** the policy's six, each in the text the console set for that supervisor where it set one,
  then the questions the console added for it under new names (「The routes」 below). A text set for one supervisor
  is read by no other, and nothing the console sets stands for every supervisor:

| Name | What it asks |
|---|---|
| `intent` | why the words were said and what the speaker wants; whether to answer is already decided, so this asks the reason |
| `information` | what the message needs to know and what more must be read; where what the session holds falls short and the brain may hold it, the brain's index opened or searched and read |
| `facts` | whether what the message rests on is true and where each fact came from: the system's live state read now from the service that owns it, nothing undecided spoken of as known, no variable guessed |
| `memory` | the brain brought up to date with its tools: the memories the answer needs recalled, similar ones among them; the facts established so far engraved; promises and checks fulfilled marked done; what the conversation showed no longer holds invalidated; what still holds confirmed, its `lastConfirmedAt` moved; and where the opening block's index carries the due line and the brain has unconsolidated episodes (`docs/bmo-window.md`, 「What every turn carries」), a consolidation in this turn: the first of the unconsolidated episodes the brain answers, as many as the turn can afford and never all of them, what each worth keeping comes to drafted, a consequential draft (one closing a live fact, or of importance 8 or more) reviewed by the agent's own second reading before it is written (`docs/unified-bmo.md`, 「The brain's consolidation」), each draft that passes written with `brain_learn`, and every one read marked with `brain_consolidated` but those of a draft the review stopped for a contradiction, with no `brain_fold` |
| `promises` | whether anything said like a promise was actually done; what was not done is done now or the words change |
| `final` | looking at what the turn established, write the message now, as this room's agent says it to this person, or send nothing; always the last |

- **The process's fixed rules and each question's own text stand in the room's block, not in a turn:** that the
  round's line names the questions the draft turn follows, each followed in the order named before the message
  is written, that the message is reviewed before it goes out and that a review which rejects it returns it to
  be done and written again, the confidence rule and the source rule above, that nothing is hedged, that
  a skip in its form ends the process and is for words that ask nothing and
  change nothing, that it names its reason on one line and never stands bare, so a person's telling the agent
  something of themselves, which it keeps, is answered in the
  same round, that a draft or a note is not a message the room has heard and never the reason for a skip, and
  the marks a message that arrived meanwhile takes (`chat-line.toml`, `[room]`, `process`). The process asks the
  agent for no progress line: what a working turn shows the room is read off its run as it goes
  (`docs/receiver.md`, 「The event stream」). The room's block
  also carries the texts of the questions the supervisor's process follows, out of the supervisor's own catalogue
  (`drafting.toml`, `[questions]`; 「The routes」 below); a question its row has switched off
  stands in no line of it (「The process table」 below). The block goes to a session once per version and by its
  mark after that, so a turn carries only what changes from round to round: which questions its round follows.
- **A session reads one set of instructions for the process, never two.** The process's part of the room's
  block is a supervisor's own and the same in every room it answers: its fixed rules, the stage lines, the voice a message is written in, the
  rules of the items and the checks, the agent's first judgement and the review's instructions. Where it is no
  longer what the last turn of an agent's one session
  across rooms carried, that session is not handed the new text beside the old: it is replaced through its
  handoff before its next turn, with the reason `process`, and the fresh session reads the block as it now
  stands (`docs/unified-bmo.md`, 「Replacing the session」). A change that leaves that part of the block as it
  was — the same row set again, the text of a question no list names — replaces nothing, and neither does a change set
  back before the session's next turn. A session an agent keeps in each room is the agency's to resume, and
  reads the changed block in full as a new version.
- **The round's line rides the round's own turn:** the room's words as the window composes them, then the line;
  the closing rules stand in the room's block. The line is the policy's (`drafting.toml`, `round`), the shortest
  that says what the block cannot: the names of the questions this round follows, in their order, joined by
  `+`. No turn carries a question's text, an instruction about how to follow it, or that the message is
  reviewed: those stand in the room's block.

## The process table

- **One table says which questions a round follows,** a row to a supervisor. A row's key is a provider, a tier
  and an agent, each a name or `*` for any: the policy's one row is `*/*/*`, and a supervisor's own row, which the
  console sets, names the supervisor under any provider and any tier. A row holds:
  - `rounds`, the questions every turn's round follows, in order, whatever the turn carried: a quiet instruction
    alone, a word of thanks or a long request alike. No turn is lighter than another, since what a supervisor does
    on a short word or a quiet instruction — a launch, a hold, a review's move, a deployment — is held to the
    same questions as anything it does;
  - `off`, the questions of that list that are switched off: each keeps its place in it and is not followed;
  - `stagesOff`, the stages after the message that are switched off, each by its name: `review`, the items
    `arrived`, `owed`, `owner_directive`, `record`, `wake` and `rule`, and the check `format`. None is switched
    off where the row names none, which is the policy's row and every row the console set without it.
- **A question switched off is not part of the process until it is switched on again.** A round follows its
  list without the questions `off` names, in the list's order: the round's line names only the questions
  followed, and the room's block carries no line for one switched off.
  Switching it on again puts it back at the place it kept. `final` writes the message and is never switched off,
  so a list that holds questions always follows at least that one. `off` names only questions the row's lists
  holds: a name the list does not hold is dropped as the row is set, so a question taken out of the list leaves
  `off` with it.
- **The message is written in the persona's voice.** The room's block says once, beside the stage lines, that
  the draft turn's answer is the message itself, in the voice the agent's persona sets for the person it
  answers, its register toward them included, and in their language, never as notes or a report
  (`drafting.toml`, `voice`); it holds however long the session runs, and the closing check carries the voice
  line again where a message already stands. The receiver carries no register of its own and names no person's
  register: that is the persona's.
- **The policy's one row is what a supervisor reads until it has a row of its own** (`drafting.toml`): `intent`,
  `information`, `facts`, `memory`, `promises` and `final`, in that order, for every turn.
- **The console sets a supervisor's row and no other** (「The routes」 below): a row is one supervisor's, read by
  that supervisor alone, and no row the console sets stands for every supervisor. The policy's row is the
  deployment's, changed in the policy file.
- **A round reads its row before its turn goes out,** since the turn carries the round's line: the row of the
  supervisor whose turn it is where the console set it one, and the policy's where it did not.
- **A list that holds questions ends in `final` and holds it nowhere else.** An empty list follows nothing: the
  round's turn goes out as the window composed it and its answer is the message, held to the items, the review
  and the checks below that the row does not switch off.
- **A stage switched off is not asked, and the room's block carries no rule for it.** The review switched off
  sends the message as the draft turn, the closing check and the items left it, with no review row and no
  revision; an item switched off is not asked whatever the code's reading of the message says; the `format` check
  switched off reads nothing of the final text and withholds nothing. The process's fixed rules in the room's
  block stand whatever the row switches off. A row that switches off every question and every stage makes the
  draft turn's answer the message as it stands, with no further turn.

## A supervisor whose brain or reference is off

- **A supervisor's brain switch and reference switch are the agency's settings,** `brainOn` and `referenceOn`
  among its settings (the agency's `docs/supervisors.md`, 「Its settings」), both read by the receiver from the one
  settings page, asked afresh as a round composes its turn; a switch its settings do not say, a supervisor the
  agency does not know, or an agency that does not answer reads as on, so every round runs as it did.
- **Where either is off, nothing of the brain reaches the supervisor's turns from the receiver:** the reference
  switch off closes what the receiver carries exactly as the brain switch off does, since the index, the core and
  the rules about them are the turn's reference:
  - the room's block carries no index, no hint and no due line, and none of the block's rules about the index
    (`docs/bmo-window.md`, 「What every turn carries」);
  - the turn's core is not asked of the gate's reference selection, so no sheet, memory, errand or woken row is
    attached; a person's own thread, which is the room's record, still goes in;
  - a round handed to the double carries the room's block as composed above.
- **The tools that reach the brain are the agency's to leave out** of the supervisor's session record under the
  same switch (the agency's `docs/supervisors.md`). The process asks no question about the brain by itself: a
  row that keeps `memory` or `information` while the brain or the reference is off leaves the agent to answer
  them with what it holds, so a supervisor run with either off switches those questions off on its row.

## The situation note

- **The note is an analysis of the room, and it sends nothing.** Its turn writes the note and nothing else: it
  answers nobody and writes no reply, and nothing of it goes to the room. It says what the room is about, what
  has been talked about and settled so far, who is talking with whom, who waits on whom, what the agent has
  itself been asked or promised, and what is still to be done, and then it judges what came in against that
  state: what it changes and how the room is best answered, as the course to take and never as the message.
- **The note says what is talked about and what is owed, never what went out.** It never says that a reply was
  sent, attached, fixed or answered: where it would say something was
  sent, it says what remains to be done. It never copies a message or restates the agent's own answer, and it
  holds no first-person claim that a reply, a fix, an attachment or a send happened, whether the claim is fresh or
  copied. A note that copies a shown sentence or makes such a claim is refused, and the note as it stood stays.
  Only a message the room's record holds counts as sent.
- **One note a room, the receiver's own.** The note stands in `room_situations` (`docs/store.md`), one row a room,
  replaced when it changes, with the row of the room's record it has read to. It is the room's live state, not the
  agent's long-term memory: the brain's `situation` rows are rules for a situation, while this note is written and
  read by the receiver alone. It is no reference document and stands nowhere in the turn's core.
- **The note is written when a session leaves a room, never by a round.** Three moments write it, each one turn a room on the agent's one session across rooms:
  - **another room's words:** where a round with words opens in a room and the agent last answered in another,
    the note of the room it last answered in is updated first, and then the round goes on in the new room;
  - **a handoff:** before a session is asked for its handoff file (`docs/unified-bmo.md`, 「Replacing the
    session」), the note of every room it answered in is updated. The note is the room's own and is no part of
    the handoff document;
  - **a keepalive touch:** where the cache warmer's touch reaches the session (`docs/bmo-window.md`, 「The
    touch」), the note of every room it answered in is updated, and those turns are the touch: the keepalive
    prompt goes in only where no room is owed a note.
- **A room is owed a note from the agent's answer in it until its note is updated.** The receiver keeps, for
  each agent, the rooms it has sent a round's message in since their note was last updated, in the order of the
  last answer (`<agent>_notes_owed`, `docs/store.md`): the last of them is the room last answered in, and all of
  them are the rooms a handoff and a touch update. An update takes its room off the list whatever its turn
  answered, and a room whose record holds nothing past the note's mark is taken off with no turn. A turn whose
  run failed leaves its room owed, and so does a session that ended with no handoff asked: the update reads the
  room's record, never what a session remembers, so whichever session stands at the next of the three moments
  writes it. A session an agent keeps in each room leaves no room, is handed over by no handoff of the
  receiver's and takes no touch, so its room's note is written by none of them.
- **The update turn reads only what the note has not.** The note's instruction stands
  in the room's block, the session's system prompt, beside the draft process's stages, assembled from the
  situation policy with its ceiling rendered into the text (`chat-round.toml`, `[situation]`, `instruction`,
  `ceiling_chars`). The turn itself carries only a short mark that this is the note turn (`marker`), the note as
  it stands and the room's messages that came in since the note last read the record, the agent's own answers
  among them, under the policy's labels;
  its answer is the note, and the row of the last message it read to is kept with it. The note as it stands goes
  whole where the session has not read that version, and as the policy's one-line marker where it has
  (`chat-round.toml`, `[situation]`, `unchanged`; the version is the note's text, kept under
  `bmo_session_notes`, `docs/store.md`). Where the room has no note
  yet, it carries the latest messages the policy names (`recent_messages`, 30) and takes its first position at the
  newest of them. A turn that answers `<<SKIP>>` or `<<WAIT>>`, or nothing, leaves the note's words as
  it stood but takes its position forward with what it read. Where more messages stand between the note's mark
  and the newest than `gap_messages` (200), the update reads the newest `gap_messages` alone instead of the whole
  gap — a room left long is caught up from its latest words, and the mark moves to the newest of them; a
  `gap_messages` of 0 reads every message since the mark.
- **A session reads a room's note once.** The round that first takes a session into a room — the session's
  first round there, which is its first round of all or one in a room other than the room it was last in —
  carries the room's note under the policy's line: in the judgement turn, before the room's recent words and the
  words judged, where the round is the agent's to judge first, and otherwise at the head of the draft turn,
  above the room's words. Every later round of that session in that room carries none, however the note has
  changed since: the session holds what it read there and what it has itself said and written. The receiver
  keeps which rooms' notes a session holds (`bmo_session_notes`, `docs/store.md`), a note it wrote among them,
  and lets the record go where the session is replaced, folds or a new generation opens under it. The fresh
  session then reads each room's note once again, which is how what a replaced session knew of a room reaches
  the session after it.
- **The note changes the material a round reads, never an addressee rule:** a conversation between others is
  left alone, and a line that asks the agent or resumes an earlier thread with it is answered.
- **The note stays under `ceiling_chars`, 600 characters.** A longer answer is cut at the ceiling and marked with
  the policy's `cut`; the instruction in the room's block names the ceiling, and the ceiling leaves room for the
  state, what came in and the course to take together. The cost is one turn for a room left, never one a
  round: the note it reads, under the ceiling, and the room's messages since the note last read the record. A
  room with no note yet pays the `recent_messages` window once.
- **The console reads the note beside each judgement.** The note as it stood when a round opened stands on every
  message that round judged (`docs/api.md`, the `Message`'s `situation`), so a message's judgement record
  says what the room's situation was as the round found it; its `changed` is false, since no round writes the
  note. A room answers the note as it stands now, with when it last changed, at `GET /rooms/{roomId}/situation`.
- **An owner's directive is not skipped because a note or an earlier draft says it was handled.** The
  round's answer to the owner's words is asked over once more where no message the room's record holds follows
  those words; a note, a draft or a turn of this session is not a sent message.

## What came in meanwhile

- **Words that come while the draft turn works go into it.** Words the room queue hands over while the draft
  turn works are pushed into it at its
  next look (`docs/bmo-window.md`, `push`): the paragraph carries the push's marker and the words as data, what
  the marker means standing in the room's block, and the agent's program takes them between tool calls. The
  turn takes the quiet instructions delivered meanwhile in the same paragraph.
- **What no push put in reaches the agent in the closing check.** Words the
  turn refused or whose provider takes no words while it works, and words that came while the push was held back
  wait in the room's buffer. Once the draft turn's answer comes back, the receiver takes what waited
  and shows it in the closing check as one block, composed as a push's paragraph is but for the
  rule each message is judged by, which the room's block states: each message stands with its number.
- **Pushed or shown in the closing check, the words keep the round's privilege,** read by people's words alone as the
  round's own is (`docs/unified-bmo.md`, the run-privilege mark): a round that is the owner's takes them where every
  person's words among them are his, an agent's line neither keeping them out nor letting them in; a round that is
  not his never takes words that are his alone. The others wait and open the round after, where the owner's own
  words carry his privilege.
- **Once the message stands, one closing check.** Where words waited when the message came to stand, and the
  row does not switch off `arrived`, the session is asked once more before the items and the checks: the
  marker that names the check (`arrived_marker`), the message as it stands, then the block of what arrived. What
  the check does stands in the room's block (`arrived`). The answer, marks taken out, is the whole
  message as it should now go out, answering the words that arrived as well, and it takes the standing message's
  place: nothing is joined to anything, so no part of the message goes out twice. An answer that is nothing but
  marks sends the standing message unchanged, a wait leaves it too, and a skip sends nothing; the marks keep their
  meaning for the messages shown. The check is not asked where the process came to nothing to send: those words
  wait for the next round.
- **The agent decides each message it was shown, by mark,** a message pushed into the draft turn being shown
  to the round as one in the closing check is:
  - by default the round's message answers it: it is the round's own from then on, the read mark moves to it, and
    the owner's words among it are owed an answer;
  - `<<DONE #<number>>>`: it needs no answer: the read mark moves past it, nothing is sent for it and nothing is
    owed it;
  - `<<SEPARATE #<number>>>`: it is answered in a round of its own: once the round's message is made it goes back
    to the front of what waits, and the round after takes it with a pregate of its own;
  - `<<REPLY #<number>>>`: it is the message the answer answers, and the answer goes out as a reply to it; the
    agent sets it by the rule `docs/wake.md` 「Replies」 states, which the process's lines restate.

  The number is the message's own as the round showed it, its minus sign kept where it is negative. A mark counts
  in any answer of the process, outside a code span or fence, and only for a message of the round, its own or one
  shown to it; a message marked both done and separate is separate, and of several `REPLY` marks
  the last stands. On the round's own messages a done or separate mark counts only where several people said
  them (「Words of several people」). Marks are taken out of an answer before anything else reads it, and never reach the room.
- **Each message shown to the round carries a verdict of what its mark made of it.** Once the round's message is
  made, every message pushed into the draft turn or shown in the closing check is written down under itself as
  a verdict of the stage `read-with`, naming the message the round's own verdict stands under: `READ: #<number>`
  where it became the round's own, `SKIP: #<number>` where it was marked done, the number left out where that
  verdict stands under no message. A message marked separate is written down by the round that takes it, as its
  own words are.
- **The answer is pinned only to the message the agent marks.** A `<<REPLY #<number>>>` or `<<REPLY:<number>>>`
  mark names the message, the round's own or one older than it, and the answer is pinned to it; a bare
  `<<REPLY>>`, or the number zero, names the newest message the round answers; a mark the supervisor's judgement
  set stands where the process's answers set none. With no mark the answer goes out plain, whatever the round held,
  whoever spoke since and whatever the owner is owed: the process never picks a message to pin by itself. Where
  the road cannot aim at the message, or the room's record does not hold it, nothing is pinned.
- **The fold is asked by the message itself.** `<<FOLD>>` is no mark on a message shown to the round: it stands in
  the message's words, so the answer that becomes the message folds where it carries the mark and goes out
  unfolded where it does not, whatever an earlier answer of the process carried; the agent sets it by the rule
  `docs/wake.md` 「Folds」 states. An answer whose words are nothing but marks, the fold among them, is an answer
  of marks alone: to the closing check it sends the standing message as it stands.
- **The record keeps the round; the pin is the visible reply.** The answer's row names the message as its
  `replyToMessageId` only where the pin stands; the round stands as it was, and a turn with no pin shows no reply
  in the console or on the chat service. The answer is still the round's answer, whatever the visible pin.
- **Words of several people are each answered or set apart.** Where the round's own messages come from more
  than one person, the turn says so under them (`chat-round.toml`, `[blocks]`, `people`, with the count of
  people): the message answers each of them, and one that needs no answer or an answer of its own is marked
  done or separate. Once the message is made, each of the round's own messages is written down under itself as
  a `read-with` verdict as a message shown to the round is: `READ` where the message answers it, `SKIP` where
  it was marked done. One marked separate leaves the round and goes back to the front of what waits, opening the
  round after, which writes it down as its own words; where every one of them was marked separate, none is set
  apart, since a round sets none of its words apart from itself. So no person's words in such a round end with
  no answer and no record of why. A round of one person's words reads no done or separate mark on them.
- **Nothing else opens a round for a message shown inside a round.** Words that come after the closing check, or
  while the closing check, an item, the review or a check works, are pushed into no turn, were shown to nobody and
  open the next round.
- **A turn whose run failed showed nothing:** the block it carried and the words pushed into it go back to wait,
  and a process that is stopped gives back every block it showed and every word pushed into it.

## Time

- **No clock and no count bounds a round.** The draft turn works until it has its message or the agent skips
  deliberately; no turn has a time bound, no round has a cap, and no tool call is counted. A
  round that takes a long time still ends with its message, and a failing check gets the repair round it would
  get at any other time. The review's eight rejections are the one bound, and they send the last draft rather
  than silence; a review that gives no guidance or repeats itself ends the review early the same way. Nothing
  else makes the room hear silence but an answer that says so.
- **Before the draft turn has answered, no message stands.** A process that has its message goes on to the
  closing check, the items, the review and the checks below.

## The items after the message

The code's own readings of the message are items of the process, each asked where its row does not switch it
off, once the message stands (the draft turn's answer) and after the closing check where one was asked
(「What came in meanwhile」 above), each asked once as one more turn of the same session. The turn carries only
the marker naming the item or the check and the data its rule reads; the rule itself stands in the room's block
(`second-asks.toml`, `[errands]`, `[data]`).

The `record`, `wake` and `rule` items ask only for the recording: their answer is the short mark `CHECK`, never a
message, and the message that stands goes out unchanged with nothing about the recording said in any room. `owed`
and `owner_directive`, asked only where no message stands yet, answer with the message itself.

- **`owed`:** where the process ended with no message and the round read the owner's own words in the owner's
  own chat, the session is asked once more with the silence withdrawn (`docs/wake.md`, 「The owner's own chat is
  always answered」); a skip again sends nothing.
- **`owner_directive`:** where the owner's words of the turn have no sent answer, the session is asked once more
  for it. The owner's words are those filed under the owner's sender, never words read as the owner's for having
  come through the console: another account's console words stand under its record identity and carry no
  directive (`docs/receiver.md`, 「Console rooms and accounts」).
- **`record`:** the message promises what the agent will do from now on and no turn of the process wrote it down
  (`docs/answer-checks.md`, 「A promise stands on a record」).
- **`wake`:** the message promises to act once something happens and no turn of the process set what wakes the
  agent when it does.
- **`rule`:** the owner's words of the round set a rule for a situation to come and no turn of the process filed
  one; asked only where the turn's tools are on.

## The turn's mark

- **A turn hands its material once.** The main round hands the core's documents and references and the room's
  words in full. Every later round of the same turn carries each of those documents as a line naming its path and
  the room's lines as one line naming their numbers; a line or an entry the turn has not handed yet goes in full.
  A round that arrives with new words carries those words whole and then marks them for the round after.
- **The mark is the turn's, not the session's.** It is kept under the turn's round name, separate from the
  session's loads and the session's own marks, and is let go when the turn ends. A later turn starts with no turn
  mark, so nothing is assumed about what a new turn has read; the session mark still governs what the session
  holds across turns.
- **Nothing unread is dropped.** A piece is marked only once it has gone in the main round, and a piece that was
  not, or a room line that arrived since, is written whole.

## The review

- **The review audits the draft turn before the message goes out,** where the round's row does not switch it
  off (「The process table」 above). It is one more turn of the same session, the first of the gate, and it audits whether the draft turn did what it was asked: whether each question its
  line named was carried out — what was to be read was read, each fact has its source, the brain was brought up
  to date, what was promised was done or the words changed — and whether the message stands on that work, on
  the room's words and on the rules and knowledge the round carries. What the review is asked, the verdict it
  answers and what a revision does with the reason stand in the room's block beside the stage lines
  (`drafting.toml`, `[review]`, `instruction` and `revise`): it answers `PASS` alone, or
  `REJECT: <one short reason>` naming the one question not carried out or the one thing that must change, and
  nothing else. The review turn itself carries only the marker that names the round, the questions the draft
  turn followed, the draft, the room's words, the people the message answers and the rules it carries
  (`marker`, `followed`, `draft`, `words`, `answers` with a `person` line each, `rules`); it is handed no
  instruction sentence and no verdict wording.
- **Each person is addressed as the brain holds.** The people the message answers are every sender of the
  round's words and of the words shown to it, each once by the name they go by and their sender number: never
  the agent itself, and never the sender of words the answer marked `<<DONE>>` or `<<SEPARATE>>`; a round whose
  words are a quiet instruction alone answers no one and shows no such head. The review holds the draft to the
  address term the brain keeps for each of them, their `name` memories and any rule or knowledge on how they are
  called, and to the register the persona sets toward them; where the reference the round carries shows none of
  a person's names, the review reads their name memories with its brain tool, the one reading it does, and where
  the brain holds none the name they go by in the room's words stands. A draft that calls a person by another
  name or title, by one their names say not to use, or in another register is a reason to reject, the reason
  saying the term or register the brain holds. The receiver reads no name of the brain itself: what the review
  is shown of a person's names is the reference the gate attached and what its own tool reads.
- **A thing said not to be possible, and a step put on the owner, stand on an attempt.** Where the message
  claims a road, a tool, a permission, a file or a capability is missing or cannot work, or asks the owner to
  do a step, the review checks that the draft turn actually tried it with its tools and that the message says
  what it tried and what answered; a step the round could take itself, or a thing called impossible on no
  attempt, is a reason to reject.
- **A rejection returns to the draft.** The session is asked again, its turn carrying only the reason
  the review gave: it does what the reason names as not done, with its tools, and writes the message again, and
  the new draft is reviewed again, eight times at most. At the eighth rejection the last
  draft is sent, not withheld, and the round's end row reports the reasons in order. A review that gives no
  verdict is a rejection with its own words as the reason. A review's answer that skips is the review giving no
  verdict too, and is never the round's own skip: one naming a reason is a rejection carrying the reason it names,
  and one naming none is asked once over as any round turn's is and, naming none again, is a rejection whose reason
  is that the review gave no verdict.
- **A review that writes the message again gives no reason.** An answer of the review opening on `<<SEND>>` is
  the message as the review would send it, not a reason for a revision: where it is the draft unchanged, white
  space aside, the review passes; otherwise it takes the draft's place and is reviewed again, its row a rejection
  whose reason says the review wrote the message again, counted among the eight.
- **A rejection that gives no guidance ends the review.** Where the reason is empty, or is the draft's own
  text after trimming and collapsing white space, no revision is asked: the draft as it stands is sent
  and the end row says the review gave no usable reason. The same holds where two consecutive rejections carry
  the same reason, since the revision is not converging; the draft as it stands is sent and the end row says
  the review repeated itself. A reason names a change where it is not empty and not the draft itself.
- **Every verdict and reason is written down.** A review and its verdict are rows of the round log,
  `PASS` or `REJECT`, with the reason; the console reads them with the rest of the round.
- **The format check runs after the review passes.** A format failure gets the repair round it had before; a
  review rejection has already gone back to the draft.

## The checks on the final text

- **Once the review has passed, or where the row switches the review off, the text is checked before it is
  sent,** with no model, each check the row does not switch off:
  - `format`: none of the process's own lines stands in the text: no text of a question the round followed, not
    the round's line, and not the head of the block of words that came meanwhile.
  There is no length check, and no check reads the language an answer is written in: that is the agent's own
  judgement (`docs/answer-checks.md`, 「The language an answer goes out in」).
- **A turn the process cannot do without fails the round where its run fails:** the review, the revision a
  rejection asks for, and a check. The round then gives back what it took, as any failed round does
  (`docs/bmo-window.md`, 「Quiet instruction」), and no message goes out unreviewed or unchecked. An item's turn
  whose run fails gives back the words it showed, and the process goes on without it.
- **A check that fails is asked once:** one more turn of the same session, the check's errand naming what is
  wrong, its answer the text from then on. The check then reads the new text. A `format` check that fails again
  sends nothing, and the end of the process records the check that failed. A check that fails only after
  another's asking is asked once in its turn.
- **A skip in the answer to an item or a check** ends the process with nothing sent, as a skip in the draft
  turn does.

## After the process

- **The message goes to the round's send** (`docs/wake.md`, 「The room's queue and the round」): the words'
  form reads it there, once. Once it went out, the window restarts, the read mark is written and the fold
  reads the context the last turn of the process left; a message no try of the send put in fails the round, its
  words left unread for the next.
- **Nothing the process asked goes to the room** and none of it is archived: the questions and their answers are
  the session's own and the round log's.

## The round log

- **Every turn of the process leaves a row of the receiver's round log** (`draft_rounds`, `docs/store.md`): the
  round it belongs to, the room, the agent, its number (from 1, the draft turn and then each item, review and
  check in the order asked), its name, its kind, how long its answer is in characters, whether the answer differs
  from the one before it, how it ended, the answer itself and how long the turn took.
- **A turn that spanned several turns of the agent's program keeps each one's answer on its row,** its
  `versions`, in order and with what opened each (「What the receiver reads of an answer」), so the round log shows
  every version the turn wrote beside the answer built from them; a turn that spanned one keeps none.
- **The round's own turn is the draft turn's row:** number 1, of kind `question`, named for the questions it
  followed, in their order and joined by `+` as the round's line names them, its time counted from the round's
  start. Its kind is `turn` only where the row's list is empty and the turn followed no question, its answer
  being the message either way.
- **A round the agent judges first** (`docs/wake.md`, 「The room's queue and the round」) writes that judgement
  as a row before the process's own: number 0, of kind `supervisor`, named `supervisor`, ending `skip` where the
  round ends there, `answered` where it goes on, or `no-verdict` where the answer skipped naming no reason. The
  asking once over that a bare skip gets is a turn of the judgement's own: it leaves the next row, of the same kind
  and name, whose `detail` carries the reason where the round ended in a skip. A round that ended at its judgement
  writes no row beyond these.
- **The end row names what the round read,** its `reads`, each a `path`, what the piece holds
  (`holds`), the brain memory it is (`memoryId`) and how it was read (`how`). `holds` is the index's words for the
  piece where the gate's index lists it, else the name the piece came with: a piece of the turn's core carries the
  name its maker gives it, the gate's where the gate names one and the policy's word for the gate's sort of piece
  otherwise (`chat-round.toml`, `[reference.holds]`), a person's thread the thread's own (`[thread]`, `holds`), and
  the room's situation note the situation policy's (`[situation]`, `holds`). A thread whose lines were all handed to
  the session before the round stands as `held` by that path, with no block of its own in the turn, and a note the
  session holds at its version stands as `held` the same way. `memoryId` stands where the piece is one memory of the brain: the id the gate's index
  gives the entry, else the one its path names, `memories/<id>` or `memories/<room>/memory-<id>`; the room's due
  list, `memories/<room>`, and every other piece carry none. `how` is `attached`, a piece of the turn's core that
  went in full; `held`, a piece of the core that went as a mark because the session already held that version, and
  a piece the agent read itself
  through the index at a version the session already held before the round's turn; or `read`, a piece the agent
  read itself through the index during the round at a version the session did not hold. A read through the index is
  a load the session gained or changed between the round's turn and its end (the agency's `GET
  /session-loads/{sessionId}`), or a call the round's runs logged whose input names an entry of the index by its
  `group` and `slug`, so a piece read again stands once, `held`. The console's 브레인 chip and the round's line in the
  draft log read them.
- **The end row names the session the round's turn ran on and what that session had read in this room before
  it,** its `session` (null where the turn ran on none) and its `loaded`: the pieces the room's earlier rounds of the
  same generation of that session read in full. `session` is the agency's id of the session, which a new
  generation keeps, so the generation is told by a cut: wherever the receiver releases what an ended generation
  held (`docs/unified-bmo.md`, 「Replacing the session」), as a session it replaced or a generation the agency
  opened under it, it marks the round log's last row as the session's cut (`loaded_cut:<session>`,
  `docs/store.md`). `loaded` is the `loaded` of the end row of the same room and the same session written last
  before it and after that session's cut, plus that row's reads whose `how` is `attached` or `read`; a `held` read
  counts nothing, and where no such row stands, or the turn ran on no session, it is 0. So no count carries across
  sessions or generations: a new session, and a new generation of the same session, start at 0, and the console's
  브레인 figure reads as the running count, `loaded` and then `+` this round's full reads. Every row but the end
  carries neither.
- **The kinds are `supervisor`, `turn`, `question`, `item`, `check` and `end`;** the closing check is an item
  named `arrived`. A turn's row ends `answered`, `send` where its answer opened on `<<SEND>>` and became the
  message, `no-verdict` where it skipped naming no reason and the same turn was asked once over, `skip` or `failed`.
  A row ending `skip` carries in its `detail` the reason its skip named, or `none given` where the turn named none
  twice and the round ended for that.
- **The process's end is a row of its own,** of kind `end`: `message` where a text goes to the send, `skip` where
  nothing does, `withheld` where a check failed twice, and in its detail the row of the table the turn read, the
  reason the round skipped for where it skipped (`; skipped: ` then the reason, or `none given`), and the checks
  that withheld it.
- **Each row is told on the event stream as it is written,** a `draft` event (`docs/receiver.md`, 「The event
  stream」), and the log is read by round (`GET /rounds/drafts`): the rounds a reader names, as a screen names
  those its messages stand under, in the order of their names; or, where it names none, the room's newest rounds, at most
  `limit` of them (20 by default, 200 at most), oldest first, the page's `more` saying older rounds stand.
- **The draft turn is told as it starts, too,** before its answer: a `draft` event of kind `start` with the
  `round`, the `agentName`, `number` and `count`, both 1, `question`, the name its row goes by, and `at`. It is
  no row of the log, and the turn's own row follows it once the answer is in. A screen tells the message being
  written by it; items, reviews and checks are told only as rows.
- **The gate's `draft` stage and the console's draft chip read this log.** The gate takes every `draft` event of
  a row off the event stream and writes it as a judgement of its `draft` stage; a `start` event is no row, and the
  gate lets it go by its kind (`docs/boundary-api.md`, 「Contract gaps」).

## The supervisor's opening

- **The room's block carries the shape of a turn and its rounds.** One short line names a **turn** as one pass over
  a room's words and its **rounds** as the turns the session is asked inside it, this round's rule (a skip in its
  form or one short line, and no other action), and, for the later rounds, only the rulebook each follows: the room
  block's draft process for the draft round, the review's own line and each item's rule there. It names them
  there so the agent knows which rule applies at
  each round without the prompt repeating texts that stand elsewhere. The judgement turn itself carries a marker,
  the room's note where the session has not read it, and the room's words as data; the block says what the
  marker asks, the judgement's head and question with it.

## The routes

- **`GET /draft-processes`** answers every row of the table, the policy's and then each supervisor's by name,
  each with its `source`: `set` where the console set it, `default` where the policy's stands. With
  `agentName` it answers the one row that supervisor reads: its own, or the policy's where it has none.
- **`PUT /draft-processes/{provider}/{tier}/{agentName}`** sets a supervisor's row and answers it as it now
  stands: the provider and the tier are `*` and the agent a supervisor's name, and any other key is refused
  `bad-request`. `off` and `stagesOff` are left out for none. A list naming a question the supervisor's catalogue
  does not hold, a list that holds questions and does not end in `final` or holds it before the end, an `off`
  that names `final`, or a `stagesOff` naming anything but the stages 「The process table」 names is refused
  `bad-request`, and nothing is written; a stage named twice is kept once, in the order the table names them. A row that still carries `light`, `lightChars`,
  `sendFrom`, `turnSeconds`, `ceilingSeconds` or `pastCeiling` loads with them ignored.
- **`DELETE /draft-processes/{provider}/{tier}/{agentName}`** takes a supervisor's row away, and the supervisor
  reads the policy's again. A key the console set no row for is `not-found`.
- **`GET /draft-questions`** answers a supervisor's catalogue where `agentName` names one, and the policy's
  alone where it names none, each question with its `source`; **`PUT /draft-questions/{agentName}/{name}`** sets
  a question's text for that supervisor, a name being a lowercase word of letters, digits and dashes, and a text
  at most 4,000 characters and not empty.
- **`DELETE /draft-questions/{agentName}/{name}`** takes the console's text of a supervisor's question away. A
  question the console added leaves that supervisor's catalogue; one the policy holds stands again in the
  policy's text, and so does the agent's first judgement. A question the console added that a list of the
  supervisor's row still names is refused `conflict`, since the list would name nothing: it is taken out of the
  lists first. A name the console set no text for is `not-found`.
- **A supervisor's catalogue holds its first judgement too,** first of all, named `supervisor`: its text is what
  the room's block says the judgement asks, the round policy's head and question as one text where the console
  set none for that supervisor (`chat-round.toml`, `[supervisor]`, `rule`), and the judgement turn carries the marker, the room's note,
  where the session has not read it, its recent words and the words judged as data. It is set like any question,
  and no list of the table may name it, since it is asked before any process.
- A change reaches the next round of the supervisor it was set for; no round under way changes, and no other
  supervisor's does. Where it changes the process's part of that supervisor's room block, the supervisor's one
  session is replaced before its next turn (「The questions」 above).
