# bmo/watch

A watch: the call runs a shell check at once and then every few seconds, and answers once a run of it exits 0.
Run it through `bmo use --async` so the turn goes on and the call's end wakes you in the room.

## Arguments

- `check`: the shell command, run through `sh -c`. It holds when it exits 0. Use absolute paths: it does not run
  in your working directory.
- `every`: seconds between runs, 60 where not given. A run that outlasts it is ended and counts as not holding.
- `within`: seconds after which a check that has not held ends the watch as lapsed. None where not given; the
  unit's longest run, one day unless `--max-seconds` names another, ends it then.
- `note`: what the answer says when the watch ends.

## Examples

Wake when a file appears, checking every minute for at most two hours:

    bmo use bmo/watch --async --arg check="test -e /srv/export/done" --arg every=60 --arg within=7200 --arg note="The export is done"

Wake when a page answers:

    bmo use bmo/watch --async --arg check="curl -fsS https://example.org/health" --arg every=300 --arg note="The site is back"

## What happens

- Once a run holds, the answer's first line is the note; then the run it held on and what the check wrote, its
  last 2000 characters.
- Where `within` passes first, the first line is `lapsed:` and the note, then the number of runs and what the
  last run wrote.
- To cancel, stop the unit `bmo use --async` answered: `systemctl --user stop bmo-use-<id>`.
