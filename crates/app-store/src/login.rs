//! App login (docs/spec.md §13). Exactly one of password, PIN or WebAuthn, or none.
//!
//! This is the gate in front of Welcome. It does not wrap age identities and it does not
//! decrypt files. A folder password never satisfies it. WebAuthn stores the credential id
//! and the P-256 public key; the assertion is a signature over the challenge, checked here.
//! Touch and a PIN are not combined: choosing WebAuthn means login does not ask for a PIN.

use std::fmt;

use argon2::{Algorithm, Argon2, Params, PasswordHasher, PasswordVerifier, Version};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use library_core::SourceError;
use p256::ecdsa::signature::{Signer, Verifier};
use p256::ecdsa::{Signature, SigningKey, VerifyingKey};
use rand::RngCore;
use rand::rngs::OsRng;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;

use crate::AppStore;
use crate::json_file::JsonFile;

const LOGIN_FILE: &str = "login.json";
/// Relying party for the app gate. Not a folder, and not a URL the library is opened from.
const ORIGIN: &str = "https://egl-viewer.local";
const RP_ID: &str = "egl-viewer.local";

/// Which login is turned on. The file holds exactly one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LoginMethod {
    None,
    Password,
    Pin,
    WebAuthn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LoginStatus {
    pub method: LoginMethod,
}

/// What the user presents at the gate. One variant: the methods are not combined.
#[derive(Clone, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum LoginProof {
    Password { secret: String },
    Pin { secret: String },
    WebAuthn,
}

impl fmt::Debug for LoginProof {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Password { .. } => "Password",
            Self::Pin { .. } => "Pin",
            Self::WebAuthn => "WebAuthn",
        })
    }
}

/// A credential created by an authenticator. Only the id and the public key are stored.
#[derive(Clone)]
pub struct WebAuthnCredential {
    pub id: Vec<u8>,
    /// Compressed SEC1 P-256 point.
    pub public_key: Vec<u8>,
}

/// An assertion to check against the stored credential. The signature covers
/// `authenticatorData || SHA-256(clientDataJSON)`.
#[derive(Clone)]
pub struct WebAuthnAssertion {
    pub id: Vec<u8>,
    pub authenticator_data: Vec<u8>,
    pub client_data_json: Vec<u8>,
    pub signature: Vec<u8>,
}

/// Creates and asserts a credential. The system bridge talks to a YubiKey; tests use
/// [`SoftwareAuthenticator`], which signs with a P-256 key and never touches a folder.
pub trait Authenticator {
    fn register(&mut self, challenge: &[u8]) -> Result<WebAuthnCredential, SourceError>;
    fn authenticate(
        &mut self,
        challenge: &[u8],
        credential_id: &[u8],
    ) -> Result<WebAuthnAssertion, SourceError>;
}

/// A P-256 authenticator with no USB device. Login tests use it in place of a YubiKey 5C.
#[derive(Clone)]
pub struct SoftwareAuthenticator {
    id: Vec<u8>,
    signing: SigningKey,
    counter: u32,
}

impl SoftwareAuthenticator {
    pub fn generate() -> Self {
        let mut id = [0u8; 16];
        OsRng.fill_bytes(&mut id);
        Self {
            id: id.to_vec(),
            signing: SigningKey::random(&mut OsRng),
            counter: 0,
        }
    }

    pub fn credential_id(&self) -> &[u8] {
        &self.id
    }
}

impl Authenticator for SoftwareAuthenticator {
    fn register(&mut self, _challenge: &[u8]) -> Result<WebAuthnCredential, SourceError> {
        let public = self
            .signing
            .verifying_key()
            .to_encoded_point(true)
            .as_bytes()
            .to_vec();
        Ok(WebAuthnCredential {
            id: self.id.clone(),
            public_key: public,
        })
    }

    fn authenticate(
        &mut self,
        challenge: &[u8],
        credential_id: &[u8],
    ) -> Result<WebAuthnAssertion, SourceError> {
        if credential_id != self.id {
            return Err(SourceError::WrongPassword);
        }
        self.counter = self.counter.saturating_add(1);
        let client_data_json = client_data(challenge);
        let authenticator_data = authenticator_data(self.counter);
        let message = signed_message(&authenticator_data, &client_data_json);
        let signature: Signature = self.signing.sign(&message);
        Ok(WebAuthnAssertion {
            id: self.id.clone(),
            authenticator_data,
            client_data_json,
            signature: signature.to_bytes().to_vec(),
        })
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(
    tag = "method",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum Stored {
    #[default]
    None,
    Password {
        hash: String,
    },
    Pin {
        hash: String,
    },
    WebAuthn {
        credential_id: String,
        public_key: String,
        sign_count: u32,
    },
}

impl Stored {
    fn method(&self) -> LoginMethod {
        match self {
            Self::None => LoginMethod::None,
            Self::Password { .. } => LoginMethod::Password,
            Self::Pin { .. } => LoginMethod::Pin,
            Self::WebAuthn { .. } => LoginMethod::WebAuthn,
        }
    }
}

impl AppStore {
    fn login_file(&self) -> JsonFile {
        JsonFile::new(self.dir(), LOGIN_FILE)
    }

    pub fn login_status(&self) -> LoginStatus {
        LoginStatus {
            method: self.login_file().load::<Stored>().method(),
        }
    }

    /// No login: the app opens straight to Welcome.
    pub fn clear_login(&self) -> Result<LoginStatus, SourceError> {
        self.login_file().update(|stored: &mut Stored| {
            *stored = Stored::None;
        })?;
        Ok(self.login_status())
    }

    /// Replaces whatever login was set. The hash is Argon2id, not an age key.
    pub fn set_login_password(&self, password: &SecretString) -> Result<LoginStatus, SourceError> {
        if password.expose_secret().is_empty() {
            return Err(SourceError::Invalid("Enter a password.".into()));
        }
        let hash = hash_secret(password)?;
        self.login_file().update(|stored: &mut Stored| {
            *stored = Stored::Password { hash };
        })?;
        Ok(self.login_status())
    }

    /// A numeric PIN, 4 to 8 digits. Replaces whatever login was set.
    pub fn set_login_pin(&self, pin: &SecretString) -> Result<LoginStatus, SourceError> {
        check_login_pin(pin.expose_secret())?;
        let hash = hash_secret(pin)?;
        self.login_file().update(|stored: &mut Stored| {
            *stored = Stored::Pin { hash };
        })?;
        Ok(self.login_status())
    }

    /// Stores the credential id and public key, and nothing else. Replaces the previous login.
    pub fn set_login_webauthn(
        &self,
        credential: &WebAuthnCredential,
    ) -> Result<LoginStatus, SourceError> {
        if credential.id.is_empty()
            || VerifyingKey::from_sec1_bytes(&credential.public_key).is_err()
        {
            return Err(SourceError::Invalid(
                "The WebAuthn credential has no P-256 public key.".into(),
            ));
        }
        let credential_id = STANDARD.encode(&credential.id);
        let public_key = STANDARD.encode(&credential.public_key);
        self.login_file().update(|stored: &mut Stored| {
            *stored = Stored::WebAuthn {
                credential_id,
                public_key,
                sign_count: 0,
            };
        })?;
        Ok(self.login_status())
    }

    /// Checks the one configured method. A proof of another method does not pass.
    pub fn unlock_app(&self, proof: &LoginProof) -> Result<(), SourceError> {
        match proof {
            LoginProof::Password { secret } => {
                self.check_secret(LoginMethod::Password, &SecretString::from(secret.clone()))
            }
            LoginProof::Pin { secret } => {
                self.check_secret(LoginMethod::Pin, &SecretString::from(secret.clone()))
            }
            LoginProof::WebAuthn => Err(SourceError::Invalid(
                "WebAuthn login is a touch, checked with the authenticator.".into(),
            )),
        }
    }

    /// Enrolls WebAuthn through `authenticator` and stores only the public credential.
    pub fn enroll_webauthn(
        &self,
        authenticator: &mut impl Authenticator,
    ) -> Result<LoginStatus, SourceError> {
        let challenge = challenge();
        let credential = authenticator.register(&challenge)?;
        self.set_login_webauthn(&credential)
    }

    /// Asserts the stored credential. A success is `Ok(())`: no folder key comes back.
    pub fn login_webauthn(
        &self,
        authenticator: &mut impl Authenticator,
    ) -> Result<(), SourceError> {
        let stored = self.login_file().load::<Stored>();
        let Stored::WebAuthn { credential_id, .. } = &stored else {
            return Err(SourceError::Invalid("App login is not a YubiKey.".into()));
        };
        let id = STANDARD
            .decode(credential_id)
            .map_err(|_| SourceError::Storage("unreadable login".into()))?;
        let challenge = challenge();
        let assertion = authenticator.authenticate(&challenge, &id)?;
        self.check_assertion(&challenge, &assertion)
    }

    fn check_secret(
        &self,
        expected: LoginMethod,
        secret: &SecretString,
    ) -> Result<(), SourceError> {
        let stored = self.login_file().load::<Stored>();
        if stored.method() != expected {
            return Err(SourceError::Invalid(method_message(stored.method())));
        }
        let hash = match &stored {
            Stored::Password { hash } | Stored::Pin { hash } => hash,
            _ => return Err(SourceError::Invalid(method_message(stored.method()))),
        };
        Argon2::default()
            .verify_password(secret.expose_secret().as_bytes(), hash.as_str())
            .map_err(|_| SourceError::WrongPassword)
    }

    fn check_assertion(
        &self,
        challenge: &[u8],
        assertion: &WebAuthnAssertion,
    ) -> Result<(), SourceError> {
        let mut matched = false;
        let result = self.login_file().update(|stored: &mut Stored| {
            let Stored::WebAuthn {
                credential_id,
                public_key,
                sign_count,
            } = stored
            else {
                return Err(SourceError::Invalid("App login is not a YubiKey.".into()));
            };
            let id = STANDARD
                .decode(credential_id.as_str())
                .map_err(|_| SourceError::Storage("unreadable login".into()))?;
            if assertion.id != id {
                return Err(SourceError::WrongPassword);
            }
            let key_bytes = STANDARD
                .decode(public_key.as_str())
                .map_err(|_| SourceError::Storage("unreadable login".into()))?;
            verify_assertion(&key_bytes, challenge, assertion, *sign_count)?;
            let next = sign_count_of(&assertion.authenticator_data).unwrap_or(0);
            if next > 0 {
                *sign_count = next;
            }
            matched = true;
            Ok(())
        })?;
        if !matched {
            return result;
        }
        result
    }
}

fn method_message(method: LoginMethod) -> String {
    match method {
        LoginMethod::None => "Login is not turned on.".into(),
        LoginMethod::Password => "App login is a password.".into(),
        LoginMethod::Pin => "App login is a PIN.".into(),
        LoginMethod::WebAuthn => "App login is a YubiKey touch.".into(),
    }
}

fn check_login_pin(pin: &str) -> Result<(), SourceError> {
    if (4..=8).contains(&pin.len()) && pin.bytes().all(|byte| byte.is_ascii_digit()) {
        Ok(())
    } else {
        Err(SourceError::Invalid(
            "The login PIN is 4 to 8 digits.".into(),
        ))
    }
}

fn hasher() -> Argon2<'static> {
    let params = if cfg!(test) {
        Params::new(64, 1, 1, None).expect("test Argon2id parameters")
    } else {
        Params::new(64 * 1024, 3, 1, None).expect("Argon2id parameters")
    };
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

fn hash_secret(secret: &SecretString) -> Result<String, SourceError> {
    hasher()
        .hash_password(secret.expose_secret().as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|_| SourceError::Storage("could not store the login".into()))
}

fn challenge() -> Vec<u8> {
    let mut bytes = vec![0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes
}

fn client_data(challenge: &[u8]) -> Vec<u8> {
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(challenge);
    format!(
        r#"{{"type":"webauthn.get","challenge":"{challenge}","origin":"{ORIGIN}","crossOrigin":false}}"#
    )
    .into_bytes()
}

fn authenticator_data(counter: u32) -> Vec<u8> {
    let mut data = Vec::with_capacity(37);
    data.extend_from_slice(&Sha256::digest(RP_ID.as_bytes()));
    data.push(0x01); // user present: the touch. User verification is not required.
    data.extend_from_slice(&counter.to_be_bytes());
    data
}

fn signed_message(authenticator_data: &[u8], client_data_json: &[u8]) -> Vec<u8> {
    let mut message = Vec::with_capacity(authenticator_data.len() + 32);
    message.extend_from_slice(authenticator_data);
    message.extend_from_slice(&Sha256::digest(client_data_json));
    message
}

fn sign_count_of(authenticator_data: &[u8]) -> Option<u32> {
    let bytes: [u8; 4] = authenticator_data.get(33..37)?.try_into().ok()?;
    Some(u32::from_be_bytes(bytes))
}

fn verify_assertion(
    public_key: &[u8],
    challenge: &[u8],
    assertion: &WebAuthnAssertion,
    stored_count: u32,
) -> Result<(), SourceError> {
    if assertion.authenticator_data.len() < 37 {
        return Err(SourceError::WrongPassword);
    }
    let flags = assertion.authenticator_data[32];
    if flags & 0x01 == 0 {
        return Err(SourceError::WrongPassword);
    }
    let rp = Sha256::digest(RP_ID.as_bytes());
    if assertion.authenticator_data[..32] != rp[..] {
        return Err(SourceError::WrongPassword);
    }
    let client: serde_json::Value = serde_json::from_slice(&assertion.client_data_json)
        .map_err(|_| SourceError::WrongPassword)?;
    let expected = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(challenge);
    if client.get("type").and_then(|value| value.as_str()) != Some("webauthn.get")
        || client.get("challenge").and_then(|value| value.as_str()) != Some(expected.as_str())
        || client.get("origin").and_then(|value| value.as_str()) != Some(ORIGIN)
    {
        return Err(SourceError::WrongPassword);
    }
    let count = sign_count_of(&assertion.authenticator_data).unwrap_or(0);
    if count != 0 && count <= stored_count {
        return Err(SourceError::WrongPassword);
    }
    let key = VerifyingKey::from_sec1_bytes(public_key).map_err(|_| SourceError::WrongPassword)?;
    let signature =
        Signature::from_slice(&assertion.signature).map_err(|_| SourceError::WrongPassword)?;
    let message = signed_message(&assertion.authenticator_data, &assertion.client_data_json);
    key.verify(&message, &signature)
        .map_err(|_| SourceError::WrongPassword)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, AppStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = AppStore::new(dir.path());
        (dir, store)
    }

    #[test]
    fn no_login_opens_straight_through() {
        let (_dir, store) = store();
        assert_eq!(store.login_status().method, LoginMethod::None);
        assert!(matches!(
            store.unlock_app(&LoginProof::Password {
                secret: "secret".into()
            }),
            Err(SourceError::Invalid(_))
        ));
    }

    #[test]
    fn password_pin_and_webauthn_replace_each_other() {
        let (_dir, store) = store();
        store
            .set_login_password(&SecretString::from("correct horse"))
            .unwrap();
        assert_eq!(store.login_status().method, LoginMethod::Password);
        assert!(
            store
                .unlock_app(&LoginProof::Password {
                    secret: "correct horse".into()
                })
                .is_ok()
        );
        assert!(matches!(
            store.unlock_app(&LoginProof::Pin {
                secret: "1234".into()
            }),
            Err(SourceError::Invalid(_))
        ));
        assert!(matches!(
            store.unlock_app(&LoginProof::Password {
                secret: "nope".into()
            }),
            Err(SourceError::WrongPassword)
        ));

        store.set_login_pin(&SecretString::from("1234")).unwrap();
        assert_eq!(store.login_status().method, LoginMethod::Pin);
        assert!(matches!(
            store.unlock_app(&LoginProof::Password {
                secret: "1234".into()
            }),
            Err(SourceError::Invalid(_))
        ));
        assert!(
            store
                .unlock_app(&LoginProof::Pin {
                    secret: "1234".into()
                })
                .is_ok()
        );
        assert!(store.set_login_pin(&SecretString::from("12")).is_err());
        assert!(store.set_login_pin(&SecretString::from("12ab")).is_err());

        let mut key = SoftwareAuthenticator::generate();
        store.enroll_webauthn(&mut key).unwrap();
        assert_eq!(store.login_status().method, LoginMethod::WebAuthn);
        assert!(matches!(
            store.unlock_app(&LoginProof::Pin {
                secret: "1234".into()
            }),
            Err(SourceError::Invalid(_))
        ));
        store.login_webauthn(&mut key).unwrap();
        let issued = challenge();
        let id = key.credential_id().to_vec();
        let replay = key.authenticate(&issued, &id).unwrap();
        store.check_assertion(&issued, &replay).unwrap();
        assert!(matches!(
            store.check_assertion(&issued, &replay),
            Err(SourceError::WrongPassword)
        ));
        let saved = std::fs::read_to_string(_dir.path().join(LOGIN_FILE)).unwrap();
        assert!(saved.contains("credentialId"));
        assert!(saved.contains("publicKey"));
        assert!(!saved.contains("AGE-SECRET"));
        assert!(!saved.contains("wrappedIdentity"));
        assert!(!saved.contains("argon2"));

        store.clear_login().unwrap();
        assert_eq!(store.login_status().method, LoginMethod::None);
        assert!(store.login_webauthn(&mut key).is_err());
    }

    #[test]
    fn a_webauthn_assertion_for_another_challenge_is_refused() {
        let (_dir, store) = store();
        let mut key = SoftwareAuthenticator::generate();
        store.enroll_webauthn(&mut key).unwrap();
        let issued = challenge();
        let id = key.credential_id().to_vec();
        let assertion = key.authenticate(&issued, &id).unwrap();
        let wrong = challenge();
        assert!(matches!(
            store.check_assertion(&wrong, &assertion),
            Err(SourceError::WrongPassword)
        ));
    }

    #[test]
    fn login_success_is_not_key_material() {
        let (_dir, store) = store();
        let mut key = SoftwareAuthenticator::generate();
        store.enroll_webauthn(&mut key).unwrap();
        let unlocked: Result<(), SourceError> = store.login_webauthn(&mut key);
        assert_eq!(unlocked, Ok(()));
    }
}
