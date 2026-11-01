# The receiver

What the receiver is, what it carries, and how it stands among the units around it.

## Where it stands

```mermaid
flowchart TD
  CS[Chat services<br/>through bmo-chat] -->|POST /message, /attachment, /edited, /gathered| Rx[Receiver: rooms, wake, roads]
  Console[Console] -->|POST /owner/messages, /owner/message-files, /owner/instructions, reads| Rx
  Rx --> DB[(The receiver's database)]
  Rx --> Conv[The one runner:<br/>each agent's conversations]
  Pass[The instruction pass] --> Conv
  Job[bmo-job] -->|POST /instructions, /rooms/{roomId}/notices, /talks| Pass
  Conv -->|the envelope| Gate[the gate]
  Conv -->|POST /sessions/{sessionId}/turns| Agency[the agency]
  Agency -->|turn.ended and its other events| Conv
  Conv -->|answers, through bmo-chat-client| CS
  Rx -->|GET /events, GET /live| Screens[Console screens]
```

- **One receiver.** It stands where its configuration names its database, the owner and the chat agent; without
  the owner it stands no receiver and says so on its output.
- **What it carries:**
  - the secret-shaped runs taken out of a message's words before anything keeps or forwards them
    (`docs/messages.md`, 「Secrets taken out」);
  - the judgement of a message: whether the room is listened to; the owner's commands (allow, shut, pictures,
    mute, the two notices) with their acknowledgements queued as voiced notices; the
    message kept with the room's title; an agent's own words and a message already read stopped there; an edit
    written in place;
  - the wake: which designated agents of the room a message wakes, by the one rule (`docs/wake.md`), with
    everything that stops a message before it:
    - the owner's answer to a job's open question in the owner's private chat, handed to bmo-job;
    - the rest's gate, read off the agency's rest (`GET /rest`, and `GET /stand-in` for a road the rest closes);
    - the read mark and a mute;
    - the window each agent keeps in the room, and the one a call opens for it;
    - the files a person posted just before the words of theirs that woke the chat agent;
  - the conversations (`docs/wake.md`, `docs/bmo-window.md`): each room's queue, rounds, window and waits, the
    round's turn on the agent's session with the room's words and the quiet instructions that arrive while it
    works pushed into it, the draft process its answer is made in and the room's words no push put in shown at its
    question boundaries
    (`docs/drafting.md`), the answer sent on the road the turn
    answers on with the room shown typing meanwhile, the fold, the failure orders and the cache warmer's touch;
    each answer sent is handed back to the receiver like any word filed in the room;
  - the instruction pass over every agent's rows (`docs/bmo-window.md`, 「The agent's queue」): each row handed to
    its room's conversation on the tick and at once where a row was queued, cold wakes paced, the rows that ride
    a round handed to it as it starts, and, as the receiver starts, every row a stopped round left handed and
    unsettled made pending again;
  - the restart (`docs/voiced-notices.md`, 「Deploy and restart」): a rest lifted past its end, every room but the
    owner's read up to its latest message and the owner told where it was a limit's rest; and, as the receiver
    starts, the owner's back notice with the deployment's unit, commit, subject, item and what it cut, then the
    words a restart left unread handed to every room that answers, one room at a time and in room order, the
    room's read mark advancing as its turn goes out; a room with nothing waiting is not served, and a line already served
    is not served again;
  - the console's talks about an item and a document (`POST /talks`), each turn a round of the owner's chat on the
    chat agent's session;
  - the one runner: each woken agent's turns in its conversations, the chat agent's and every other agent's alike,
    every agent's rows of the instruction pass, and the words the room sessions left untaken taken up as the
    receiver starts, each answer handed back to the receiver like any word filed in the room;
  - the console's road in: `POST /owner/messages` and `POST /owner/message-files` into any room it holds, the
    second with `voice` for a recording;
  - the delivery of every kept message to the room's chat providers (「A room's chat providers」 below);
  - the words' form of its own sends: the scaffolding guard, the dash net, and the words handed to the room's chat
    service in the form its dialect reads; refused where nothing but scaffolding is left;
  - the going-down line said to the owner as it stops (`docs/voiced-notices.md`, 「System notices」);
  - the event stream and the live board (「The event stream」 below).
- **What it does not carry:** a chat service's platform (its reading, its sending, its gathering of files and its
  numbering are the chat service's, behind bmo-chat), the agent sessions and their stand-ins (the agency's), the
  gate stage (the gate's), the memories and digests (the brain's), reminders, waits and watches (the wait tool's,
  run through bmo-cli's `bmo use`, `docs/reminders.md`), the job queue (bmo-job's), the accounts and grants (the web unit's), the voice (the voice
  unit's), and the keepalive pass (the cache warmer's).
- **A woken agent's turn is its conversations' own:** the message goes into its room's queue with the files
  leading up to it ahead of it, opening the window where its ground does. A turn the rest's gate closed is taken
  only where the agent is pinned to a provider the rest does not close, or a stand-in answers on its road. A
  message read through somebody else's account, or by a read mode its chat service declares answers cannot be
  pinned to, goes in as read through a reader, so no answer is aimed at it. The owner's words from the screen go
  into the owner's conversation the same way, opening nothing.
- **Another unit reaches the conversations through the instruction pass alone:** a row it queues is delivered as
  any is.
- **A row queued for an agent that no longer answers its room goes to the room's voice:** a run's ending whose
  asker was taken off the room or rests, or a job's instruction for an agent gone from the room, is queued for the
  agent that speaks for the room (`docs/boundary-api.md`, 「For bmo-job」), so the result is told in the room's
  own voice. A row whose agent stops answering after it was queued is let go by the pass, as every row no turn
  could take.

## The one hard rule

- **A chat service's key may be stored, logged, shown and looked up. It may never be compared to a literal.**
- Allowed: kept on a message row, written to the log, shown on a console screen, handed to bmo-chat-client to reach
  that service.
- Forbidden: a comparison against any service's key anywhere in the receiver, and equally a chat-number range test
  or a message id sign test standing in for one.
- **Every difference is asked of the declaration instead** (`GET /declaration` and the room routes): the
  capabilities, the read modes, the dialect, the numbering, who the agent is in this room, whether this room is
  the owner's private chat, whose one-to-one with the agent it is, whether the service lists its account's rooms
  and whether it answers in the account of the person it reads.
- **A screen follows the same rule:** what the receiver answers a screen about a provider is read off its
  declaration, so a screen offers each provider what it declares and branches on no provider's name either.
- The reason is the cost of the next chat service. A comparison to a literal is a branch somebody has to find and
  extend; a question asked of the declaration answers itself for a service that did not exist when the question
  was written.

## A room's chat providers

- **A chat service that refuses the connection is skipped at once for the turn.** Its client offers the request
  once, the receiver logs the failure once, and the turn goes on to the room's other roads; a service that is
  live but slow is still waited on. The 120-second patience other units use is not spent per room on a road that
  is down.

- **A room is on one or more chat providers, and which is one setting of the room's registration.** A provider is
  a road a room's messages come and go on, and the setting says both ways which: whether a messenger's messages
  for the room are taken, and whether what is said in the room goes out to that messenger. Every room stands in
  the web console whatever its providers, since the console reads the archive. each chat service the configuration lists, and the room's record, the
  road the receiver keeps itself, which is the web console's, since the console reads the archive. Each is named
  by its key, as the record's stands in the policy and a chat service's in the configuration; nothing here names
  one provider apart from the others.
- **The list is read and set over `GET` and `PUT /rooms/{roomId}/providers`** (`docs/api.md`, 「Rooms」). A room
  whose list was never set is on every provider that has handed it a message or kept one in it, in the order
  each first did, and on the chat service that says the room is its own, so a room stands where it stood until
  its list is set.
- **Every message kept in a room goes to each of the room's providers,** whoever said it and whichever road it
  came in on: a person's words a chat service handed over, the owner's words from the console, an agent's
  answer, a line the receiver says itself. The rule is the same in every room and turns on nothing but the list.
  - A provider the message already stands on is not handed it again: the road it came in on, and the road a
    line of the receiver's went out on, under whose number the record keeps it. The record holds every kept
    message by being the archive, so it is never sent anything.
  - Every other provider on the list is handed the message through its own `POST /sends`, as the words or the
    files the message carried, the caption with the first file, answering the message it answers where that
    message stands on that provider under a number of the provider's own.
  - A message the owner said goes in the owner's own voice, `human-reader`, as the chat protocol names a send in
    the owner's own words; any other goes in the voice the provider answers the room by.
  - Each send carries a `tag`, and the number the provider gave it is kept beside the message. A message a
    provider hands over that is the echo of such a send, by its tag or by that number, is the message already
    kept: it is answered as held, kept no second time and wakes nobody.
  - A message a provider hands over that answers its copy of a message kept under another number, by the number
    the provider gave the copy, answers the kept message: it is kept naming the number the room keeps that message
    under, and is read, woken on and carried on as a reply to it.
  - A provider that refuses or does not answer is said in the journal and leaves the message kept as it is.
- **A chosen list rules both directions.** A message a chat service hands over for a room whose chosen list does
  not name that service is not taken: it is not kept, wakes nobody and is not answered, and the journal says so
  once for the room and the service. Nothing of the room goes out on a chat service the list does not name: no
  answer, no send, no line the receiver says itself, no notice and no picture.
- **A chosen list names the room's own road: its first provider.** A turn whose words came in on a road the list
  names answers on that road; every other answer, every send that names no road or one the list does not name,
  and every line nobody asked for goes on the room's own road. Where a room has chosen its list, neither whether a
  road shows the room's whole line nor whether a chat service reaches the room beyond its own rooms is read for
  it: the list alone decides.
- **A room on the record alone** reaches no messenger: what is said there stands in the archive and the console,
  whether or not a chat service holds the room. A room whose list names the record alone is such a room; so is a
  room no chat service holds and whose list was never chosen, where the receiver numbered it on the record or
  keeps messages of it: an agent's console room is born with the record as its one provider.
- **No chat service is asked of a room the record holds alone by its own rows:** one whose chosen list names no
  chat service the configuration lists, or one whose list was never chosen and whose number the record gave. What
  such a room is comes from its record, whether every chat service is up or down: it is the owner's private chat
  only where it is the owner's private room the configuration names, it is no one-to-one, the agent's words there
  stand under no number of a road's, no road reads it in the owner's account, and its own road is the record. Its
  words are kept and wake its answerers by the one rule every room's words are judged by (`docs/wake.md`). Every
  other room's facts are asked of the chat services that may hold it.
- **Detaching a messenger is taking it off the list,** and attaching it again is putting it back, from the
  console's provider setting: from then on its messages for the room are taken again and every message kept is
  carried to it. Nothing else of the room changes, its record and its registration among them.

## A read-only room

- **A chat service reads a room in the owner's account alone** where its answer for the room states the room's
  `modes` and they hold `human-reader` alone. Where its answer states no modes, the room is read so where the
  service reads through the owner's account (it declares the `human-reader` mode) and either that is the one
  voice it has active, or its answer holds the room under no identity of its own: no words of its own account
  stand there, only the owner's. The service's declaration and its answer for the room are read as they are kept
  (「What is kept for a moment」), so a room whose service starts answering there, the account the road sends
  through joining it, stops being read so at the next read of its facts.
- **A room is read-only where its list of providers names at least one chat service and every chat service on it
  reads the room in the owner's account alone.** The room's record on the list changes nothing either way: a room
  on the record alone is answered in the console as any room is.
- **A read-only room is listed, read and kept like any other,** and its messages stand in the archive, where the
  agents read them as archived messages from their turns elsewhere.
- **The people's words in a read-only room are read only where the room was added.** A chat service reading a
  room through the owner's account asks the receiver for the room (`docs/api.md`, 「The chat services」), and the
  answer's `read` is true only for a room added on that service (「Adding a room」) that is still on its list of
  providers and is read-only; for every other room it is false. So a room of the owner's account that was never
  added hands over the owner's own words alone, and one added hands over everyone's.
- **The agents never answer in it:** no message wakes an agent there, whether it calls one by name, replies to
  one's words, falls in an open window or carries a picture (`docs/wake.md`, 「Who reaches the session」); a quiet
  instruction queued for it is let go undelivered; a send an agent asks into it is refused `read-only-room`
  before anything is sent or kept; and a line the receiver says itself is not said there.
- **The owner's own words still go out:** words the owner typed into the room from the console are kept and
  carried to its chat services in the owner's own voice, `human-reader` (「A room's chat providers」), as in any
  room.
- **A chat service that reads a room in the owner's account alone is handed the owner's words alone,** in any room
  whose list carries it: every other message kept there is not carried to it, and nothing else is sent to it in
  that room.

## Adding a room

- **A room is added by choosing a chat service, one of the rooms its account is in or a conversation started
  with its contacts, and a mode,** and the receiver owns the room and its mode from then on (`docs/api.md`,
  「Rooms」). Each choice is offered by what the service declares, asked afresh each time since a mode's standing
  moves while the service runs:
  - the rooms offered are the list of a service that declares `roomList`, relayed page by page as it answers it,
    each marked `held` where the receiver already holds it: its own, one it keeps a title or a message of, or one
    added. Whether a room is held is the receiver's own record, never the service's (bmo-chat's
    `docs/contract.md`, 「The rooms of the account」);
  - the contacts offered are the list of a service that declares `contactList`, relayed the same way, each with
    the one-to-one the service's account already has with it, marked held as a room is. A one-to-one is started
    with one contact where the service declares `startChat`, and a group with several under a title where it
    declares `startGroup`; the service starts it, and the room it comes to is added as a room chosen from the
    list is. What the started room is, read-only or answered, is the service's answer for it;
  - `record-only` is offered on every chat service, and `avatar` only where the service's `human-avatar` mode
    stands active. A platform whose terms forbid automating a person's account declares that mode `unsupported`,
    so it is never offered there. A room whose answer states `modes` without `human-avatar` takes no `avatar`
    either: a read-only room is never answered in.
- **Adding keeps the room on the service and the record:** the room's mode, its title and its list of providers
  set to the service it was chosen on and the room's record (「A room's chat providers」 above), so whatever mode
  it is in, every message said in it stands in the archive. A room the service does not say is its own, a mode
  the service does not offer or the room does not take, and a room already added are refused, and nothing is
  written. A mode the service does not offer is refused before a conversation is started, so nothing is made on
  the platform for an addition that cannot stand.
- **The mode is what the room is to the agents:**
  - `record-only` is the user alone: the room's messages are kept and carried as any room's, and no agent is
    woken by them (`docs/wake.md`, 「Who reaches the session」);
  - `avatar` is a room answered as the user: the room's designated agents are woken by the one rule, and every
    send the receiver makes there that names no voice of its own, an answer, a line or a typing indicator, goes
    in the `human-avatar` voice, the account of the person the service reads.
- **A room never added has no mode** and stands as the roads that handed it messages make it.

## The owner's words

- **`POST /owner/messages` and `POST /owner/message-files` put the owner's words into a room from the console.**
  - The words, and the caption a file or a voice note carries, are read for secret-shaped runs before anything
    is kept (`docs/messages.md`, 「Secrets taken out」); words that are nothing but markers are kept and wake no
    agent.
  - The words are archived in the room under a console number (`docs/unified-bmo.md`, 「The cases」), under the
    owner's number and the owner's own name, and handed to the room's conversations as any of the owner's words
    are; then they go to the room's other providers as every kept message does, in the owner's own voice.
  - A `replyTo` names the archived message the words answer; one the archive does not hold is refused 409 and
    nothing is kept.
  - Files are archived each as its own message with the words typed beside them; they go to each provider as
    one album of the pictures and videos, one of the sounds and each document on its own, the words on the first
    send that takes a caption that long and ahead of the files as words otherwise. A file
    past the ceiling a road takes is refused 413 and turned away with its name, and a call none of whose files
    stood answers as one refusal.
  - A voice note is the same form with `voice`: one sound file, kept as a message under the voice word and the
    caption beside it, and handed to the room's other providers as a voice message with that caption. The
    console records it; no reading of it is made, so what it says stands only in the recording. A call with
    no file, more than one, or one that is no sound is refused 400, and a file past the ceiling 413.
- **`POST /owner/instructions` says the owner's words to a room's session**, framed as the owner speaking from the console,
  read for secret-shaped runs first (`docs/messages.md`, 「Secrets taken out」) and delivered as a quiet instruction, so the room sees nothing but the answer it leads to; a chat the archive has
  never seen is refused.

## The event stream

- **The frames of `GET /events`:**
  - `event: run` first, naming the stream's run, the moment the receiver's process started. A cursor is read only
    against the run it was taken in; one from another run, or one naming no run, is handed the whole ring;
  - each event as `id: <n>`, `event: bot` and its JSON, out of a ring of the last 500, replayed after the cursor;
  - `event: live` with the picture as it stands, after the replay and with no id;
  - `: ping` every fifteen seconds;
  - `event: stopping` with no id when the receiver stops on purpose, after which the stream ends. The signal
    tells every stream at once and the serve lets go only after them, so a stop never waits out a stream that
    was not told.
- **What goes into it** is the news the receiver makes itself: a message it kept or wrote again, a window opened
  or restarted, a window closed as `window-closed` with its room, its agent and `closedAt`, the moment it closed
  (`docs/wake.md`, 「The names and the window」), a verdict it wrote of a round as `judgement` with its
  `messageId` (null where it stands under no message yet), `stage`, `verdict` and `detail`, each row of a round's
  draft process as `draft` with its `round`, `agentName`, `number`, `question`, `kind`, `answerChars`, `changed`,
  `outcome`, `detail`, `ms`, `at`, `reads`, `session` and `loaded`, on the end what the round read from the brain,
  the session its turn ran on and what that session had read in full in the room before it, and empty or null on
  every other row (`docs/drafting.md`, 「The round log」), each question of it as it starts as
  `draft` of kind `start` with its `round`, `agentName`, `number`, `count`, `question` and `at` and none of a
  row's answer fields, a turn of a conversation as
  it begins, moves, sends and ends, each of these under the turn's `speaker` (`docs/api.md`, 「Events and the live
  board」), a room whose
  registration was removed as `room-removed` with its
  number, the picture whenever it
  changes, and what `POST /events` says (a message or a briefing checked against the archive first, the profile
  pictures a chat service took).
- **The picture (`GET /live`) is drawn of what the receiver runs itself:** the chat turns of every agent's conversations,
  the rooms whose session it is folding and the message each room queued for a turn, the picture going
  out again as each of them begins, moves or ends. A turn names the messages it read and, as `messageId`, the one it
  reads last; words it takes while it works — pushed into it, or shown under a question of its round — move it on,
  so from then it names the newest of them as the one it reads last, among the rest it read. A queued message is
  let go once a turn of its room has read it — every message the turn read, those it took while it worked among
  them, not only the last — or when another is put in after it. With none of them standing, it is empty.
- **A turn's progress is read off the lines its run leaves in the agency's log as they come** (the agency's
  `docs/run-records.md`, 「Agent log」): the stream lines of an agent's own road, and the `progress` and `answer`
  lines a pane program's record writes. A record naming no phase, no tool or no usage leaves what the picture
  already holds: the phase, the tool being called, the thinking and the tokens each request spent, the written
  estimate, and how full the session stands. A pane program's `progress` line carries the turn's spend so far,
  which is set where it stands rather than added to; its `answer` line carries the turn's whole spend the same way.
- **`GET /health` answers for the receiver itself:** `ok`, the owner, `bot` with this process's number and the
  moment it started, and its version. A deployment's wait matches `bot`'s number against the unit's main process.

## Removing a room's registration

- **The administrator removes a room's registration** through `GET /rooms/remove`, which answers what would go,
  and `POST /rooms/remove`, which takes it. Both refuse anyone but the administrator (`GET /users/{user}` on the web
  unit). No other act or chat service's event removes a room.
- **What goes is the room's whole record in the receiver:** every row of every receiver table holding the room's
  number in a `chat_id` column, the tables read off the database's own schema as the removal runs rather than off
  any list (its registration and title, its switches and answerers, its messages and the metadata they arrived
  with, its reads, its collected-file rows, its draws); what an owner of room state outside those columns declares
  beside its own code (its picture, the settings that name it, its place in the console's pinned order, what its
  runs wrote down, and its folders in the message-file store, the picture store
  and the outboxes, `docs/room-files.md`).
- **Every other unit's part goes through that unit:** the agency forgets the room
  (`DELETE /room-holdings/{roomId}`), and every declared holder (`PUT /room-holders/{unit}`) is asked what it holds
  (`GET <address>/rooms/{chat}/holdings`): what it keeps stays and keeps the room's chat, what it warns of is shown
  in the confirmation.
- **The brain is not asked.** A room is one of its subjects, and what an agent remembers of the room stays as that
  agent's own memory after the room's registration is gone.
- **What stays is work filed from the room, and accounting:** its items, jobs, job questions and searches, each
  keeping the room's number as where it came from, the log lines of a job's turns, and the usage of every run
  there. Each table or row of the receiver's that outlives its room says so in a declaration beside the code that
  writes it, and the tests fail on any column holding a room's number that the removal neither sweeps nor finds
  declared.
- **A setting that names a room is made only through a declaration** whose prefix and shape build its key from the
  room's number, and the removal takes every key a declaration matches. Any other setting whose key holds the
  room's number as one of its parts is listed in the confirmation as undeclared and kept: a private chat's number
  may be its person's number too.
- **Some rooms are refused,** with the reason and nothing written: a room the agent room seeds declare, which the
  next start would make again; the infrastructure room the configuration names; and the administrator's own
  private chat on any road. Each refusal is declared by the code that depends on the room, not kept in one list.
- **The confirmation lists what goes and what stays before anything is taken,** with a count for each table, the
  folders with their files and every holder's answer, and it warns: of each other agent answering the room, which
  stays with this room no longer to answer in; of open items and job questions filed from the room; of another
  user's private chat, naming the user; of a road that can still reach the room, where a later message would make
  it anew.
- **Once begun it runs to its end,** whatever becomes of the request that asked for it: open turns in the room are
  cut and its sessions ended through the agency; every holder of live per-room state forgets the room (every
  agent's conversation there with its timer, its queue and its window, and the receiver's caches kept by room);
  the rows go in one write; once that write stands, the picture and the folders go; and the event stream carries
  one `room-removed` event naming the room.
- **Kept work whose room is gone reaches the administrator instead:** bmo-job reads a room whose registration is
  gone as working in the owner's private chat (`GET /rooms/{chat}`, `work_chat`), and an instruction queued for
  such a room, a run's ending among them, goes to the owner's private chat naming the room it came from
  (`docs/boundary-api.md`, 「For bmo-job」).
- **A room that speaks again afterwards is a new registration** with an empty record, answered as any room the
  receiver has never seen, numbered as its chat service numbers a room. None of the old record comes back.

## Identity, and reaching the owner

- **What a chat service says of a room is asked through its room route** (bmo-chat's `GET /rooms/{roomId}`): whether
  the service holds the room, the room's kind, the agent's number there and whose one-to-one it is. These four are
  the room's facts, and none of them needs the room's title, which the receiver reads off its own record; the
  route's title is read only where a room is added (「Adding a room」 above).
- **Who the agent is differs per room,** so it is asked per room, the room route's `identity`. A chat service where
  the agent holds an ordinary account has the agent's own words under an ordinary sender number, and a receiver that
  derived the agent's identity from one service's token would count its own words as unread and fail to recognise a
  reply to itself.
- **The owner's private chat is the room numbered as his own number,** the one he has always had, or the room the
  configuration's `owner_room` names in its place. It reads as the owner's private chat on every screen and in
  every round, and a chat service's `kind` of `owner` is read for no other room. Which messengers it is on is its
  list of providers like any room's: on the record alone, it is answered in the console alone, and a messenger he
  detached from it is attached again from the console's provider setting.
- **`owner-room` stands up a room on the record alone** (`docs/boundary-api.md`, 「The service and its commands」),
  the owner and the chat agent its members, for a configuration that names it as `owner_room`.
- **Whose one-to-one with the agent a room is, is a chat service's question too** (its `privateTo`). The
  receiver turns the answer into a user's private chat only where that person's sender is linked to a user
  (`docs/wake.md`, 「Who reaches the session」); a one-to-one with somebody linked to no user is an ordinary room.
- **In a room the console made, the chat agent and each agent added there have numbers of the console's own,**
  given as the rooms are stood up (`POST /companions`), and an agent's words are filed under the number it was
  given and under its own name, the label its registration carries (the chat agent's, the deployment's sender
  name), each message naming the agent that said it, so one room holds every speaker apart.
- **Every agent holds one number on the record,** under the record's own key, the one it was first given in
  whichever room: its words in every room the record numbers stand under it, and its kept picture with them.
- **Reaching the owner without naming a transport** (the login notice, the restart notice and a briefing's
  lines) is the owner's private room, kept in its record and carried to the providers its list names. A rest
  notice and the other system notices go to the notices room where one is named, as below.
- **Every reading of the owner's room is that one room:** a briefing's room always, the room the system's other
  notices go to where `notices_room` names none, the room an instruction goes to where it names none, the room
  kept work whose room is gone reaches, the room a console talk and a line typed into the agent's program run as,
  the room that is always served and always hears its rest notice, and `GET /owner`'s `room`. The owner as a
  sender stays his number on each road; a room is never read off it.

## The room's situation

- **A room's situation note is the receiver's own row** (`room_situations`, `docs/store.md`), one a room, replaced
  when it changes. It is the room's live state, not the agent's long-term memory: the brain's `situation` rows are
  rules for a situation, and what the agent remembers beyond the room is the brain's. The supervisor updates the
  note before its judgement (`docs/drafting.md`, 「The situation note」), and every round's draft carries it as a
  piece of the core.

## What is kept for a moment

- **A room's facts and the agency's list of agents are read once and kept for a short span,** rather than asked
  again for every room a request walks, every page of a room's line and every step of a round. The spans are the
  policy's (`kept.toml`): a minute for a room's facts, ten seconds for the agents, whose standing a wake turns on.
- **A room's facts are kept by road and room:** the answer of each chat service asked, a room the service does not
  hold kept as not held, a room the record holds alone read off the record each time and never asked of a service
  (「A room's chat providers」), and an answer that did not come kept not at all, so the next read asks again.
  Within the span every reader of the facts reads the kept answer: the room list and one room, the message page's
  `by`, the wake, the roads a line goes out on.
- **The agents are kept as the agency's one list,** awake or not, and every reader of an agent's registration reads
  it: the wake, the answerers of a room, the room list, a page asked by an agent, the labels a room is told. The
  room list reads it once for all its rooms.
- **What the receiver changes itself is let go at once:** an agent it registers or takes out lets the kept list
  go, and a room added or removed lets go what is kept of that room. A change made elsewhere is read once the span
  has passed.

