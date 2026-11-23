//! The pull requests open on the hosts of the projects' repositories, each with the work it belongs to: what the host
//! says of it, and what the work does now and when something last happened to it, as the records say. A project whose
//! host could not say is said by itself with why, never as having none, and a project whose host has no pull requests
//! is said so.
//!
//! A pull request is a work's by its project and its number: the latest pull request a work keeps is the one it
//! checks through, and no other of its is counted.

use serde::{Deserialize, Serialize};

use crate::works::{Item, Items, Now};

/// What the hosts of some projects say of their open pull requests.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pulls {
    /// The open pull requests, in the order of the projects asked, each project's by number.
    pub pulls: Vec<OpenPull>,
    /// The projects whose host could not say, each with why: none of their pull requests is in `pulls`, and none is
    /// said to be missing.
    pub failed: Vec<Unsaid>,
    /// The projects whose host has no pull requests, as the local repository.
    pub without: Vec<String>,
}

/// A pull request open on the host of a project's repository.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenPull {
    pub project: String,
    pub number: u64,
    pub title: String,
    pub url: String,
    pub draft: bool,
    /// When the host last updated it, in milliseconds since 1970.
    pub updated: u64,
    /// The work it belongs to; none where no work of the project keeps it as its latest.
    pub work: Option<PullWork>,
}

/// The work a pull request belongs to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullWork {
    pub work: u64,
    pub item: u64,
    /// What the work does now.
    pub now: Now,
    /// When something last happened to the work; none where nothing has.
    pub last: Option<u64>,
}

/// A project whose host could not say what is open, and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Unsaid {
    pub project: String,
    pub why: String,
}

impl Pulls {
    /// What each project's host said, in the order given, with the work each pull request belongs to among these items.
    pub fn of(items: &Items, said: Vec<(String, Result<Option<Vec<service::OpenPull>>, String>)>) -> Pulls {
        let mut pulls = Pulls {
            pulls: Vec::new(),
            failed: Vec::new(),
            without: Vec::new(),
        };
        for (project, said) in said {
            match said {
                Err(why) => pulls.failed.push(Unsaid { project, why }),
                Ok(None) => pulls.without.push(project),
                Ok(Some(mut open)) => {
                    open.sort_by_key(|pull| pull.number);
                    for pull in open {
                        let work = (items.items.iter())
                            .find(|item| keeps(item, &project, pull.number))
                            .map(|item| PullWork {
                                work: item.work.number,
                                item: item.number,
                                now: item.work.now.clone(),
                                last: item.work.last,
                            });
                        pulls.pulls.push(OpenPull {
                            project: project.clone(),
                            number: pull.number,
                            title: pull.title,
                            url: pull.url,
                            draft: pull.draft,
                            updated: pull.updated,
                            work,
                        });
                    }
                }
            }
        }
        pulls
    }
}

/// Whether the item's work keeps this pull request of this project as its latest.
fn keeps(item: &Item, project: &str, number: u64) -> bool {
    item.project.as_deref() == Some(project) && item.work.pull.as_ref().is_some_and(|pull| pull.number == number)
}
