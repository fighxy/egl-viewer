//! The app-login WebAuthn bridge (docs/spec.md §13.2).
//!
//! A YubiKey 5C touch creates or asserts a credential. The app stores the credential id and
//! the P-256 public key and checks the assertion; the touch does not unwrap a folder identity
//! and login does not ask for a PIN. With no authenticator, enrollment and login refuse.
//! Tests assert a software P-256 credential through the same check, without this bridge.

use app_store::{Authenticator, WebAuthnAssertion, WebAuthnCredential};
use library_core::SourceError;

pub struct SystemWebAuthn;

impl Authenticator for SystemWebAuthn {
    fn register(&mut self, _challenge: &[u8]) -> Result<WebAuthnCredential, SourceError> {
        Err(SourceError::NotSupported(
            "Connect a YubiKey 5C and touch it. This login does not unlock folders.".into(),
        ))
    }

    fn authenticate(
        &mut self,
        _challenge: &[u8],
        _credential_id: &[u8],
    ) -> Result<WebAuthnAssertion, SourceError> {
        Err(SourceError::NotSupported(
            "Touch the YubiKey to log in. No YubiKey is connected.".into(),
        ))
    }
}
