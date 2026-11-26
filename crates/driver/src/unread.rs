//! What the owner has not read of a work (decision the-engine-keeps-what-the-owner-read), defined once: the item card's
//! count of replies, each reply's mark in the item window, the place `m` goes to, and whether the item is new or has
//! changed are all read from here, from the work's records alone (principle decide-purely).
//!
//! A reply is a version a cell's run wrote to the cell's own words block after the owner had spoken to that cell, with
//! something in it. What the owner wrote, a cell's words to nobody who asked, and a version with nothing in it are no
//! reply. An item is new or has changed where a cell's run wrote a version of a block the item's card shows that the
//! owner has not read: new where the owner has read no version of any such block, changed where they have read one.
//! What a person wrote, the owner's own text of the item among it, is never new to the owner.

use std::collections::BTreeSet;

use decide::{BlockId, Outsider, Records, Seq};

use crate::Kept;

/// Whether the item is new to the owner, or has changed since they read it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fresh {
    /// Nothing a cell wrote for the card is unread.
    #[default]
    None,
    /// A cell wrote what the card shows, and the owner has read none of it.
    New,
    /// The owner has read some of what the card shows, and a cell wrote a later version they have not.
    Changed,
}

/// What the owner has not read of a work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Unread {
    pub fresh: Fresh,
    /// How many replies the owner has not read.
    pub replies_unread: u32,
}

/// Every version of the work the owner has read, each by its block and the place of the entry that wrote it.
pub fn read(records: &Records) -> BTreeSet<(BlockId, Seq)> {
    (records.owner_reads.iter())
        .flat_map(|read| read.versions.iter().cloned())
        .collect()
}

/// A version of a block, as far as what the owner has not read is concerned: the place that wrote it, whether a run did
/// and not a person or the outside, and whether it holds nothing.
struct Version {
    at: Seq,
    run: bool,
    empty: bool,
}

/// The versions of the block, in the order they were written.
fn versions(records: &Records, block: &BlockId) -> Vec<Version> {
    let people = (records.written.iter())
        .filter(|written| &written.block == block)
        .map(|written| Version {
            at: written.at,
            run: false,
            empty: written.empty,
        });
    let runs = (records.finished.iter()).flat_map(|finished| {
        (finished.writes.iter())
            .filter(|write| &write.block == block)
            .map(|write| Version {
                at: finished.at,
                run: true,
                empty: write.empty,
            })
    });
    let mut all: Vec<Version> = people.chain(runs).collect();
    all.sort_by_key(|version| version.at);
    all
}

/// The replies of the work, each by its block and version: of each of these cells' words blocks, a version a run wrote
/// with something in it after the first version a person wrote there with something in it, which is the owner's
/// speaking to the cell.
pub fn replies(records: &Records, words: &[BlockId]) -> Vec<(BlockId, Seq)> {
    let mut found = Vec::new();
    let mut seen = BTreeSet::new();
    for block in words.iter().filter(|block| seen.insert(*block)) {
        let spoken = (records.written.iter())
            .filter(|written| &written.block == block && written.by == Outsider::Person && !written.empty)
            .map(|written| written.at)
            .min();
        let Some(spoken) = spoken else {
            continue;
        };
        found.extend(
            (versions(records, block).into_iter())
                .filter(|version| version.run && !version.empty && version.at > spoken)
                .map(|version| (block.clone(), version.at)),
        );
    }
    found
}

/// What the owner has not read: `card` is what the item's card shows of the work, `replies` the replies the work has
/// ([`replies`], or [`replies_of`] where the work's contents are known).
pub fn unread(records: &Records, card: &[BlockId], replies: &[(BlockId, Seq)]) -> Unread {
    let read = read(records);
    let mut some_read = false;
    let mut some_unread = false;
    for block in card {
        some_read |= read.iter().any(|(read, _)| read == block);
        let latest = (versions(records, block).into_iter())
            .rev()
            .find(|version| version.run && !version.empty);
        some_unread |= latest.is_some_and(|version| !read.contains(&(block.clone(), version.at)));
    }
    let fresh = match (some_unread, some_read) {
        (false, _) => Fresh::None,
        (true, false) => Fresh::New,
        (true, true) => Fresh::Changed,
    };
    let replies_unread = replies.iter().filter(|reply| !read.contains(*reply)).count();
    Unread {
        fresh,
        replies_unread: u32::try_from(replies_unread).unwrap_or(u32::MAX),
    }
}

/// The words block of each cell of the version of its flow the work is on and of those it was on before, as each version
/// declares it: a block is a cell's words by what the flow says of it, never by its name (principle
/// engine-knows-no-cell), and a work is read by the flow it ran on (principle the-past-as-it-ran).
pub fn words_of(kept: &Kept<'_>) -> Vec<BlockId> {
    let outlines = kept
        .on
        .outline()
        .into_iter()
        .chain(kept.passed.iter().map(|outline| &**outline));
    (outlines.flat_map(|outline| outline.cells.iter()))
        .filter_map(|cell| cell.words.as_ref().map(|words| words.block.clone()))
        .collect()
}

/// What the item's card shows of the work: what the version of its flow the work is on names, or, of a work on none
/// kept, what the flow installed names.
pub fn card_of(kept: &Kept<'_>) -> Vec<BlockId> {
    match kept.on.outline() {
        Some(outline) => outline.card.clone(),
        None => kept.flow.card.clone(),
    }
}

/// What a cell's answer says, out of what the engine wrote of it as a version of the cell's words: the version's content
/// is the answer put in the cell's `reply` text, `{reply}` standing where the answer does; a content that does not read
/// so is the answer whole.
fn answer<'a>(template: Option<&str>, content: &'a str) -> &'a str {
    let Some((before, after)) = template.and_then(|text| text.split_once("{reply}")) else {
        return content;
    };
    (content.strip_prefix(before))
        .and_then(|rest| rest.strip_suffix(after))
        .unwrap_or(content)
}

/// The replies of the work: those [`replies`] finds in its records, less each whose answer says nothing, as the item
/// window shows a cell's answer by what it says and leaves one that says nothing out of the talk.
pub fn replies_of(kept: &Kept<'_>) -> Vec<(BlockId, Seq)> {
    let outlines: Vec<&flow::Outline> = (kept.on.outline().into_iter())
        .chain(kept.passed.iter().map(|outline| &**outline))
        .collect();
    let template = |block: &BlockId| {
        (outlines.iter().flat_map(|outline| outline.cells.iter()))
            .filter_map(|cell| cell.words.as_ref().filter(|words| &words.block == block))
            .find_map(|words| words.writes.iter().find(|(name, _)| name == "reply"))
            .map(|(_, text)| text.as_str())
    };
    (replies(&kept.work.records, &words_of(kept)).into_iter())
        .filter(|(block, at)| {
            let content = kept.work.contents.get(&(block.clone(), *at));
            content.is_some_and(|content| !answer(template(block), content).trim().is_empty())
        })
        .collect()
}

/// What the owner has not read of the work.
pub fn of_work(kept: &Kept<'_>) -> Unread {
    unread(&kept.work.records, &card_of(kept), &replies_of(kept))
}

#[cfg(test)]
mod tests {
    //! The owner's reading of a work, on histories made by hand: what is new, what has changed, what is a reply and what
    //! is read. Each case would give another count were the rule it names broken.

    use decide::{Finished, OwnerRead, RunId, Write, Written};
    use flow::{Compiled, Has, Source, compile};
    use store::{First, Item, ItemNumber, WorkHead, WorkNumber};
    use work::{Time, Work};

    use super::*;

    fn block(name: &str) -> BlockId {
        BlockId::from(name)
    }

    /// A person wrote the block, with this much in it.
    fn person(records: &mut Records, name: &str, at: u64, empty: bool) {
        records.written.push(Written {
            block: block(name),
            by: Outsider::Person,
            matters: true,
            value: None,
            empty,
            said_to: None,
            at: Seq(at),
        });
    }

    /// A run wrote the block, with this much in it.
    fn run(records: &mut Records, name: &str, at: u64, empty: bool) {
        records.finished.push(Finished {
            run: RunId(at),
            at: Seq(at),
            writes: vec![Write {
                block: block(name),
                meaning_unchanged: false,
                value: None,
                empty,
            }],
            dealt_with: None,
            waits_for: None,
        });
    }

    fn read_at(records: &mut Records, at: u64, versions: &[(&str, u64)]) {
        records.owner_reads.push(OwnerRead {
            versions: (versions.iter()).map(|(name, at)| (block(name), Seq(*at))).collect(),
            at: Seq(at),
        });
    }

    fn card() -> Vec<BlockId> {
        vec![block("background"), block("plan_owner")]
    }

    fn words() -> Vec<BlockId> {
        vec![block("r1_words")]
    }

    /// What the owner has not read of these records, the replies being those the records say.
    fn unread_of(records: &Records) -> Unread {
        unread(records, &card(), &replies(records, &words()))
    }

    #[test]
    fn an_item_is_new_until_the_owner_reads_what_a_cell_wrote_for_its_card() {
        let mut records = Records::default();
        person(&mut records, "background", 1, false);
        assert_eq!(unread_of(&records).fresh, Fresh::None, "only the owner's text");
        run(&mut records, "plan_owner", 5, false);
        assert_eq!(unread_of(&records).fresh, Fresh::New);
        read_at(&mut records, 6, &[("background", 1), ("plan_owner", 5)]);
        assert_eq!(unread_of(&records).fresh, Fresh::None);
    }

    #[test]
    fn an_item_has_changed_when_a_later_version_than_the_one_read_is_written() {
        let mut records = Records::default();
        person(&mut records, "background", 1, false);
        run(&mut records, "plan_owner", 5, false);
        read_at(&mut records, 6, &[("plan_owner", 5)]);
        run(&mut records, "plan_owner", 7, false);
        assert_eq!(unread_of(&records).fresh, Fresh::Changed);
        read_at(&mut records, 8, &[("plan_owner", 7)]);
        assert_eq!(unread_of(&records).fresh, Fresh::None);
    }

    #[test]
    fn a_version_with_nothing_in_it_does_not_make_an_item_new_and_one_a_person_wrote_is_never_new() {
        let mut records = Records::default();
        run(&mut records, "plan_owner", 5, true);
        person(&mut records, "plan_owner", 6, false);
        assert_eq!(unread_of(&records), Unread::default());
    }

    #[test]
    fn a_reply_is_a_cells_version_with_something_in_it_after_the_owner_spoke_to_the_cell() {
        let mut records = Records::default();
        // A version the cell wrote before the owner spoke is no reply to anything.
        run(&mut records, "r1_words", 2, false);
        assert!(replies(&records, &words()).is_empty());
        person(&mut records, "r1_words", 3, false);
        // The owner's own words are no reply, and a reply with nothing in it is none.
        run(&mut records, "r1_words", 4, true);
        assert!(replies(&records, &words()).is_empty());
        run(&mut records, "r1_words", 5, false);
        assert_eq!(replies(&records, &words()), vec![(block("r1_words"), Seq(5))]);
        assert_eq!(unread_of(&records).replies_unread, 1);
        read_at(&mut records, 6, &[("r1_words", 5)]);
        assert_eq!(unread_of(&records).replies_unread, 0);
        // A reply written after is another.
        run(&mut records, "r1_words", 7, false);
        assert_eq!(unread_of(&records).replies_unread, 1);
    }

    #[test]
    fn what_a_cell_answers_is_what_stands_where_its_reply_text_has_the_answer() {
        let template = Some("An agent:\n\n{reply}");
        assert_eq!(answer(template, "An agent:\n\nIt is done."), "It is done.");
        assert_eq!(answer(template, "An agent:\n\n").trim(), "");
        assert_eq!(answer(template, "No template in it."), "No template in it.");
        assert_eq!(answer(None, "As it is."), "As it is.");
        assert_eq!(answer(Some("{reply}!"), "Done!"), "Done");
    }

    #[test]
    fn words_the_owner_wrote_with_nothing_in_them_are_not_the_owner_speaking() {
        let mut records = Records::default();
        person(&mut records, "r1_words", 3, true);
        run(&mut records, "r1_words", 5, false);
        assert!(replies(&records, &words()).is_empty());
    }

    #[test]
    fn a_block_no_cell_declares_as_its_words_has_no_replies() {
        let mut records = Records::default();
        person(&mut records, "other", 3, false);
        run(&mut records, "other", 5, false);
        assert!(replies(&records, &words()).is_empty());
        assert_eq!(unread(&records, &[], &[]), Unread::default());
    }

    /// A writer that may be spoken to, whose words say how the engine writes its answers, and the merge; the card shows
    /// the text. The block `notes` is named like no cell's words and is none.
    fn asking() -> Compiled {
        let text = r#"
            name = "asking"
            order = ["write", "merge"]
            card = ["text"]
            [limits]
            runs = 3
            returns = 2
            calls = 3
            [lines.main]
            [blocks.request]
            people = true
            [blocks.main]
            outside = "main"
            [blocks.text]
            [blocks.notes]
            [blocks.write_questions]
            outside = "answer"
            [blocks.write_words]
            outside = "words"
            [blocks.write_words.writes]
            owner = "The owner:\n\n{words}"
            reply = "An agent:\n\n{reply}"
            [blocks.merged]
            shown = true
        "#;
        let write = r#"
            title = "Write"
            kind = "agent"
            effort = "high"
            line = "main"
            task = "Write."
            watches = ["request", "write_questions", "write_words"]
            outputs = ["text", "notes", "write_questions", "write_words"]
            [kinds]
            request = "last_and_latest"
            write_questions = "all"
            write_words = "all"
        "#;
        let merge = r#"
            title = "Merge"
            kind = "engine"
            act = "merge"
            watches = ["text", "main"]
            outputs = ["merged"]
        "#;
        let source = Source {
            flow: text.to_string(),
            cells: [("write", write), ("merge", merge)]
                .iter()
                .map(|(id, text)| (id.to_string(), text.to_string()))
                .collect(),
            files: [(
                flow::waits_file(),
                include_str!("../../../flow/agents/waits.toml").to_string(),
            )]
            .into(),
            ..Source::default()
        };
        let has = Has {
            sources: ["answer", "main", "words"].iter().map(|s| s.to_string()).collect(),
            acts: ["merge".to_string()].into(),
            ..Has::default()
        };
        compile(&source, &has).expect("the definition compiles")
    }

    // The flow says which block is a cell's words and which blocks its card shows, and the owner's reading follows it: a
    // cell's answer that says nothing is no reply, a block named like words that no cell declares has none, and what the
    // owner has read is not counted again.
    #[test]
    fn what_the_owner_has_not_read_of_a_work_follows_what_its_flow_declares() {
        let flow = asking();
        let head = WorkHead {
            number: WorkNumber(1),
            item: ItemNumber(1),
            flow: "asking".to_string(),
            added: Time(0),
        };
        let first = First {
            title: "Write".to_string(),
            level: None,
            project: None,
            depends_on: Vec::new(),
            tags: Vec::new(),
            continues: None,
        };
        let item = Item::new(ItemNumber(1), Time(0), first, Vec::new());
        let on = crate::On::installed_flow(&flow);
        let mut work = Work::default();
        person(&mut work.records, "request", 1, false);
        person(&mut work.records, "write_words", 3, false);
        // A run answers the owner and writes the text; a later run answers with nothing but the engine's own words; and
        // another writes a block that is no cell's words.
        run(&mut work.records, "text", 5, false);
        run(&mut work.records, "write_words", 5, false);
        run(&mut work.records, "write_words", 7, false);
        run(&mut work.records, "notes", 9, false);
        for (name, at, content) in [
            ("text", 5, "The text."),
            ("write_words", 5, "An agent:\n\nIt is done."),
            ("write_words", 7, "An agent:\n\n"),
            ("notes", 9, "Written."),
        ] {
            work.contents
                .insert((BlockId::from(name), Seq(at)), content.to_string());
        }
        let kept = |work: &Work| -> Unread {
            let kept = Kept {
                head: &head,
                item: &item,
                flow: &flow,
                on: &on,
                passed: &[],
                work,
            };
            assert_eq!(words_of(&kept), vec![block("write_words")]);
            assert_eq!(card_of(&kept), vec![block("text")]);
            of_work(&kept)
        };
        assert_eq!(
            kept(&work),
            Unread {
                fresh: Fresh::New,
                replies_unread: 1
            }
        );
        read_at(&mut work.records, 10, &[("write_words", 5)]);
        assert_eq!(
            kept(&work),
            Unread {
                fresh: Fresh::New,
                replies_unread: 0
            }
        );
        read_at(&mut work.records, 11, &[("text", 5)]);
        assert_eq!(kept(&work), Unread::default());
    }
}
