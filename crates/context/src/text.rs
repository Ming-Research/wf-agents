//! A text of the flow's files as it is sent: what is written as written, and each place filled in a run apart, so that
//! one text serves the engine, which fills every place, and the page, which shows the places open. The pieces are the
//! system prompt's ([`flow::system`]), so that the page shows every text sent alike, and a text that is not the flow's
//! words is set apart as the system prompt sets a file of the project apart ([`fenced`]).

pub use flow::system::{Piece, as_written, fenced, fill, filled, join, with_slots, written};

/// Pieces with each place of this name replaced by these pieces, which are not looked into again; what follows the
/// place follows them as a place filled with them would be followed ([`flow::agents::after`]).
pub fn put(pieces: Vec<Piece>, name: &str, with: &[Piece]) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::new();
    // What the place just before was filled with ends in this text, where it was filled with one.
    let mut ends: Option<String> = None;
    for piece in pieces {
        match piece {
            Piece::Slot { name: n } if n == name => {
                out.extend(with.iter().cloned());
                ends = match with.last() {
                    Some(Piece::Text { text }) => Some(text.clone()),
                    _ => None,
                };
                continue;
            }
            Piece::Text { text } => out.push(Piece::Text {
                text: ends
                    .as_deref()
                    .map_or(text.as_str(), |value| flow::agents::after(value, &text))
                    .to_string(),
            }),
            piece => out.push(piece),
        }
        ends = None;
    }
    join(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(text: &str) -> Piece {
        Piece::Text { text: text.into() }
    }

    fn slot(name: &str) -> Piece {
        Piece::Slot { name: name.into() }
    }

    #[test]
    fn what_puts_pieces_in_a_place_is_not_looked_into_again() {
        assert_eq!(
            put(with_slots("a {b} c"), "b", &[slot("x"), text("{y}")]),
            vec![text("a "), slot("x"), text("{y} c")]
        );
        assert_eq!(
            filled(with_slots("## {task} {x}"), &[("task", "say {{q}} and {x}")]),
            vec![text("## say {{q}} and {x} "), slot("x")]
        );
    }
}
