# The draft process

How a round's message is made: the questions the round's session is asked, the table that says which, the
markers that end the process early, the items and checks the message passes, and the record each turn of it
leaves.

## What it is

- **The room's situation is updated first, in every room, and then the round goes on.** Every round with words
  opens on one light turn that updates the room's situation note from the new lines: the window's words, a direct
  message to a private chat, a light turn's words, the owner's own chat among them. Where the round is the agent's
  to judge first, the next step is that judgement, reading the note with the new lines. Neither step asks a
  reference, reads an index or runs a gate; a skip ends the round there.
- **The process establishes what the message needs first and writes the message last.** A round's turn goes to
  the agent as every round's does (`docs/wake.md`, 「The room's queue and the round」), after that judgement where
  one is asked, and after the window's pregate: the core the gate attaches, the room's situation note and the
  brain's index. The turn carries,
  with the room's words and the index, the process's first question, and asks no answer:
  the agent reads, finds the facts the message rests on and engraves what it establishes in the brain with its
  tools as it goes. The receiver then asks the same session the process's further questions, one turn each, in
  order; every answer but the last is the agent's notes, and the answer to the last question is the message.
  Nothing stands as a message before then.
- **A round's notes are what it read, what it established and what is still unknown.** The facts themselves go
  to the brain during the round, by the agent's own tools; the notes go to the round log (「The round log」
  below), so the brain and the draft record both hold what the round established.
- **The first question has the most freedom,** and a send is honored wherever it stands (「The process table」
  below). It says, in the owner's words: a simple call is answered now, and where what is now known is enough
  the message is written now, with `<<SEND>>` beginning it; where the agent cannot tell whether a fact it found
  is stale, it searches before deciding, with the web search, the web fetch, the translation and the brain's
  tools its session holds; otherwise it notes what the message needs and goes on.
- **Confidence comes from a fact's dates, and hedging is no option.** The brain's facts carry when they hold from
  and when they were last confirmed: a fact found or confirmed recently is said with confidence, and one the
  agent is not confident of is searched for quietly before it answers. The message never tells the room that
  something may be out of date; the search is the agent's own and the room hears only its result.
- **A fact about the system's live state has one source.** A login, a usage window, a model, what is running,
  what is deployed, a setting: such a fact counts only when read now from the service that owns it, by its route
  or its tool. A file, a cache or an earlier answer that mirrors that state is not a source. What has no source
  is read now, and what cannot be read now is said as not confirmed, never as fact. The first question and the
  `facts` question both say so.
- **A question is a turn of the same session,** opened as `question` (`docs/bmo-window.md`, 「The openings」),
  so it holds the round's intent, its reference and its cache; the first question is the round's own turn. Every
  tool the round's turn has is open in every question: reading and writing memories, searching, filing a job,
  `say`. A question to read more is answered by reading; a question whether a promise was kept is answered by
  keeping it.
- **The receiver reads four things of an answer and nothing else:** `<<SKIP>>` standing in it outside a code
  span or fence, which ends the process in any round with nothing sent; `<<SEND>>` opening it, wherever it
  stands, which makes what follows the marker the message and ends the questions there, so a question already
  answerable pays no further round and a message written in a draft is never read as notes; the marks
  `<<DONE #<number>>>` and `<<SEPARATE #<number>>>` on a message shown to the round (「What came in meanwhile」
  below); and the answer itself. A skip anywhere in an answer wins over a send that opens it, and a send with
  nothing after it is a skip.
- **A draft is not a message.** The room's messages are the only record of what the room has heard. A round
  log's answer, a note, or an earlier turn of the session that was not sent is not something said, and no round
  skips a repeated request because it wrote the answer in one; the duplicate rule reads the room's own record and
  nothing else.
- **A round whose first answer is `<<WAIT>>` runs no further process:** the wait stands as `docs/wake.md`,
  「Waiting」, says, and the round that takes the waited words opens the process again.
- **Every agent's round runs the process,** the chat agent's and any other agent's alike: no agent's turns are
  exempt from it. A console talk, a typed line, the touch and the line before a fold are no rounds of a room and
  run none.

## A voice note

- **The turn reads a voice note as text.** The receiver transcribes the saved recording before the turn
  (`docs/messages.md`, 「Every kind of message」), and the line stands as a voice message with its length and what
  was said. The room block tells the agent voice arrives as text. A recording that could not be transcribed is one
  line saying so, never nothing.
- **The transcript is not the message's words.** The archived text stays the service's voice mark and its caption;
  the transcript stands on its own column and the console reads it under the player.

## The questions

- **The questions are a catalogue, by name,** each a text the session is asked. The policy holds six, in English
  as every line an agent reads, each a rule with no sentence for the agent to copy, and the console changes any
  text or adds a question under a new name (「The routes」 below):

| Name | What it asks |
|---|---|
| `intent` | why the words were said and what the speaker wants; whether to answer is already decided, so this asks the reason |
| `information` | what the message needs to know and what more must be read; where what the session holds falls short and the brain may hold it, the brain's index opened or searched and read |
| `facts` | whether what the message rests on is true and where each fact came from: the system's live state read now from the service that owns it, nothing undecided spoken of as known, no variable guessed |
| `memory` | the brain brought up to date with its tools: the memories the answer needs recalled, similar ones among them; the facts established so far engraved; promises and checks fulfilled marked done; what the conversation showed no longer holds invalidated; what still holds confirmed, its `lastConfirmedAt` moved |
| `promises` | whether anything said like a promise was actually done; what was not done is done now or the words change |
| `final` | looking at what came in meanwhile and at the notes the process gathered, write the message now, as this room's agent says it to this person, or send nothing; always the last |

- **The process's fixed rules stand in the room's block, not in a turn:** that the message is written last, that
  it is reviewed before it goes out and that a review which rejects it returns it to be written again, that
  an answer before the last is notes of what was read, the facts established and engraved in the brain, each with
  its source, and what is still unknown, the confidence rule and the source rule above, that nothing is hedged,
  that `<<SKIP>>` ends the process and is for words that ask nothing and change nothing, so a person's telling the
  agent something of themselves, which it keeps, is answered in the same round, that a draft or a note is not a
  message the room has heard and never the reason for a skip, and the marks a message that arrived meanwhile takes (`chat-line.toml`,
  `[room]`, `process`). The room's block goes to a session once per version and by its mark after that, so a
  turn carries only what changes from question to question.
- **The first question rides the round's own turn:** the room's words and the round's closing lines as the window
  composes them, the closing lines saying that the question below is to be answered, then the question's opening
  and the question. The opening is the policy's: the place and count of the question, that it is the first and
  freest, and whether a send is read at this question (`send_open` or `send_closed`). Where the first question is
  the last, as a light turn's `final` is, it opens on the last question's line instead and its answer is the
  message, so the round is one turn.
- **Every later question goes out as one more turn** opening on the process's lead line, naming the question's
  place and count and whether a send is read at this question; then the question; then the turn's scope; then
  the block of messages that arrived since the last answer, where any did (「What came in meanwhile」 below). Every
  question but the message question closes on the messages still waiting and that they are shown with the next
  question; the message question opens on its own line, that its answer is the message and that the message is
  reviewed before it goes out. The lines are the policy's (`drafting.toml`).
- **No gate is asked for a later question:** the turn carries no reference of its own, and no judgement is
  written for it.

## The process table

- **One table says which questions a turn is asked:** one row for every provider, tier and agent, every part `*`.
  The matching code still reads a provider, a tier and an agent, but the policy's one row matches all of them, and a
  console row of any other key is ignored. A row holds:
  - `rounds`, the questions of an ordinary turn, in order;
  - `sendFrom`, the question of `rounds` from which the process asks for the message, a send being honored
    wherever it stands; empty for the first;
  - `light`, the questions of a light turn (「A light turn」 below);
  - `lightChars`, how short the room's words must be for a turn to be light;
- **A `<<SEND>>` is honored at any question.** The row's `sendFrom` names the question from which the process
  asks for the message, and the question's line says that a complete message may be written now; a send before it
  is the message too, and the process ends there. Where the agent does not send, the questions that establish the
  facts are asked before anything is written. A light turn's list asks for a send at its first question whatever
  the row names.
- **A question whose answer may be the message asks for it in the persona's voice.** At the `sendFrom` question and
  every one after it, `final` among them, and at the closing check, the question's line says that an answer that
  becomes the message is written as the message itself, in the voice the agent's persona sets for the person it
  answers, its register toward them included, and in their language, never as notes or a report
  (`drafting.toml`, `voice`); the line says so in every such turn, so the voice holds however long the session
  runs. The receiver carries no register of its own and names no person's register: that is the persona's. A
  message sent before `sendFrom` is written as the message itself and in that voice, though the line there names
  no voice; an answer that is not sent stays notes.
- **The policy's one row is the default for every agent** (`drafting.toml`): `information`, `facts`, `memory`,
  `promises` and `final`, in that order; a send is read from `facts` on; every light turn is `final` alone; 120
  seconds a turn is the journal's note; 12 characters make a light turn. A deployment that still carries the old
  two rows loads with them ignored in favour of this one. `intent` stays in the catalogue and out of the
  default rounds, so the five are not the whole catalogue.
- **The console's rows do not make a second process** (「The routes」 below): a row of a key other than the one row's
  is ignored, and the one row's own key replaces the default where the console sets it. Every agent still reads
  the same five.
- **A round reads its row before its turn goes out,** since the turn carries the first question: the provider and
  the tier the agency's state names for the session the round goes on (a session's state answers them from its
  start, before its first turn) and the agent whose turn it is. The most specific row that matches stands, an
  agent's row before a provider's and a provider's before a tier's. A session whose state names no tier matches
  only rows of any tier.
- **A list that holds questions ends in `final` and holds it nowhere else.** An empty list asks nothing: the
  round's turn goes out as the window composed it and its answer is the message, held to the items and checks
  below like any.

## A light turn

- **A turn is light** where it carried quiet instructions and none of the room's words, or where the room's words
  it carried hold no file and run together, white space aside, to at most the row's `lightChars` characters: a
  greeting, a word of thanks, a short call. A light turn is asked the row's `light` list. Whether a turn is light
  is read of what the round took when its turn went out.

## The situation note

- **The note is an analysis of the room, and it sends nothing.** Its round writes the note and nothing else: it
  answers nobody and writes no reply, and nothing of it goes to the room. It says what the room is about, what
  has been talked about and settled so far, who is talking with whom, who waits on whom, what the agent has
  itself been asked or promised, and what is still to be done, and then it judges what came in against that
  state: what it changes and how the room is best answered, as the course to take and never as the message.
- **The note says what is talked about and what is owed, never what went out.** It is written before the round
  answers, so it never says that a reply was sent, attached, fixed or answered: where it would say something was
  sent, it says what remains to be done. It never copies a message or restates the agent's own answer, and it
  holds no first-person claim that a reply, a fix, an attachment or a send happened, whether the claim is fresh or
  copied. A note that copies a shown sentence or makes such a claim is refused, and the note as it stood stays.
  Only a message the room's record holds counts as sent.
- **One note a room, the receiver's own.** The note stands in `room_situations` (`docs/store.md`), one row a room,
  replaced when it changes, with the row of the room's record it has read to. It is the room's live state, not the
  agent's long-term memory: the brain's `situation` rows are rules for a situation, while this note is written and
  read by the receiver alone. It is no reference document and stands nowhere in the turn's core: the judgement
  reads it under its own line, and the turn that writes the message reads it as that answer's instruction.
- **One light turn a round, in every room, reading only what the note has not.** The update turn carries the note
  as it stands and the room's messages that came in since the note last read the record, in the policy's words;
  its answer is the note, and the row of the last message it read to is kept with it. Where the room has no note
  yet, it carries the latest messages the policy names (`recent_messages`, 30) and takes its first position at the
  newest of them. It runs for every round with words, the owner's private chat included, before that round's
  judgement or its own turn. A turn that answers `<<SKIP>>` or `<<WAIT>>`, or nothing, leaves the note's words as
  it stood but takes its position forward with what it read. The note is stored before the judgement and the
  round's draft, so both read the same note.
- **The note is the message round's instruction.** The note advises how the next answer is to be given, so the
  turn that writes the message carries it first, above the room's words, under the policy's front line; the room's
  words follow and the turn's question closes it. It is no reference piece: a turn that is not writing the message
  is not handed it, and the note never stands among the core's documents.
- **The note stays under `ceiling_chars`, 1,200 characters.** A longer answer is cut at the ceiling and marked with
  the policy's `cut`; the update prompt names the ceiling, and the ceiling leaves room for the state, what came in
  and the course to take together. The extra cost is one light turn a round with words: the
  note it reads, under the ceiling, and the room's messages since the note last read the record, which on a room
  keeping up is the round's own words. A room with no note yet pays the `recent_messages` window once. The fixed
  room context stands in that turn and the round's, so the update turn is the larger part of the extra.
- **The console reads the note beside each judgement.** The note stands on every message the round judged, with
  whether that round wrote it afresh (`docs/api.md`, the `Message`'s `situation`), so a message's judgement record
  says what the room's situation was as the round read it; the owner's private chat's messages carry it too. A room
  answers the note as it stands now, with when it last changed, at `GET /rooms/{roomId}/situation`.
- **An owner's directive is not skipped because a note or an earlier draft says it was handled.** The
  round's answer to the owner's words is asked over once more where no message the room's record holds follows
  those words; a note, a draft or a turn of this session is not a sent message.
- **The judgement reads it with the new lines.** The judging turn carries the note under `[supervisor] situation`,
  before the room's recent words and the words judged. The note changes the material the judgement reads, never an
  addressee rule: a conversation between others is left alone, and a line that asks the agent or resumes an earlier
  thread with it is answered.

## What came in meanwhile

- **No room message goes into a turn of the process while it works.** From the round's own turn to the process's
  end, words that come into the room wait in the room's buffer. The push into a working turn
  (`docs/bmo-window.md`, `push`) carries only what is no room message, a quiet instruction such as one the owner
  types on the console; a room's words pushed in would reach the session after whatever tool call is running and
  mix into the answer of the question under way.
- **The words reach the agent at a question boundary alone.** Each time an answer of the process comes back, the
  receiver takes what waited since its last look and puts it under the next question as one block, composed the
  way a push's words are: the window selects for what the round does not already hold, and each message stands
  with its number. The block takes the waiting words only where they carry the round's privilege, read by people's
  words alone as the round's own is (`docs/unified-bmo.md`, the run-privilege mark): a round that is the owner's
  takes them where every person's words among them are his, an agent's line neither keeping them out nor letting
  them in; a round that is not his never takes words that are his alone. The others wait and open the round after,
  where the owner's own words carry his privilege.
- **Once the message stands, one closing check.** Where words waited when the message came to stand — the answer
  to `final`, or one a send marker opened — the session is asked once more before the items and the checks: the
  policy's `arrived` line, the message as it stands, then the block. The answer, marks taken out, is the whole
  message as it should now go out, answering the words that arrived as well, and it takes the standing message's
  place: nothing is joined to anything, so no part of the message goes out twice. An answer that is nothing but
  marks sends the standing message unchanged, a wait leaves it too, and a skip sends nothing; the marks keep their
  meaning for the messages shown. The check is not asked where the process came to nothing to send: those words
  wait for the next round.
- **The agent decides each message it was shown, by mark:**
  - by default the round's message answers it: it is the round's own from then on, the read mark moves to it, and
    the owner's words among it are owed an answer;
  - `<<DONE #<number>>>`: it needs no answer: the read mark moves past it, nothing is sent for it and nothing is
    owed it;
  - `<<SEPARATE #<number>>>`: it is answered in a round of its own: once the round's message is made it goes back
    to the front of what waits, and the round after takes it with a pregate of its own;
  - `<<REPLY #<number>>>`: it is the message the answer answers, and the answer is pinned to it.

  A mark counts in any answer of the process, outside a code span or fence, and only for a message of the round,
  its own or one shown to it; a message marked both done and separate is separate, and of several `REPLY` marks
  the last stands. Marks are taken out of an answer before anything else reads it, and never reach the room.
- **The answer is pinned to the message it answers only where the reader needs it.** A `<<REPLY #<number>>>` mark
  names the message, including one older than the round's own, and the answer is pinned to it. Where none is named,
  the message is the oldest of the
  round's messages it answers that asks something of the agent — one that calls it by a word it answers to, or
  asks a question; else the newest — and the answer is pinned to that message only when another message has been
  said in the room since it. An answer to the room's newest line goes unpinned. Owing the owner an answer does not
  move the pin. Where the road cannot aim at the message, nothing is pinned.
- **The record keeps the round; the pin is the visible reply.** The answer's row names the message as its
  `replyToMessageId` only where the pin stands; the round stands as it was, and a turn with no pin shows no reply
  in the console or on the chat service. The answer is still the round's answer, whatever the visible pin.
- **Nothing else opens a round for a message shown inside a round.** Words that come after the closing check, or
  while an item or a check works, were shown to nobody and open the next round.
- **A turn whose run failed showed nothing:** the block it carried goes back to wait, and a process that is
  stopped gives back every block it showed.

## Time

- **No clock and no count bounds a round.** The process asks its questions in order until it has a message or
  the agent skips deliberately; no turn has a time bound, no round has a cap, and no tool call is counted. A
  round that takes a long time still ends with its message, and a failing check gets the repair round it would
  get at any other time. The review's eight rejections are the one bound, and they send the last draft rather
  than silence. Nothing else makes the room hear silence but an answer that says so.
- **Before `final` has run, no message stands:** an answer before the last is notes. A process that already has
  its message, from `final` or from a send marker, goes on to the closing check, the items and the checks below.
- **A later question whose run failed** is passed over to `final`, asked at once; a `final` whose run failed
  leaves no message, and the round ends as a skip.

## The items after the message

The code's own readings of the message are items of the process, asked once the message stands (the last
question's answer, or the answer a send marker opened) and after the closing check where one was asked
(「What came in meanwhile」 above), each asked once as one more turn of the same session, its answer the message
where it says something: neither empty nor a skip nor a wait.

- **`owed`:** where the process ended with no message and the round read the owner's own words in the owner's
  own chat, the session is asked once more with the silence withdrawn (`docs/wake.md`, 「The owner's own chat is
  always answered」); a skip again sends nothing.
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

- **The gate's first turn reviews the message before it goes out.** It reads the draft against the room's words,
  what the round asked and established, and the rules and knowledge the round carries, and answers `PASS` alone,
  or `REJECT: <one short reason>` naming the one thing that must change.
- **A rejection returns to the draft.** The final question is asked again with the reason in its words, and the
  new draft is reviewed again, eight times at most. At the eighth rejection the last draft is sent, not
  withheld, and the round's end row reports the reasons in order. A review that gives no verdict is a rejection
  with its own words as the reason.
- **Every verdict and reason is written down.** A review question and its verdict are rows of the round log,
  `PASS` or `REJECT`, with the reason; the console reads them with the rest of the round.
- **The format check runs after the review passes.** A format failure gets the repair round it had before; a
  review rejection has already gone back to the draft.

## The checks on the final text

- **Once the review has passed, the text is checked before it is sent,** with no model:
  - `format`: none of the process's own lines stands in the text: no question it was asked, and not the head of
    the block of words that came meanwhile.
  There is no length check, and no check reads the language an answer is written in: that is the agent's own
  judgement (`docs/answer-checks.md`, 「The language an answer goes out in」).
- **A check that fails is asked once:** one more turn of the same session, the check's errand naming what is
  wrong, its answer the text from then on. The check then reads the new text. A `format` check that fails again
  sends nothing, and the end of the process records the check that failed. A check that fails only after
  another's asking is asked once in its turn.
- **A skip in the answer to an item or a check** ends the process with nothing sent, as a skip in any question
  does.

## After the process

- **The message goes to the round's send** (`docs/wake.md`, 「The room's queue and the round」): the words'
  form reads it there, once. The window restarts, the read mark is written and the fold
  reads the context the last turn of the process left.
- **Nothing the process asked goes to the room** and none of it is archived: the questions and their answers are
  the session's own and the round log's.

## The round log

- **Every turn of the process leaves a row of the receiver's round log** (`draft_rounds`, `docs/store.md`): the
  round it belongs to, the room, the agent, its number (from 1, each question, item and check in the order
  asked), the question's name, its kind, how long its answer is in characters, whether the answer differs from
  the one before it, how it ended, the answer itself and how long the turn took. The answer of a question before
  the last is that round's notes.
- **The round's own turn is the first question's row:** number 1, of kind `question`, named for the row's first
  question, its time counted from the round's start. Its kind is `turn` only where the row's list is empty and
  the turn asked no question, its answer being the message.
- **A round the agent judges first** (`docs/wake.md`, 「The room's queue and the round」) writes that judgement
  as a row before the process's own: number 0, of kind `supervisor`, named `supervisor`, ending `skip` where the
  round ends there and `answered` where it goes on. A round that ended there writes no other row.
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
- **The kinds are `supervisor`, `turn`, `question`, `item`, `check` and `end`;** the closing check is an item named `arrived`. A turn's row ends `answered`, `send` where its
  answer opened on `<<SEND>>` and became the message, `skip` or `failed`.
- **The process's end is a row of its own,** of kind `end`: `message` where a text goes to the send, `skip` where
  nothing does, `withheld` where a check failed twice, and in its detail the row of the table the turn read and the
  checks that withheld it.
- **Each row is told on the event stream as it is written,** a `draft` event (`docs/receiver.md`, 「The event
  stream」), and the log is read by round (`GET /rounds/drafts`).
- **A question is told as it starts, too,** before its answer: a `draft` event of kind `start` with the `round`,
  the `agentName`, `number`, the question's place in the process (1 for the one the round's own turn carries),
  `count`, how many questions the process asks, `question`, its name, and `at`. It is no row of the log, and the
  question's own row follows it once the answer is in. A screen tells the round under way by it, and the message
  being written where `number` equals `count`; items and checks are told only as rows.
- **The gate's `draft` stage and the console's draft chip read this log.** The gate takes every `draft` event of
  a row off the event stream and writes it as a judgement of its `draft` stage; a `start` event is no row, and the
  gate lets it go by its kind (`docs/boundary-api.md`, 「Contract gaps」).

## The supervisor's opening

- **The supervisor prompt opens on the shape of a turn and its rounds.** Before its own head it carries one short
  line naming a **turn** as one pass over a room's words and a **round** as one question inside it, this round's rule
  (`<<SKIP>>` or one short line, and no other action), and, for the later rounds, only the rulebook each follows:
  the room block's draft process, the review's own line and each item's errand. It names them there so the agent
  knows which rule applies at each round without the prompt repeating texts that stand elsewhere; the judgement
  head follows, and the draft process's rules stay in the room block.

## The routes

- **`GET /draft-processes`** answers every row of the table, the policy's and the console's, each with its
  `source`: `set` where the console set it, `default` where the policy's stands.
- **`PUT /draft-processes/{provider}/{tier}/{agentName}`** sets a row, `*` standing for any in each place, and
  answers it as it now stands. A list naming a question the catalogue does not hold, a list that holds questions
  and does not end in `final` or holds it before the end, a `sendFrom` that is not empty and names no question
  of `rounds`, a turn's note outside 1 to 3,600 seconds, or a light bound outside 0 to 1,000 characters is refused
  `bad-request`, and nothing is written. A row that still carries `turnSeconds`, `ceilingSeconds` or
  `pastCeiling` loads with them ignored.
- **`DELETE /draft-processes/{provider}/{tier}/{agentName}`** takes the console's row away; the policy's row of the
  key stands again where it holds one. A key the console set no row for is `not-found`.
- **`GET /draft-questions`** answers the catalogue, each question with its `source`; **`PUT
  /draft-questions/{name}`** sets a question's text, a name being a lowercase word of letters, digits and
  dashes, and a text at most 4,000 characters and not empty.
- **The catalogue holds the agent's first judgement too,** first of all, named `supervisor`: its text is what the
  judging turn opens on, the round policy's head and question as one text where the console set none
  (`chat-round.toml`, `[supervisor]`), and the room's recent words and the words judged stand after it. It is set
  like any question, and no list of the table may name it, since it is asked before any process.
- A change reaches the next round that reads the table; no round under way changes.
