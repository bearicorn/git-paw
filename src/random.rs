//! OS-randomness helpers.
//!
//! `uuid` is not in the approved dependency set (`worker-session-id-capture`
//! design D3), so this reads 16 bytes straight from the OS and formats them
//! as a version-4 UUID by hand. Unix-only, matching the platforms git-paw
//! ships for ([`dist-workspace.toml`](../../dist-workspace.toml) targets are
//! all `*-apple-darwin` / `*-unknown-linux-gnu`).

use std::io::Read;
use std::path::Path;

use crate::error::PawError;

/// Generates a version-4-formatted UUID (`xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx`)
/// from OS-provided randomness.
///
/// Reads 16 bytes from `/dev/urandom`, sets the version nibble to `4` and the
/// variant nibble to RFC 4122's `10xx`, then formats the canonical
/// hyphenated, lowercase-hex string.
///
/// Returns `Err` rather than panicking when `/dev/urandom` cannot be opened
/// or read — a filesystem sandbox profile (Seatbelt, bwrap) that does not
/// grant it must surface a handled error on the launch path, not bring down
/// the whole process.
pub fn generate_uuid_v4() -> Result<String, PawError> {
    generate_uuid_v4_from(Path::new("/dev/urandom"))
}

/// Generates a hex-encoded token from `bytes` bytes of OS-provided randomness
/// (`2 * bytes` lowercase hex characters).
///
/// Used for the per-session broker authority credential. Returns `Err`
/// rather than panicking when `/dev/urandom` cannot be read, as
/// [`generate_uuid_v4`] does.
pub fn generate_hex_token(bytes: usize) -> Result<String, PawError> {
    generate_hex_token_from(Path::new("/dev/urandom"), bytes)
}

/// [`generate_hex_token`] parameterised on the randomness source.
fn generate_hex_token_from(source: &Path, bytes: usize) -> Result<String, PawError> {
    use std::fmt::Write;
    let mut buf = vec![0u8; bytes];
    std::fs::File::open(source)?.read_exact(&mut buf)?;
    Ok(buf
        .iter()
        .fold(String::with_capacity(bytes * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        }))
}

/// [`generate_uuid_v4`]'s implementation, parameterised on the randomness
/// source so the unreadable-source path is unit-testable without touching
/// the real `/dev/urandom`.
fn generate_uuid_v4_from(source: &Path) -> Result<String, PawError> {
    let mut bytes = [0u8; 16];
    let mut f = std::fs::File::open(source)?;
    f.read_exact(&mut bytes)?;

    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    Ok(format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15],
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        generate_hex_token, generate_hex_token_from, generate_uuid_v4, generate_uuid_v4_from,
    };
    use std::path::Path;

    /// Spec scenario groundwork: the id must match the v4 UUID shape
    /// (`xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx`, lowercase hex, version
    /// nibble `4`, variant nibble in `8`/`9`/`a`/`b`).
    #[test]
    fn generated_id_matches_v4_uuid_shape() {
        let id = generate_uuid_v4().expect("/dev/urandom is readable in the test environment");
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 5, "expected 5 hyphen-separated groups: {id}");
        assert_eq!(
            [
                parts[0].len(),
                parts[1].len(),
                parts[2].len(),
                parts[3].len(),
                parts[4].len()
            ],
            [8, 4, 4, 4, 12],
            "unexpected group lengths: {id}"
        );
        assert!(
            id.chars()
                .all(|c| c == '-' || c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "expected lowercase hex + hyphens only: {id}"
        );
        assert!(parts[2].starts_with('4'), "version nibble must be 4: {id}");
        let variant = parts[3].chars().next().unwrap();
        assert!(
            matches!(variant, '8' | '9' | 'a' | 'b'),
            "variant nibble must be 8/9/a/b: {id}"
        );
    }

    /// Spec scenario groundwork ("Each worker gets a distinct id"): repeated
    /// generation does not collide in a small sample.
    #[test]
    fn generated_ids_are_distinct() {
        let ids: std::collections::HashSet<String> = (0..100)
            .map(|_| generate_uuid_v4().expect("/dev/urandom is readable in the test environment"))
            .collect();
        assert_eq!(ids.len(), 100, "expected 100 distinct ids");
    }

    /// An unreadable randomness source (e.g. a sandbox profile that denies
    /// `/dev/urandom`) returns a handled `Err`, never a panic.
    #[test]
    fn unreadable_randomness_source_returns_error_not_panic() {
        let result = generate_uuid_v4_from(Path::new(
            "/nonexistent/path/git-paw-random-test-does-not-exist",
        ));
        assert!(
            result.is_err(),
            "an unreadable source must return Err, not panic or succeed"
        );
    }

    /// A 32-byte token is 64 lowercase hex characters, and two draws differ.
    #[test]
    fn hex_token_is_lowercase_hex_of_twice_the_byte_count_and_distinct() {
        let a = generate_hex_token(32).expect("/dev/urandom is readable");
        let b = generate_hex_token(32).expect("/dev/urandom is readable");
        assert_eq!(a.len(), 64, "{a}");
        assert!(
            a.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "{a}"
        );
        assert_ne!(a, b);
    }

    /// An unreadable source yields a handled `Err` for the token too.
    #[test]
    fn hex_token_from_unreadable_source_returns_error() {
        assert!(
            generate_hex_token_from(Path::new("/nonexistent/git-paw-random-test"), 32).is_err()
        );
    }
}
