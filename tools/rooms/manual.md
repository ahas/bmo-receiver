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

## Adding a chat-service room

`room_add` adds a room the owner asked for on a chat service, in `record-only` mode: the messages are kept and
carried as any room's, and no agent is woken there. Only the owner's word in the turn is served; a job, a worker
or a turn that is not the owner's is refused. The mode is never chosen by the call, and an existing room's mode
is never changed here.

Name the chat service's key and one of `roomId`, the room as that service numbers it, `contactId`, a contact to
start a one-to-one with, or `contactIds`, the contacts to start a group with under `title`. Where the owner names
no room exactly, call it with the provider alone: it answers the provider's rooms and contacts, each by its name
and with whether the receiver already holds it, so the owner picks one, and you call again with its id.

The addition is the receiver's own `POST /rooms`, so a room already added, a room the service does not say is its
own and a mode the service does not offer are refused in the receiver's own words.

## Its arguments

- `room_open`: `title`, the room's title; `supervisors`, the agents that answer it.
- `room_list`: `supervisor`, whose rooms are read; the calling agent where absent.
- `room_supervisors`: `roomId`, the room; `supervisors`, the whole list that answers it.
- `room_add`: `provider`, the chat service's key; one of `roomId`, `contactId` or `contactIds` with `title` for a
  group, or none of them to list the provider's rooms and contacts; `title`, optional except for a group.

## The answer

The receiver's own answer as JSON: a room for `room_open`, `room_supervisors` and `room_add`, a page of rooms for
`room_list`, and the provider's rooms and contacts by name for `room_add` when none is named. A refusal is an
English sentence saying what stood in the way.
