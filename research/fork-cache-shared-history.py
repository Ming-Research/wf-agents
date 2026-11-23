"""Does a fork of a session read the session's history from the prompt cache, and does a second fork read what the
first one wrote? Run as `python3 -I fork-cache-shared-history.py <empty directory> <phase>`, phase 1, 2 or 3; it calls the
agents' program `claude` about ten times on two small models and prints each call's tokens (see
fork-cache-shared-history.md).

Whom it serves: the owner and the engineer of item #33, who decide how forks are asked. It is kept until the report
is replaced by a measurement of the engine's own forks.
"""

import json
import random
import subprocess
import sys
import uuid

DIR = sys.argv[1]
PHASE = sys.argv[2]
random.seed({"1": 7, "2": 11, "3": 13}[PHASE])
WORDS = [f"w{random.randrange(10**6):06d}x{random.randrange(10**4):04d}" for _ in range(2600)]
HISTORY = "Remember this list, it is only padding: " + " ".join(WORDS) + "\nReply with the single word OK."
SYSTEM = "You answer in one short sentence."


def call(model, message, resume=None, fork=False, session=None, at=None):
    """One call of the program; gives the session it ran in, the id of its last assistant message and its usage."""
    args = [
        "claude", "-p", "--output-format", "stream-json", "--verbose", "--model", model,
        "--append-system-prompt", SYSTEM, "--setting-sources", "project", "--tools", "",
    ]
    if session:
        args += ["--session-id", session]
    if resume:
        args += ["--resume", resume]
    if fork:
        args += ["--fork-session"]
    if at:
        args += ["--resume-session-at", at]
    done = subprocess.run(args, input=message, capture_output=True, text=True, cwd=DIR, check=False)
    last = None
    result = None
    for line in done.stdout.splitlines():
        v = json.loads(line)
        if v.get("type") == "assistant":
            last = v["uuid"]
        if v.get("type") == "result":
            result = v
    u = result["usage"]
    return result["session_id"], last, (u["input_tokens"], u["cache_read_input_tokens"], u["cache_creation_input_tokens"]), result["total_cost_usd"]


def call_many(model, messages, resume, fork=True, at=None):
    """One call of the program given several user messages on its input, as stream-json: gives the usage of each
    request the model was sent (the first of each message), in order."""
    args = [
        "claude", "-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose", "--model", model,
        "--append-system-prompt", SYSTEM, "--setting-sources", "project", "--tools", "", "--resume", resume,
    ]
    if fork:
        args += ["--fork-session"]
    if at:
        args += ["--resume-session-at", at]
    lines = "".join(json.dumps({"type": "user", "message": {"role": "user", "content": [{"type": "text", "text": m}]}}) + "\n" for m in messages)
    done = subprocess.run(args, input=lines, capture_output=True, text=True, cwd=DIR, check=False)
    seen, uses, total = set(), [], None
    for line in done.stdout.splitlines():
        v = json.loads(line)
        if v.get("type") == "assistant" and v["message"]["id"] not in seen:
            seen.add(v["message"]["id"])
            u = v["message"]["usage"]
            uses.append((u["input_tokens"], u["cache_read_input_tokens"], u["cache_creation_input_tokens"]))
        if v.get("type") == "result":
            total = v["total_cost_usd"]
    return uses, total


def show(label, model, got):
    sid, last, (fresh, read, written), cost = got
    print(f"{label:<50} {model:<7} fresh={fresh:<4} read={read:<6} written={written:<6} total=${cost:.4f}", flush=True)
    return sid, last


if PHASE == "1":
    s = str(uuid.uuid4())
    show("E0 the session begins (history ~21k tokens)", "haiku", call("haiku", HISTORY, session=s))
    for i in (1, 2):
        show(f"W{i} fork of it, its cache alive", "haiku", call("haiku", f"Question {i}: what is 1+{i}?", resume=s, fork=True))
    # Another model has no cache of the session: the session is as cold as one whose cache has expired.
    for i in (3, 4):
        show(f"C{i-2} fork of it, cold", "sonnet", call("sonnet", f"Question {i}: what is 1+{i}?", resume=s, fork=True))
    show("P  a ping in the session itself, cold", "sonnet", call("sonnet", "ping", resume=s))
    for i in (5, 6):
        show(f"A{i-4} fork of it after the ping", "sonnet", call("sonnet", f"Question {i}: what is 1+{i}?", resume=s, fork=True))
elif PHASE == "3":
    # A fork given a fixed first message and then its question, three times, on a session whose cache is cold.
    s = str(uuid.uuid4())
    show("E0 the session begins (history ~21k tokens)", "haiku", call("haiku", HISTORY, session=s))
    for i in (1, 2, 3):
        uses, total = call_many("sonnet", ["Reply with the single word OK.", f"Question {i}: what is 1+{i}?"], s)
        for k, (fresh, read, written) in enumerate(uses, 1):
            print(f"X{i} fork, request {k} of the program, cold   sonnet  fresh={fresh:<4} read={read:<6} written={written:<6}", flush=True)
        print(f"X{i} total=${total:.4f}", flush=True)
else:
    # A session that went on past the point a fork begins at, then cold: two forks at the point; then one fork at the
    # point that only pings, and two forks of that fork.
    s = str(uuid.uuid4())
    _, point = show("E0 the session begins (history ~21k tokens)", "haiku", call("haiku", HISTORY, session=s))
    show("E1 the session goes on past the point", "haiku", call("haiku", "Say OK again.", resume=s))
    for i in (1, 2):
        show(f"B{i} fork at the point, cold", "sonnet", call("sonnet", f"Question {i}: what is 1+{i}?", resume=s, fork=True, at=point))
    f0, _ = show("F0 fork at the point that only pings, cold", "sonnet", call("sonnet", "ping", resume=s, fork=True, at=point))
    for i in (3, 4):
        show(f"G{i-2} fork of F0", "sonnet", call("sonnet", f"Question {i}: what is 1+{i}?", resume=f0, fork=True))
