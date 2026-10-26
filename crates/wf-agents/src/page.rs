//! The page that shows a flow as a state machine: each graph drawn as a diagram, each cell's inputs, returns, rows
//! and checks a click away, and what changed against another flow. The page draws itself from the compiled flow it
//! carries, so it shows exactly what the engine would run.

/// The page's markup and script; the compiled flows go where it says `{{flow}}` and `{{against}}`.
const TEMPLATE: &str = include_str!("page.html");

/// The page for a compiled flow; with `against`, it also lists what changed from that one.
pub fn page(flow: &flow::Compiled, against: Option<&flow::Compiled>) -> Result<String, String> {
    let base = match against {
        Some(c) => data(c)?,
        None => "null".to_string(),
    };
    // The flows go in last to first, so that neither is searched for the other's placeholder.
    let (head, rest) = TEMPLATE
        .split_once("{{flow}}")
        .ok_or("the page template has no {{flow}}")?;
    let (mid, tail) = rest
        .split_once("{{against}}")
        .ok_or("the page template has no {{against}} after {{flow}}")?;
    Ok([head, &data(flow)?, mid, &base, tail].concat())
}

/// A compiled flow as the JSON a script element can hold: `</` would end the element early, and JSON may write it
/// as `<\/`.
fn data(c: &flow::Compiled) -> Result<String, String> {
    serde_json::to_string(c)
        .map(|s| s.replace("</", "<\\/"))
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compiled() -> flow::Compiled {
        let relations = db::relations(db::SCHEMA).unwrap();
        flow::compile(flow::FLOW, &flow::Context { relations }).unwrap()
    }

    /// The JSON held by the script element with this id.
    fn held(page: &str, id: &str) -> serde_json::Value {
        let open = format!("<script type=\"application/json\" id=\"{id}\">");
        let start = page.find(&open).unwrap() + open.len();
        let end = start + page[start..].find("</script>").unwrap();
        serde_json::from_str(&page[start..end]).unwrap()
    }

    #[test]
    fn the_page_carries_the_flow_whole_and_ends_no_script_early() {
        let c = compiled();
        let p = page(&c, Some(&c)).unwrap();
        assert!(!p.contains("{{flow}}") && !p.contains("{{against}}"));
        assert_eq!(held(&p, "flow"), serde_json::to_value(&c).unwrap());
        assert_eq!(held(&p, "against"), serde_json::to_value(&c).unwrap());
    }

    #[test]
    fn a_page_with_nothing_to_compare_against_carries_null() {
        let p = page(&compiled(), None).unwrap();
        assert_eq!(held(&p, "against"), serde_json::Value::Null);
    }
}
