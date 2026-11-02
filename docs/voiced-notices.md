# Voiced notices

How a room is told something on the system's own account: a restart, a setting changed, a rest lifted, a
background run that failed. There are no fixed sentences in the receiver: every room has its own voice, so the
agent that answers the room says the line.

## The rule

- **A briefing goes to the owner's private room, and to no other room unless that room's own setting asks for
  it.** The deployment briefing, a notice turn's summary, the voice briefing's switch line and a deployment's
  going-down and back lines are briefings; the configuration's `notices_room` never moves one.
- **Every other system notice to the owner stands in his notices room:** the room the configuration's
  `notices_room` names (`docs/boundary-api.md`, 「Configuration」), his private chat where it names none. The tool
  host's changes, a rest's and a fallback's notices, a worker's fall, a lost handoff and a run past its time go
  there; a notice below said to go to the owner's private chat goes to his notices room where one is named.

- **Every line the system says in a chat on its own account is a voiced notice.** The receiver does not send a
  sentence. It hands the room's voice a quiet instruction carrying the facts, and the session says it in one
  short line, in that room's voice and manner.
- **The room's voice is the agent that answers it:** the chat agent in a room it answers (every room that names
  nobody awake, and every room naming it among its answerers) and, in a room that names only other agents, the
  first of them by name. The row is that agent's, which only
  its turns take, so a notice for a room the chat agent does not answer reaches the room rather than being let
  go as its.
- **The caller owns its facts.** The code that knows what happened writes what the session is told: what
  happened, who asked for it, whether anything follows, as a code with its fields. There is no shared table of
  wordings; the only shared piece is the frame around the facts.
- **The frame** asks the session for:
  - the room's language, plain text, one short line, in this room's manner;
  - nothing about models, code, prompts, hosts or other internals, unless the chat is the owner's private chat;
  - its own words, never the fact text copied out;
  - no `<<WAIT>>` and no `<<REPLY>>`; `<<SKIP>>` only where the facts say the line is optional.
- **Recipients and switches are the notice's own.** A voiced notice goes to exactly the chats the line is for,
  under the per-chat switches (`notice:rest`, `notice:restart`), and the rest rule holds: a rest and its lifting
  are told to the owner alone; a room hears only its compaction when its rest notice is on.
- **Pass-through content is not a notice.** A search, browse or say result, or a session's own answer is already
  written by an agent and goes out as it is. A job's report is not among them: it is the worker's English to the
  agent, and the agent tells it (「A job's end」 below).
- **In the owner's private chat a notice stays in the chat's record.** A turn of the system's own lines answers
  into the owner's chat's record, which the console's screen shows, and the lines the receiver says itself in
  that chat do too (`docs/unified-bmo.md`, 「The cases」).

## Delivery

- **Through the instruction queue, always.** A notice is a queued instruction row, never a direct hand-off to a
  live session: the queue holds a row while its chat is silenced, where a direct hand-off into a silenced chat
  would be lost. bmo-job queues its notices with `POST /rooms/{chat}/notices`, the brain its owner notices with
  `POST /owner/notices`.
- **Delivered at once, unless it rides.** The instruction pass delivers pending rows on its regular tick; a
  notice queued through a route or by the receiver itself also asks for a pass straight away, so it does not wait
  for the tick. A notice queued to ride waits for the next round its room opens for something else instead
  (`docs/bmo-window.md`, 「Rows that ride」).
- **One line per kind.** Each notice carries a kind (a setting, a rest lifted, a deploy going down). A notice of
  a kind still undelivered for that chat is replaced by the newer one, so a setting toggled twice, or a state
  that flapped, leaves one line.
- **A notice waits until it is said.** A notice carries no expiry: it waits for a turn to take it, however long
  that is, and a state that has moved on meanwhile replaces it by the newer notice of the same kind.
- **Shut rooms are not woken.** A row for a chat that is not allowed is discarded and logged, never left pending.
  The owner's private chat is always deliverable.
- **Nothing waits on a notice.** A launcher that starts a slow tool returns at once; no tool holds a room while a
  notice is spoken. Where a session starts the tool itself, its own turn already is the notice.

## What each path says

- **Search and browse.** No script announces a search. The session that launches it says in the same turn, in
  its own voice, that it is looking something up, or stays silent where its room's persona wants a quiet lookup.
  The result returns later as a quiet instruction. A run somebody stopped hands back what it had written where
  there is anything to hand back; with nothing written, the room hears one line saying the work was cut off
  with no result and that nothing more is coming.
- **Deploy and restart.** The deployment's purpose says why this goes live now, and whether the work is finished
  or continues.
  1. Before the receiver stops for a deployment, the deployment records its purpose and the items going live,
     and the receiver sends the going-down notice as it stops (「System notices」); no session is woken to say
     it, since a turn spent on going down is a turn the owner's session reads again on every turn after it.
  2. After starting, the receiver queues one back notice for the owner's chat naming the unit deployed, its
     commit and subject, the item it served, and whether it cut a chat turn or a job. The unit, commit and
     subject are the ones a deployment recorded as the `restart_reason` setting where one stands; where none
     does, they are the receiver's own deployment record, `deployed.json` under the deploy root its release
     stands in, where that record went live within the fresh window (`policy/restart.toml`,
     `fresh_minutes`). The same facts the deployment briefing names, so the two never contradict. When a job
     is still running or will report, the notice says so, and the session neither summarises the work nor
     calls it finished: the job's own report does that once. A turn the restart cut is named as cut, and what
     it had read is handed back after the restart.
  3. The back notice after a deployment rides for three minutes: the deployment's outcome fact (`deployed`, or
     the way it stopped short) follows within seconds and opens the one turn the restart costs, with the back
     notice and the queue standing at start-up in it. A deployment that tells no outcome leaves the back notice
     to open its own turn when the three minutes are up.
  4. The back line is never sent verbatim. A job's line is English, as every close's reason is, and the session
     says it in the room's own language.
  5. A restart no deployment announced, one with neither a recorded reason nor a fresh deployment record, has no
     purpose to tell: its going-down notice is the restart one, and its
     back notice opens its turn at once and says only that the agent is back from an unannounced restart.
  6. **What went live is briefed to the owner, not the room.** At the first round after a deployment went live,
     the deployment briefing (`docs/wake.md`, 「Quiet instructions」) goes to the owner's private room,
     naming one clause a unit: the unit, its version, its deploy message or subject, its commit and when it went
     live, as the deployment record holds them. It asks for one short paragraph in the owner's language, in the
     agent's own words, with no talk of tools and nothing claimed of a release beyond those facts. It is the one
     voiced briefing that is a paragraph, because it names each deployment. A turn that carries only the
     instruction writes the brief; a turn the owner's own words opened answers him first. A release that failed
     its checks or never went live has no deployment record and is named in no brief; the deployment outcome
     notice tells that release's outcome as it happened.
- **Rest and fallback.** Going to rest is a fixed line (see below), said in the owner's notices room as the agency's
  `rest.changed` comes: `rest-begins` with its end for a limit, `provider-down` for an outage. Coming back
  (the rest lifted, the outage over, the compaction done, the provider answering again, a stand-in taking over
  or stepping down, a downgraded answer) is voiced. The agency publishes the stand-in's moves and a worker's
  fall as events (`fallback.changed`, `worker.changed`) and the receiver queues each notice for the owner's
  notices room: the stand-in's provider and model, with the one before it where one stood, or the stand-in done; and
  the worker, the model it fell to and the one it was launched on, its job, and whether its session was dropped
  or kept.
- **A login's end.** The agency's `account.changed` carries its `notice`, the words its policy composes
  with the one command the owner runs; the receiver says it in the owner's chat as it stands, pass-through
  content no session rewords.
- **Room settings.** The owner's in-chat setting commands are acknowledged by that chat's session after the
  setting has changed, so a change that relaunches the session is acknowledged by the new one.
- **A broken run.** A chat turn of the agent's own that died, or a job that failed while it worked, is handed to
  the agent's session in the owner's chat as an order: the facts read where it broke, and an instruction to find
  the cause and fix it at the root. The agent tells the owner in its own words; the receiver sends a fixed line
  only when the agent cannot answer at all. A turn somebody stopped is the other case, and it goes the other
  way: the room that was watching the agent type hears one voiced line saying the answer was cut off with no
  result, the owner hears nothing at all, and nothing is recorded to recover from: a stop is not a failure.
- **A turn past its clock.** A chat agent's turn past its clock, the chat agent's or an added agent's, gets no
  holding line in any room: the console shows the answering agent at work. The order below is for a worker's job
  alone, in the notices room. A worker's turn that reached its bound (the agency's `overtime-request.created`) is
  handed to the agent's session in the owner's chat as an order to decide on, keep or cut, never as a line to
  say; the order is withdrawn where `overtime-request.changed` comes first.
  - The order carries the ask's number, what the run is doing, its agent and run, how long it has run and since
    when, its clock, its last sign of work and when it was, and the word that nothing cuts the run without a
    verdict; it names the procedure `overtime` and the verb the event names, with its keep and cut forms.
  - It is kept under the run's own kind (`overtime:<run>`), so a later ask on the same run replaces one not yet
    handed over, and the ask's closing withdraws only an order no round has taken.
- **A job's end.** The job's report is the worker's English, written to the agent, and carries the system:
  commits, test and suite counts, file counts, worker and model names, paths, commands. The receiver sends it to
  nobody: the agent that filed the item tells it. An item another agent filed is told to that agent in the room it
  was filed in, where the agent still answers it, as the one fact below, report and all, and the room hears no
  line of it besides; every other item, the chat agent's own and one whose agent no longer answers its room, is
  told by the chat agent as follows.
  1. **The agent writes the owner the report.** The ending fact is told in the owner's own room the report
     belongs to (the room that asked where that is one of the owner's own, the owner's private chat otherwise)
     and carries the worker's report whole. The agent writes the owner the report in its own words there: the
     whole account and not a summary of it, in the same shape (a summary paragraph, the detail, a closing line of
     figures), commits, paths, commands and counts kept as they stand, followed by what the report could not say:
     a deployment it starts, the next item it launches, why a run failed, a decision the owner has to make. A fact
     carrying a report is never answered `<<SKIP>>`.
  2. A room other than the owner's own that asked for the work hears one voiced line as well: that the work it
     asked for is finished, or that it is not, and what it came to for the people there.
  3. The facts of that line are the report's summary paragraph with every sentence that names the system or
     carries code taken out whole, so the session is never handed the internals to leave out, and its frame asks
     for nothing of the system on top of that.
  4. **The report is told once.** The ending fact names the room that heard a line of it, so the agent does not
     tell that room again.
  5. A fact with no report in it is an ending the agent tells by itself, from what the fact says.
  6. bmo-job decides which room the fact goes to and queues it; the receiver delivers it by the rules standing
     when it is delivered.
- **Background runs.** A digest or person-book run that failed, a person-book sort that gave up on a room, a rule
  candidate awaiting approval, a paused digest, a voice briefing that failed, a job that died without reporting:
  each is voiced to the chat that would have heard it, from the code its unit sends. A failed voice run, and a
  dead job, say plainly that nothing more is coming.

## The voice briefing

A line of the chat agent's is heard as well as read: a notice turn of the owner's private chat is spoken as a
summary, and any agent's line can be read out whole from its chip. The voice engine speaks and keeps every
briefing with its room and its line (its `docs/boundary-api.md`, 「Briefings」); the receiver decides when one is
spoken, and the session writes what is said.

- **The session writes the script the voice speaks, as written words.** Both the summary and the full reading
  are asked of the agent's own session as a spoken script: sentences a person would say aloud, with no markup,
  symbol, list or quotation, and names, acronyms, numbers, dates, times and units written as the line writes
  them. No gate stands between the session and the engine: the receiver hands the engine exactly the text the
  session wrote, as the briefing's `text`, which the engine keeps as its transcript.
- **The engine derives the spoken words.** A briefing carries no spoken words of its own: the engine makes them
  from the text, the same words as a Korean reader says them aloud, and checks them before it speaks. Spoken
  words it cannot clear are a refusal of the briefing (`unconverted-run`), handled as every refusal of it is.

### The switches

- **One switch for the whole briefing,** the receiver's setting `voice_briefing`, `on` or `off`; a missing row
  reads as on. It is read at every door: before a notice turn's summary is asked for and before a line's full
  reading is launched. A refusal is `switched-off` (`docs/codes.md`, 「VoiceRefusal」).
- **A second switch, whether a briefing plays by itself,** the setting `voice_briefing_autoplay`, on unless it
  holds `off`. It decides only whether a listener plays a summary on its own: the receiver hands it to the engine
  with each summary, as it stands at that moment, and the engine keeps it on the briefing and its events. It stays
  switchable while the whole briefing is off.
- **The console moves them** through `PUT /voice-briefing`, one switch at a time, and reads both through
  `GET /voice-briefing` (`docs/api.md`, 「The voice briefing」). A request naming neither, or both, is refused
  `one-switch`.
- **Moving the whole briefing is told to the owner.** The receiver queues the voice fact `briefing-on` or
  `briefing-off` for the owner's private room under the kind `voice-briefing`, so a switch flipped twice before
  the session speaks leaves one line, about where it ended. A write that changes nothing says nothing, and moving
  the second switch says nothing: it changes how the owner hears a briefing, not what the agent says.

### Whose voice

- **A voice is an agent's,** the `voice` its registration names at the agency, a voice the engine holds by that
  name. An agent whose registration names none is never spoken: no summary is asked of its turns and its line's
  reading is refused `voiceless`.
- **Nothing is spoken without the engine.** A configuration that names no voice engine speaks nothing: no summary
  is asked for, and a reading is refused `unreachable`, as is one the engine did not take.

### The summary of a notice turn

- **A turn qualifies** when all of these hold: it is the owner's private chat; nobody spoke in its round, so it
  carried quiet instructions and no message of anyone; it sent a line, so a `<<SKIP>>` or a `<<WAIT>>` is not
  spoken; the switch is on; and its agent has a voice.
- **The session writes the summary,** in a quiet instruction of its own, asked of the same session in one light
  turn once the line has gone out and before the round lets go of the room. The instruction carries the line as
  the facts, and asks for what is to be heard: a one-line title saying what the briefing is about, in the agent's
  own words for this briefing, then one or two sentences of the line's gist as a spoken script, never the line
  read out, every notice of the turn covered in its newest state (`policy/chat-round.toml`, `[voice]`,
  `summary`). Its answer goes to no road and stands in no room.
- **A session that gives no summary has its line's opening spoken:** the first sentence, or the first two where
  the first is short, with no title. An answer of `<<SKIP>>`, an empty answer and a failed turn are each this case.
- **The receiver carries the facts to the engine:** `POST /briefings` of kind `summary`, in the agent's voice,
  naming the room and the message the line went out as, with the title where the session gave one and
  whether it plays by itself as the second switch stands. The line already stands when the summary is asked for,
  so the summary never awaits it.
- **A summary that fails tells nobody.** The line stands in the room as text either way, so a turn that could not
  be asked, an engine that refused the briefing or a synthesis that failed is the receiver's journal line or the
  engine's own, and no notice is queued.
- **Ordinary conversation is never spoken by itself.** A round somebody spoke in, the owner in his own chat or a
  message pushed into a round a notice opened, is a conversation, and its answer stays text.

### The full reading on a line's chip

- **Every line of an agent with a voice can be read out whole,** in that agent's voice, through
  `POST /rooms/{roomId}/messages/{messageId}/reading`. The line is a message the receiver archived as an agent's
  send, with words of its own; any other message is `not-found`.
- **Nothing is made before it is asked for.** The receiver asks the engine for the line's full reading
  (`GET /briefings` by room, line and kind): one waiting or speaking answers `making`, one done answers `ready`.
  Where there is none, or the last one failed, it answers `making` at once and asks the line's agent, in one light
  turn of its session in the line's room, for the whole line as a spoken script (`policy/chat-round.toml`,
  `[voice]`, `reading`), every figure, name and conclusion kept, then launches it with `POST /briefings` of kind
  `full`; a session that gives no script, or one past its cache lifetime, which is never woken for it
  (`docs/unified-bmo.md`, 「Replacing the session」), has the line spoken as it stands. While the script is being written an
  ask answers `making` with no briefing yet; once launched, the answer names the briefing, whose sound the engine
  serves.
- **It is kept on the line** by the engine, so a later ask answers `ready` at once. It goes to no road and no
  listener plays it by itself.
- **It asks the switch and the voice** like every briefing: switched off, it is refused `switched-off`; an agent
  with no voice, `voiceless`.

### What a listener hears

- **The engine's event stream.** A listener reads `briefing.ended` on the engine's stream and plays a summary
  that is `done` and plays by itself; it shows rather than plays one it learns of more than two minutes after it
  ended. The sound is the engine's `GET /briefings/{briefingId}/sound`.
- **A briefing a unit other than the engine put in a room as a voice message** is told through the receiver's
  `POST /events` as a `briefing` event, stamped with whether it plays by itself as the second switch stands, and
  goes out on the receiver's stream to the same listeners.

## Operating notices per chat

- **The restart notice is the owner's private room's.** The going-down line is a system notice the stopping
  receiver sends, a deployment's or an unannounced stop's (「System notices」). On the next start-up the owner's
  session is told it is back, with the deployment's purpose, and says so in the same turn as the deployment's
  outcome; the same turn or the one the deployment briefing opens also briefs him on what went live (「What each
  path says」).
- **No feature announcements.** A deployment tells no group room what the agent gained, and no room hears the
  deployment briefing; the owner's own chat hears the going down, the coming back and what went live.
- **Two kinds of notice can be switched off per chat:**
  - the *rest notices*: in a room, the compaction's going to sleep and waking, since a rest itself and its
    lifting are told to the owner alone;
  - the *restart notices*: going down, back up.
- **Where they are switched:** inside the chat with `notice:rest:off` / `notice:rest:on` and
  `notice:restart:off` / `notice:restart:on` (or the words of the policy for each), or from the shell with
  `bmo room notice --chat <id> --rest on|off --restart on|off`, which refuses a value other than on or off before
  it writes either.
- **Both kinds are on unless switched off;** the setting is stored per chat in the receiver's database.
- **The rest notice cannot be switched off in the owner's private chat,** because the owner has to know why the
  agent has gone quiet everywhere; the restart notice there can.
- **The infrastructure room** is switched off for both, and hears nothing of the agent's own state whatever those
  switches say (`docs/rooms.md`, 「The infrastructure room」).

## A failure says why

- **Every failure notice carries its cause** as a plain sentence under `cause`, beside what failed and what
  follows. The session says it in its own words, so the owner hears why and not only that.
- **The run that failed owns its cause.** The unit that saw the failure writes the sentence, in English.
- **No internal message is pasted.** An error's text, a stderr tail or a list of problems goes to the unit's
  journal or the run log, never into the fact, and never cut to a length mid-sentence.
- **An unknown cause is said to be unknown**, with where the whole account is: the run log, the unit's journal, a
  job's record.
- **A broken run is an order, not a notice.** The agent reads the cause and says it itself; the one fixed line is
  the last resort for when no session can answer.

## System notices

- **A line the receiver sends as the service, not as an agent, is a system notice.** It opens with the system
  mark and states what the service is doing, in no persona's voice; the archive files it as the service's own
  line, mark and all.
- **The going-down notice is one.** The receiver sends it to the owner's private room as it stops, under the
  restart switch: a stop a fresh deployment record speaks for names the items the record carries, and a stop no
  deployment announced says the service restarts. The back line stays the agent's own, voiced (「What each path
  says」).
- **The console stands it as a system line** rather than a bubble: a message filed under the service's own
  sender whose words open with the mark.
- **It is never heard in an agent's voice.** It carries no full-reading chip, and no turn sends it, so no notice
  turn's summary speaks it.

## Lines that stay fixed

Each is sent as a code with its fields, because no session is to say it in its own words: no session can say it
at that moment, or the line must reach the room as itself. Its words in the room's language are rendered by the
web unit's language packs (`POST /words/render`, `docs/boundary-api.md`, 「The web unit」).

- **A person never reads a code.** Where the web unit cannot render a line, or its pack holds no sentence for the
  code, the line falls back to the plain sentence the receiver's policy keeps for that code in the rooms' language
  (`policy/fixed-lines.toml`), its fields filled in as words: a list as its members, a draw's result as its values.
  Where the policy holds no sentence for the code in that language, the line is dropped, nothing reaches the room,
  and the journal says which line and why. A code and its fields, or any JSON, never stand in a room.
- **A session may ask for one through a send.** The send tool's `fixed_line` kind carries a code and its fields,
  and the receiver renders and says it without the output gate or any session, for a line that must reach
  the room as itself (`docs/chat-mcp.md`, 「Its entries」). A code that neither the pack nor the policy can make a
  sentence of is refused, and nothing is said.

| Line | Why it stays fixed |
| --- | --- |
| `rest-begins` (a usage limit begins) | No model can answer while the limit holds. |
| `provider-down` (the provider stopped answering) | No model can answer during the outage. |
| `compacting` (a room's session is being folded) | The session about to be folded is asked for its own line first; the fixed line stands in only when it cannot answer. |
| `compact-failed` (the fold did not happen) | The room was told it was going for a nap and nothing changed; the session it would be asked is the one that could not be folded. Said once per cause. |
| `run-broken`, the last resort when a run broke and the agent cannot answer the order about it | No session is able to say it; it is composed out of what the broken run left. |
| `chat-denied` | The room is being shut; its session is not woken for it. |
| `going-down`, `going-down-for-update` (the system notices above) | The service is stopping. |
| `draw` (a fair draw's room line) | A gate that rewrote it would make the draw impossible to check (`docs/random-draw.md`). |
| A job's question to the owner, and the job's answers when the owner talks it over | A numbered form the owner answers by number; its options, and what the job says about them, reach the owner exactly as the job wrote them. bmo-job sends them. |
| Pass-through content (search, browse and say results) | An agent, or whoever set it, already wrote it. |
