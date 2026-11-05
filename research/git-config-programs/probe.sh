#!/bin/sh
# Reproduces #309: git runs programs that the repository configuration (which every worktree of a checkout shares,
# and which an agent's `git config` can write) names, and shows which command-line pin stops each. Run on git 2.56.0,
# 2026-11-05; each step finishes in well under a second. Nothing outside the directory it is given is touched, except
# the credential step, which asks the machine's own credential helpers to store and then forget a dummy entry for the
# host h.invalid (with osxkeychain that is the user's keychain); drop that step on a machine where that matters.
#
# Usage: probe.sh <empty-directory>
#
# Result of the run (a marker file means the planted program ran):
#   no pin   status                        -> fsm
#   no pin   merge (conflict)              -> drv, fsm
#   no pin   worktree add                  -> filt, fsm
#   no pin   push over ssh                 -> ssh
#   no pin   credential approve            -> cred       (generic and URL-scoped helper alike)
#   no pin   fetch from a local-path remote with remote.<n>.uploadpack set   -> up
#   no pin   commit with commit.gpgsign=true and a gpg.program               -> "gpg failed to sign"
#   pinned   -c core.fsmonitor=false                                         -> no fsm
#   pinned   -c core.sshCommand=ssh                                          -> no ssh (the real ssh ran)
#   pinned   -c credential.helper=      (resets the list, also the system helper and URL-scoped ones) -> no cred
#   pinned   -c commit.gpgsign=false                                         -> commit succeeds
#   pinned   -c merge.m.driver=  (by name)                                   -> no drv; "cannot run :" and a plain conflict
#   pinned   --attr-source=<empty tree> -c core.attributesFile=/dev/null     -> no drv, no filt, whatever the names
#   NOT pinnable by -c: remote.<n>.uploadpack (a second value is ignored: "using the first"), core.gitProxy (the
#   value "" or "none" still runs the proxy). `fetch --upload-pack=git-upload-pack` outranks the configured value (run:
#   no marker). That GIT_PROXY_COMMAND outranks core.gitProxy is from git's documentation and was not run.
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
mark cred 0
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
git config credential.https://h.invalid.helper "!sh $prog/cred"
git remote add ssh ssh://h.invalid/x.git
git remote add local "$root/repo"
git config remote.local.uploadpack "sh $prog/up"
rm -f "$mk"/*
echo "== no pin: status";        $G status > /dev/null 2>&1; seen
echo "== no pin: merge";         $G merge -q main > /dev/null 2>&1; seen; $G merge --abort 2> /dev/null
echo "== no pin: worktree add";  $G worktree add -q ../wt main > /dev/null 2>&1; seen
echo "== no pin: ssh push";      $G push -q ssh HEAD:refs/heads/x > /dev/null 2>&1; seen
echo "== no pin: fetch local";   $G fetch -q local > /dev/null 2>&1; seen
P="-c core.fsmonitor=false -c core.sshCommand=ssh -c credential.helper= -c core.attributesFile=/dev/null"
echo "== pinned: merge, attributes off"; $G $P --attr-source=$EMPTY_TREE merge -q main > /dev/null 2>&1; seen; $G $P merge --abort 2> /dev/null
echo "== pinned: worktree add, attributes off"; $G $P --attr-source=$EMPTY_TREE worktree add -q --detach ../wt2 main > /dev/null 2>&1; seen
echo "== pinned: ssh push";      $G $P push -q ssh HEAD:refs/heads/x > /dev/null 2>&1; seen
echo "== pinned: fetch local (-c cannot override; expect: up)"; $G $P -c remote.local.uploadpack=git-upload-pack fetch -q local > /dev/null 2>&1; seen
echo "== flag: fetch --upload-pack (expect: nothing)"; $G $P fetch -q --upload-pack=git-upload-pack local > /dev/null 2>&1; seen
