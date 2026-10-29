# bmo/wait

A reminder or a watch: the call waits for a moment, or runs a shell check until it holds, then answers with your
note. Run it through `bmo use --async` so the turn goes on and the call's end wakes you in the room with the
note and the memory you gave it.

## Arguments

- `seconds`: how many seconds to wait. Give this, `until`, or `check`.
- `until`: the moment to wait until, a date and time with its offset from UTC, such as
  `2026-10-24T09:00:00+09:00`. A moment already past is answered at once.
- `check`: the shell command of a watch, run through `sh -c` at once and then every few seconds. It holds when
  it exits 0. Use absolute paths: it does not run in your working directory.
- `every`: seconds between runs of the check, 60 where not given. A run that outlasts it is ended and counts as
  not holding. With `check` alone.
- `within`: seconds after which a check that has not held ends the watch as lapsed. None where not given; the
  unit's longest run, one day unless `--max-seconds` names another, ends it then. With `check` alone.
- `note`: what the answer says when the wait is over or the check holds.

## Examples

Remind yourself in ten minutes, in this room:

    bmo use bmo/wait --async --arg seconds=600 --arg note="Ask whether the build finished" --memory "They asked to be told before lunch"

A moment more than a day away needs the unit to live past it:

    bmo use bmo/wait --async --max-seconds 259200 --arg until=2026-10-26T09:00:00+09:00 --arg note="The meeting starts in an hour"

Wake when a file appears, checking every minute for at most two hours:

    bmo use bmo/wait --async --arg check="test -e /srv/export/done" --arg every=60 --arg within=7200 --arg note="The export is done"

Wake when a page answers:

    bmo use bmo/wait --async --arg check="curl -fsS https://example.org/health" --arg every=300 --arg note="The site is back"

## What happens

- A wait's answer's first line is the note; the next names the moment it waited until.
- Once a watch's run holds, the answer's first line is the note; then the run it held on and what the check
  wrote, its last 2000 characters. Where `within` passes first, the first line is `lapsed:` and the note, then
  the number of runs and what the last run wrote.
- The wake comes once, in the room `bmo use` names or the turn's own. To repeat, set the next one when it wakes
  you.
- To cancel, stop the unit `bmo use --async` answered: `systemctl --user stop bmo-use-<id>`. Its stop wakes you
  once more, saying the call failed.
