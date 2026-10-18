# Wording: the terms this repository's text bans

The text this repository writes is English: the sentences a caller reads with a refusal, the lines an agent
reads, and the documents. The terms below are the ones that came in with the single repository this one was
separated from, or that name the whole system where only part of it does the thing. A term missing from the
list is held to the same rule: name the unit that does it, in the words that unit's own document uses, and
write no word for the system as a whole where a service, an agent or a chat service's account is meant.

The Korean this repository holds is data the code matches on, kept under `policy/` and `tests/fixtures/`; it
is nobody's sentence to hold, so no Korean term is banned here.

## Banned terms

- Each line is `<language> · <banned term> → <what to write>`, optionally followed by ` — <when it applies>`.
- The wording check reads every tracked file that is not a test file, a line at a time, on the lines of the
  term's own language.

- en · the bot → the service or the agent — the single repository's word for the whole system; this repository is one service, and the one answering a room is an agent
- en · The bot → The service or The agent — the same word opening a sentence
- en · bot's → the service's or the agent's — the same word with its ending
- en · a bot → the account the road sends through — what a chat service stands as in a room
- en · chatbot → the agent — what the system calls the one answering a room
- en · bmo-server → bmo-receiver — the single repository's service, not this one
- en · the account service → the web unit — the unit that holds the accounts and the language packs

## Allowed uses

- Each line is `<language> · <banned term> · <file path> · <an exact excerpt of the line>`, followed by
  ` — <why it is right there>`.

- en · bmo-server · docs/inventory.md · bot/bmo-server/src/ — the folder of the single repository the inventory reads its members from, named to say where they came from
