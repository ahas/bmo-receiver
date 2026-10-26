# When an agent wakes

How the receiver decides who wakes an agent of a room and the window a room keeps, and how a woken session takes
its rounds.

## Who reaches the session

- **One rule for every designated agent.** The receiver decides for every designated agent of a room by one
  rule, the chat agent and every other agent alike, over every word filed in the room, a person's or an agent's,
  and never wakes an agent with its own words.
- **A user's own private chat needs no name and no open window,** the owner's and every other user's alike.
  - Which room is the owner's is each chat service's own answer (`POST /room/is-owner`), not a number the
    receiver reads.
  - Another user's is a room its chat service says is a one-to-one with the agent, with a person whose sender is
    linked to that user (`GET /senders/{sender}/user` on the web unit), so it holds only once the account model
    links them.
  - The turn tells the agent which of the three a room is (the owner's chat, another user's own chat or a group)
    since in a group most words are not for it.
- **A room that names only other agents never reaches the chat agent at all.**
- **A room added `record-only` wakes no agent at all** (`docs/receiver.md`, 「Adding a room」): its messages are
  kept and carried to its providers, and nothing past that is decided for them. The owner's answer to a job's
  open question is still handed to bmo-job there, since that wakes no agent.
- **A read-only room wakes no agent at all** (`docs/receiver.md`, 「A read-only room」): its messages are kept
  and stand in the archive, and no ground below is weighed for them, a call by name, a reply to an agent's words,
  an open window and a picture alike. The owner's answer to a job's open question is still handed to bmo-job,
  since that wakes no agent.
- **Three grounds, decided before any gate or session.** In any other chat the receiver itself decides, and a
  message reaches the agent's session on exactly three grounds: it calls the agent by name, it replies to a
  message of the agent's own, or that room's window is open.
- **A file is held to the same grounds as words,** read off its caption (`docs/room-files.md`, 「Who a file
  wakes」): its caption calls the agent, it replies to a message of the agent's own, the room's window is open,
  or it stands in a private chat that needs no name, the owner's file in the owner's own chat among them. A
  picture with no caption in a group whose window is closed wakes nobody, whoever sent it.
- **Everything else is cut at the receiver** (no queue, no gate, no turn) whoever sent it, the person an
  active room rule names included, files and words alike: a room rule is no ground by itself. What a room rule
  asks done with that person's pictures is the gate's situation rule, applied within a turn the grounds above
  opened. A quiet instruction is a wake of its own that opens nothing for the room (`docs/bmo-window.md`,
  「Quiet instruction」).
- **A conversation already running in the room is no ground:** it admits nothing the window would not.

## The names and the window

- **The names matched** are the agent's registered label and wake words, as the agency's registrations give
  them, matched without regard to case where they are written in Latin letters.
- **The window is the length the agent's row keeps,** and the policy's three minutes wherever the row names none,
  one length for every room; each room holds its own for each agent that keeps one, stored in the receiver's
  database.
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
- **Its close is told on the stream too,** as a `window-closed` event naming the room, the agent and `closedAt`,
  the moment it closed, so a screen stops its countdown and a unit reading the stream needs no asking again. A
  window closes by its time, when the end last written for it passes, and early, when the conversation that
  opened it closes it before then. A window an opening restarts before its end has not closed: the opening moves
  its end, and only the end that stands closes it, once. The receiver watches every end it writes, and as it
  starts every end its database holds still to come.

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
- **A `<<SKIP>>` that comes back anyway is not taken.** Where the draft process ends in a skip, the session is
  asked once more with the silence withdrawn, the process's `owed` item (`docs/drafting.md`), and what it writes
  then is the message; a session that stays silent through that asking ends the round the way it stood, with
  every turn of it in the round log.
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
  not instant (the road's send may have to be waited out) so a round that counted
  itself finished before it had sent would let the next message start a second run of the same session, and the
  room would read two answers to one thing, the older one arriving last.
- **What arrives while a round is running waits in the buffer** and is read when that round hands over.
- **Words the agent was not named in are judged first, by the agent itself.** In a room that is not the owner's
  private chat, a round whose every message reached the session on its open window alone — none of them names
  the agent (「The names and the window」), replies to its words or stands in a private chat — opens with the
  agent's own judgement of whether to answer: one light turn of the same session carrying the room's recent words
  in the room shape and the words being judged, and the round policy's question (`chat-round.toml`,
  `[supervisor]`) whether they are said to it or want its answer. It carries no reference and no index, and
  nothing of it passes any gate. `<<SKIP>>` ends the round there: nothing is sent, the words are
  read and the window keeps the time it had, as a skipped round leaves it. Any other answer opens the ordinary
  round, its reference and its draft process. A round with a named call, a reply, words of a
  private chat or the owner's chat skips this step. The judgement is a row of the round log of its own kind,
  `supervisor` (`docs/drafting.md`, 「The round log」).
- **A round goes through the receiver's own parts, every agent's conversations alike:** the turn composed by the
  window is carried into the agent's session through the agency (`POST /sessions`, `GET /sessions/{sessionId}`,
  `POST /sessions/{sessionId}/turns`, and the turn's `turn.created` and `turn.ended` on the event stream); every
  agent's turn, the chat agent's and any other's alike, opens the same way and carries the first question of the
  draft process, which makes the message (`docs/drafting.md`): the questions asked of the same
  session, the notes they establish first and the message last, the items and the checks on the final text
  (`docs/answer-checks.md`); the message is sent on the road the turn
  answers on, then handed back to the receiver as filed. The round's session is noted, a failed round tells its room's health, and the failure orders are looked
  over on the conversations' own clock.
- **A room's words never go into a working turn.** Words that come into the room while a round runs, from its
  own turn to the process's end, wait in the room's buffer and reach the agent only at a question boundary of the
  draft process: under the next question, or in the one closing check before the message is sent, where the
  agent marks each as answered with the message, needing no answer or answered in a round of its own
  (`docs/drafting.md`, 「What came in meanwhile」). A message shown inside a round opens no round afterwards
  unless the agent marked it separate; only words that came after the round showed its last block open the next.
- **What is no room message is pushed into the working turn**
  (`POST /sessions/{sessionId}/turns/{turnId}/inputs`) at the round's next look at it: a quiet instruction, such
  as one the owner types on the console. What the agency says was `delivered` joins the round, which answers it
  with the rest; what is `waiting` or `refused` goes back to wait for the round after, and a `waiting` turn is
  offered nothing more (`docs/bmo-window.md`, `push`).
- **A round folds its session only past the fold share:** once its answer has gone out, the session is folded as
  work of its own where the context the agency reports it carrying after the round's last turn (`carried`
  against `window`) has reached the
  session's fold ratio, and not otherwise. The ratio is the agency's, read with the session's state
  (`foldRatio`, the value its fold-ratio table holds for the session's provider and tier); where the state
  names none, the resident turn policy's `fold_ratio` stands in.

## An agent's reset

An agent's reset is the agency's (the agency's `docs/companions.md`, 「Starting it afresh」): it moves the
agent's reset point, publishes the cut-off, and then tells each unit holding the agent's rows to take its own.
Both go under the kind `companion-reset.created`, which is the agency's name for it, and are told apart by their
fields: a cut-off names the `point` alone, a reset names its `ranges`.

- **The cut-off** is heard for as long as the service runs. Everything waiting for the agent's turns in every
  room is let go unread: the words its rooms' queues are gathering, the words its conversations hold or wait on,
  and its quiet instructions, whose rows are settled as delivered so no turn takes them again. An agent whose
  conversations do not stand in this process has nothing waiting. A turn under way is the agency's to stop, and
  a stopped round ends as any stopped round does.

- **The reset** names the ranges it took, the rooms it took them in and the runs its own log named, once the
  cut-off has let go what waited for the agent's turns. The receiver takes its own rows of the ranges the event
  names, in one write:

  - **The conversation range** takes the whole record of each room the agent answers alone (its messages and
    the receiver's read mark of it) and, in every room, the agent's own quiet instructions and the windows its
    rooms hold for it.
  - **The run records go with them:** every record of a room the agent answers alone, read by the run name
    the room's chat runs open with, and every record written under one of the agent's own runs, whose names
    the event carries from the agency's log. A chat run's name says its room but not the agent that ran it, so a
    shared room keeps the records of the runs its other agents ran and loses only this agent's.
  - **The person range** takes the marks the agent's book of each room has read.
  - **A room counts as answered alone** where the event names it so and the receiver's own record holds the
    agent as its one answerer, so a room another agent also answers keeps its record whatever the event says.
- **What stays, whichever range:** the rooms themselves and who answers them, their titles, their settings, the
  files their messages carried, and the lowest number each room's record has handed out, so a number whose
  message went is never handed out again.

## Quiet instructions

A quiet instruction reaches an agent's session as `docs/bmo-window.md`, 「Quiet instruction」, states.

- **No instruction wakes a session past its cache lifetime.** Where the agent's one session last answered longer
  ago than the lifetime its provider declares (`providers.toml`, `cache_minutes`, else `keepalive_idle_minutes`), a row that would open a
  turn of its own stays pending — a briefing past its ride, a fact, an order alike — and waits for a round a room's
  own words open, which carries it; that round replaces the session through the handoff, as an expired session
  always is (`docs/unified-bmo.md`, 「Replacing the session」). Waking it would pay its whole prefix again for
  nothing. One is
written by the wake itself: the deployment briefing, what went live since the owner was last briefed.

- **A deployment is no session event.** The agency relays a turn into the session that stands, opening one only
  where none does, so a deployment of any unit, the agency's included, neither reopens nor relaunches the
  agent's session, and no instruction says it did. The session is replaced only as `docs/unified-bmo.md`,
  「Replacing the session」, states.
- **Each round's start looks for deployments since the last briefing:** every unit whose deployment record
  (`docs/boundary-api.md`, 「The deployments」) went live after the moment the agent's session was last briefed,
  oldest first. The moment is kept under the session's setting (`docs/store.md`): the agent's one session across
  its rooms where its registration says so, else the room's own. A session never briefed reads as briefed the
  restart policy's fresh while ago (`fresh_minutes`). Where none went live since, nothing is written.
- **One instruction names them,** in the restart policy's words (`restart.toml`, `briefing`): what went live
  since the last briefing, one clause a unit naming its unit, version, deploy message or subject, commit and the
  moment it went live, as the deployment record holds them. Its row is of the sort `briefing` and the kind
  `deployments-briefed`, which a newer one replaces while it waits, and writing it moves the session's moment to
  now.
- **It asks for the owner's briefing.** Its intent and follow-up tell the session to tell the owner, in his
  private chat, what went live: one short paragraph in his language and in the agent's own words, with no talk of
  tools and nothing claimed of a release beyond its message, subject and commit. The instruction withholds the
  silence: a turn that carries only it writes the brief. A turn the owner's own words opened answers him first
  and gives the brief after it.
- **It is the owner's own** (`docs/bmo-window.md`, 「Rows that ride」): the row is queued for his private chat, and
  only a round of that chat carries it. It is written before that round asks for its riders, so the round's own
  turn carries it. Unridden past the restart policy's ride while, it opens a turn of the owner's chat.
- **No other room hears of it.** The briefing is said in the owner's private chat alone; the room whose round
  found the deployments is told nothing of them.

### The tool host's changes

- **A tool placed on the tool host or taken from it is told to each chat agent it concerns,** as the host
  publishes it (`docs/boundary-api.md`, 「The tool host」): one instruction a chat agent whose standing tool session
  holds one of the event's namespaces, nothing to an agent whose session holds none of them.
- **The instruction is a fact** in the tool host policy's words (`policy/tool-host.toml`): which tool was placed,
  updated (a placing that replaced another version of it) or removed, each entry by its full name, how it is
  reached (`call` or `act`) and the first sentence of its description; that a tool is read with
  `bmo use <tool> --describe` before it is called; and that nothing needs saying in any room.
- **It rides and wakes nobody** (`docs/bmo-window.md`, 「Rows that ride」): it is queued in the owner's private chat
  under the kind `tools-changed:<agent>:<tool>`, so a later change of the same tool replaces the agent's one not
  yet handed over, and rides for good in a round of any room the agent's session answers.

### A road's change

- **The agency's `road.changed` is told to the owner once,** through the chat agent's session in his private
  chat, as the deployment briefing is (the agency's `docs/api.md`, 「Events」): one instruction an event, in the
  owner facts' words (`policy/owner-facts.toml`, `[road]`), under the kind `road-changed:<key>`, so a newer fact of the same
  work key replaces one still waiting and none is said twice.
- **What it carries:** for `road-moved`, the work key, the road it moved from and the road it moved to, each its
  provider, model and account, and every provider's reason it passed; for `road-returned`, the road it is back on
  and the one it left; for `road-closed`, every provider's reason, the command the agency names for reopening a
  road, set on a line of its own exactly as the agency wrote it, and the moment the first road reopens by itself.
- **It asks for one sentence to the owner,** in his language and in the agent's own words: what moved from what to
  what and why, or what is closed, what the owner does and when it reopens. A command is passed on character for
  character in a code span, never reworded or translated, since a command reworded does not run. The instruction
  withholds the silence.
- **It is the owner's own and opens his turn:** a road that moved or closed is a failure the owner is not to hear
  late, so it never rides. No other room hears of it.

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
- **A road's change:** `road.changed`, told to the owner as 「Quiet instructions」 above states.

## A job's end

- **A job's end wakes the chat agent through bmo-job's instruction:** its ending, its report and its state
  facts are queued with `POST /instructions` as a fact for the agent in the room the work was filed from, or
  the owner's private chat once that room is gone, and open a round there like any instruction
  (`docs/voiced-notices.md`, 「A job's end」).
