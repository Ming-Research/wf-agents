//! Which version of its flow each work is on, as the driver holds it (design tree, `engine/flow-versions`).
//!
//! As the driver opens, it keeps the version of each flow it is given, as its files and its outline, and reads the
//! outline of every version kept: what reading a work on one needs, for no version but the installed one is compiled.
//! A work's records are read through each of its moves in turn (decision `a-move-is-read-through-the-renumberings`).
//! A work that ended stays on the version it ended on, and is held by its end alone where that version is not the one
//! installed; every other work is put on the installed version, with each of its lines whose form the move changes
//! beginning a new session (decision `a-move-that-changes-a-lines-form-begins-its-session-anew`). The driver does not
//! start while the installed table of earlier ids would misread a version a work is on (decision
//! `a-table-that-misreads-a-kept-version-is-refused`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use decide::{CellId, End, Reading, Records};
use flow::{Compiled, Outline};
use store::{KeptFlow, Storage, WorkNumber};
use work::{Session, Time, Why, Work};

/// The version installed of a flow, as the driver opened with it: the flow compiled, the version its files make, and
/// its outline.
#[derive(Clone)]
pub(crate) struct Installed {
    pub(crate) flow: Arc<Compiled>,
    pub(crate) version: String,
    pub(crate) outline: Arc<Outline>,
}

impl Installed {
    /// A work on this version.
    pub(crate) fn on(&self) -> On {
        On::Installed {
            version: self.version.clone(),
            outline: self.outline.clone(),
        }
    }
}

/// The version of its flow a work is on, as the driver holds it (decision `a-work-that-ended-stays-on-its-version`).
#[derive(Clone, Debug)]
pub enum On {
    /// The version installed, which decides the work.
    Installed { version: String, outline: Arc<Outline> },
    /// An earlier version, on which the work ended so: its outline alone is held, no flow decides the work, and its
    /// cells read on the installed version as `reading` says.
    Earlier {
        version: String,
        outline: Arc<Outline>,
        end: End,
        reading: Reading,
    },
    /// None kept: the work ended so before the engine kept versions (decision
    /// `a-work-kept-before-the-versions-is-on-none`).
    NotKept { end: End },
}

impl On {
    /// A work on the version installed that the flow is: as whoever reads a work beside the driver holds it.
    pub fn installed_flow(flow: &Compiled) -> On {
        On::Installed {
            version: work::version_of(&flow.files).version,
            outline: Arc::new(Outline::of(flow, work::digest)),
        }
    }

    /// The outline of the version the work is on; none where it is on none kept.
    pub fn outline(&self) -> Option<&Outline> {
        match self {
            On::Installed { outline, .. } | On::Earlier { outline, .. } => Some(outline),
            On::NotKept { .. } => None,
        }
    }

    /// The version the work is on; none where it is on none kept.
    pub fn version(&self) -> Option<&str> {
        match self {
            On::Installed { version, .. } | On::Earlier { version, .. } => Some(version),
            On::NotKept { .. } => None,
        }
    }

    /// Whether the installed version decides the work.
    pub fn installed(&self) -> bool {
        matches!(self, On::Installed { .. })
    }

    /// How the work ended, where no flow decides it now; none for a work on the installed version, which the walk
    /// decides.
    pub fn end(&self) -> Option<End> {
        match self {
            On::Installed { .. } => None,
            On::Earlier { end, .. } | On::NotKept { end } => Some(*end),
        }
    }

    /// How the cells of a work on an earlier version read on the installed one.
    pub fn reading(&self) -> Option<&Reading> {
        match self {
            On::Earlier { reading, .. } => Some(reading),
            On::Installed { .. } | On::NotKept { .. } => None,
        }
    }
}

/// What a table of earlier ids installed would misread of a version a work is on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Misread {
    /// The installed table does not begin with the version's whole table: a renumbering it holds was rewritten.
    Rewritten,
    /// It reads these cells of the version as the one cell `reads_as`.
    TwoAsOne { cells: Vec<CellId>, reads_as: CellId },
    /// It reads the cell of the version as `reads_as`, which the installed flow neither has nor took out.
    Lost { cell: CellId, reads_as: CellId },
}

/// Keeps the version of the flow, as its files and its outline, where the store does not keep it already, and gives it
/// as installed.
pub(crate) fn keep(store: &mut dyn Storage, flow: Compiled) -> Result<Installed, store::Fault> {
    let version = work::version_of(&flow.files);
    let outline = Outline::of(&flow, work::digest);
    let files = (flow.files.iter())
        .map(|(path, text)| {
            let digest = version.files.get(path).cloned().unwrap_or_else(|| work::digest(text));
            (path.clone(), (digest, text.clone()))
        })
        .collect();
    store.keep_flow(&KeptFlow {
        version: version.version.clone(),
        name: flow.name.clone(),
        files,
        outline: outline.to_json(),
    })?;
    Ok(Installed {
        flow: Arc::new(flow),
        version: version.version,
        outline: Arc::new(outline),
    })
}

/// The outline of every version the store keeps, by the version, or the version whose outline does not read, with why.
pub(crate) fn outlines(store: &dyn Storage) -> Result<BTreeMap<String, Arc<Outline>>, crate::Fault> {
    let mut outlines = BTreeMap::new();
    for kept in store.outlines().map_err(crate::Fault::Store)? {
        let outline = Outline::from_json(&kept.outline).map_err(|why| crate::Fault::Outline {
            version: kept.version.clone(),
            why,
        })?;
        outlines.insert(kept.version, Arc::new(outline));
    }
    Ok(outlines)
}

/// The outline of the version a work was put on, or the fault of a work on a version the store does not keep.
pub(crate) fn outline_of(
    outlines: &BTreeMap<String, Arc<Outline>>,
    work: WorkNumber,
    version: &str,
) -> Result<Arc<Outline>, crate::Fault> {
    (outlines.get(version).cloned()).ok_or_else(|| crate::Fault::NoSuchVersion {
        work,
        version: version.to_string(),
    })
}

/// Reads what the work recorded through each of its moves in turn, as the outlines of the versions before and after
/// each say: a move from no version kept reads every name as it was recorded.
pub(crate) fn read(
    work: &mut Work,
    number: WorkNumber,
    outlines: &BTreeMap<String, Arc<Outline>>,
) -> Result<(), crate::Fault> {
    let moves = work.flows.clone();
    for pair in moves.windows(2) {
        let (Some(from), Some(to)) = (&pair[0].version, &pair[1].version) else {
            continue;
        };
        let reading = flow::reading(
            &outline_of(outlines, number, to)?,
            outline_of(outlines, number, from)?.earlier_ids.len(),
        );
        work.read(pair[1].at, reading);
    }
    Ok(())
}

/// What the installed outline's table of earlier ids would misread of the kept version's cells; none where it reads
/// each as the cell it is now, or as one taken out.
pub(crate) fn misread(installed: &Outline, kept: &Outline) -> Option<Misread> {
    let held = kept.earlier_ids.len();
    if installed.earlier_ids.get(..held) != Some(&kept.earlier_ids[..]) {
        return Some(Misread::Rewritten);
    }
    let reading = flow::reading(installed, held);
    let mut read: BTreeMap<CellId, Vec<CellId>> = BTreeMap::new();
    for cell in &kept.cells {
        read.entry(reading.cell(&cell.id)).or_default().push(cell.id.clone());
    }
    if let Some((reads_as, cells)) = read.iter().find(|(_, cells)| cells.len() > 1) {
        return Some(Misread::TwoAsOne {
            cells: cells.clone(),
            reads_as: reads_as.clone(),
        });
    }
    let taken_out = |id: &CellId| id.as_str().contains('@');
    (read.into_iter())
        .find(|(now, _)| !taken_out(now) && installed.cell(now).is_none())
        .map(|(reads_as, cells)| Misread::Lost {
            cell: cells[0].clone(),
            reads_as,
        })
}

/// How a work on an earlier version has ended on it, as the walk reads an end: the outline's last cell has its
/// record, or its cancel stands with no run open.
pub(crate) fn ended(outline: &Outline, records: &Records) -> Option<End> {
    decide::ended(outline.last()?, records)
}

/// How a work on no version kept ended: it was cancelled where its cancel stands, and merged otherwise, the only ends
/// there are.
pub(crate) fn ended_unkept(records: &Records) -> End {
    match records.cancelled() {
        Some(_) => End::Cancelled,
        None => End::Merged,
    }
}

/// Puts the work on the installed version at this time from the version it is on, `from`, with its outline, none
/// where it was on none kept: what it recorded before reads through the renumberings between the two, and each of its
/// lines that has run begins a new session where the two versions decide its form otherwise, or where the version it
/// was on is not kept, which says nothing of the form.
pub(crate) fn put_on(work: &mut Work, now: Time, from: Option<(&str, &Outline)>, to: &Installed) {
    let reading = from.map_or_else(Reading::default, |(_, outline)| {
        flow::reading(&to.outline, outline.earlier_ids.len())
    });
    work.onto(now, Some(to.version.clone()), reading);
    let line_of = |cell: &CellId| to.outline.cell(cell).and_then(|c| c.line.clone());
    let r = &work.records;
    let ran: BTreeSet<String> = (r.rounds.iter())
        .filter_map(|round| r.started.iter().find(|s| s.run == round.run))
        .filter_map(|started| line_of(&started.cell))
        .collect();
    for line in &to.outline.lines {
        let other = from.is_none_or(|(_, outline)| outline.line(&line.name).is_none_or(|was| was.form != line.form));
        if other && ran.contains(&line.name) {
            let at = work.tick();
            work.sessions.push(Session {
                at,
                line: line.name.clone(),
                why: Why::Flow {
                    from: from.map(|(version, _)| version.to_string()),
                    to: to.version.clone(),
                },
            });
        }
    }
}
