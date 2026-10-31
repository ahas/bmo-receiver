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

## Secrets taken out

- **A message's words are read for secret-shaped runs before anything keeps or forwards them,** and each run is
  replaced by the marker `<redacted-secret>`: in the row the record keeps, in what is carried to the room's other
  providers, in what a turn is handed and in what the console's live events and pages read. So are the words of an
  edit, a file's caption, every string of the payload a chat service hands over beside a message, what bmo-ear heard
  in a voice note, the new words a person's edit from the console carries, and the words and captions the owner puts
  in a room or says to a room's session from the console (`docs/receiver.md`, 「The owner's words」).
- **The shapes are data,** the policy's `secret-shapes.toml` (`docs/policy.md`), read in the order it lists them,
  each a regular expression with a name; where a shape names a part of its run `secret`, that part alone is
  replaced, so a `name=value` keeps its name and loses its value. As it stands it holds a platform's 64-character
  hexadecimal token; the provider keys written `sk-` and a run after it (OpenAI, OpenRouter, DeepSeek and
  Anthropic alike), GitHub's personal, OAuth and app tokens, Slack's tokens, an AWS access key id, a Google API key
  and a Hugging Face token; a JWT; a Telegram bot token; the session strings Telethon and Pyrogram write, a run of
  three hundred or more base64 letters; a PEM private key block, to its end or to the end of the words where it has
  none; and a name ending in `key`, `token`, `secret`, `password` or `passwd`, in any case, set with `=` or `:` to
  a value of twelve or more characters. Each is narrow enough that an ordinary sentence in any script, and a
  commit's forty-character hash, stand as they were written.
- **The value is kept nowhere and handed to no one:** no row, no copy carried to a provider, no turn and no log
  line carries it. Every row the record writes holds its words and its transcript as the shapes leave them, an
  agent's own answer among them, though what an agent sends is its own words and goes out as it wrote them. For a
  message a chat service handed over, the journal says how many runs it lost, by its room and number, and nothing
  of its words.
- **The marker is no word to the wake** (`docs/wake.md`, 「The names and the window」).
- **A message of the owner's that holds the marker reaches a turn with a note** beside it: a secret was taken out
  before the message was kept or passed on, the agent is never shown it, and the agent tells the owner so in its
  own words, with where a secret goes instead: `bmo-secret put <name>`, a command on the operations machine that
  reads the value from a hidden prompt, or the console's secret store, 「비밀 보관함」. The note is read off the
  marker the row holds, so a row `redact-kept` cleaned carries it as well.
- **A row kept without this reading is cleaned by `bmo-receiver redact-kept`** (`docs/boundary-api.md`, 「The
  service and its commands」), which reads the same shapes over the record's rows in place.

## Every kind of message

- **What a message is, the chat service says.** A message arrives as the chat protocol's `ChatMessage`: its words,
  and for every kind that is not plain words (a location, a contact, a poll, a sticker, a video, a voice note, a
  document, someone joining or leaving, a pin, a title change) the short bracketed placeholder its chat service
  writes for it, carrying what a reader could act on, plus the caption when there is one. It is archived and
  routed like any message. A message whose only words are a thing it carried reaches the session with that thing's
  note beside its empty words (`docs/room-files.md`, 「Who a file wakes」; `docs/pictures.md`, 「Pictures
  received」), never as a line with nothing in it.
- **Anything forwarded is prefixed with who it came from,** as the chat service hands it over.
- **A voice note is heard before it reaches the turn.** Once its recording stands, the receiver hands the saved
  file to bmo-ear (`ear/listen`, through the tool host) and keeps what was heard, how long it ran and whether
  it could be heard on the message's row. The turn reads it as a voice message with its length and its transcript;
  a recording that could not be heard is one line saying so, never silence. The console shows the transcript under
  the player. A voice note still being gathered is heard when its file comes.
- **A signed-in person acts on a message from the console** as the agents do on the record: reacts, pins, edits
  and deletes their own, and sends a sticker of the record's set (`docs/api.md`, 「Messages」); a room's pinned
  messages are listed newest pin first.
- **The owner forwards a message from the console** into another room: a copy kept there as his, marked as
  forwarded with where it came from and who said it, carried out on that room's own road and waking nobody
  (`docs/api.md`, 「Sends」).
- **An edit (`POST /edited`) changes the archived text in place and wakes nobody.**
- **A message kept in a room goes to each of the room's other chat providers** (`docs/receiver.md`, 「A room's
  chat providers」); a message read back from a room's past goes nowhere.

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
- **On the record, a room's road of its own, the record keeps every act on its messages,** and the console is the
  reader: a reaction stands in the message's reactions under who put it there, one to a reactor, an empty emoji
  taking that reactor's off; a pin stands as the moment it was pinned, taken off by an unpin; an edit stands as the
  new words and the moment they changed; a delete keeps the message in its place in the room's line with its words
  cleared and the moment it went; a sticker is a message of its own, its kind `sticker`, its words the sticker's
  emoji, and its picture where the deployment holds one. Each tells the room's screens with `message.changed`, and
  each goes on to the room's other providers wherever the message stands under a number of the provider's own,
  as a kept message does (`docs/receiver.md`, 「A room's chat providers」).
- **Everyday conversation is answered with a sticker** (a greeting, a joke, small talk, a remark that only wants a
  nod) when one of the set fits, sending it and then answering `<<SKIP>>` so no words follow; words are for
  questions, errands and reports.

## Typing

- **The typing state is shown only while an answer is actually on its way to the room.**
- **Nothing reaches a room until the round's checks have let an answer through:** the run's words come back whole,
  not as they are written, and until the code's own checks are done the round may still end in silence, whether
  the run said `<<SKIP>>` or `<<WAIT>>`.
- **So every round shows typing from the moment an answer is let through,** one that read new messages and one
  that carries only a quiet instruction alike, through the send, and a round that ends without
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
