# A room's files stay in that room

What a room posted, and what an agent keeps or made for it, is that room's. It never goes into another room as a
check, a test or an example, and a job never sends it at all. This document also holds how the message-file store
keeps what a message carried and whom a kept file wakes, and the tools that put a kept file back.

## The stores that keep files by room

- **Three stores under the data root keep a folder per room**, named by the room's chat id:
  - the message-file store (`collected/`), every file a message carried, the voice briefings included;
  - the picture store (`images/`), the pictures read off a room's messages;
  - the outboxes (`outbox/`), the files an agent made for a room and has not sent yet.
- **A file in one of those folders belongs to that room**, whoever's room it is: the owner's private chat is a
  room like any other, and its files do not go into a group room either.
- **The folders live as long as the room's registration.** Each store declares its folder as the room's, and the
  three go only with the administrator's removal of the room's registration, together with the room's rows of
  kept files (`docs/receiver.md`, 「Removing a room's registration」). No chat service's event, companion's removal
  or clean-up takes them.

## Message files

- **Voice notes, ordinary audio, videos and documents are kept from every sender** in allowed rooms, silently and
  within the configured attachment size cap, in the message-file store: the `collected_items` table and the files
  under `collected/<chatId>/` of the data root. Keeping a file launches nothing and notifies nobody.
- **The bytes come from the chat service:** with the hand-over (`POST /attachment`), or later as the message's
  `gathering` promised (`POST /gathered`), or fetched on asking (`POST /media`).
- **Every file is recorded in `collected_items`** (chat, sender, message id, kind, source, stored path, and the
  file's id among its message's files) and keeping the same file twice is a no-op.
- **Twice means the same file.** Every file a message carried crosses as a message file, with an id the chat
  service gives it: the same each time the same file crosses, and never shared by two files of one message
  (bmo-chat's `docs/contract.md`, 「Files」). A file is the message's file of its id: where its row stands and its
  bytes are still on the disk, a hand-over or a fetch of it again keeps nothing. Two files of one name in one
  message are two files, each with its row. A row that names no id is the message's file of its name, and the
  first file of that name handed over takes the row and records its id on it.
- **Where a file is filed:** under `collected/<chatId>/` as `<messageId>-<name>`, by its own name's last part, or
  as `<messageId>-<n>-<name>`, from 2 up, where another file of the message already stands under that name.
- **Failures:** a file past the cap, or one that could not be written, keeps its row with no stored path and why,
  and a later hand-over of the same file keeps it; a failure is logged but never takes the receiver down.
- **A video is kept as one**, whatever the chat service called it, as a row of kind `video`, by the video type it
  is handed over under. One with no file name of its own is filed as `video` with the extension its type implies,
  and a sound with none as `voice` with its own (`policy/roads.toml`, `gathering`).
- **What is kept always has a message to hang on.** A voice note, an audio file and a document are written into
  the archive as the short placeholder line the chat service makes of it, so the room reads back with the note in
  its place and the console has a row to put a listen button on.

### What the console reads of them

- **The console plays, opens and saves a message's files through the receiver** (`docs/boundary-api.md`, 「A
  message's files and picture」): it lists them in the order a page's files stand, each by its place among them,
  and asks for one's bytes by that place, never by a path.
- **Only bytes inside the room's own folder go out.** A stored path is followed through its links first and served
  only where it resolves inside the room's folder of the message-file store; a row with no stored path, or one
  that points anywhere else or at nothing left on the disk, is answered as nothing held.

### Who a file wakes

- **Most files wake nobody:** a voice note is not something a session can hear, and waking one for every note in a
  busy room would be paid for and answer nothing, so the next turn reads the line out of the archived block like
  any other.
- **Three kinds of sender are the exception** (the owner, the person an active room rule of that room names, and a
  caption that calls the agent by name) and their message is routed once the file is kept.
  - The owner's file and a calling caption are held to the same three grounds as text (`docs/wake.md`).
  - The rule's subject's file reaches the session without them, since that is the automatic handling a room rule
    exists for, and it opens no window by itself. The rule's subject's text is text like anyone's. The room's
    rules are the brain's (`POST /room-rules/read`).
  - Either way the session is never woken ahead of the bytes.
- **A file that woke nobody rides the request that follows it.** When somebody other than the owner and the rule's
  subject posts a file and then, within two minutes of it (the wake policy's `file_lead_seconds`, looked for among
  the room's latest `file_lead_messages`), says something of their own that wakes the agent (a call, a reply to
  the agent, or words inside its open window), the receiver hands the agent that person's files from those two
  minutes first, in their order, so the turn that reads the request reads the files beside it, each with its note.
  A file still being fetched when the request arrives goes in without a note, and a file a turn already read is
  not read again by the same conversation.
- **The note beside the message.** Whenever a session is shown such a message it carries a note beside it
  (`[attached file job.wav saved at <path>]`, or `was not stored` when keeping it failed) read from the message's
  `collected_items` rows, the way a photo carries its stored path.
- **What a room rule asks to be done with the file** is the session's to carry out with a tool: the rule is free
  text the owner can revoke, and a revoked rule stops the action with it.

## Tools that take a path

- **Which:** a picture named by a kept file (`bmo room photo`, and the send tool's `photo` entry).
- **A file in another room's folder is refused** before anything is sent, for every caller: the refusal names the
  room the file is kept for.
- **A file in the sending room's own folder goes**, and so does a file outside every store: a web copy a search
  made, a picture drawn for the send.
- **Links are followed first**, so a link made elsewhere that points into a room's folder is that room's file.

## Tools that take a message

- **Which:** `bmo room file`. It sends the file kept for one message, back into that message's room or on to the
  owner's private chat, and nowhere else.
- **The owner asking for a room's file is what it is for**, so a chat session's send on to the owner's chat
  stands; a job's does not (「A job's sends」).

### A file back

- **A file somebody posted, put back as the agent's own message,** through
  `bmo room file --chat <chatId> --message <messageId> [--to <ownerId>]`.
- **It is given a message and never a path:** it takes the file kept for that message from `collected_items`, and
  refuses, sending nothing, when nothing is kept, when the kept path does not resolve inside the message-file
  store's directory, or when the file is larger than the room's chat service takes.
- **It runs in the caller's own process,** since an upload is quick: the document sent as a reply to the original
  (or to the owner's chat with `--to`), the message archived under the agent's name with its reply, and a copy of
  the file kept under `collected/` for that message with the original name as its source, so the console stands
  it like a received file.

## A job's sends

- **A job sends no file any room's folder holds**, not even into that same room: a job's send is a check, and a
  check is made of material the job made itself: a short line it synthesised, a picture it drew, a file it wrote
  for the check. Every tool above refuses a job's send of a room's file, and the refusal says what to make
  instead.
- **What a job is:** the asker's `job` mark on `POST /sends`, which the job's own tools set.
- **Where the job's check goes** is the room the item names for trying a send, and nowhere else.
- **What no tool can see:** bytes copied out of a room's folder into a new file, or read out of the archive, are no
  longer a room's path. That is the rule every job reads in its instructions, and not a refusal: a job never takes
  a room's file, message or archived words as test material, whatever road it goes by.
