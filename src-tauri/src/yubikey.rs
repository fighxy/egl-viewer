//! The YubiKey 5C as a PIV card (docs/age-locked-folders.md §9).
//!
//! A connected card would do P-256 ECDH in a retired slot, the same operation
//! `age-plugin-yubikey` asks of the PIV applet. With no card, every call refuses and the
//! folder password and recovery secret are left to open the folder. Tests of the wrap use a
//! software P-256 key that produces the same `piv-p256` stanza; they do not come through here.

use library_core::{PivPolicy, PivPrompt, SourceError};
use library_native::PivCard;

const ABSENT: &str =
    "No YubiKey is connected. The password and the recovery key still open this folder.";

pub fn present() -> bool {
    false
}

pub fn enroll(_policy: PivPolicy, _pin: Option<&str>) -> Result<PivCard, SourceError> {
    Err(SourceError::NotSupported(ABSENT.into()))
}

pub fn agree(
    _serial: &str,
    _prompt: &PivPrompt,
    _ephemeral: &[u8],
) -> Result<[u8; 32], SourceError> {
    Err(SourceError::NotSupported(ABSENT.into()))
}
