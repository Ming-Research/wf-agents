//! Running a program as a child in a process group of its own, so that it and everything it starts can be stopped
//! together: told to stop, the group is sent TERM, and KILL once [`STOP_WAIT`] has passed without its ending.
//!
//! However the process of the engine ends, killed, crashed or ending by itself, the programs it runs in a group of
//! their own end with it: each has an overseer, a small shell that stops the program's group once a pipe only the
//! engine's process holds closes, and that is let go of only as the program ends with the engine there to see it
//! ([`Overseer`]); a program whose overseer cannot be started is stopped at once, and its run fails. A program's group
//! may also be named in a file while it runs, with the start time of its first process, so that one left running all
//! the same, as where its overseer was ended before the engine's process, is found and stopped before the same thing is
//! run again ([`stop_left`]), and a number the system has since given to another group is told apart from it.

use std::path::Path;
use std::process::{ExitStatus, Stdio};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::oneshot;

use crate::held::Stop;

/// How long a process group told to stop with TERM is given to end before it is killed.
pub const STOP_WAIT: Duration = Duration::from_secs(3);

/// How much of the end of a command's output is kept to say what came of it, in bytes.
pub const TAIL: usize = 4000;

/// The bytes of what a tool's run said that are kept, which no run of one is near: its result is kept whole in the
/// records, and cut only where it is given to an agent.
pub const KEPT_WHOLE: usize = 1 << 30;

/// What came of running a program.
pub(crate) enum Ran {
    /// It ended by itself.
    Ended {
        status: ExitStatus,
        /// What it printed on its standard output, where its lines were not each given as they came.
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    /// It was told to stop, and was stopped.
    Stopped,
}

/// What is done with each line of a program's standard output, without its newline, as it comes.
pub(crate) type Lines<'a> = &'a mut (dyn FnMut(&[u8]) + Send);

/// Runs the program, with this on its standard input where something is given, until it ends or is told to stop;
/// or says why it could not be started, or was stopped at once as no overseer of it could be. Where `group_file` is
/// given, the program's process group is written there while it runs, and the file taken away once it has ended.
pub(crate) async fn run(
    command: Command,
    stdin: Option<Vec<u8>>,
    group_file: Option<&Path>,
    stop: &mut Stop,
) -> Result<Ran, String> {
    run_with(command, stdin, group_file, stop, None, None).await
}

/// Runs the program as [`run`] does, each line of its standard output given to `lines` as it comes, and not kept; and
/// stops it as it is told to stop, or as `halt` is sent, which whatever reads its lines may do. A `halt` dropped unsent
/// stops nothing.
pub(crate) async fn streamed(
    command: Command,
    stdin: Option<Vec<u8>>,
    group_file: Option<&Path>,
    stop: &mut Stop,
    halt: Option<oneshot::Receiver<()>>,
    lines: Lines<'_>,
) -> Result<Ran, String> {
    run_with(command, stdin, group_file, stop, halt, Some(lines)).await
}

async fn run_with(
    mut command: Command,
    stdin: Option<Vec<u8>>,
    group_file: Option<&Path>,
    stop: &mut Stop,
    halt: Option<oneshot::Receiver<()>>,
    lines: Option<Lines<'_>>,
) -> Result<Ran, String> {
    command
        .process_group(0)
        .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let program = command.as_std().get_program().to_string_lossy().into_owned();
    let mut child = command
        .spawn()
        .map_err(|e| format!("{program} cannot be started: {e}"))?;
    let group = child.id();
    // A program the engine's process could end under without stopping it is not run: it would spend and change its
    // worktree where nothing guards, counts or stops it.
    let overseer = match group.map(Overseer::of).transpose() {
        Ok(overseer) => overseer,
        Err(why) => {
            if let Some(group) = group {
                signal(group, "KILL").await;
            }
            let _ = child.wait().await;
            return Err(format!("{program} was stopped as it began: {why}"));
        }
    };
    if let (Some(file), Some(group)) = (group_file, group) {
        // A group not written down is only not stopped after a stop of the engine: no reason to stop this one.
        let started = started(group).await.unwrap_or_default();
        let _ = tokio::fs::write(file, format!("{group}\n{started}\n")).await;
    }
    let ran = wait(child, stdin, group, overseer, &program, stop, halt, lines).await;
    if let Some(file) = group_file {
        let _ = tokio::fs::remove_file(file).await;
    }
    ran
}

/// Waits for the child to end, writing its input and reading its output, each line of it given to `lines` where
/// they are given, or stops its group once told to. Its overseer is let go of once its output has ended, before the
/// child is reaped, after which its group's number may be given to another; so a program that closes its standard
/// output and runs on after it is not overseen from then until it ends. Dropped with this before that, as when the
/// runtime of a process of the engine that ends by itself drops the round, the overseer stops the group.
#[allow(clippy::too_many_arguments)]
async fn wait(
    mut child: tokio::process::Child,
    stdin: Option<Vec<u8>>,
    group: Option<u32>,
    overseer: Option<Overseer>,
    program: &str,
    stop: &mut Stop,
    halt: Option<oneshot::Receiver<()>>,
    mut lines: Option<Lines<'_>>,
) -> Result<Ran, String> {
    // A halt sent stops the program as a stop does; one dropped unsent, or none, never comes.
    let halted = async {
        let sent = match halt {
            Some(halt) => halt.await.is_ok(),
            None => false,
        };
        if !sent {
            std::future::pending::<()>().await;
        }
    };
    if let (Some(bytes), Some(mut pipe)) = (stdin, child.stdin.take()) {
        // Written beside the reading of the output, so that a child that writes before it has read all does not
        // wait on a pipe nobody empties. A child that ends without reading all of it closes the pipe: that is no
        // fault of the writing. Dropped once written, the pipe closes, which tells the child it has all of it.
        tokio::spawn(async move {
            let _ = pipe.write_all(&bytes).await;
        });
    }
    // The standard error is read beside, so that neither pipe fills while the other is read.
    let stderr = child.stderr.take().map(|mut pipe| {
        tokio::spawn(async move {
            let mut read = Vec::new();
            let _ = pipe.read_to_end(&mut read).await;
            read
        })
    });
    let stdout = child.stdout.take();
    let output = async {
        let mut kept = Vec::new();
        if let Some(stdout) = stdout {
            let mut reader = BufReader::new(stdout);
            let mut line = Vec::new();
            loop {
                line.clear();
                let read = (reader.read_until(b'\n', &mut line).await)
                    .map_err(|e| format!("{program}'s output could not be read: {e}"))?;
                if read == 0 {
                    break;
                }
                match &mut lines {
                    Some(lines) => lines(line.strip_suffix(b"\n").unwrap_or(&line)),
                    None => kept.extend_from_slice(&line),
                }
            }
        }
        if let Some(overseer) = overseer {
            overseer.let_go().await;
        }
        let status = (child.wait().await).map_err(|e| format!("{program} could not be waited for: {e}"))?;
        Ok::<_, String>((status, kept))
    };
    tokio::pin!(output);
    tokio::select! {
        output = &mut output => {
            let (status, stdout) = output?;
            let stderr = match stderr {
                Some(read) => read.await.unwrap_or_default(),
                None => Vec::new(),
            };
            Ok(Ran::Ended { status, stdout, stderr })
        }
        () = async { tokio::select! { () = stop.told() => {}, () = halted => {} } } => {
            if let Some(group) = group {
                signal(group, "TERM").await;
                if tokio::time::timeout(STOP_WAIT, &mut output).await.is_err() {
                    signal(group, "KILL").await;
                    let _ = output.await;
                }
            }
            Ok(Ran::Stopped)
        }
    }
}

/// What a program's overseer runs, the group being `$1` and the wait `$2`: it waits for its standard input to close,
/// then stops the group as a stop does, TERM and then KILL.
const OVERSEEING: &str = r#"read -r line; kill -s TERM -- "-$1"; sleep "$2"; kill -s KILL -- "-$1""#;

/// What stops a program's process group as the process of the engine that runs it ends, however it ends. The group is
/// out of reach of the terminal's signals, and no handler of the engine's sees a crash or a `kill -9`: so a small shell
/// waits on a pipe that only the engine's process holds, which closes as that process ends, and then stops the group.
/// It is in a process group of its own, so that the terminal's signals do not end it first. Only let go of, as the
/// program's output ends with the engine there to see it, does it end without touching the group: dropped otherwise,
/// as the rounds are when the runtime of a process of the engine that ends by itself is dropped, it closes its pipe as
/// well, and the shell stops the group; the runtime reaps it while the engine's process runs, and the system once that
/// process has ended.
struct Overseer(tokio::process::Child);

impl Overseer {
    /// The overseer of the group; or why no shell could be started for it.
    fn of(group: u32) -> Result<Overseer, String> {
        let mut command = Command::new("sh");
        let wait = STOP_WAIT.as_secs().to_string();
        (command.args(["-c", OVERSEEING, "overseer", &group.to_string(), &wait]))
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        (command.spawn())
            .map(Overseer)
            .map_err(|e| format!("no overseer of its process group could be started: {e}"))
    }

    /// Lets the overseer go: the shell is killed while its pipe is still open, since only the pipe closing makes it
    /// stop the group, and waited for.
    async fn let_go(mut self) {
        let _ = self.0.kill().await;
    }
}

/// Stops the process group the file names, where one was left running by a process of the engine that ended: TERM,
/// then KILL once [`STOP_WAIT`] has passed with any of it left; and takes the file away. The group is ours, and
/// signalled, only where its first process is alive and started when the file says, or where it is gone and a process
/// left in the group works in `dir`, the round's worktree: what a program started, a build or its tests, outlives it in
/// its group. Otherwise the number now names another group, after a restart of the machine or numbers that came
/// round again, and nothing is signalled; nor is the engine's own group ever.
pub(crate) async fn stop_left(file: &Path, dir: &Path) {
    let Ok(text) = tokio::fs::read_to_string(file).await else {
        return;
    };
    let mut lines = text.lines();
    let group = lines.next().and_then(|line| line.trim().parse::<u32>().ok());
    let written = lines.next().unwrap_or_default().trim().to_string();
    if let Some(group) = group
        && ours(group, &written, dir).await
    {
        signal(group, "TERM").await;
        let gone = async {
            while alive(group).await {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        };
        if tokio::time::timeout(STOP_WAIT, gone).await.is_err() {
            signal(group, "KILL").await;
        }
    }
    let _ = tokio::fs::remove_file(file).await;
}

/// Whether the group is the one a round of ours started: see [`stop_left`].
async fn ours(group: u32, written: &str, dir: &Path) -> bool {
    if group == 0 || Some(group) == group_of(std::process::id()).await {
        return false;
    }
    if alive_process(group).await {
        return !written.is_empty() && started(group).await.as_deref() == Some(written);
    }
    let dir = std::fs::canonicalize(dir).ok();
    for member in members(group).await {
        if let (Some(cwd), Some(dir)) = (cwd(member).await, &dir)
            && std::fs::canonicalize(&cwd).ok().as_ref() == Some(dir)
        {
            return true;
        }
    }
    false
}

/// What `ps` prints of a process, or none where it has no such process.
async fn ps(args: &[&str]) -> Option<String> {
    let shown = Command::new("ps")
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .await
        .ok()?;
    let text = String::from_utf8_lossy(&shown.stdout).trim().to_string();
    (shown.status.success() && !text.is_empty()).then_some(text)
}

/// When the process started, as `ps -o lstart=` prints it.
async fn started(process: u32) -> Option<String> {
    let started = ps(&["-o", "lstart=", "-p", &process.to_string()]).await?;
    Some(started.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// The process group of the process.
async fn group_of(process: u32) -> Option<u32> {
    ps(&["-o", "pgid=", "-p", &process.to_string()]).await?.parse().ok()
}

/// The processes of the group.
async fn members(group: u32) -> Vec<u32> {
    let listed = ps(&["-A", "-o", "pid=,pgid="]).await.unwrap_or_default();
    (listed.lines())
        .filter_map(|line| {
            let mut numbers = line.split_whitespace().map(|n| n.parse::<u32>().ok());
            Some((numbers.next()??, numbers.next()??))
        })
        .filter(|(_, g)| *g == group)
        .map(|(pid, _)| pid)
        .collect()
}

/// The directory the process works in: from `/proc` where the system has it, from `lsof` where it does not; none
/// where neither can be read.
async fn cwd(process: u32) -> Option<std::path::PathBuf> {
    if let Ok(dir) = tokio::fs::read_link(format!("/proc/{process}/cwd")).await {
        return Some(dir);
    }
    let listed = Command::new("lsof")
        .args(["-a", "-d", "cwd", "-p", &process.to_string(), "-Fn"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .await
        .ok()?;
    let text = String::from_utf8_lossy(&listed.stdout);
    text.lines()
        .find_map(|line| line.strip_prefix('n'))
        .map(std::path::PathBuf::from)
}

/// Whether the process is alive.
async fn alive_process(process: u32) -> bool {
    let status = Command::new("kill")
        .args(["-0", &process.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
    status.is_ok_and(|status| status.success())
}

/// Whether any process of the group is left.
async fn alive(group: u32) -> bool {
    let status = Command::new("kill")
        .args(["-0", "--", &format!("-{group}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
    status.is_ok_and(|status| status.success())
}

/// Sends the signal to the process group, with the system's `kill`. A group that has ended already is no fault.
async fn signal(group: u32, name: &str) {
    let _ = Command::new("kill")
        .args(["-s", name, "--", &format!("-{group}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
}

/// A shell command run in a directory, its standard error written where its standard output is, so that the two
/// come in the order they were written. It is given of the engine's environment only what the agents are given of it
/// (`cli::KEPT`, every `LC_*`, and the variables named, which are the settings' `agents.env`): what it prints goes to
/// an agent and into the records, and the engine's credentials must reach neither.
pub(crate) fn shell(command: &str, dir: &std::path::Path, named: &[String]) -> Command {
    let kept =
        |name: &str| crate::cli::KEPT.contains(&name) || name.starts_with("LC_") || named.iter().any(|n| n == name);
    let mut shell = Command::new("sh");
    shell
        .arg("-c")
        .arg("exec 2>&1; eval \"$1\"")
        .arg("sh")
        .arg(command)
        .current_dir(dir)
        .env_clear()
        .envs(std::env::vars().filter(|(name, _)| kept(name)));
    shell
}

/// Runs the program to its end and gives what it printed; or, where it does not end within the limit, kills it and
/// says so: a program the engine waits for, a network call above all, must not hold the engine for ever.
pub(crate) async fn output(mut command: Command, limit: Duration) -> Result<std::process::Output, String> {
    let program = command.as_std().get_program().to_string_lossy().into_owned();
    command.kill_on_drop(true);
    match tokio::time::timeout(limit, command.output()).await {
        Ok(output) => output.map_err(|e| format!("{program} cannot be run: {e}")),
        Err(_) => Err(format!(
            "{program} did not end within {} seconds, and was stopped",
            limit.as_secs()
        )),
    }
}

/// The last [`TAIL`] bytes of an output, as text, beginning at a whole character.
pub(crate) fn tail(output: &[u8]) -> String {
    let mut from = output.len().saturating_sub(TAIL);
    while from < output.len() && (output[from] & 0b1100_0000) == 0b1000_0000 {
        from += 1;
    }
    String::from_utf8_lossy(&output[from..]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The variable that has the test binary, run again, play a process of the engine running a program, its group
    /// named in the directory the variable gives.
    const AN_ENGINE: &str = "WF_AGENTS_TEST_AN_ENGINE";

    /// The file in that directory that names the program's group.
    const NAMED: &str = "program.pgid";

    /// The file in that directory that the program writes `ready` in once it goes on when told TERM, and a line `TERM`
    /// each time it is told it.
    const TOLD: &str = "term.told";

    // Played by the test binary run again, and only so: runs a program that does not end, and goes on when told TERM,
    // writing that it was, as a process of the engine runs an agent's program, until the process is ended.
    #[tokio::test]
    #[ignore = "run only by the test that ends it"]
    async fn an_engine_running_a_program_until_it_is_ended() {
        let Ok(dir) = std::env::var(AN_ENGINE) else {
            return;
        };
        let (file, told) = (Path::new(&dir).join(NAMED), Path::new(&dir).join(TOLD));
        let mut command = Command::new("sh");
        let program = r#"trap 'echo TERM >> "$1"' TERM; echo ready >> "$1"; while :; do sleep 1; done"#;
        command.args(["-c", program, "program"]).arg(&told);
        let _ = run(command, None, Some(&file), &mut Stop::never()).await;
    }

    // Played by the test binary run again, and only so: runs a program that does not end as a round of its own, as a
    // process of the engine does, and ends by itself once the program's group is named, its runtime dropping the round
    // as that of a process of the engine does whose loop has stopped.
    #[tokio::test]
    #[ignore = "run only by the test that waits for it to end"]
    async fn an_engine_ending_by_itself_while_a_program_runs() {
        let Ok(dir) = std::env::var(AN_ENGINE) else {
            return;
        };
        let file = Path::new(&dir).join(NAMED);
        let named = file.clone();
        tokio::spawn(async move {
            let mut command = Command::new("sh");
            command.args(["-c", "sleep 120 & wait"]);
            let _ = run(command, None, Some(&named), &mut Stop::never()).await;
        });
        while group_named(&file).is_none() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    // However the process of the engine ends, a program it runs ends with it: killed, as by `kill -9` or a crash, or
    // ended by a signal a terminal sends the engine's process group, Ctrl-C's INT or a closed terminal's HUP. The
    // process runs nothing as it ends, and the program's group is told TERM first and killed once the wait the stop
    // gives it has passed, so that one that goes on when told TERM lives that long. Each engine is in a group of its
    // own, which the signal is sent to as a terminal sends it, so that an overseer in the engine's group would end with
    // it. About three seconds alone, the three engines side by side and each program killed after three; no other test
    // ends an engine from outside it under its program.
    #[tokio::test]
    async fn a_program_ends_with_the_process_of_the_engine_that_runs_it_when_that_is_killed_or_its_terminal_ends_it() {
        let ending = ["KILL", "INT", "HUP"];
        let dirs: Vec<_> = ending.iter().map(|_| tempfile::tempdir().unwrap()).collect();
        let mut engines: Vec<Engine> = (dirs.iter())
            .map(|dir| Engine::of("an_engine_running_a_program_until_it_is_ended", dir.path()))
            .collect();
        let mut groups = Vec::new();
        for dir in &dirs {
            groups.push(named_in(&dir.path().join(NAMED)).await);
            // Ended before it traps TERM, the program would end at TERM, and could not show it was told it first.
            let told = dir.path().join(TOLD);
            let began = std::time::Instant::now();
            while !std::fs::read_to_string(&told).is_ok_and(|told| told.lines().any(|line| line == "ready")) {
                assert!(began.elapsed() < Duration::from_secs(20), "the program never went on");
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
        let began = std::time::Instant::now();
        for (engine, name) in engines.iter_mut().zip(ending) {
            engine.signalled(name).await;
        }
        let lived = ends_all(&groups, began).await;
        let mut wrong = Vec::new();
        for ((name, dir), lived) in ending.iter().zip(&dirs).zip(lived) {
            let told = std::fs::read_to_string(dir.path().join(TOLD)).unwrap_or_default();
            match lived {
                None => wrong.push(format!("the program outlived the engine that ran it, ended by {name}")),
                Some(_) if !told.lines().any(|line| line == "TERM") => wrong.push(format!(
                    "the program was not told TERM before it was killed, the engine ended by {name}"
                )),
                Some(lived) if lived < STOP_WAIT - Duration::from_secs(1) => wrong.push(format!(
                    "the program was killed {lived:?} after the engine was ended by {name}, before the stop's wait"
                )),
                Some(_) => {}
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("; "));
    }

    // Ending by itself, as when its loop or the server of its pages stops, the process of the engine drops the rounds
    // it runs, and the program's group is stopped as it is when the process is killed. Under a second alone; no other
    // test has an engine end by itself under its program.
    #[tokio::test]
    async fn a_program_ends_with_the_process_of_the_engine_that_runs_it_when_that_ends_by_itself() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::of("an_engine_ending_by_itself_while_a_program_runs", dir.path());
        let group = named_in(&dir.path().join(NAMED)).await;
        assert!(
            engine.ended_within(Duration::from_secs(20)).await,
            "the engine did not end by itself"
        );
        assert!(
            ends(group).await,
            "the program outlived the process of the engine that ran it"
        );
    }

    /// The test binary run again as a process of the engine, playing a test, in a process group of its own as a
    /// terminal's job is; killed with its group where a test leaves it, so that the overseer of its program, in a group
    /// of its own, stops that as well.
    struct Engine(std::process::Child);

    impl Engine {
        /// The engine playing the test of this name, in the directory given.
        fn of(test: &str, dir: &Path) -> Engine {
            let exe = std::env::current_exe().expect("the test binary");
            let test = format!("process::tests::{test}");
            let mut command = std::process::Command::new(exe);
            (command.args(["--exact", &test, "--ignored", "--test-threads=1"]))
                .env(AN_ENGINE, dir)
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            std::os::unix::process::CommandExt::process_group(&mut command, 0);
            Engine(command.spawn().expect("the test binary runs again"))
        }

        /// Whether the engine ends within the limit.
        async fn ended_within(&mut self, limit: Duration) -> bool {
            let began = std::time::Instant::now();
            while self.0.try_wait().expect("the engine's status").is_none() {
                if began.elapsed() > limit {
                    return false;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            true
        }

        /// Sends the engine's process group the signal of this name, as a terminal sends it to its job, and waits for
        /// the engine to end.
        async fn signalled(&mut self, name: &str) {
            signal(self.0.id(), name).await;
            assert!(
                self.ended_within(Duration::from_secs(10)).await,
                "the engine did not end on {name}"
            );
        }
    }

    impl Drop for Engine {
        fn drop(&mut self) {
            if self.0.try_wait().ok().flatten().is_none() {
                let group = format!("-{}", self.0.id());
                let _ = std::process::Command::new("kill")
                    .args(["-s", "KILL", "--", &group])
                    .status();
                let _ = self.0.wait();
            }
        }
    }

    /// The group the file names, where it names one.
    fn group_named(file: &Path) -> Option<u32> {
        let text = std::fs::read_to_string(file).ok()?;
        text.lines().next()?.trim().parse().ok()
    }

    /// The group of the program named in the file, once it is named.
    async fn named_in(file: &Path) -> u32 {
        let began = std::time::Instant::now();
        loop {
            if let Some(group) = group_named(file) {
                return group;
            }
            assert!(began.elapsed() < Duration::from_secs(20), "no program was run");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// How long after `from` each group was seen to have ended, within fifteen seconds of it; one that has not by then
    /// is killed, and has none.
    async fn ends_all(groups: &[u32], from: std::time::Instant) -> Vec<Option<Duration>> {
        let mut ended = vec![None; groups.len()];
        loop {
            for (group, ended) in groups.iter().zip(ended.iter_mut()) {
                if ended.is_none() && !alive(*group).await {
                    *ended = Some(from.elapsed());
                }
            }
            if ended.iter().all(Option::is_some) {
                return ended;
            }
            if from.elapsed() > Duration::from_secs(15) {
                for (group, ended) in groups.iter().zip(&ended) {
                    if ended.is_none() {
                        signal(*group, "KILL").await;
                    }
                }
                return ended;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Whether the group ends within fifteen seconds; one that does not is killed.
    async fn ends(group: u32) -> bool {
        ends_all(&[group], std::time::Instant::now()).await[0].is_some()
    }

    /// The process of the overseer of the group, found by what it runs as `ps` lists it.
    async fn overseer_of(group: u32) -> Option<u32> {
        let listed = ps(&["-A", "-o", "pid=,args="]).await?;
        let runs = format!("overseer {group} {}", STOP_WAIT.as_secs());
        let line = listed.lines().find(|line| line.trim_end().ends_with(&runs))?;
        line.split_whitespace().next()?.parse().ok()
    }

    // A round dropped while the process of the engine runs on, as a panic within it drops it, has its program's group
    // stopped by the overseer, whose shell, once it has ended after the stop's wait, the runtime reaps: none is left
    // behind as a zombie of the engine's. About four seconds alone, the stop's wait and a second; no other test drops a
    // round while the engine runs on.
    #[tokio::test]
    async fn a_round_dropped_while_the_engine_runs_on_has_its_program_stopped_and_leaves_no_shell_behind() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(NAMED);
        let named = file.clone();
        let round = tokio::spawn(async move {
            let mut command = Command::new("sh");
            command.args(["-c", "sleep 120 & wait"]);
            let _ = run(command, None, Some(&named), &mut Stop::never()).await;
        });
        let group = named_in(&file).await;
        let shell = overseer_of(group).await;
        round.abort();
        let _ = round.await;
        let ended = ends(group).await;
        let shell = shell.expect("the program's overseer runs");
        tokio::time::sleep(STOP_WAIT + Duration::from_secs(1)).await;
        let behind = alive_process(shell).await;
        if behind {
            let _ = std::process::Command::new("kill")
                .args(["-s", "KILL", &shell.to_string()])
                .status();
        }
        assert!(ended, "the program outlived its round");
        assert!(!behind, "the overseer's shell was left behind");
    }

    // An overseer let go of, as its program ends with the engine there to see it, is gone and has left the group alone:
    // it signals no group the number is given to after the program is reaped, nor what the program left in its group,
    // which runs on as it did before overseers. Let go of directly, and as a program run here ends by itself leaving a
    // process in its group. Under half a second alone; the tests of an engine that ends hold only overseers that are
    // not let go of.
    #[tokio::test]
    async fn an_overseer_let_go_of_is_gone_and_has_left_its_group_alone() {
        let mut command = std::process::Command::new("sh");
        command.args(["-c", "sleep 120 & wait"]);
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
        let mut first = command.spawn().expect("the group starts");
        let group = first.id();
        let overseer = Overseer::of(group).expect("an overseer");
        let shell = overseer_of(group).await.expect("the overseer runs");
        overseer.let_go().await;
        // Long enough for a shell whose pipe closed to have stopped the group.
        tokio::time::sleep(Duration::from_millis(200)).await;
        let left = first.try_wait().expect("the group's first process").is_none();
        let gone = !alive_process(shell).await;
        signal(group, "KILL").await;
        let _ = first.wait();
        assert!(left, "an overseer let go of stopped the group");
        assert!(gone, "an overseer let go of still runs");

        let mut command = Command::new("sh");
        command.args(["-c", "sleep 30 >/dev/null 2>&1 & echo $!"]);
        let ran = run(command, None, None, &mut Stop::never())
            .await
            .expect("the program runs");
        let Ran::Ended { stdout, .. } = ran else {
            panic!("the program was stopped");
        };
        let left: u32 = (String::from_utf8_lossy(&stdout).trim().parse()).expect("what the program left");
        tokio::time::sleep(Duration::from_millis(200)).await;
        let running = alive_process(left).await;
        let _ = std::process::Command::new("kill")
            .args(["-s", "KILL", &left.to_string()])
            .status();
        assert!(
            running,
            "the overseer of a program that ended by itself stopped what the program left in its group"
        );
    }

    #[tokio::test]
    async fn a_left_group_is_stopped_though_its_first_process_has_ended() {
        let dir = tempfile::tempdir().unwrap();
        // The first process starts one that outlives it in its group, and ends.
        let mut first = std::process::Command::new("sh");
        first.current_dir(dir.path());
        std::os::unix::process::CommandExt::process_group(&mut first, 0);
        let output = (first.args(["-c", "sleep 30 >/dev/null 2>&1 & echo $!"]))
            .output()
            .expect("it runs");
        let child: u32 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .expect("the child's number");
        let leader = group_of(child);
        let file = dir.path().join("left.pgid");
        std::fs::write(&file, format!("{leader}\nMon Jan  1 00:00:00 2001\n")).unwrap();
        assert!(alive(leader).await, "the child is alive in the group");
        stop_left(&file, dir.path()).await;
        assert!(!file.exists());
        let began = std::time::Instant::now();
        while alive(leader).await {
            assert!(began.elapsed() < Duration::from_secs(10), "the group was not stopped");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    #[tokio::test]
    async fn a_file_naming_a_group_that_is_not_ours_is_taken_away_and_nothing_signalled() {
        let dir = tempfile::tempdir().unwrap();
        let mut other = std::process::Command::new("sleep");
        other.arg("30");
        std::os::unix::process::CommandExt::process_group(&mut other, 0);
        let mut other = other.spawn().expect("it runs");
        let group = other.id();
        let file = dir.path().join("stale.pgid");
        // Its number, with a start time it does not have: the number came round to another group.
        std::fs::write(&file, format!("{group}\nMon Jan  1 00:00:00 2001\n")).unwrap();
        stop_left(&file, dir.path()).await;
        assert!(!file.exists(), "the stale file is taken away");
        assert!(alive(group).await, "the other group was let be");
        // Nor is the engine's own group signalled, whatever the file says.
        let own = group_of(std::process::id());
        let started = started(std::process::id()).await.unwrap_or_default();
        std::fs::write(&file, format!("{own}\n{started}\n")).unwrap();
        stop_left(&file, dir.path()).await;
        assert!(alive(own).await);
        other.kill().unwrap();
        other.wait().unwrap();
    }

    /// The process group of the process, as `ps` gives it.
    fn group_of(process: u32) -> u32 {
        let shown = std::process::Command::new("ps")
            .args(["-o", "pgid=", "-p", &process.to_string()])
            .output()
            .expect("ps runs");
        String::from_utf8_lossy(&shown.stdout).trim().parse().expect("a group")
    }

    #[tokio::test]
    async fn a_program_that_does_not_end_within_its_limit_is_given_up_and_killed() {
        let dir = tempfile::tempdir().unwrap();
        let pid = dir.path().join("pid");
        let mut command = Command::new("sh");
        command.args(["-c", &format!("echo $$ > '{}'; exec sleep 30", pid.display())]);
        let began = std::time::Instant::now();
        let given_up = output(command, Duration::from_millis(500)).await.expect_err("given up");
        assert!(given_up.contains("did not end within"), "{given_up}");
        assert!(began.elapsed() < Duration::from_secs(10));
        let pid: u32 = std::fs::read_to_string(&pid).unwrap().trim().parse().unwrap();
        let gone = std::time::Instant::now();
        while alive_process(pid).await {
            assert!(gone.elapsed() < Duration::from_secs(10), "the program was not killed");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let mut quick = Command::new("sh");
        quick.args(["-c", "echo done"]);
        let ended = output(quick, Duration::from_secs(10)).await.expect("it ends");
        assert_eq!(String::from_utf8_lossy(&ended.stdout), "done\n");
    }

    #[test]
    fn the_tail_begins_at_a_whole_character() {
        let mut output = vec![b'a'; TAIL - 1];
        output.splice(0..0, "é".bytes());
        // The first byte of é is cut off, and the second is not taken for a character.
        assert_eq!(tail(&output), "a".repeat(TAIL - 1));
        assert_eq!(tail(b"short"), "short");
    }
}
