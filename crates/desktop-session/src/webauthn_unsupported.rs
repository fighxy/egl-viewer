//! App-login WebAuthn on platforms without the Windows WebAuthn API.
//! Enrollment and login refuse; password and PIN login still work.

use app_store::{Authenticator, WebAuthnAssertion, WebAuthnCredential};
use library_core::SourceError;

const UNSUPPORTED: &str = "YubiKey login is available on Windows only.";

pub struct SystemWebAuthn;

impl Authenticator for SystemWebAuthn {
    fn register(&mut self, _challenge: &[u8]) -> Result<WebAuthnCredential, SourceError> {
        Err(SourceError::NotSupported(UNSUPPORTED.into()))
    }

    fn authenticate(
        &mut self,
        _challenge: &[u8],
        _credential_id: &[u8],
    ) -> Result<WebAuthnAssertion, SourceError> {
        Err(SourceError::NotSupported(UNSUPPORTED.into()))
    }
}
