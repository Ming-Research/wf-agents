#!/usr/bin/env bash
# The gate that keeps a machine of self-hosted runners quiet while a measurement times on it, installed on the machine
# as docs/installing.md says, for the runners of every project that measures there.
#
# Each runner of the machine runs this file as its job-started hook (ACTIONS_RUNNER_HOOK_JOB_STARTED): a job waits here
# while a measurement holds the gate, and starts once the measurement lets it go. A measurement's workflow sources this
# file in the step that times and calls `wait_for_a_quiet_machine` before it times: the function takes the gate, so that
# no job starts on the machine, then waits until every job already running there has ended. The step holds the gate
# until its shell, and every process that took the gate from it, has exited: at the step's end, or when the step is
# cancelled or times out.
#
# A job is a `Runner.Worker` process, which a runner starts for each job and which exits when the job ends. A job that
# waits at the gate, in the hook or as a measurement before it takes the gate, leaves a mark in `waiting/` named by its
# worker's pid, so that a measurement does not wait for it; a mark whose worker has ended is let go of. The runners of the
# machine share a home, and the gate's lock and marks are kept in `$HOME/.wf-agents-runner`, beside this file.

gate_dir="$HOME/.wf-agents-runner"

# The pid of the Runner.Worker that runs this job: the nearest ancestor of this shell so named. It fails, saying so,
# where none is.
gate_worker() {
  local pid=$$ comm stat
  while [ "$pid" -gt 1 ]; do
    comm=$(cat "/proc/$pid/comm" 2> /dev/null) || break
    if [ "$comm" = Runner.Worker ]; then
      echo "$pid"
      return 0
    fi
    # The parent's pid is the second field after the name, which is in parentheses and may hold spaces.
    stat=$(cat "/proc/$pid/stat" 2> /dev/null) || break
    stat=${stat##*) }
    pid=$(cut -d ' ' -f 2 <<< "$stat")
  done
  echo "measure-gate: no Runner.Worker runs this shell, so it is no job of a runner" >&2
  return 1
}

# Lets go of each mark whose worker has ended.
gate_sweep() {
  local mark pid
  for mark in "$gate_dir"/waiting/*; do
    [ -e "$mark" ] || continue
    pid=${mark##*/}
    [ "$(cat "/proc/$pid/comm" 2> /dev/null)" = Runner.Worker ] || rm -f "$mark"
  done
}

# The hook: marks this job as waiting, takes the gate shared and lets it go at once, which waits while a measurement
# holds it, and takes the mark away. A wait of more than a second is said in the job's log.
gate_hook() {
  local worker start waited
  worker=$(gate_worker)
  mkdir -p "$gate_dir/waiting"
  gate_sweep
  : > "$gate_dir/waiting/$worker"
  start=$(date +%s)
  exec 8> "$gate_dir/gate"
  flock -s 8
  exec 8>&-
  rm -f "$gate_dir/waiting/$worker"
  waited=$(($(date +%s) - start))
  if [ "$waited" -gt 1 ]; then
    echo "measure-gate: this job waited ${waited} s for a measurement on this machine to end"
  fi
}

# For a measurement's step: takes the gate for the rest of the step, then waits until no job runs on the machine but
# this one and those that wait at the gate, saying every minute which it waits for, and at the end how long it waited.
wait_for_a_quiet_machine() {
  local worker start last now pid
  local -a others
  worker=$(gate_worker) || return 1
  mkdir -p "$gate_dir/waiting"
  gate_sweep
  : > "$gate_dir/waiting/$worker"
  start=$(date +%s)
  exec 9> "$gate_dir/gate"
  flock 9
  rm -f "$gate_dir/waiting/$worker"
  last=$start
  while :; do
    gate_sweep
    others=()
    for pid in $(pgrep -x Runner.Worker || true); do
      [ "$pid" = "$worker" ] && continue
      [ -e "$gate_dir/waiting/$pid" ] && continue
      others+=("$pid")
    done
    [ ${#others[@]} -eq 0 ] && break
    now=$(date +%s)
    if [ $((now - last)) -ge 60 ]; then
      echo "measure-gate: waited $((now - start)) s so far for the jobs of the workers ${others[*]} to end"
      last=$now
    fi
    sleep 1
  done
  echo "measure-gate: the machine is quiet after $(($(date +%s) - start)) s of waiting; timing begins"
}

# Run as a file, it is the hook; sourced, it only defines the functions.
if [ "${BASH_SOURCE[0]}" = "$0" ]; then
  set -euo pipefail
  gate_hook
fi
