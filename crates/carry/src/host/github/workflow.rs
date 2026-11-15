//! What the engine reads of a workflow of the project's own, as the file of it on main holds it: the jobs that sum up
//! the others, each of which needs every other job of the workflow, so that it is red where any of them is.
//!
//! Only what that takes is read, the jobs of `jobs` by their keys, and of each its `needs` and its `name`, written as
//! GitHub's workflow files write them: a job a key indented under `jobs`, its keys indented under it, `needs` a name, a
//! list in brackets or a list of lines each beginning with `- `. A file the engine cannot read so sums up nothing.

/// A job of a workflow: its key, its `name` as written, and the keys of the jobs it needs.
#[derive(Debug, Default, PartialEq, Eq)]
struct Job {
    key: String,
    name: Option<String>,
    needs: Vec<String>,
}

/// The names GitHub gives the jobs of the workflow that need every other job of it: a job's `name` where it gives one
/// with no expression in it, its key where it gives none; a job whose name holds an expression is none, its name being
/// known only as the run makes it.
pub(super) fn summing(text: &str) -> Vec<String> {
    let jobs = jobs(text);
    (jobs.iter())
        .filter(|job| {
            let others: Vec<&String> = (jobs.iter()).filter(|o| o.key != job.key).map(|o| &o.key).collect();
            !others.is_empty() && others.iter().all(|other| job.needs.contains(other))
        })
        .filter_map(|job| match &job.name {
            Some(name) if name.contains("${{") => None,
            Some(name) => Some(name.clone()),
            None => Some(job.key.clone()),
        })
        .collect()
}

/// The jobs of the workflow, in the order it writes them.
fn jobs(text: &str) -> Vec<Job> {
    let indent = |line: &str| line.len() - line.trim_start().len();
    let lines: Vec<&str> = (text.lines())
        .map(uncommented)
        .filter(|line| !line.trim().is_empty())
        .collect();
    let Some(start) = lines.iter().position(|line| *line == "jobs:") else {
        return Vec::new();
    };
    let mut jobs: Vec<Job> = Vec::new();
    let (mut at_job, mut at_key) = (None, None);
    let mut in_needs = false;
    for line in &lines[start + 1..] {
        let (depth, text) = (indent(line), line.trim());
        if depth == 0 {
            break;
        }
        let job_depth = *at_job.get_or_insert(depth);
        if depth == job_depth {
            let Some(key) = text.strip_suffix(':') else {
                return Vec::new();
            };
            jobs.push(Job {
                key: key.to_string(),
                ..Job::default()
            });
            (at_key, in_needs) = (None, false);
            continue;
        }
        let Some(job) = jobs.last_mut() else {
            return Vec::new();
        };
        if depth < job_depth {
            return Vec::new();
        }
        let key_depth = *at_key.get_or_insert(depth);
        if in_needs
            && depth >= key_depth
            && let Some(item) = text.strip_prefix("- ")
        {
            job.needs.push(unquoted(item));
            continue;
        }
        in_needs = false;
        if depth != key_depth {
            continue;
        }
        if let Some(value) = text.strip_prefix("needs:") {
            let value = value.trim();
            if let Some(list) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
                job.needs = (list.split(',')).map(unquoted).filter(|n| !n.is_empty()).collect();
            } else if value.is_empty() {
                in_needs = true;
            } else {
                job.needs = vec![unquoted(value)];
            }
        } else if let Some(value) = text.strip_prefix("name:") {
            job.name = Some(unquoted(value));
        }
    }
    jobs
}

/// The line without a comment at its end: what follows a `#` at its start or after a space.
fn uncommented(line: &str) -> &str {
    if line.trim_start().starts_with('#') {
        return "";
    }
    line.find(" #").map_or(line, |at| &line[..at]).trim_end()
}

/// A value without the quotes around it.
fn unquoted(value: &str) -> String {
    let value = value.trim();
    let quoted = |q: char| value.len() >= 2 && value.starts_with(q) && value.ends_with(q);
    if quoted('"') || quoted('\'') {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The project's own full workflow has one job that sums up the others, `check`, which needs every other job.
    #[test]
    fn the_projects_full_workflow_sums_up_its_jobs_in_check() {
        let ci = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.github/workflows/ci.yml"));
        assert_eq!(summing(ci), ["check"]);
    }

    // A job that needs every other job sums them up, by the name it gives or by its key, its needs written as a list in
    // brackets, a list of lines or one name; one that needs only some sums up nothing, nor one whose name holds an
    // expression, nor anything of a file with no jobs or none the engine reads.
    #[test]
    fn a_job_that_needs_every_other_job_sums_them_up() {
        let file = |summing_job: &str| {
            format!(
                "name: ci\non:\n  push:\njobs:\n  # The tests.\n  build:\n    runs-on: x\n    steps:\n      - run: make # it\n  \
                 test:\n    name: \"the tests\"\n    needs: build\n{summing_job}\n"
            )
        };
        let all = file("  sum:\n    needs: [build, 'test']\n    if: always()");
        assert_eq!(summing(&all), ["sum"]);
        let lines = file("  sum:\n    name: everything\n    needs:\n      - build\n      - test\n    runs-on: x");
        assert_eq!(summing(&lines), ["everything"]);
        let some = file("  sum:\n    needs: [build]");
        assert_eq!(summing(&some), Vec::<String>::new());
        let named = file("  sum:\n    name: sum ${{ matrix.k }}\n    needs: [build, test]");
        assert_eq!(summing(&named), Vec::<String>::new());
        assert_eq!(summing("name: ci\non: push\n"), Vec::<String>::new());
        assert_eq!(summing("jobs:\n  - a\n"), Vec::<String>::new());
        let one = "jobs:\n  only:\n    needs: []\n";
        assert_eq!(summing(one), Vec::<String>::new());
    }
}
