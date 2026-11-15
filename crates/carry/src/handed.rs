//! What the engine holds what a return hands in to, as its round ends: the head of the work's branch and the runs of
//! the project's tests the work made, as the engine found and recorded them, never what the agent says of them.
//!
//! - A block the flow marks as naming the head (`names_the_head`) is the commit that holds what its writer hands in: a
//!   return that writes it names a commit that is the branch's head as the round left it, or one under it with only
//!   the engine's own merges of main on top, each known by its address, its words and the main it merged
//!   ([`git::engine_merge`]). Written as changing nothing in meaning, it names a head its version before named too: a
//!   commit of its own changes what it hands in, and so runs what reads it, the review among them.
//! - A result of a test that a block says a run gave, in the words the tool's file gives for it (`claim` of
//!   `agents/tools/run_tests.toml`), is one a run of the work's `run_tests` on that commit gave, as the engine kept what
//!   the run reported ([`work::tested`]).
//!
//! The check before the merge holds the head to the same blocks ([`crate::acts`]), with [`named`] and [`names_the_head`]
//! read alike.

use decide::BlockId;
use decide::run::Unmet;
use flow::Compiled;
use flow::agents::ResultWords;
use work::tested::Outcome;
use work::{Handed, Work};

use crate::git;
use crate::settings::Project;

/// The words a return is read by: the tool's template of a claim, and its words for what became of a test.
pub(crate) struct Claims<'a> {
    pub(crate) claim: &'a str,
    pub(crate) result: &'a ResultWords,
}

/// What is wrong with what the return writes, as the round left the work's branch: each block that names the head and
/// names another commit, or names a new one as changing nothing in meaning, and each result it says a run gave that no
/// run of the work gave. An `Err` is why the branch could not be looked at.
pub(crate) async fn held(
    project: &Project,
    place: &git::Place,
    flow: &Compiled,
    work: &Work,
    writes: &[Handed],
    claims: Option<&Claims<'_>>,
) -> Result<Vec<Unmet>, String> {
    let mut found = Vec::new();
    let naming: Vec<&Handed> = (writes.iter())
        .filter(|h| flow.blocks.get(&h.block).is_some_and(|b| b.names_the_head))
        .collect();
    if !naming.is_empty() {
        let head = git::git(&project.repo, &["rev-parse", &place.reference()]).await?;
        let heads = heads(project, &head).await?;
        for handed in naming {
            let names = named(project, &handed.content).await?;
            if !names_the_head(&names, &heads) {
                found.push(Unmet::NotTheHead {
                    block: handed.block.clone(),
                    head: head.clone(),
                });
                continue;
            }
            if !handed.matters {
                let before = latest(work, &handed.block).unwrap_or_default();
                if !names_the_head(&named(project, before).await?, &heads) {
                    found.push(Unmet::HeadChanged {
                        block: handed.block.clone(),
                    });
                }
            }
        }
    }
    let Some(claims) = claims else {
        return Ok(found);
    };
    for handed in writes.iter().filter(|h| h.value.is_none()) {
        for said in read(claims, &handed.content) {
            let commit = resolved(project, &said.commit).await?;
            let ran = commit.is_some_and(|commit| {
                (work.tested.values()).any(|t| t.commit == commit && t.became(&said.test) == Some(said.became))
            });
            if !ran {
                found.push(Unmet::NoSuchRun {
                    block: handed.block.clone(),
                    claim: said.text,
                });
            }
        }
    }
    Ok(found)
}

/// The content of the latest version of the block, where it has one.
fn latest<'a>(work: &'a Work, block: &BlockId) -> Option<&'a str> {
    let versions = decide::versions(&work.records);
    let at = versions.get(block)?.last()?.at;
    work.contents.get(&(block.clone(), at)).map(String::as_str)
}

/// Each word of the text that could be a commit's name, with the commit the repository resolves it to, each commit
/// once, in the order the text names them.
pub(crate) async fn named(project: &Project, text: &str) -> Result<Vec<(String, String)>, String> {
    let mut named: Vec<(String, String)> = Vec::new();
    let words = text.split(|c: char| !c.is_ascii_alphanumeric());
    for word in words.filter(|w| is_hex(w)) {
        if let Some(commit) = resolved(project, word).await?
            && !named.iter().any(|(_, full)| *full == commit)
        {
            named.push((word.to_string(), commit));
        }
    }
    Ok(named)
}

/// Whether a word has the form of a commit's name: seven to forty hexadecimal figures.
fn is_hex(word: &str) -> bool {
    (7..=40).contains(&word.len()) && word.chars().all(|c| c.is_ascii_hexdigit())
}

/// The commit the repository resolves the name to, where it is one.
async fn resolved(project: &Project, name: &str) -> Result<Option<String>, String> {
    if !is_hex(name) {
        return Ok(None);
    }
    let commit = format!("{name}^{{commit}}");
    let said = git::said(&project.repo, &["rev-parse", "--verify", "--quiet", &commit]).await?;
    Ok(said.ok.then_some(said.out))
}

/// The head, and each commit under it with only the engine's own merges of main between, each by its first parent,
/// the branch's side.
pub(crate) async fn heads(project: &Project, head: &str) -> Result<Vec<String>, String> {
    let mut heads = vec![head.to_string()];
    while let Some(first) = git::engine_merge(project, heads.last().expect("one at least")).await? {
        heads.push(first);
    }
    Ok(heads)
}

/// Whether a commit named is the head or under it with only the engine's own merges of main on top ([`heads`]).
pub(crate) fn names_the_head(named: &[(String, String)], heads: &[String]) -> bool {
    named.iter().any(|(_, commit)| heads.contains(commit))
}

/// A result of a test a text says a run gave: the words as written, the test, what became of it, and the commit as
/// written.
#[derive(Debug, PartialEq, Eq)]
struct Said {
    text: String,
    test: String,
    became: Outcome,
    commit: String,
}

/// Each result of a test the text says a run gave, in the words of the claim: the words before `{test}`, which end
/// where its name begins and which the name does not hold, then the name on one line, the words between it and
/// `{became}`, what became of it (`passed` or `failed`), the words between that and `{commit}`, and the commit, a word
/// of hexadecimal figures. A template that does not have the three in that order with words before the test reads
/// none.
fn read(claims: &Claims<'_>, text: &str) -> Vec<Said> {
    let Some(parts) = parts(claims.claim) else {
        return Vec::new();
    };
    let [before, between, after] = parts;
    let mut found = Vec::new();
    for (word, became) in [
        (&claims.result.passed, Outcome::Passed),
        (&claims.result.failed, Outcome::Failed),
    ] {
        let needle = format!("{between}{word}{after}");
        for (at, _) in text.match_indices(&needle) {
            let line_start = text[..at].rfind('\n').map_or(0, |i| i + 1);
            let Some(open) = text[line_start..at].rfind(before.as_str()).map(|i| line_start + i) else {
                continue;
            };
            let test = &text[open + before.len()..at];
            let rest = &text[at + needle.len()..];
            let commit: String = rest.chars().take_while(char::is_ascii_alphanumeric).collect();
            if test.trim().is_empty() || !is_hex(&commit) {
                continue;
            }
            found.push(Said {
                text: text[open..at + needle.len() + commit.len()].to_string(),
                test: test.trim().to_string(),
                became,
                commit,
            });
        }
    }
    found.sort_by_key(|said| text.find(&said.text));
    found.dedup_by(|a, b| a.text == b.text);
    found
}

/// The words of a claim around its places: before `{test}`, between it and `{became}`, and between that and
/// `{commit}`, which ends it; none where the template is not so.
fn parts(claim: &str) -> Option<[String; 3]> {
    let (before, rest) = claim.split_once("{test}")?;
    let (between, rest) = rest.split_once("{became}")?;
    let (after, end) = rest.split_once("{commit}")?;
    (!before.is_empty() && end.is_empty()).then(|| [before, between, after].map(str::to_string))
}

/// What is wrong with the claim's words of the tool's file, for reading back what a return says a run gave: `{test}`,
/// `{became}` and `{commit}` in that order, words before the test, and nothing after the commit. The engine is not run
/// with words that are wrong.
pub fn claim_faults(claim: Option<&str>) -> Vec<String> {
    let file = format!("{}/tools/run_tests.toml", flow::agents::AGENTS_DIR);
    match claim {
        Some(claim) if parts(claim).is_none() => vec![format!(
            "{file}: its claim must hold {{test}}, {{became}} and {{commit}} in this order, with words before {{test}} \
             and nothing after {{commit}}, so that what a return says a run gave is read back: it is `{claim}`"
        )],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use flow::agents::fill;

    use super::*;
    use crate::git::tests::{Repo, commit, run};

    /// A claim's words filled, as an agent is told to write it.
    fn claimed(claims: &Claims<'_>, test: &str, became: &str, commit: &str) -> String {
        fill(claims.claim, &[("test", test), ("became", became), ("commit", commit)])
    }

    fn words() -> flow::agents::Tool {
        let text = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../flow/agents/tools/run_tests.toml"
        ));
        toml::from_str(text).expect("the file of run_tests reads")
    }

    // What a block says a run gave is read back in the words this repository's file gives, each claim on its line,
    // with its test, what became of it and its commit; words that only look like one, a name that is no commit, or a
    // claim cut across lines, are none.
    #[test]
    fn each_result_a_block_says_a_run_gave_is_read_back_in_the_files_words() {
        let tool = words();
        let claims = Claims {
            claim: tool.claim.as_deref().expect("the file gives a claim"),
            result: tool.result.as_ref().expect("the file gives its result's words"),
        };
        let passed = claimed(&claims, "tests::a_row", "passed", "3c01b7d4");
        let failed = claimed(&claims, "a page > shows the list", "failed", "97a46e2c1234");
        let text = format!(
            "- {passed}, which it did not do before: {failed}.\n- `no_commit` passed on main\n- `cut\n` passed on 3c01b7d4"
        );
        let read = read(&claims, &text);
        assert_eq!(
            read,
            [
                Said {
                    text: passed,
                    test: "tests::a_row".to_string(),
                    became: Outcome::Passed,
                    commit: "3c01b7d4".to_string(),
                },
                Said {
                    text: failed,
                    test: "a page > shows the list".to_string(),
                    became: Outcome::Failed,
                    commit: "97a46e2c1234".to_string(),
                },
            ]
        );
    }

    // The words this repository gives are read back; a template that lacks a place, has them out of order, has no
    // words before the test or words after the commit, is a fault of its file.
    #[test]
    fn a_claim_whose_words_cannot_be_read_back_is_a_fault_of_its_file() {
        assert_eq!(claim_faults(words().claim.as_deref()), Vec::<String>::new());
        assert_eq!(claim_faults(None), Vec::<String>::new());
        for wrong in [
            "{test} {became} on {commit}",
            "`{test}` on {commit} {became}",
            "`{test}` {became}",
            "`{test}` {became} on {commit}.",
        ] {
            assert_eq!(claim_faults(Some(wrong)).len(), 1, "{wrong}");
        }
    }

    /// The flow of the tests of the real carriers, whose `code` names the head of the work's branch.
    fn flow() -> Compiled {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/real/flow");
        let source = flow::read(&dir).expect("the flow is read");
        let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
        let has = flow::Has {
            sources: names(&["tree", "main"]),
            acts: names(&["start", "check", "merge"]),
            ..Default::default()
        };
        flow::compile(&source, &has).expect("the flow compiles")
    }

    /// A repository with a work's branch, which has a commit of its own: the repository, the branch's place, and that
    /// commit.
    async fn started() -> (Repo, git::Place, String) {
        let repo = Repo::new("true");
        let place = repo.place(store::WorkNumber(1));
        let main = run(&repo.project.repo, &["rev-parse", "main"]);
        git::start(&repo.project, &place, &main).await.expect("the work starts");
        let first = commit(&place.dir, "farewell.txt", "Bye.\n");
        (repo, place, first)
    }

    fn code(content: &str, matters: bool) -> Handed {
        Handed {
            block: BlockId::from("code"),
            content: content.to_string(),
            matters,
            value: None,
        }
    }

    /// What is wrong with what the return writes, with no claim read.
    async fn wrong(repo: &Repo, place: &git::Place, work: &Work, writes: &[Handed]) -> Vec<Unmet> {
        held(&repo.project, place, &flow(), work, writes, None)
            .await
            .expect("the branch is looked at")
    }

    // A block that names the head is held to the head as the round left it, short or whole: a commit under it, a word
    // that is no commit, or none at all, is not it; a block that does not name the head is not held to it.
    #[tokio::test]
    async fn a_block_that_names_the_head_is_sent_back_unless_it_names_the_branchs_head() {
        let (repo, place, first) = started().await;
        let work = Work::default();
        let head = commit(&place.dir, "weather.txt", "Sunny.\n");
        let short = format!("Says goodbye and the weather, in {}.", &head[..9]);
        assert_eq!(
            wrong(&repo, &place, &work, &[code(&short, true)]).await,
            Vec::<Unmet>::new()
        );
        let not_the_head = |content: &str| {
            (
                code(content, true),
                Unmet::NotTheHead {
                    block: BlockId::from("code"),
                    head: head.clone(),
                },
            )
        };
        for content in [
            format!("Says goodbye, in {first}."),
            "In deadbeefcafe.".to_string(),
            String::new(),
        ] {
            let (handed, unmet) = not_the_head(&content);
            assert_eq!(wrong(&repo, &place, &work, &[handed]).await, [unmet], "{content}");
        }
        let title = Handed {
            block: BlockId::from("title"),
            ..code("Say goodbye", true)
        };
        assert_eq!(wrong(&repo, &place, &work, &[title]).await, Vec::<Unmet>::new());
    }

    // Written as changing nothing in meaning, a block that names the head names one its version before named too: a
    // version before that named an older commit, or none, makes the new head a change of its meaning.
    #[tokio::test]
    async fn a_new_head_handed_in_as_changing_nothing_in_meaning_is_sent_back() {
        let (repo, place, first) = started().await;
        let before = |content: &str| {
            let mut work = Work::default();
            let at = decide::Seq(1);
            work.records.written.push(decide::Written {
                block: BlockId::from("code"),
                by: decide::Outsider::Person,
                matters: true,
                value: None,
                said_to: None,
                at,
            });
            work.contents.insert((BlockId::from("code"), at), content.to_string());
            work
        };
        let mended = code(&format!("Says goodbye, in commit {first}."), false);
        let same = before(&format!("Says goodbye, in {first}."));
        assert_eq!(
            wrong(&repo, &place, &same, std::slice::from_ref(&mended)).await,
            Vec::<Unmet>::new()
        );
        let head = commit(&place.dir, "weather.txt", "Sunny.\n");
        let moved = code(&format!("Says goodbye and the weather, in {head}."), false);
        let changed = [Unmet::HeadChanged {
            block: BlockId::from("code"),
        }];
        assert_eq!(wrong(&repo, &place, &same, std::slice::from_ref(&moved)).await, changed);
        assert_eq!(
            wrong(&repo, &place, &Work::default(), std::slice::from_ref(&moved)).await,
            changed
        );
        // Handed in as a change of its meaning, the new head is what it hands in.
        let changing = Handed { matters: true, ..moved };
        assert_eq!(wrong(&repo, &place, &same, &[changing]).await, Vec::<Unmet>::new());
    }

    // A result a block says a run gave is held to what the work's runs reported: the test, what became of it, and the
    // commit the run ran on, short or whole; one of another outcome, another commit, a test the run did not report, or
    // a word that is no commit, is sent back with the block and the claim as it was written.
    #[tokio::test]
    async fn a_result_no_run_of_the_work_gave_is_sent_back() {
        let (repo, place, first) = started().await;
        let head = commit(&place.dir, "weather.txt", "Sunny.\n");
        let tool = words();
        let claims = Claims {
            claim: tool.claim.as_deref().expect("the file gives a claim"),
            result: tool.result.as_ref().expect("the file gives its result's words"),
        };
        let case = |name: &str, outcome| work::tested::Case {
            unit: "carry".to_string(),
            name: name.to_string(),
            outcome,
        };
        let mut work = Work::default();
        work.tested.insert(
            decide::CallId(1),
            work::Tested {
                commit: first.clone(),
                cases: vec![case("tests::a_row", Outcome::Failed)],
            },
        );
        work.tested.insert(
            decide::CallId(2),
            work::Tested {
                commit: head.clone(),
                cases: vec![
                    case("tests::a_row", Outcome::Passed),
                    case("tests::later", Outcome::Skipped),
                ],
            },
        );
        let said = |test: &str, became: &str, commit: &str| claimed(&claims, test, became, commit);
        let true_ones = [
            said("tests::a_row", "failed", &first[..8]),
            said("tests::a_row", "passed", &head),
        ];
        let false_ones = [
            said("tests::a_row", "passed", &first[..8]),
            said("tests::a_row", "failed", &head),
            said("tests::later", "passed", &head),
            said("tests::other", "passed", &head),
            said("tests::a_row", "passed", "deadbeefcafe"),
        ];
        let evidence = Handed {
            block: BlockId::from("title"),
            ..code(&format!("{}\n{}", true_ones.join("; "), false_ones.join("\n")), true)
        };
        let found = held(&repo.project, &place, &flow(), &work, &[evidence], Some(&claims)).await;
        let expected: Vec<Unmet> = (false_ones.iter())
            .map(|claim| Unmet::NoSuchRun {
                block: BlockId::from("title"),
                claim: claim.clone(),
            })
            .collect();
        assert_eq!(found, Ok(expected));
    }
}
