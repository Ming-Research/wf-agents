//! The page's source stamp, and the check of it that the build script of `web` makes. `npm run build` writes
//! `client/dist/source.sha256`: a line for each file the build read outside `node_modules` (the sources, the
//! configuration, the lock file, the entry page), the SHA-256 of its bytes in hex, two spaces, its path from `client/`.
//! The build script reads each of those files again, so that a page built from other sources than the ones the program
//! is built with is refused, whatever made it so. A file the build does not read, a test or a cache of the type
//! checker, is not in the stamp and changes nothing. The hash is written here, not taken from a package, because a
//! package would change the lock file, which every CI run holds fixed (`--locked`); the tests hold it to the standard's
//! own examples.

use std::path::Path;

/// What follows every refusal of the check.
pub const MEND: &str = "run `npm ci` and `npm run build` in client/";

/// The SHA-256 of `bytes` in hex (FIPS 180-4).
pub fn sha256_hex(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98,
        0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
        0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8,
        0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819,
        0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
        0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];
    // The message, a one bit, zeros, and its length in bits, to a whole number of 64-byte blocks.
    let mut message = bytes.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&((bytes.len() as u64) * 8).to_be_bytes());
    let (blocks, _) = message.as_chunks::<64>();
    for block in blocks {
        let mut w = [0u32; 64];
        let (words, _) = block.as_chunks::<4>();
        for (i, word) in words.iter().enumerate() {
            w[i] = u32::from_be_bytes(*word);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for (k, word) in K.iter().zip(w.iter()) {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ (!e & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(choose)
                .wrapping_add(*k)
                .wrapping_add(*word);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (slot, add) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(add);
        }
    }
    state.iter().map(|word| format!("{word:08x}")).collect()
}

/// The files the stamp lists, each as the hash it holds and its path from `client/`; or what is wrong with a line.
pub fn parse(stamp: &str) -> Result<Vec<(String, String)>, String> {
    let mut listed = Vec::new();
    for (n, line) in stamp.lines().enumerate() {
        let Some((hash, path)) = line.split_once("  ") else {
            return Err(format!("line {}: no hash and path", n + 1));
        };
        let hash_ok = hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if !hash_ok {
            return Err(format!("line {}: {hash:?} is no SHA-256", n + 1));
        }
        if path.is_empty() || path.starts_with('/') || path.split('/').any(|part| part == "..") {
            return Err(format!("line {}: {path:?} is no path from client/", n + 1));
        }
        listed.push((hash.to_string(), path.to_string()));
    }
    Ok(listed)
}

/// Reads every file the stamp lists, under `client`, and returns their paths from `client/` where each is as the page
/// was built; or says every file that is not and every other fault, with how to mend it.
pub fn check(client: &Path, stamp: &str) -> Result<Vec<String>, String> {
    let listed = parse(stamp).map_err(|e| format!("client/dist/source.sha256 is unreadable: {e}; {MEND}"))?;
    // A stamp that names no source shows no more than a missing one that the page was made from the sources at hand.
    if listed.is_empty() {
        return Err(format!(
            "client/dist/source.sha256 lists no file, so the page is not shown to be built from the sources of client/; {MEND}"
        ));
    }
    let mut wrong = Vec::new();
    for (hash, path) in &listed {
        match std::fs::read(client.join(path)) {
            Ok(bytes) if sha256_hex(&bytes) == *hash => {}
            Ok(_) => wrong.push(format!("{path} is not as the page was built from it")),
            Err(e) => wrong.push(format!("{path} cannot be read ({e}), and the page was built from it")),
        }
    }
    if wrong.is_empty() {
        Ok(listed.into_iter().map(|(_, path)| path).collect())
    } else {
        Err(format!(
            "client/dist was not built from the sources of client/: {}; {MEND}",
            wrong.join("; ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hash_is_the_standards_on_its_own_examples() {
        // FIPS 180-4 and NIST's examples: the empty message, one block, the message of two blocks, and a million a's.
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        assert_eq!(
            sha256_hex(&vec![b'a'; 1_000_000]),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    fn stamped(client: &Path, files: &[(&str, &str)]) -> String {
        let mut stamp = String::new();
        for (path, text) in files {
            let place = client.join(path);
            std::fs::create_dir_all(place.parent().unwrap()).unwrap();
            std::fs::write(&place, text).unwrap();
            stamp.push_str(&format!("{}  {path}\n", sha256_hex(text.as_bytes())));
        }
        stamp
    }

    #[test]
    fn a_page_built_from_the_sources_at_hand_holds() {
        let client = tempfile::tempdir().unwrap();
        let stamp = stamped(client.path(), &[("src/a.ts", "a"), ("package-lock.json", "{}")]);
        assert_eq!(check(client.path(), &stamp).unwrap(), ["src/a.ts", "package-lock.json"]);
    }

    #[test]
    fn a_stamp_that_does_not_hold_is_refused_with_how_to_mend_it() {
        let client = tempfile::tempdir().unwrap();
        let stamp = stamped(
            client.path(),
            &[("src/a.ts", "a"), ("src/b.ts", "b"), ("src/c.ts", "c")],
        );
        std::fs::write(client.path().join("src/a.ts"), "changed").unwrap();
        std::fs::remove_file(client.path().join("src/b.ts")).unwrap();
        let said = check(client.path(), &stamp).unwrap_err();
        assert!(said.contains("src/a.ts is not as the page was built from it"), "{said}");
        assert!(said.contains("src/b.ts cannot be read"), "{said}");
        assert!(!said.contains("src/c.ts"), "{said}");
        assert!(said.ends_with(MEND), "{said}");
    }

    #[test]
    fn a_stamp_that_cannot_be_read_is_refused() {
        let client = tempfile::tempdir().unwrap();
        for stamp in [
            "not a stamp\n".to_string(),
            "abc  src/a.ts\n".to_string(),
            format!("{}  /etc/passwd\n", "0".repeat(64)),
            format!("{}  ../outside\n", "0".repeat(64)),
        ] {
            let said = check(client.path(), &stamp).unwrap_err();
            assert!(said.contains("source.sha256 is unreadable"), "{said}");
            assert!(said.ends_with(MEND), "{said}");
        }
    }

    #[test]
    fn a_stamp_that_lists_no_file_is_refused() {
        let client = tempfile::tempdir().unwrap();
        let said = check(client.path(), "").unwrap_err();
        assert!(said.contains("lists no file"), "{said}");
        assert!(said.ends_with(MEND), "{said}");
    }

    #[test]
    fn a_file_the_build_does_not_read_may_change() {
        let client = tempfile::tempdir().unwrap();
        let stamp = stamped(client.path(), &[("src/a.ts", "a")]);
        std::fs::write(client.path().join("src/a.test.ts"), "a test, written after").unwrap();
        std::fs::write(client.path().join("tsconfig.tsbuildinfo"), "kept by the type checker").unwrap();
        assert_eq!(check(client.path(), &stamp).unwrap(), ["src/a.ts"]);
    }
}
