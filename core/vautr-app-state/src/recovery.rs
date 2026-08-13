//! Emergency Recovery integration (emergency-recovery-account.md §2–§4).
//!
//! Client-side building blocks for the Recovery Key (RK) escape hatch:
//!   * deriving the Ed25519 recovery-auth keypair from a BIP-39 mnemonic (§2.2),
//!   * the onboarding proof-of-possession gate ("enter word 4, 12, 20", §3.2),
//!   * a small async state machine for the server challenge/verify/complete
//!     handshake (§2.3).
//!
//! The actual SVK unwrap/re-wrap lives in `vautr-keyring::recover` and
//! `vautr-crypto::recovery`; this module wires the authentication credential
//! (which `vautr-crypto` does not provide) and the UX validation.

use ed25519_dalek::SigningKey;
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroizing;

use base64::Engine;

use vautr_crypto::recovery::{decode_recovery_mnemonic, generate_recovery_mnemonic};

/// HKDF info string for the RK authentication key (§2.2 `"vautr-rk-auth"`).
const RK_AUTH_INFO: &[u8] = b"vautr-rk-auth";
/// Positions (1-based) of the words the onboarding gate asks the user to retype.
/// The spec (§3.2) mandates word 4, word 12 and word 20.
pub const POSSESSION_PROBE_INDICES: [usize; 3] = [4, 12, 20];

/// A validated Recovery Key and the credentials derived from it.
#[derive(Clone)]
pub struct RecoveryCredentials {
    /// The decoded 24-word BIP-39 mnemonic (used for KEK_RK / SVK unwrap).
    pub mnemonic: String,
    /// The derived Ed25519 recovery-auth signing key (proves possession, §2.3).
    pub signing_key: SigningKey,
    /// The corresponding Ed25519 public key (registered with the server).
    pub verifying_key_bytes: [u8; 32],
}

/// Generate a fresh 24-word Recovery Key mnemonic.
pub fn generate_recovery_key() -> String {
    generate_recovery_mnemonic().expect("CSPRNG-backed mnemonic generation")
}

/// Derive the recovery-auth Ed25519 keypair from a mnemonic (§2.2 step 3).
///
/// `RK_Bytes -> HKDF(RK_Bytes, "vautr-rk-auth") -> Ed25519_PrivateKey`.
/// We use the BIP-39 seed as the RK byte source (the same secret that derives
/// KEK_RK in `vautr-crypto::recovery`), so the mnemonic is the single source of
/// truth.
pub fn derive_recovery_credentials(mnemonic: &str) -> Option<RecoveryCredentials> {
    let m = decode_recovery_mnemonic(mnemonic).ok()?;
    let seed = m.to_seed(""); // BR-7: no passphrase
    let hk = Hkdf::<Sha256>::new(None, &seed[..]);
    let mut okm = Zeroizing::new([0u8; 32]);
    hk.expand(RK_AUTH_INFO, &mut *okm).ok()?;
    let signing_key = SigningKey::from_bytes(&okm);
    Some(RecoveryCredentials {
        mnemonic: m.to_string(),
        verifying_key_bytes: signing_key.verifying_key().to_bytes(),
        signing_key,
    })
}

/// The onboarding proof-of-possession gate (§3.2).
///
/// Given the generated mnemonic and the words the user retyped (indexed 0-based
/// into the probe positions), returns `true` only if every supplied word matches
/// its expected position. The caller asks for the words at `POSSESSION_PROBE_INDICES`
/// (word 4, 12, 20).
pub fn verify_recovery_key_possession(mnemonic: &str, supplied: &[&str]) -> bool {
    let Ok(m) = decode_recovery_mnemonic(mnemonic) else {
        return false;
    };
    let words: Vec<&str> = m.words().collect();
    if supplied.len() != POSSESSION_PROBE_INDICES.len() {
        return false;
    }
    POSSESSION_PROBE_INDICES
        .iter()
        .zip(supplied.iter())
        .all(|(pos, given)| {
            // `pos` is 1-based; probe position maps to 0-based index.
            words
                .get(pos - 1)
                .map(|w| w.eq_ignore_ascii_case(given))
                .unwrap_or(false)
        })
}

/// The server recovery handshake (§2.3): challenge → sign → verify → complete.
///
/// `sign_nonce` signs the server challenge nonce with the RK Ed25519 key (proof
/// of possession, §2.3 step 2). The transport round-trips are driven by the
/// orchestrator against its transport; this function is the crypto gate.
pub fn sign_nonce(creds: &RecoveryCredentials, nonce: &[u8]) -> String {
    use ed25519_dalek::Signer;
    let sig = creds.signing_key.sign(nonce);
    base64::engine::general_purpose::STANDARD.encode(sig.to_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_mnemonic() -> String {
        // Valid 24-word BIP-39 mnemonic (trezor test vector) with a checksum
        // that `bip39` accepts: 23x "abandon" + "art".
        [
            "abandon", "abandon", "abandon", "abandon", "abandon", "abandon", "abandon", "abandon",
            "abandon", "abandon", "abandon", "abandon", "abandon", "abandon", "abandon", "abandon",
            "abandon", "abandon", "abandon", "abandon", "abandon", "abandon", "abandon", "art",
        ]
        .join(" ")
    }

    #[test]
    fn possession_gate_accepts_correct_words() {
        let mn = sample_mnemonic();
        let words: Vec<&str> = decode_recovery_mnemonic(&mn).unwrap().words().collect();
        let supplied: Vec<&str> = POSSESSION_PROBE_INDICES
            .iter()
            .map(|p| words[*p - 1])
            .collect();
        assert!(verify_recovery_key_possession(&mn, &supplied));
    }

    #[test]
    fn possession_gate_rejects_wrong_words() {
        let mn = sample_mnemonic();
        // Wrong word at the first probe position.
        assert!(!verify_recovery_key_possession(
            &mn,
            &["wrongword", "x", "y"]
        ));
        // Wrong arity.
        assert!(!verify_recovery_key_possession(&mn, &["a", "b"]));
    }

    #[test]
    fn recovery_credentials_derive_stable_key() {
        let mn = sample_mnemonic();
        let a = derive_recovery_credentials(&mn).expect("creds");
        let b = derive_recovery_credentials(&mn).expect("creds");
        assert_eq!(a.verifying_key_bytes, b.verifying_key_bytes);
        assert_eq!(a.mnemonic, mn);
    }

    #[test]
    fn generated_key_is_valid_24_words() {
        let mn = generate_recovery_key();
        let words: Vec<&str> = mn.split_whitespace().collect();
        assert_eq!(words.len(), 24);
        assert!(derive_recovery_credentials(&mn).is_some());
    }
}
