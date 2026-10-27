# Room switches

The per-room switches the owner throws from inside a chat (who may wake the agent, delegation, muting), the
reply budget, and the infrastructure room. Default access, owner-only work and room rules are
`docs/unified-bmo.md` (「Default access and per-room delegation」). The words each command is matched by are data,
in `policy/commands.toml`; each command also has a plain form that is the same in every language, given below.

## The infrastructure room

- **The configuration may name one room as the infrastructure room,** the owner's board for the machines and
  services the deployment stands on.
- **What stands there is a report on work done to one of them,** put there by whoever did that work through a
  send; the receiver writes none of it by itself.
- **What may never stand there is the agent's own news.** A job's report and a job that died without reporting go
  to the owner instead of that room, the going to sleep and waking of a session are not said there at all, and
  the rest and restart notices are the owner's.
- **A person speaking in the room is another matter:** the agent answers there as it answers anywhere.

## Modes

- **Switched by an owner message:** `mode:owner_only` wakes on the owner's messages; `mode:everyone` lets anyone
  wake the agent.
- **An active owner delegation also admits its named sender,** who still wakes the agent only on the grounds in
  `docs/wake.md` (「Who reaches the session」).

## Delegation

- **A per-chat flag, off by default, stored in the receiver's database.** The owner switches it on from inside a
  chat with `delegate:on` (or the delegation words of the policy) and off with `delegate:off`.
- **Every prompt states whether delegation is on in the chat,** and the agent keys off that flag alone: it never
  guesses who is in the room.
- **Where delegation is on,** a request the room's own context names as somebody else's to handle is pointed to
  whoever it names, in one short line, instead of being done by the agent; which requests those are and who takes
  them belongs to that room's layer, never to the fixed prompt. The notice the switch leaves names the delegate
  by its registered label.
- **Where delegation is off, or where the room names nobody,** the agent simply answers, searching when the
  answer needs the web.

## Delegations

- **A grant is made or taken away on the owner's own message in the room,** through the web unit's grants
  (`POST /grants`, `DELETE /grants`): the granter is the sender of the message the grant is made on, and the web
  unit checks that they may grant what they give. What is in force in a room is read the same way, and the
  `bmo room rules` command shows it.

## Mute

- **The owner silences a member inside a chat** with `member:mute` and lifts it with `member:unmute` (or the
  mute words of the policy), naming the target either by replying to one of that member's messages or by putting
  the member's numeric user id in the text.
- **A muted member's messages are still archived** but never wake an agent or join a conversation, so they cost
  nothing.
- **The mute is stored per chat and per user in the receiver's database.**

## Operating notices and pictures

- **A notice and a picture go only where the room's providers say:** kept in the room's record and carried to the
  messengers its list names, never out on one the list does not name (`docs/receiver.md`, 「A room's chat
  providers」).
- **The owner switches the room's rest notice and restart notice** on and off with `notice:rest:on|off` and
  `notice:restart:on|off`, and whether the agent opens the pictures the room stores with `photos:on|off` (or the
  words of the policy for each).

## Reply budget

- **After the agent has answered the same non-owner member five times within ten minutes in a chat,** that
  member's messages stop reaching the agent until the window has passed.
- **The owner is exempt.**
- **The counter lives in memory,** so a restart clears it.
