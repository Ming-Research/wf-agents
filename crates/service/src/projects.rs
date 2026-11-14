//! The projects of the items: an item belongs to one project, whose repository its works run in. The installation's
//! settings name its projects; an item is added to one of them, which it keeps. The one project of an engine whose
//! agents follow a script comes from the script, not from settings, and its refusals name no key.

/// The projects an installation names, in the order of their names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Projects {
    names: Vec<String>,
    /// Whether the projects are the settings', whose key a refusal names.
    from_settings: bool,
}

impl Projects {
    /// The settings' projects, by their names.
    pub fn new(names: impl IntoIterator<Item = String>) -> Projects {
        let mut names: Vec<String> = names.into_iter().collect();
        names.sort();
        names.dedup();
        Projects {
            names,
            from_settings: true,
        }
    }

    /// One project, from no settings.
    pub fn one(name: &str) -> Projects {
        Projects {
            names: vec![name.to_string()],
            from_settings: false,
        }
    }

    /// The names of the projects, in their order.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// The project an item asks to be added to, where it is one of these; or why it cannot be, in words.
    pub fn of_item(&self, asked: &str) -> Result<String, String> {
        if self.names.iter().any(|name| name == asked) {
            return Ok(asked.to_string());
        }
        let known = self.names.join(", ");
        Err(match self.from_settings {
            true => format!("projects: the installation names no project {asked:?}; it names {known}"),
            false => format!("There is no project {asked:?}; there is {known}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_item_is_added_only_to_a_project_the_installation_names() {
        let projects = Projects::new(["wf".to_string(), "Firn".to_string(), "wf".to_string()]);
        assert_eq!(projects.names(), ["Firn", "wf"]);
        assert_eq!(projects.of_item("Firn"), Ok("Firn".to_string()));
        assert_eq!(
            projects.of_item("Halo"),
            Err("projects: the installation names no project \"Halo\"; it names Firn, wf".to_string())
        );
        let one = Projects::one("trial");
        assert_eq!(one.of_item("trial"), Ok("trial".to_string()));
        assert_eq!(
            one.of_item("other"),
            Err("There is no project \"other\"; there is trial".to_string())
        );
    }
}
