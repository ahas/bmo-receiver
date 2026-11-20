# The instruction pipeline

How a round's message is made and then checked: the round's own message turn, whose answer is the message; the chain of
instruction stages the supervisor sets in its settings, which the message passes before it goes out and where the agent
reprints the message or answers PASS; the markers that end the round early; the words that arrive while it works; and the
record each turn of it leaves. A quiet instruction is a different thing — a row of the instruction table (`docs/bmo-window.md`,
「Quiet instruction」), which this pipeline leaves unchanged.

## What reaches an agent

- **What reaches an agent is a message or an instruction** (`docs/bmo-window.md`, 「A message and an instruction」). A
  message is a room's words, and the turn that carries them is the round's message turn. An instruction is a quiet one the
  system hands the session, or a system instruction the round itself opens: the audit of the people the message answers,
  each instruction stage of the chain, and the turn that shows the words that arrived meanwhile.
- **The room's block says how to read each and how to answer each, and nothing more.** It carries, once: how a turn that
  carries a room's words is read and that its answer is the message (`instruction.toml`, `message_rule`); how a turn marked
  `INSTRUCTION` is read, the system speaking and no room, one instruction about the message, which is the agent's last
  answer (`instruction_rule`); how an instruction is answered, the whole message as it should now go out or `PASS` alone where it
  needs no change (`answer_rule`, whose `{pass}` mark fills to `pass`); and what the turn marked `ARRIVED MESSAGES` asks
  (`arrived`). It describes no chain, names no stage and carries no stage's words, so the block is the same for every
  supervisor whatever its chain.
- **An instruction's words ride its own turn.** A stage's turn opens on the line naming the stage (`marker`, its `{name}` the
  stage's name), then the stage's words and the round's data (「The stages before the send」 below).
- **The message is the agent's own, and no later turn of the round carries it again.** The agent's own output is what goes
  out, so an instruction is about the last message the session wrote in the round: its last answer, or the one before it
  where it answered `PASS` since. A round holds the agent's session from its message turn to its end, the summary of a
  notice turn included, so no other room's turn comes between and the message is always the session's own; a stage, the
  audit, the closing check and the summary carry it no more (`docs/voiced-notices.md`, 「The summary of a notice turn」).
- **A screen tells a message received only on the message turn.** Each turn of the round is told on the event stream as it
  starts with what reaches the agent on it, `message` or `system` (「The round log」 below), so a screen showing a system
  instruction shows no message received for it.

## What it is

- **A round is the agent's judgement where one is asked, and then one message turn and its stages.** Where the round is
  the agent's to judge first (`docs/wake.md`, 「The room's queue and the round」), its first turn is that judgement,
  which asks no reference, reads no index and runs no gate; a skip ends the round there. The room's situation note is no
  step of a round: it is written when the session leaves a room, before a handoff and on a keepalive touch, and a session
  reads a room's note once (「The situation note」 below).
- **The message turn is one turn, and its answer is the message.** A round's turn goes to the agent as every round's
  does (`docs/wake.md`, 「The room's queue and the round」), after that judgement where one is asked, and after the
  window's pregate: the core the gate attaches, where the supervisor's brain is on (「How a supervisor reads the chain」
  below). The brain's index rides the room's block at session opening, never the turn. The turn carries the room's words
  as every round's does; in that one turn the agent thinks, reads with its tools what the message needs, engraves what
  it establishes in the brain, and then writes the message as its answer: its output is the message. Nothing stands as a
  message before that answer.
- **Once the message stands it passes the stage chain.** The chain is an ordered list of instruction stages the supervisor
  sets; each stage is one more turn of the same session, a system instruction carrying the stage's words and the message as
  it stands. On a stage the agent answers with the whole revised message, or answers `PASS` alone when it changes nothing
  and the message that stands goes on unchanged. After the last stage the agent's final response
  is the message that goes out. A supervisor with an empty chain sends its message unchecked.
- **The instruction pipeline is the only road the agent's words take into a room.** A round's message is the one text an
  agent puts in a room, and it goes out only once the pipeline, its stages included, has let it through. No send carries
  words: a line, a spoken talk, a voice note or a caption asked of the send tool or a shell verb in the round's own room
  is refused, and one asked into another room or from outside a round is handed to that room's supervisor, whose round
  there writes and reviews it (`docs/messages.md`, 「Words handed to the room's supervisor」). A progress line is no
  exception: while work runs, nothing is said until the message. A message its round asked said aloud with `speak`
  takes the same road and goes out, once let through, as a voice note saying it (`docs/voiced-notices.md`, 「An
  answer said aloud」).
- **Every tool the round's turn has is open in the message turn:** reading and writing memories, searching, filing a job,
  and the sends that carry no words. A question to read more is followed by reading; a question whether a promise was kept
  is followed by keeping it.
- **Every later turn of the pipeline is a turn of the same session,** opened as `question` (`docs/bmo-window.md`,
  「The openings」), so it holds the round's intent, its reference and its cache: a stage turn and the closing check.
- **The receiver reads three things of an answer and nothing else:** a skip standing in it outside a code span or fence,
  which ends the round with nothing sent; the marks `<<DONE #<number>>>` and `<<SEPARATE #<number>>>` on a message shown
  to the round (「What came in meanwhile」 below); and the answer itself. `<<SEND>>` opening an answer is taken out of it,
  the answer being the message with the marker or without, and a marker with nothing after it names no reason. A skip
  anywhere in an answer wins, and where an answer carries skips naming reasons and not, one that names a reason wins.
- **A later turn of the program that says nothing leaves the answer written before it.** One turn of the pipeline can span
  several turns of the agent's program: words pushed after its last tool call each open one, and the agency hands each
  one's answer with what opened it (its `docs/agent-control.md`, 「The round's answer」). The receiver builds the answer
  from them as the agency does — a turn opened by words handed over replaces it, a turn the program opened on its own adds
  to it — save that a turn whose answer says nothing, a skip of either shape, a wait or no words, never displaces an
  answer that says something: the answer written before it stands and is read as above. A line that carries its program's
  background apart, as the chat line does, reads its rounds with notices apart: a turn its program opens on a background
  task's notice is no round's, so the agency hands no such turn, and the work's end reaches the room as an instruction of
  its own (「Long work in a round」 below). So a turn opened by a quiet instruction alone that skips does not take away the
  message written before it. A cancellation pushed into the turn is answered with a reply saying so (`chat-round.toml`,
  `[push]`), never with a skip. Where the agency hands no turns, the answer it built stands.
- **A stage's turn answers `PASS` alone when it changes nothing** and the message that stands goes out unchanged; where
  the agent answers `PASS` it is the answer with the stage, so a stage never rewrites the room's answer. A revision takes
  the message's place. A turn whose answer is nothing but `PASS` leaves the standing message unchanged. A skip in a stage's
  answer, or a wait in it, changes nothing: the round's skip decision belongs to its message turn, and the message that stands
  goes on through the chain. A message turn that skipped naming no reason is asked once more under the line naming the form,
  the session holding the round already; a second answer naming none ends the round as a skip whose reason is recorded as
  `none given` (`instruction.toml`, `skip_again`).
- **The bare marker stands where a rule gives it whole.** The handoff turn, a consolidation turn and the situation-note turn
  are each told to output exactly `<<SKIP>>` and ask no reason, since nothing of them goes to a room and what they leave
  unsaid is settled by their own rule; the reader of those turns takes a skip naming a reason as the same nothing to say.
- **A revised message is not a new answer the room has heard twice.** The room's messages are the only record of what the
  room has heard; a stage's answer is the message that goes out, and it is archived once when it goes out.
- **A round whose message turn answers `<<WAIT>>` runs no further pipeline:** the wait stands as `docs/wake.md`,
  「Waiting」, says, and the round that takes the waited words opens the pipeline again.
- **A round runs what its chain holds,** every supervisor's alike: no agent's turns are exempt from the pipeline, and what
  a round leaves out is what its chain switches off — a stage taken out of the chain is not asked. A chain that holds no
  stage runs no stage and sends the message turn's answer unchecked. A console talk, a typed line, the touch and the line
  before a fold are no rounds of a room and run none.

## Long work in a round

- **The agent answers every round in its own session.** The message turn and every stage of its pipeline go to the session
  that holds the agent's rooms (`docs/bmo-window.md`), and the message goes out by the round's own send. No round is handed
  to another session or run, so a reply to a person never waits on a slot, a role's line or another session's work.
- **Work longer than moments goes behind the round, after the person is answered.** Where a round calls for work longer than
  moments — an investigation across files or machines, a change made and checked, something to wait on, more than a handful
  of tool calls — the round's message tells the person, in a few words, what the agent will do, and the work goes behind
  the round by one of the roads the agency's rule for a supervisor's long work gives (the agency's `docs/supervisors.md`,
  「A supervisor's long work」): a call made apart, whose result wakes the agent in the room it was asked in; the program's
  own background, a shell command or a subagent put there, for work that ends within the session: the turn that launched it
  ends at its stop with the work still running, so the round takes that turn's answer as its message at once and runs its
  stages on it, and the work, living only inside the program, is lost with it; a job item for work whose result is code or
  a document. The agent comes back to the room with the result, in the round that result opens. The pipeline's fixed rules
  say so (`chat-line.toml`, `process`). A subagent run inside the turn, for reading whose conclusion the message needs,
  holds the round while it runs.
- **The end of background work opens its room's round as an instruction.** The agency hands the receiver an order for the
  supervisor in the room the work was launched in, under the sort `background`, the kind `background-ended:<task>` or
  `background-lost:<task>` and the cause `reply` (`docs/api.md`, 「Instructions」). The receiver names no sort in reading
  it: the order opens the agent's turn in that room as any room reply does, and the answer goes through the pipeline into
  the room. The turn the program itself opens on the notice is no round's and reaches no room.

## A voice note

- **The message turn reads a voice note as text.** The receiver transcribes the saved recording before the turn (`docs/messages.md`,
  「Every kind of message」), and the line stands as a voice message with its length and what was said. The room block tells
  the agent voice arrives as text. A recording that could not be transcribed is one line saying so, never nothing.
- **The transcript is not the message's words.** The archived text stays the service's voice mark and its caption; the
  transcript stands on its own column and the console reads it under the player.

## Confidence and the source of a fact

- **Confidence comes from a fact's dates, and hedging is no option.** The brain's facts carry when they hold from and when
  they were last confirmed: a fact found or confirmed recently is said with confidence, and one the agent is not confident
  of is searched for quietly before it answers. The message never tells the room that something may be out of date; the
  search is the agent's own and the room hears only its result.
- **A fact about the system's live state has one source.** A login, a usage window, a model, what is running, what is
  deployed, a setting: such a fact counts only when read now from the service that owns it, by its route or its tool. A
  file, a cache or an earlier answer that mirrors that state is not a source. What has no source is read now, and what
  cannot be read now is said as not confirmed, never as fact. The pipeline's fixed rules say so.

## The stage chain and its texts

- **The chain is an ordered list of stage names; each stage has a text its turn carries as the instruction.** The
  policy holds the default chain and the default text of each stage; a supervisor sets its own chain and its own text for
  any stage, read by that supervisor alone, and may add a stage under a new name. A text set for one supervisor is read by
  no other, and nothing the console sets stands for every supervisor.
- **Each stage's text names what the stage holds the message to, and quotes no sentence for the agent to copy.** Its
  default stands in the policy (`instruction.toml`, `[stages]`) and is the words the deployment set for it, in English
  as every line an agent reads; the console sets a supervisor's text over it (「The routes」 below), in the words the
  person sets. A stage's name is a lowercase word of letters, digits and dashes opening on a letter (of at most 40
  characters), never `message`, `arrived` or the audit's `audit`, which the round's fixed turns go by; its text is not
  empty and at most 4,000 characters. The policy's default chain is the single `review` stage, and its text asks of the
  last answer, for every person it answers or names, whether it addresses them by the form the brain holds for them and
  in the right register and writes in their language; whether the brain was read for what the answer needs and every
  statement about live system state comes from a read made in this turn; whether it answers only what was asked without
  restating what the agent was told; and whether it follows the writing rules, with no dash. What a stage is answered
  with stands once in the block's rule for answering an instruction; the default `review` text closes by restating it,
  the corrected message whole or PASS. A supervisor chains several stages by listing them in the order to run them.
- **The block carries how a turn is read, and a stage's turn carries the stage.** What the block says once stands in
  「What reaches an agent」 above; beside it the block carries the confidence rule, the source rule, that nothing is hedged,
  that a skip in its form ends the round and is for words that ask nothing and change nothing (naming its reason on one
  line, never standing bare), that a person's telling the agent something of themselves which it keeps is answered in the
  same round, that an earlier unsent answer or a note is not a message the room has heard and never a reason for a skip,
  and the marks a message that arrived meanwhile takes (`chat-line.toml`, `[room]`, `process`). The pipeline asks the agent
  for no progress line: what a working turn shows the room is read off its run as it goes (`docs/receiver.md`, 「The event
  stream」). The block goes to a session once per version and by its mark after that, so a turn carries only what changes
  from round to round.
- **A session reads one set of reading rules, never two.** The block's rules for reading and answering a turn are the
  deployment's, the same for every supervisor. Where they are no longer what the last turn of an agent's one session across
  rooms carried, that session is not handed the new text beside the old: it is replaced through its handoff before its
  next turn, with the reason `process`, and the fresh session reads the block as it now stands (`docs/unified-bmo.md`,
  「Replacing the session」). A chain or a stage's words the console sets changes nothing of the block and replaces no
  session: the next round's stage turns carry the words as they now stand.

## How a supervisor reads the chain

- **One chain a supervisor follows,** its own where the console set one and the policy's where it did not: the ordered
  stage names it runs once its message stands. An empty chain runs no stage and the message goes out unchecked. A stage
  the chain names but whose text the catalogue does not hold is refused at the set, so a stage taken out of the chain leaves
  nothing behind.
- **The policy's default chain is what a supervisor reads until the console sets one of its own** (`instruction.toml`,
  `chain`). The console sets one supervisor's chain and no other (「The routes」 below): a chain is one supervisor's, read
  by that supervisor alone, and no chain the console sets stands for every supervisor; the policy's chain is the deployment's,
  changed in the policy file.
- **The voice a message is written in is the persona's.** The block says once that a message is written as the message
  itself, in the voice the agent's persona sets for the person it answers, its register toward them included, and in their
  language, never as notes or a report (`instruction.toml`, `voice`); it holds however long the session runs. The receiver
  carries no register of its own and names no person's register: that is the persona's.

## A supervisor whose brain or reference is off

- **A supervisor's brain switch and reference switch are the agency's settings,** `brainOn` and `referenceOn` among its
  settings (the agency's `docs/supervisors.md`, 「Its settings」), both read by the receiver from the one settings page, asked
  afresh as a round composes its turn; a switch its settings do not say, a supervisor the agency does not know, or an agency
  that does not answer reads as on, so every round runs as it did.
- **Where either is off, nothing of the brain reaches the supervisor's turns from the receiver:** the reference switch off
  closes what the receiver carries exactly as the brain switch off does, since the index, the core and the rules about them
  are the turn's reference: the room's block carries no index, no hint and no due line, and none of the block's rules about
  the index (`docs/bmo-window.md`, 「What every turn carries」); the turn's core is not asked of the gate's reference selection,
  so no sheet, memory, errand or woken row is attached; a person's own thread, which is the room's record, still goes in.
- **The tools that reach the brain are the agency's to leave out** of the supervisor's session record under the same switch
  (the agency's `docs/supervisors.md`). A stage that asks the brain (like the default `review`) still runs while the brain
  is off; the agent answers it with what it holds.

## The situation note

- **The note is an analysis of the room, and it sends nothing.** Its turn writes the note and nothing else: it answers nobody
  and writes no reply, and nothing of it goes to a room. It says what the room is about, what has been talked about and
  settled so far, who is talking with whom, who waits on whom, what the agent has itself been asked or promised, and what
  is still to be done, and then it judges what came in against that state: what it changes and how the room is best answered,
  as the course to take and never as the message.
- **The note says what is talked about and what is owed, never what went out.** It never says that a reply was sent, attached,
  fixed or answered: where it would say something was sent, it says what remains to be done. It never copies a message or
  restates the agent's own answer, and it holds no first-person claim that a reply, a fix, an attachment or a send happened,
  whether the claim is fresh or copied. A note that copies a shown sentence or makes such a claim is refused, and the note
  as it stood stays. Only a message the room's record holds counts as sent.
- **One note a room, the receiver's own.** The note stands in `room_situations` (`docs/store.md`), one row a room, replaced
  when it changes, with the row of the room's record it has read to. It is the room's live state, not the agent's long-term
  memory: the brain's `situation` rows are rules for a situation, while this note is written and read by the receiver alone.
  It is no reference document and stands nowhere in the turn's core.
- **The note is written when a session leaves a room, never by a round.** Three moments write it, each one turn a room on the
  agent's one session across rooms: another room's words; a handoff; a keepalive touch. A room is owed a note from the agent's
  answer in it until its note is updated, kept in `<agent>_notes_owed` (`docs/store.md`), the last of them the room last
  answered in.
- **The update turn reads only what the note has not.** The note's instruction stands in the room's block, the session's
  system prompt, beside the rules for reading a turn, assembled from the situation policy with its ceiling rendered into the text
  (`chat-round.toml`, `[situation]`, `instruction`, `ceiling_chars`). The turn itself carries only a short mark that this is
  the note turn (`marker`), the note as it stands and the room's messages that came in since the note last read the record, the
  agent's own answers among them, under the policy's labels; its answer is the note, and the row of the last message it read to
  is kept with it. The note as it stands goes whole where the session has not read that version, and as the policy's one-line
  marker where it has (`chat-round.toml`, `[situation]`, `unchanged`; the version is the note's text, kept under `bmo_session_notes`,
  `docs/store.md`). Where the room has no note yet, it carries the latest messages the policy names (`recent_messages`, 30) and takes
  its first position at the newest of them. A turn that answers `<<SKIP>>` or `<<WAIT>>`, or nothing, leaves the note's words as
  they stood but takes its position forward with what it read. Where more messages stand between the note's mark and the newest than
  `gap_messages` (200), the update reads the newest `gap_messages` alone instead of the whole gap. The note stays under `ceiling_chars`,
  600 characters.
- **A session reads a room's note once.** The round that first takes a session into a room carries the room's note under the
  policy's line; every later round of that session in that room carries none. The receiver keeps which rooms' notes a session holds
  (`bmo_session_notes`, `docs/store.md`).
- **The console reads the note beside each judgement.** The note as it stood when a round opened stands on every message that
  round judged (`docs/api.md`, the `Message`'s `situation`), so a message's judgement record says what the room's situation was as
  the round found it; its `changed` is false, since no round writes the note. A room answers the note as it stands now, with when
  it last changed, at `GET /rooms/{roomId}/situation`.
- **An owner's directive is not skipped because a note or an earlier unsent answer says it was handled.** The block says so
  (`chat-line.toml`, `[room]`, `process`): a note, a stage or a turn of this session is not a sent message, and only a message the
  room's record holds counts.

## What came in meanwhile

- **Words that come while the message turn works go into it.** Words the room queue hands over while the message turn works are
  pushed into it at its next look (`docs/bmo-window.md`, `push`): the paragraph carries the push's marker and the words as data, what
  the marker means standing in the room's block, and the agent's program takes them between tool calls. The turn takes the quiet
  instructions delivered meanwhile in the same paragraph.
- **What no push put in reaches the agent in the closing check.** Words the turn refused or whose provider takes no words while it
  works, and words that came while the push was held back wait in the room's buffer. Once the message turn's answer comes back, the
  receiver takes what waited and shows it in the closing check as one block, composed as a push's paragraph is but for the rule each
  message is judged by, which the room's block states: each message stands with its number.
- **Once the message stands, one closing check.** Where words waited when the message came to stand, and the session is asked once
  more before the stages: the marker that names the check (`arrived_marker`), then the block of what arrived; the message is the
  last one the session wrote, and the turn carries it no more (「What reaches an agent」 above).
  What the check does stands in the room's block (`instruction.toml`, `arrived`). The answer, marks taken out, is the whole message as
  it should now go out, answering the words that arrived as well, and it takes the standing message's place: nothing is joined to
  anything, so no part of the message goes out twice. An answer that is nothing but marks sends the standing message unchanged, a wait
  leaves it too, and a skip sends nothing; the marks keep their meaning for the messages shown. The check is not asked where the pipeline
  came to nothing to send.
- **The agent decides each message it was shown, by mark,** a message pushed into the message turn being shown to the round as one
  in the closing check is: by default the round's message answers it; `<<DONE #<number>>>` needs no answer; `<<SEPARATE #<number>>>` is
  answered in a round of its own; `<<REPLY #<number>>>` is the message the answer answers. The number is the message's own as the round
  showed it, its minus sign kept where it is negative. A mark counts in any answer of the pipeline, outside a code span or fence, and
  only for a message of the round, its own or one shown to it; a message marked both done and separate is separate. Marks are taken out
  of an answer before anything else reads it, and never reach the room.
- **Each message shown to the round carries a verdict of what its mark made of it.** Once the round's message is made, every message
  pushed into the message turn or shown in the closing check is written down under itself as a verdict of the stage `read-with`, naming
  the message the round's own verdict stands under: `READ: #<number>` where it became the round's own, `SKIP: #<number>` where it was
  marked done, the number left out where that verdict stands under no message. A message marked separate is written down by the round that
  takes it.
- **The answer is pinned only to the message the agent marks.** A `<<REPLY #<number>>>` or `<<REPLY:<number>>>` mark names the message,
  the round's own or one older than it, and the answer is pinned to it; a bare `<<REPLY>>`, or the number zero, names the newest message
  the round answers; a mark the supervisor's judgement set stands where the pipeline's answers set none. With no mark the answer goes out
  plain, whatever the round held, whoever spoke since and whatever the owner is owed: the pipeline never picks a message to pin by itself.
- **The fold is asked by the message itself.** `<<FOLD>>` is no mark on a message shown to the round: it stands in the message's words, so
  the answer that becomes the message folds where it carries the mark and goes out unfolded where it does not, whatever an earlier answer of
  the pipeline carried.
- **Words of several people are each answered or set apart.** Where the round's own messages come from more than one person, the turn says so
  under them (`chat-round.toml`, `[blocks]`, `people`): the message answers each of them, and one that needs no answer or an answer of its own
  is marked done or separate. Once the message is made, each of the round's own messages is written down under itself as a `read-with` verdict.
  One marked separate leaves the round and goes back to wait, opening the round after. A round of one person's words reads no done or separate
  mark on them.
- **Nothing else opens a round for a message shown inside a round.** Words that come after the closing check, or while a stage works, are pushed
  into no turn, were shown to nobody and open the next round.
- **A turn whose run failed showed nothing:** the block it carried and the words pushed into it go back to wait, and a pipeline that is stopped gives
  back every block it showed and every word pushed into it.

## Time

- **No clock and no count bounds a round.** The message turn works until it has its message or the agent skips deliberately; no turn has a time
  bound, no round has a cap, and no tool call is counted. Each stage of the chain is asked once. A round that takes a long time still ends with its
  message. Nothing else makes the room hear silence but an answer that says so.
- **Before the message turn has answered, no message stands.** A pipeline that has its message goes on to the closing check and the stages below.

## The audit before the stages

- **A fixed audit runs before the chain, on every round whose message stands,** and no setting switches it off or
  edits it: it is the policy's (`instruction.toml`, `[audit]`). It asks whether this session has read anything from the
  brain about each person the message answers, every sender of the round's words and of those shown to it but the agent
  itself and words marked done or separate.
- **What the session read is the agency's record of its loads,** the pieces the gate handed the session and those it read
  through the gate's index (the agency's `GET /session-loads/{sessionId}`), with the round's own core: a person counts as
  read where a piece is their sheet, told by the person's key in its path (`…/people/<key>/…`), or a memory the brain
  names them a subject of. Only the brain counts. The brain's own marks of what
  it handed a session are not read, since no live session's recall keeps them, and the agent's own brain tool reads are
  not counted, since its calls reach the brain without the gate and leave no load.
- **A person read in this session is passed over.** For one read in nothing, the brain is asked what it holds of them
  (「The brain」 of `docs/boundary-api.md`): their records and whether one of them is a name. The audit names each such
  person on a line of its own (`read`, `ask` or `read_ask`): where the brain holds records, the agent is to read them,
  the form of address and the traits; where none of them is a name, or the brain holds nothing of them, the agent is to
  have the message ask them once how they want to be called, and to record the answer as a `name` memory about them
  with its brain tool once they answer.
- **A person is asked once.** The people the audit had the message ask stand per agent under `<agent>_names_asked`
  (`docs/store.md`), so a later round in any room never asks the same person again, the answer standing in the brain once
  given. A person asked before whom the brain still holds nothing of is passed over.
- **The audit is one system instruction,** asked as a stage is (「The stages before the send」 below): its marker names
  the stage `audit`, its words are the policy's `text` and the people's lines, and it is answered with the message
  reprinted whole or `PASS`. Where no person is owed a line, where the brain does not reach the supervisor's turns
  (「A supervisor whose brain or reference is off」) or where the agent has no brain, no audit is asked.

## The stages before the send

- **The chain runs in order once the message stands.** Each stage is one more turn of the same session, a system instruction:
  the line naming the stage (`marker`), the stage's words as the supervisor's catalogue holds them, then the room's words, the people the message answers and the rules this round carries (shown under their
  heads `words`, `answers`, `person`, `rules`, `handed`, `room_handed` as data), so the agent can hold the message to what the
  stage asks of it. The final answer after the last stage is the message that is sent. A stage the chain names whose words
  the catalogue no longer holds is not asked.
- **A turn whose run failed fails the round with it,** a stage, the closing check and the message turn alike: the round gives
  back what it took and no message goes out past a turn it did not pass through. A stage's answer that skips or waits changes
  nothing, and its row keeps the reason a skip named.
- **A stage passes the standing message unchanged when its answer is `PASS` alone** (`instruction.toml`, `pass`, read whatever its case
  and spacing), when it reprints the message that stands, or when it answers nothing but marks; otherwise
  the whole revised answer takes the message's place. No check reads the final text for the pipeline's own lines: the answer marks are taken out of a turn's answer
  and prompt scaffolding is cut where it begins, so no stage marker stands in a message that goes out.

## The turn's mark

- **A turn hands its material once.** The main round hands the core's documents and references and the room's words in full. Every later round of the same turn —
  a stage or the closing check — carries each of those documents as a line naming its path and the room's lines as one line naming their numbers; a line or an entry
  the turn has not handed yet goes in full. A round that arrives with new words carries those words whole and then marks them for the round after.
- **The mark is the turn's, not the session's.** It is kept under the turn's round name, separate from the session's loads and the session's own marks, and is let go
  when the turn ends. A later turn starts with no turn mark, so nothing is assumed about what a new turn has read; the session mark still governs what the session holds
  across turns.
- **Nothing unread is dropped.** A piece is marked only once it has gone in full, and a piece that was not, or a room line that arrived since, is written whole.

## After the pipeline

- **The message goes to the round's send** (`docs/wake.md`, 「The room's queue and the round」): the words' form reads it there, once. Once it went out, the window restarts,
  the read mark is written and the fold reads the context the last turn of the pipeline left; a message no try of the send put in fails the round, its words left unread for the next.
- **Nothing the pipeline asked goes to the room** and none of it is archived: the stage answers and their records are the session's own and the round log's.

## The round log

- **Every turn of the pipeline leaves a row of the receiver's round log** (`instruction_rounds`, `docs/store.md`): the round it belongs to, the room, the agent, its number
  (from 1, the message turn and then the closing check, the audit and each stage in the order asked), its name (the stage's name, `arrived` for the closing check, `message` for the message turn),
  its kind, how long its answer is in characters, whether the answer differs from the one before it, how it ended, the answer itself and how long the turn took.
- **The kinds are `supervisor`, `turn`, `arrived`, `audit`, `stage` and `end`;** the closing check is a turn of kind `arrived`, the audit one of kind `audit` named `audit`, and a stage's turn one of kind `stage`. A turn's row ends `answered`,
  `send` where its answer opened on `<<SEND>>` and became the message, `no-verdict` where it skipped naming no reason and the same turn was asked once over, `skip` or `failed`; a stage's
  row and the audit's carry the agent's answer and end `pass` where the message went on unchanged, `revised` where the answer took its place, or `failed`, and its detail keeps the reason a skip named.
- **The round's end is a row of its own,** of kind `end`: `message` where a text goes to the send, `skip` where nothing does, and in its detail the supervisor whose chain the round read and,
  where the round skipped, the reason (`; skipped: ` then the reason, or `none given`).
- **The end row names what the round read,** its `reads`, each a `path`, what the piece holds (`holds`), the brain memory it is (`memoryId`) and how it was read (`how`), and the session
  its turn ran on and how many pieces that session's earlier rounds in the room read in full, as `docs/unified-bmo.md`, 「Replacing the session」 counts them. Each row is told on the event
  stream as it is written, a `draft` event (`docs/receiver.md`, 「The event stream」), and the log is read by round (`GET /rounds/pipeline`): the rounds a reader names, as a screen names those its
  messages stand under, in the order of their names; or, where it names none, the room's newest rounds, at most `limit` of them (20 by default, 200 at most), oldest first, the page's
  `more` saying older rounds stand.
- **Each turn of the pipeline is told as it starts, too,** before its answer: a `draft` event of kind `start` with the `round`, the `agentName`, `number` and `count` (the message turn,
  the closing check and the audit 1 of 1, a stage its place in the chain and the chain's length), `question` the name its row goes by, `input` what reaches the agent on it (`message` for the
  message turn, `system` for the closing check, the audit and each stage) and `at`. It is no row of the log, and the turn's own row follows it once the answer is in. A screen tells the message
  being written by the message turn and the stage being asked by a stage's, and shows no message received on a system instruction. The gate's `draft` stage and the console's pipeline
  chip read this log.

## The supervisor's opening

- **The room's block carries the shape of a turn and its rounds.** One short line names a **turn** as one pass over a room's words and its **rounds** as the turns the session is asked
  inside it, this round's rule (a skip in its form or one short line, and no other action), and that the block's rules for reading a message and an instruction and for answering each are
  the rulebooks for the rest (`chat-round.toml`, `[supervisor]`, `structure`). The judgement turn itself carries a marker, the room's note where the session has not read it, and the room's
  words as data; the block says what the marker asks, the judgement's head and question with it, both the policy's.

## The routes

- **`GET /instruction-stages`** answers every chain of the table, the policy's and then each supervisor's own, each with its `source`: `set` where the console set it,
  `default` where the policy's stands. With `agentName` it answers the one chain that supervisor follows: its own, or the policy's where it has none.
- **`PUT /instruction-stages/{agentName}`** sets a supervisor's chain and answers it as it now stands, its key the supervisor's name. A chain
  naming a stage whose text the catalogue does not hold, or naming a stage twice (kept once, in the order named), is refused `bad-request`, and nothing is written.
- **`DELETE /instruction-stages/{agentName}`** takes a supervisor's chain away, and the supervisor reads the policy's again. A key the console set no chain for is `not-found`.
- **`GET /instruction-texts`** answers a supervisor's stage catalogue where `agentName` names one, and the policy's alone where it names none, each stage with its `source` (`set` where the console set its words, `default` where the policy's stand) and its `policyText`, the policy's words of the stage, null for one the console added.
  **`PUT /instruction-texts/{agentName}/{name}`** sets a stage's text for that supervisor, a name being a lowercase word of letters, digits and dashes opening on a letter, and a text at most 4,000 characters and not empty.
- **`DELETE /instruction-texts/{agentName}/{name}`** takes the console's text of a supervisor's stage away. A stage the console added leaves that supervisor's catalogue; one the policy holds
  stands again in the policy's text. A stage the console added that a chain of its supervisor still names is refused `conflict`, since the chain would name nothing: it is taken out of the chain first. A name the
  console set no text for is `not-found`.
- **A supervisor's catalogue holds the stages its chain may name,** the policy's stages plus any the console added for it; a chain that names a stage outside the catalogue is refused.
- A change reaches the next round of the supervisor it was set for; no round under way changes, and no other supervisor's does. No change of a chain or of a stage's words replaces a
  session (「The stage chain and its texts」 above).
