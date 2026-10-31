//! The images the owner sends with words, as `attachments` keeps them: what the owner may send (a PNG, JPEG, GIF or
//! WebP image of at most 5 MB, at most 10 to a message), keeping a message's images with its words in the write that
//! keeps the words, and reading them back, for the engine, which gives each to an agent's session with the words that
//! name it, and for the owner's page. The page sends an image as base64, and the command line takes it so.

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use sha2::{Digest, Sha256};

use crate::{Error, Result};

/// The kinds of image the owner may send, by their media types.
pub const KINDS: [&str; 4] = ["image/png", "image/jpeg", "image/gif", "image/webp"];

/// The most bytes one image may hold: 5 MB.
pub const MOST_BYTES: usize = 5 * 1024 * 1024;

/// The most images one message may carry.
pub const MOST: usize = 10;

/// What an image is named by in what an agent is given, before its id: `image:12`.
const PREFIX: &str = "image:";

/// An image as the owner sends it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub media_type: String,
    pub bytes: Vec<u8>,
}

/// Whether bytes begin as an image of a kind does: the signature its format opens with.
fn is_kind(media_type: &str, b: &[u8]) -> bool {
    match media_type {
        "image/png" => b.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => b.starts_with(b"\xff\xd8\xff"),
        "image/gif" => b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a"),
        "image/webp" => b.len() >= 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP",
        _ => false,
    }
}

/// Checks the images of one message, as the page does before it sends them: at most 10, each a PNG, JPEG, GIF or WebP
/// image whose bytes are of the kind it says, holding something and at most 5 MB. Says plainly what is wrong.
pub fn check(images: &[Image]) -> std::result::Result<(), String> {
    if images.len() > MOST {
        return Err(format!(
            "a message carries at most {MOST} images, and this one has {}",
            images.len()
        ));
    }
    for (i, im) in images.iter().enumerate() {
        let n = i + 1;
        if !KINDS.contains(&im.media_type.as_str()) {
            return Err(format!(
                "image {n} is of type {}: only PNG, JPEG, GIF and WebP images can be sent",
                im.media_type
            ));
        }
        if im.bytes.is_empty() {
            return Err(format!("image {n} is empty"));
        }
        if im.bytes.len() > MOST_BYTES {
            return Err(format!(
                "image {n} is {:.1} MB: an image can be at most 5 MB",
                im.bytes.len() as f64 / (1024.0 * 1024.0)
            ));
        }
        if !is_kind(&im.media_type, &im.bytes) {
            return Err(format!("image {n} is not the {} image it says it is", im.media_type));
        }
    }
    Ok(())
}

/// The words a message's images came with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Words {
    /// What the owner said to a work's line.
    Interjection(i64),
    /// The owner's words a matter of an architect holds: an owner_words matter's words, or the owner's reply to the
    /// engineer's question a matter holds.
    Matter(i64),
    /// The owner's answer to an ask.
    Ask(i64),
}

impl Words {
    fn column(self) -> (&'static str, i64) {
        match self {
            Words::Interjection(id) => ("interjection", id),
            Words::Matter(id) => ("matter", id),
            Words::Ask(id) => ("ask", id),
        }
    }
}

/// The SHA-256 of an image's bytes, in hex, as `attachments` keeps it.
pub fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Keeps a message's images with its words, in the order given, in the write that keeps the words; gives their ids.
/// Images the owner may not send are refused, and the write with them.
pub fn attach(tx: &Transaction<'_>, words: Words, images: &[Image]) -> Result<Vec<i64>> {
    check(images).map_err(Error::Abort)?;
    let (column, id) = words.column();
    let mut ids = Vec::new();
    for (i, im) in images.iter().enumerate() {
        tx.execute(
            &format!(
                "INSERT INTO attachments ({column}, position, media_type, size, hash, bytes)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
            ),
            params![
                id,
                i as i64 + 1,
                im.media_type,
                im.bytes.len() as i64,
                hash(&im.bytes),
                im.bytes
            ],
        )?;
        ids.push(tx.last_insert_rowid());
    }
    Ok(ids)
}

/// The images words came with, by their ids, in order.
pub fn of(conn: &Connection, words: Words) -> rusqlite::Result<Vec<i64>> {
    let (column, id) = words.column();
    let mut st = conn.prepare(&format!(
        "SELECT id FROM attachments WHERE {column} = ?1 ORDER BY position"
    ))?;
    st.query_map([id], |r| r.get(0))?.collect()
}

/// The images of what was said to a work's line: its own, or, for the owner's words a matter holds that the line is
/// told too, the matter's.
pub fn of_interjection(conn: &Connection, interjection: i64) -> rusqlite::Result<Vec<i64>> {
    let mut st = conn.prepare(
        "SELECT a.id FROM attachments a JOIN interjections j ON j.id = ?1
         WHERE a.interjection = j.id OR a.matter = j.matter ORDER BY a.position",
    )?;
    st.query_map([interjection], |r| r.get(0))?.collect()
}

/// A kept image, apart from its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    pub id: i64,
    pub media_type: String,
    pub size: i64,
    pub hash: String,
}

/// A kept image, apart from its bytes; none for an id that is no image's.
pub fn kept(conn: &Connection, id: i64) -> rusqlite::Result<Option<Kept>> {
    conn.query_row(
        "SELECT id, media_type, size, hash FROM attachments WHERE id = ?1",
        [id],
        |r| {
            Ok(Kept {
                id: r.get(0)?,
                media_type: r.get(1)?,
                size: r.get(2)?,
                hash: r.get(3)?,
            })
        },
    )
    .optional()
}

/// A kept image's media type and bytes; none for an id that is no image's.
pub fn bytes(conn: &Connection, id: i64) -> rusqlite::Result<Option<(String, Vec<u8>)>> {
    conn.query_row("SELECT media_type, bytes FROM attachments WHERE id = ?1", [id], |r| {
        Ok((r.get(0)?, r.get(1)?))
    })
    .optional()
}

/// The address an image is named by in what an agent is given, as an interjection is `ij:<id>`: `image:<id>`.
pub fn address(id: i64) -> String {
    format!("{PREFIX}{id}")
}

/// The line that names the images words came with, which ends the words wherever an agent is given them as a text;
/// empty for none.
pub fn named(ids: &[i64]) -> String {
    if ids.is_empty() {
        return String::new();
    }
    let all: Vec<String> = ids.iter().map(|i| address(*i)).collect();
    format!("Images: {}", all.join(", "))
}

/// Words as an agent is given them as a text: the words, then the line that names their images.
pub fn with_named(text: &str, ids: &[i64]) -> String {
    let line = named(ids);
    match (text.trim().is_empty(), line.is_empty()) {
        (_, true) => text.to_string(),
        (true, false) => line,
        (false, false) => format!("{}\n\n{line}", text.trim_end()),
    }
}

/// The images a text names by their addresses, each once, in the order first named: `image:` and its id, not inside
/// a longer word.
pub fn named_in(text: &str) -> Vec<i64> {
    let mut out = Vec::new();
    let mut rest = text;
    let mut before: Option<char> = None;
    while let Some(at) = rest.find(PREFIX) {
        let lead = rest[..at].chars().next_back().or(before);
        let after = &rest[at + PREFIX.len()..];
        let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
        let tail = after[digits.len()..].chars().next();
        let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
        if !digits.is_empty()
            && !word(lead)
            && !word(tail)
            && let Ok(id) = digits.parse::<i64>()
            && !out.contains(&id)
        {
            out.push(id);
        }
        before = Some(':');
        rest = after;
    }
    out
}

/// What the owner's words say of themselves to the architect as a matter: "The owner says: " and their first sentence,
/// or, for images sent with no words, how many.
pub fn owner_says(text: &str, images: usize) -> String {
    let text = text.trim();
    let sent = match images {
        0 => String::new(),
        1 => "an image".to_string(),
        n => format!("{n} images"),
    };
    match (text.is_empty(), sent.is_empty()) {
        (true, _) => format!("The owner sends {sent}"),
        (false, true) => format!("The owner says: {}", crate::one_sentence(text)),
        (false, false) => format!("The owner says, with {sent}: {}", crate::one_sentence(text)),
    }
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Bytes as base64, the standard alphabet with padding, as the command line takes an image.
pub fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The bytes base64 holds, the standard alphabet with padding, as the page sends an image; none for a text that is
/// not.
pub fn from_base64(text: &str) -> Option<Vec<u8>> {
    let t = text.as_bytes();
    if !t.len().is_multiple_of(4) {
        return None;
    }
    let value = |c: u8| ALPHABET.iter().position(|a| *a == c).map(|v| v as u32);
    let mut out = Vec::with_capacity(t.len() / 4 * 3);
    for (k, quad) in t.chunks(4).enumerate() {
        let last = k + 1 == t.len() / 4;
        let pad = quad.iter().rev().take_while(|c| **c == b'=').count();
        if pad > 2 || (pad > 0 && !last) {
            return None;
        }
        let mut n = 0u32;
        for (i, c) in quad.iter().enumerate() {
            let v = if i >= 4 - pad { 0 } else { value(*c)? };
            n = (n << 6) | v;
        }
        let three = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&three[..3 - pad]);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Actor, Db};

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n rest of a png";

    fn png() -> Image {
        Image {
            media_type: "image/png".into(),
            bytes: PNG.to_vec(),
        }
    }

    #[test]
    fn base64_goes_there_and_back() {
        for n in 0..8 {
            let b: Vec<u8> = (0..n).map(|i| (i * 37 + 200) as u8).collect();
            assert_eq!(from_base64(&base64(&b)).as_deref(), Some(b.as_slice()), "{n}");
        }
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
        assert_eq!(from_base64("TWE="), Some(b"Ma".to_vec()));
        assert_eq!(from_base64("TWE"), None);
        assert_eq!(from_base64("T=E="), None);
        assert_eq!(from_base64("TQ==TWFu"), None);
        assert_eq!(from_base64("TW!u"), None);
    }

    // The owner may send PNG, JPEG, GIF and WebP images of at most 5 MB, at most 10 to a message, each the kind it
    // says it is; anything else is refused, saying why.
    #[test]
    fn only_the_images_the_owner_may_send_are_taken() {
        assert!(check(&vec![png(); 10]).is_ok());
        assert!(check(&vec![png(); 11]).unwrap_err().contains("at most 10"));
        let pdf = Image {
            media_type: "application/pdf".into(),
            bytes: b"%PDF-1.7".to_vec(),
        };
        assert!(
            check(&[png(), pdf])
                .unwrap_err()
                .contains("image 2 is of type application/pdf")
        );
        let mut big = PNG.to_vec();
        big.resize(MOST_BYTES + 1, 0);
        let big = Image {
            media_type: "image/png".into(),
            bytes: big,
        };
        assert!(check(&[big]).unwrap_err().contains("at most 5 MB"));
        let lying = Image {
            media_type: "image/jpeg".into(),
            bytes: PNG.to_vec(),
        };
        assert!(check(&[lying]).unwrap_err().contains("not the image/jpeg image"));
        let webp = Image {
            media_type: "image/webp".into(),
            bytes: b"RIFF\x10\0\0\0WEBPVP8 ".to_vec(),
        };
        let gif = Image {
            media_type: "image/gif".into(),
            bytes: b"GIF89a....".to_vec(),
        };
        let jpeg = Image {
            media_type: "image/jpeg".into(),
            bytes: b"\xff\xd8\xff\xe0....".to_vec(),
        };
        assert!(check(&[webp, gif, jpeg]).is_ok());
    }

    // An image names itself by its address; the images a text names are found by it, each once, and not inside a
    // longer word.
    #[test]
    fn images_are_named_by_their_addresses() {
        assert_eq!(named(&[3, 12]), "Images: image:3, image:12");
        assert_eq!(with_named("Look.", &[3]), "Look.\n\nImages: image:3");
        assert_eq!(with_named("", &[3]), "Images: image:3");
        assert_eq!(with_named("Look.", &[]), "Look.");
        assert_eq!(
            named_in("See image:12, then image:3 and image:12 again; myimage:4, image:5x, image:, (image:7)."),
            [12, 3, 7]
        );
        assert_eq!(owner_says("", 2), "The owner sends 2 images");
        assert_eq!(owner_says("Look. More.", 1), "The owner says, with an image: Look.");
        assert_eq!(owner_says("Look.", 0), "The owner says: Look.");
    }

    // A message's images are kept with its words in order, and read back by them; an interjection that holds the
    // owner's words of a matter has the matter's.
    #[test]
    fn a_message_s_images_are_kept_with_its_words() {
        let mut db = Db::in_memory().unwrap();
        let jpeg = Image {
            media_type: "image/jpeg".into(),
            bytes: b"\xff\xd8\xff\xe0 jpeg".to_vec(),
        };
        let (j, m, ids) = db
            .write(Actor::Owner, |tx| {
                tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
                tx.execute(
                    "INSERT INTO works (flow_version, graph, at) VALUES (1, 'engineer', 'R1')",
                    [],
                )?;
                tx.execute(
                    "INSERT INTO architect_matters (project, kind, what, body, at) VALUES ('p', 'owner_words', 'w', '{}', 1)",
                    [],
                )?;
                let m = tx.last_insert_rowid();
                tx.execute(
                    "INSERT INTO interjections (work, speaker, text, matter) VALUES (1, 'owner', 'Look.', ?1)",
                    [m],
                )?;
                let j = tx.last_insert_rowid();
                let ids = attach(tx, Words::Matter(m), &[png(), jpeg.clone()])?;
                Ok((j, m, ids))
            })
            .unwrap();
        assert_eq!(of(db.conn(), Words::Matter(m)).unwrap(), ids);
        assert_eq!(of_interjection(db.conn(), j).unwrap(), ids);
        assert!(of(db.conn(), Words::Interjection(j)).unwrap().is_empty());
        let k = kept(db.conn(), ids[1]).unwrap().unwrap();
        assert_eq!((k.media_type.as_str(), k.size), ("image/jpeg", jpeg.bytes.len() as i64));
        assert_eq!(k.hash, hash(&jpeg.bytes));
        assert_eq!(
            bytes(db.conn(), ids[0]).unwrap().unwrap(),
            ("image/png".into(), PNG.to_vec())
        );
        assert!(kept(db.conn(), 99).unwrap().is_none());
        // Too many images are refused, and the write with them.
        let refused = db.write(Actor::Owner, |tx| attach(tx, Words::Matter(m), &vec![png(); 11]));
        assert!(refused.is_err());
        assert_eq!(of(db.conn(), Words::Matter(m)).unwrap().len(), 2);
    }
}
