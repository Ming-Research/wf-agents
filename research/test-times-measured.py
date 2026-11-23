"""Reads the JUnit of the artifact `test-results` of ci.yml runs on main, each run in its own directory named by the run's
id, and says how the time of a test spreads from run to run and what a line worked out from the history of its kind
(`2 x largest - median`, as crates/driver/src/history.rs works it out) judges.

Run as: python3 -I research/test-times-measured.py <directory of the full runs> [<directory of fast runs>]
The fast runs are the runs of fast.yml, each in a directory named by its run id, with the same artifact.
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
    def half(history):
        medians = {k: st.median([r[k] for r in history if k in r]) for k in set().union(*history)}
        times = sorted(medians.values(), reverse=True)
        held = 0.0
        for n, value in enumerate(times, 1):
            held += value
            if held >= sum(times) / 2:
                return n, value

    for i in (len(runs) - 1, 13):
        n, value = half(runs[max(0, i - LEAST) : i])
        print(f"before run {i}: by each test's median over up to {LEAST} runs, the slowest {n} tests hold half of the "
              f"suite's time; the time of the {n}th is {value:.2f} s")
    previous = set()
    for i in range(LEAST, len(runs)):
        past = set()
        for key, value in runs[i].items():
            before = [r[key] for r in runs[i - LEAST : i] if key in r]
            if len(before) >= LEAST and value > line(before):
                past.add(key)
        print(f"run {i}: {len(past)} tests past their own line, {len(past & previous)} of them past it in the run "
              f"before too")
        previous = past
    before, batch = runs[12], runs[13]
    ranked = sorted(((v, k[1][:40]) for k, v in batch.items() if k not in before), reverse=True)[:5]
    below = sorted(before.values())
    for value, name in ranked:
        print(f"added test {value:.2f} s, slower than {100 * sum(1 for x in below if x < value) / len(below):.1f} % "
              f"of the tests before it: {name}")
    if len(sys.argv) > 2:
        medians = {}
        for key in set().union(*runs):
            values = [r[key] for r in runs if key in r]
            if len(values) >= 10:
                medians[key] = st.median(values)
        ratios = []
        for d in pathlib.Path(sys.argv[2]).iterdir():
            for path in d.glob("rust*.xml"):
                for case in ET.parse(path).iter("testcase"):
                    if case.find("failure") is not None or case.find("error") is not None or case.find("skipped") is not None:
                        continue
                    key = (case.get("classname"), case.get("name"))
                    if medians.get(key, 0) >= 0.1:
                        ratios.append(float(case.get("time") or 0) / medians[key])
        ratios.sort()
        print(f"{len(ratios)} passed Rust tests of fast runs with a full-run median of 0.1 s or more: time in the fast "
              f"run over that median is {ratios[len(ratios) // 10]:.2f} at the 10th percentile, "
              f"{st.median(ratios):.2f} at the median, {ratios[int(0.9 * len(ratios))]:.2f} at the 90th")
    every = [v for r in runs for v in r.values()]
    print(f"the line of every test's times: {line(every):.1f} s (largest {max(every):.1f} s, median {st.median(every):.3f} s)")


main()
