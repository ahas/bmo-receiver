# The store

The receiver keeps its rooms in a PostgreSQL database of its own and their files in folders of its own under the
data root. No other unit reads or writes either: a unit that needs a room's rows asks the receiver
(`docs/boundary-api.md`), and the receiver reaches another unit's tables only through that unit's routes.

## The database

- **The address is the configuration's** (`docs/boundary-api.md`, 「Configuration」). The receiver connects
  through a pool that never hands out a connection opened before the database's shape last moved under the
  process, and runs every migration it has not run yet before it serves.
- **Migrations are run by SeaORM's migrator,** which keeps the ones it ran in its own table, `seaql_migrations`,
  beside the receiver's. Each migration states its tables in SQL and takes them down in the reverse order.
- **Every statement goes through the store's three helpers,** one row, every row, or what a statement wrote.
  Each binds every number at its widest, since a text the driver prepared with one width refuses a value of
  another on the same connection. A statement prepared before a migration moved a table under it is refused by
  the server; the helper marks the shape moved and runs it once more on a connection that prepares it afresh.
  Inside a transaction that second run can only meet the aborted transaction, so the first error stands.
- **A time is a `bigint` of epoch milliseconds,** as the API's times are, and a duration is a `bigint` of the unit
  its column names (a message's `answer_seconds` is seconds). A database holding times in Unix seconds has them
  multiplied in place as its migrations run. A room, a person and a message are signed 64-bit numbers, as the chat
  protocol numbers them (bmo-chat's `docs/protocol.md`, 「Numbers」).
- **Every table carries a `rowid`,** the row's own number where its key is one number and an identity of its
  own otherwise, so a row can be named in one column whatever its key.

## The tables

The table shapes are the ones the receiver's rows have always had. An instruction names the agent whose session
takes it outright: the table holds no default agent.

| Table | One row is | Key |
|---|---|---|
| `messages` | a message said in a room, by a person or an agent: its sender and name, time, words, stored picture, the message it answers, how long its answer took, the round that answered it, its album, the chat service it came by, and — on a send the receiver archived itself — the agent whose words it holds and the voice it was sent in, both null on a message a provider handed over; its kind where it is a sticker, and when it was edited, deleted and pinned on the record; on a voice note, what bmo-ear heard, how long it ran and whether it could be heard; on a copy the owner forwarded, the room, the message and the name it copies; and, on a message the supervisor judged, the room's situation note as that round read it and whether the round changed it | room and message |
| `message_reactions` | a reaction kept on the record on a message, its emoji and who put it there, one to a reactor | room, message and reactor |
| `message_metadata` | what a chat service handed over beside a message as its context's `metadata`, kept as it arrived and never read into (`docs/boundary-api.md`, 「Reading a room」) | room and message |
| `instructions` | a quiet instruction waiting for, handed to or delivered to an agent's session in a room (`docs/bmo-window.md`): its words, kind, sort and summary, whether it is for the agent alone and no room hears it, events, expiry, ride, the round that settled it and the run it carries the result of | its number |
| `settings` | one setting: a key and its text value (below) | the key |
| `allowed_chats` | a room the receiver listens to | room |
| `tool_chats` | a room where the agents' tools are allowed | room |
| `delegate_chats` | a room in delegate mode (`docs/rooms.md`) | room |
| `photo_chats` | a room's word on whether its agent opens the pictures it stores (`docs/pictures.md`) | room |
| `muted_members` | a person muted in a room (`docs/rooms.md`) | room and person |
| `people_languages` | the language a person writes in a room, read off their newest message there that holds prose, the message it was read off and when (`docs/answer-checks.md`, 「The language an answer goes out in」) | room and person |
| `chat_notices` | a room's rest or restart notice switched on or off (`docs/voiced-notices.md`) | room and kind |
| `chat_reads` | the last message of a room the receiver has read | room |
| `chat_titles` | a room's title | room |
| `room_answerers` | an agent answering in a room | room and agent |
| `people_reads` | a message an agent's book of a room has read, and when the mark was made | agent, room and message |
| `room_windows` | when a room's window for an agent closes, in epoch milliseconds (`docs/wake.md`, 「The names and the window」) | room and agent |
| `run_records` | what one of a room's runs wrote down beside it: the run it was written under, the kind of record, its words, the number of the row it wrote, whether it woke the agent and when (`docs/answer-checks.md`) | its number |
| `bmo_rooms` | a room the chat agent's one shared session has answered in (`docs/unified-bmo.md`) | room |
| `bmo_room_contexts` | the fingerprint of the room context a chat session was last given | session |
| `bmo_session_rooms` | a room a chat session has been given the context of | session and room |
| `collected_items` | a file a message carried, kept in the message-file store (`docs/room-files.md`) | its number; one per room, message and the file's id among its message's files, `file_unique_id` |
| `avatars` | the picture a room or a person wears, its photo id, stored path and when it was taken (`docs/pictures.md`) | kind and subject |
| `avatar_sets` | the picture set on a room or a person from the console, its stored path and when it was set (`docs/pictures.md`) | kind and subject |
| `random_draws` | a fair draw made in a room (`docs/random-draw.md`) | its number |
| `random_draw_rooms` | the seed a room's next draw will use and its commitment | room |
| `say_relays` | an answer to relay back to the room that asked another room something | its number |
| `reload_pending` | a document moved that a room's session has not been told of yet | room and path |
| `reload_sent` | when a room was last told of moved documents | room |
| `provider_rooms` | a room a chat service numbered: its own room id, the number and its title | chat service and room id; one per number |
| `provider_users` | a person a chat service numbered: their own user id and the number | chat service and user id; one per number |
| `provider_private_rooms` | a room a chat service says is one person's one-to-one with the agent | chat service and room |
| `provider_owner_users` | a user id of a chat service that is the owner in a room | chat service, room and user id |
| `message_number_marks` | the lowest number the room's own record has handed out, so a number whose message left the record is never handed out again (`docs/unified-bmo.md`) | room |
| `room_modes` | the mode a room was added in, `avatar` or `record-only`, and when it was added; a room never added has none (`docs/receiver.md`, 「Adding a room」) | room |
| `room_providers` | a chat provider a room is on, by its key, and its place in the room's list; a room with none stands on its default (`docs/receiver.md`, 「A room's chat providers」) | room and provider |
| `room_deliveries` | a kept message carried to one of the room's providers: the provider, the number it gave its copy once it answers, and when it went; its number is the send's tag | its number |
| `news_briefs` | a room's morning news edition: the time it is sent, how long before it the writing starts, how long its link lives, whether it is on and the day it last started | its number |
| `holidays` | a day off, so no morning news edition goes out on it | its day |
| `draft_processes` | a row of the draft process table the console set, over the policy's (`docs/drafting.md`, 「The process table」): the questions of an ordinary and a light turn, the question a send is read from, the light bound, the ceiling and what is done past it, and when it was set | provider, tier and agent |
| `draft_questions` | a question's text the console set, over the policy's catalogue, and when | its name |
| `draft_rounds` | one turn of a round's draft process, or its end (`docs/drafting.md`, 「The round log」): the round, the room, the agent, its number, the question, its kind, its answer's length, whether it changed, how it ended, a detail, the answer, how long it took and when, and on the end what the round read from the brain | its number |
| `room_situations` | a room's situation note, the receiver's own (`docs/drafting.md`, 「The situation note」): its text, replaced when it changes, and when it last did | room |

A room's situation note is the receiver's own row, one a room: the room's live state, not the agent's memory. The
brain's `situation` rows stay rules for a situation, and this note is written and read by the receiver alone. Its
text is the version the round's record names.

A room's line is read newest first or from a message the reader holds, by when each message was sent and then by
its row, so `messages` is indexed by room, time and row, and a page reads only the messages it answers. The roads a
room's messages came in on are read off an index of the messages that name one, by room and road.

The morning news briefing's schedule and the holiday calendar its days skip are this store's. The page goes to
its room through the receiver's own sends.

## The settings

A setting is one row of `settings`, its value text. The store supplies reading, writing, forgetting and taking a
key, the numbers of a family of keys, and every key carrying a name after a prefix with its value, for the members
such a family is kept for; each key is named by the code that writes it, and the table below is the whole list. A key that names a room is built only through a room setting's declaration
(`docs/receiver.md`, 「Removing a room's registration」).

| Key | Value | Kept by |
|---|---|---|
| `mode` | `owner_only` or `everyone`: whose messages wake the agents (`docs/rooms.md`) | the owner's mode command |
| `photos_look` | `on` while the agent opens the pictures rooms store; absent otherwise | the photos switch |
| `private_thread` | `off` turns a person's own thread in a group room off (`chat-round.toml`, its `setting`) | the thread |
| `thread_mark:<session>` | the last line of a person's own thread handed to a session, as `room:messageId`; a session continued across a restart keeps it, and a new or replaced session has none (`docs/bmo-window.md`, 「What every turn carries」) | the thread |
| `voice_briefing` | `off` while voice briefings are switched off | the voice switch |
| `voice_briefing_autoplay` | `off` while a briefing does not play by itself | the voice switch |
| `consult_room` | the consult room's number (`consult.toml`, its `room_setting`) | the consult room (`docs/consult.md`) |
| `restart_reason` | what a deployment recorded before the receiver stopped: its purpose and the items going live, taken once at start-up | the restart |
| `fallback_since`, `fallback_provider` | since when, in epoch milliseconds, a stand-in has answered for the chat agent, and which one | the agency's `fallback.changed` |
| `job_board_messages` | the messages a job's board left standing in rooms, as JSON | the board left when a job ends |
| `job_board_message_id` | the older single form of the same, read once and cleared | the board left when a job ends |
| `console_pinned_rooms` | the rooms the console stands over the rest, as a JSON list of room numbers in the owner's order; a room's removal takes its number out, and a number the record no longer has stands for nothing | the console's pinned order (`docs/boundary-api.md`, 「The pinned order」), read by the console's room list |
| `room_holder:<unit>` | a unit declared a holder of rows naming a room: its API base and its holdings, as JSON | the holder's declaration (`PUT /room-holders/{unit}`) |
| `<agent>_active_route` | the road an agent's one chat session stands on, as JSON, with the tail of what the session answered in each room, the transcript a replacement falls back on | the chat session |
| `<agent>_force_reset` | the owner's word to start an agent's session afresh, with its reason, spent by the replacement it asks for | the chat session |
| `<agent>_cache:<session>` | what the receiver knows of a session's cache between turns, as JSON: the fingerprint of the head and the agency's reason for a changed prefix its last turn stood on, when that turn was, and whether a resumed turn read nothing from the cache (`docs/unified-bmo.md`, 「Replacing the session」) | the chat session |
| `<agent>_handoff_pending` | the path of the handoff file an agent's standing session wrote when the agency asked ahead of a fresh session, taken by the next turn (`docs/unified-bmo.md`, 「Replacing the session」) | the handoff the agency asks for |
| `<agent>_briefed_at`, `<agent>_briefed_at:<room>` | when the agent's one session, or its session of the room, was last briefed on what went live, or first looked, in epoch milliseconds (`docs/wake.md`, 「Quiet instructions」) | the deployment briefing |
| `<agent>_last_chat_launch` | the launch the agent's one chat session last stood on, as JSON: the room it stood in, whether every word of it was the owner's, whether the room's tools were on, and when it went out, in epoch milliseconds | the chat session |

No setting holds the note a replaced session leaves: that note is its handoff file under the data root
(「The data root's folders」 below).

## The data root's folders

The data root is the configuration's. Three stores under it keep a folder per room, named by the room's number
(`docs/room-files.md`, 「The stores that keep files by room」):

| Store | Folder | Moved by |
|---|---|---|
| the message-file store | `collected/<room>/` | `BMO_COLLECT_DIR`, naming the folder that stands for `collected/` |
| the picture store | `images/<room>/` | `BMO_IMAGE_DIR`, naming the folder that stands for `images/` |
| the outboxes | `outbox/<room>/` | nothing |

A room's folder in a store is found only where it stands; finding it makes nothing. The folders go only with
the removal of the room's registration.

One folder under the data root is kept per agent rather than per room:

| Store | Folder | Holds |
|---|---|---|
| the handoffs | `handoffs/<agent>/` | each handoff a replaced session of the agent's wrote, as `<epoch-ms>.md`, the moment it was asked for, and `latest.md`, a copy of the newest (`docs/unified-bmo.md`, 「Replacing the session」) |

A handoff is written by the agent's session with its own file tool and read by the receiver once, for the new
session's first turn; the receiver makes the folder as it asks for one and removes nothing from it.
