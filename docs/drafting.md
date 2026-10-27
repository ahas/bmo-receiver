# The draft process

How a round's message is made: the questions the round's session is asked, the table that says which, the
markers that end the process early, the items and checks the message passes, and the record each turn of it
leaves.

## What it is

- **The agent decides first whether to answer.** Where the open window alone let the words in, the round's very
  first step is the agent's own judgement of whether to answer them (`docs/wake.md`, 「The room's queue and the
  round」), before every pregate: no reference is asked, no index read and no gate run until it answers that the
  round goes on. A skip ends the round there.
- **The process establishes what the message needs first and writes the message last.** A round's turn goes to
  the agent as every round's does (`docs/wake.md`, 「The room's queue and the round」), after that judgement where
  one is asked, and after the window's pregate: the core the gate attaches and the brain's index. The turn carries,
  with the room's words and the index, the process's first question, and asks no answer:
  the agent reads, finds the facts the message rests on and engraves what it establishes in the brain with its
  tools as it goes. The receiver then asks the same session the process's further questions, one turn each, in
  order; every answer but the last is the agent's notes, and the answer to the last question is the message.
  Nothing stands as a message before then.
- **A round's notes are what it read, what it established and what is still unknown.** The facts themselves go
  to the brain during the round, by the agent's own tools; the notes go to the round log (「The round log」
  below), so the brain and the draft record both hold what the round established.
- **The first question has the most freedom,** up to the question the row reads a send from (「The process
  table」 below). It says, in the owner's words: where a send is read at it, a simple call is answered now and
  the process stops, and where the facts already found and what is now known are enough, the message is written
  now; where the agent cannot tell whether a fact it found is stale, it searches before deciding, with the web
  search, the web fetch, the translation and the brain's tools its session holds; otherwise it notes what the
  message needs and goes on.
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
  span or fence, which ends the process in any round with nothing sent; `<<SEND>>` opening it, which, from the
  question the row reads a send from on, makes what follows the marker the message and ends the questions there,
  so a question already answerable pays no further round, and before that question is notes like any answer; the
  marks `<<DONE #<number>>>` and `<<SEPARATE #<number>>>` on a message shown to the round (「What came in
  meanwhile」 below); and the answer itself. A skip anywhere in an answer wins over a
  send that opens it, and a send with nothing after it is a skip.
- **A round whose first answer is `<<WAIT>>` runs no further process:** the wait stands as `docs/wake.md`,
  「Waiting」, says, and the round that takes the waited words opens the process again.
- **Every agent's round runs the process,** the chat agent's and any other agent's alike: no agent's turns are
  exempt from it. A console talk, a typed line, the touch and the line before a fold are no rounds of a room and
  run none.

## The questions

- **The questions are a catalogue, by name,** each a text the session is asked. The policy holds seven, in English
  as every line an agent reads, each a rule with no sentence for the agent to copy, and the console changes any
  text or adds a question under a new name (「The routes」 below):

| Name | What it asks |
|---|---|
| `intent` | why the words were said and what the speaker wants; whether to answer is already decided, so this asks the reason |
| `information` | what the message needs to know and what more must be read; where what the session holds falls short and the brain may hold it, the brain's index opened or searched and read |
| `facts` | whether what the message rests on is true and where each fact came from: the system's live state read now from the service that owns it, nothing undecided spoken of as known, no variable guessed |
| `memory` | the brain brought up to date with its tools: the memories the answer needs recalled, similar ones among them; the facts established so far engraved; promises and checks fulfilled marked done; what the conversation showed no longer holds invalidated; what still holds confirmed, its `lastConfirmedAt` moved |
| `promises` | whether anything said like a promise was actually done; what was not done is done now or the words change |
| `fit` | whether the message fits the room's rules, the persona and the language the other person speaks |
| `final` | looking at what came in meanwhile and at the notes the process gathered, write the message now, as this room's agent says it to this person, or send nothing; always the last |

- **The process's fixed rules stand in the room's block, not in a turn:** that the message is written last, that
  an answer before the last is notes of what was read, the facts established and engraved in the brain, each with
  its source, and what is still unknown, the confidence rule and the source rule above, that nothing is hedged,
  that `<<SKIP>>` ends the process and is for words that ask nothing and change nothing, so a person's telling the
  agent something of themselves, which it keeps, is answered in the same round, and the marks a message that arrived meanwhile takes (`chat-line.toml`,
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
  question but the last closes on the messages still waiting and that they are shown with the next question; the
  last opens on its own line, that its answer is the message. The lines are the policy's (`drafting.toml`).
- **No gate is asked for a later question:** the turn carries no reference of its own, and no judgement is
  written for it.

## The process table

- **One table says which questions a turn is asked:** rows keyed by a provider, a tier and an agent, each a name
  or `*` for any. A row holds:
  - `rounds`, the questions of an ordinary turn, in order;
  - `sendFrom`, the question of `rounds` from whose answer on a send is read; empty for the first;
  - `light`, the questions of a light turn (「A light turn」 below);
  - `lightChars`, how short the room's words must be for a turn to be light;
  - `ceilingSeconds`, the time the whole process may take (「Time」 below);
  - `turnSeconds`, the time one turn of it may run before the journal says it ran long (「Time」 below);
  - `pastCeiling`, `send` or `skip`: what becomes of the turn once the ceiling has passed.
- **A send is read from the row's `sendFrom` question on.** Before it, an answer opening on `<<SEND>>` is notes
  and the process goes on, so the questions that establish the facts are asked before anything is written; each
  question's own line says whether a send is read at it. A light turn's list reads a send from its first question
  whatever the row names.
- **A question whose answer may be the message asks for it in the persona's voice.** At the `sendFrom` question and
  every one after it, `final` among them, and at the closing check, the question's line says that an answer that
  becomes the message is written as the message itself, in the voice the agent's persona sets for the person it
  answers, its register toward them included, and in their language, never as notes or a report
  (`drafting.toml`, `voice`); the line says so in every such turn, so the voice holds however long the session
  runs. The receiver carries no register of its own and names no person's register: that is the persona's. The
  questions before `sendFrom` stay notes and say nothing of the voice.
- **The policy's rows are the defaults** (`drafting.toml`): any provider, `information`, `facts` and `final`, the
  small models' process; Claude's provider, `information`, `facts`, `memory`, `promises` and `final`; both read a
  send from `facts` on; every light turn `final` alone; 150 seconds, past which `final` is asked at once; 120
  seconds a turn; 12 characters for a light turn.
- **The console's rows stand over them** (「The routes」 below): a row set for a key the policy also holds takes its
  place, and a row set for a key of its own adds one, which is how an agent is given a process of its own.
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
  meaning for the messages shown. The check is not asked past the ceiling, nor where the process came to nothing
  to send: those words wait for the next round.
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
- **The answer is pinned to the message it answers:** the one a `<<REPLY #<number>>>` mark names among the
  round's messages it answers; where none is named, the oldest of them that asks something of the agent — one
  that calls it by a word it answers to, or asks a question; else the newest of them. Owing the owner an answer
  does not move the pin. Where the road cannot aim at the message, nothing is pinned.
- **The pin stands on the room's record:** the answer's row there names the message it is pinned to as its
  `replyToMessageId`, whichever road it went out on, so the console and the chat service show the same reply.
- **Nothing else opens a round for a message shown inside a round.** Words that come after the closing check, or
  while an item or a check works, were shown to nobody and open the next round.
- **A turn whose run failed showed nothing:** the block it carried goes back to wait, and a process that is
  stopped gives back every block it showed.

## Time

- **The process runs inside the room's window,** so a row names a ceiling for the whole of it, counted from the
  moment the round's own turn, which carries the first question, went out. No later question starts past it; a
  question under way is let finish.
- **A turn past its bound is let finish, and the room hears nothing of it.** Where one turn of the process, the
  round's own turn that carries the first question among them, is still running once the row's `turnSeconds`
  have passed since it went out, the journal says so once in the round and the turn goes on to its end; the
  console's row of the answering agent already shows it at work, and no holding line goes into any room
  (`docs/voiced-notices.md`, 「A turn past its clock」).
- **Before `final` has run, no message stands:** an answer before the last is notes.
- **Past the ceiling, `send` asks `final` at once:** one last question, the notes already in the session, its
  answer the message. `skip` sends nothing. A process that already has its message, from `final` or from a send
  marker, is not touched by the ceiling but for the closing check, the items and the checks below.
- **A later question whose run failed** is passed over to `final`, asked at once; a `final` whose run failed
  leaves no message, and the round ends as a skip.
- **Past the ceiling, no item or check is asked again** (below): the checks still read the message, and one that
  fails withholds it. The owed answer is the one asking that goes out whatever the time, since the owner's own
  chat is always answered.

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

## The checks on the final text

- **Once the items are done, the text is checked before it is sent,** with no model:
  - `length`: at most the policy's `max_chars` characters;
  - `language`: the text holds the language of at least one person the turn answers, each person's language
    read off their own prose in the room (`docs/answer-checks.md`, 「The language an answer goes out in」);
  - `format`: none of the process's own lines stands in the text: no question it was asked, and not the head of
    the block of words that came meanwhile.
- **A check that fails is asked once:** one more turn of the same session, the check's errand naming what is
  wrong, its answer the text from then on. The checks then read the new text. A `length` or `format` check that
  fails again sends nothing, and the end of the process records the checks that failed; the `language` check
  failing again puts back the text it was asked over, which goes out as it stood, and a `language` check alone
  failing past the ceiling holds nothing back either. A check that fails only after another's asking is asked
  once in its turn.
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
- **The end row names what the round read from the brain,** its `reads`, each a `path`, what the piece holds
  (`holds`), the brain memory it is (`memoryId`) and how it was read (`how`). `holds` is the index's words for the
  piece where the gate's index lists it, else the name the piece came with: a piece of the turn's core carries the
  name its maker gives it, the gate's where the gate names one and the policy's word for the gate's sort of piece
  otherwise (`chat-round.toml`, `[reference.holds]`), and a person's thread the thread's own (`[thread]`, `holds`).
  A thread whose lines were all handed to the session before the round stands as `held` by that path, with no block
  of its own in the turn. `memoryId` stands where the piece is one memory of the brain: the id the gate's index
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
  nothing does, `withheld` where a check failed twice, `ceiling-message` where the ceiling had `final` asked at
  once and its answer went, `ceiling-skip` where the ceiling ended it with nothing sent, and in its detail the row
  of the table the turn read and the checks that withheld it.
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

## The routes

- **`GET /draft-processes`** answers every row of the table, the policy's and the console's, each with its
  `source`: `set` where the console set it, `default` where the policy's stands.
- **`PUT /draft-processes/{provider}/{tier}/{agentName}`** sets a row, `*` standing for any in each place, and
  answers it as it now stands. A list naming a question the catalogue does not hold, a list that holds questions
  and does not end in `final` or holds it before the end, a `sendFrom` that is not empty and names no question
  of `rounds`, a ceiling or a turn's bound outside 1 to 3,600 seconds, a light bound outside 0 to 1,000
  characters or a `pastCeiling` other than `send` and `skip` is refused `bad-request`, and nothing is written. A
  set that names no `turnSeconds` takes the policy's 120.
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
