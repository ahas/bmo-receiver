# Reminders, as the receiver delivers them

A reminder wakes an agent in its room at a moment of the room's own calendar. The reminder itself (what it
holds, its moment, its repeat rule, a moment missed while a service was down, listing and cancelling) is the
brain's: its rows, its clock and its routes (the brain's `docs/boundary-api.md`, 「Watches and reminders」). The
receiver takes a reminder asked in a room to the brain, and delivers its moment's wake.

## Taking a reminder

- **In a turn with tools,** an agent takes reminders: any member of the room asks for one, for themselves or for
  someone else, and the agent sets it with the send tool's `remind` entry or `bmo reminder set` from a shell.
- **The send tool posts to the brain directly** (`POST /reminders/set`, `POST /reminders/cancel`,
  `POST /reminders/list`), with the turn's agent and room, its schema and shown fields (`at`, `every`, `reason`,
  `memory`) kept as they are.
- **A `remind` asked of `POST /sends`** (from a shell, or a turn that asks the receiver) is carried the same way,
  to the brain's `POST /reminders/set`, as the asker's agent in its room, and answered with the reading the brain
  gives.

## Its moment wakes the agent in its room

- **The brain queues the wake** with `POST /instructions/quiet`, kind `reminder`, for the agent in the reminder's
  room, carrying the reason, the memory as it stands at that moment and who it is for.
- **The receiver queues it as a quiet instruction** of that room and wakes the agent's turn there through the
  instruction pass (`docs/bmo-window.md`, 「Quiet instruction」). The agent looks at how things stand and writes
  what the room hears then, in that room's manner, or says nothing where nothing needs saying.
- **The wake is kept until it is taken:** once the instruction stands in the receiver's instruction table it
  survives a restart until the room's session takes it, which is why the brain counts a moment done once the
  route answers.

## A reminder and a watch

- A watch wakes the agent on a condition, or at a moment at most a week out, in the chat that set it; its memo
  comes the same way, as a quiet instruction of kind `watch`. A reminder wakes the agent in its room at a moment
  of the room's own calendar, however far ahead and however often.
