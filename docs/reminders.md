# Reminders, as the receiver delivers them

A reminder wakes an agent in its room at a moment of the room's own calendar. Neither the receiver nor the brain
holds one: a reminder, like a wait on an event and a watch, is the wait tool's, a tool run outside the brain that
holds the thing waited for and wakes the agent with the memory it was told to carry (the brain's
`docs/design.md`, 「What the brain acts on by itself」).

## Setting a reminder

- **No reminder can be set** until the wait tool outside the brain exists. The send tool holds no entry for one,
  `POST /sends` takes no act for one, and the receiver's command line has no verb for one.

## Its moment wakes the agent in its room

- **What holds the reminder queues the wake** with `POST /instructions`, kind `reminder`, for the agent in the
  reminder's room, carrying the reason, the memory as it stands at that moment and who it is for.
- **The receiver queues it as a quiet instruction** of that room and wakes the agent's turn there through the
  instruction pass (`docs/bmo-window.md`, 「Quiet instruction」). The agent looks at how things stand and writes
  what the room hears then, in that room's manner, or says nothing where nothing needs saying.
- **The wake is kept until it is taken:** once the instruction stands in the receiver's instruction table it
  survives a restart until the room's session takes it, so what queued it may count the moment done once the
  route answers.

## A reminder and a watch

- A watch wakes the agent on a condition, or at a moment at most a week out, in the chat that set it; its memo
  comes the same way, as a quiet instruction of kind `watch`. A reminder wakes the agent in its room at a moment
  of the room's own calendar, however far ahead and however often.
