#!/usr/bin/env python3
"""Times the repeated paths of wf-agents on a synthetic database built at the field's scale and at ten times it.

Whom it serves: item #248 (the list of paths that run for every event, round, work or item) and, after it, the items
that fix those paths and want to run the same program again. It is wired to one caller, the `measure` tool's workflow
(`.github/workflows/measure.yml`), whose command is

    python3 -I research/repeated-paths/repeated_paths.py run --scales 1,10

and it is removed when the paths it times have each a scale test of their own in the code that owns them, or when
`research/repeated-paths.md` is deleted.

What it does, by mode:

  run        builds (or reuses) a database from `db/schema.sql` at each scale, then times every path of PATHS and every
             view on each, and prints one tab-separated line for each (see PATHTIME below);
  summarize  reads the log of a `run` (the measurement artifact's command.log) and prints the table of the report.

Scale 1 is the engine's own database as it stood on 2026-11-05 (365 works, 14843 rounds, 55886 changes: RECIPE below
holds each figure with where it was measured). Scale 10 is ten times as many works, items, rounds, tool runs, changes
and flow versions, each work and each row as large as at scale 1: the same engine run ten times as long. Rows of the
tables the research environment's `query` tool hides (pings, round_calls, round_measures, cache_misses) are not built, so
the paths that read them are timed as reading empty tables; the report says which.

Only the Python standard library is used. Nothing here writes outside the cache directory (REPEATED_PATHS_CACHE, else a
directory in RUNNER_TEMP, else in the temporary directory).
"""

import argparse
import bisect
import hashlib
import json
import math
import os
import random
import re
import shutil
import sqlite3
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCHEMA = ROOT / "db" / "schema.sql"
T0 = 1791446671000  # the first round of the field's database, in milliseconds
DAY = 86400000

# The field's figures at scale 1, measured with the engine's `query` tool on 2026-11-05 (read-only queries on the
# database, records R365-9 to R365-17 of work 365), or, where marked "assumed", not readable by that tool.
RECIPE = {
    "works": 365,  # 287 engineer, 76 architect, 2 policy or other (merged into the architect count below)
    "rounds": 14843,
    "tool_runs": 6736,
    "changes": 55886,
    "items": 311,
    "item_links": 288,
    "flow_versions": 56,
    "interjections": 374,
    "owner_asks": 92,
    "owner_messages": 470,
    "architect_matters": 903,
    "design_cards": 21,
    "blockers": 123,
    "observations": 280,
    "notebook_edits": 410,
    "architect_item_changes": 1833,
    "slot_waits": 34,
    "dispatch_queue": 22,
    "engine_notices": 250,  # assumed: hidden; the change log holds 318 changes to it
    "sessions": 300,  # assumed: hidden; the change log holds 1784 changes to it
    "conversation_reads": 30,  # assumed: hidden; the change log holds 1722 changes to it
}

# Tables whose rows grow with the engine's life (all of RECIPE but these); these stay as they are at every scale.
FIXED = {"dispatch_queue"}
# Figures of RECIPE that the research environment could not read, so that the built table is not held to them.
ASSUMED = {"engine_notices", "sessions", "conversation_reads"}

# cell, rounds, rounds with a session, mean input bytes, mean output bytes, sent back, counted. From the field.
CELLS = {
    "engineer": [
        ("X1", 1574, 0, 2658, 7281, 4, 11), ("T6", 640, 538, 12852, 1758, 13, 532), ("I1", 608, 608, 8420, 4169, 63, 459),
        ("T1", 588, 588, 7225, 2592, 35, 399), ("T4", 321, 321, 17472, 5506, 35, 115), ("T7", 304, 0, 11026, 50, 0, 1),
        ("M1", 300, 0, 20578, 2, 0, 15), ("R1", 289, 289, 7314, 3478, 1, 182), ("D2", 202, 201, 6644, 8558, 19, 33),
        ("D1", 178, 178, 7227, 3275, 0, 24), ("T2", 139, 55, 4250, 707, 1, 19), ("D3", 115, 72, 9649, 1299, 3, 54),
        ("T3", 111, 111, 8421, 2173, 6, 34), ("M2", 42, 0, 1339, 321, 0, 14), ("S1", 110, 0, 2, 232, 0, 0),
        ("D4", 109, 25, 1973, 2038, 0, 11), ("D6", 95, 87, 6069, 2058, 0, 6), ("D7", 90, 79, 6242, 1049, 1, 9),
        ("A4", 83, 83, 7300, 405, 0, 10), ("D8", 80, 11, 1183, 85, 0, 1), ("13", 75, 0, 2367, 4105, 1, 0),
        ("A1", 70, 70, 8772, 2694, 0, 39), ("D10", 70, 70, 6743, 644, 1, 9), ("L2", 66, 66, 6963, 1427, 1, 3),
        ("D5", 859, 600, 6500, 1500, 35, 170),
    ],
    "architect": [
        ("Q0", 1270, 0, 9008, 43, 0, 0), ("Q2", 1173, 0, 645, 2, 0, 0), ("B1", 756, 8, 14413, 21, 1, 653),
        ("X1", 634, 0, 769, 6806, 0, 0), ("Q1", 456, 450, 8005, 282, 0, 0), ("H5", 335, 329, 8059, 1116, 34, 83),
        ("H1", 326, 324, 8433, 1463, 9, 134), ("H3", 276, 271, 9205, 2928, 13, 105), ("H4", 262, 260, 7948, 810, 0, 202),
        ("H8", 235, 230, 8484, 2178, 3, 127), ("B2", 230, 230, 7952, 1667, 0, 122), ("H2", 210, 210, 8962, 856, 0, 188),
        ("U1", 181, 181, 9146, 2230, 8, 129), ("N1", 133, 133, 9120, 3147, 10, 75), ("X2", 124, 124, 6472, 685, 1, 1),
        ("B4", 97, 95, 7603, 675, 1, 94), ("G1", 83, 0, 3569, 2, 0, 57), ("S1", 83, 0, 4551, 2, 0, 0),
        ("N0", 82, 0, 4101, 43, 0, 0), ("B3", 65, 64, 7675, 7097, 2, 64), ("N2", 414, 330, 8000, 1500, 12, 160),
    ],
}
# The graph's last cell: a work at it with that round ended is done, and the engine does not mark it ended.
END = {"engineer": "E1", "architect": "R1", "policy": "R1"}

# tool, runs, mean result bytes
TOOLS = [("query", 4378, 1766), ("ci", 978, 4214), ("gh", 697, 6111), ("ci_before", 362, 7047), ("merge", 208, 390),
         ("full_ci", 65, 1019), ("watch_main", 39, 640), ("measure", 9, 618)]

# table, changes, mean old_row bytes (an update), mean new_row bytes: the field's change log by table.
CHANGES = [
    ("items", 25200, 4925, 4922), ("works", 15774, 354, 354), ("architect_matters", 3845, 15333, 12145),
    ("architect_item_changes", 3601, 564, 576), ("sessions", 1784, 149, 152), ("conversation_reads", 1722, 56, 56),
    ("owner_messages", 820, 376, 358), ("interjections", 706, 642, 639), ("observations", 636, 709, 694),
    ("item_links", 410, 172, 113), ("notebook_edits", 410, 0, 2065), ("blockers", 346, 555, 987),
    ("engine_notices", 318, 286, 273), ("owner_asks", 169, 3579, 3683), ("flow_versions", 56, 0, 1778600),
    ("design_cards", 51, 3986, 3955), ("dispatch_queue", 22, 27, 26),
]

LETTERS = "abcdefghijklmnopqrstuvwxyz      ,."


def log(msg):
    print("%s %s" % (time.strftime("%H:%M:%S"), msg), file=sys.stderr, flush=True)


# ---------------------------------------------------------------------------------------------------------------------
# The synthetic database
# ---------------------------------------------------------------------------------------------------------------------


class Sink:
    """Rows of one table, inserted in batches."""

    def __init__(self, con, table, columns):
        self.con = con
        self.sql = "INSERT INTO %s (%s) VALUES (%s)" % (table, ",".join(columns), ",".join("?" * len(columns)))
        self.table = table
        self.rows = []
        self.bytes = 0
        self.total = 0

    def add(self, row):
        self.rows.append(row)
        for v in row:
            if isinstance(v, str):
                self.bytes += len(v)
        if len(self.rows) >= 5000 or self.bytes >= 48_000_000:
            self.flush()

    def flush(self):
        if self.rows:
            self.con.executemany(self.sql, self.rows)
            self.total += len(self.rows)
            self.rows = []
            self.bytes = 0


class Builder:
    def __init__(self, path, scale, stretch=1):
        self.scale = scale
        # The architect's longest work (3330 rounds in the field) has this many times its rounds: it never ends, so it
        # grows with the engine's age by itself, apart from the number of works.
        self.stretch = stretch
        self.rnd = random.Random(248)
        self.pool = "".join(self.rnd.choice(LETTERS) for _ in range(1 << 20))
        self.members = ['"k%d":"%s"' % (i, self.txt(60)) for i in range(8192)]
        self.con = sqlite3.connect(path, isolation_level=None)
        self.con.execute("PRAGMA journal_mode=OFF")
        self.con.execute("PRAGMA synchronous=OFF")
        self.con.execute("PRAGMA cache_size=-1000000")
        self.sinks = {}
        self.cum = {}
        for g, cells in CELLS.items():
            acc = 0
            cum = []
            for c in cells:
                acc += c[1]
                cum.append(acc)
            self.cum[g] = cum

    # -- text -----------------------------------------------------------------------------------------------------
    def txt(self, n):
        n = max(0, min(n, len(self.pool) - 1))
        o = self.rnd.randrange(len(self.pool) - n)
        return self.pool[o:o + n]

    def sz(self, mean, sigma=0.6, cap=175000):
        """A size around `mean` with a long tail, as the field's blobs have."""
        v = mean * self.rnd.lognormvariate(-(sigma ** 2) / 2, sigma)
        return max(2, min(cap, int(v)))

    def blob(self, size, head=""):
        """A JSON object of about `size` bytes: `head` (members already in JSON), then members of 70 bytes."""
        m = max(0, (size - len(head)) // 71)
        m = min(m, len(self.members) - 1)
        o = self.rnd.randrange(len(self.members) - m) if m else 0
        body = ",".join(self.members[o:o + m])
        if head and body:
            return "{" + head + "," + body + "}"
        return "{" + head + body + "}"

    def n(self, x):
        return max(1, int(round(x * self.scale)))

    def sink(self, table, columns):
        s = Sink(self.con, table, columns.split(","))
        self.sinks[table] = s
        return s

    # -- the works --------------------------------------------------------------------------------------------------
    def work_specs(self):
        r = self.rnd
        rest = 127
        raw = [40 ** (j / (rest - 1)) for j in range(rest)]
        k = (7420 - 160 - 326) / (sum(raw) - raw[-1])
        multi = [max(2, int(round(k * x))) for x in raw[:-1]] + [326]
        states = ["done"] * 74 + ["ended"] * 41 + ["open"] * 8 + ["paused"] * 4
        r.shuffle(states)
        if states[-1] != "done":
            i = states.index("done")
            states[i], states[-1] = states[-1], states[i]
        eng = [("engineer", 1, "done")] * 160 + [("engineer", c, s) for c, s in zip(multi, states)]
        big = [(3330, "open"), (502, "ended"), (449, "open"), (273, "done"), (209, "open")]
        rest_counts = [r.randint(10, 65) for _ in range(71)]
        rest_counts[-1] += 2662 - sum(rest_counts)
        rest_counts[-1] = max(5, rest_counts[-1])
        rest_states = ["done"] * 15 + ["ended"] * 3 + ["open"] * 43 + ["paused"] * 10
        r.shuffle(rest_states)
        arch = [("architect", c, s) for c, s in big] + [("architect", c, s) for c, s in zip(rest_counts, rest_states)]
        base = eng + arch
        if self.stretch != 1:
            base = [(g, c * self.stretch if (g == "architect" and c == 3330) else c, s) for g, c, s in base]
        out = []
        copies = max(1, int(round(self.scale)))
        for _ in range(copies):
            r.shuffle(base)
            out.extend(base)
        if self.scale < 1:
            out = r.sample(base, max(2, int(round(len(base) * self.scale))))
        return out

    def build(self):
        r = self.rnd
        t_start = time.time()
        self.con.executescript(SCHEMA.read_text())
        self.con.execute("BEGIN")
        specs = self.work_specs()
        nwork = len(specs)
        n_items = self.n(RECIPE["items"])
        versions = self.n(RECIPE["flow_versions"])
        eng_with_item = max(1, int(round(269 * self.scale))) if self.scale >= 1 else max(1, n_items * 269 // 311)
        works = self.sink(
            "works",
            "id,item,flow_version,graph,at,waiting,recorded,machine,worktree,branch,pr,pushed_head,paused,ended,"
            "paused_by,project",
        )
        rounds = self.sink(
            "rounds",
            "work,n,cell,session,input,output,transition,rejected,config_commit,model,usage,compactions,started_at,"
            "ended_at,counts",
        )
        sessions = self.sink("sessions", "id,work,line,cli,cli_session,ended,started")
        info = {
            "specs": nwork, "n_items": n_items, "versions": versions,
            "w_eng": None, "w_eng_big": None, "w_arch": None, "eng_cli": None, "arch_cli": None,
            "rej_round": None, "item": None, "matter": None, "w_ci": None, "t_end": T0,
        }
        self.info = info
        session_id = 0
        sess_of = {}  # (work, line) -> (id, cli_session)
        x1_rounds = []
        agent_eng = []
        agent_arch = []
        arch_rounds = []
        works_of_item = {}
        first_work_of_item = {}
        eng_index = 0
        arch_index = 0
        best_eng = (10 ** 9, None)
        biggest_eng = (0, None)
        biggest_arch = (0, None)
        eng_with_rounds = {}
        total_rounds = 0
        merged = 0
        for wid, (graph, nr, state) in enumerate(specs, 1):
            cells = CELLS[graph]
            cum = self.cum[graph]
            total = cum[-1]
            item = None
            if graph == "engineer":
                item = 1 + (eng_index % eng_with_item) if n_items >= 1 else None
                item = min(item, n_items)
                eng_index += 1
                works_of_item.setdefault(item, []).append(wid)
                first_work_of_item.setdefault(item, wid)
            else:
                chain = arch_index // 12
                arch_index += 1
            t = T0 + wid * 600000
            merge_here = graph == "engineer" and state == "done" and nr > 2 and r.random() < 68 / 74
            last_cell = None
            sess_here = {}
            for n in range(1, nr + 1):
                final = n == nr
                transition = "done"
                rejected = None
                head_out = '"progress":"%s"' % self.txt(60)
                if state == "done" and final and graph == "engineer":
                    spec = ("E1", 1, 0, 263, 208, 0, 0)
                    transition = None
                elif state == "done" and final:
                    spec = (END[graph], 1, 0, 3000, 200, 0, 0)
                    transition = None
                elif merge_here and n == nr - 1:
                    spec = ("M2", 1, 0, 1339, 321, 0, 0)
                    transition = "merged"
                    head_out += ',"result":{"merge_commit":"%040x"}' % r.getrandbits(160)
                    merged += 1
                else:
                    spec = cells[bisect.bisect(cum, r.random() * total)]
                    if r.random() < spec[5] / spec[1]:
                        transition = None
                        rejected = '{"check":"%s","problems":[{"check":"x","field":null,"text":"%s"}]}' % (
                            self.txt(40), self.txt(self.sz(440)))
                cell = spec[0]
                agent = spec[2] > 0 and r.random() < spec[2] / spec[1]
                session = None
                usage = None
                model = None
                if agent:
                    line = ("review" if cell in ("T1", "T3", "T4") else "engineer") if graph == "engineer" else "architect"
                    key = (wid, line)
                    if key not in sess_of:
                        session_id += 1
                        cli = "c%d-%s" % (wid, line) if graph == "engineer" else "arch-%d" % chain
                        sess_of[key] = (session_id, cli)
                        ended = None if state in ("open",) else t
                        sessions.add((session_id, wid, line, "claude", cli, ended, "line"))
                    session = sess_of[key][0]
                    sess_here[line] = sess_of[key]
                    model = "claude-opus"
                    usage = '{"cost_usd":%.4f,"usage":{"input_tokens":%d,"cache_read_input_tokens":%d,' \
                            '"cache_creation_input_tokens":%d},"pad":"%s"}' % (
                                r.random() * 2, r.randint(1, 900), r.randint(1000, 200000), r.randint(0, 20000),
                                self.txt(1250))
                counts = None
                if rejected is not None:
                    counts = "sent_back"
                elif transition is not None and r.random() < spec[6] / spec[1]:
                    counts = r.choice(["reviews", "asks", "rows"])
                head_in = '"call":{"level":"engineer","command":"claude"},"engine":{"interjections":[]}' if agent \
                    else '"engine":{}'
                t += r.randint(1000, 90000)
                started = t
                ended_at = t + r.randint(500, 600000)
                rounds.add((
                    wid, n, cell, session, self.blob(self.sz(spec[3]), head_in), self.blob(self.sz(spec[4]), head_out),
                    transition, rejected, "0" * 40, model, usage, 0, started, ended_at, counts,
                ))
                total_rounds += 1
                last_cell = cell
                if cell == "X1" and graph == "engineer":
                    x1_rounds.append((wid, n))
                if agent and graph == "engineer" and total_rounds % 5 == 0:
                    agent_eng.append((wid, n))
                if graph == "architect" and total_rounds % 6 == 0:
                    arch_rounds.append((wid, n))
                if rejected is not None and graph == "engineer" and nr >= 20 and info["rej_round"] is None:
                    info["rej_round"] = (wid, n)
            info["t_end"] = max(info["t_end"], t)
            # the work's own row
            if graph == "engineer":
                at = last_cell
            elif state in ("open", "paused"):
                at = "E2"
            else:
                at = last_cell
            paused = "paused by the engine" if state == "paused" else None
            ended_text = "item cancelled: synthetic" if state == "ended" else None
            version = 1 + min(versions - 1, int(versions * (wid - 1) / max(1, nwork)))
            works.add((
                wid, item, version, graph, at, "[]", '{"ci":"passed"}', "m1",
                "/work/%d" % wid if state in ("open", "paused") else None, "wf-agents/%d" % wid,
                1000 + wid if merge_here else None, "%040x" % r.getrandbits(160) if merge_here else None,
                paused, ended_text, "engine" if paused else None, "wf-agents" if graph == "architect" else None,
            ))
            if graph == "engineer":
                eng_with_rounds[wid] = nr
                if nr >= 10 and ("engineer" in sess_here) and abs(nr - 86) < best_eng[0]:
                    best_eng = (abs(nr - 86), wid)
                if nr > biggest_eng[0]:
                    biggest_eng = (nr, wid)
            else:
                if nr > biggest_arch[0]:
                    biggest_arch = (nr, wid)
        rounds.flush()
        works.flush()
        sessions.flush()
        info["w_eng"] = best_eng[1] or biggest_eng[1]
        info["w_eng_big"] = biggest_eng[1]
        info["w_arch"] = biggest_arch[1]
        for (w, line), (sid, cli) in sess_of.items():
            if w == info["w_eng"] and line == "engineer":
                info["eng_cli"] = cli
            if w == info["w_arch"] and line == "architect":
                info["arch_cli"] = cli
        log("works, rounds, sessions done: %d works, %d rounds (%.0fs)" % (nwork, total_rounds, time.time() - t_start))
        self.con.execute("COMMIT")
        self.con.execute("BEGIN")

        # -- items, links, queue --------------------------------------------------------------------------------------
        items = self.sink(
            "items",
            "id,project,title,problem,why,approach,deliver,level,acceptance,acceptance_work,acceptance_round,cancelled,"
            "held,rank",
        )
        for i in range(1, n_items + 1):
            fw = first_work_of_item.get(i)
            acc = None
            if fw is not None and r.random() < 91 / 311:
                acc = '[{"by":"test","id":"A1","condition":"%s"}]' % self.txt(self.sz(1300))
            items.add((
                i, "wf-agents", self.txt(50), self.txt(self.sz(257)), self.txt(200),
                self.txt(300) if r.random() < 0.5 else None, "report" if r.random() < 0.1 else "change",
                r.choice(["L1", "L2", "L3"]), acc, fw if acc else None, 1 if acc else None,
                "cancelled: synthetic" if r.random() < 0.13 else None, "held" if r.random() < 0.03 else None,
                r.randint(1, 400) if r.random() < 0.6 else None,
            ))
        items.flush()
        links = self.sink("item_links", "item,depends_on,needs,kind")
        seen = set()
        for _ in range(self.n(RECIPE["item_links"])):
            a = r.randint(2, n_items) if n_items >= 2 else 1
            b = r.randint(1, a - 1) if a > 1 else 1
            if a == b or (a, b) in seen:
                continue
            seen.add((a, b))
            links.add((a, b, self.txt(80), "blocking" if r.random() < 0.7 else "parallel"))
        links.flush()
        queue = self.sink("dispatch_queue", "item,position")
        for i in range(1, min(22, n_items) + 1):
            queue.add((n_items + 1 - i, float(i)))
        queue.flush()
        counts = {}
        for item, ws in works_of_item.items():
            counts[item] = len(ws)
        if counts:
            info["item"] = max(counts, key=lambda k: (counts[k], -k))

        # -- tool runs --------------------------------------------------------------------------------------------------
        runs = self.sink("tool_runs", "work,round,tool,args,attempt,result,files,queued_at,started_at,ended_at")
        per_work_ci = {}
        total_tools = self.n(RECIPE["tool_runs"])
        weights = [t[1] for t in TOOLS]
        for _ in range(total_tools):
            tool, _, mean = r.choices(TOOLS, weights=weights)[0]
            if not x1_rounds:
                break
            w, n = x1_rounds[r.randrange(len(x1_rounds))]
            if tool == "query":
                args = '{"sql":"%s"}' % self.txt(self.sz(400))
            elif tool == "gh":
                args = '{"command":"%s"}' % self.txt(self.sz(120))
            else:
                args = self.blob(self.sz(700), '"head":"%040x"' % r.getrandbits(160))
            result = self.blob(self.sz(mean, 1.0, 160000), '"verdict":"ran"')
            ts = T0 + w * 600000 + n * 1000
            runs.add((w, n, tool, args, 1, result, "[]", ts, ts, ts + r.randint(100, 120000)))
            if tool == "ci":
                per_work_ci[w] = per_work_ci.get(w, 0) + 1
        runs.flush()
        if per_work_ci:
            info["w_ci"] = max(per_work_ci, key=lambda k: (per_work_ci[k], -k))
        log("tool runs done (%.0fs)" % (time.time() - t_start))

        # -- the other tables the paths read ------------------------------------------------------------------------
        n_arch_rounds = len(arch_rounds)
        interj = self.sink("interjections", "work,line,speaker,declared,text,delivered_round,informs")
        for _ in range(self.n(RECIPE["interjections"])):
            if not agent_eng:
                break
            w, n = agent_eng[r.randrange(len(agent_eng))]
            interj.add((w, "engineer", r.choice(["owner", "architect"]), "owner", self.txt(self.sz(464)),
                        n if r.random() < 0.89 else None, 0))
        interj.flush()
        asks = self.sink("owner_asks", "work,round,body,answer")
        for i in range(self.n(RECIPE["owner_asks"])):
            if not agent_eng:
                break
            w, n = agent_eng[r.randrange(len(agent_eng))]
            asks.add((w, n, self.blob(self.sz(3500), '"kind":"points","question":"%s"' % self.txt(80)),
                      None if i % 7 == 0 else '{"verdict":"ok"}'))
        asks.flush()
        msgs = self.sink("owner_messages", "work,round,text,read_at,design_card")
        for _ in range(self.n(RECIPE["owner_messages"])):
            if not agent_eng:
                break
            w, n = agent_eng[r.randrange(len(agent_eng))]
            msgs.add((w, n, self.txt(self.sz(268)), T0 if r.random() < 0.9 else None, None))
        msgs.flush()
        notices = self.sink("engine_notices", "work,text,at,read_at,ask,fault,reminder")
        for i in range(self.n(RECIPE["engine_notices"])):
            notices.add((r.randint(1, nwork), self.txt(self.sz(280)), T0 + i * 10000, T0, None,
                         None if r.random() < 0.9 else "fault", 1 if i % 25 == 0 else 0))
        notices.flush()
        # kind, matters, handled, on a work, with a reply, asked, withdrawn, mean body bytes: the field's, by kind
        kinds = [("report", 271, 268, 220, 0, 3, 1, 1594), ("set_straight", 180, 148, 169, 0, 10, 30, 621),
                 ("owner_words", 131, 131, 0, 0, 0, 0, 402), ("open_question", 127, 123, 127, 0, 28, 4, 850),
                 ("make_plan", 57, 57, 57, 0, 0, 0, 239), ("engineer_question", 37, 35, 37, 35, 0, 2, 7817),
                 ("slots_free", 30, 30, 0, 0, 0, 0, 21), ("group_talk", 27, 27, 0, 0, 0, 0, 10),
                 ("failure", 16, 14, 16, 14, 0, 2, 4750), ("design_card", 15, 11, 15, 0, 0, 6, 17),
                 ("check_policy", 13, 12, 13, 0, 9, 0, 239)]
        kind_weights = [k[1] for k in kinds]
        matters = self.sink(
            "architect_matters",
            "project,kind,detail,what,body,item,work,round,at,handled_work,handled_round,reply,asked,resume_cell,"
            "withdrawn",
        )
        n_matters = self.n(RECIPE["architect_matters"])
        n_asks = self.n(RECIPE["owner_asks"])
        for i in range(n_matters):
            kind, total, handled, on_work, replied, asked, withdrawn, body_mean = r.choices(kinds, weights=kind_weights)[0]
            has_work = bool(agent_eng) and r.random() < on_work / total
            w, n = agent_eng[r.randrange(len(agent_eng))] if has_work else (None, None)
            hw, hn = arch_rounds[r.randrange(n_arch_rounds)] if (arch_rounds and r.random() < handled / total) \
                else (None, None)
            body = self.blob(self.sz(body_mean, 1.2, 132000),
                             '"text":"%s","where":"work","work":%d,"kind":"%s"' % (self.txt(min(100, body_mean)),
                                                                                  w or 0, kind))
            reply = '{"text":"%s","by":"architect","at":%d}' % (self.txt(self.sz(400)), T0) \
                if has_work and r.random() < replied / total else None
            ask = r.randint(1, n_asks) if r.random() < asked / total else None
            matters.add(("wf-agents", kind, None, self.txt(100), body, None, w, n, T0 + i * 20000, hw, hn, reply, ask,
                         "A3" if ask else None, "withdrawn" if r.random() < withdrawn / total else None))
        matters.flush()
        info["matter"] = 1 if n_matters else None
        cards = self.sink("design_cards", "work,round,key,body,status")
        for i in range(self.n(RECIPE["design_cards"])):
            if not agent_eng:
                break
            w, n = agent_eng[r.randrange(len(agent_eng))]
            cards.add((w, n, "P%d" % (i + 1), self.blob(self.sz(3900)), "ruled" if i % 3 else "open"))
        cards.flush()
        blockers = self.sink("blockers", "work,cell,round,kind,what,holder,project,at,since,ended,outcome,ended_at")
        for i in range(self.n(RECIPE["blockers"])):
            if not agent_eng:
                break
            w, n = agent_eng[r.randrange(len(agent_eng))]
            open_ = i == 0
            blockers.add((w, "A2", n, "question", self.txt(200), "architect", "wf-agents", T0 + i * 1000, T0 + i * 1000,
                          None if open_ else "resolved", None if open_ else "{}", None if open_ else T0 + i * 2000))
        blockers.flush()
        obs = self.sink("observations", "project,title,what,source,times_reported")
        for i in range(self.n(RECIPE["observations"])):
            obs.add(("wf-agents", self.txt(60), self.txt(self.sz(700)), self.txt(40), 1))
        obs.flush()
        nb = self.sink("notebook_edits", "project,section,text,work,round")
        aitems = self.sink("architect_item_changes", "work,round,matter,change,state,outcome")
        if arch_rounds:
            for i in range(self.n(RECIPE["notebook_edits"])):
                w, n = arch_rounds[r.randrange(n_arch_rounds)]
                nb.add(("wf-agents", "plan", self.txt(self.sz(2065)), w, n))
            for i in range(self.n(RECIPE["architect_item_changes"])):
                w, n = arch_rounds[r.randrange(n_arch_rounds)]
                aitems.add((w, n, r.randint(1, n_matters), '{"title":"%s"}' % self.txt(self.sz(400)),
                            "collected" if i % 40 == 0 else "made", None))
        nb.flush()
        aitems.flush()
        waits = self.sink("slot_waits", "work,since,until,ended")
        for i in range(self.n(RECIPE["slot_waits"])):
            waits.add((r.randint(1, nwork), T0 + i * 1000, T0 + i * 1000 + 5000, "granted"))
        waits.flush()
        reads = self.sink("conversation_reads", "conversation,at")
        for i in range(self.n(RECIPE["conversation_reads"])):
            reads.add(("work:%d" % (i + 1), T0))
        reads.flush()
        runs_t = self.sink("engine_runs", "started_at,built_from,config_commit,flow_version")
        for i in range(2):
            runs_t.add((T0 + i, "x" * 40, "y" * 40, 1))
        runs_t.flush()

        # -- flow versions: the compiled flow is a megabyte or more of JSON (graphs, cells, rows) --------------------
        fv = self.sink("flow_versions", "digest,compiled")
        compiled_texts = []
        cell_names = {g: [c[0] for c in cells] for g, cells in CELLS.items()}
        graphs = []
        for g in ("engineer", "architect"):
            cs = []
            for c in cell_names[g]:
                rows = ",".join('{"name":"%s","counts":%s}' % (nm, '"reviews"' if nm != "done" else "null")
                                for nm in ("done", "sent_back", "asked", "merged", "ready", "stop"))
                cs.append('{"id":"%s","rows":[%s],"pad":"%s"}' % (c, rows, self.txt(12000)))
            graphs.append('{"name":"%s","end":"%s","cells":[%s]}' % (g, END[g], ",".join(cs)))
        # cells besides the two graphs', to bring a version to about the field's mean size of 1.78 MB
        extra = ",".join('{"id":"X%d","rows":[],"pad":"%s"}' % (j, self.txt(12000)) for j in range(110))
        tail = '"graphs":[%s],"extra":[%s]}' % (",".join(graphs), extra)
        for i in range(versions):
            text = '{"version":%d,%s' % (i, tail)
            fv.add(("digest-%d" % i, text))
            if len(compiled_texts) < 8:
                compiled_texts.append(text)
        fv.flush()
        log("tables done (%.0fs)" % (time.time() - t_start))

        # -- the change log ---------------------------------------------------------------------------------------------
        ch = self.sink("changes", "tx,at,actor,table_name,row_key,old_row,new_row")
        table_rows = {
            "items": n_items, "works": nwork, "architect_matters": n_matters,
            "architect_item_changes": self.n(RECIPE["architect_item_changes"]), "sessions": session_id,
            "conversation_reads": self.n(RECIPE["conversation_reads"]), "owner_messages": self.n(RECIPE["owner_messages"]),
            "interjections": self.n(RECIPE["interjections"]), "observations": self.n(RECIPE["observations"]),
            "item_links": len(seen), "notebook_edits": self.n(RECIPE["notebook_edits"]),
            "blockers": self.n(RECIPE["blockers"]), "engine_notices": self.n(RECIPE["engine_notices"]),
            "owner_asks": self.n(RECIPE["owner_asks"]), "flow_versions": versions,
            "design_cards": self.n(RECIPE["design_cards"]), "dispatch_queue": 22,
        }
        specs_c = []
        for table, total, ao, an in CHANGES:
            nrows = table_rows[table]
            n_changes = total if table in FIXED else self.n(total)
            if table in FIXED:
                n_changes = total
            inserts = min(nrows, n_changes)
            for k in range(n_changes):
                if k < inserts:
                    specs_c.append((table, k + 1, False, ao, an))
                else:
                    specs_c.append((table, r.randint(1, max(1, nrows)), True, ao, an))
        r.shuffle(specs_c)
        tx = 0
        at = T0
        for i, (table, rid, update, ao, an) in enumerate(specs_c):
            if i % 3 == 0:
                tx += 1
                at += r.randint(50, 5000)
            key = '{"id":%d}' % rid
            if table == "flow_versions":
                new = compiled_texts[(rid - 1) % len(compiled_texts)]
                old = None
            elif table == "works":
                new = '{"id":%d,"at":"%s","paused":%s,"ended":null,"pr":null,"pad":"%s"}' % (
                    rid, r.choice(["X1", "I1", "T6", "E1"]), "null" if r.random() < 0.98 else '"x"', self.txt(260))
                old = new.replace('"at":"', '"at":"p') if update else None
            elif table == "items":
                lv = r.choice(["L1", "L2", "L3"])
                new = self.blob(self.sz(an), '"id":%d,"level":"%s"' % (rid, lv))
                old = self.blob(self.sz(ao), '"id":%d,"level":"%s"' % (rid, lv if r.random() > 0.015 else "L1")) \
                    if update else None
            else:
                new = self.blob(self.sz(an), '"id":%d' % rid)
                old = self.blob(self.sz(ao), '"id":%d' % rid) if update else None
            ch.add((tx, at, "engine", table, key, old, new))
        ch.flush()
        log("changes done (%.0fs)" % (time.time() - t_start))

        # -- what the paths need to know ------------------------------------------------------------------------------
        bp = self.con.execute("CREATE TABLE bench_params (name TEXT PRIMARY KEY, value TEXT)")
        intended = {}
        for name, s in self.sinks.items():
            intended[name] = s.total
        info["intended"] = intended
        info["scale"] = self.scale
        info["rowid_max"] = total_rounds
        info["merged"] = merged
        for k, v in info.items():
            self.con.execute("INSERT INTO bench_params VALUES (?, ?)", (k, json.dumps(v)))
        self.con.execute("COMMIT")
        self.con.close()
        log("built in %.0fs" % (time.time() - t_start))


def parse_scale(label):
    """A scale is written 10 (ten times the field), 0.2, or 1L10: the field, with its longest architect work stretched
    ten times. Gives the scale and the stretch."""
    if "L" in label:
        base, stretch = label.split("L", 1)
        return float(base), int(stretch)
    return float(label), 1


def database(label, cache):
    """The path of the database at the scale `label` names, built if it is not in the cache."""
    scale, stretch = parse_scale(label)
    key = hashlib.sha1((Path(__file__).read_bytes() + SCHEMA.read_bytes())).hexdigest()[:10]
    cache.mkdir(parents=True, exist_ok=True)
    path = cache / ("scale-%s-%s.db" % (label, key))
    if path.exists():
        return path
    tmp = Path(str(path) + ".tmp")
    if tmp.exists():
        tmp.unlink()
    log("building %s" % path.name)
    Builder(str(tmp), scale, stretch).build()
    tmp.rename(path)
    return path


# ---------------------------------------------------------------------------------------------------------------------
# The paths
# ---------------------------------------------------------------------------------------------------------------------

COLS = "r.work, r.n, r.cell, r.session, r.input, r.output, r.transition, r.rejected, r.compactions, r.started_at, " \
       "r.ended_at, r.usage"

ROUND = """SELECT r.n, r.cell, s.line, r.transition, r.rejected, r.started_at, r.ended_at, r.model,
       json_extract(r.usage, '$.cost_usd'), r.compactions,
       (SELECT count(*) FROM tool_runs t WHERE t.work = r.work AND t.round = r.n),
       json_extract(r.output, '$.progress'),
       r.work, w.flow_version, w.graph, s.cli, s.cli_session, r.config_commit, r.input, r.output, r.usage,
       json_extract(r.input, '$.engine.not_applicable'), r.session, json_extract(r.input, '$.call.level'),
       json_extract(r.input, '$.call.command'),
       CASE WHEN s.started <> 'carried'
         AND r.n = (SELECT min(x.n) FROM rounds x WHERE x.work = r.work AND x.session = r.session)
       THEN s.started END,
       CASE WHEN r.session IS NULL THEN json_extract(r.input, '$.engine.new_session') END,
       r.set_aside
FROM rounds r JOIN works w ON w.id = r.work LEFT JOIN sessions s ON s.id = r.session"""

WORK_ROW = ("SELECT id, item, flow_version, graph, at, resume_at, waiting, recorded, machine, worktree, branch, pr, "
            "pushed_head, paused, ended, paused_by, project FROM works WHERE id = ?")


def size_of(rows):
    n = 0
    for r in rows:
        for v in r:
            if isinstance(v, (str, bytes)):
                n += len(v)
            elif v is not None:
                n += 8
    return n


def sql_path(*statements):
    """A path made of statements run in turn, each given the named parameters P."""

    def run(conn, P):
        rows = 0
        size = 0
        for s in statements:
            got = conn.execute(s, P).fetchall()
            rows += len(got)
            size += size_of(got)
        return rows, size

    run.statements = statements
    return run


def wk(conn, wid):
    """`Engine::work`: the work's row, its project (twice) and the head of its rounds' transitions."""
    w = conn.execute(WORK_ROW, (wid,)).fetchone()
    if w[1] is not None:
        conn.execute("SELECT project FROM items WHERE id = ?", (w[1],)).fetchone()
    conn.execute(WORK_ROW, (wid,)).fetchone()
    conn.execute("SELECT cell, transition FROM rounds WHERE work = ? AND transition IS NOT NULL ORDER BY n DESC",
                 (wid,)).fetchone()
    return w


def done(conn, w):
    """`Engine::done`: at the graph's end cell, and a round there ended without being sent back."""
    if w[1] is not None:
        conn.execute("SELECT project FROM items WHERE id = ?", (w[1],)).fetchone()
    if w[4] != END.get(w[3]):
        return False
    rows = conn.execute("SELECT %s FROM rounds r WHERE r.work = ? AND r.cell = ? ORDER BY r.n DESC" % COLS,
                        (w[0], w[4])).fetchall()
    return any(r[10] is not None and r[7] is None for r in rows)


def active_ids(conn):
    """`Engine::active`: every work not ended and not paused, loaded, with `done` asked of each. Gives the works not
    done, as (id, graph), and how many it loaded."""
    ids = [r[0] for r in conn.execute("SELECT id FROM works WHERE ended IS NULL AND paused IS NULL ORDER BY id")]
    live = []
    for i in ids:
        w = wk(conn, i)
        if not done(conn, w):
            live.append((i, w[3]))
    return live, len(ids)


def active_pass(conn, P):
    live, loaded = active_ids(conn)
    return loaded, len(live)


def slots_pass(conn, P):
    """`Engine::slots_by_machine` (free_slots, room_for): `active` again, then each active engineer work loaded and asked
    whether it waits (its last round)."""
    live, loaded = active_ids(conn)
    for i, graph in live:
        if graph == "engineer":
            wk(conn, i)
            conn.execute("SELECT cell, transition FROM rounds WHERE work = ? ORDER BY n DESC LIMIT 1", (i,)).fetchone()
    return loaded, len(live)


def unended_pass(conn, P):
    """The scan of `keep_warm`: every work not ended (paused ones too), loaded, `done` asked, and for a work not done
    the last round read."""
    ids = [r[0] for r in conn.execute("SELECT id FROM works WHERE ended IS NULL ORDER BY id")]
    for i in ids:
        w = wk(conn, i)
        if not done(conn, w):
            conn.execute("SELECT cell, transition FROM rounds WHERE work = ? ORDER BY n DESC LIMIT 1", (i,)).fetchone()
    return len(ids), 0


def event_lookups(conn, P):
    """`api::events`: for each of 200 changed rows, one point lookup of its round and one of its work."""
    top = P["rowid_max"]
    rnd = random.Random(7)
    for _ in range(200):
        row = conn.execute("SELECT work, n FROM rounds WHERE rowid = ?", (rnd.randint(1, top),)).fetchone()
        if row:
            conn.execute("SELECT graph IN ('architect', 'policy') FROM works WHERE id = ?", (row[0],)).fetchone()
    return 200, 0


def job_open(conn, P):
    """`Db::open` of a forked job: a connection, its pragmas and the migrations check."""
    c = sqlite3.connect(P["_uri"], uri=True)
    c.execute("PRAGMA query_only=ON")
    c.execute("SELECT name FROM migrations").fetchall()
    c.execute("PRAGMA user_version").fetchone()
    c.close()
    return 1, 0


def reload_config(conn, P):
    """`config::committed`: `git rev-parse`, `git ls-tree -r` and one `git show` for each file of the configuration,
    run as the engine runs them (a process each), on a repository of the same number of files. 16 files are assumed:
    the engine's data directory is not readable from the research environment."""
    d = Path(tempfile.mkdtemp(prefix="rp-config-"))
    env = dict(os.environ, GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_SYSTEM="/dev/null")

    def git(*a):
        return subprocess.run(["git", "-C", str(d)] + list(a), check=True, capture_output=True, env=env).stdout

    git("init", "-q")
    files = ["install.toml", "projects/wf-agents.toml"] + ["shared/s%d.toml" % i for i in range(2)] + \
            ["rules/r%d.md" % i for i in range(12)]
    for f in files:
        p = d / f
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text("x = 1\n" * 40)
    git("add", ".")
    git("-c", "user.name=x", "-c", "user.email=x@x", "commit", "-q", "-m", "c")
    P["_config_repo"] = d
    return 1, 0


def reload_config_run(conn, P):
    d = P["_config_repo"]
    env = dict(os.environ, GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_SYSTEM="/dev/null")

    def git(*a):
        return subprocess.run(["git", "-C", str(d)] + list(a), check=True, capture_output=True, env=env).stdout

    git("rev-parse", "--verify", "HEAD")
    names = git("ls-tree", "-r", "--name-only", "HEAD").decode().split()
    for nme in names:
        git("show", "HEAD:" + nme)
    return len(names) + 2, 0


def build_paths():
    """Every path timed: (id, area, what, function). A statement uses the named parameters of the database (:w_eng...)."""
    p = []

    def add(pid, area, what, fn):
        p.append((pid, area, what, fn))

    # -- engine: once per turn of the loop (a turn is a job's end, or 10 seconds) -------------------------------------
    add("engine.turn.active_pass", "engine", "Engine::active: Work::load and done for each work not ended and not paused",
        active_pass)
    add("engine.turn.slots_pass", "engine", "Engine::slots_by_machine: active again, then the waiting of each active "
        "engineer work (called by free_slots in queue_due, start_ready and status)", slots_pass)
    add("engine.turn.stopping", "engine", "db::stopping (the page's status): the works with a stop asked",
        sql_path("SELECT w.id FROM works w WHERE w.ended IS NULL AND w.at NOT IN ('E1') AND EXISTS (SELECT 1 FROM "
                 "outbox o WHERE (o.kind = 'start_over' AND o.work = w.id) OR (o.kind = 'cancel_item' AND "
                 "json_extract(o.args, '$.item') = w.item)) ORDER BY w.id"))
    add("engine.turn.keep_warm_scan", "engine", "keep_warm::due: the same for each work not ended, paused ones too",
        unended_pass)
    add("engine.turn.reload_config", "engine", "config::committed: git rev-parse, ls-tree and show for each file",
        reload_config_run)
    add("engine.turn.job_open", "engine", "Db::open of a forked job: a connection and the migrations check", job_open)
    add("engine.turn.ready_items", "engine", "start_ready / queue_due: the ready_items view",
        sql_path("SELECT item, project, position FROM ready_items"))
    add("engine.turn.reminder", "engine", "reminders::due: the last reminder, then the oldest waiting ask",
        sql_path("SELECT max(at) FROM engine_notices WHERE reminder = 1",
                 "SELECT id, at, count(*) OVER () FROM conversation_entries WHERE source = 'ask' AND waiting "
                 "ORDER BY at, id LIMIT 1"))
    add("engine.turn.architect_queue", "engine", "architect::start: the project's unhandled matters",
        sql_path("SELECT q.*, m.body AS body FROM architect_queue q JOIN architect_matters m ON m.id = q.matter "
                 "WHERE q.project = 'wf-agents' AND q.kind <> 'group_talk' "
                 "AND NOT (q.kind IN ('report', 'other_architect') AND q.detail IS 'flow_feedback') "
                 "ORDER BY q.at, q.matter"))
    add("engine.turn.slots_free_history", "engine", "queue_due: the last handled slots_free matter",
        sql_path("SELECT r.ended_at, m.body FROM architect_matters m JOIN rounds r ON r.work = m.handled_work "
                 "AND r.n = m.handled_round WHERE m.project = 'wf-agents' AND m.kind = 'slots_free' "
                 "AND r.ended_at IS NOT NULL ORDER BY r.ended_at DESC LIMIT 1"))
    add("engine.turn.architect_running", "engine", "architect::running: the project's running architect work",
        sql_path("SELECT max(id) FROM works WHERE graph = 'architect' AND project = 'wf-agents' AND ended IS NULL"))
    add("engine.turn.main_watch_matters", "engine", "main_watch::look: matters of one commit",
        sql_path("SELECT count(*) FROM architect_matters WHERE project = 'wf-agents' AND kind = 'main_red' "
                 "AND json_extract(body, '$.commit') = 'none'"))
    # -- engine: once per write ---------------------------------------------------------------------------------------
    add("engine.write.next_tx", "engine", "Db::write: the next transaction number",
        sql_path("SELECT coalesce(max(tx), 0) + 1 FROM changes"))
    # -- engine: once per step of a work ------------------------------------------------------------------------------
    add("engine.step.stop_asked", "engine", "stop_asked: the outbox scanned for the work",
        sql_path("SELECT EXISTS (SELECT 1 FROM outbox o WHERE (o.kind = 'start_over' AND o.work = w.id) "
                 "OR (o.kind = 'cancel_item' AND json_extract(o.args, '$.item') = w.item)) FROM works w "
                 "WHERE w.id = :w_eng"))
    add("engine.step.take_if_past", "engine", "loops::take_if_past: the work's last rounds",
        sql_path("SELECT session IS NOT NULL, cell, started_at, n FROM rounds WHERE work = :w_eng ORDER BY n DESC "
                 "LIMIT 10"))
    add("engine.step.resumed_at", "engine", "work::resumed_at: when the work was last let go on (twice per agent round)",
        sql_path("SELECT max(at) FROM changes WHERE table_name = 'works' AND json_extract(row_key, '$.id') = :w_eng "
                 "AND json_extract(old_row, '$.paused') IS NOT NULL AND json_extract(new_row, '$.paused') IS NULL"))
    add("engine.step.open_round.eng", "engine", "work::open_round of an engineer work (86 rounds)",
        sql_path("SELECT %s FROM rounds r WHERE r.work = :w_eng AND r.ended_at IS NULL ORDER BY r.n DESC LIMIT 1" % COLS))
    add("engine.step.open_round.arch", "engine", "work::open_round of the architect's longest work (3330 rounds)",
        sql_path("SELECT %s FROM rounds r WHERE r.work = :w_arch AND r.ended_at IS NULL ORDER BY r.n DESC LIMIT 1" % COLS))
    # -- engine: once per round ---------------------------------------------------------------------------------------
    add("engine.round.rounds.eng", "engine", "work::rounds of an engineer work (86 rounds): every round, columns read",
        sql_path("SELECT %s FROM rounds r WHERE r.work = :w_eng ORDER BY r.n" % COLS))
    add("engine.round.rounds.eng_big", "engine", "work::rounds of the longest engineer work (326 rounds)",
        sql_path("SELECT %s FROM rounds r WHERE r.work = :w_eng_big ORDER BY r.n" % COLS))
    add("engine.round.rounds.arch", "engine", "work::rounds of the architect's longest work (3330 rounds)",
        sql_path("SELECT %s FROM rounds r WHERE r.work = :w_arch ORDER BY r.n" % COLS))
    add("engine.round.rounds_at", "engine", "work::rounds_at: the work's rounds at one cell (called many times a round)",
        sql_path("SELECT %s FROM rounds r WHERE r.work = :w_eng_big AND r.cell = 'I1' ORDER BY r.n DESC" % COLS))
    add("engine.round.own_rounds.eng", "engine", "sessions::round::own_rounds: every round of an engineer CLI session",
        sql_path("SELECT %s FROM rounds r JOIN sessions s ON s.id = r.session WHERE s.cli_session = :eng_cli "
                 "ORDER BY r.work, r.n" % COLS))
    add("engine.round.own_rounds.arch", "engine",
        "sessions::round::own_rounds: every round of the architect's CLI session (carried across works)",
        sql_path("SELECT %s FROM rounds r JOIN sessions s ON s.id = r.session WHERE s.cli_session = :arch_cli "
                 "ORDER BY r.work, r.n" % COLS))
    add("engine.round.opens", "engine", "choose::opens: the last row that opened a session",
        sql_path("SELECT max(n) FROM rounds WHERE work = :w_eng_big AND transition IS NOT NULL "
                 "AND cell || '/' || transition IN (SELECT value FROM json_each('[\"I1/done\",\"T1/done\"]'))"))
    add("engine.round.time_limit", "engine", "limits::stretch_ms: the work's time since its last pause",
        sql_path("SELECT coalesce(sum(coalesce(ended_at, :t_end) - started_at), 0) FROM rounds WHERE work = :w_eng_big "
                 "AND n > 0 AND n <= 100000 AND (session IS NOT NULL OR cell IN ('X1', '13', 'tools', 'TOOLS'))"))
    add("engine.round.counter_of_work", "engine", "limits::tell_past: one counter of one work, from work_counters",
        sql_path("SELECT coalesce(max(times), 0) FROM work_counters WHERE work = :w_eng_big AND counter = 'asks'"))
    add("engine.round.counters_all", "engine", "the work_counters view read whole (cell A1, the owner's interface)",
        sql_path("SELECT work, counter, times FROM work_counters"))
    add("engine.round.counts_from_flow", "engine", "work::finish_round of a sent-back round: the row's counter, found in "
        "the compiled flow of the work's version", sql_path(
            "SELECT json_extract(row.value, '$.counts') FROM round_rows x JOIN works w ON w.id = x.work "
            "JOIN flow_versions v ON v.id = w.flow_version JOIN json_each(v.compiled, '$.graphs') g "
            "ON json_extract(g.value, '$.name') = w.graph JOIN json_each(g.value, '$.cells') c "
            "ON json_extract(c.value, '$.id') = x.cell JOIN json_each(c.value, '$.rows') row "
            "ON json_extract(row.value, '$.name') = x.took WHERE x.work = :rej_work AND x.round = :rej_n"))
    add("engine.round.ci_results", "engine", "checks: every ci result of the work, parsed (several times a return)",
        sql_path("SELECT result FROM tool_runs WHERE work = :w_ci AND tool = 'ci' AND result IS NOT NULL "
                 "ORDER BY id DESC LIMIT 1000"))
    add("engine.round.fact_basis", "engine", "checks::fact_basis_ran: a substring search in every tool result of the work",
        sql_path("SELECT count(*) FROM tool_runs WHERE work = :w_ci AND instr(result, 'no-such-text') > 0"))
    add("engine.round.item_merged", "engine", "values::item_merged: the rounds of every work of one item",
        sql_path("SELECT count(*) FROM rounds r JOIN works w ON w.id = r.work WHERE w.item = :item "
                 "AND r.cell IN ('M2', '9', 'merge_queue', 'merge.queue') AND r.transition = 'merged'"))
    add("engine.round.asked_matters", "engine", "review::asked: the work's matters, found by a scan of architect_matters",
        sql_path("SELECT id, reply, withdrawn IS NOT NULL FROM architect_matters WHERE work = :w_eng "
                 "AND kind = 'engineer_question' AND detail = 'x' AND round > 0 ORDER BY id DESC LIMIT 1"))
    add("engine.round.interjections_words", "engine", "verification::words_given: the work's interjections",
        sql_path("SELECT count(*) FROM interjections WHERE work = :w_eng AND delivered_round > "
                 "coalesce((SELECT max(n) FROM rounds WHERE work = :w_eng AND cell IN ('M2') "
                 "AND transition = 'merged'), 0)"))
    add("engine.round.rounds_of_matter", "engine", "work::rounds_of_matter: the architect's rounds at a cell, "
        "found without an index", sql_path(
            "SELECT %s FROM rounds r WHERE r.cell = 'H1' AND (r.session IN (SELECT id FROM sessions "
            "WHERE matter = :matter) OR :matter = (SELECT json_extract(l.output, '$.next.matter') FROM rounds l "
            "WHERE l.work = r.work AND l.cell = 'N0' AND l.n < r.n AND l.transition IS NOT NULL "
            "ORDER BY l.n DESC LIMIT 1)) ORDER BY r.ended_at DESC, r.n DESC" % COLS))
    add("engine.round.architect_time", "engine", "blockers::architect_time: the project's architect rounds in a span",
        sql_path("SELECT coalesce(sum(min(coalesce(r.ended_at, :t_end), :t_end) - max(r.started_at, :t_since)), 0) "
                 "FROM rounds r JOIN works w ON w.id = r.work WHERE w.graph = 'architect' AND w.project = 'wf-agents' "
                 "AND coalesce(r.ended_at, :t_end) > :t_since AND r.started_at < :t_end"))
    add("engine.rare.merges_of_commit", "engine", "main_watch::merges: the output of every round parsed to find one commit",
        sql_path("SELECT w.id, w.item FROM rounds r JOIN works w ON w.id = r.work WHERE w.graph <> 'architect' "
                 "AND json_extract(r.output, '$.result.merge_commit') = 'no-such-commit' LIMIT 1"))
    # -- interface: once per read, and a read follows an event batch the page names ------------------------------------
    add("api.events.row_lookups", "interface", "api::events: two point lookups for each of 200 changed rows (control)",
        event_lookups)
    add("api.works", "interface", "GET /works (also /items, /system, /architect): the whole-table statements",
        sql_path("SELECT work, count(*) FROM owner_asks WHERE answer IS NULL GROUP BY work",
                 "SELECT work, count(*) FROM design_cards WHERE status = 'open' GROUP BY work",
                 "SELECT s.work, sum(coalesce(json_extract(p.usage, '$.cost_usd'), 0)) FROM pings p "
                 "JOIN sessions s ON s.id = p.session GROUP BY s.work",
                 "SELECT DISTINCT work, round FROM tool_runs WHERE started_at IS NOT NULL AND ended_at IS NULL",
                 "SELECT w.id, w.item, i.title, coalesce(i.project, w.project), w.flow_version, w.graph, w.at, "
                 "w.resume_at, w.waiting, w.recorded, w.machine, w.worktree, w.branch, w.pr, w.pushed_head, w.paused, "
                 "w.ended, w.paused_by, w.moved_from FROM works w LEFT JOIN items i ON i.id = w.item "
                 "WHERE w.graph NOT IN ('architect', 'policy') ORDER BY w.id",
                 "SELECT n, work, cell, transition, rejected, started_at, ended_at, json_extract(usage, '$.cost_usd') "
                 "FROM rounds WHERE work IN (SELECT w.id FROM works w WHERE w.graph NOT IN ('architect', 'policy')) "
                 "ORDER BY work, n"))
    add("api.items_extra", "interface", "GET /items besides /works: blocked-by, the item list, the links",
        sql_path("SELECT item, depends_on FROM item_blocked_by ORDER BY item, depends_on",
                 "SELECT i.id, i.project, i.title, i.deliver, i.level, i.cancelled, q.position, i.imported_pr, "
                 "(SELECT r.route FROM work_routes r WHERE r.item = i.id ORDER BY r.work DESC LIMIT 1) "
                 "FROM items i LEFT JOIN dispatch_queue q ON q.item = i.id ORDER BY i.id",
                 "SELECT item, depends_on, needs, kind FROM item_links ORDER BY item, depends_on"))
    add("api.item.outputs", "interface", "GET /items/{id}: the output of every finished round of the item's works",
        sql_path("SELECT r.n, r.work, r.cell, r.output, w.flow_version, w.graph FROM rounds r JOIN works w "
                 "ON w.id = r.work WHERE w.item = :item AND r.cell IS NOT NULL AND r.output IS NOT NULL "
                 "AND r.transition IS NOT NULL ORDER BY r.work, r.n"))
    add("api.item.level_changes", "interface", "GET /items/{id}: the item's level changes, from the change log",
        sql_path("SELECT at, actor, json_extract(old_row, '$.level'), json_extract(new_row, '$.level') FROM changes "
                 "WHERE table_name = 'items' AND json_extract(row_key, '$.id') = :item AND old_row IS NOT NULL "
                 "AND new_row IS NOT NULL AND json_extract(old_row, '$.level') <> json_extract(new_row, '$.level') "
                 "ORDER BY id"))
    add("api.work.eng", "interface", "GET /works/{id} of an engineer work (86 rounds): the round list",
        sql_path(ROUND + " WHERE r.work = :w_eng ORDER BY r.n"))
    add("api.work.arch", "interface", "GET /works/{id} of the architect's longest work (3330 rounds)",
        sql_path(ROUND + " WHERE r.work = :w_arch ORDER BY r.n"))
    add("api.round", "interface", "GET /works/{id}/rounds/{n}: one round (control, bounded)",
        sql_path(ROUND + " WHERE r.work = :w_eng_big AND r.n = 40"))
    add("api.content", "interface", "GET /works/{id}/contents/{hash}: every input of the work searched",
        sql_path("SELECT input FROM rounds WHERE work = :w_eng_big AND instr(input, 'no-such-text') > 0 ORDER BY n"))
    add("api.conversations", "interface", "GET /conversations: the conversation_shown view, read whole",
        sql_path("SELECT conversation, at, sender, waiting, fault IS NOT NULL, "
                 "source = 'ask' AND id IN (SELECT id FROM owner_asks WHERE answer IS NOT NULL), recipient = 'owner' "
                 "FROM conversation_shown"))
    add("api.conversation.work", "interface", "GET /conversation of one work: its entries",
        sql_path("SELECT conversation, source, id, sender, recipient, text, at, work, round, ask, design_card, matter, "
                 "read_at, waiting, pending, undelivered, answerable, fault FROM conversation_shown "
                 "WHERE conversation = 'work:' || :w_eng"))
    add("api.conversation.architect", "interface", "GET /conversation of the architect: its entries",
        sql_path("SELECT conversation, source, id, sender, recipient, text, at, work, round, ask, design_card, matter, "
                 "read_at, waiting, pending, undelivered, answerable, fault FROM conversation_shown "
                 "WHERE conversation = 'architect:wf-agents'"))
    add("api.architect.matters", "interface", "GET /architect/panel: the last 100 handled matters with their rounds' output",
        sql_path("SELECT m.id, m.kind, m.detail, m.what, m.handled_work, m.handled_round, coalesce(r.cell, ''), "
                 "r.ended_at, m.work, json_extract(m.reply, '$.text'), m.asked, r.output FROM architect_matters m "
                 "JOIN rounds r ON r.work = m.handled_work AND r.n = m.handled_round WHERE m.project = 'wf-agents' "
                 "AND m.handled_round IS NOT NULL ORDER BY m.handled_work DESC, m.handled_round DESC, m.id DESC "
                 "LIMIT 100"))
    add("api.architect.notebook", "interface", "GET /architect/panel: every notebook version",
        sql_path("SELECT e.section, e.text, e.work, e.round, r.cell, r.ended_at FROM notebook_edits e JOIN rounds r "
                 "ON r.work = e.work AND r.n = e.round WHERE e.project = 'wf-agents' ORDER BY e.id DESC"))
    add("api.architect.rounds_window", "interface", "GET /architect/rounds: the newest 100 rounds, found by a sort",
        sql_path("SELECT r.started_at, r.work, r.n FROM rounds r JOIN works w ON w.id = r.work WHERE "
                 "w.project = 'wf-agents' AND w.graph IN ('architect', 'policy') "
                 "ORDER BY r.started_at DESC, r.work DESC, r.n DESC LIMIT 100"))
    add("api.system.works_of", "interface", "GET /system: every work with its first and last round",
        sql_path("SELECT w.id, w.graph, w.flow_version, w.item, i.title, coalesce(i.project, w.project), w.at, w.pr, "
                 "w.ended IS NOT NULL, (SELECT min(x.started_at) FROM rounds x WHERE x.work = w.id), "
                 "(SELECT max(coalesce(x.ended_at, x.started_at)) FROM rounds x WHERE x.work = w.id) "
                 "FROM works w LEFT JOIN items i ON i.id = w.item"))
    add("api.system.rounds_since", "interface", "GET /system: the rounds of the last week, found by a scan",
        sql_path("SELECT r.work, r.cell, r.transition, r.rejected, r.started_at, r.ended_at, coalesce(r.model, ''), "
                 "s.line, coalesce(json_extract(r.usage, '$.cost_usd'), 0), "
                 "CAST(coalesce(json_extract(r.usage, '$.usage.input_tokens'), 0) AS INTEGER), "
                 "CAST(coalesce(json_extract(r.usage, '$.usage.cache_read_input_tokens'), 0) AS INTEGER), "
                 "CAST(coalesce(json_extract(r.usage, '$.usage.cache_creation_input_tokens'), 0) AS INTEGER) "
                 "FROM rounds r LEFT JOIN sessions s ON s.id = r.session WHERE r.started_at >= :t_since7 "
                 "ORDER BY r.work, r.n"))
    add("api.system.counted", "interface", "GET /system: every taken row of every round, for the counters",
        sql_path("SELECT work, n, cell, transition, coalesce(ended_at, started_at), started_at FROM rounds "
                 "WHERE cell IS NOT NULL AND transition IS NOT NULL ORDER BY work, n"))
    add("api.system.work_changes", "interface", "GET /system: every change of a work row (the OR terms of the original "
        "filter are left out, so this is a lower bound)", sql_path(
            "SELECT json_extract(c.row_key, '$.id'), c.at, json_extract(c.new_row, '$.pr') IS NOT NULL, "
            "json_extract(c.new_row, '$.paused') FROM changes c WHERE c.table_name = 'works' "
            "AND c.new_row IS NOT NULL ORDER BY c.id"))
    add("api.system.item_events", "interface", "GET /system: the creation time of every item, from the change log",
        sql_path("SELECT i.id, i.title, min(c.at), NULL FROM items i JOIN changes c ON c.table_name = 'items' "
                 "AND c.row_key = json_object('id', i.id) WHERE c.old_row IS NULL GROUP BY i.id ORDER BY 3"))
    add("api.observations", "interface", "GET /observations: every observation with its change times",
        sql_path("SELECT id, title, what, tags, source, times_reported, refs, folded_into, closed, closed_why, created, "
                 "updated FROM observations_all WHERE project = 'wf-agents' "
                 "ORDER BY closed IS NOT NULL OR folded_into IS NOT NULL, coalesce(updated, created) DESC, id DESC"))
    add("api.flows", "interface", "GET /flows: the works of each flow version, counted by a scan of works",
        sql_path("SELECT v.id, v.digest, (SELECT count(*) FROM works w WHERE w.flow_version = v.id) "
                 "FROM flow_versions v ORDER BY v.id DESC"))
    return p


def views(conn):
    return [r[0] for r in conn.execute("SELECT name FROM sqlite_master WHERE type = 'view' ORDER BY name")]


def params(conn, uri):
    P = {}
    for name, value in conn.execute("SELECT name, value FROM bench_params"):
        P[name] = json.loads(value)
    P["t_since"] = P["t_end"] - 3 * DAY
    P["t_since7"] = P["t_end"] - 7 * DAY
    P["rej_work"], P["rej_n"] = P["rej_round"] if P.get("rej_round") else (None, None)
    P["_uri"] = uri
    return P


class Timeout(Exception):
    pass


def timed(conn, fn, P, limit):
    deadline = time.perf_counter() + limit
    conn.set_progress_handler(lambda: 1 if time.perf_counter() > deadline else 0, 100000)
    t = time.perf_counter()
    try:
        rows, size = fn(conn, P)
    except sqlite3.OperationalError as e:
        if "interrupted" in str(e):
            raise Timeout()
        raise
    finally:
        conn.set_progress_handler(None, 0)
    return time.perf_counter() - t, rows, size


def measure_path(conn, fn, P, limit):
    first, rows, size = timed(conn, fn, P, limit)
    if first > 20:
        samples = [first]
    else:
        samples = [timed(conn, fn, P, limit)[0] for _ in range(5 if first < 2 else 3)]
    return first, samples, rows, size


def plan_of(conn, fn, P):
    """The query plan of each statement of a path made of statements, in one line."""
    out = []
    for s in getattr(fn, "statements", ()):
        try:
            rows = conn.execute("EXPLAIN QUERY PLAN " + s, P).fetchall()
            out.append(" / ".join(r[3] for r in rows[:8]))
        except sqlite3.Error as e:
            out.append("plan failed: %s" % e)
    return " || ".join(out)


def run(args):
    cache = Path(args.cache) if args.cache else Path(os.environ.get("REPEATED_PATHS_CACHE") or
                                                    (Path(os.environ.get("RUNNER_TEMP", tempfile.gettempdir())) /
                                                     "repeated-paths-db"))
    scales = args.scales.split(",")
    side = os.path.basename(os.getcwd())
    print("PROCESS\t%s\t%s" % (side, time.strftime("%Y-%m-%dT%H:%M:%S")), flush=True)
    print("SQLITE\t%s" % sqlite3.sqlite_version, flush=True)
    conns = {}
    P = {}
    failed = False
    for s in scales:
        path = database(s, cache)
        uri = "file:%s?mode=ro" % path
        c = sqlite3.connect(uri, uri=True)
        c.execute("PRAGMA query_only=ON")
        P[s] = params(c, uri)
        conns[s] = c
        print("DBSIZE\t%s\t%d" % (s, path.stat().st_size), flush=True)
        intended = P[s]["intended"]
        for table, n in sorted(intended.items()):
            got = c.execute("SELECT count(*) FROM %s" % table).fetchone()[0]
            ok = got == n
            failed = failed or not ok
            field = RECIPE.get(table)
            print("TABLE\t%s\t%s\t%d\t%s\t%s" % (s, table, got, "ok" if ok else "FAIL intended %d" % n,
                                                 ("field %d" % field) if field else ""), flush=True)
        # the recipe against the field, at scale 1
        if s == "1":
            for table, n in RECIPE.items():
                if table in ASSUMED:
                    continue
                if table in intended and abs(intended[table] - n) > 0.05 * n:
                    print("CHECK\tFAIL\t%s built %d, field %d" % (table, intended[table], n), flush=True)
                    failed = True
    paths = build_paths()
    for v in views(conns[scales[0]]):
        paths.append(("view.%s" % v, "database view", "SELECT count(*) FROM %s" % v,
                      sql_path('SELECT count(*) FROM "%s"' % v)))
    only = re.compile(args.only) if args.only else None
    started = time.time()
    # the configuration repository for reload_config, made once
    cfg_holder = {}
    reload_config(conns[scales[0]], cfg_holder)
    for pid, area, what, fn in paths:
        if only and not only.search(pid):
            continue
        if time.time() - started > args.budget_min * 60:
            print("SKIP\t%s\tbudget of %d minutes used" % (pid, args.budget_min), flush=True)
            continue
        for s in scales:
            c = conns[s]
            p = dict(P[s])
            p["_config_repo"] = cfg_holder["_config_repo"]
            if pid == "engine.turn.reload_config" and s != scales[0]:
                continue
            missing = [k for k in re.findall(r"(?<!\w):([a-z_][a-z_0-9]*)", " ".join(getattr(fn, "statements", ())))
                       if p.get(k) is None]
            if missing:
                print("SKIP\t%s\t%s\tno value for %s" % (pid, s, ",".join(sorted(set(missing)))), flush=True)
                continue
            try:
                first, samples, rows, size = measure_path(c, fn, p, args.limit_s)
            except Timeout:
                print("TIMEOUT\t%s\t%s\t%d s" % (pid, s, args.limit_s), flush=True)
                continue
            except Exception as e:  # a path that fails is reported, and the others go on
                failed = True
                print("ERROR\t%s\t%s\t%s" % (pid, s, str(e).replace("\n", " ")[:300]), flush=True)
                continue
            ms = [x * 1000 for x in samples]
            print("PATHTIME\t%s\t%s\t%d\t%.3f\t%.3f\t%.3f\t%d\t%d" % (
                pid, s, len(ms), statistics.median(ms), min(ms), first * 1000, rows, size), flush=True)
            if s == scales[0]:
                plan = plan_of(c, fn, p)
                if plan:
                    print("PLAN\t%s\t%s" % (pid, plan), flush=True)
    shutil.rmtree(cfg_holder["_config_repo"], ignore_errors=True)
    print("DONE\t%s\t%.0f s" % ("failed" if failed else "ok", time.time() - started), flush=True)
    return 1 if failed else 0


# ---------------------------------------------------------------------------------------------------------------------
# The summary
# ---------------------------------------------------------------------------------------------------------------------

T95 = {1: 12.706, 2: 4.303, 3: 3.182, 4: 2.776, 5: 2.571, 6: 2.447, 7: 2.365, 8: 2.306, 9: 2.262, 10: 2.228,
       11: 2.201, 12: 2.179, 13: 2.16, 14: 2.145, 15: 2.131, 16: 2.12, 17: 2.11, 18: 2.101, 19: 2.093, 20: 2.086}


def interval(values):
    """The mean of `values` (logarithms) and the half width of its 95% interval, by Student's t."""
    n = len(values)
    if n < 2:
        return (values[0] if values else float("nan")), float("nan")
    m = statistics.mean(values)
    se = statistics.stdev(values) / math.sqrt(n)
    return m, T95.get(n - 1, 1.96) * se


def summarize(args):
    procs = []
    cur = None
    plans = {}
    errors = []
    for line in Path(args.log).read_text(errors="replace").splitlines():
        f = line.split("\t")
        if f[0] == "PROCESS":
            cur = {"side": f[1], "t": {}}
            procs.append(cur)
        elif f[0] == "PATHTIME" and cur is not None:
            cur["t"][(f[1], f[2])] = {"n": int(f[3]), "med": float(f[4]), "min": float(f[5]), "first": float(f[6]),
                                      "rows": int(f[7]), "bytes": int(f[8])}
        elif f[0] == "PLAN":
            plans[f[1]] = f[2]
        elif f[0] in ("ERROR", "TIMEOUT", "SKIP"):
            errors.append(line)
    paths = sorted({k[0] for p in procs for k in p["t"]})
    labels = []
    for p in procs:
        for k in p["t"]:
            if k[1] not in labels:
                labels.append(k[1])
    print("processes: %d (%s)" % (len(procs), ", ".join(p["side"] for p in procs)))
    for hi in labels[1:]:
        print()
        table(procs, paths, labels[0], hi)
    flush_cost(procs, labels)
    print()
    for pid, plan in sorted(plans.items()):
        print("PLAN %s: %s" % (pid, plan))
    for e in sorted(set(errors)):
        print(e)


# What the page reads again after one batch of events that name a round (`client/src/live.ts`, `named`): the lists of
# works, items, conversations and the system view.
FLUSH = ["api.works", "api.items_extra", "api.conversations", "api.system.works_of", "api.system.rounds_since",
         "api.system.counted", "api.system.work_changes", "api.system.item_events"]


def flush_cost(procs, labels):
    print()
    print("One flush of the page (the statements of %s), median over the processes:" % ", ".join(FLUSH))
    for lab in labels:
        ms = []
        size = []
        for p in procs:
            got = [p["t"].get((pid, lab)) for pid in FLUSH]
            if all(got):
                ms.append(sum(g["med"] for g in got))
                size.append(sum(g["bytes"] for g in got))
        if ms:
            print("  scale %s: %.1f ms, %d bytes of result (%d processes)" % (lab, statistics.median(ms),
                                                                              statistics.median(size), len(ms)))


def table(procs, paths, lo, hi):
    """The table of one scale against the first: per path the rows and bytes it returns at the first, the time at
    both, and the ratio of the two with its 95% interval over the processes."""
    factor = parse_scale(hi)[1] if "L" in hi else parse_scale(hi)[0] / parse_scale(lo)[0]
    print("| path | rows %s | bytes %s | ms %s | ms %s | ratio %s/%s (95%% interval) | class | noise of ms %s (twin/base) |"
          % (lo, lo, lo, hi, hi, lo, lo))
    print("|---|---|---|---|---|---|---|---|")
    for pid in paths:
        logs = []
        small = []
        big = []
        for p in procs:
            a = p["t"].get((pid, lo))
            b = p["t"].get((pid, hi))
            if a:
                small.append(a["med"])
            if b:
                big.append(b["med"])
            if a and b and a["med"] > 0 and b["med"] > 0:
                logs.append(math.log(b["med"] / a["med"]))
        first = next((p["t"][(pid, lo)] for p in procs if (pid, lo) in p["t"]), None)
        if not logs or first is None:
            print("| %s | | | %s | %s | no pair | | |" % (pid, "%.3f" % statistics.median(small) if small else "",
                                                       "%.3f" % statistics.median(big) if big else ""))
            continue
        m, h = interval(logs)
        r_lo, r_hi = math.exp(m - h) if h == h else float("nan"), math.exp(m + h) if h == h else float("nan")
        if h != h:
            cls = "one pair"
        elif r_hi < 1:
            cls = "faster"
        elif r_lo > factor:
            cls = "superlinear"
        elif r_lo > 1 and r_hi >= factor:
            cls = "linear"
        elif r_lo > 1:
            cls = "grows, below linear"
        else:
            cls = "flat"
        # the noise: the time at the small scale of the twin over the base of each pair of three processes
        noise = []
        for i in range(0, len(procs) - 2, 3):
            grp = {q["side"]: q["t"].get((pid, lo)) for q in procs[i:i + 3]}
            if grp.get("twin") and grp.get("base") and grp["twin"]["med"] > 0 and grp["base"]["med"] > 0:
                noise.append(math.log(grp["twin"]["med"] / grp["base"]["med"]))
        if len(noise) >= 2:
            nm, nh = interval(noise)
            nz = "%.3f to %.3f" % (math.exp(nm - nh), math.exp(nm + nh))
        else:
            nz = "n/a"
        print("| %s | %d | %d | %.3f | %.3f | %.2f to %.2f (mean %.2f) | %s | %s |" % (
            pid, first["rows"], first["bytes"], statistics.median(small), statistics.median(big), r_lo, r_hi,
            math.exp(m), cls, nz))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="mode", required=True)
    r = sub.add_parser("run")
    r.add_argument("--scales", default="1,10")
    r.add_argument("--cache", default=None)
    r.add_argument("--only", default=None, help="a regular expression: only the paths whose id it matches")
    r.add_argument("--limit-s", type=int, default=120, help="seconds one run of a path may take")
    r.add_argument("--budget-min", type=int, default=45, help="minutes after which the paths left are skipped")
    s = sub.add_parser("summarize")
    s.add_argument("log")
    args = ap.parse_args()
    if args.mode == "run":
        return run(args)
    summarize(args)
    return 0


if __name__ == "__main__":
    sys.exit(main())
