# Messages in and out

What becomes of a message a chat service hands in (how it is archived, what each kind turns into, how far an
agent has read) and what an agent can send back and when a room sees it typing.

## The archive

- **Every message a chat service hands in, in every chat, is stored** in the receiver's database together with its
  sender.
- **An answer of an agent's own is stored with how long the room waited for it**: the whole seconds from the arrival
  of the earliest message that answer answers to the moment the answer went out, both ends on the chat service's
  own clock; an answer that began no wait is stored without one.
- **An agent can scan the whole archive,** newest to oldest, with its `archive_scan` tool (`bmo archive scan` under
  it), or read one message by its number with its `id`.
- **A scan answers under the standing ceiling on what a verb returns**: the messages that fit, a long one cut in
  place, and the id to carry on from, while `--id` hands back the one message it names whole.

## Every kind of message

- **What a message is, the chat service says.** A message arrives as the chat protocol's `ChatMessage`: its words,
  and for every kind that is not plain words (a location, a contact, a poll, a sticker, a video, a voice note, a
  document, someone joining or leaving, a pin, a title change) the short bracketed placeholder its chat service
  writes for it, carrying what a reader could act on, plus the caption when there is one. It is archived and
  routed like any message.
- **Anything forwarded is prefixed with who it came from,** as the chat service hands it over.
- **An edit (`POST /edited`) changes the archived text in place and wakes nobody.**

## What an agent can send back

- **Each send is an entry of the send tool** (`docs/chat-mcp.md`), and some are a noun of the receiver's command
  line (docs/boundary-api.md, 「The unit command」):
  - text (`say`; `bmo room say`, whose words are said by a run of their own);
  - a photo, or several as one album (`photo`; `bmo photo`);
  - a file from this machine, or several as one album (`file`);
  - a voice note, OGG/Opus with its caption (`voice`);
  - a fixed line, a code and its fields the web unit renders, said without the output gate or any session
    (`fixed_line`);
  - a reply pinned to a message (`<<REPLY>>`);
  - an emoji reaction (`react`; `bmo react`, allowed in restricted runs too);
  - one of its own stickers (`sticker`; `bmo sticker`, from the sticker set the deployment names, picked by
    feeling emoji, allowed in restricted runs too, archived like a received sticker);
  - a pinned message (`pin`; `bmo pin`, the owner's alone by the session's judgement).
- **A send asks the room's road first:** the road its caller's turn is read on (`via`) where that road answers in
  the room, the room's own otherwise; the agent whose turn it is, which sends only into a room it answers and only
  where its role gives sending; and whether the road performs the verb at all, as its chat service's capabilities
  declare. A refusal is the road's one sentence, and nothing is sent.
- **Every send goes through bmo-chat-client** to the chat service the room's road names (`POST /send`,
  `POST /media`), in the words' form the dialect of that chat service reads (`POST /dialect/read`).
- **Everyday conversation is answered with a sticker** (a greeting, a joke, small talk, a remark that only wants a
  nod) when one of the set fits, sending it and then answering `<<SKIP>>` so no words follow; words are for
  questions, errands and reports.

## Typing

- **The typing state is shown only while an answer is actually on its way to the room.**
- **Nothing reaches a room until the round's checks have let an answer through:** the run's words come back whole,
  not as they are written, and until the code's own checks are done the round may still end in silence, whether
  the run said `<<SKIP>>` or `<<WAIT>>`.
- **So every round shows typing from the moment an answer is let through,** one that read new messages and one
  that carries only a quiet instruction alike, through the sanitiser and the send, and a round that ends without
  sending anything never shows it at all.
- **Nothing shows it** during a rest in a room no stand-in is carrying, before the provider has answered again
  after an outage, or while that room's compaction is in progress, and a turn of a console talk never moves a
  room's typing state.
- **A chat service's typing state fades after a few seconds,** so it is renewed every four while on, one renewal
  per room however often it is asked for, and it is off whenever a round has ended, answered or failed; a failure
  to send the state never touches the turn.

## Read marker

- **The receiver records, per chat, the id of the last message an agent has finished answering.**
- **The marker advances only when a conversation ends,** so a run killed mid-answer leaves its messages unread.
- **It only ever moves forward in the room's line:** a conversation handed older words after newer ones still marks
  the newest it read.
- **The brain's read marks are another record:** one per agent and message, kept for its recollectors
  (`POST /read-marks/keep`), and never moved by a conversation.

## Backlog

- **On start-up the receiver re-reads the unread messages of every chat** and answers them if one of them is recent
  enough (six hours) and either calls the agent by name or sits in the owner's private chat, where no name is
  needed.
- **The scan runs a few seconds after the chat services report ready,** so the messages a chat service held while
  the receiver was down are archived first and answered in the same pass. A message still waiting in its room's
  queue when the receiver stopped is one of them, since it was archived before it queued.
- **A chat already talking when the scan runs is not passed over** (most often the turn that says the agent is
  back, which reads no message as its own to answer): its unread messages are handed into that talk as new
  messages, and one the talk already holds is not taken twice.
- **A chat with no marker yet is marked as read up to its newest message instead,** so old history is never
  replayed.
- **Messages already answered are ignored when a chat service hands them over again.**
