# bmo/wait

A reminder or a watch: the call waits for a moment, or runs a shell check until it holds, then answers with your
note. Run it through `bmo use --async` so the turn goes on and the call's end wakes you in the room with the
note and the memory you gave it.

Name the room the wake is for with `--room <room>` and yourself with `--agent <name>`. Where your shell carries
no turn room — a session that spans several rooms — a call that names none is kept with no room: its wake goes to
the owner's private chat, and the console cannot show it under the room it was for.

A deployment of the tool host ends every wait as it restarts, and you are woken with the failure saying the
host stopped: set the same reminder or watch again.

## Arguments

- `seconds`: how many seconds to wait. Give this, `until`, or `check`.
- `until`: the moment to wait until, a date and time with its offset from UTC, such as
  `2026-10-24T09:00:00+09:00`. A moment already past is answered at once.
- `check`: the shell command of a watch, run through `sh -c` at once and then every few seconds. It holds when
  it exits 0. Use absolute paths: it does not run in your working directory.
- `every`: seconds from one run of the check ending to the next starting, 60 where not given. Each run is waited
  on until it exits, however long it takes. With `check` alone.
- `note`: what the answer says when the wait is over or the check holds.

## Examples

Remind yourself in ten minutes, in this room:

    bmo use bmo/wait --async --room -1001 --agent agent-a --arg seconds=600 --arg note="Ask whether the build finished" --memory "They asked to be told before lunch"

A moment days away is set the same way; nothing but the moment or a stop ends the wait:

    bmo use bmo/wait --async --room -1001 --agent agent-a --arg until=2026-10-26T09:00:00+09:00 --arg note="The meeting starts in an hour"

Wake when a file appears, checking every minute:

    bmo use bmo/wait --async --room -1001 --agent agent-a --arg check="test -e /srv/export/done" --arg every=60 --arg note="The export is done"

Wake when a page answers:

    bmo use bmo/wait --async --room -1001 --agent agent-a --arg check="curl -fsS https://example.org/health" --arg every=300 --arg note="The site is back"

## What happens

- A wait's answer's first line is the note; the next names the moment it waited until.
- Once a watch's run holds, the answer's first line is the note; then the run it held on and what the check
  wrote, its last 2000 characters. A watch ends only when its check holds or it is stopped; to hear at a moment
  whether it has held by then, set a reminder for that moment beside it.
- The wake comes once, in the room `bmo use` names or the turn's own. To repeat, set the next one when it wakes
  you.
- To cancel, stop the unit `bmo use --async` answered: `systemctl --user stop bmo-use-<id>`. Its stop wakes you
  once more, saying the call failed.
