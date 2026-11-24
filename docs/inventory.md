# Inventory: what the single repository held, and where each part stands

Old members are read at the single repository's pinned commit; the paths there are under `bot/bmo-server/src/`
unless another root is named. Each row gives the member, what stands in this repository for it, and the task of
item #1737 that carried it. The sets are the ones the work order numbered S1 to S13.

## S1 — the receiver's judgement, its roads and a room's record

| Old member | Stands here as | Carried by |
|---|---|---|
| `receiver/mod.rs` | `src/receiver/mod.rs` | build-6 (Rooms, roads and intake) |
| `receiver/commands.rs` | `src/receiver/commands.rs`, its phrases read from `policy/commands.toml` | build-6 |
| `receiver/rows.rs` | `src/receiver/rows.rs` | build-6 |
| `receiver/speech.rs` | `src/receiver/speech.rs` | build-6 |
| `receiver/notices.rs` | `src/receiver/notices.rs` | build-6 |
| `receiver/left_board.rs` | `src/receiver/left_board.rs` | build-6 |
| `receiver/roads.rs` | not carried: the road a room answers on is asked of the chat services, and the record of it stands in `src/roads/record.rs` | build-6 |
| `providers/mod.rs`, `providers/declared.rs`, `providers/record.rs` | `src/roads/mod.rs`, `src/roads/declared.rs`, `src/roads/record.rs` | build-6 |
| `control/receive.rs` | the judgement in `src/receiver/mod.rs`; the four hand-overs in `src/receiver/handover.rs`, served through `bmo-chat-client` | build-6 |
| `room_record/mod.rs`, `room_record/setting.rs`, `room_record/forget.rs` | `src/room_record/` of the same names | build-6 |
| `room_record/tests.rs` | `tests/integration/room_record.rs` | build-6 |
| the colocated test modules of `receiver/commands`, `receiver/notices`, `room_record/mod`, `room_record/setting` | `tests/integration/receiver/commands.rs`, `tests/integration/receiver/notices.rs` | build-6 |
| `tests/integration/receiver.rs`, `receiver/owner_road.rs` | of the same names under `tests/integration/` | build-6 |
| `providers/telegram/files.rs`, `providers/telegram/pictures.rs`, `verbs/files.rs` | `src/receiver/files.rs`, `src/receiver/pictures.rs`, the numbers and names in `policy/roads.toml` | build-18 (Keep handed files as the documents state) |
| the file rules' tests | `tests/integration/receiver/files.rs`, one per rule | build-18 |

## S2 — the sends, the owner's words and the room-calls crate

| Old member | Stands here as | Carried by |
|---|---|---|
| `control/sends.rs` | `src/routes/sends.rs` | build-11 (Sends and room switches) |
| `control/say.rs` | `src/routes/say.rs` | build-11 |
| `control/as_owner.rs` | `src/routes/as_owner.rs` | build-11 |
| `control/owner_words.rs` | `src/routes/owner_words.rs` | build-11 |
| `verbs/sends.rs`, `messages.rs`, `asked_sends.rs`, `turn.rs`, `room_switches.rs`, `room_files.rs`, `room_rules.rs`, `draws.rs`, `photo_repair.rs` | `src/verbs/` of the same names; `room_rules` asks the web port instead of the old tables | build-11 |
| `chat/room-acts`: `lib`, `accounts`, `acted_on`, `bot`, `messages`, `mode`, `procedures`, `promises`, `reading`, `sends`, `turn` | `crates/room-calls/` of the same names but `acted_on`, which no entry takes, with its own manifest | build-11 |
| `chat/room-acts/launches.rs` | not here: it starts runs, which the services whose runs they are own | S4 |
| `verbs_messages.rs`, `verbs_draws.rs`, `receiver/owner_sends.rs`, `receiver/owner_words.rs` under the old `tests/integration/` | of the same names under `tests/integration/` | build-11 |
| the colocated test modules of `verbs/turn`, `verbs/draws`, `verbs/room_files` | `tests/turn.rs`, `tests/draws.rs`, `tests/room_files.rs` | build-11 |
| the colocated test modules of `chat/room-acts` | `crates/room-calls/tests/` for `bot`, `promises`, `reading`, `procedures`, `sends`; the `accounts` and `turn` modules are not carried, as they lean on the archive and the sidecar machinery the split cuts | build-11 |

## S3 — the send tool

| Old member | Stands here as | Carried by |
|---|---|---|
| `tools/send/tool.toml`, `Cargo.toml`, `src/lib.rs`, `src/main.rs` | of the same names under `tools/send/` | build-15 (Port the send tool) |
| its seven schemas: `say`, `photo`, `remind`, `react`, `pin`, `sticker`, `random_draw` | the JSON files under `tools/send/` of every one but `remind`, which stands nowhere: no reminder is set through the receiver | build-15 |
| — | `crates/tool-call/`, the call protocol the tool speaks: the S3 enumeration counted only the tool's own files, and the old shared tool crate is a forbidden dependency, so the protocol stands as a crate of this repository | build-15 |
| — | `tests/send.rs`, one schema and route test per entry, and the manifest held to the executable | build-15 |

## S4 — what did not come, and the unit that owns it

| Old member | Count | Owner |
|---|---|---|
| `chat/stand_in.rs`, `stand_in.rs` | 2 | none: a supervisor stands on one road, and the rest's gate asks the agency whether that road answers |
| `door/` | 17 | bmo-agency |
| `lines/` | 17 | bmo-agency |
| `worker_session/` | 4 | bmo-agency |
| `run_log/` | 6 | bmo-agency |
| the added supervisors' folder: guard, handoff, overtaken, run, sessions | 5 | bmo-agency |
| `conversational.rs`, `host_run.rs`, `doctor.rs` | 3 | bmo-agency |
| `watches/mod.rs`, `live_runs.rs`, `links.rs` | 3 | bmo-agency |
| `gates/` | 22 | bmo-gate |
| `reference/` | 7 | bmo-gate |
| `watches/attachments.rs`, `watches/laya.rs` | 2 | bmo-gate |
| `keepalive/` | 13 | bmo-cache-warmer and bmo-agency |
| `editors/` | 26 | bmo-brain |
| `schedules/reminders.rs`, `schedules/watch_sweep.rs` | 2 | bmo-brain |
| `words/` | 2 | bmo-job |
| `schedules/launches.rs`, `schedules/rows.rs` | 2 | bmo-job |
| `schedules/full_suite.rs` | 1 | bmo-testbed |
| `schedules/news_brief.rs` | 1 | bmo-voice |
| `schedules/claude_usage.rs`, `codex_usage.rs`, `usage_windows.rs` | 3 | bmo-cache-siren |
| `schedules/prunes.rs` | 1 | the owners of the pruned tables |
| `schedules/parity.rs` | 1 | goes with the loops it tests |
| `schedules/mod.rs` | 1 | none: the receiver keeps no loop of its own |
| `watches/machine.rs`, `watches/hardware/` | 13 | bmo-node |
| `word_chain/`, `control/word_chain`, `bot/word-chain` | 3 | the word-game unit |
| `providers/telegram/` | 10 | the provider repositories |
| `providers/teams/` | 5 | the provider repositories |
| `providers/discord.rs`, `providers/kakao.rs`, `providers/numbers.rs` | 3 | the provider repositories |
| `bot/bot/roads.ts`, `bot/bot/receiver-post.ts` | 2 | the provider repositories |
| `control/telegram_send.rs`, `control/teams_send.rs` | 2 | the provider repositories |
| `providers/console.rs` | 1 | bmo-web |
| `grants.rs`, `users.rs`, `phones.rs`, `checks.rs`, `routes.rs`, `rules.rs`, `lib.rs`, `bmo.sz` | 8 | bmo-web |
| `data_move.rs` | 1 | stays with the old repository for the switch-over's data move |
| `control/runs.rs`, `chat/room-acts/launches.rs` | 2 | the services whose runs they are |
| the other `control/` files | 71 | other units |
| the other `verbs/` files | 203 | other units |

## S5 — the store's tables

| Member | Stands here as | Carried by |
|---|---|---|
| `messages`, `instructions`, `bmo_rooms`, `bmo_room_contexts`, `bmo_session_rooms`, `chat_notices`, `chat_reads`, `chat_titles`, `collected_items`, `delegate_chats`, `room_answerers`, `tool_chats`, `reload_sent`, `photo_chats`, `say_relays`, `muted_members`, `random_draws`, `random_draw_rooms`, `provider_rooms`, `provider_users`, `provider_private_rooms`, `provider_owner_users`, `avatars`, `allowed_chats`, `reload_pending` | one migration, `src/store/migration/m20261017_000001_receiver.rs`, with the settings table beside them | build-4 (Stand up the store) |
| `message_number_marks` | `src/store/migration/m20261017_000002_record_numbers.rs`: the room's own record numbers with it, so it is this repository's rather than a provider's | build-6 |
| `room_windows` | `src/store/migration/m20261017_000003_room_windows.rs`; the wake reads and writes it, and the receiver alone keeps it | build-7 (Wake decisions) |
| `run_records` | `src/store/migration/m20261017_000001_receiver.rs`: the conversations write what a turn recorded, and the shape is the pinned archive's unchanged | build-8 (Queue, window and prompt) |
| `people_reads` | `src/store/migration/m20261017_000004_people_reads.rs`: the console's reads of a room's people | build-12 (Serve the owed contracts) |
| the settings keys the receiver writes | `src/store/settings.rs`, which holds no key name; `docs/store.md` lists them all | build-4 |
| the room folders under the data root | `src/data_root.rs`: `collected/`, `images/`, `outbox/` | build-4 |
| the scratch database the tests use | `tests/integration/common/scratch.rs` and the `Scratch` guard in `tests/integration/common/mod.rs`, migrated by this repository's own migrator | build-17 (Carry the scratch database helper) |

## S6 — the policy the receiver reads

| Member | Stands here as | Carried by |
|---|---|---|
| own: `chat-round`, `roads`, `wake`, `talk`, `restart`, `answer-reading`, `handover`, `files`, `code-review`, `queue` | `policy/` of the same names | build-3 (Copy the receiver's policy) |
| shared, the keys the receiver acts on: `chat-health`, `chat-line`, `rest`, `resident-turn`, `run-lines`, `roles`, `keepalive`, `providers` | `policy/` of the same names, the old `providers/` folder become one `providers.toml` | build-3 |
| `commands.toml` | `policy/commands.toml`: the phrases the owner's commands are matched by, the polite endings and the option suffixes | build-2 (Turn room words into codes) |
| the loader | `crates/receiver-policy`, one module per file, held to the folder by its load test | build-3 |

## S7 — the language-pack modules the old code sent words from

Each became a code with fields in `crates/receiver-codes`, or a line an agent reads in `policy/`, by build-2.

| Old module | Became |
|---|---|
| `agent_catalogue` | the roster line in `policy/chat-line.toml`, written with the `{owner}` and `{agent}` marks |
| `chat_health` | `FixedLine::ProviderDown`, the turn openings of kind `fold` and `fold-deferred` |
| `chat_turn` | `RunTitle::Chat`, `RunTitle::Fold` |
| the added supervisors' words | `SupervisorRefusal`, `SupervisorFact::RoomTakenBack` |
| `draws` | `FixedLine::Draw` with its line, asked and result fields |
| `owner_sends` | `OwnerSendRefusal`, `FileUnsent` with a per-file reason, sizes in bytes |
| `providers` | `SkipReason::ProviderOff` |
| `roads` | `FixedLine::GoingDown`, `FixedLine::GoingDownForUpdate`; the photo and voice marks stay as data in `policy/roads.toml` |
| `room_removal` | `RoomRemovalRefusal`, `RoomRemovalWarning` (which also takes `private`) |
| `sends` | `SendRefusal`, the verbs named as the chat protocol names a capability |
| `sessions` | the `SessionReason` kind; the transcript lines are English the agent reads, so they moved to the policy |
| `talk` | `TalkRefusal`, with bmo-job's `resting`, `limit` and `outage` at bmo-job's statuses |
| `voice` | `VoiceRefusal`, the briefing title as `RunTitle::VoiceBriefing` |
| `word_chain` | not taken: it belongs to the launches the room-calls crate does not carry |
| `control_answers`, `draws` outside S7 | `SupervisorRefusal::Action`, read by the supervisor routes |

## S8 — the old files holding Korean literals

| Old file | How its Korean left the code | Carried by |
|---|---|---|
| `receiver/commands.rs` | `BOT_ADDRESSES` became the deployment's words for the agent, longest first, and the phrases and endings are read from `policy/commands.toml` | build-2, copied by build-6 |
| `receiver/wake/question.rs` | the option suffix is read from `policy/commands.toml`; the test options became English | build-2, copied by build-7 |
| `control/turn_openings.rs` | the Korean refusal became an English one; the test strings are neutral | build-2, carried by build-13 |
| `text.rs` | the Hangul test input became non-Hangul | build-2, carried by build-8 |
| `events/relay.rs` | the Hangul test input became non-Hangul | build-2, carried by build-13 |
| `control/events.rs` | the face names are the closed emotions list in `policy/chat-round.toml`, kept as data | build-2, carried by build-13 |
| `receiver/notices.rs` | the agent name became the delegate's registered label as a parameter | build-2, carried by build-6 |

## S9 — the tests the old repository held for this code

| Old test | Stands here as | Carried by |
|---|---|---|
| `tests/integration/receiver.rs`, `receiver/owner_road.rs`, `room_record.rs` | of the same names under `tests/integration/` | build-6 |
| `tests/integration/receiver/wake.rs`, the colocated `wake/question` and `wake/rest` modules | `tests/integration/wake.rs`, `tests/integration/wake/question.rs`, `tests/integration/wake/rest.rs` | build-7 |
| the colocated modules of `chat/conversation`, `mid_turn`, `window`, `shape`, `room`, `context`, `records`, `form`, `text`, `dashes`, `js`, `web_address` | `tests/` of those names | build-8 |
| `tests/integration/private_thread.rs` | of the same name | build-8 |
| the colocated modules of `chat/ask`, `progress`, `again`, and the promises reading | `tests/ask.rs`, `tests/progress.rs`, `tests/stages.rs`, `tests/promises.rs` | build-9 |
| `tests/integration/stop_notice.rs` | of the same name | build-9 |
| `receiver/talk.rs` and `chat/talk`'s colocated module | `tests/integration/talks.rs`, `tests/talk.rs` | build-10 |
| `tests/integration/verbs_messages.rs`, `verbs_draws.rs`, `receiver/owner_sends.rs`, `receiver/owner_words.rs` | of the same names | build-11 |
| the colocated modules of `verbs/turn`, `verbs/draws`, `verbs/room_files`, `chat/room-acts` | `tests/turn.rs`, `tests/draws.rs`, `tests/room_files.rs`, `crates/room-calls/tests/` | build-11 |
| `tests/integration/receiver/job_queue.rs`, `receiver/worker_settings.rs`, the `common/queue` helper, and a contract test per sibling document | `tests/integration/contracts.rs` and `tests/integration/common/queue.rs` | build-12 |
| `tests/integration/room_removal.rs`, `receiver/voice.rs`, `tests/fixtures/event-parity.json`, the colocated modules of `events/mod`, `events/relay`, `control/events`, `control/turn_openings` | `tests/integration/room_removal.rs`, `tests/integration/voice.rs`, `tests/fixtures/event-parity.json`, `tests/events.rs`, `tests/relay.rs`, `tests/parity.rs`, `tests/openings.rs` | build-13 |
| `tests/integration/control.rs` (the receiver's routes only), `src/bin/bmo/tests.rs` | `tests/integration/routes.rs`, `tests/cli.rs` | build-14 |
| the send tool's entry tests | `tools/send/tests/send.rs` | build-15 |
| the provider tests, the TS control and bot tests, the testbed's own tests, the word-chain fixtures | not carried: they belong to the provider repositories, the other units and the word-game unit | S4 |

## S10 — the documents this repository carries

`docs/boundary-api.md` and the area documents, written to the present tense and free of personal and machine
detail, by build-1 (Write the boundary document): `receiver.md`, `bmo-window.md`, `wake.md`,
`voiced-notices.md`, `answer-checks.md`, `random-draw.md`, `messages.md`, `room-files.md`,
`pictures.md`, `chat-mcp.md`, `rooms.md`, `reminders.md` (delivery only), `unified-bmo.md`, and `policy.md`.
`docs/codes.md` came with build-2, `docs/store.md` with build-4, `docs/wording.md` with build-16.

Not carried: `chat-providers.md` (bmo-chat), `gates.md` and `output-gate.md` (bmo-gate),
`provider-presence.md` (bmo-agency), `telegram-formatting.md`, `teams.md`, `kakao*.md`, `discord.md` (the
provider repositories).

## S11 — the routes served

The whole list, with fields and codes, is `docs/boundary-api.md` 「Routes served」. Each group was stated by
build-1 and implemented by the task named here.

| Group | Count | Implemented by |
|---|---|---|
| the brain's reads of rooms, messages, read marks, people, promises, roads and the quiet instruction | 18 | build-12 (Serve the owed contracts) |
| bmo-job's instructions, notices, rooms, messages, talks and room holders | 11 | build-12; `POST /talks` with build-10 |
| the chat protocol's hand-overs | 4 | build-6, through `bmo-chat-client` |
| the cache warmer's ping hook | 1 | build-12 |
| the agency's hooks answered | 3 | build-12 |
| the receiver's own: sends, the owner's words, say, events, live, health, room removal, openings, voice, supervisors | — | build-6, build-10, build-11, build-13 |
| the console's reads, under `/console/` | 8 | build-12 |

## S12 — the ports out

Stated by build-1, carried as a trait and an HTTP client per unit by build-5 (Open the ports): the agency,
bmo-job, the gate, the web unit, the cache warmer and the chat services through `bmo-chat-client`. The brain has
no port, since the receiver makes no call on it, and the word chain needs none: it reads the event stream and
speaks through the sends.

## S13 — the personal data stripped as the code came in

| What stood in the old tree | How it stands | Carried by |
|---|---|---|
| the handle of the service's own account, and the Korean name variants in `BOT_ADDRESSES` | the agent's registered label and call names (`aliases`), read from the agency's `GET /supervisors`, with the chat service's declared handle | build-2, build-6 |
| the agent's name written into a notice and into the delegate's words | the delegate's registered label as a parameter, from the configuration | build-2, build-14 |
| machine names, account names, domain names and private network addresses | none stands: the configuration names addresses, and the tests use neutral names such as `agent-a`, `road-a` and `room-a` | build-0's names check, each copy task as it copied |
| real chat ids in test fixtures | the test's own fixed ids | build-6 onward |
| home paths in command fixtures | neutral words (`bin/bmo`, `D=bin`, `cd data`) | build-8 |
| the old fallback infrastructure room | the configuration's `infra_room` | build-6 |

## Members no set named

| Member | Why it stands here | Carried by |
|---|---|---|
| `crates/tool-call/` | the tool host's call protocol, which the S3 enumeration counted only as the tool's own files | build-15 |
| `src/receiver/labels.rs` | the label drawn into a picture's own band, which the file rules of build-18 need | build-18 |
| `src/receiver/reads.rs` | the room reads another unit asks of them, which the owed contracts of build-12 answer | build-12 |
| `src/routes/hooks.rs`, `src/routes/mod.rs` | the token check, the refusal shape and the route table every route shares | build-10, reworked by build-14 |
| `src/language.rs` | none: the language an answer is written in is the agent's own judgement, never read off a message's prose (`docs/answer-checks.md`) | build-9 |
| `src/js.rs`, `src/chat/promises.rs` | the text readings and the answer readings; both moved into `crates/room-calls` and are re-exported from the root package | build-8, build-11 |
