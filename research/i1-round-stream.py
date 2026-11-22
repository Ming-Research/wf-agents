"""Measures one round's stream, for research/i1-round-cost-measured.md.

Run as `python3 -I research/i1-round-stream.py <rounds/W/RUN-ROUND.stream.jsonl>`. It reads the file only and prints:

- the model the program ran on (its `init` event) and the calls to it (each distinct assistant message id once);
- the round's time (the last event timestamp less the first), the tool time (for each user event, its timestamp less
  that of the assistant event before it) and the output tokens a second;
- the tool calls by name (each `tool_use` block of an assistant message once, by its id), the bytes of their inputs
  and of their results;
- the error results (a `tool_result` with `is_error`, counted under the tool its `tool_use_id` names);
- the repeated reads: a Read of a file and range that an earlier Read already gave, with no Edit or Write of that
  file and no compaction in between (an earlier Read of the whole file covers every later range);
- the compactions (each `compact_boundary` event: tokens before and after, and the time it took) and, after each, the
  Reads of files the round had already read, edited or written.

With `--fit` and several stream files it instead fits, for each model, the round's time less its compactions as
`a * calls + b * output tokens` (least squares, no intercept, the rounds of three calls or more), and prints `a`, the
speed 1 / b and the median relative error.

A script and not `jq` and the shell, because it matches each result to its call by `tool_use_id`, decides whether a
Read repeats an earlier one from the Edits, Writes and compactions between them, and solves the fit.

It serves whoever measures the cell's rounds again; it is kept with the report and goes when the report does.
"""
import collections
import datetime
import json
import sys


def seconds(stamp):
    return datetime.datetime.fromisoformat(stamp.replace("Z", "+00:00")).timestamp()


def size(content):
    return len(content) if isinstance(content, str) else len(json.dumps(content))


def fit(paths):
    rounds = collections.defaultdict(list)
    for path in paths:
        events = [json.loads(line) for line in open(path) if line.strip()]
        stamps = [seconds(e["timestamp"]) for e in events if e.get("timestamp")]
        model = next((e["model"] for e in events if e.get("subtype") == "init"), None)
        calls = len({e["message"]["id"] for e in events if e["type"] == "assistant"})
        output = next((e["usage"]["output_tokens"] for e in events if e["type"] == "result"), 0)
        compaction = sum(e["compact_metadata"]["duration_ms"] / 1000 for e in events
                         if e.get("subtype") == "compact_boundary")
        if len(stamps) > 1 and calls >= 3:
            rounds[model].append((calls, output, stamps[-1] - stamps[0] - compaction))
    for model, rows in sorted(rounds.items()):
        s11 = sum(c * c for c, _, _ in rows)
        s12 = sum(c * o for c, o, _ in rows)
        s22 = sum(o * o for _, o, _ in rows)
        b1 = sum(c * t for c, _, t in rows)
        b2 = sum(o * t for _, o, t in rows)
        det = s11 * s22 - s12 * s12
        a = (b1 * s22 - b2 * s12) / det
        b = (s11 * b2 - s12 * b1) / det
        errors = sorted(abs(a * c + b * o - t) / t for c, o, t in rows)
        print(f"{model}: {len(rows)} rounds, {a:.2f} s a call, {1 / b:.0f} output tokens a second, "
              f"median error {errors[len(errors) // 2]:.2f}")


def main(path):
    events = [json.loads(line) for line in open(path) if line.strip()]
    stamps = [seconds(e["timestamp"]) for e in events if e.get("timestamp")]
    total = stamps[-1] - stamps[0]
    model = next((e["model"] for e in events if e.get("subtype") == "init"), None)
    api_calls = len({e["message"]["id"] for e in events if e["type"] == "assistant"})

    names, calls, results, errors = {}, collections.Counter(), collections.Counter(), collections.Counter()
    inputs = collections.Counter()
    order = []
    tool_time, after_assistant = 0.0, None
    output_tokens = 0
    for e in events:
        if e["type"] == "result":
            output_tokens = e["usage"]["output_tokens"]
        if e["type"] == "system" and e.get("subtype") == "compact_boundary":
            meta = e["compact_metadata"]
            order.append(("COMPACT", meta, None))
        if e["type"] == "assistant":
            if e.get("timestamp"):
                after_assistant = seconds(e["timestamp"])
            for block in e["message"]["content"]:
                if block["type"] == "tool_use":
                    names[block["id"]] = block["name"]
                    calls[block["name"]] += 1
                    inputs[block["name"]] += len(json.dumps(block["input"]))
                    order.append((block["name"], block["input"], block["id"]))
        elif e["type"] == "user":
            if e.get("timestamp") and after_assistant is not None:
                tool_time += seconds(e["timestamp"]) - after_assistant
                after_assistant = None
            if isinstance(e["message"]["content"], list):
                for part in e["message"]["content"]:
                    if part.get("type") == "tool_result":
                        name = names.get(part["tool_use_id"], "?")
                        results[name] += size(part["content"])
                        if part.get("is_error"):
                            errors[name] += 1

    print(f"model {model}, {api_calls} calls to it")
    print(f"round time {total:.0f} s, tool time {tool_time:.0f} s, output tokens {output_tokens}, "
          f"{output_tokens / total:.0f} a second")
    print("tool calls", dict(calls), "total", sum(calls.values()))
    print("input bytes", dict(inputs))
    print("result bytes", dict(results))
    print("error results", dict(errors), "total", sum(errors.values()))

    result_size = {}
    for e in events:
        if e["type"] == "user" and isinstance(e["message"]["content"], list):
            for part in e["message"]["content"]:
                if part.get("type") == "tool_result":
                    result_size[part["tool_use_id"]] = size(part["content"])

    seen, repeats, repeat_bytes = {}, 0, 0
    known, compactions = set(), 0
    reread = []
    for name, data, ident in order:
        if name == "COMPACT":
            seen = {}
            compactions += 1
            reread.append([0, 0])
            print(f"compaction {compactions}: {data['pre_tokens']} tokens before, {data['post_tokens']} after, "
                  f"{data['duration_ms'] / 1000:.0f} s")
            continue
        path = data.get("file_path")
        if name in ("Edit", "Write") and path:
            seen.pop(path, None)
            known.add(path)
        elif name == "Read" and path:
            key = (data.get("offset"), data.get("limit"))
            if path in seen and any(k in (key, (None, None)) for k in seen[path]):
                repeats += 1
                repeat_bytes += result_size.get(ident, 0)
            seen.setdefault(path, []).append(key)
            if compactions and path in known:
                reread[-1][0] += 1
                reread[-1][1] += result_size.get(ident, 0)
            known.add(path)
    print(f"repeated reads (same file and range, file unchanged, no compaction between): {repeats}, {repeat_bytes} bytes")
    for number, (count, volume) in enumerate(reread, 1):
        print(f"after compaction {number}: {count} reads of files already read, edited or written, {volume} bytes")


if __name__ == "__main__":
    if sys.argv[1] == "--fit":
        fit(sys.argv[2:])
    else:
        main(sys.argv[1])
