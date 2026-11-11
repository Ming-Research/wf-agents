# For each phrase given after --args, print how many string values of a flow file contain it.
#
# Run once for each flow file to compare (here: the three revisions of the report), for example:
#   jq -rn -f probe.jq --slurpfile f flow.json --args 'Decisions:' 'relayed by the delegate'
# The output is one line for each phrase: the count, a tab, the phrase.
[$ARGS.positional[] as $p
  | {phrase: $p, n: ([$f[0] | .. | strings | select(contains($p))] | length)}]
| .[]
| "\(.n)\t\(.phrase)"
