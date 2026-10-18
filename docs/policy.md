# The policy

The receiver's fixed numbers and words, as TOML files under `policy/`, read by the crate `receiver-policy` and
built into the binary: one module to a file, each file read once on first use into its type, a file that does not
read as its type failing the crate's load test. A line an agent reads is English, and its `{name}` marks say what
each line carries; `fill_marks` fills them. Korean words the code matches on (the command phrases, the words the
answer checks read promises and rules by, the marks a picture and a voice are kept under) are data and stand here,
never in code. No sentence a person reads in a room stands here: those are codes the agent says, or the few fixed
lines the web unit's language packs render (`docs/codes.md`).

## The receiver's own files

| File | What it holds | Read by |
|---|---|---|
| `chat-round.toml` | every fixed line a round's prompt is written in: the scope and header lines, the blocks the room's words stand in, the notes each message carries, the lines that close the prompt (the language line takes `{language}`), the push lines, the typed-line lead, the console instruction, the relay of another room's answer, a person's own thread; how much of the closing words of the message a turn follows on from, in characters, the gate is handed to choose a turn's reference; the voice briefing's lines and the closed list of emotions a briefing may be spoken with; and the clocks: the queue's gathering and the wait, the conversation window, the instruction pass's tick and cold-wake gap, the restart's limit, the backlog's age and settling, the progress gap, the push's look and its gap after a refusal, the typing renewal, the voice run's cap | the conversations, the window, the instruction pass |
| `roads.toml` | the room's own record (its key, label, the verbs it keeps, what a turn answering into it is told, the words an agent's own file, voice and audio file are kept under in it, and its dialect), the marks a picture and a voice are kept under, which the receiver matches on, how a picture a message carried is filed (the size past which it is brought down, its longest side, its JPEG quality), and what a file handed over with no name is named (a sound, a video, and the extension each type implies) | the sends, the record, the picture repair, the files a message carried |
| `wake.toml` | `file_lead_seconds` and `file_lead_messages`: how far back a person's file is read as leading up to their words | the wake (`docs/room-files.md`, 「Who a file wakes」) |
| `talk.toml` | the lines a console talk's turn is composed in, the item states its head names, and how many earlier turns it restates and how much of each | the talks |
| `restart.toml` | the back notice a restart leaves the owner's session: its facts, how long a deployment's record speaks for a stop, how long the notice rides and lives | the restart |
| `second-asks.toml` | the words each answer check reads by and every errand a second asking sends | the answer checks (`docs/answer-checks.md`) |
| `consult.toml` | the consult room's setting key, the lines of what an agent is shown of other agents' work, how many items it shows, the line a consult call answers its agent with, and the lines the consult verbs answer with | the consult block, `POST /consult/say` and the consult verbs (`docs/consult.md`) |
| `handover.toml` | how much of a closed session's transcript is kept, the lines a transcript is written in, the blocks of a round that are the conversation, and the notes a room's new session opens with | the session replacement |
| `files.toml` | the largest file the console may attach, in bytes, and the name a file with none goes by | `POST /say-file` |
| `code-review.toml` | the lines a code review's talk is composed in, the places a review is pinned to, how much of the reviewed lines a turn carries | the talks |
| `queue.toml` | how long the owner may hold a job's question clock stopped from the console | the owner's answer to a job's question |
| `commands.toml` | the owner's in-room command phrases (each command's plain form first), the polite endings a command may carry, and the suffix an answer to a job's question may put after an option's number | the receiver's commands, a question's answer |

## Keys the receiver takes from files the agency also holds

The agency keeps its own copy of each of these files for its own side of a turn; the receiver holds only the keys
it acts on itself, each named here.

| File | Keys | What the receiver does with them |
|---|---|---|
| `chat-health.toml` | `outage_lifted`, `rest_lifted`, `stopped`, `failed` (each a notice's `fact`, `intent`, `follow_up`, `run_log`, `kind`, `ttl_minutes`), `order` (`what`, `check_seconds`), `fold` (`timeout_minutes`, `prompt`, `fact`, `intent`, `kind`, `ttl_minutes`), `touch` (`note`, `label`, `rank`, `idle_ms`, `lifetime_ms`, `margin_ms`, `ping_bound_ms`, `warm_by_default`, `failure_touches`, `honours_idle_override`) | the facts of a failed or stopped round, the lifting of an outage and a rest, the failure order and its minute, the fold, the touch's note and the chat touch target as the receiver declares it |
| `chat-line.toml` | `persona` (`header`, `tone_header`, `room_header`), `room` (the block's lines, its `mark`, the `roster` of the owner's private chat and its `kinds`), `capabilities`, `downgrade`, `fallback`, `rules` | the room's block, its capability lines, the downgrade and fallback facts, the rules and grants in force |
| `rest.toml` | `threshold`, `weekly_threshold`, `fallback_seconds`, `outage_seconds`, `lift_check_seconds` | when the rest's gate closes the rooms, how long an outage's rest lasts and when its lifting is checked |
| `resident-turn.toml` | `round_lead`, `fold_ratio` | the lead a round's turn opens with, when a room's session is folded |
| `companion-runner.toml` | `skip`, `echoed_line_chars`, `engine_wait_ms`, `answer_stage`, `turn_stage`, `no_note`, `limits`, `words`, `blocks`, `runs.turn`, `log` (`start`, `skipped`, `answered`, `cleared`, `system_changed`, `carried`, `waits`, `waits_item`), `answer`, `refused`, `cut` (`instruction`, `kind`, `sort`) | a companion's turn as the receiver composes it and the checks its answer passes; the line its room hears when a turn is cut is the fixed line `companion-cut` |
| `run-lines.toml` | each line's `keeps_session`, and the chat line's `composes_typed_lines` | whether a line's turns go on one session, whose typed lines are composed |
| `roles.toml` | `send`, and each role's `name` and `bundles` | whether the asker's role gives sending |
| `keepalive.toml` | `prompt` | the touch's prompt |
| `companions.toml` | `name_pattern`, `added_icon` | what a companion added on the console may be named, the mark beside it |
| `providers.toml` | each provider's `turns` and `handover`, Claude Code's `keepalive_idle_minutes`, and the `own_words` of the providers whose programs write words of their own into a turn | how a turn goes in and is read back, how a full session is handed over, when the touch is due, which words of an answer are the program's |

## What is configuration rather than policy

A value that names a room, an agent or a file of the deployment is the deployment's, read from the receiver's
configuration (`docs/boundary-api.md`, 「Configuration」), never written here: the rooms the deployment seeds for
companions (which a removal refuses), the tone layer a room takes where its own persona names none, the sticker
set the agents send from, the infrastructure room and the consult room's seed.

## Names and languages in the lines

No line of the policy names an agent, the owner or a language. Where a line an agent reads names the owner, it
carries the mark `{owner}`, filled with the owner's name as the configuration gives it; where it names an agent,
`{agent}`, filled with the agent's registered label from the agency; where it names the language to answer in,
`{language}`, the room's language. So the talks' lines (`talk.toml`, `code-review.toml`), the roster of the owner's
private chat (`chat-line.toml`, `room.roster`), the lines a transcript is written in (`handover.toml`,
`transcript`) and the language line of a round (`chat-round.toml`, `closing.language_named`) carry these marks.
