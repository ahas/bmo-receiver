# When an agent wakes

How the receiver decides who wakes an agent of a room and the window a room keeps, and how a woken session takes
its rounds.

## Who reaches the session

- **A room's designated agents are the supervisors it names,** its `supervisors` (`docs/api.md`,
  「Supervisors」). A room that names none is answered by the supervisor whose registration names, as its
  `ownerAccount`, the user the account model links the person's sender to, where the room's road says it is a
  one-to-one with a person; the first by name answers where more than one supervisor names that user. The
  owner's private chat and every room on the record alone naming none are answered by the default answerer, the
  supervisor the receiver's setting names. A chat service's group room naming none is answered by the supervisors
  whose accounts stand in it (below), and by nobody where none does: the default answerer would answer it in the
  owner's own account, and a room is kept from that by naming nobody. A room naming none is answered alone in the owner's private chat
  and in a user's own private chat.
- **A supervisor whose account stands in a chat service's room answers it, named or not.** A room whose
  member list on its chat service holds one of a supervisor's accounts there (the agency's `docs/supervisors.md`,
  「Its accounts on chat services」), matched by the service's key and the member's number, has that supervisor
  among its designated agents beside the ones it names: inviting the account is calling the supervisor in. Such a
  supervisor's words go out through its account; a named supervisor whose account stands nowhere in the room
  answers in the owner's own account, behind its name tag (`docs/receiver.md`, 「A room's chat providers」). The member list is asked of the service as
  a room's words are weighed and kept for a moment, as long as `kept.toml` keeps a service's facts of a room; a service that cannot list
  its members adds no supervisor.
- **In a one-to-one, a supervisor's own account is the room's alone.** A person's private chat with a supervisor's
  own account holds that account, and its supervisor answers the room alone, as a user's own private chat is answered. The default answerer's account is the service's own, which
  every person's private chat with the service holds, so it calls nobody into a one-to-one: the person's own
  supervisor, or the default answerer, answers that one as ever.
- **A supervisor standing in a room may be turned off there,** named or by its account, and on again
  (`PUT /rooms/{roomId}/supervisors/{supervisorName}/answering`): it stays in the room, its account a member still,
  and answers nothing there while it is off. A room whose every supervisor is off is answered by nobody, never by
  the default answerer in their place. The supervisors that answer a room are those standing in it that are on.
- **A user's own private chat whose person owns no awake supervisor falls back to the default answerer,** and
  the journal says once for the room that the fallback was taken.
- **A group room is answered by its leader's BMO where it names one.** The console makes a group room naming the
  supervisor of the person who made it, so that supervisor alone answers the room and no other member's does; a
  group room naming none carries none, and the default answerer answers it. The journal says once for a group
  room which BMO answers it. A member's own BMO reads the room without answering it (`docs/api.md`,
  「Messages」).
- **A member calls their own supervisor into a group room by name: membership is the call right.** A person whose
  sender the account model links to a user, a messenger's identity or an account's record identity alike, and who
  is a member of the room wakes the awake supervisor that user owns by calling it by its name, though the room does
  not name it: that call reaches the supervisor's session as a call does, and opens its window in the room. A
  member is a person whose words came over a chat service carrying the room, which holds them among the room's
  people, or an account the room holds among its console members (`docs/receiver.md`, 「Console rooms and
  accounts」); no grant is asked of either, and nobody who is neither calls it. Without membership, or without the
  name, the person's own supervisor only reads the room.
- **In a room added `avatar` the supervisors the room names are the whole of who its words may wake.** The
  member's call of their own supervisor reaches no supervisor outside that list; membership opens no door past
  it. A room added `avatar` naming none is the setting left unset and wakes by the fallbacks above. Every other
  room keeps membership as the call right.
- **One rule for every designated agent.** The receiver decides for every designated agent of a room by one
  rule, every supervisor alike, over every word filed in the room, a person's or an agent's, and never wakes an
  agent with its own words.
- **A user's own private chat needs no name and no open window,** the owner's and every other user's alike.
  - The owner's private chat is his private room alone, the room numbered as his own number or the one the
    configuration's `owner_room` names (`docs/receiver.md`, 「Identity, and reaching the owner」); a chat
    service's `kind` of `owner` is read for no other room.
  - A user's own private chat is a room its chat service says is a one-to-one with the agent (its `privateTo`),
    with a person whose sender is linked to that user (`GET /senders/{sender}/user` on the web unit), so it holds
    only once the account model links them. The owner's own one-to-one on a messenger is such a chat, his sender
    being linked to his user like anyone's.
  - A BMO's private room in the console is such a chat too: the record room written as the supervisor was added,
    one-to-one with the supervisor's owner account by that account's record identity (`docs/receiver.md`,
    「Console rooms and accounts」), so that account's words wake it with no name and no open window.
  - A room its chat service answers as a group with no `privateTo` is no one's private chat, whoever is in it, a
    room only the owner's own account is in among them: words there reach the session on the three grounds below
    alone.
  - The turn tells the agent which of the three a room is (the owner's chat, another user's own chat or a group)
    since in a group most words are not for it.
- **A private chat needs no name for a person's words alone.** An agent's words filed there, another agent's
  answer in the owner's private chat among them, wake an agent on the three grounds below and not because the chat
  is private, so two agents answering one private chat do not wake each other on every answer.
- **A room that names supervisors never reaches the default answerer** unless it names it among them.
- **A supervisor of an account whose machine is gone is woken nowhere.** A supervisor whose row names an owner
  account answers only while that account stands: bound to no machine, or bound to one the hub has attached, as
  the web unit answers (`docs/boundary-api.md`, 「The web unit」). While the machine is gone, words that would wake
  it are kept and wake nothing, and a quiet instruction for it stays queued, delivered once the machine is attached
  again. A web unit that cannot be asked leaves the supervisor standing, since that says nothing of the machine.
- **A supervisor of an account that left is woken nowhere again,** as the same answer of the web unit says
  (`left`): words that would wake it are kept and wake nothing, and a quiet instruction for it is let go.
- **A room added `record-only` wakes no agent at all** (`docs/receiver.md`, 「Adding a room」): its messages are
  kept and carried to its providers, and nothing past that is decided for them. The owner's answer to a job's
  open question is still handed to bmo-job there, since that wakes no agent.
- **A read-only room wakes no agent at all** (`docs/receiver.md`, 「A read-only room」): its messages are kept
  and stand in the archive, and no ground below is weighed for them, a call by name, a reply to an agent's words,
  an open window and a picture alike. The owner's answer to a job's open question is still handed to bmo-job,
  since that wakes no agent.
- **Only a room the receiver listens to wakes an agent.** It listens to the rooms the console's room list
  holds: the owner's private room, which needs nothing more, a room added in a mode, a room naming
  supervisors, a pinned room, and a room on the listen list (`docs/store.md`, `allowed_chats`). Every other
  room's messages are kept and stand in the archive, and nothing past that is decided for them: no ground below
  is weighed, no session starts, and the start-up backlog passes the room by. The owner's answer to a job's
  open question in his private chat is still handed to bmo-job.
- **Three grounds, decided before any gate or session.** In any other chat the receiver itself decides, and a
  message reaches the agent's session on exactly three grounds: it calls the agent by name, it replies to a
  message of the agent's own, or that room's window is open.
- **A message of the agent's own is one the room's record holds as its words:** kept as a send of the agent's,
  its answers and every message its tools put in the room alike (`docs/messages.md`, 「What an agent
  can send back」), or filed under the number its words stand under. A line sent on a messenger is kept under the
  number the messenger gave it, which is the number a reply there names (`docs/unified-bmo.md`, 「The cases」), and
  a reply a messenger hands over to its copy of a message the record numbered answers that message
  (`docs/receiver.md`, 「A room's chat providers」). So a reply on a messenger to an agent's line, a voice note
  among them, is a reply to that agent's words and wakes it, in a room with several answerers too.
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
- **A person's message the room keeps and no agent takes says why in the journal, once for that message.** The
  line names the room and the message and the one stop that held it before every agent: words that are nothing
  but the secret marker, a room not listened to, added `record-only`, archived or read-only, a message at or
  before the room's read position, a muted sender. Where the room was weighed agent by agent, it names each
  designated agent with what kept it out: no call, no reply to its words and its window closed, its road closed
  by the rest, its account's machine gone or left, no conversations standing. A message an agent filed, or one
  standing under a designated agent's own number, is no word owed an answer and says nothing; the owner's
  answer to a job's question, handed to bmo-job, says nothing either.
- **A conversation that took a person's words and lets them go unanswered says so the same way**, once with
  the messages it held: a round its agent's road cannot take under the rest (「The room's queue and the round」),
  words a restart left unread in a room whose answerer's road the rest closes, and words given back by rounds
  that failed the policy's limit. Words the rest held stay unread, the owner's private chat's among them, and
  are answered as the rest lifts (`docs/messages.md`, 「Backlog」).

## The names and the window

- **The names matched** are the agent's registered label and its call names, the `aliases` the agency serves on
  `GET /supervisors` as one line, the names separated by commas. A changed label or call name is matched once the
  receiver's kept list of agents is read again, within ten seconds (`docs/receiver.md`, 「What is kept for a
  moment」).
- **A name is matched as a word,** without regard to case where it is written in Latin letters: only where no
  letter stands right before it, and where what follows it is the end of the words, a character that is not a
  letter, or a Hangul letter. So a Korean particle or a punctuation mark may follow a name and still call the
  agent, while a name standing inside a longer word calls nobody: a Latin name followed by a Latin letter, and
  any name with a letter right before it, as a Korean name ending another Korean word.
- **The secret marker is no word** (`docs/messages.md`, 「Secrets taken out」): no name is matched in it, and
  words that are nothing but markers wake no agent at all, in a user's own private chat too. A file whose caption
  is nothing but markers is judged as a file with no caption.
- **The window is the length the agent's row keeps,** and the policy's three minutes wherever the row names none,
  one length for every room; each room holds its own for each agent that keeps one, stored in the receiver's
  database.
- **A call opens it the moment the receiver sees the call,** so what the caller adds before the answer needs no
  name either.
- **While it runs, new messages keep streaming into the same session.** The agent decides each round whether to
  answer, keep waiting while the author may still be typing (`<<WAIT>>`), or stay silent because the messages
  were not addressed to it (a skip in its form, group chats only: members talking to each other are left alone).
- **Two things restart the window and no others:** an answer that goes out, and a call. Each answer restarts it,
  so a follow-up needs no name; the conversation ends once the window has run out with nothing left to read.
- **The room merely speaking again does not restart it**: that would let a room that talks among itself hold the
  agent awake with no one ever calling it.
- **A round that ends in silence leaves the time it already had to run out** (a skip) because passing over
  what a room said is not answering it, and a window that grew every time the agent stayed quiet would hold a
  session open through a conversation it is not in.
- **A round that ends in silence is written down with why.** Its ending is a judgement of the stage `skipped`
  with the verdict `SKIP`, under the last message the round read, or under none where it read nothing of the
  room, and its `detail` is why the round sent nothing: the reason the skip named, on the one line it is, as the
  round log keeps it, whether the agent's own judgement of the room's words or the round's pipeline ended in
  the skip; and `none given` where the agent named none even when asked over (`docs/instruction.md`, 「The round
  log」). Only a round
  that holds no reason at all carries none, as a quiet turn that only read an instruction does. The stream is told
  the same as `skipped`, with the room, that message, the verdict and that `detail`, so a screen says why the
  agent held its peace.
- **The message the agent writes for words handed to it is an answer** (`docs/messages.md`, 「Words handed to the
  room's supervisor」) and opens one as any answer does: the room has been spoken to, so what anyone there says
  back needs no name to reach the session. Words handed to the agent and not yet said open nothing, since nothing
  reached the room.
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
  came since, and the agent must then answer with what it has; a skip stays valid even then.
- **The room's window is unaffected by the wait** and still runs its three minutes from the call or the answer.

## The owner's own chat is always answered

- **A round that read the owner's own words in the owner's own chat answers them in its own message,** and the
  process's fixed rules hold the agent to that: every message of the owner's the round took is owed a reply
  whatever else the round was carrying, so a skip there is only for words that ask nothing of the agent. A
  supervisor that wants a room held to this chains an instruction stage whose text asks whether the owner's words
  were answered.
- **The turn is told so in as many words**: the permission a quiet instruction brings covers the instruction
  alone, and the line a closed window puts in front of the turn withholds the silence too.
- **A skip that comes back anyway is not taken,** and nothing asks the round a second time over its silence: where
  the message turn skips, the round ends with nothing sent and every turn of it stands in the round log. The
  instruction stages run only once a message stands, so they never turn a silence into a message.
- **The round is where this is held, because the round is what consumed the messages:** they are spliced out of
  the room's buffer as it opens and are nowhere else to be found once it has them, so a silence taken there is a
  question of the owner's that nothing will ever come back to.
- **The owner's own words are those filed under his sender,** on a messenger and in the console alike: words the
  account the web unit links to his sender says in the console stand under it, and another account's words stand
  under that account's record identity and are owed nothing of this (`docs/receiver.md`, 「Console rooms and
  accounts」).
- **A room is judged the other way round and keeps its silence,** because there the agent is one voice among
  several and staying out of an exchange is an answer.

## Replies

- **Answers go out as plain messages, and pinning one to a message is the agent's own judgement.** An answer is
  sent as a reply only where the agent marks it so, in a room whose chat service declares it can point an answer
  at a message; where it cannot, the words simply land in the room. The receiver never pins an answer the agent
  left unmarked and never withholds or moves a pin the agent set: when an answer is pinned, and to what, is a rule
  the agent is given, which the round's pipeline restates where it names the mark (`docs/instruction.md`).
- **The rule the agent is given:** a plain message is the default. A reply is for when the room has moved past the
  message being answered, other people's words standing between them, or when the answer takes one of several
  open questions and has to say which; never in the owner's private chat. Its target is the message the answer
  actually answers, named by the number the transcript shows for it, never a message the answer does not
  address, and an answer carries one reply at most.
- **`<<REPLY:<message number>>` or `<<REPLY #<message number>>>` pins it to the message that number names,**
  however far back it is. The number is the message's own as the transcript shows it: the road's where a road
  numbered it, negative where the record did. A bare `<<REPLY>>`, zero, or a number too large to name a message either side of zero pins it to
  the newest message the round answers, and a number the room's record does not hold pins it to nothing.
- **The sentinel is stripped either way,** and every send to a chat removes any `<<REPLY>>`, `<<FOLD>>`,
  `<<WAIT>>`, `<<SKIP>>` or `<<SKIP: <reason>>>` still standing in the text as its last step, leaving one alone
  only inside a code span or fence, where it is being quoted.
- **Nothing checks the number beyond the room's record:** the chat service refuses a message it cannot reach, the
  answer goes out pinned to nothing and reaches the room either way, and the room and the number it was pointing
  at go to the log, which is how a session aiming at another room's numbers or at messages that are gone is ever
  noticed.
- **A message that replies to another carries that message into the transcript** on its own line: the target's
  number, who sent it, when, and its words clipped to a couple of lines. A target the same prompt already shows
  is pointed at by number and sender alone, since its words are on the page anyway, and a target the archive
  does not hold is named as absent rather than left as a bare number.

## Folds

- **Answers go out unfolded, and folding one is the agent's own judgement.** An answer folds only where the agent
  marks it so, on a road whose dialect declares it folds (its declaration's `foldable`); the room's block carries
  the rule for folding only on such a road. The receiver never folds an answer the agent left unmarked, whatever
  its length, and never drops a fold the agent asked for: the send carries the decision to the chat service as its
  `fold`, and where the fold falls in the words is the dialect's.
- **The rule the agent is given:** an answer whose body runs long behind a lead the room reads first folds: past
  about 600 characters or eight lines, a lead of a sentence or a short paragraph, then sections, a list, a
  quotation, code or details a reader opens only if they want them, however warmly it is written. A short
  answer, a back-and-forth reply of a few lines and an answer that is all one piece with no lead standing apart
  stay open. The measure is the dialect's own fold measure, so an answer the agent folds is long enough to fold.
- **`<<FOLD>>` in the message asks its fold.** It is the message's own mark, so it travels with the message's
  words: whichever answer of the instruction pipeline becomes the message carries it or not, and an answer of nothing
  but the mark is no words. It counts outside a code span or fence, where it is not being quoted, and every send
  to a chat removes it with the other markers.
- **The copy the room's record keeps carries the same decision** to the room's other providers, and every other
  line the receiver carries to a provider (a person's words, a forwarded copy, a line the record keeps) goes
  unfolded.

## The room's queue and the round

- **A message that is to reach the session does not wake it by itself:** it joins its room's queue, the first
  message into an empty queue starts that room's five-second timer (the round policy's `queue_ms`), and when it
  fires everything gathered meanwhile is handed over at once: woken as one batch, or forwarded to the
  conversation already talking.
- **The queues can be held.** Whoever holds an agent's conversations may hold every room's queue: a timer that
  fires while the hold stands waits until it is let go, and everything gathered by then is handed over as one.
- **An empty queue has no timer,** so a quiet room costs nothing and a busy one costs one run per five seconds
  instead of one per message.
- **Quiet instructions, touches and the start-up backlog do not queue.** The start-up backlog is one turn per room
  and agent its unread lines wake, in a room the receiver listens to, handed in room order by the backlog's rule
  (`docs/messages.md`, 「Backlog」); its lines are read as the turn goes out, a room with nothing waiting is not
  served, and a line served is not served again.
- **A session answers one round at a time, and a round is over only once its answer has gone out.** Sending is
  not instant (the road's send may have to be waited out) so a round that counted
  itself finished before it had sent would let the next message start a second run of the same session, and the
  room would read two answers to one thing, the older one arriving last.
- **A message that did not go out is sent again, and a round none of whose sends put it in fails.** The send
  is tried on the road the turn answers on up to the round policy's `[send]` `tries`, `gap_ms` apart; each send
  that put nothing in the room says why in the journal and in the send's `FAILED` verdict. A send that failed
  after it may have reached the chat service, its request broken once it could have been read or its success
  answered in a shape the route does not have, is not tried again and fails the round at once, so the room
  never reads the message twice; a road that carried the message and gave it no number has sent it, and the
  record keeps it under its own number (`docs/unified-bmo.md`, 「The cases」). The round counts as
  answered, its read mark written and its window restarted, only once its message went out. A message no try
  put in fails the round as a failure of the receiver's own, counted: its words, and every word it read
  meanwhile, are left unread and open the next round (`docs/messages.md`, 「Read marker」), and the round's
  failure order is raised.
- **What arrives while a round is running waits in the buffer** and is read when that round hands over.
- **Words the agent was not named in are judged first, by the agent itself.** In
  a room that is not the owner's private chat, a round whose every message reached the session on its open window
  alone — none of them names the agent (「The names and the window」), replies to its words or stands in a private
  chat — opens with the agent's own judgement of whether to answer, before every pregate. The
  judgement is one turn of the same session carrying a marker, the room's situation note where the session
  has not read it (`docs/instruction.md`, 「The situation note」), the room's
  recent words in the room shape and the words being judged as data — whether they are said to it or want its
  answer. What the marker asks is the round policy's head and question (`chat-round.toml`, `[supervisor]`), said in
  the room's block. The turn carries no reference and no index,
  and nothing of it passes any gate. A skip in the round's form (`<<SKIP: <reason>>>`, `docs/instruction.md`, 「What
  it is」) ends the round there: nothing is sent, the words are
  read and the window keeps the time it had, as a skipped round leaves it, and the judgement's row of the round log
  carries the reason it named. An answer skipping with no reason is no verdict: the same judgement turn is asked
  once more under one line naming the form, and a second answer naming none ends the round with its reason recorded
  as `none given`. Any other answer opens the ordinary
  round, its reference and its pipeline. A round with a named call, a reply, words of a
  private chat or the owner's chat skips this step. The judgement is a row of the round log of its own kind,
  `supervisor` (`docs/instruction.md`, 「The round log」).
- **A round goes through the receiver's own parts, every agent's conversations alike:** the turn composed by the
  window is carried into the agent's session through the agency (`POST /sessions`, `GET /sessions/{sessionId}`,
  `POST /sessions/{sessionId}/turns`, and the turn's `turn.created` and `turn.ended` on the event stream); every
  agent's turn, every supervisor's alike, opens the same way as the round's message turn, whose answer is the
  message, which then passes the supervisor's chain of instruction stages (`docs/instruction.md`); the message is
  sent on the road the turn
  answers on, then handed back to the receiver as filed. The round's session is noted, a failed round of the
  agent's own raises its failure order, and the failure orders are looked over on the conversations' own clock.
- **A room's words go into the working turn.** Words the room queue hands over while a turn of a round works, from
  the round's own turn to the question that writes the message, are pushed into that turn
  (`POST /sessions/{sessionId}/turns/{turnId}/inputs`) at the round's next look at it, with the quiet
  instructions delivered meanwhile where the turn is the round's own (`docs/bmo-window.md`, `push`). The agent
  judges each message: answered with the message, set aside as needing no answer, or held to be answered in a
  round of its own (`docs/instruction.md`, 「What came in meanwhile」). What the agency says was `delivered` is the
  round's; what is `waiting` or `refused` goes back to wait, and a `waiting` turn is offered nothing more.
- **What no push put in waits for a question boundary:** under the round's next question, or in the one closing
  check before the message is sent, where the agent marks it the same way. A message pushed or shown inside a
  round opens no round afterwards unless the agent held it; only words the round neither pushed nor showed open
  the next.
- **A round folds its session only past the fold share:** once its answer has gone out, the session is folded as
  work of its own where the context the agency reports it carrying after the round's last turn (its session
  state's `context`, `carried` against `window`) has reached the
  session's fold ratio, and not otherwise. The ratio is the agency's, read with the session's state
  (`foldRatio`, the compaction threshold of the supervisor the session runs for); where the state
  names none, the resident turn policy's `fold_ratio` stands in.
- **The fold is the agency's command, and the brain is told of it.** The session is first asked one quiet turn to
  consolidate its brain (`docs/unified-bmo.md`, 「The brain's consolidation」), which the fold never waits on
  past its end or its failure. The receiver then asks the fold of the session
  (`POST /sessions/{sessionId}/folds`), the program's own compaction, with no words to the agent and nothing
  said in any room before or after it; once the agency answers that the session folded, the receiver reports the
  fold to the agent's brain
  (`POST /brains/{brainId}/folds`, the session's id as its key, `docs/boundary-api.md`, 「The brain」), so what the
  brain marked as handed to that session may attach to it again. A fold the agency refused is reported to nobody,
  and a session whose program carries its own fold is folded and reported by no one here.
- **A session whose program carries its own fold is never folded by the receiver:** where the session's state
  says so (`foldsItself`), no fold is judged or asked for it, whatever its context; the journal says it once for
  the session, "the program carries its fold".
- **A round whose answer went out and whose session does not fold says why** in the service's journal, where
  the reason is other than a context under the share: no session held, no context the session's state names (a
  state that could not be read, with the agency's answer, among them), a fold of the room already under way, or
  the session past its cache lifetime.

## An agent's reset

An agent's reset is the agency's (the agency's `docs/supervisors.md`, 「Starting it afresh」), asked through the
receiver's `POST /supervisors/{supervisorName}/resets` with the rooms the receiver names (`docs/api.md`,
「Supervisors」): it moves the agent's reset point, publishes the cut-off, and then tells each unit holding the
agent's rows to take its own. Both go under the kind `supervisor-reset.created` and are told apart by their
fields: a cut-off names the `point` alone, a reset names its `ranges`.

- **The reset point is the agency's.** The agency answers it as `resetPoint` at its read of the supervisor's
  reset point (`docs/boundary-api.md`, 「The agency」), and a send asked in a turn that began at or before
  it is refused (`docs/chat-mcp.md`, 「Turn context」). Each such ask reads the agent's reset point afresh and
  judges the turn by the later of that point and the one the receiver keeps (`docs/store.md`, 「The settings」),
  keeping the point it read; the kept point alone stands where the agency does not answer or holds no supervisor
  of the name. A reset the receiver did not hear, published while its stream was down and no longer kept by the
  agency, still overtakes the turns it cut off.
- **The cut-off is the live signal,** heard for as long as the service runs. Its point is kept as the agent's
  reset point, a later point in place of an earlier one. Everything waiting for the
  agent's turns in every room is let go unread: the words its rooms' queues are gathering, the words its conversations hold or wait on,
  the words a push took for a working turn that the agency has not yet answered for, and its quiet instructions,
  among them those such a push took, whose rows are let go as `reset` so no turn takes them again. The rows are
  settled as the cut-off is heard, whatever the agency later answers the push; a push it overtook adds nothing to
  its round and gives nothing back to wait (`docs/bmo-window.md`, 「The openings」, `push`). An agent whose
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

- **Every instruction carries its cause, and the cause decides whether it waits.** A row is a room reply or
  housekeeping, as its producer declares it (`docs/api.md`, 「Instructions」, `cause`):
  - **A room reply** answers something a room or the owner asked or set going: a research run's result
    (`research-ended`), a delegate's or a worker's answer, a job's ending and report, a reminder or a watch come
    due (`use-ended`), words handed to the room's supervisor, a relayed answer, an
    instruction typed on the console. It starts the agent's turn by itself the moment the pass finds it, on a
    session past its cache lifetime too: that turn replaces the session through the handoff and the transcript
    exactly as a room's own round does (`docs/unified-bmo.md`, 「Replacing the session」), and no person's words
    are waited for. A row whose producer names no cause is a room reply.
  - **Housekeeping** is a record for the agent alone that nobody is waiting on: every row marked `agent_only`
    (the tool host's changes, a document reload, a failure order, a run past its clock) and the deployment
    briefing. On a session past its cache lifetime (`providers.toml`, `cache_minutes`, else
    `keepalive_idle_minutes`) a housekeeping row that would open a turn of its own stays pending and waits for
    the next round anything else opens, which carries it; waking a cold session for it would pay its whole
    prefix again for nothing. The keepalive touch is no row and never wakes such a session either
    (`docs/bmo-window.md`, 「The touch」).
- **A row keeps the unit or source that published it.** `POST /instructions` may name it as `publisher`; where
  the row carries none, the receiver takes the first word of its `kind` and keeps that with the row, and a row
  naming neither stands as `the system`. The turn the row reaches names what it kept (`docs/bmo-window.md`,
  「Quiet instruction」).
- **A pending room reply is an obligation the receiver keeps until a round has taken it.** It carries no age and
  nothing lets it go for having waited. The pass looks at every pending row on each tick, at every opening of
  the agency's event stream (「What the agency's events wake」 below) and as the receiver starts, and hands each
  reply it finds. A row handed into a round that fails goes back to pending, its failure counted only where the
  agency answered it (`docs/bmo-window.md`, 「Quiet instruction」), so the next pass hands it again; while the
  agency does not answer, the pass holds the agent's rows rather than handing them into rounds that cannot
  reach it, and hands them once a read of the agency is answered. Housekeeping goes back the same way: a round
  that failed settles nothing it took. A session the agency ended is
  no reason to wait: the reply's turn asks the agency for the agent's session, which starts a fresh one where
  none stands, and the agency's `session.deleted` brings the pass forward at once.
- **The deployment briefing is written by the wake itself:** what went live since the owner was last briefed.

- **A deployment is no session event.** The agency relays a turn into the session that stands, opening one only
  where none does, so a deployment of any unit, the agency's included, neither reopens nor relaunches the
  agent's session, and no instruction says it did. The session is replaced only as `docs/unified-bmo.md`,
  「Replacing the session」, states.
- **Each round's start looks for deployments since the last briefing:** every unit whose deployment record
  (`docs/boundary-api.md`, 「The deployments」) went live after the moment the agent's session was last briefed,
  oldest first. The moment is kept under the session's setting (`docs/store.md`): the agent's one session across
  its rooms where its registration says so, else the room's own. A session never briefed reads as briefed the
  restart policy's fresh while ago (`fresh_minutes`). Where none went live since, nothing is written.
- **Only an agent that answers the owner's private room briefs him,** by the rule 「Who reaches the session」 reads
  who answers a room. A round of any other agent — a person's own supervisor in their room, a group's BMO — looks
  for nothing and writes nothing, and its moment stays unset: a row for an agent that does not answer the owner's
  room would never be handed there. Where who answers cannot be read, nothing is written either.
- **One instruction names them,** in the restart policy's words (`restart.toml`, `briefing`): what went live
  since the last briefing, one clause a unit naming its unit, version, deploy message or subject, commit and the
  moment it went live, as the deployment record holds them. Its row is of the sort `briefing` and the kind
  `deployments-briefed`, which a newer one replaces while it waits, and writing it moves the session's moment to
  now.
- **It asks for the owner's briefing.** Its intent and follow-up tell the session to tell the owner, in his
  private room, what went live: one short paragraph in his language and in the agent's own words, with no talk of
  tools and nothing claimed of a release beyond its message, subject and commit. The instruction withholds the
  silence: a turn that carries only it writes the brief. A turn the owner's own words opened answers him first
  and gives the brief after it.
- **It is the owner's own** (`docs/bmo-window.md`, 「Rows that ride」): the row is queued for his private room, and
  only a round of that room carries it. It is written before that round asks for its riders, so the round's own
  turn carries it. Unridden past the restart policy's ride while, it opens a turn of that room.
- **No other room hears of it.** The briefing is said in the owner's private room alone; the room whose round
  found the deployments is told nothing of them.

### The tool host's changes

- **A tool placed on the tool host or taken from it is told to each supervisor it concerns,** as the host
  publishes it (`docs/boundary-api.md`, 「The tool host」): one notice a supervisor whose standing tool session
  holds one of the event's namespaces, nothing to an agent whose session holds none of them.
- **The notice is a marker and a fact** in the tool host policy's words (`policy/tool-host.toml`): the short
  marker that names it, then which tool was placed, updated (a placing that replaced another version of it) or
  removed, each entry by its full name, how it is reached (`call` or `act`) and the first sentence of its
  description. The rule the marker stands for — that a tool is read with `bmo use <tool> --describe` before it
  is called, and that nothing needs saying in any room — stands once in the room's block, never in the notice.
- **It rides and wakes nobody** (`docs/bmo-window.md`, 「Rows that ride」): it is queued in the owner's notices room
  under the kind `tools-changed:<agent>:<tool>`, so a later change of the same tool replaces the agent's one not
  yet handed over, and rides for good in a round of any room the agent's session answers. It is marked as an
  agent-only notice: a round carrying only it sends no room message (`docs/bmo-window.md`, 「Quiet instruction」).

### A road's change

- **The agency's `road.changed` is told to the owner once,** through the default answerer's session in his private
  chat, as the deployment briefing is (the agency's `docs/api.md`, 「Events」): one instruction an event, in the
  owner facts' words (`policy/owner-facts.toml`, `[road]`), under the kind `road-changed:supervisor:<name>` or
  `road-changed:role:<name>`, so a newer fact of the same supervisor or role replaces one still waiting and none
  is said twice.
- **What it carries:** the supervisor or the role whose work it is, named as such; for `road-moved`, the road it
  moved from and the road it moved to, each its
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
to, and those that tell the default answerer something wake it as an instruction in the owner's private chat, which
opens a round there like any instruction:

- **A voiced notice:** a job's run fallen to a lesser model, named by its role and its repository with the
  checkout it ran in beside it (`docs/voiced-notices.md`, 「Rest and a run's fall」).
- **An order:** a run past its clock, withdrawn where its ask closes first (`docs/voiced-notices.md`, 「A turn
  past its clock」).
- **No session at all:** the deployment resting, told in its fixed line, and a login's end, said as the agency
  words it; neither wakes anyone.
- **A road's change:** `road.changed`, told to the owner as 「Quiet instructions」 above states.
- **A session ended:** `session.deleted` wakes nobody by itself; it brings the instruction pass forward, so a room
  reply owed to the agent starts its turn on a fresh session at once.

The stream is heard from where the receiver left it:

- **The last event heard is kept** (`docs/store.md`, 「The settings」, `agency_events_heard`): its number and the
  moment the agency published it, written as each event is handed to its hearers.
- **A stream opened again resumes after it.** Each time the stream opens, the receiver's start among them, the
  live stream is opened first and then the events the agency still keeps are read as a page
  (`docs/boundary-api.md`, 「The agency」): where the page holds the last event heard under the same number and
  moment, every event after it is handed in order; where it does not, the agency numbered its events afresh or
  let the mark go, and every event it keeps is handed as new. The agency's numbers only rise, across its own
  restarts too, so a page that does not hold the mark holds only events after it. A live event numbered at or
  below the last one handed is not handed twice.
- **Then the receiver reconciles its rows with what the agency still holds.** Every row it marked handed whose
  room's conversation no longer stands in this process, and which rides no round of another room, is made
  pending again with its carrying cleared, and the instruction pass runs at once over every pending row. A round
  whose turn the agency lost meanwhile ends failed on its own, uncounted (`docs/bmo-window.md`, 「Quiet
  instruction」), giving back everything it took.
- **What the receiver cannot recover** is an event the agency no longer keeps: one published while the receiver
  was away past the newest events the agency keeps in its store. The reconcile and the pass's own rescan stand for
  those, so no reply owed is lost with them.

## Words handed over

- **Words anything but the room's supervisor wants said wake that supervisor as an order** in the room they are
  for (`docs/messages.md`, 「Words handed to the room's supervisor」): one instruction of the sort `handed` each,
  which opens a round there like any instruction and never rides, since somebody is waiting on it to be said.

## A job's end

- **A job's end wakes the default answerer through bmo-job's instruction:** its ending, its report and its state
  facts are queued with `POST /instructions` as a fact for the agent in the room the work was filed from, or
  the owner's private chat once that room is gone, and open a round there like any instruction
  (`docs/voiced-notices.md`, 「A job's end」).
