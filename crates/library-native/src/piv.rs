//! A YubiKey as a third wrap of one locked folder's age identity (docs/age-locked-folders.md §9,
//! spec §13).
//!
//! The on-disk ciphertext is an age file whose recipient stanza is `piv-p256`, the stanza
//! `age-plugin-yubikey` 0.5 writes for an ECDSA P-256 key in a retired PIV slot (0x82–0x95).
//! Encrypting needs only the public key. Decrypting needs the card to do P-256 ECDH, or, in
//! tests, a software key that does the same ECDH. Touch and PIN are alternatives: a wrap is
//! stored as one or the other, and a prompt of the other kind is refused before any ECDH.

use std::collections::HashSet;

use age::secrecy::ExposeSecret;
use age_core::format::{FILE_KEY_BYTES, FileKey, Stanza};
use age_core::primitives::{aead_decrypt, aead_encrypt, hkdf};
use base64::Engine;
use base64::engine::general_purpose::STANDARD_NO_PAD;
#[cfg(test)]
use bech32::{Bech32, Hrp};
use library_core::{PivPolicy, PivPrompt, SourceError};
#[cfg(test)]
use p256::SecretKey;
use p256::ecdh::EphemeralSecret;
#[cfg(test)]
use p256::ecdh::diffie_hellman;
use p256::elliptic_curve::sec1::ToEncodedPoint;
use p256::{EncodedPoint, PublicKey};
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::vault::{FolderKey, VaultError};

/// First retired PIV slot (R1). `age-plugin-yubikey` uses only 0x82..=0x95.
pub const SLOT_FIRST: u8 = 0x82;
/// Last retired PIV slot (R20).
pub const SLOT_LAST: u8 = 0x95;

const STANZA_TAG: &str = "piv-p256";
const STANZA_LABEL: &[u8] = b"piv-p256";
const RECIPIENT_HRP: &str = "age1yubikey";
const TAG_BYTES: usize = 4;
const COMPRESSED_LEN: usize = 33;

/// The public half of one card, stored on the folder. The private key is not here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PivCard {
    pub serial: String,
    pub slot: u8,
    pub policy: PivPolicy,
    pub recipient: String,
}

impl PivCard {
    /// Checks the serial, the retired slot and the age-plugin-yubikey recipient.
    pub fn new(
        serial: impl Into<String>,
        slot: u8,
        policy: PivPolicy,
        recipient: impl Into<String>,
    ) -> Result<Self, SourceError> {
        let serial = serial.into();
        if serial.is_empty()
            || serial.len() > 10
            || !serial.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(SourceError::Invalid(
                "A YubiKey serial is up to 10 digits.".into(),
            ));
        }
        if !(SLOT_FIRST..=SLOT_LAST).contains(&slot) {
            return Err(SourceError::Invalid(
                "A YubiKey wrap uses a retired PIV slot, 82 to 95.".into(),
            ));
        }
        let recipient = recipient.into();
        public_key(&recipient).map_err(|_| {
            SourceError::Invalid("This is not a YubiKey recipient for an ECDSA P-256 key.".into())
        })?;
        Ok(Self {
            serial,
            slot,
            policy,
            recipient,
        })
    }
}

/// Touch takes no PIN. PIN takes the YubiKey PIN and no touch. Anything else is refused.
pub fn check_enrollment(policy: PivPolicy, pin: Option<&str>) -> Result<(), SourceError> {
    match (policy, pin.map(str::trim).filter(|pin| !pin.is_empty())) {
        (PivPolicy::Touch, None) => Ok(()),
        (PivPolicy::Pin, Some(pin)) => check_piv_pin(pin),
        (PivPolicy::Touch, Some(_)) => Err(SourceError::Invalid(
            "Touch does not ask for a PIN. Choose one.".into(),
        )),
        (PivPolicy::Pin, None) => Err(SourceError::Invalid(
            "Enter the YubiKey PIN. It replaces the touch.".into(),
        )),
    }
}

/// A YubiKey PIV PIN is 6 to 8 digits. It is not the app login PIN.
pub fn check_piv_pin(pin: &str) -> Result<(), SourceError> {
    if (6..=8).contains(&pin.len()) && pin.bytes().all(|byte| byte.is_ascii_digit()) {
        Ok(())
    } else {
        Err(SourceError::Invalid(
            "The YubiKey PIN is 6 to 8 digits.".into(),
        ))
    }
}

/// The prompt has to be the policy stored for this card. The other one is not a second step.
pub fn prompt_matches(policy: PivPolicy, prompt: &PivPrompt) -> bool {
    matches!(
        (policy, prompt),
        (PivPolicy::Touch, PivPrompt::Touch) | (PivPolicy::Pin, PivPrompt::Pin { .. })
    )
}

/// A P-256 key that stands in for a YubiKey in tests. ECDH matches the card; nothing here
/// talks to USB.
#[cfg(test)]
pub(crate) struct SoftwarePiv {
    secret: SecretKey,
    card: PivCard,
    pin: Option<String>,
}

#[cfg(test)]
impl SoftwarePiv {
    pub(crate) fn generate(serial: &str, slot: u8, policy: PivPolicy, pin: Option<&str>) -> Self {
        check_enrollment(policy, pin).expect("the test builds a touch or a PIN, not both");
        let secret = SecretKey::random(&mut OsRng);
        let recipient = recipient_of(&secret.public_key());
        let card = PivCard::new(serial, slot, policy, recipient).expect("generated card is valid");
        Self {
            secret,
            card,
            pin: pin.map(str::to_owned),
        }
    }

    pub(crate) fn card(&self) -> &PivCard {
        &self.card
    }

    /// ECDH for one `piv-p256` stanza. A PIN policy checks the PIN and does not require a
    /// touch; a touch policy has no PIN to check.
    pub(crate) fn agree(
        &self,
        prompt: &PivPrompt,
        uncompressed: &[u8],
    ) -> Result<[u8; 32], VaultError> {
        match (&self.card.policy, prompt, &self.pin) {
            (PivPolicy::Touch, PivPrompt::Touch, _) => {}
            (PivPolicy::Pin, PivPrompt::Pin { pin }, Some(expected)) if pin == expected => {}
            _ => return Err(VaultError::WrongKey),
        }
        let public = PublicKey::from_sec1_bytes(uncompressed)
            .map_err(|_| VaultError::Corrupt("ephemeral key"))?;
        let shared = diffie_hellman(self.secret.to_nonzero_scalar(), public.as_affine());
        let raw = shared.raw_secret_bytes();
        let mut out = [0u8; 32];
        out.copy_from_slice(raw);
        Ok(out)
    }
}

/// Encrypts the folder identity to a YubiKey recipient. The card is not needed.
pub(crate) fn seal_identity(identity: &FolderKey, recipient: &str) -> Result<Vec<u8>, VaultError> {
    let public = public_key(recipient)?;
    let recipient = PivRecipient { public };
    let text = identity.identity_text();
    age::encrypt(&recipient, text.as_bytes()).map_err(|error| VaultError::Age(error.to_string()))
}

/// Decrypts a folder identity. `agree` does the P-256 ECDH the card would do.
pub(crate) fn open_identity(
    blob: &[u8],
    recipient: &str,
    agree: impl Fn(&[u8]) -> Result<[u8; 32], VaultError>,
) -> Result<FolderKey, VaultError> {
    let public = public_key(recipient)?;
    let identity = PivIdentity { public, agree };
    let plain = Zeroizing::new(age::decrypt(&identity, blob).map_err(|_| VaultError::WrongKey)?);
    let text = std::str::from_utf8(&plain).map_err(|_| VaultError::Corrupt("identity"))?;
    FolderKey::from_identity_text(text)
}

fn public_key(recipient: &str) -> Result<PublicKey, VaultError> {
    let (hrp, data) = bech32::decode(recipient).map_err(|_| VaultError::Corrupt("recipient"))?;
    if hrp.as_str() != RECIPIENT_HRP || data.len() != COMPRESSED_LEN {
        return Err(VaultError::Corrupt("recipient"));
    }
    if data.len() != COMPRESSED_LEN || (data[0] != 2 && data[0] != 3) {
        return Err(VaultError::Corrupt("recipient"));
    }
    PublicKey::from_sec1_bytes(&data).map_err(|_| VaultError::Corrupt("recipient"))
}

#[cfg(test)]
fn recipient_of(public: &PublicKey) -> String {
    let compressed = public.to_encoded_point(true);
    let hrp = Hrp::parse(RECIPIENT_HRP).expect("age1yubikey is a valid bech32 HRP");
    bech32::encode::<Bech32>(hrp, compressed.as_bytes()).expect("33 bytes encode")
}

fn tag_of(compressed: &[u8]) -> [u8; 4] {
    let digest = Sha256::digest(compressed);
    digest[..TAG_BYTES]
        .try_into()
        .expect("sha256 is longer than 4 bytes")
}

fn wrapping_key(epk: &[u8], recipient: &[u8], shared: &[u8]) -> [u8; 32] {
    let mut salt = Vec::with_capacity(epk.len() + recipient.len());
    salt.extend_from_slice(epk);
    salt.extend_from_slice(recipient);
    hkdf(&salt, STANZA_LABEL, shared)
}

struct PivRecipient {
    public: PublicKey,
}

impl age::Recipient for PivRecipient {
    fn wrap_file_key(
        &self,
        file_key: &FileKey,
    ) -> Result<(Vec<Stanza>, HashSet<String>), age::EncryptError> {
        let esk = EphemeralSecret::random(&mut OsRng);
        let epk = esk.public_key();
        let epk_compressed = epk.to_encoded_point(true);
        let recipient = self.public.to_encoded_point(true);
        let shared = esk.diffie_hellman(&self.public);
        let key = wrapping_key(
            epk_compressed.as_bytes(),
            recipient.as_bytes(),
            shared.raw_secret_bytes(),
        );
        let encrypted = aead_encrypt(&key, file_key.expose_secret());
        let stanza = Stanza {
            tag: STANZA_TAG.to_owned(),
            args: vec![
                STANDARD_NO_PAD.encode(tag_of(recipient.as_bytes())),
                STANDARD_NO_PAD.encode(epk_compressed.as_bytes()),
            ],
            body: encrypted,
        };
        Ok((vec![stanza], HashSet::new()))
    }
}

struct PivIdentity<F> {
    public: PublicKey,
    agree: F,
}

impl<F> age::Identity for PivIdentity<F>
where
    F: Fn(&[u8]) -> Result<[u8; 32], VaultError>,
{
    fn unwrap_stanza(&self, stanza: &Stanza) -> Option<Result<FileKey, age::DecryptError>> {
        let parsed = match parse_stanza(stanza) {
            None => return None,
            Some(Err(())) => return Some(Err(age::DecryptError::KeyDecryptionFailed)),
            Some(Ok(parsed)) => parsed,
        };
        let recipient = self.public.to_encoded_point(true);
        if parsed.tag != tag_of(recipient.as_bytes()) {
            return None;
        }
        let shared = match (self.agree)(&parsed.epk_uncompressed) {
            Ok(shared) => shared,
            Err(_) => return Some(Err(age::DecryptError::KeyDecryptionFailed)),
        };
        let key = wrapping_key(
            parsed.epk_compressed.as_bytes(),
            recipient.as_bytes(),
            &shared,
        );
        let plain = aead_decrypt(&key, FILE_KEY_BYTES, &parsed.encrypted).ok()?;
        let bytes: [u8; FILE_KEY_BYTES] = plain.try_into().ok()?;
        Some(Ok(FileKey::new(Box::new(bytes))))
    }
}

struct ParsedStanza {
    tag: [u8; TAG_BYTES],
    epk_compressed: EncodedPoint,
    epk_uncompressed: Vec<u8>,
    encrypted: Vec<u8>,
}

fn parse_stanza(stanza: &Stanza) -> Option<Result<ParsedStanza, ()>> {
    if stanza.tag != STANZA_TAG {
        return None;
    }
    let Some([tag, epk]) = stanza.args.get(..2).map(|args| {
        let mut iter = args.iter();
        [iter.next().unwrap(), iter.next().unwrap()]
    }) else {
        return Some(Err(()));
    };
    if stanza.args.len() != 2 {
        return Some(Err(()));
    }
    let tag = decode_fixed::<TAG_BYTES>(tag)?;
    let epk_bytes = decode_fixed::<COMPRESSED_LEN>(epk)?;
    let (tag, epk_bytes) = match (tag, epk_bytes) {
        (Some(tag), Some(epk_bytes)) => (tag, epk_bytes),
        _ => return Some(Err(())),
    };
    let point = match EncodedPoint::from_bytes(epk_bytes) {
        Ok(point) if point.is_compressed() => point,
        _ => return Some(Err(())),
    };
    let public = match PublicKey::from_sec1_bytes(point.as_bytes()) {
        Ok(public) => public,
        Err(_) => return Some(Err(())),
    };
    if stanza.body.len() != FILE_KEY_BYTES + 16 {
        return Some(Err(()));
    }
    Some(Ok(ParsedStanza {
        tag,
        epk_compressed: point,
        epk_uncompressed: public.to_encoded_point(false).as_bytes().to_vec(),
        encrypted: stanza.body.clone(),
    }))
}

fn decode_fixed<const N: usize>(text: &str) -> Option<Option<[u8; N]>> {
    let bytes = STANDARD_NO_PAD.decode(text).ok()?;
    Some(bytes.try_into().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch_key() -> SoftwarePiv {
        SoftwarePiv::generate("12345678", 0x82, PivPolicy::Touch, None)
    }

    #[test]
    fn a_software_key_round_trips_the_piv_p256_stanza() {
        let card = touch_key();
        let identity = FolderKey::generate();
        let sealed = seal_identity(&identity, &card.card().recipient).unwrap();
        assert!(sealed.starts_with(b"age-encryption.org/v1\n"));
        let opened = open_identity(&sealed, &card.card().recipient, |epk| {
            card.agree(&PivPrompt::Touch, epk)
        })
        .unwrap();
        assert_eq!(
            opened.identity_text().as_str(),
            identity.identity_text().as_str()
        );
    }

    #[test]
    fn another_key_does_not_open_the_wrap() {
        let card = touch_key();
        let other = SoftwarePiv::generate("99", 0x83, PivPolicy::Touch, None);
        let sealed = seal_identity(&FolderKey::generate(), &card.card().recipient).unwrap();
        let opened = open_identity(&sealed, &card.card().recipient, |epk| {
            other.agree(&PivPrompt::Touch, epk)
        });
        assert_eq!(opened.err(), Some(VaultError::WrongKey));
    }

    #[test]
    fn touch_and_pin_are_alternatives() {
        let touch = touch_key();
        let pin = SoftwarePiv::generate("55", 0x95, PivPolicy::Pin, Some("123456"));
        let sealed = seal_identity(&FolderKey::generate(), &pin.card().recipient).unwrap();
        assert!(prompt_matches(PivPolicy::Touch, &PivPrompt::Touch));
        assert!(!prompt_matches(
            PivPolicy::Touch,
            &PivPrompt::Pin {
                pin: "123456".into()
            }
        ));
        assert!(prompt_matches(
            PivPolicy::Pin,
            &PivPrompt::Pin { pin: "1".into() }
        ));
        assert!(!prompt_matches(PivPolicy::Pin, &PivPrompt::Touch));
        assert_eq!(
            touch
                .agree(
                    &PivPrompt::Pin {
                        pin: "123456".into()
                    },
                    &[]
                )
                .err(),
            Some(VaultError::WrongKey)
        );
        assert_eq!(
            pin.agree(&PivPrompt::Touch, &[]).err(),
            Some(VaultError::WrongKey)
        );
        assert_eq!(
            pin.agree(
                &PivPrompt::Pin {
                    pin: "000000".into()
                },
                &[]
            )
            .err(),
            Some(VaultError::WrongKey)
        );
        let opened = open_identity(&sealed, &pin.card().recipient, |epk| {
            pin.agree(
                &PivPrompt::Pin {
                    pin: "123456".into(),
                },
                epk,
            )
        });
        assert!(opened.is_ok());
    }

    #[test]
    fn only_retired_slots_and_a_yubikey_recipient_are_accepted() {
        let card = touch_key();
        assert!(card.card().recipient.starts_with("age1yubikey1"));
        assert!(PivCard::new("1", 0x9d, PivPolicy::Touch, card.card().recipient.clone()).is_err());
        assert!(PivCard::new("1", 0x81, PivPolicy::Touch, card.card().recipient.clone()).is_err());
        assert!(PivCard::new("1", 0x96, PivPolicy::Touch, card.card().recipient.clone()).is_err());
        assert!(PivCard::new("1", 0x82, PivPolicy::Touch, card.card().recipient.clone()).is_ok());
        assert!(PivCard::new("1", 0x95, PivPolicy::Touch, card.card().recipient.clone()).is_ok());
        assert!(PivCard::new("", 0x82, PivPolicy::Touch, card.card().recipient.clone()).is_err());
        assert!(PivCard::new("12a", 0x82, PivPolicy::Touch, "age1yubikey1qqqq").is_err());
        assert!(check_enrollment(PivPolicy::Touch, None).is_ok());
        assert!(check_enrollment(PivPolicy::Touch, Some("123456")).is_err());
        assert!(check_enrollment(PivPolicy::Pin, None).is_err());
        assert!(check_enrollment(PivPolicy::Pin, Some("12345")).is_err());
        assert!(check_enrollment(PivPolicy::Pin, Some("123456")).is_ok());
        assert!(check_enrollment(PivPolicy::Pin, Some("12345678")).is_ok());
        assert!(check_enrollment(PivPolicy::Pin, Some("123456789")).is_err());
    }

    #[test]
    fn the_stanza_tag_is_the_sha256_prefix_of_the_compressed_key() {
        let card = touch_key();
        let public = public_key(&card.card().recipient).unwrap();
        let compressed = public.to_encoded_point(true);
        let digest = Sha256::digest(compressed.as_bytes());
        assert_eq!(&tag_of(compressed.as_bytes())[..], &digest[..4]);
    }
}
