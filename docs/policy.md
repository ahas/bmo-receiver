# The policy

The receiver's fixed numbers and words, as TOML files under `policy/`, read by its policy loader and built into
the binary. A line an agent reads is English, and its `{name}` marks say what each line carries. Korean words the
code matches on (the command phrases, the words the answer checks read promises and rules by) are data and stand
here, never in code. No sentence a person reads in a room stands here: those are codes the agent says, or the few
fixed lines the web unit's language packs render (`docs/voiced-notices.md`).

## The receiver's own files

| File | What it holds | Read by |
|---|---|---|
| `chat-round.toml` | every fixed line a round's prompt is written in: the scope and header lines, the blocks the room's words stand in, the notes each message carries, the lines that close the prompt, the push lines, the typed-line lead; and the round's numbers: the backlog's age and settling, the conversation window, the cold-wake gap, the restart's limit, the progress gap, the typing renewal, the voice run's cap | the conversations, the window, the instruction pass |
| `roads.toml` | the line a shut room is told, the console's unnumbered mark, the console's dialect, the picture mark, the room's record keys, the refusal to an agent that may not speak in a room | the receiver's judgement, the sends |
| `wake.toml` | `file_lead_seconds` and `file_lead_messages`: how far back a person's file is read as leading up to their words | the wake (`docs/room-files.md`, 「Who a file wakes」) |
| `talk.toml` | the lines a console talk's turn is composed in, and how many earlier turns it restates and how much of each | the talks |
| `restart.toml` | the back notice a restart leaves the owner's session: its fact, how long a deployment's record speaks for a stop, how long the notice rides and lives | the restart |
| `second-asks.toml` | the words each answer check reads by and every errand a second asking sends | the answer checks (`docs/answer-checks.md`) |
| `consult.toml` | the consult room's setting key, the lines of what an agent is shown of other agents' work, and how many items it shows | the consult block (`docs/consult.md`) |
| `handover.toml` | how much of a closed session's transcript the note's turn carries | the session replacement |
| `files.toml` | the largest file the console may attach, in bytes | `POST /say-file` |
| `code-review.toml` | the lines a code review's talk is composed in, how much of the reviewed lines a turn carries | the talks |
| `queue.toml` | how long the owner may hold a job's question clock stopped from the console | the owner's answer to a job's question |
| `commands.toml` | the owner's in-room command phrases and the polite endings a command may carry, the option-number suffix a question's answer may carry, and the face names an avatar is drawn with | the receiver's commands, a question's answer, the event stream |

## Keys the receiver takes from files the agency also holds

The agency keeps its own copy of each of these files for its own side of a turn; the receiver holds only the keys
it acts on itself, each named here.

| File | Keys | What the receiver does with them |
|---|---|---|
| `chat-health.toml` | `failed`, `stopped`, `fold` (`prompt`, `timeout_minutes`), `order` (`check_seconds`, `what`), `rest_lifted`, `outage_lifted`, `touch.note` | the facts of a failed or stopped round, the fold's word and its bound, the failure order's minute, the rest's and the outage's lifting, the touch's note |
| `chat-line.toml` | `capabilities` (`document` among them), `downgrade`, `fallback`, `persona.header`, `room`, `rules.room_rules` | the room block's capability lines, the downgrade and fallback facts, the persona's header, the room's rules heading |
| `rest.toml` | `threshold`, `weekly_threshold`, `fallback_seconds`, `lift_check_seconds`, `resting` | when the rest's gate closes the rooms and when its lifting is checked |
| `resident-turn.toml` | `fold_ratio`, `round_lead` | when a room's session is folded, the lead a round's turn opens with |
| `companion-runner.toml` | `answer` (`verdict`, `verdict_chars`), `answer_stage`, `turn_stage`, `blocks`, `cut` (`kind`, `mark`, `notice`, `sort`), `echoed_line_chars`, `engine_wait_ms`, `limits`, `log`, `no_note`, `refused` (`echoed`, `empty`), `runs.turn`, `skip`, `words` (`owner_mark`, `quiet_head`) | a companion's turn as the receiver composes it and the checks its answer passes |
| `run-lines.toml` | `composes_typed_lines`, `line` | which agent's typed lines are composed, and the chat line's name |
| `roles.toml` | `bundles_of`, `send` | whether the asker's role gives sending |
| `keepalive.toml` | `prompt` | the touch's prompt |
| `providers/*.toml` | the providers' declared conversation limits, read where a companion's turn is watched | a companion's turn |
