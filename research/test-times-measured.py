"""Reads the JUnit of the artifact `test-results` of ci.yml runs on main, each run in its own directory named by the run's
id, and says how the time of a test spreads from run to run and what a line worked out from the history of its kind
(`2 x largest - median`, as crates/driver/src/history.rs works it out) judges.

Run as: python3 -I research/test-times-measured.py <directory of the runs>
Kept with research/test-times-measured.md, which says what it was run on and what it found."""
import pathlib
import statistics as st
import sys
import xml.etree.ElementTree as ET

LEAST = 19  # the default of `history.samples`


def read(directory):
    runs = []
    for d in sorted(pathlib.Path(directory).iterdir(), key=lambda p: int(p.name)):
        times = {}
        for path in d.glob("*.xml"):
            for case in ET.parse(path).iter("testcase"):
                times[(case.get("classname"), case.get("name"))] = float(case.get("time") or 0)
        runs.append(times)
    return runs


def line(values):
    return 2 * max(values) - st.median(values)


def main():
    runs = read(sys.argv[1])
    print(f"{len(runs)} runs; tests per run {runs[0] and len(runs[0])} to {len(runs[-1])}")
    totals = [sum(r.values()) for r in runs]
    print(f"sum of the tests' times per run: least {min(totals):.0f} s, most {max(totals):.0f} s")
    last = runs[-LEAST:]
    common = set.intersection(*[set(r) for r in last])
    ratios = sorted(max(r[k] for r in last) / min(r[k] for r in last) for k in common if min(r[k] for r in last) > 0.005)
    print(f"the {len(ratios)} tests above 5 ms in each of the last {LEAST} runs: their slowest run over their fastest "
          f"is {st.median(ratios):.2f} at the median, {ratios[int(0.9 * len(ratios))]:.2f} at the 90th percentile")
    for i in range(len(runs) - 5, len(runs)):
        history, now = runs[i - LEAST : i], runs[i]
        judged = past = 0
        for key, value in now.items():
            before = [r[key] for r in history if key in r]
            if len(before) < LEAST:
                continue
            judged += 1
            past += value > line(before)
        print(f"run {i}: {judged} tests judged by their own {LEAST} earlier times, {past} past their line")
    added = []
    for i in range(1, len(runs)):
        added.append(sum(v for k, v in runs[i].items() if k not in runs[i - 1]))
    print("what each merge added (the times of the tests the run before did not report), in s:",
          " ".join(f"{a:.1f}" for a in added))
    for j in range(LEAST, len(added)):
        print(f"merge {j + 1}: added {added[j]:.1f} s, line of the earlier additions {line(added[j - LEAST : j]):.1f} s")
    for i in (-12, -1):
        times = sorted(runs[i].values(), reverse=True)
        held = 0.0
        for n, value in enumerate(times, 1):
            held += value
            if held >= sum(times) / 2:
                print(f"run {i}: the slowest {n} of {len(times)} tests hold half of {sum(times):.0f} s; "
                      f"the time of the {n}th is {value:.2f} s")
                break
    before, batch = runs[12], runs[13]
    ranked = sorted(((v, k[1][:40]) for k, v in batch.items() if k not in before), reverse=True)[:5]
    below = sorted(before.values())
    for value, name in ranked:
        print(f"added test {value:.2f} s, slower than {100 * sum(1 for x in below if x < value) / len(below):.1f} % "
              f"of the tests before it: {name}")
    every = [v for r in runs for v in r.values()]
    print(f"the line of every test's times: {line(every):.1f} s (largest {max(every):.1f} s, median {st.median(every):.3f} s)")


main()
