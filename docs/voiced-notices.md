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
  host's changes, a rest's notices, a job's run's fall, a lost handoff and a run past its time go
  there; a notice below said to go to the owner's private chat goes to his notices room where one is named.

- **Every line the system says in a chat on its own account is a voiced notice.** The receiver does not send a
  sentence. It hands the room's voice a quiet instruction carrying the facts, and the session says it in one
  short line, in that room's voice and manner.
- **The room's voice is the agent that answers it:** the supervisor a user's own private chat's person owns where
  one stands; otherwise the default answerer in a room it answers (every room that names nobody awake, and every
  room naming it among its answerers) and, in a room that names only other agents, the first of them by name. The
  row is that agent's, which only
  its turns take, so a notice for a room the default answerer does not answer reaches the room rather than being let
  go as its.
- **The caller owns its facts.** The code that knows what happened writes what the session is told: what
  happened, who asked for it, whether anything follows, as a code with its fields. There is no shared table of
  wordings; the only shared piece is the frame around the facts.
- **The frame is a marker and data.** The turn opens on `NOTICE.`, or `NOTICE, MAY SKIP.` where the line may be
  left unsaid, then the facts whole, then what the line is for and what follows it. It carries no instruction
  sentence: the room's block says, for each marker, that the session says the line in the room's language, plain
  text, one short line, in this room's manner; that it says nothing about models, code, prompts, hosts or other
  internals unless the chat is the owner's private chat; that it uses its own words, never copying the fact text
  out; and that it answers no `<<WAIT>>` or `<<REPLY>>`, and skips only under the optional marker, in the round's
  form, which names why (`docs/instruction.md`, 「What it is」).
- **Recipients and switches are the notice's own.** A voiced notice goes to exactly the chats the line is for,
  under the per-chat switch (`notice:restart`), and the rest rule holds: a rest and its lifting are told to the
  owner alone, and a room's session folding says nothing to any room.
- **Pass-through content is not a notice.** A search, browse or say result, or a session's own answer is already
  written by an agent and goes out as it is. A job's report is not among them: it is the worker's English to the
  agent, and the agent tells it (「A job's end」 below).
- **In the owner's private chat a notice stays in the chat's record.** A turn of the system's own lines answers
  into the owner's chat's record, which the console's screen shows, and the lines the receiver says itself in
  that chat do too (`docs/unified-bmo.md`, 「The cases」).

## Delivery

- **Through the instruction queue, always.** A notice is a queued instruction row, never a direct hand-off to a
  live session: the queue holds a row while the rest closes its agent's road, where a direct hand-off to a
  session that cannot take a turn would be lost. The agency queues the job queue's notices with `POST /instructions`, as `bmo job ask` does a job's
  question from the job's own process; a unit that tells the owner a fact as a code and its parameters, with
  no words of its own, queues it with `POST /owner/notices`.
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
  its own voice, that it is looking something up, or stays silent where its persona wants a quiet lookup.
  The result returns later as a quiet instruction. A run somebody stopped hands back what it had written where
  there is anything to hand back, and says nothing more.
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
- **Rest and a run's fall.** Going to rest is a fixed line (see below), said in the owner's notices room as the
  agency's `rest.changed` comes: `rest-begins` with its end for a limit, `provider-down` for an outage. Coming back
  (the rest lifted, the outage over, the provider answering again, a downgraded answer) is voiced. A supervisor
  stands on one road, so no stand-in answers for it while that road is closed and no notice tells of one. The
  agency publishes a job's run's fall as an event (`worker.changed`) and the receiver queues its notice for the
  owner's notices room: the role the run ran as and the repository it ran in, with the checkout of it beside it,
  the model it fell to and the one it was launched on, its job, and whether the session its role keeps there was
  dropped or kept. A checkout no repository holds any longer is named by the checkout alone.
- **A login's end.** The agency's `account.changed` carries its `notice`, the words its policy composes
  with the one command the owner runs; the receiver says it in the owner's chat as it stands, pass-through
  content no session rewords.
- **Room settings.** The owner's in-chat setting commands are acknowledged by that chat's session after the
  setting has changed, so a change that relaunches the session is acknowledged by the new one.
- **A broken run.** A chat turn of the agent's own that died, or a job that failed while it worked, is handed to
  the agent's session in the owner's chat as an order: the facts read where it broke, and an instruction to find
  the cause and fix it at the root. The agent tells the owner in its own words; the receiver sends a fixed line
  only when the agent cannot answer at all. A round that failed, and a round that ended with nothing said, send
  no line to their room. A turn somebody stopped says nothing anywhere: the room that was watching the agent
  type hears nothing, the owner hears nothing at all, and nothing is recorded to recover from: a stop is not a
  failure.
- **A turn past its clock.** A supervisor's turn past its clock gets no
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
  line of it besides; every other item, the default answerer's own and one whose agent no longer answers its room, is
  told by the default answerer as follows.
  1. **The agent writes the owner the report.** The ending fact is told in the owner's own room the report
     belongs to (the room that asked where that is one of the owner's own, the owner's private chat otherwise)
     and carries the worker's report whole. The agent writes the owner the report in its own words there: the
     whole account and not a summary of it, in the same shape (a summary paragraph, the detail, a closing line of
     figures), commits, paths, commands and counts kept as they stand, followed by what the report could not say:
     a deployment it starts, the next item it launches, why a run failed, a decision the owner has to make. A fact
     carrying a report is never answered with a skip.
  2. A room other than the owner's own that asked for the work hears one voiced line as well: that the work it
     asked for is finished, or that it is not, and what it came to for the people there.
  3. The facts of that line are the report's summary paragraph with every sentence that names the system or
     carries code taken out whole, so the session is never handed the internals to leave out, and its frame asks
     for nothing of the system on top of that.
  4. **The report is told once.** The ending fact names the room that heard a line of it, so the agent does not
     tell that room again.
  5. A fact with no report in it is an ending the agent tells by itself, from what the fact says.
  6. The agency decides which room the fact goes to — the room of the supervisor that ordered the item, up the
     chain of items it was raised from (its `docs/agent-control.md`, 「An item's change is told to its
     orderer」) — and queues it; the receiver delivers it by the rules standing
     when it is delivered.
- **Background runs.** A digest or person-book run that failed, a person-book sort that gave up on a room, a rule
  candidate awaiting approval, a paused digest, a voice briefing that failed, a job that died without reporting:
  each is voiced to the chat that would have heard it, from the code its unit sends. A failed voice run, and a
  dead job, say plainly that nothing more is coming.

## The voice briefing

A line of the default answerer's is heard as well as read: a notice turn of the owner's private chat is spoken as a
summary, and any agent's line can be read out whole from its chip. An answer the agent itself says aloud is no briefing
of these: it goes out as a voice note (「An answer said aloud」). The voice engine speaks and keeps every
briefing with its room and its line (its `docs/boundary-api.md`, 「Briefings」); the receiver decides when one is
spoken, and the session writes what is said.

- **The session writes the script the voice speaks, as written words.** Both the summary and the full reading
  are asked of the agent's own session as a spoken script: sentences a person would say aloud, with no markup,
  symbol, list or quotation, and names, acronyms, numbers, dates, times and units written as the line writes
  them. No gate stands between the session and the engine: the receiver hands the engine exactly the text the
  session wrote, its emotion line taken off, as the briefing's `text`, which the engine keeps as its transcript.
- **The session names the emotion a briefing is said with.** The answer to a summary or a reading opens on an
  emotion line: the label `[voice]` `emotion_label` names and one of the briefing emotions, `[voice]`
  `emotions` (`policy/chat-round.toml`), the set the voice engine speaks with and the face draws a face for.
  The marker's rule in the room's block lists them and asks for the one that fits what the line tells. The
  receiver takes that line off the answer and hands its emotion to the engine as the briefing's `emotion`, which
  the engine keeps on the briefing and its events, so a face listening shows it while the briefing plays.
- **A briefing whose emotion is unknown goes without one.** An answer with no emotion line is spoken all the
  same with no `emotion`; so is one whose emotion line names a word outside the set, that line still taken off;
  and so are a line's opening spoken in place of a summary and a line spoken as it stands. The engine keeps such a briefing with no
  emotion, and a face shows its default for it.
- **The engine derives the spoken words.** A briefing carries no spoken words of its own: the engine makes them
  from the text, the same words as a Korean reader says them aloud, and checks them before it speaks. Spoken
  words it cannot clear are a refusal of the briefing (`unconverted-run`), handled as every refusal of it is.

### The switches

- **One switch for what BMO speaks by itself,** the receiver's setting `voice_briefing`, `on` or `off`; a missing
  row reads as on. It governs the summaries of notice turns, which nobody asks for, and nothing else: a line's
  full reading is asked for by a person and is made and served whichever way the switch stands. It is read before
  a notice turn's summary is asked for, and again at the moment the summary is handed to the engine, so a script the
  session wrote while the switch moved goes nowhere.
- **Switching it off silences the summaries the engine already holds.** Right after the setting is written `off`,
  the receiver asks the engine to stop every summary of every room that has not ended (`POST /briefings/stop` with
  no room and the kind `summary`), so a summary waiting or speaking is stopped, the console's player falls silent
  and no summary taken before plays afterwards, while a full reading waiting or speaking goes on. A summary the
  engine took while the switch was being set off is stopped as well: the receiver reads the switch again once the
  engine has taken it, and asks for the stop where it now stands off. A stop that fails is logged and asked again
  at the policy's pace (`voice-briefing.toml`) for as long as the switch stays off, and the receiver asks for it
  once more at its own start while the switch stands off, so a stop a restart cut short is still made. Writing
  `off` where it already stood asks for the stop as well. Switching it back on starts nothing that was stopped: a
  stopped briefing never plays again.
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
  carried quiet instructions and no message of anyone; it sent a line, so a skip or a wait is not
  spoken; the switch is on; and its agent has a voice.
- **A line the service itself put in the room is nobody speaking.** Such a line stands under no sender, or under
  the room's own account on a chat service's road; the going-down notice (「System notices」) is one. A round
  whose new messages hold only such lines, as the first round after a restart may hold the going-down notice the
  backlog hands it (`docs/messages.md`, 「Backlog」), still qualifies; one message under any other sender makes
  the round a conversation.
- **The session writes the summary,** in a quiet instruction of its own, asked of the same session in one turn once the line has gone out and before the round lets go of the room. The instruction is marked and
  names the line as the message the session just sent (`summary`) and carries it no more: the session wrote it, a stage that writes
  nothing leaves it as it was, and the round holds the session until the summary is written, so no other room's turn
  comes between; what the marker asks for — the emotion line, then a one-line title saying what the briefing is about,
  in the agent's own words for this briefing, then one or two sentences of the line's gist as a spoken script,
  never the line read out, every notice of the turn covered in its newest state — stands in the room's block
  (`policy/chat-round.toml`, `[voice]`, `summary_marker`, `summary_rule`). Its answer goes to no road and stands
  in no room.
- **The answer is the script, and the turn sends nothing.** The voice of a line is that line's own briefing, which
  the console pairs with the line on its chip, never a message of its own. The marker's rule says tools are off
  for the turn and nothing is sent into any room from it: no talk, no send, no voice note. The receiver holds to
  it: while it carries the summary turn of an agent in a room, every send that agent asks into that room is
  refused `briefing-turn` (`docs/codes.md`, 「SendRefusal」), whatever its act, and nothing reaches the room. A send
  of the agent's into another room, a job's, and any send once the turn has ended are judged as every send is.
- **A session that gives no summary has its line's opening spoken:** the first sentence, or the first two where
  the first is short, with no title and no emotion. An answer of `<<SKIP>>`, an empty answer and a failed turn are each this case.
- **The receiver carries the facts to the engine:** `POST /briefings` of kind `summary`, in the agent's voice,
  naming the room and the message the line went out as, with the title where the session gave one, the
  emotion where it named one of the set, and whether it plays by itself as the second switch stands. The line already stands when the summary is asked for,
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
  Where there is none, or the last one failed or was stopped, it answers `making` at once and asks the line's agent, in one turn of its session in the line's room, for the whole line as a spoken script: the turn is marked and carries
  the line as data, and what the marker asks for stands in the room's block (`policy/chat-round.toml`, `[voice]`,
  `reading_marker`, `reading_rule`), the emotion line and then every figure, name and conclusion kept, then launches it with `POST /briefings` of kind
  `full`, with the emotion the script named; a session that gives no script, or one past its cache lifetime, which is never woken for it
  (`docs/unified-bmo.md`, 「Replacing the session」), has the line spoken as it stands. While the script is being written an
  ask answers `making` with no briefing yet; once launched, the answer names the briefing, whose sound the engine
  serves.
- **The reading turn sends nothing either.** Its marker's rule says tools are off for the turn and nothing is
  sent into any room from it, and while the receiver carries it, every send its agent asks into the line's room is
  refused `briefing-turn`, as during a summary turn.
- **It is kept on the line** by the engine, so a later ask answers `ready` at once. It goes to no road and no
  listener plays it by itself.
- **It asks the voice, never the switch.** A person asked for it, so it is launched with the whole switch off as
  with it on, and switching the briefing off leaves it waiting or speaking; an agent with no voice is refused
  `voiceless` (`docs/codes.md`, 「VoiceRefusal」).

### What a listener hears

- **The engine's event stream.** A listener reads `briefing.ended` on the engine's stream and plays a summary
  that is `done` and plays by itself; it shows rather than plays one it learns of more than two minutes after it
  ended. The sound is the engine's `GET /briefings/{briefingId}/sound`.
- **A briefing a unit other than the engine put in a room as a voice message** is told through the receiver's
  `POST /events` as a `briefing` event, stamped with whether it plays by itself as the second switch stands, and
  goes out on the receiver's stream to the same listeners.

## An answer said aloud

An agent asked to say something aloud in a room it answers says its round's answer in its own voice: the answer
goes out as a voice note, and the voice engine keeps it as the agent speaking as itself (its `docs/boundary-api.md`,
「Briefings」, the kind `speech`), which a listener showing the agent's face plays as it is said. Any other sound
an agent makes, a voice generated through the voice's own tool among them, is a file it sends, and is no speech.

- **The round asks it with `speak`,** an entry of the send tool (`docs/chat-mcp.md`), naming the emotion the answer
  is said with, one of `[voice]` `emotions` (`policy/chat-round.toml`), or none. It carries no words: it marks the
  answer the round writes, which the message turn writes and the instruction stages check as every answer is
  (`docs/instruction.md`), and is answered with the line `[voice]` `speak_marked` says. A later `speak` of the same
  round replaces the earlier mark, and a round that ends without sending leaves no mark on the next.
- **Only the agent's own round, in the room it answers, is marked.** `speak` asked anywhere else, from a shell, a
  job, another room or once the round has ended, is refused `speak-outside-round`; one naming an emotion outside the
  set is refused `speak-emotion`; one of an agent whose registration names no voice is refused `speak-voiceless`;
  and one where no voice engine is named is refused `speak-no-engine` (`docs/codes.md`, 「SendRefusal」). A voice
  briefing turn's refusal, `briefing-turn`, comes before every other.
- **A road that sends no voice note says so.** `speak` asks the room's road, as every send does, whether it performs
  `voice`, and is refused with the road's own sentence where it does not, so the round answers in writing. A room's
  block leaves the entry out where its road does not perform `voice` (`docs/chat-mcp.md`, 「One list in every room」).
- **The answer is spoken once it has passed its stages.** As the round's message goes out, the receiver hands the
  answer, as it stands then, to the engine with `POST /briefings` of kind `speech`: in the agent's voice, for the
  room, awaiting its line, with the emotion named, playing by itself, and keyed by the round, so a send tried again
  takes the same speech. It reads the speech at the pace `speech_read_every_ms` sets (`policy/voice-briefing.toml`)
  until it is done, and fetches its sound.
- **It goes out as a voice note with its words.** The note carries the answer as what it says, which a road shows or
  folds with the note, so the words stand in the room as well as the sound; the room's record keeps the message under
  the voice mark and the note's sound under it (`docs/room-files.md`, 「Message files」), and the speech is told the
  line it went out as (`POST /briefings/{briefingId}/line`).
- **An answer that could not be said goes out written.** Where the road answering does not perform `voice`, the
  engine refuses or fails the speech, or the speech has not ended within `speech_within_ms`, the answer goes out as
  text, as any answer does, the journal saying why; a speech the bound let go is stopped (`POST /briefings/stop`, the
  room and the kind `speech`).
- **The voice briefing's switches do not govern it.** Somebody asked for it, so it is said with the whole briefing
  switched off, and it plays by itself whatever the second switch says.

## Operating notices per chat

- **The restart notice is the owner's private room's.** The going-down line is a system notice the stopping
  receiver sends, a deployment's or an unannounced stop's (「System notices」). On the next start-up the owner's
  session is told it is back, with the deployment's purpose, and says so in the same turn as the deployment's
  outcome; the same turn or the one the deployment briefing opens also briefs him on what went live (「What each
  path says」).
- **No feature announcements.** A deployment tells no group room what the agent gained, and no room hears the
  deployment briefing; the owner's own chat hears the going down, the coming back and what went live.
- **The restart notice can be switched off per chat:** going down, back up.
- **Where it is switched:** inside the chat with `notice:restart:off` / `notice:restart:on` (or the words of the
  policy), which refuses a value other than on or off before it writes.
- **It is on unless switched off;** the setting is stored per chat in the receiver's database.
- **The infrastructure room** is switched off for it, and hears nothing of the agent's own state whatever the
  switch says (`docs/rooms.md`, 「The infrastructure room」).

## A failure says why

- **Every failure order carries its cause** as a plain sentence under `cause`, beside what failed and what
  follows. The agent reads it and says it in its own words, so the owner hears why and not only that.
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
- **It never makes a round a conversation.** Handed to a round as a new message, it counts as nobody speaking,
  so the notice turn it rides in is still summarised (「The summary of a notice turn」).

## Lines that stay fixed

Each is sent as a code with its fields, because no session is to say it in its own words: no session can say it
at that moment, or the line must reach the room as itself. Its words in the room's language are rendered by the
web unit's language packs (`POST /words/render`, `docs/boundary-api.md`, 「The web unit」).

- **A person never reads a code.** Where the web unit cannot render a line, or its pack holds no sentence for the
  code, the line falls back to the plain sentence the receiver's policy keeps for that code in the rooms' language
  (`policy/fixed-lines.toml`), its fields filled in as words: a list as its members, a draw's result as its values.
  Where the policy holds no sentence for the code in that language, the line is dropped, nothing reaches the room,
  and the journal says which line and why. A code and its fields, or any JSON, never stand in a room.
- **A fixed line is the service's own, never an agent's.** No send asks for one: a session's words reach a room
  only as its round's message, which its instruction stages check (`docs/instruction.md`).
- **A line a chat service hands over is kept as a fixed line.** A person coming into a room or going out of it, said
  by the platform as a line (bmo-chat's `docs/contract.md`, 「Lines」), is kept with the kind `fixed-line` and its
  code and fields as they came, beside the plain sentence the service wrote for it (`docs/api.md`, 「The chat
  services」).
- **A fixed line the receiver says into a room itself is kept as one.** The room's record keeps its words as they
  were rendered, with the kind `fixed-line` and the line it was said from, its code and its fields, so a screen
  reading the room knows what the line is without matching its words (`docs/api.md`, 「Messages」). This holds for
  each line in the table below the receiver says on its own: the rest, a broken run's last resort, a shut room, a
  memory being tidied and a room's members by account. A draw's line goes out as the agent's send and the
  going-down notices as system notices under the service's mark (「System notices」), so neither carries the kind.

| Line | Why it stays fixed |
| --- | --- |
| `rest-begins` (a usage limit begins) | No model can answer while the limit holds. |
| `provider-down` (the provider stopped answering) | No model can answer during the outage. |
| `run-broken`, the last resort when a run broke and the agent cannot answer the order about it | No session is able to say it; it is composed out of what the broken run left. |
| `chat-denied` | The room is being shut; its session is not woken for it. |
| `going-down`, `going-down-for-update` (the system notices above) | The service is stopping. |
| `account-let-in`, `accounts-let-in`, `unit-let-in`, `account-left`, `account-taken-out` (an account let into a room, several let in at once, an organisation unit's members let in, an account leaving or taken out, each naming the accounts by their keys, `docs/receiver.md`, 「Console rooms and accounts」) | It records who did it, as it was done, for every member to read; no session is woken for it. |
| `draw` (a fair draw's room line) | A gate that rewrote it would make the draw impossible to check (`docs/random-draw.md`). |
| A login's end (「What each path says」) | The agency composed it from its policy, and the model whose login ended may be the one that would say it. |
