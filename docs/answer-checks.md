# The readings an answer is held to

The readings the code holds a chat answer and the owner's words to: the answer's own sentences, with code and
anything it quotes set aside, and whether a text waits for something to happen before it can be done. Nothing
on the send path asks the session again over what a reading finds: what holds a message to its rules before it
goes out are the instruction stages the supervisor chains (`docs/instruction.md`, 「The stages before the send」).

## Where the readings stand

- **Two things read an answer's own sentences.** The situation note is refused where it copies a sentence a shown
  message carried, or makes a first-person claim that a reply, a fix, an attachment or a send happened; only a
  message the room's record holds counts (`docs/instruction.md`, 「The situation note」). The brain refuses a promise
  row whose action waits on a happening and names nothing to wait on, and answers whether a text waits at
  `POST /promises/waits` (`docs/api.md`).
- **What the readings use stands in the receiver's policy** (`policy/answer-reading.toml`, `[reading]`): the syllables a
  sentence of an answer ends on, beside the marks that end one wherever it stands, and the words of something that has
  to happen first — a job's end, a unit on another machine, a deployment, a result. The Hangul syllable block a word is
  told by stands in `policy/scripts.toml`. These are words of the room's language the code matches on, and so data.

## The language an answer goes out in

- Everything the machine itself says to a chat session is English: the turn's system lines, a quiet instruction
  and the result it carries, and the note a replaced session holds of its own past.
- English is the language this machine talks to its agents in; it is not the language of the room.
- **The language an answer is written in is the agent's own judgement.** No turn names a language and nothing
  reads one off a message: the agent answers each person in the language it knows for that person — what the
  brain holds of them, their country and the language they usually speak, and how the conversation with them has
  gone — and the agent's manual states the rule. A message carrying commands, paths or code in another language
  is still the message of the person who wrote it, and what it quotes never decides the language of the answer.
- **Nothing on the send path reads the language.** No check, stage or withholding reads the language of the
  answer, and an answer in a language a person does not write goes out as the agent wrote it.

## A promise stands on a record

- **A session's memory is gone the next time it is replaced**, so a promise about what the agent will do from now on
  (to do something some way on, to check first next time, to keep something in mind, and a promise to behave some way
  only for this session above all) stands only where the same turn wrote it down: a working rule or standard as a note,
  which the digest carries into the room's rulebook and the library; something to do later as a memory; what to do
  whenever a situation comes back as a memory carrying a condition; a change to code or to a manual as an item or words
  appended to one. A promise kept only in this session is no promise at all.
- **Recording a promise is the agent's own**, with its tools, in the turn that made it. A supervisor that wants a room
  held to it chains a stage whose text asks whether the promise was recorded; no item turn of the receiver's asks.
  Where a promise was filed as a row that waits on a happening with nothing set to wake it, the brain refuses the row
  and names what it waits on.
- **What a run wrote is kept beside it** (`run_records`, `docs/store.md`): each recording call of a tool or of a `bmo`
  verb that did not fail, read by the record its catalogue entry declares for what it named. A row filed or appended in
  the room by bmo-job since the turn began counts the same way. The keeps stand so a reader can tell what a turn recorded
  without asking it again; no reading of them withholds a message.

## A rule the owner sets stands on a row

- **Words that set what is to be done from a situation on** stand as a situation memory, filed by the agent with its
  tools in the turn that read them: `memory_add` with the room, a condition and an action, `by` and `message` naming who
  said it and where, `replaces` taking the place of a rule already standing, `slug` naming it, and `global` for a rule
  tied to no room at all. The condition is the half a later situation is judged against, so it is written as the
  situation rather than as this one turn.
- **No turn of the receiver files it and none asks after it**: filing is the agent's own, and a supervisor that wants
  the room held to it chains a stage whose text asks.
