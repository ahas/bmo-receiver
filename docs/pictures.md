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
- **A `--url` that is neither a web address nor a file under the data root is refused** before anything goes. A
  relative path is read against the folder that holds the data root, the data root itself and the working folder,
  in that order. A web address goes as it stands and the chat service fetches it; a file is handed over as a
  file on this machine's disk, read from it only as the send carries it (bmo-chat's `docs/protocol.md`, 「Files
  carried as parts」), so a chat service needs no reach into this machine's data root and the receiver never holds
  the file whole. A picture with a label drawn into it is read here to be drawn, and goes as the drawn bytes.

## A picture an agent sent

- **A picture an agent sent is kept like a picture it received.** The bytes handed over with the send are filed
  under the message the send is kept as, the first as `images/<chatId>/<messageId>.jpg` and an album's later
  pictures as `<messageId>-1.jpg` and on, and the row points at the first.
- **A picture sent as a file is kept as a picture too.** A file send's file of an image type is filed in the
  picture store under the message the file is kept as, so the console draws it as that message's picture; every
  other file of a send goes to the message-file store (`docs/room-files.md`, 「Message files」).
- **A web address is the chat service's to fetch:** nothing of it is read here, so it is not filed unless the
  picture is handed over with the send.
- **A picture the road said did not go is left out of the archive,** and keeping never fails a send: the picture
  is already in the room by the time it runs. An album keeps the pictures that went, in their order.
- **A row that holds an address and no picture** is repaired by `bmo archive photo-repair`, which reads the
  address out of each such row, redraws what it finds as JPEG laid on white and files it under the message. It
  prints what it would try and writes nothing without `--write`, and it leaves the text of a repaired row alone.

## Pictures received

- **Photos posted in an allowed room are kept**, as the chat service hands them over or fetches them, as
  `images/<chatId>/<messageId>.jpg` under the data root, a later picture of the same message as
  `<messageId>-1.jpg` and on. An image the chat service hands over as a document of an image type goes down the
  same path.
- **A picture the chat service did not carry with its message** is filed the same way when a pass fills the row
  later (`docs/room-files.md`, 「Files filled in later」).
- **A picture larger than the resize threshold** is downscaled to the maximum dimension and re-encoded as JPEG
  before it is stored, so a session always reads a file it can afford to open; a picture that is not a JPEG and has
  transparency is laid on white and stored as JPEG whatever its size.
- **A sticker or an animation that moves is kept as it came.** A picture the chat service hands over as a sticker
  or an animation (bmo-chat's `docs/contract.md`, 「Files」) whose bytes are an animated WebP or a GIF of more than
  one frame is filed byte for byte under the extension of its own type, `<messageId>.webp` or `<messageId>.gif`,
  and served with that type: a redraw keeps the first frame alone, so neither the threshold nor its transparency
  redraws it. A still one is kept as any picture is.
- **A message whose picture came as a sticker is marked a sticker:** its `kind` is `sticker` (`docs/api.md`,
  「Messages」), as a sticker the record sends is, so the console draws it as a sticker rather than a photo.
- **The message is archived like any other,** with the caption as its text or the picture placeholder when there
  is none, and the archive row keeps the stored path plus the id of the message it replies to.
- **A picture with no caption still reaches the session with its note.** The line stands its sender's words, empty
  where there were none, then the note naming the saved path (`[attached image saved at <path>]` where the room
  looks at pictures, the note that it does not look where it does not). A picture that could not be written says
  it was not stored rather than leaving a line with nothing in it; the turn reads the message after the file is
  kept, never the hand-over that came before it.
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
- **The viewer walks a room's pictures across the whole room** (`docs/api.md`, 「Messages」): the messages a person
  said that stored a picture, in the order the room's line stands them, asked for by a picture the reader holds,
  with how many stand before the stretch and how many the room holds, so the viewer can say where it stands among
  them.
- **A sticker is left out of that walk:** a message of kind `sticker` (「Pictures received」) is drawn as a sticker
  and never opened in the viewer, so the room's pictures, their pages and their count hold none, and stepping from
  one photo lands on the next photo.

## Profile pictures

- **Every room and every person the archive knows wears, in the console, the picture its chat service shows for
  it**: a room's, a person's and the owner's own account's, from whichever chat service carries it.
- **The receiver is the console's one source of them.** Every chat service's pictures reach the console through
  the receiver's picture routes alone; a chat service's own picture routes are the receiver's to read and never
  the console's, so a chat service never decides how the console learns a picture, and one more chat service
  adds no path to the console. A profile picture is the first of the elements every chat service has in common,
  keyed by its kind and its subject; a further common element takes the same shape.
- **The receiver takes each chat service's pictures from that service,** through bmo-chat's avatar routes
  (`GET /avatars`, `GET /rooms/{roomId}/avatar`, `GET /people/{personId}/avatar`), on every road the
  configuration lists. This pull is the one road a picture takes. bmo-chat's contract already has every provider
  keep its own pictures, serve them on those routes and tell the receiving side which subjects changed, so the
  receiver reads them on that word and no provider carries a picture to the receiver by a road of its own or is
  changed for it. A picture is taken three ways:
  - **on word that pictures changed** (`POST /chat/avatar-changes`, its `roomIds` and `personIds`): each subject
    the word names is asked of the road that sent it before the word is answered. A picture the road serves is
    kept, one it answers `not-found` for is let go, and a road that does not answer leaves the kept one standing;
  - **as the service starts**, each road's list is walked whole, and every picture on it taken later than the one
    kept is asked for, so a word missed while the receiver was down is caught up;
  - **then every room and person whose words came on a road and that holds no picture from a chat service** is
    asked of that road by its subject, so a picture a road serves but leaves off its own list still reaches the
    console.
- **A picture is kept under the number the road gave its subject,** the number the room's and the person's lines
  are kept under, the owner's own account being the person his words on that road stand under. It is laid on white
  and filed as a JPEG under the data root as `avatars/room/<roomId>.jpg` or `avatars/person/<personId>.jpg`; one
  that cannot be read as a picture is said in the log and not kept. Taking a picture never stops a room's words.
- **The console asks one number for one person:** a message's face by the number its sender is archived under,
  and a member sheet's row by the number the members route answers. Both are the number the road gives the person
  now, so a picture is found under it from either surface. A sender kept under a number the road gave before is
  matched to the room's member of the same nickname as the service starts (`docs/messages.md`, 「A sender matched
  to the room's members」), so the message moves to the person's number and a kept picture never moves for it; a
  sender left unmatched keeps the first letter of its name.
- **The `avatars` table** keeps each subject's stored path and when its picture was taken: the `takenAt` the
  road's list gave, else the moment the receiver fetched it. Every room and person the archive numbers has its
  row there once a road serves its picture, the supervisors added on the console and their rooms among them.
- **The receiver serves the kept pictures** on the shape every chat service's protocol gives them: `GET /avatars`
  is a page of the pictures kept, each `kind`, `id` and `takenAt`, oldest taken first; `GET /rooms/{roomId}/avatar`
  and `GET /people/{personId}/avatar` are the picture's bytes, typed by `Content-Type`, 404 `not-found` where none
  is kept or its file is gone. Only a file inside the data root's `avatars/` folder goes out. The console reads
  these routes for every picture it shows, and carries `takenAt` in the address, so a browser reuses the picture
  it holds until the picture itself changes. Every picture taken, let go, set or unset tells the console with
  `avatars.changed`. Wherever there is no picture, a name's first letter stands in for one.
- **A picture is set from the console too,** on one route shape for three kinds of subject:
  `PUT /avatars/{kind}/{subject}`, the body a form with one `picture`, where `kind` is `room` (a room's number),
  `person` (a person's number, the owner's own among them) or `agent` (an agent's registration name). The picture
  is laid on white, filed as a JPEG under the data root's `avatars/set/` and kept in `avatar_sets`, beside the
  picture a chat service hands over, never in its place.
- **An agent's picture is kept under its name,** in `agent_avatar_sets`, its file under `avatars/set/agent/`
  named by the name's bytes in hexadecimal, whether or not the agent stands under any number yet, so a new agent
  wears its picture from its first line. It is worn at once under every number the agent stands under in the
  rooms (each a road or the record gave it, and each its own words are archived under), and later under each
  number the agent takes: the moment the receiver first files the agent's words under a number (a send it
  archives, a line under the room's own number, a speaker set on a kept message) or gives the agent a number in a
  room added for it. Each number then holds a `person` row in `avatar_sets` naming the agent's file, set at that
  moment, so the console is told with `avatars.changed` and the picture stands over one a chat service handed over
  before. A number whose own picture was set from the console after the agent's keeps its own. Setting the
  agent's picture sets it anew under every number that wears it or that the agent stands under; letting it go lets
  the console-set picture go under each of those numbers and drops the agent's row and file. Letting a person's picture go never removes the file an agent's
  picture holds.
- **The newer one stands:** a room's and a person's picture is the newer of the one its chat service handed over
  and the one set from the console, on every route that serves one (`GET /rooms/{roomId}/avatar`,
  `GET /people/{personId}/avatar`) and in `GET /avatars`, which lists each subject once, `takenAt` the moment its
  standing picture was taken. `DELETE /avatars/{kind}/{subject}` lets the console-set picture go, so the chat
  service's stands where one is kept.
- **Who may set one** is read off the console relay's `Owner-Key`, the signed-in login's user id, and
  `Owner-Role`, `administrator` or `member`: an agent's and a room's picture are the administrator's to set or
  let go; a person's is the administrator's, and any user's whose own senders, as the web unit's account model
  links them to the user, hold that person. Anyone else is refused 403 `picture-forbidden`.

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
  asks something, or plainly invites a reaction, and otherwise stays silent with a skip naming on one line what the
  picture was (`docs/drafting.md`, 「What it is」).
- **An older picture is fetched on demand.** With the switch on, a new message in a turn with tools that replies to
  a message whose image the receiver never stored or no longer holds has that message's media fetched through its
  chat service into the same directory, so old photos stay answerable and a second question about the same photo
  costs nothing.
- **When the fetch fails,** the prompt says so instead of showing nothing. In a turn with tools off the file cannot
  be opened and the agent says so instead of pretending to have looked.
- **A failed download is logged,** the message is still archived and answered without the image, and the receiver
  keeps running.
