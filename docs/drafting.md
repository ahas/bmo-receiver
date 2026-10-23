# The draft process

How a round's answer is made before it goes out: the questions its turn is followed by, the table that says
which, the items and checks the last text passes, and the record each turn of it leaves.

## What it is

- **The process is how the message is made, not a check of a finished one.** A round's turn goes to the agent as
  every round's does (`docs/wake.md`, 「The room's queue and the round」), after the window's pregate: the
  reference the gate selects, the sanitiser on the words coming in, the gate's reading of whether a reply is
  wanted. What the turn answers is a draft. The receiver holds it and asks the same session the process's
  questions, one turn each, in order; the answer to the last question is the message.
- **A question is a turn of the same session,** opened as `question` (`docs/bmo-window.md`, 「The openings」), so
  it holds the round's intent, its reference and its cache. The question makes the agent think; the answer is
  free: thoughts, notes, what it read or did, a message written out. Every tool the round's turn has is open in
  every question: reading and writing memories, searching, filing a job, `say`. A question to read more is
  answered by reading; a question whether a promise was kept is answered by keeping it.
- **The receiver reads two things of an answer and nothing else:** `<<SKIP>>` standing in it outside a code span or
  fence, which ends the process in any round with nothing sent, and the answer itself.
- **A turn that answers `<<WAIT>>` runs no process:** the wait stands as `docs/wake.md`, 「Waiting」, says, and the
  round that takes the waited words runs one.
- **A companion's round runs no process:** it is its own turn (`docs/wake.md`, 「A companion's round」). A console
  talk, a typed line, the touch and the line before a fold are no rounds of a room and run none either.

## The questions

- **The questions are a catalogue, by name,** each a text the session is asked. The policy holds six, in the words
  the owner wrote them and in the owner's language, and the console changes any text or adds a question under a
  new name (「The routes」 below):

| Name | What it asks |
|---|---|
| `intent` | why the words were said and what the speaker wants; whether to answer is the pregate's reading, so this asks the reason |
| `information` | what the answer needs to know and what more must be read; where the selected reference falls short, read |
| `facts` | whether what the answer rests on is true; nothing undecided spoken of as known, no variable guessed |
| `promises` | whether anything said like a promise was actually done; what was not done is done now or the words change |
| `fit` | whether the answer fits the room's rules, the persona and the language the other person speaks |
| `final` | looking at what came in meanwhile, write the message now, fix the draft, or send nothing; always the last |

- **A question turn goes out as** the process's lead line, naming the question's place and count, that the answer
  is free and the last answer is the message, and that `<<SKIP>>` ends it; then the question; then the turn's
  scope. Every question but the last closes on the messages waiting and that they are shown with the last
  question; the last carries them instead (「What came in meanwhile」 below). The lines are the policy's
  (`drafting.toml`).
- **No gate is asked for a question:** the turn carries the round's own selection again, its documents marks by
  now, and no reference is selected and no judgement written for it.

## The process table

- **One table says which questions a turn is asked:** rows keyed by a provider, a tier and an agent, each a name
  or `*` for any. A row holds:
  - `rounds`, the questions of an ordinary turn, in order;
  - `light`, the questions of a light turn (「A light turn」 below);
  - `lightChars`, how short the room's words must be for a turn to be light;
  - `ceilingSeconds`, the time the whole process may take (「Time」 below);
  - `pastCeiling`, `send` or `skip`: what becomes of the turn once the ceiling has passed.
- **The policy's rows are the defaults** (`drafting.toml`): any provider, `intent`, `information`, `facts`,
  `promises` and `final`, the small models' process; Claude's provider, `information`, `promises` and `final`;
  every light turn `final` alone; 150 seconds, past which what stands is sent; 12 characters for a light turn.
- **The console's rows stand over them** (「The routes」 below): a row set for a key the policy also holds takes its
  place, and a row set for a key of its own adds one, which is how an agent is given a process of its own.
- **A turn reads the most specific row that matches it:** the provider its session stands on, the tier the agency
  names for the session and the agent whose turn it is, an agent's row before a provider's and a provider's
  before a tier's. A session whose state names no tier matches only rows of any tier.
- **A list that holds questions ends in `final` and holds it nowhere else.** An empty list asks nothing: the
  turn's own answer is the message, held to the items and checks below like any.

## A light turn

- **A turn is light** where it carried quiet instructions and none of the room's words, or where the room's words
  it carried hold no file and run together, white space aside, to at most the row's `lightChars` characters: a
  greeting, a word of thanks, a short call. A light turn is asked the row's `light` list.

## What came in meanwhile

- **Words that come into the room while a question works are not pushed into it:** they wait in the room's buffer
  as words wait between rounds.
- **The last question takes them,** as one block composed the way words pushed into a working turn are
  (`docs/bmo-window.md`, `push`): the window selects for what the round does not already hold, and the block
  says what arrived and that this same answer takes them in. In a round that read only the owner's words, the
  block takes the waiting words only where they are all the owner's, as a push does.
- **What the last question took is the round's own** from then on: the read mark moves to it, the owner's words
  among it are owed an answer, and the answer is pinned to the newest of it. Words that come while the last
  question works are pushed into it, as into any working turn; words that come after it wait for the next round.

## Time

- **The process runs inside the room's window,** so a row names a ceiling for the whole of it, counted from the
  moment the round's own turn answered its draft: the turn itself is the agent's, however long it takes, and the
  ceiling bounds what the process adds to it. No question starts past it; a question under way is let finish.
- **Past the ceiling, `send` sends what stands:** the last question's answer where the last question ran, and the
  turn's own draft otherwise, since a question's answer before the last is the agent's notes and no message.
  `skip` sends nothing.
- **Past the ceiling, no item or check is asked again** (below): the checks still read what stands, and one that
  fails withholds it. The owed answer is the one asking that goes out whatever the time, since the owner's own
  chat is always answered.

## The items after the last question

The code's own readings of the turn are items of the process, each asked once as one more turn of the same
session, its answer the message where it says something: neither empty nor a skip nor a wait.

- **`owed`:** where the process ended in a skip and the round read the owner's own words in the owner's own chat,
  the session is asked once more with the silence withdrawn (`docs/wake.md`, 「The owner's own chat is always
  answered」); a skip again sends nothing.
- **`record`:** the message promises what the agent will do from now on and no turn of the process wrote it down
  (`docs/answer-checks.md`, 「A promise stands on a record」).
- **`wake`:** the message promises to act once something happens and no turn of the process set what wakes the
  agent when it does.
- **`rule`:** the owner's words of the round set a rule for a situation to come and no turn of the process filed
  one; asked only where the turn's tools are on.

## The checks on the final text

- **Once the items are done, the text is checked before it is sent,** with no model:
  - `length`: at most the policy's `max_chars` characters;
  - `language`: in a room that speaks another language, the text is not in the machine's
    (`docs/answer-checks.md`, 「The language an answer goes out in」);
  - `wording`: no term the deployment's wording document bans (`docs/answer-checks.md`, 「The banned words」);
  - `format`: none of the process's own lines stands in the text: no question it was asked, and not the head of
    the block of words that came meanwhile.
- **A check that fails is asked once:** one more turn of the same session, the check's errand naming what is
  wrong, its answer the text from then on. The checks then read the new text. A check that fails again sends
  nothing, and the end of the process records the checks that failed. A check that fails only after another's
  asking is asked once in its turn.
- **A skip in the answer to an item or a check** ends the process with nothing sent, as a skip in any question
  does.

## After the process

- **The message goes to the round's send** (`docs/wake.md`, 「The room's queue and the round」): the outgoing
  sanitiser and the words' form read it there, once. The window restarts, the read mark is written and the fold
  reads the context the last turn of the process left.
- **Nothing the process asked goes to the room** and none of it is archived: the questions and their answers are
  the session's own and the round log's.

## The round log

- **Every turn of the process leaves a row of the receiver's round log** (`draft_rounds`, `docs/store.md`): the
  round it belongs to, the room, the agent, its number (0 for the round's own turn, then each question, item and
  check in the order asked), the question's name, its kind (`turn`, `question`, `item` or `check`), how long its
  answer is in characters, whether the answer differs from the one before it, how it ended (`answered`, `skip` or
  `failed`), the answer itself and how long the turn took.
- **The process's end is a row of its own,** of kind `end`: `message` where a text goes to the send, `skip` where
  nothing does, `withheld` where a check failed twice, `ceiling-message` or `ceiling-skip` where the ceiling ended
  it, and in its detail the row of the table the turn read and the checks that withheld it.
- **Each row is told on the event stream as it is written,** a `draft` event (`docs/receiver.md`, 「The event
  stream」), and the log is read by round (`GET /rounds/drafts`).
- **The gate's `draft` stage and the console's draft chip read this log.** What the gate is to write of it is a
  contract the gate has not stated yet (`docs/boundary-api.md`, 「Contract gaps」).

## The routes

- **`GET /draft-processes`** answers every row of the table, the policy's and the console's, each with its
  `source`: `set` where the console set it, `default` where the policy's stands.
- **`PUT /draft-processes/{provider}/{tier}/{agentName}`** sets a row, `*` standing for any in each place, and
  answers it as it now stands. A list naming a question the catalogue does not hold, a list that holds questions
  and does not end in `final` or holds it before the end, a ceiling outside 1 to 3,600 seconds, a light bound
  outside 0 to 1,000 characters or a `pastCeiling` other than `send` and `skip` is refused `bad-request`, and
  nothing is written.
- **`DELETE /draft-processes/{provider}/{tier}/{agentName}`** takes the console's row away; the policy's row of the
  key stands again where it holds one. A key the console set no row for is `not-found`.
- **`GET /draft-questions`** answers the catalogue, each question with its `source`; **`PUT
  /draft-questions/{name}`** sets a question's text, a name being a lowercase word of letters, digits and
  dashes, and a text at most 4,000 characters and not empty.
- A change reaches the next round that reads the table; no round under way changes.
