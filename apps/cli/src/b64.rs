//! Base64 helpers for the Vautr wire format.
//!
//! The server encodes all binary fields (OPAQUE messages, secret ciphertext)
//! with `base64::STANDARD` (`=padding`, `+`/`/`), so the CLI must use the same
//! engine on the wire. Internal-only values (the locally stored project keys)
//! use the same helper for consistency.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

use crate::error::CliResult;

/// STANDARD base64 encode.
pub fn encode(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

/// STANDARD base64 decode (tolerates surrounding whitespace).
pub fn decode(s: &str) -> CliResult<Vec<u8>> {
    STANDARD
        .decode(s.trim())
        .map_err(crate::error::CliError::Base64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let data = b"\x00\x01secret\xff";
        assert_eq!(decode(&encode(data)).unwrap(), data);
    }

    #[test]
    fn standard_uses_padding() {
        assert_eq!(encode(b"a"), "YQ==");
        assert_eq!(decode("YQ==").unwrap(), b"a");
    }
}
