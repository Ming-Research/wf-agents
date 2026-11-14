//! A text of the flow's files as it is sent: what is written as written, and each place filled in a run apart, so that
//! one text serves the engine, which fills every place, and the page, which shows the places open. The pieces are the
//! system prompt's ([`flow::system`]), so that the page shows every text sent alike.

pub use flow::system::{Piece, as_written, fill, join, with_slots, written};

/// Pieces with the places these give filled, as [`fill`] fills them.
pub fn filled(pieces: Vec<Piece>, known: &[(&str, &str)]) -> Vec<Piece> {
    let pieces = (pieces.into_iter())
        .map(|piece| match &piece {
            Piece::Slot { name } => match known.iter().find(|(k, _)| k == name) {
                Some((_, value)) => Piece::Text {
                    text: value.to_string(),
                },
                None => piece,
            },
            Piece::Text { .. } => piece,
        })
        .collect();
    join(pieces)
}

/// Pieces with each place of this name replaced by these pieces, which are not looked into again.
pub fn put(pieces: Vec<Piece>, name: &str, with: &[Piece]) -> Vec<Piece> {
    let mut out = Vec::new();
    for piece in pieces {
        match &piece {
            Piece::Slot { name: n } if n == name => out.extend(with.iter().cloned()),
            _ => out.push(piece),
        }
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
