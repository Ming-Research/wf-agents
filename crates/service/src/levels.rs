//! The levels of the items: how hard an item's work is, which chooses what its agents' rounds run on. The installation
//! names its levels; an item is given one as it is added, the one the installation names for an item that is given
//! none, and none above the highest it lets an item be at. A round runs at its item's level, or above it where its
//! line says so, but never above that highest level.
//!
//! The levels are in their order by their names: by number where they are numbers, by name otherwise, so that `10`
//! comes after `2`.

/// The levels of an installation's items.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Levels {
    /// The levels' names, lowest first.
    names: Vec<String>,
    /// Where among them the level an item is added at where it is given none is.
    default: usize,
    /// Where among them the highest level an item may be at is.
    highest: usize,
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
    /// The levels of these names, with the one an item is added at where it is given none, and the highest an item may
    /// be at: the top level where none is named.
    pub fn new(
        names: impl IntoIterator<Item = String>,
        default: &str,
        highest: Option<&str>,
    ) -> Result<Levels, LevelsFault> {
        let mut names: Vec<String> = names.into_iter().collect();
        names.sort_by_key(|name| (name.parse::<u64>().ok(), name.clone()));
        names.dedup();
        if names.is_empty() {
            return Err(LevelsFault::None);
        }
        let at = |name: &str| names.iter().position(|n| n == name);
        let default_at = at(default).ok_or_else(|| LevelsFault::NoSuchDefault(default.to_string()))?;
        let highest_at = match highest {
            Some(highest) => at(highest).ok_or_else(|| LevelsFault::NoSuchHighest(highest.to_string()))?,
            None => names.len() - 1,
        };
        if default_at > highest_at {
            return Err(LevelsFault::DefaultAboveHighest {
                default: default.to_string(),
                highest: names[highest_at].clone(),
            });
        }
        Ok(Levels {
            names,
            default: default_at,
            highest: highest_at,
        })
    }

    /// One level of this name, at which every item is added.
    pub fn one(name: &str) -> Levels {
        Levels {
            names: vec![name.to_string()],
            default: 0,
            highest: 0,
        }
    }

    /// The levels' names, lowest first.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// The level an item asked to be at is added at: the one asked for, or the default where none is; refused, in
    /// words, where it is no level or above the highest an item may be at.
    pub fn of_item(&self, asked: Option<&str>) -> Result<String, String> {
        match asked {
            None => Ok(self.names[self.default].clone()),
            Some(level) => self.at(level).map(|at| self.names[at].clone()),
        }
    }

    /// The level a round runs at: its item's, or this many above it, but none above the highest an item may be at. An
    /// item that was added before items had levels has none, and its rounds run at the default. Refused, in words,
    /// where the item's level is no longer one of the levels, or is above the highest an item may be at.
    pub fn of_round(&self, item: Option<&str>, above: u32) -> Result<String, String> {
        let at = match item {
            None => self.default,
            Some(level) => self.at(level).map_err(|why| format!("the item's level: {why}"))?,
        };
        let _ = above;
        Ok(self.names[at].clone())
    }

    /// Where the level of this name is, where an item may be at it; or why it may not, in words.
    fn at(&self, level: &str) -> Result<usize, String> {
        match self.names.iter().position(|n| n == level) {
            Some(at) if at <= self.highest => Ok(at),
            Some(_) => Err(format!(
                "{level} is above {}, the highest level an item may be at",
                self.names[self.highest]
            )),
            None => Err(format!("{level} is no level: the levels are {}", self.names.join(", "))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn made(names: &[&str], default: &str, highest: Option<&str>) -> Result<Levels, LevelsFault> {
        Levels::new(names.iter().map(|n| n.to_string()), default, highest)
    }

    #[test]
    fn levels_go_by_number_where_they_are_numbers() {
        let levels = made(&["10", "2", "1"], "2", None).unwrap();
        assert_eq!(levels.names(), ["1", "2", "10"]);
    }

    #[test]
    fn an_item_is_at_the_default_where_it_asks_for_none_and_never_above_the_highest() {
        let levels = made(&["1", "2", "3", "4"], "2", Some("3")).unwrap();
        assert_eq!(levels.of_item(None).unwrap(), "2");
        assert_eq!(levels.of_item(Some("1")).unwrap(), "1");
        assert_eq!(levels.of_item(Some("3")).unwrap(), "3");
        let above = levels.of_item(Some("4")).unwrap_err();
        assert_eq!(above, "4 is above 3, the highest level an item may be at");
        let none = levels.of_item(Some("5")).unwrap_err();
        assert_eq!(none, "5 is no level: the levels are 1, 2, 3, 4");
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
