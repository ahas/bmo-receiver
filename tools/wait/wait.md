# bmo/wait

A reminder: the call waits, then answers with your note. Run it through `bmo use --async` so the turn goes on
and the call's end wakes you in the room with the note and the memory you gave it.

## Arguments

- `seconds`: how many seconds to wait. Give this or `until`.
- `until`: the moment to wait until, a date and time with its offset from UTC, such as
  `2026-10-24T09:00:00+09:00`. A moment already past is answered at once.
- `note`: what the answer says when the moment comes.

## Examples

Remind yourself in ten minutes, in this room:

    bmo use bmo/wait --async --arg seconds=600 --arg note="Ask whether the build finished" --memory "They asked to be told before lunch"

A moment more than a day away needs the unit to live past it:

    bmo use bmo/wait --async --max-seconds 259200 --arg until=2026-10-26T09:00:00+09:00 --arg note="The meeting starts in an hour"

## What happens

- The answer's first line is the note; the next names the moment it waited until.
- The wake comes once, in the room `bmo use` names or the turn's own. To repeat, set the next one when it wakes
  you.
- To cancel, stop the unit `bmo use --async` answered: `systemctl --user stop bmo-use-<id>`. Its stop wakes you
  once more, saying the call failed.
