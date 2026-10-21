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
- **A round goes through the receiver's own parts, every agent's conversations alike:** the turn composed by the
  window is carried into the agent's session through the agency (`POST /sessions`, `GET /sessions/{sessionId}`,
  `POST /sessions/{sessionId}/turns`, and the turn's `turn.created` and `turn.ended` on the event stream); a
  companion's round is its own turn, sent on the road its room answers on; any other answer is held to the answer
  checks (`docs/answer-checks.md`) and sent on the road the turn answers on, then handed back to the receiver as
  filed. The round's session is noted, a failed round tells its room's health, and the failure orders are looked
  over on the conversations' own clock.
- **Words that come while the turn works are pushed into it**
  (`POST /sessions/{sessionId}/turns/{turnId}/inputs`) at the round's next look at the working turn: what the
  agency says was `delivered` joins the round, which answers it with the rest; what is `waiting` or `refused` goes
  back to the front of the room's buffer for the round after, and a `waiting` turn is offered nothing more
  (`docs/bmo-window.md`, `push`).
- **A round folds its session only past the fold share:** once its answer has gone out, the session is folded as
  work of its own where the context the agency reports it carrying (`carried` against `window`) has reached the
  resident turn policy's `fold_ratio`, and not otherwise.

## A companion's round

A companion's round is its own turn (the agency's `docs/companions.md`): the room's words and its quiet
instructions composed into one prompt, carried into the room's session, and the answer sent on the road the room
answers on under the companion's name. Its reset is the agency's; what the reset cuts off in the rooms is the
receiver's (the agency's `docs/companions.md`, 「What the reset cuts off」).

- **A turn begins as its round takes its words,** and a reset whose point stands at or after that moment
  overtakes it. The point is the agency's (`GET /agents/{agent}`), read as the round takes its words and
  again whenever the turn is to keep anything.
- **What the reset cut off reaches no turn:** words said at or before the point are dropped, a message's moment
  being counted to the millisecond, so one said in the point's own millisecond goes with those before it; and a quiet instruction
  handing back a run is dropped where that run began at or before the point. A run is read by its id from the
  agency; one that cannot be read keeps its instruction. A round left with nothing to answer runs no turn.
- **A turn the reset overtook keeps nothing:** where the agency answers that it stopped the turn, or where the
  point read after the turn ended stands at or after its beginning, no answer goes into the room, no judgement is
  written and no window is moved; the room is free for the next round.
- **An answer is kept under a hold the reset's cut-off waits on:** the point is read again under it, and the
  hold stands until the answer is sent, the room answered and its window moved. A cut-off begun meanwhile takes
  what waits only after; a turn that reads the point after it moved finds itself overtaken.
- **The answer is read as the companion wrote it:** an answer that is the skip sentinel alone sends nothing and
  the turn ends answered, its window moved; the reply sentinel is taken off and pins the answer to the message it
  names, the last one the turn read where it names none, or to none where the answer carries no sentinel.
- **A turn that failed ends its round, not the conversation:** its reason is told to the room's screens on the
  turn's line under the companion's name, the room counts as answered and its window moves, and words that came
  meanwhile follow as the next round. No health line and no failure order is raised for it.
- **A turn its engine ended with an error tells the room in one line** under the companion's own name: the fixed
  line `companion-engine-error`, its kind read from the error kind the failed turn's `turn.ended` carries
  (`docs/codes.md`, `EngineError`), sent on the road the room answers on. Any other failure says nothing in the
  room.
- **The cut-off itself** is published with the reset point alone, under the kind `companion-reset.created`,
  and is heard for as long as the service runs. Under the hold every answer is kept under, everything waiting
  for the companion's turns in every room is let go unread: the words its rooms' queues are gathering, the words
  its conversations hold or wait on, and its quiet instructions, whose rows are settled as delivered so no turn
  takes them again. A companion whose conversations do not stand in this process has nothing waiting.

## A companion's reset

The reset is the agency's; it tells each unit holding the companion's rows to take its own, under the same
`companion-reset.created` kind, naming the ranges it took and the rooms it took them in (the agency's
`docs/companions.md`, 「Starting it afresh」), once the cut-off has let go what waited for the companion's
turns. The two parts of the kind are told apart by their fields: a cut-off names the `point` alone, a reset
names its `ranges`. The receiver takes its own rows of the ranges the event names, in one write:

- **The conversation range** takes the whole record of each room the companion answers alone (its messages and
  the receiver's read mark of it) and, in every room, the companion's own quiet instructions and the windows its
  rooms hold for it.
- **The person range** takes the marks the companion's book of each room has read.
- **A room counts as answered alone** where the event names it so and the receiver's own record holds the
  companion as its one answerer, so a room another agent also answers keeps its record whatever the event says.
- **What stays, whichever range:** the rooms themselves and who answers them, their titles, their settings, the
  files their messages carried, and the lowest number each room's record has handed out, so a number whose
  message went is never handed out again.

## What the agency's events wake

Each event the receiver reads (`docs/boundary-api.md`, 「The agency」) is heard by the part its effect belongs
to, and those that tell the chat agent something wake it as an instruction in the owner's private chat, which
opens a round there like any instruction:

- **A voiced notice:** a stand-in answering or ended, and a worker fallen to a lesser model
  (`docs/voiced-notices.md`, 「Rest and fallback」).
- **An order:** a run past its clock, withdrawn where its ask closes first (`docs/voiced-notices.md`, 「A turn
  past its clock」).
- **No session at all:** the deployment resting, told in its fixed line, and a login's end, said as the agency
  words it; neither wakes anyone.

## A job's end

- **A job's end wakes the chat agent through bmo-job's instruction:** its ending, its report and its state
  facts are queued with `POST /instructions` as a fact for the agent in the room the work was filed from, or
  the owner's private chat once that room is gone, and open a round there like any instruction
  (`docs/voiced-notices.md`, 「A job's end」).
