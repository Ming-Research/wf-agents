//! A text of the flow's files as it is sent: what is written as written, and each place filled in a run apart, so that
//! one text serves the engine, which fills every place, and the page, which shows the places open. The pieces are the
//! system prompt's ([`flow::system`]), so that the page shows every text sent alike, and a text that is not the flow's
//! words is set apart as the system prompt sets a file of the project apart ([`fenced`]).

pub use flow::system::{Piece, as_written, fenced, fill, filled, join, with_slots, written};

/// Pieces with each place of this name replaced by these pieces, which are not looked into again; what follows the
/// place follows them as a place filled with them would be followed ([`flow::agents::after`]).
pub fn put(pieces: Vec<Piece>, name: &str, with: &[Piece]) -> Vec<Piece> {
    let put_here = |piece: &Piece| matches!(piece, Piece::Slot { name: n } if n == name);
    // What a place put here ends with, where the pieces put end in a text.
    let ends = match with.last() {
        Some(Piece::Text { text }) => Some(text.as_str()),
        _ => None,
    };
    let mut out: Vec<Piece> = Vec::new();
    for (at, piece) in pieces.iter().enumerate() {
        let before = at.checked_sub(1).is_some_and(|b| put_here(&pieces[b]));
        match (piece, ends) {
            _ if put_here(piece) => out.extend(with.iter().cloned()),
            (Piece::Text { text }, Some(value)) if before => {
                // What follows the words: the next place, as written, or what this puts there.
                let then = pieces.get(at + 1).map(|next| match next {
                    Piece::Slot { .. } if put_here(next) => as_written(with),
                    next => as_written(std::slice::from_ref(next)),
                });
                out.push(Piece::Text {
                    text: flow::agents::after(value, text, then.as_deref()).to_string(),
                });
            }
            (piece, _) => out.push(piece.clone()),
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

    // Pieces put in a place are followed as a place filled with them is: a stop after them is left out where they end a
    // sentence and nothing or a space follows it, and stays where a place left open does.
    #[test]
    fn what_follows_pieces_put_in_a_place_follows_them_as_a_filled_place() {
        assert_eq!(
            as_written(&put(with_slots("{d}. Next"), "d", &[text("Done.")])),
            "Done. Next"
        );
        assert_eq!(as_written(&put(with_slots("{d}."), "d", &[text("Done?")])), "Done?");
        assert_eq!(
            as_written(&put(with_slots("{d}.{e}"), "d", &[text("Done.")])),
            "Done..{e}"
        );
        assert_eq!(
            as_written(&put(with_slots("{d}. Next"), "d", &[text("Done")])),
            "Done. Next"
        );
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
