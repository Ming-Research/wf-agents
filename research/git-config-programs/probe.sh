#!/bin/sh
# Reproduces the core of #309: git runs programs that the repository configuration (which every worktree of a checkout
# shares, and which an agent's `git config` can write) names, and a command-line pin stops some of them but not all.
# Nothing outside the directory it is given is touched.
#
# This script itself has not been run. The same steps were run by hand, one command at a time, on git 2.56.0 on
# 2026-11-05, and gave what is written below as the expected result of each step; the engine's tests
# (`crates/engine/tests/git_programs.rs`) are what keeps the claims, each with a control that runs the program.
#
# Usage: probe.sh <empty-directory>
#
# Expected result of each step (a marker name means the planted program ran):
#   no pin   status                              -> fsm
#   no pin   merge (conflict)                    -> drv, fsm
#   no pin   worktree add                        -> filt, fsm
#   no pin   push over ssh                       -> ssh
#   no pin   fetch from a local path remote      -> up    (remote.<n>.uploadpack set)
#   pinned   --attr-source=<empty tree>, merge   -> nothing (no drv, no filt, whatever the names)
#   pinned   --attr-source=<empty tree>, worktree add -> nothing
#   pinned   -c core.sshCommand=ssh              -> nothing (the machine's ssh ran, and found no host)
#   pinned   -c remote.<n>.uploadpack=...        -> up: git takes the first value it reads, so -c cannot replace it
#   flag     fetch --upload-pack=git-upload-pack -> nothing: a command-line option outranks the configuration
set -u
root=$1
mk="$root/markers"
prog="$root/prog"
mkdir -p "$mk" "$prog"
# Programs are run as `sh <file>`, so that no executable bit is needed.
mark() { printf 'touch "%s/%s"\nexit %s\n' "$mk" "$1" "$2" > "$prog/$1"; }
mark filt 0
mark ssh 1
mark drv 1
mark fsm 1
mark up 1
G="git -c user.name=t -c user.email=t@t -c commit.gpgsign=false -c core.hooksPath=/dev/null"
seen() { ls "$mk" 2> /dev/null | tr '\n' ' '; rm -f "$mk"/*; echo; }
EMPTY_TREE=4b825dc642cb6eb9a060e54bf8d69288fbee4904
cd "$root" && $G init -q -b main repo && cd repo
echo base > a.txt
echo '*.txt filter=f merge=m' > .gitattributes
$G add -A && $G commit -q -m base && $G branch side
echo main-change > a.txt && $G commit -qam main
$G checkout -q side
echo side-change > a.txt && $G commit -qam side
# what an agent could write
git config filter.f.clean "sh $prog/filt"
git config filter.f.smudge "sh $prog/filt"
git config merge.m.driver "sh $prog/drv %A"
git config core.fsmonitor "sh $prog/fsm"
git config core.sshCommand "sh $prog/ssh"
git remote add ssh ssh://h.invalid/x.git
git remote add local "$root/repo"
git config remote.local.uploadpack "sh $prog/up"
rm -f "$mk"/*
echo "== no pin: status";        $G status > /dev/null 2>&1; seen
echo "== no pin: merge";         $G merge -q main > /dev/null 2>&1; seen; $G merge --abort 2> /dev/null
echo "== no pin: worktree add";  $G worktree add -q ../wt main > /dev/null 2>&1; seen
echo "== no pin: ssh push";      $G push -q ssh HEAD:refs/heads/x > /dev/null 2>&1; seen
echo "== no pin: fetch local";   $G fetch -q local > /dev/null 2>&1; seen
P="-c core.fsmonitor=false -c core.sshCommand=ssh -c core.attributesFile=/dev/null"
echo "== pinned: merge, attributes off"; $G $P --attr-source=$EMPTY_TREE merge -q main > /dev/null 2>&1; seen; $G $P merge --abort 2> /dev/null
echo "== pinned: worktree add, attributes off"; $G $P --attr-source=$EMPTY_TREE worktree add -q --detach ../wt2 main > /dev/null 2>&1; seen
echo "== pinned: ssh push";      $G $P push -q ssh HEAD:refs/heads/x > /dev/null 2>&1; seen
echo "== pinned: fetch local (-c cannot override; expect: up)"; $G $P -c remote.local.uploadpack=git-upload-pack fetch -q local > /dev/null 2>&1; seen
echo "== flag: fetch --upload-pack (expect: nothing)"; $G $P fetch -q --upload-pack=git-upload-pack local > /dev/null 2>&1; seen
