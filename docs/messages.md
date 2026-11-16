# Messages in and out

What becomes of a message a chat service hands in (how it is archived, what each kind turns into, how far an
agent has read), what an agent can send back, how words anything else wants said reach the room's supervisor,
and when a room sees it typing.

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

## A sender matched to the room's members

- **A message's sender is the person the room's member list names,** under the number the room's own road gives
  that person, so the face the console draws beside a message, the member sheet (`docs/api.md`, 「Rooms」) and the
  profile pictures (`docs/pictures.md`, 「Profile pictures」) name one person by one number.
- **A road can number one person twice:** words kept under a number the road gave when it read the room another
  way, and the same person in the room's member list under the number the road gives now. Such a sender holds no
  profile picture, since the road keeps its pictures under the number it gives now.
- **As the service starts, once the profile pictures are taken,** every room whose own road carried words from a
  sender that holds no picture has that road's member list asked once. Each such sender of the words that came on
  that road is compared by nickname, the name its words were archived under, with the names the list gives:
  - **one member of that nickname, with a number:** the sender's messages in that room, its mute there and its
    reactions there move to that member's number, in one write; the names the messages were archived under stay
    as they were;
  - **no member of that nickname:** nothing moves, since the person left the room or the list does not show them;
  - **two or more members of that nickname, or several nicknames of the sender naming different members:** nothing
    moves, and the log names the room and the nickname, since the name alone cannot say which person it is.
- **Never moved:** the owner's number and every number the room's road maps onto the owner, every number an
  agent's own words stand under, and a sender the member list already names. A room whose road lists no members,
  holds none of the room or does not answer is left as it stands and asked again at the next start.

## Secrets taken out

- **A message's words are read for secret-shaped runs before anything keeps or forwards them,** and each run is
  replaced by the marker `<redacted-secret>`: in the row the record keeps, in what is carried to the room's other
  providers, in what a turn is handed and in what the console's live events and pages read. So are the words of an
  edit, a file's caption, every string of the payload a chat service hands over beside a message, what bmo-ear heard
  in a voice note, the new words a person's edit from the console carries, and the words and captions the owner puts
  in a room or says to a room's session from the console (`docs/receiver.md`, 「The owner's words」), the
  progress lines a working turn's picture carries (`docs/receiver.md`, 「The event stream」), and the thinking
  block that picture carries (「A working turn's thinking」 below).
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
- **An account forwards a message from the console** into another room: a copy kept there as the forwarding
  account's, the owner's where the call names no account, marked as forwarded with where it came from and who said
  it, carried out on that room's own road and waking nobody
  (`docs/api.md`, 「Sends」). A picture the message stores is kept again under the copy, so the copy shows and
  reads as a picture wherever the original does, and goes to the room's other providers as a picture with the
  message's words as its caption; a picture whose file is no longer there leaves the copy with the words alone.
- **An edit (`POST /edited`) changes the archived text in place and wakes nobody.**
- **A message kept in a room goes to each of the room's other chat providers** (`docs/receiver.md`, 「A room's
  chat providers」); a message read back from a room's past goes nowhere.

## What an agent can send back

- **Each send is an entry of the send tool** (`docs/chat-mcp.md`), and some are a noun of the receiver's command
  line (docs/boundary-api.md, 「The unit command」), which sends only inside the agent's own round and hands its
  words over everywhere else (「Words handed to the room's supervisor」):
  - text (`say`; `bmo room say`, whose words are said by a run of their own);
  - a photo, or several as one album (`photo`; `bmo photo`);
  - a file from this machine, or several as one album (`file`);
  - a voice note, OGG/Opus with its caption (`voice`);
  - a fixed line, a code and its fields the web unit renders, said without the output gate or any session
    (`fixed_line`);
  - a reply pinned to a message (`<<REPLY>>`);
  - an answer folded where its road's dialect folds (`<<FOLD>>`, `docs/wake.md` 「Folds」);
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
- **A send that puts a message in the room is kept in the room's record as the words of the agent it went out
  as,** whichever road carried it, the road's own voice where no agent's turn asked it. It is kept once the road
  has answered, under the number the road gave the message (`docs/unified-bmo.md`, 「The cases」), the record's
  own number where the road is the record or gave none, filed under the number the room's road gave the agent
  where it numbered it and under who the agent is in the room otherwise, with the agent named. Its words are what
  the record keeps of a send: a line as it stands, a picture under its caption, a file, a sound and a voice note
  under the marks the policy keeps them with. It goes on to the room's other providers as every kept message
  does. A reaction, a pin, an edit or a delete puts no message in the room, and a send of which nothing went out
  keeps nothing.
- **On the record, a room's road of its own, the record keeps every act on its messages,** and the console is the
  reader: a reaction stands in the message's reactions under who put it there, one to a reactor, an empty emoji
  taking that reactor's off; a pin stands as the moment it was pinned, taken off by an unpin; an edit stands as the
  new words and the moment they changed; a delete keeps the message in its place in the room's line with its words
  cleared and the moment it went; a sticker is a message of its own, its kind `sticker`, its words the sticker's
  emoji, and its picture where the deployment holds one. Each tells the room's screens with `message.changed`, and
  each goes on to the room's other providers wherever the message stands under a number of the provider's own,
  as a kept message does (`docs/receiver.md`, 「A room's chat providers」). An act carried in a voice the provider
  refuses it in goes again in the voice the provider answers the room by, where that is another voice and the
  provider does not read the room in the owner's account alone. An act that does not go is never passed over
  without a word: each provider that holds no number for the message, that stands under no chat service, or that
  turns the act down in every voice it has for the room is named as not carried, in the service's log and — where
  the act came from the console — in the answer's `notCarried` (`docs/api.md`, 「Messages」). The record keeps such
  an act as it keeps any other, and the room's screens are told of it the same way.
- **A chat service's word of what happened to a message on its platform is kept the same way** (bmo-chat's
  `docs/contract.md`, 「The receiving side, as a provider sees it」): a reaction change stands in the message's
  reactions, each person it lists under their number, or under the provider's key where it names the account
  itself, taking the first emoji listed for them or none, and where the change is the message's whole set, every
  other reactor's taken off; a message deleted on the platform keeps its place and its words, marked with the
  moment it went, so the console can still open it. Each tells the room's screens with `message.changed`. A
  reaction change for a message the room does not hold is answered `not-found`, and a deletion passes over one.
- **Everyday conversation is answered with a sticker** (a greeting, a joke, small talk, a remark that only wants a
  nod) when one of the set fits, sending it and then skipping in the round's form so no words follow; words are for
  questions, errands and reports.

## A working turn's thinking

- **A working turn's thinking is shown in the turn's picture and never sent as a message.** The picture of a
  supervisor's turn working in the owner's private chat carries the last thinking block its run wrote, as
  `thinking` (`docs/receiver.md`, 「The event stream」): the block as it stands, in its own language and
  untranslated, its secret-shaped runs taken out as any message's are, cut at the policy's ceiling with an
  ellipsis marking the cut (`chat-round.toml`, `[thinking]`). A newer block takes its place, and it goes with the
  picture when the turn ends: no room's record keeps it.
- **Only a turn in the owner's private chat carries it.** Thinking can quote private or other-room material and a
  group room holds other people, so the picture of a turn in any other room carries none.
- **A message of kind `thinking` is no part of a room's conversation.** Where a room's record holds one, the room
  is shown it and no turn is handed it: the room's recent messages as a turn, a situation note or a keepalive's
  touch reads them leave it out, it is none of the unread messages an agent's book is handed, and it never counts
  as the agent having answered what the room said. Nothing the receiver sends is of that kind.

## Words handed to the room's supervisor

Every message a room receives comes from that room's supervisor, the one that answers it. Anything else
that wants something said there hands the words to that supervisor, and the supervisor says it in its own turn.

- **A send is carried only in its supervisor's own round.** `POST /sends` carries an act as asked where its asker
  names an agent, no job asks it, and that agent has a round working at that moment in the room that set the send
  off (the asker's `by`, `chat:<room>`): the round's own turn and every later question of its draft process, in
  which the send tool's entries and the shell verbs of that turn run. The room it sends into may be another room
  the agent answers. A round's own answer is the other way a supervisor speaks, and asks nothing of this.
- **A voice briefing turn is no round to send from.** The turn that writes a line's summary or full reading
  answers with the script alone, and a send its agent asks into that room meanwhile is refused `briefing-turn`
  (`docs/voiced-notices.md`, 「The summary of a notice turn」).
- **Every other send is handed over, and nothing reaches the room on that call.** A command at a shell, a job's
  run, a worker, a runner, a unit's own process, a script or a turn's tool call that failed over to the shell asks
  the same route, and the receiver queues a quiet instruction marked as words handed to the supervisor and
  carrying what was asked as data: who handed it, the command or unit it came through where the asker names one
  (`from`), the room, the act and its arguments (the words, the pictures, the files, the voice note, the
  reaction, the pin, the sticker, the draw or the fixed line) and the road it named. The rule the marker stands
  for — that every message a room receives comes from its supervisor, that it says the words there in its own
  turn or skips in the round's form, and that an act handed as an entry of the send tool goes out through that same
  entry — stands in the room's block. The asker is answered with one line naming the agent
  it was handed to, and the instruction's number as `handed`.
- **Who it is handed to:** the asker's own agent where that agent answers the room (a supervisor's shell outside
  its round, a process of its own past its turn), and the room's voice otherwise (`docs/voiced-notices.md`,
  「The rule」).
- **The supervisor decides in its own turn.** The instruction is an order of the sort `handed`, queued for the room
  the words are for and on the road the asker named; it opens a turn there and never rides. The supervisor says
  it in its own words and voice, as its rule for handed lines says (the agency's chat manual): it relays it as it
  stands, rephrases it, folds it, pins it as a reply, speaks it as a voice note, or judges it not worth saying and
  skips in the round's form. Whatever it sends is its own send, through every gate its turn goes through. An act
  handed as an entry of the send tool goes out through that same entry where the supervisor sends it: a picture as a
  picture, a file as a file.
- **A voice note is handed as its words.** A spoken talk asks the act `speak` (the voice named, the written words,
  and the spoken words where given) before anything is synthesised. Outside the asker's own round it is handed
  like any other act, so the supervisor speaks it in its own voice; in the asker's own round the receiver answers
  `own` and sends nothing, and the command makes the note and sends it as `voice`.
- **A person's own words are not handed:** what the owner puts in a room from the console, forwards there or does
  to a message is his own (`docs/receiver.md`, 「The owner's words」), and a line the receiver says as the service
  is the system's own (`docs/voiced-notices.md`, 「System notices」 and 「Lines that stay fixed」).

## Typing

- **The typing state is shown only while an answer is actually on its way to the room.**
- **Nothing reaches a room until the round's checks have let an answer through:** the run's words come back whole,
  not as they are written, and until the code's own checks are done the round may still end in silence, whether
  the run skipped or waited.
- **So every round shows typing from the moment an answer is let through,** one that read new messages and one
  that carries only a quiet instruction alike, through the send, and a round that ends without
  sending anything never shows it at all.
- **Nothing shows it** in a room whose turn the rest's gate holds back, before the provider has answered again
  after an outage, or while that room's compaction is in progress, and a turn of a console talk never moves a
  room's typing state.
- **A chat service's typing state fades after a few seconds,** so it is renewed every four while on, one renewal
  per room however often it is asked for, and it is off whenever a round has ended, answered or failed; a failure
  to send the state never touches the turn.

## Read marker

- **The receiver records, per chat, the id of the last message an agent has finished answering.**
- **The marker advances only when a conversation ends,** so a run killed mid-answer leaves its messages unread.
- **A round that failed leaves its words unread.** A round whose message no send put into the room is such a
  round (`docs/wake.md`, 「The room's queue and the round」). A round that failed for anything but a stop asked
  of it puts the marker back where it stood before the round, and the words it took, with those that came in while it
  worked, wait for the room's next round, whatever opens it — the failure order among them. They are that
  round's new messages, so its turn stands under them as the failed one did: its speakers, its reference, its
  reply and what the owner is owed. Words given back the instruction policy's `restart_limit` times are left
  read with the round that failed last, whose failure order still stands. They are kept by the running service
  alone: after a restart the backlog finds them unread.
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
