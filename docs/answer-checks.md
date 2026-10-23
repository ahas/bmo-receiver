# The checks an answer passes

The checks the code holds every chat answer to before it goes out: the room's language, a promise that stands on
a record, a rule the owner sets that stands on a row, and the banned words of the deployment's wording document.
Each sends an answer back to the same session at most a fixed number of times.

## Where the checks stand

- **The round holds an answer to these checks, in this order:** an owed answer over a skip (`docs/wake.md`), the
  language, a promise recorded, a promise set to wake, a rule the owner set, and the banned words.
- **What the checks read stands in the receiver's policy**: the words of time to come, of keeping in mind, of
  waiting on something, of a condition and of a manner, the volitional and imperative endings, and the
  language check's thresholds (`policy/second-asks.toml`), and the Hangul syllable block a word is told by
  (`policy/scripts.toml`). These are words of the room's language the code matches on, and so data. So is
  every errand a second asking sends, in English as every agent's text is.
- **A second asking goes back to the same session** with the round's reference, the turn's scope, the messages
  waiting for the next turn, and a line saying later messages are not acted on now.
  - Its answer is written down as a turn's is, under the asking's own stage, as a judgement of the room
    (`POST /judgements` on the gate).
  - A run of it that failed is written as a judgement of the round with no figures, and a stop ends the round
    rather than the asking.

## The language an answer goes out in

- Everything the machine itself says to a chat session is English: the turn's system lines, a quiet instruction
  and the result it carries, and the note a replaced session holds of its own past.
- English is the language this machine talks to its agents in; it is not the language of the room, and a turn
  with nothing but the machine's language in front of it answers in it.
  - **The prompt names the room's language** on every turn that has the machine's language in front of it: a
    turn carrying a quiet instruction, a turn with no new message from the room, and the first turn after the
    session's memory was replaced (folded up, swapped for a fresh one carrying the note, or opened again because
    the account changed), which says it in the note about the replacement. The room's language is the language
    of the messages on the page and of the agent's own earlier answers to it, and the line names it outright.
  - **Every answer is held to it** before it goes out. In a room that has been speaking a language other than
    English, an answer is in the machine's language when its prose (what is left once code blocks, inline code,
    addresses and paths are set aside) runs to at least four English words and fewer than a quarter of its words
    are in the room's language. Such an answer is written once more by the same session in the room's language,
    and the second writing goes out when it passes; otherwise the first one stands.
  - **What is never turned back**: an answer carrying the room's language beside its code, its commands, its
    paths and its English names, since a particle on an English name makes the word the room's; a short answer
    whose English is only a name or two, and a model name, a version or a hyphenated code, none of which counts
    as an English word; and any answer in a room that speaks English.
  - Where the session is replaced inside the run, which is what the account change and the forced reset do, the
    prompt has already gone out without the line and the answer is held to the language on its own.

## A promise stands on a record

- A session's memory is gone the next time it is replaced, so a promise about what the agent will do from now on
  (to do something some way from now on, to check first next time, to keep something in mind, and a promise to
  behave some way only for this session above all) stands only where the same turn wrote it down: a working
  rule or standard as a note (which the digest carries into the room's rulebook and the library), something to do
  later as a memory, what to do whenever a situation comes back as a memory carrying a condition, a change to
  code or to a manual as an item or words appended to one.
  - **The code holds every answer to it**, after the language check, because only the code sees the turn's
    writes. An answer is read as promising when a sentence of its own (code and anything it quotes set aside)
    carries a first-person volitional ending beside a word of time to come or a word of keeping in mind, as the
    policy lists them. A promise about this very turn's work carries neither and is not one.
  - **Whether the turn recorded is read off what it wrote**: the records its run's writing verbs kept under the
    run (`run_records`), and the items bmo-job holds filed or appended from the room since the turn began.
  - **A run that wrote nothing is read off its calls instead**, out of the agency's run log
    (`GET /log-lines`): a call of one of the recording tools, or of the `bmo` verbs under them, that did not
    fail. A tool call counts by the record its catalogue entry declares for what it named. A verb is recognised
    by the word after the binary, the binary being the word `bmo` standing alone or ending a path however the
    path was written.
  - **An answer that promised with nothing written** is sent back to the same session, under `record-again`, to
    record the promise and write the answer again, saying where it went where that is a note or an item, and
    saying nothing about where it went where the place is the agent's memories, which are quiet in every room; a
    turn whose tools are off is asked to take the promise out instead. The second writing goes out when it is an
    answer; otherwise the first one stands.
  - **The answer that comes back is read the same way**, and where it promises again with nothing written it is
    asked once more with the tools off, which is the form that takes the promise out rather than recording it.
    That is the end of it (two extra rounds at the most) so no answer reaches a room carrying a promise nothing
    recorded.
  - **A promise to act once something happens** (to report back when something is done, to do one thing after
    another finishes) stands only where the same turn set what wakes the agent when it does: a watch, or a
    memory carrying an event or a check to wait on. An answer that made one with nothing set goes back under
    `wake-again` to set it, the same two rounds at the most, the last with the tools off to take the promise out.
    The receiver answers the brain whether a text waits on a happening by the same reading
    (`POST /promises/waits`).

## A rule the owner sets stands on a row

- The check for a rule the owner sets sits beside the promise's. It reads the turn's own incoming messages rather
  than the answer, and only the owner's: what anybody else asks for in a room is that turn's request.
- Words are read as setting a rule when a sentence of the owner's (code and anything quoted set aside) tells
  somebody to do something and carries the reach of a rule with it: a word of time to come, or a condition
  pointed at a manner, as the policy lists them. A request for this once carries neither and is not one.
  - **Where the owner set one and the turn filed no rule**, the round asks the session once, under `rule-again`,
    to file it as a situation memory and write the answer again saying so. Once and no more: where it still files
    nothing the answer stands, and the rule is not invented for the owner. A turn whose tools are off cannot file
    one and is not asked.

## The banned words

- An answer is held to the banned words of the deployment's wording document, which the configuration names,
  last of these checks, in every room and the owner's own chat alike. An answer that uses one outside its quoted
  spans is asked for once more, under `wording-again`, with each term named beside what to write; the second
  writing goes out when it is an answer in the room's language, and otherwise the first one stands. Where the
  document cannot be read, no answer is held to it.
