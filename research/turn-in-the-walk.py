"""Reads the check and merge rounds of the owner's store, as the query in `turn-in-the-walk.md` gives them, and says how
often a check began while another work of the project had a check or merge round under way, how long it waited
inside its round, in what order the overlapping checks ended, and how long a passed check was followed by nothing.

For: the research of item #87 (`turn-in-the-walk.md`), whose numbers it gives. Kept beside that note, and deleted
with it.

Usage: python3 -I turn-in-the-walk.py RESULT.json SINCE_MS
RESULT.json holds `{"rows": [...]}`, the query's rows; SINCE_MS is the first time, in milliseconds, whose rounds count.
"""

import json
import sys

MERGED_GAP_MS = 1000  # a round that begins within this of the last one's end goes on from it


def main(path, since):
    rows = json.load(open(path))["rows"]
    now = max(r["t1"] or 0 for r in rows)
    for r in rows:
        r["t1"] = r["t1"] or now  # a round still under way as the store was read
    rows = [r for r in rows if r["project"] == "wf-agents" and r["t0"] >= since]
    rows.sort(key=lambda r: (r["t0"], r["work"]))
    by_work = {}
    for r in rows:
        by_work.setdefault(r["work"], []).append(r)

    def chain_end(r):
        """The end of the run of check and merge rounds of r's work that r begins."""
        end = r["t1"]
        runs = by_work[r["work"]]
        for nxt in runs[runs.index(r) + 1:]:
            if nxt["t0"] - end > MERGED_GAP_MS:
                break
            end = nxt["t1"]
        return end

    checks = [r for r in rows if r["cell"] == "M1"]
    met = []
    for r in checks:
        ahead = [o for o in rows if o["work"] != r["work"] and o["t0"] < r["t0"] < o["t1"]]
        if ahead:
            until = min(max(chain_end(o) for o in ahead), r["t1"])
            met.append((r, max(0, until - r["t0"])))
    single = [(r, w) for r, w in met if r["nrounds"] == 1]
    multi = [(r, w) for r, w in met if r["nrounds"] > 1]
    print("checks:", len(checks), "begun while another work's check or merge was under way:", len(met))
    print("  of one round:", len(single), "inferred wait s:", round(sum(w for _, w in single) / 1000),
          "longest s:", round(max((w for _, w in single), default=0) / 1000),
          "most moments inside one:", max((r["inside"] for r, _ in single), default=0))
    print("  of more rounds:", len(multi), "inferred wait s:", round(sum(w for _, w in multi) / 1000))

    later_first = [(a["work"], a["run"], a["nrounds"], b["work"], b["run"], b["nrounds"])
                   for a in checks for b in checks
                   if a["work"] != b["work"] and a["t0"] < b["t0"] < a["t1"] and b["t1"] < a["t1"]]
    print("pairs in which the check begun later ended first (work, run, rounds; work, run, rounds):")
    print("  where both had one round:", [p for p in later_first if p[2] == 1 and p[5] == 1])
    print("  where one had more:", len(later_first) - len([p for p in later_first if p[2] == 1 and p[5] == 1]))
    ties = [(a, b) for a in checks for b in checks if a["work"] < b["work"] and a["t0"] == b["t0"]]
    print("checks begun in the same millisecond, the lower work first to end:",
          [(a["work"], b["work"], a["t1"] <= b["t1"]) for a, b in ties])

    print("passed checks followed by no check or merge round of their work for more than a second (work, run, s):")
    for r in checks:
        if r["checked"] != "yes":
            continue
        nxt = [o for o in by_work[r["work"]] if o is not r and o["t0"] >= r["t1"]]
        gap = (nxt[0]["t0"] - r["t1"]) / 1000 if nxt else None
        if gap is None or gap > 1:
            print(" ", r["work"], r["run"], gap)


if __name__ == "__main__":
    main(sys.argv[1], int(sys.argv[2]))
