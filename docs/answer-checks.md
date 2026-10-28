# The checks an answer passes

The readings the code holds a chat answer to before it goes out: a promise that stands on a record and a rule
the owner sets that stands on a row. Each is an item or a check of the draft process (`docs/drafting.md`): where
it finds something, the same session is asked once more, as one more turn of the process. The language an answer
is written in is the agent's rule, not a check; nothing on the send path reads it.

## Where the checks stand

- **The process asks them once the message stands, in this order:** an owed answer over a skip
  (`docs/wake.md`), a promise recorded, a promise set to wake and a rule the owner set, as items; then the
  review and the format, as the gate on the final text (`docs/drafting.md`, 「The review」 and 「The checks on the
  final text」).
- **What the checks read stands in the receiver's policy**: the words of time to come, of keeping in mind, of
  waiting on something, of a condition and of a manner, and the volitional and imperative endings
  (`policy/second-asks.toml`), and the Hangul syllable block a word is told by (`policy/scripts.toml`). These are words of the room's language the code matches on, and so data. So is
  every errand an item or a check sends, in English as every agent's text is.
- **An item or a check goes back to the same session** with the round's reference, the turn's scope, the
  check's errand and the messages waiting for the next turn. It is a turn of the process: its answer is written
  in the round log under the item's or the check's name (`docs/drafting.md`, 「The round log」), and no
  judgement is written for it at the gate. A run of it that failed leaves the text as it stood, and a stop ends
  the round.

## The language an answer goes out in

- Everything the machine itself says to a chat session is English: the turn's system lines, a quiet instruction
  and the result it carries, and the note a replaced session holds of its own past.
- English is the language this machine talks to its agents in; it is not the language of the room, and a turn
  with nothing but the machine's language in front of it answers in it.
- **The answer language is a person's, never a turn's.** The agent answers each person in the language that
  person writes; a turn that answers two people answers each in their own.
  - **A person's language in a room is read off their own prose there:** the newest message of theirs on the
    turn's page or in its words that holds prose names it, and it is remembered for the room and the person in
    the store (`people_languages`, `docs/store.md`). Prose is what a message keeps once its quoted spans, its
    code, its addresses and paths and its bracketed spans — the placeholder a chat service writes for a file, a
    picture or a sticker it carried — are set aside, and only where it still holds at least the number of
    letters `policy/chat-round.toml` names (`[language]`, `prose_letters`). A message with no prose — a file, a
    bare link, code, numbers, an emoji — never changes a person's language.
  - Where a person has written no prose in the room yet, the deployment's language stands for theirs; the
    receiver keeps no language of a room's own.
  - **The people a turn answers** are the speakers of its words, the agent's own aside; a turn with no words of
    anybody's answers the owner in the owner's own chat, and elsewhere the newest speaker on the page who is not
    the agent.
- **The prompt names each person's language** in its one `ANSWER LANGUAGE` line on every turn: one clause a
  person it answers, `<name>: <language>`, so a turn answering one person carries one clause; a turn answering
  nobody names the deployment's language as the room's. The line says the language is the person's, whatever
  language the machine's own lines, an instruction or its result came in. The first turn after the session's
  memory was replaced (folded up, swapped for a fresh one carrying the note, or opened again because the account
  changed) says it in the note about the replacement.
- **The room's block carries the rule** (`policy/chat-line.toml`, `[room]`, `languages`): answer each person in
  the language they write, and where one turn answers several people, each in their own.
- **The language a text is written in** is told by the letters of its prose, quoted spans set aside: Hangul
  letters count for Korean, kana for Japanese, each twice, since one stands for a syllable, and Latin letters
  for English, once; the language whose letters weigh most is the text's, and a text with none of them, or two
  that weigh the same, is in no language that can be told.
  - **Nothing on the send path reads the language.** The answer language line and the room's block carry the rule
    to the agent; no check, errand or withholding reads the language of the answer, and an answer in a language a
    person does not write goes out as the agent wrote it.

## A promise stands on a record

- A session's memory is gone the next time it is replaced, so a promise about what the agent will do from now on
  (to do something some way from now on, to check first next time, to keep something in mind, and a promise to
  behave some way only for this session above all) stands only where the same turn wrote it down: a working
  rule or standard as a note (which the digest carries into the room's rulebook and the library), something to do
  later as a memory, what to do whenever a situation comes back as a memory carrying a condition, a change to
  code or to a manual as an item or words appended to one.
  - **The code holds every answer to it**, as the process's `record` item, because only the code sees the
    turn's writes. An answer is read as promising when a sentence of its own (code and anything it quotes set aside)
    carries a first-person volitional ending beside a word of time to come or a word of keeping in mind, as the
    policy lists them. A promise about this very turn's work carries neither and is not one.
  - **Whether the turn recorded is read off what it wrote**: the records every run of the process's turns kept
    under the run (`run_records`), and the items bmo-job holds filed or appended from the room since the turn
    began.
  - **A run that wrote nothing is read off its calls instead**, out of the agency's run log
    (`GET /log-lines`): a call of one of the recording tools, or of the `bmo` verbs under them, that did not
    fail. A tool call counts by the record its catalogue entry declares for what it named. A verb is recognised
    by the word after the binary, the binary being the word `bmo` standing alone or ending a path however the
    path was written.
  - **An answer that promised with nothing written** is asked once more, the `record` item, to record the
    promise and write the answer again, saying where it went where that is a note or an item, and saying nothing
    about where it went where the place is the agent's memories, which are quiet in every room; a turn whose
    tools are off is asked to take the promise out instead. The second writing is the answer where it says
    something; otherwise the first one stands.
  - **A promise to act once something happens** (to report back when something is done, to do one thing after
    another finishes) stands only where a turn of the process set what wakes the agent when it does: a watch, or
    a memory carrying an event or a check to wait on. An answer that made one with nothing set is asked once more,
    the `wake` item, to set it, the same way. The receiver answers the brain whether a text waits on a happening
    by the same reading (`POST /promises/waits`).

## A rule the owner sets stands on a row

- The check for a rule the owner sets sits beside the promise's. It reads the turn's own incoming messages rather
  than the answer, and only the owner's: what anybody else asks for in a room is that turn's request.
- Words are read as setting a rule when a sentence of the owner's (code and anything quoted set aside) tells
  somebody to do something and carries the reach of a rule with it: a word of time to come, or a condition
  pointed at a manner, as the policy lists them. A request for this once carries neither and is not one.
  - **Where the owner set one and no turn of the process filed a rule**, the session is asked once, the `rule`
    item, to file it as a situation memory and write the answer again saying so. Once and no more: where it
    still files nothing the answer stands, and the rule is not invented for the owner. A turn whose tools are off
    cannot file one and is not asked.
