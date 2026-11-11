//! The levels of the items: how hard an item's work is, which chooses what its agents' rounds run on. The installation's
//! settings name its levels, each with what work it is for; an item is given one as it is added, or later by the owner,
//! the one the settings name for an item that is given none, and none above the highest they let an item be at. A round
//! runs at its item's level, or above it where its line says so, but never above that highest level.
//!
//! The levels are in their order by their names: by number where they are numbers, by name otherwise, so that `10`
//! comes after `2`. A refusal names the key of the settings it comes from, so that the owner knows where to look; the
//! one level of an engine whose agents follow a script comes from no settings, and its refusals name none.

/// The levels of an installation's items.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Levels {
    /// The levels, lowest first.
    levels: Vec<Level>,
    /// Where among them the level an item is added at where it is given none is.
    default: usize,
    /// Where among them the highest level an item may be at is.
    highest: usize,
    /// Whether the levels are the settings', whose keys a refusal names.
    from_settings: bool,
}

/// A level: its name, and what work it is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Level {
    pub name: String,
    pub desc: String,
}

/// Why levels cannot be made of what was given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LevelsFault {
    /// There is no level at all.
    None,
    /// The level an item is added at where it is given none is not among them.
    NoSuchDefault(String),
    /// The highest level an item may be at is not among them.
    NoSuchHighest(String),
    /// The level an item is added at where it is given none is above the highest an item may be at.
    DefaultAboveHighest { default: String, highest: String },
}

impl Levels {
    /// The settings' levels, with the one an item is added at where it is given none, and the highest an item may be
    /// at: the top level where none is named.
    pub fn new(
        levels: impl IntoIterator<Item = Level>,
        default: &str,
        highest: Option<&str>,
    ) -> Result<Levels, LevelsFault> {
        let mut levels: Vec<Level> = levels.into_iter().collect();
        levels.sort_by_key(|level| (level.name.parse::<u64>().ok(), level.name.clone()));
        levels.dedup_by(|a, b| a.name == b.name);
        if levels.is_empty() {
            return Err(LevelsFault::None);
        }
        let at = |name: &str| levels.iter().position(|l| l.name == name);
        let default_at = at(default).ok_or_else(|| LevelsFault::NoSuchDefault(default.to_string()))?;
        let highest_at = match highest {
            Some(highest) => at(highest).ok_or_else(|| LevelsFault::NoSuchHighest(highest.to_string()))?,
            None => levels.len() - 1,
        };
        if default_at > highest_at {
            return Err(LevelsFault::DefaultAboveHighest {
                default: default.to_string(),
                highest: levels[highest_at].name.clone(),
            });
        }
        Ok(Levels {
            levels,
            default: default_at,
            highest: highest_at,
            from_settings: true,
        })
    }

    /// One level, at which every item is added, from no settings.
    pub fn one(name: &str, desc: &str) -> Levels {
        let level = Level {
            name: name.to_string(),
            desc: desc.to_string(),
        };
        Levels {
            levels: vec![level],
            default: 0,
            highest: 0,
            from_settings: false,
        }
    }

    /// The levels, lowest first.
    pub fn levels(&self) -> &[Level] {
        &self.levels
    }

    /// The level an item is added at where it is given none.
    pub fn default(&self) -> &str {
        &self.levels[self.default].name
    }

    /// The highest level an item may be at.
    pub fn highest(&self) -> &str {
        &self.levels[self.highest].name
    }

    /// The level an item asked to be at is given: the one asked for, or the default where none is; refused, in words,
    /// where it is no level or above the highest an item may be at.
    pub fn of_item(&self, asked: Option<&str>) -> Result<String, String> {
        match asked {
            None => Ok(self.default().to_string()),
            Some(level) => self.at(level).map(|at| self.levels[at].name.clone()),
        }
    }

    /// The level a round runs at: its item's, or this many above it, but none above the highest an item may be at. An
    /// item that has no level runs at the default: no level can be given to one whose level was never kept without
    /// guessing. Refused, in words, where the item's level is not one of the levels, or is above the highest an item
    /// may be at: such an item is not run at another level, which would spend at a model nobody chose for it.
    pub fn of_round(&self, item: Option<&str>, above: u32) -> Result<String, String> {
        let at = match item {
            None => self.default,
            Some(level) => self.at(level).map_err(|why| format!("the item's level: {why}"))?,
        };
        let raised = at.saturating_add(usize::try_from(above).unwrap_or(usize::MAX));
        Ok(self.levels[raised.min(self.highest)].name.clone())
    }

    /// Where the level of this name is, where an item may be at it; or why it may not, in words.
    fn at(&self, level: &str) -> Result<usize, String> {
        let key = |key: &str| match self.from_settings {
            true => format!(" ({key})"),
            false => String::new(),
        };
        match self.levels.iter().position(|l| l.name == level) {
            Some(at) if at <= self.highest => Ok(at),
            Some(_) => Err(format!(
                "{level} is above {}, the highest level an item may be at{}",
                self.highest(),
                key("items.highest_level")
            )),
            None => {
                let names: Vec<&str> = self.levels.iter().map(|l| l.name.as_str()).collect();
                Err(format!(
                    "{level} is no level: the levels are {}{}",
                    names.join(", "),
                    key("agents.levels")
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn made(names: &[&str], default: &str, highest: Option<&str>) -> Result<Levels, LevelsFault> {
        let levels = names.iter().map(|n| Level {
            name: n.to_string(),
            desc: format!("Work of level {n}"),
        });
        Levels::new(levels, default, highest)
    }

    fn names(levels: &Levels) -> Vec<&str> {
        levels.levels().iter().map(|l| l.name.as_str()).collect()
    }

    #[test]
    fn levels_go_by_number_where_they_are_numbers() {
        let levels = made(&["10", "2", "1"], "2", None).unwrap();
        assert_eq!(names(&levels), ["1", "2", "10"]);
        assert_eq!(levels.levels()[2].desc, "Work of level 10");
        assert_eq!((levels.default(), levels.highest()), ("2", "10"));
    }

    #[test]
    fn an_item_is_at_the_default_where_it_asks_for_none_and_never_above_the_highest() {
        let levels = made(&["1", "2", "3", "4"], "2", Some("3")).unwrap();
        assert_eq!(levels.of_item(None).unwrap(), "2");
        assert_eq!(levels.of_item(Some("1")).unwrap(), "1");
        assert_eq!(levels.of_item(Some("3")).unwrap(), "3");
        let above = levels.of_item(Some("4")).unwrap_err();
        assert_eq!(
            above,
            "4 is above 3, the highest level an item may be at (items.highest_level)"
        );
        let none = levels.of_item(Some("5")).unwrap_err();
        assert_eq!(none, "5 is no level: the levels are 1, 2, 3, 4 (agents.levels)");
    }

    #[test]
    fn a_round_runs_above_its_item_s_level_where_its_line_says_but_never_above_the_highest() {
        let levels = made(&["1", "2", "3", "4"], "2", Some("3")).unwrap();
        assert_eq!(levels.of_round(Some("1"), 0).unwrap(), "1");
        assert_eq!(levels.of_round(Some("1"), 1).unwrap(), "2");
        assert_eq!(levels.of_round(Some("2"), 1).unwrap(), "3");
        assert_eq!(
            levels.of_round(Some("3"), 1).unwrap(),
            "3",
            "the highest an item may be at"
        );
        assert_eq!(
            levels.of_round(None, 1).unwrap(),
            "3",
            "an item of no level is at the default"
        );
        let gone = levels.of_round(Some("9"), 0).unwrap_err();
        assert!(gone.starts_with("the item's level: 9 is no level"), "{gone}");
        let top = made(&["1", "2"], "1", None).unwrap();
        assert_eq!(
            top.of_round(Some("2"), 1).unwrap(),
            "2",
            "with no highest named, the top level"
        );
    }

    #[test]
    fn the_one_level_of_no_settings_names_no_key_when_it_refuses() {
        let one = Levels::one("1", "The one level");
        assert_eq!(one.of_item(None).unwrap(), "1");
        assert_eq!(one.of_item(Some("2")).unwrap_err(), "2 is no level: the levels are 1");
    }

    #[test]
    fn levels_that_name_what_they_do_not_have_are_refused() {
        assert_eq!(made(&[], "1", None), Err(LevelsFault::None));
        assert_eq!(
            made(&["1"], "2", None),
            Err(LevelsFault::NoSuchDefault("2".to_string()))
        );
        assert_eq!(
            made(&["1"], "1", Some("3")),
            Err(LevelsFault::NoSuchHighest("3".to_string()))
        );
        assert_eq!(
            made(&["1", "2"], "2", Some("1")),
            Err(LevelsFault::DefaultAboveHighest {
                default: "2".to_string(),
                highest: "1".to_string()
            })
        );
    }
}
