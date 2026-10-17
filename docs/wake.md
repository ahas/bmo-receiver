# When an agent wakes

How the receiver decides who wakes an agent of a room and the window a room keeps, and how a woken session takes
its rounds.

## Who reaches the session

- **One rule for every designated agent.** The receiver decides for every designated agent of a room by one
  rule, the chat agent and every companion alike, over every word filed in the room, a person's or an agent's,
  and never wakes an agent with its own words.
- **A user's own private chat needs no name and no open window,** the owner's and every other user's alike.
  - Which room is the owner's is each chat service's own answer (`POST /room/is-owner`), not a number the
    receiver reads.
  - Another user's is a room its chat service says is a one-to-one with the agent, with a person whose sender is
    linked to that user (`GET /senders/{sender}/user` on the web unit), so it holds only once the account model
    links them.
  - The turn tells the agent which of the three a room is (the owner's chat, another user's own chat or a group)
    since in a group most words are not for it.
- **A room a companion answers alone never reaches the chat agent at all.**
- **Three grounds, decided before any gate or session.** In any other chat the receiver itself decides, and a
  message reaches the agent's session on exactly three grounds: it calls the agent by name, it replies to a
  message of the agent's own, or that room's window is open.
- **Everything else is cut at the receiver** (no queue, no gate, no turn) whoever sent it, the person an
  active room rule names included. The one exception is a file from that person (`docs/room-files.md`,
  「Message files」), and a quiet instruction is a wake of its own that opens nothing for the room
  (`docs/bmo-window.md`, 「Quiet instruction」).
- **A conversation already running in the room is no ground:** it admits nothing the window would not.

## The names and the window

- **The names matched** are the agent's registered label and wake words, as the agency's registrations give
  them, matched without regard to case where they are written in Latin letters.
- **The window is the length the agent's row keeps,** three minutes by the policy, one length for every room,
  and each room holds its own for each agent that keeps one, stored in the receiver's database.
- **A call opens it the moment the receiver sees the call,** so what the caller adds before the answer needs no
  name either.
- **While it runs, new messages keep streaming into the same session.** The agent decides each round whether to
  answer, keep waiting while the author may still be typing (`<<WAIT>>`), or stay silent because the messages
  were not addressed to it (`<<SKIP>>`, group chats only: members talking to each other are left alone).
- **Two things restart the window and no others:** an answer that goes out, and a call. Each answer restarts it,
  so a follow-up needs no name; the conversation ends once the window has run out with nothing left to read.
- **The room merely speaking again does not restart it**: that would let a room that talks among itself hold the
  agent awake with no one ever calling it.
- **A round that ends in silence leaves the time it already had to run out** (a `<<SKIP>>`) because passing over
  what a room said is not answering it, and a window that grew every time the agent stayed quiet would hold a
  session open through a conversation it is not in.
- **A line put in the room from outside a session** (a `say` through `POST /sends`) opens one as an answer does:
  the room has been spoken to, so what anyone there says back needs no name to reach the session.
- **The open window is stored in the receiver's database,** and a turn of the room's own that ends while such a
  window is running leaves it alone rather than closing the room behind it. A restart in the middle of a
  conversation does not close it either.
- **Every write of it is told on the event stream** as a `window` event naming the room, so a screen standing
  on the room reads the new time the moment a call opens it or an answer restarts it.

## Waiting

- **The words a `<<WAIT>>` held stay unanswered and ride every later round** until a round answers or passes
  over them, whoever spoke to open that round: the prompt shows them again under a heading of their own, ahead
  of the new messages, so somebody else talking in the meantime never pushes them out of the turn.
- **The wait is kept per person**, apart from the room's window. Each person whose words a wait holds is waited
  on for the round policy's `wait_ms`, ten seconds, from the wait that took their latest words; a message of
  theirs reaching the room starts their ten seconds over, even while the room's queue still holds it, and
  anybody else's words leave it running.
- **Those words open a round of their own,** which reads the held words with them and may wait again.
- **Once any held person has stood quiet for their ten seconds** the turn opens on the held words, with whatever
  came since, and the agent must then answer with what it has; `<<SKIP>>` stays valid even then.
- **The room's window is unaffected by the wait** and still runs its three minutes from the call or the answer.

## The owner's own chat is always answered

- **A round that read the owner's own words in the owner's own chat never ends in silence,** and the code holds
  it to that rather than asking: every message of the owner's the round took is owed a reply whatever else the
  round was carrying.
- **The turn is told so in as many words**: the permission a quiet instruction brings covers the instruction
  alone, and the line a closed window puts in front of the turn withholds the silence too.
- **A `<<SKIP>>` that comes back anyway is not taken.** The session is asked once more for that same turn with
  the silence withdrawn, and what it writes then is the turn's answer; a session that stays silent through the
  second asking ends the round the way it stood, with both askings in the record.
- **The round is where this is held, because the round is what consumed the messages:** they are spliced out of
  the room's buffer as it opens and are nowhere else to be found once it has them, so a silence taken there is a
  question of the owner's that nothing will ever come back to.
- **A room is judged the other way round and keeps its silence,** because there the agent is one voice among
  several and staying out of an exchange is an answer.

## Replies

- **Answers go out as ordinary messages.** One prefixed with `<<REPLY>>` is sent as a reply to the message
  being answered instead, in a room whose chat service declares it can point an answer at a message; where it
  cannot, the words simply land in the room.
- **`<<REPLY:<message number>>` pins it to the message that number names,** however far back it is, for when the
  room has already moved past it: the agent's own judgement, never used in the owner's private chat.
- **The sentinel is stripped either way,** and every send to a chat removes any `<<REPLY>>`, `<<SKIP>>` or
  `<<WAIT>>` still standing in the text as its last step, leaving one alone only inside a code span or fence,
  where it is being quoted.
- **Nothing checks the number a session names:** the chat service refuses a message it cannot reach, the answer
  goes out pinned to nothing and reaches the room either way, and the room and the number it was pointing at go
  to the log, which is how a session aiming at another room's numbers or at messages that are gone is ever
  noticed.
- **A message that replies to another carries that message into the transcript** on its own line: the target's
  number, who sent it, when, and its words clipped to a couple of lines. A target the same prompt already shows
  is pointed at by number and sender alone, since its words are on the page anyway, and a target the archive
  does not hold is named as absent rather than left as a bare number.

## The room's queue and the round

- **A message that is to reach the session does not wake it by itself:** it joins its room's queue, the first
  message into an empty queue starts that room's five-second timer (the round policy's `queue_ms`), and when it
  fires everything gathered meanwhile is handed over at once: woken as one batch, or forwarded to the
  conversation already talking.
- **An empty queue has no timer,** so a quiet room costs nothing and a busy one costs one run per five seconds
  instead of one per message.
- **Quiet instructions, touches and the start-up backlog do not queue.**
- **A session answers one round at a time, and a round is over only once its answer has gone out.** Sending is
  not instant (the outgoing sanitiser runs inside it and may have to be waited out) so a round that counted
  itself finished before it had sent would let the next message start a second run of the same session, and the
  room would read two answers to one thing, the older one arriving last.
- **What arrives while a round is running waits in the buffer** and is read when that round hands over.

## A job's end

- **A job's end wakes the chat agent through bmo-job's instruction:** its ending, its report and its state
  facts are queued with `POST /instructions` as a fact for the agent in the room the work was filed from, or
  the owner's private chat once that room is gone, and open a round there like any instruction
  (`docs/voiced-notices.md`, 「A job's end」).
