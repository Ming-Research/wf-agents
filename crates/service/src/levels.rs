//! The levels of the items: how hard an item's work is, which chooses what its agents' rounds run on. The installation's
//! settings name its levels, each with what work it is for; an item is given one as it is added, or later by the owner,
//! the one the settings name for an item that is given none, and none above the highest they let an item be at. A round
//! runs at its item's level, or above it where its line says so, but never above the highest level they let one round
//! run at, which is no lower than the highest an item may be at.
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
    items_highest: usize,
    /// Where among them the highest level one round may run at is: no lower than `items_highest`, so that a round of
    /// an item at its highest level can run at it.
    rounds_highest: usize,
    /// Whether the levels are the settings', whose keys a refusal names.
    from_settings: bool,
    /// The most words an item's title may have, as [`crate::title_words`] counts them; none, any number. These are the
    /// rules the settings give an item as it is added or changed, and this one is held with them.
    title_max: Option<usize>,
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
    NoSuchItemsHighest(String),
    /// The highest level one round may run at is not among them.
    NoSuchRoundsHighest(String),
    /// The level an item is added at where it is given none is above the highest an item may be at.
    DefaultAboveItemsHighest { default: String, highest: String },
    /// The highest level an item may be at is above the highest level one round may run at.
    ItemsHighestAboveRounds { items: String, rounds: String },
}

/// The level a round runs at, and how far it was raised.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Round {
    /// The level it runs at.
    pub level: String,
    /// How many levels above its item's it runs at: the line's raise, less what the highest level one round may run at
    /// took off it.
    pub raised: u32,
    /// How many levels above its item's its line asks it to run at, whether or not it could.
    pub asks: u32,
}

impl Levels {
    /// The settings' levels, with the one an item is added at where it is given none, the highest an item may be at and
    /// the highest one round may run at: the top level where none is named.
    pub fn new(
        levels: impl IntoIterator<Item = Level>,
        default: &str,
        items_highest: Option<&str>,
        rounds_highest: Option<&str>,
    ) -> Result<Levels, LevelsFault> {
        let mut levels: Vec<Level> = levels.into_iter().collect();
        levels.sort_by_key(|level| (level.name.parse::<u64>().ok(), level.name.clone()));
        levels.dedup_by(|a, b| a.name == b.name);
        if levels.is_empty() {
            return Err(LevelsFault::None);
        }
        let at = |name: &str| levels.iter().position(|l| l.name == name);
        let default_at = at(default).ok_or_else(|| LevelsFault::NoSuchDefault(default.to_string()))?;
        let items_at = match items_highest {
            Some(highest) => at(highest).ok_or_else(|| LevelsFault::NoSuchItemsHighest(highest.to_string()))?,
            None => levels.len() - 1,
        };
        let rounds_at = match rounds_highest {
            Some(highest) => at(highest).ok_or_else(|| LevelsFault::NoSuchRoundsHighest(highest.to_string()))?,
            None => levels.len() - 1,
        };
        if default_at > items_at {
            return Err(LevelsFault::DefaultAboveItemsHighest {
                default: default.to_string(),
                highest: levels[items_at].name.clone(),
            });
        }
        if items_at > rounds_at {
            return Err(LevelsFault::ItemsHighestAboveRounds {
                items: levels[items_at].name.clone(),
                rounds: levels[rounds_at].name.clone(),
            });
        }
        Ok(Levels {
            levels,
            default: default_at,
            items_highest: items_at,
            rounds_highest: rounds_at,
            from_settings: true,
            title_max: None,
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
            items_highest: 0,
            rounds_highest: 0,
            from_settings: false,
            title_max: None,
        }
    }

    /// These levels, holding a title to this many words at most.
    pub fn with_title_max(mut self, title_max: usize) -> Levels {
        self.title_max = Some(title_max);
        self
    }

    /// Whether an item's title is within the words the settings allow; if not, why not.
    pub fn title_fits(&self, title: &str) -> Result<(), String> {
        match self.title_max {
            Some(max) => crate::title_fits(title, max),
            None => Ok(()),
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
    pub fn items_highest(&self) -> &str {
        &self.levels[self.items_highest].name
    }

    /// The key of the settings that holds a round to its highest level, which a page says on pointing at a round it
    /// held; none for the one level of no settings.
    pub fn rounds_highest_key(&self) -> Option<&'static str> {
        self.from_settings.then_some("agents.highest_level")
    }

    /// The level an item asked to be at is given: the one asked for, or the default where none is; refused, in words,
    /// where it is no level or above the highest an item may be at.
    pub fn of_item(&self, asked: Option<&str>) -> Result<String, String> {
        match asked {
            None => Ok(self.default().to_string()),
            Some(level) => self.at(level).map(|at| self.levels[at].name.clone()),
        }
    }

    /// The level a round runs at: its item's, or this many above it, but none above the highest one round may run at.
    /// An item that has no level runs at the default: no level can be given to one whose level was never kept without
    /// guessing. Refused, in words, where the item's level is not one of the levels, or is above the highest an item
    /// may be at: such an item is not run at another level, which would spend at a model nobody chose for it.
    pub fn of_round(&self, item: Option<&str>, above: u32) -> Result<Round, String> {
        let at = match item {
            None => self.default,
            Some(level) => self.at(level).map_err(|why| format!("the item's level: {why}"))?,
        };
        let wanted = at.saturating_add(usize::try_from(above).unwrap_or(usize::MAX));
        let ran = wanted.min(self.rounds_highest);
        Ok(Round {
            level: self.levels[ran].name.clone(),
            raised: u32::try_from(ran - at).unwrap_or(u32::MAX),
            asks: above,
        })
    }

    /// Where the level of this name is, where an item may be at it; or why it may not, in words.
    fn at(&self, level: &str) -> Result<usize, String> {
        let key = |key: &str| match self.from_settings {
            true => format!(" ({key})"),
            false => String::new(),
        };
        match self.levels.iter().position(|l| l.name == level) {
            Some(at) if at <= self.items_highest => Ok(at),
            Some(_) => Err(format!(
                "{level} is above {}, the highest level an item may be at{}",
                self.items_highest(),
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

    fn made(names: &[&str], default: &str, items: Option<&str>, rounds: Option<&str>) -> Result<Levels, LevelsFault> {
        let levels = names.iter().map(|n| Level {
            name: n.to_string(),
            desc: format!("Work of level {n}"),
        });
        Levels::new(levels, default, items, rounds)
    }

    fn names(levels: &Levels) -> Vec<&str> {
        levels.levels().iter().map(|l| l.name.as_str()).collect()
    }

    /// What a round ran at, how far it was raised and how far its line asked: `(level, raised, asks)`.
    fn ran(levels: &Levels, item: Option<&str>, above: u32) -> (String, u32, u32) {
        let round = levels.of_round(item, above).unwrap();
        (round.level, round.raised, round.asks)
    }

    #[test]
    fn levels_go_by_number_where_they_are_numbers() {
        let levels = made(&["10", "2", "1"], "2", None, None).unwrap();
        assert_eq!(names(&levels), ["1", "2", "10"]);
        assert_eq!(levels.levels()[2].desc, "Work of level 10");
        assert_eq!((levels.default(), levels.items_highest()), ("2", "10"));
        assert_eq!(
            ran(&levels, Some("1"), 5),
            ("10".to_string(), 2, 5),
            "the top level, where none is named"
        );
    }

    #[test]
    fn an_item_is_at_the_default_where_it_asks_for_none_and_never_above_its_highest() {
        let levels = made(&["1", "2", "3", "4"], "2", Some("3"), None).unwrap();
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
        let held = made(&["1", "2", "3", "4"], "2", Some("3"), Some("3")).unwrap();
        assert_eq!(
            held.of_item(Some("4")).unwrap_err(),
            above,
            "the round's highest does not move it"
        );
    }

    #[test]
    fn a_round_runs_above_its_item_s_level_where_its_line_says_up_to_the_rounds_highest() {
        // An item at most at 3, a round at most at 4: a line one above runs on 4 for an item at 3.
        let levels = made(&["1", "2", "3", "4"], "2", Some("3"), Some("4")).unwrap();
        assert_eq!(ran(&levels, Some("1"), 0), ("1".to_string(), 0, 0));
        assert_eq!(ran(&levels, Some("1"), 1), ("2".to_string(), 1, 1));
        assert_eq!(ran(&levels, Some("3"), 1), ("4".to_string(), 1, 1));
        assert_eq!(
            ran(&levels, Some("3"), 2),
            ("4".to_string(), 1, 2),
            "held by the rounds' highest"
        );
        assert_eq!(
            ran(&levels, None, 1),
            ("3".to_string(), 1, 1),
            "an item of no level is at the default"
        );
        // With no highest for rounds named, the top level: the items' highest holds no round.
        let open = made(&["1", "2", "3", "4"], "2", Some("3"), None).unwrap();
        assert_eq!(ran(&open, Some("3"), 1), ("4".to_string(), 1, 1));
        // Both at 3: the round of an item at 3 is held there, and says its line asked for one more.
        let held = made(&["1", "2", "3", "4"], "2", Some("3"), Some("3")).unwrap();
        assert_eq!(ran(&held, Some("2"), 1), ("3".to_string(), 1, 1));
        assert_eq!(ran(&held, Some("3"), 1), ("3".to_string(), 0, 1));
        let gone = held.of_round(Some("9"), 0).unwrap_err();
        assert!(gone.starts_with("the item's level: 9 is no level"), "{gone}");
        let top = made(&["1", "2"], "1", None, None).unwrap();
        assert_eq!(
            ran(&top, Some("2"), 1),
            ("2".to_string(), 0, 1),
            "with none named, the top level"
        );
    }

    #[test]
    fn the_one_level_of_no_settings_names_no_key_when_it_refuses() {
        let one = Levels::one("1", "The one level");
        assert_eq!(one.of_item(None).unwrap(), "1");
        assert_eq!(one.of_item(Some("2")).unwrap_err(), "2 is no level: the levels are 1");
        assert_eq!(one.rounds_highest_key(), None);
        let settings = made(&["1"], "1", None, None).unwrap();
        assert_eq!(settings.rounds_highest_key(), Some("agents.highest_level"));
    }

    #[test]
    fn levels_that_name_what_they_do_not_have_are_refused() {
        assert_eq!(made(&[], "1", None, None), Err(LevelsFault::None));
        assert_eq!(
            made(&["1"], "2", None, None),
            Err(LevelsFault::NoSuchDefault("2".to_string()))
        );
        assert_eq!(
            made(&["1"], "1", Some("3"), None),
            Err(LevelsFault::NoSuchItemsHighest("3".to_string()))
        );
        assert_eq!(
            made(&["1"], "1", None, Some("3")),
            Err(LevelsFault::NoSuchRoundsHighest("3".to_string()))
        );
        assert_eq!(
            made(&["1", "2"], "2", Some("1"), None),
            Err(LevelsFault::DefaultAboveItemsHighest {
                default: "2".to_string(),
                highest: "1".to_string()
            })
        );
        assert_eq!(
            made(&["1", "2", "3"], "1", Some("3"), Some("2")),
            Err(LevelsFault::ItemsHighestAboveRounds {
                items: "3".to_string(),
                rounds: "2".to_string()
            })
        );
        assert_eq!(
            made(&["1", "2", "3"], "1", None, Some("2")),
            Err(LevelsFault::ItemsHighestAboveRounds {
                items: "3".to_string(),
                rounds: "2".to_string()
            }),
            "an items' highest left out is the top level, above the rounds'"
        );
    }
}
