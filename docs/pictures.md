# Pictures

Pictures an agent sends, pictures it receives and keeps, the profile pictures the console shows, and whether an
agent opens a picture at all. How a chat service fetches, uploads and re-encodes a picture is that service's; the
receiver keeps the pictures and decides what a session is shown.

## Photos

- **An agent can put pictures in a chat, not just text,** with the send tool's `photo` entry or
  `bmo room photo [--chat <chatId>] [--via <road>] --url <imageUrl> [--label <word>] [--url ... [--label ...]]
  [--caption <text>]` (the owner's chat when `--chat` is absent).
- **One picture goes as a photo and several as one album**, as many to an album as the room's chat service
  declares, the caption on the first picture, on the road `via` names where that road reaches the room. Every
  picture is archived like any message the agent sent. The pictures of an album are never sent one by one.
- **A `--label` after a `--url` is drawn into that picture** on a dark band along its bottom edge, so pictures of
  different things are told apart by the pictures alone.
- **A picture with transparency goes laid on white,** since the diagrams a search keeps are black lines on a
  transparent ground, and a labelled picture is laid on white before its band is drawn.
- **A chat service may refuse a picture;** each picture that did not go is printed with why, and a call that sent
  none fails on a refusal naming each.
- **If nothing can go the caption and the web addresses go as plain text** rather than losing the answer. A path
  on this machine is never put in that text, since it is nothing to the room, and where no web address is left
  nothing is sent at all.
- **A `--url` that is neither a web address nor a file under the data root is refused** before anything goes.

## A picture an agent sent

- **A picture an agent sent is kept like a picture it received.** Once a picture is in the room, the receiver
  fetches it back through the chat service (`POST /media`) and stores it as `images/<chatId>/<messageId>.jpg`, one
  file and one archive row per picture, and the row points at it.
- **An album of six leaves six rows,** each naming the album's first picture as its album, which the console draws
  as one group, and the row's text is the caption or the picture placeholder exactly as a received picture's is.
- **A picture that could not be kept leaves the address in the row's text instead,** since that is then the only
  record of what the picture was, and keeping never fails a send: the picture is already in the room by the time
  it runs.
- **A row that holds an address and no picture** is repaired by `bmo archive photo-repair`, which reads the
  address out of each such row, redraws what it finds as JPEG laid on white and files it under the message. It
  prints what it would try and writes nothing without `--write`, and it leaves the text of a repaired row alone.

## Pictures received

- **Photos posted in an allowed room are kept**, as the chat service hands them over or fetches them, as
  `images/<chatId>/<messageId>.jpg` under the data root. An image the chat service hands over as a document of an
  image type goes down the same path.
- **A picture larger than the resize threshold** is downscaled to the maximum dimension and re-encoded as JPEG
  before it is stored, so a session always reads a file it can afford to open; a picture that is not a JPEG and has
  transparency is laid on white and stored as JPEG whatever its size.
- **The message is archived like any other,** with the caption as its text or the picture placeholder when there
  is none, and the archive row keeps the stored path plus the id of the message it replies to.
- **Several photos sent at once reach the agent as one message.** The archive keeps a row for each picture of an
  album, each naming the album's first message, and every list of a room's messages a session reads (the new
  messages, the archived stretch, the messages waited on, a push mid-turn, the recent lines the gate reads) stands
  an album as one line: under its first message's number, its words once, a note for each picture and a note
  naming the other messages it went out with, so an answer can still be pinned to any of them.
- **Stored pictures are kept for good:** nothing deletes them but the room's removal.

## The console's picture viewer

- **A message's picture is served by its message** (`docs/boundary-api.md`, 「A message's files and picture」),
  and only from inside the room's own folder of the picture store, its stored path followed through its links
  first.
- **The viewer walks a room's pictures across the whole room** (「A room's pictures」): the messages a person said
  that stored a picture, in the order the room's line stands them, asked for by a picture the reader holds, with
  how many stand before the stretch and how many the room holds, so the viewer can say where it stands among them.

## Profile pictures

- **Every room and every person the archive knows wears, in the console, the picture its chat service shows for
  it.** The chat service that can see them hands them over, and tells the receiver with `POST /event`
  `avatars-updated`; the receiver keeps them under the data root as `avatars/room/<chatId>.jpg` or
  `avatars/person/<userId>.jpg`.
- **The `avatars` table** keeps each subject's photo id, its stored path and when it was last taken; the console
  reads it and carries the time in the address, so a browser reuses the picture it holds until the picture itself
  changes. Wherever there is no picture, a name's first letter stands in for one.

## Whether an agent looks

- **Looking is a switch of its own, off by default, and it belongs to the room.** Opening a picture costs a full
  read of the file in the session, and the pictures are not spread evenly: one room may carry a hundred in a week
  where the next carries a handful.
- **A room says its own word and keeps it whatever the rest do;** a room that has said nothing follows the
  default.
- **Where it is switched:** the owner switches a room from inside it (`photos:on`, `photos:off` or the words of
  the policy, the way the operating notices are switched) and from the shell for any room:
  `bmo room photos look on|off --chat <id>`, `look default --chat <id>` to take a room's word off again,
  `look on|off` with no room for the default, and `status` for what every room is doing and why.
- **Where it is kept:** a room's word is a row in the receiver's `photo_chats` table and the default is the
  `photos_look` setting; every run reads them as it builds its prompt.
- **While it is off,** a message with a picture reaches the session carrying only the note that an image is
  attached and that it does not look at images (no path, no invitation to open anything) and an older picture is
  never fetched either.
- **While it is on,** the message's line in the prompt names the absolute path of the image file (and, for a reply,
  the path of the replied-to message's image as well) so the agent opens the picture, and the same note tells it to
  decide afterwards whether the picture calls for an answer at all: it answers when the picture is addressed to it,
  asks something, or plainly invites a reaction, and otherwise stays silent with `<<SKIP>>`.
- **An older picture is fetched on demand.** With the switch on, a new message in a turn with tools that replies to
  a message whose image the receiver never stored or no longer holds has that message's media fetched through its
  chat service into the same directory, so old photos stay answerable and a second question about the same photo
  costs nothing.
- **When the fetch fails,** the prompt says so instead of showing nothing. In a turn with tools off the file cannot
  be opened and the agent says so instead of pretending to have looked.
- **A failed download is logged,** the message is still archived and answered without the image, and the receiver
  keeps running.
