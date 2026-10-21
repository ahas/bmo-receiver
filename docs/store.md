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
| `messages` | a message said in a room, by a person or an agent: its sender and name, time, words, stored picture, the message it answers, how long its answer took, the round that answered it, its album and the chat service it came by | room and message |
| `message_metadata` | what a chat service handed over beside a message as its context's `metadata`, kept as it arrived and never read into (`docs/boundary-api.md`, 「Reading a room」) | room and message |
| `instructions` | a quiet instruction waiting for, handed to or delivered to an agent's session in a room (`docs/bmo-window.md`): its words, kind, sort and summary, events, expiry, ride, the round that settled it and the run it carries the result of | its number |
| `settings` | one setting: a key and its text value (below) | the key |
| `allowed_chats` | a room the receiver listens to | room |
| `tool_chats` | a room where the agents' tools are allowed | room |
| `delegate_chats` | a room in delegate mode (`docs/rooms.md`) | room |
| `photo_chats` | a room's word on whether its agent opens the pictures it stores (`docs/pictures.md`) | room |
| `muted_members` | a person muted in a room (`docs/rooms.md`) | room and person |
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
| `news_briefs` | a room's morning news edition: the time it is sent, how long before it the writing starts, how long its link lives, whether it is on and the day it last started | its number |
| `holidays` | a day off, so no morning news edition goes out on it | its day |

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
| `voice_briefing` | `off` while voice briefings are switched off | the voice switch |
| `voice_briefing_autoplay` | `off` while a briefing does not play by itself | the voice switch |
| `consult_room` | the consult room's number (`consult.toml`, its `room_setting`) | the consult room (`docs/consult.md`) |
| `restart_reason` | what a deployment recorded before the receiver stopped: its purpose and the items going live, taken once at start-up | the restart |
| `fallback_since`, `fallback_provider` | since when, in epoch milliseconds, a stand-in has answered for the chat agent, and which one | the agency's `fallback.changed` |
| `job_board_messages` | the messages a job's board left standing in rooms, as JSON | the board left when a job ends |
| `job_board_message_id` | the older single form of the same, read once and cleared | the board left when a job ends |
| `console_pinned_rooms` | the rooms the console stands over the rest, as a JSON list of room numbers in the owner's order; a room's removal takes its number out, and a number the record no longer has stands for nothing | the console's pinned order (`docs/boundary-api.md`, 「The pinned order」), read by the console's room list |
| `room_holder:<unit>` | a unit declared a holder of rows naming a room: its API base and its holdings, as JSON | the holder's declaration (`PUT /room-holders/{unit}`) |
| `<agent>_active_route` | the road an agent's one chat session stands on, as JSON | the chat session |
| `<agent>_pending_summary` | the note a replaced session left for the next fresh one | the chat session |
| `<agent>_force_reset` | the owner's word to start an agent's session afresh, with its reason | the chat session |
| `<agent>_cache:<session>` | what the receiver knows of a session's cache between turns, as JSON | the chat session |
| `<agent>_last_chat_launch` | the launch the agent's one chat session last stood on, as JSON: the room it stood in, whether every word of it was the owner's, whether the room's tools were on, and when it went out, in epoch milliseconds | the chat session |

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
