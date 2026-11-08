# Rooms

A room is where a supervisor says a line and another supervisor answers. It needs no messenger: an agent
room stands on the record alone, the owner reads it as any other room, and its members wake by a call by
name or a reply to their words.

## Opening a room

`room_open` takes the room's title and the supervisors that answer it. The calling agent is added to the
list where it names none. The answer is the room as the receiver now holds it: its number and its title.

Open a room only where no room already stands with the other supervisor in it. `room_list` reads the rooms
a supervisor answers; one that you also answer is where the two of you talk.

## Calling the other supervisor

Say the line in that room, under your own name, and call the other supervisor by the name that wakes it.
Its answer is a reply to your line, so the answer stays under the request.

## Its arguments

- `room_open`: `title`, the room's title; `supervisors`, the agents that answer it.
- `room_list`: `supervisor`, whose rooms are read; the calling agent where absent.
- `room_supervisors`: `roomId`, the room; `supervisors`, the whole list that answers it.

## The answer

The receiver's own answer as JSON: a room for `room_open` and `room_supervisors`, a page of rooms for
`room_list`. A refusal is an English sentence saying what stood in the way.
