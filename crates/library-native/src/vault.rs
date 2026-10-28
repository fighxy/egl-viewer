//! The cryptography of locked folders (docs/age-locked-folders.md, D-018): the folder identity
//! and its two wraps (Argon2id of the password, the recovery secret), item keys, and age
//! encryption of files and of the sensitive item fields. Nothing here touches the database.

use std::fmt;
use std::io::{self, Read, Write};
use std::iter;
use std::str::FromStr;

use age::secrecy::{ExposeSecret, SecretString};
use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

/// The first bytes of every age file: how an encrypted original or preview is recognised.
#[cfg(test)]
pub(crate) const AGE_MAGIC: &[u8] = b"age-encryption.org/v1\n";

/// Bytes of the Argon2id salt (docs/age-locked-folders.md §1).
pub(crate) const SALT_LEN: usize = 16;

/// The version byte of a wrapped identity: XChaCha20-Poly1305, a 24-byte nonce.
const WRAP_VERSION: u8 = 1;
const NONCE_LEN: usize = 24;

/// Why a key did not open a wrap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum VaultError {
    /// The password or the recovery secret is not the one that made the wrap.
    WrongKey,
    /// Stored lock data that this app cannot read.
    Corrupt(&'static str),
    /// Text that is not a recovery key or phrase.
    BadRecoveryText,
    /// age failed on a file or a field.
    Age(String),
}

impl fmt::Display for VaultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongKey => f.write_str("the key does not match"),
            Self::Corrupt(what) => write!(f, "unreadable lock data: {what}"),
            Self::BadRecoveryText => f.write_str("not a recovery key or phrase"),
            Self::Age(message) => write!(f, "age: {message}"),
        }
    }
}

impl std::error::Error for VaultError {}

pub(crate) type Result<T> = std::result::Result<T, VaultError>;

/// Argon2id parameters, stored as JSON on the folder row so each folder keeps its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KdfParams {
    pub algorithm: KdfAlgorithm,
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum KdfAlgorithm {
    Argon2id,
}

impl KdfParams {
    /// 64 MiB, three passes, one lane (docs/age-locked-folders.md §1).
    pub(crate) const DEFAULT: Self = Self {
        algorithm: KdfAlgorithm::Argon2id,
        memory_kib: 64 * 1024,
        iterations: 3,
        parallelism: 1,
    };

    /// Cheap parameters for tests; never written by the app.
    #[cfg(test)]
    pub(crate) const FAST: Self = Self {
        algorithm: KdfAlgorithm::Argon2id,
        memory_kib: 64,
        iterations: 1,
        parallelism: 1,
    };

    pub(crate) fn to_json(self) -> String {
        serde_json::to_string(&self).unwrap_or_default()
    }

    /// Reads stored parameters; refuses values no folder of this app was made with, so a
    /// tampered row cannot make an unlock run for hours.
    pub(crate) fn from_json(text: &str) -> Result<Self> {
        let params: Self =
            serde_json::from_str(text).map_err(|_| VaultError::Corrupt("kdf parameters"))?;
        let sane = (8..=4 * 1024 * 1024).contains(&params.memory_kib)
            && (1..=16).contains(&params.iterations)
            && (1..=16).contains(&params.parallelism);
        if sane {
            Ok(params)
        } else {
            Err(VaultError::Corrupt("kdf parameters"))
        }
    }
}

/// 32 random bytes from the operating system.
fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).expect("the operating system has no random source");
    bytes
}

pub(crate) fn new_salt() -> [u8; SALT_LEN] {
    random()
}

/// A 256-bit key that wraps a folder identity; wiped when dropped.
pub(crate) struct WrapKey(Zeroizing<[u8; 32]>);

impl WrapKey {
    /// Argon2id of the password: one pass of the KDF per unlock, not per file.
    pub(crate) fn from_password(
        password: &SecretString,
        salt: &[u8],
        params: KdfParams,
    ) -> Result<Self> {
        let argon_params = argon2::Params::new(
            params.memory_kib,
            params.iterations,
            params.parallelism,
            Some(32),
        )
        .map_err(|_| VaultError::Corrupt("kdf parameters"))?;
        let argon = argon2::Argon2::new(
            argon2::Algorithm::Argon2id,
            argon2::Version::V0x13,
            argon_params,
        );
        let mut key = Zeroizing::new([0u8; 32]);
        argon
            .hash_password_into(password.expose_secret().as_bytes(), salt, key.as_mut())
            .map_err(|_| VaultError::Corrupt("kdf input"))?;
        Ok(Self(key))
    }

    /// The recovery secret is 128 random bits: no stretching needed, a domain-separated hash
    /// makes it a wrapping key.
    pub(crate) fn from_recovery(secret: &RecoverySecret) -> Self {
        Self(Zeroizing::new(blake3::derive_key(
            "egl-viewer 2026-10-28 locked folder recovery wrap",
            secret.0.as_ref(),
        )))
    }

    fn cipher(&self) -> XChaCha20Poly1305 {
        let key: &[u8; 32] = &self.0;
        XChaCha20Poly1305::new(key.into())
    }
}

/// What a wrap protects, bound into it with the folder id: a wrap cannot be moved to another
/// folder or used in the other role.
#[derive(Debug, Clone, Copy)]
pub(crate) enum WrapRole {
    Password,
    Recovery,
}

fn wrap_aad(role: WrapRole, folder_id: &str) -> Vec<u8> {
    let role = match role {
        WrapRole::Password => "password",
        WrapRole::Recovery => "recovery",
    };
    format!("egl-viewer folder identity\0{role}\0{folder_id}").into_bytes()
}

/// A locked folder's age identity, held in memory while the folder is unlocked.
pub(crate) struct FolderKey(age::x25519::Identity);

impl FolderKey {
    pub(crate) fn generate() -> Self {
        Self(age::x25519::Identity::generate())
    }

    pub(crate) fn recipient(&self) -> age::x25519::Recipient {
        self.0.to_public()
    }

    /// The identity encrypted under `key` for `folder_id`: version, nonce, ciphertext.
    pub(crate) fn wrap(&self, key: &WrapKey, role: WrapRole, folder_id: &str) -> Vec<u8> {
        let nonce: [u8; NONCE_LEN] = random();
        let secret = self.0.to_string();
        let aad = wrap_aad(role, folder_id);
        let sealed = key
            .cipher()
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: secret.expose_secret().as_bytes(),
                    aad: &aad,
                },
            )
            .expect("XChaCha20-Poly1305 encrypts any length we give it");
        let mut out = Vec::with_capacity(1 + NONCE_LEN + sealed.len());
        out.push(WRAP_VERSION);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&sealed);
        out
    }

    pub(crate) fn unwrap(
        blob: &[u8],
        key: &WrapKey,
        role: WrapRole,
        folder_id: &str,
    ) -> Result<Self> {
        let (&version, rest) = blob
            .split_first()
            .ok_or(VaultError::Corrupt("wrapped identity"))?;
        if version != WRAP_VERSION || rest.len() <= NONCE_LEN {
            return Err(VaultError::Corrupt("wrapped identity"));
        }
        let (nonce, sealed) = rest.split_at(NONCE_LEN);
        let aad = wrap_aad(role, folder_id);
        let plain = Zeroizing::new(
            key.cipher()
                .decrypt(
                    XNonce::from_slice(nonce),
                    Payload {
                        msg: sealed,
                        aad: &aad,
                    },
                )
                .map_err(|_| VaultError::WrongKey)?,
        );
        let text = std::str::from_utf8(&plain).map_err(|_| VaultError::Corrupt("identity"))?;
        age::x25519::Identity::from_str(text)
            .map(Self)
            .map_err(|_| VaultError::Corrupt("identity"))
    }
}

/// The key of one item: an age identity of its own, wrapped once per locked folder that holds
/// the item, so one folder's password never opens another's items (§2).
pub(crate) struct ItemKey(age::x25519::Identity);

impl ItemKey {
    pub(crate) fn generate() -> Self {
        Self(age::x25519::Identity::generate())
    }

    /// The item key encrypted to a folder: a row of `item_key_wrap`.
    pub(crate) fn wrap_for(&self, folder: &age::x25519::Recipient) -> Vec<u8> {
        let secret = self.0.to_string();
        encrypt_bytes(folder, secret.expose_secret().as_bytes())
    }

    pub(crate) fn unwrap(blob: &[u8], folder: &FolderKey) -> Result<Self> {
        let plain = Zeroizing::new(decrypt_bytes(&folder.0, blob)?);
        let text = std::str::from_utf8(&plain).map_err(|_| VaultError::Corrupt("item key"))?;
        age::x25519::Identity::from_str(text)
            .map(Self)
            .map_err(|_| VaultError::Corrupt("item key"))
    }

    /// An age message to this item (the sealed fields, a preview).
    pub(crate) fn seal(&self, plain: &[u8]) -> Vec<u8> {
        encrypt_bytes(&self.0.to_public(), plain)
    }

    pub(crate) fn open(&self, sealed: &[u8]) -> Result<Vec<u8>> {
        decrypt_bytes(&self.0, sealed)
    }

    /// Streams a file to this item: write the plaintext, then call `finish`.
    pub(crate) fn encrypt_to<W: Write>(
        &self,
        output: W,
    ) -> io::Result<age::stream::StreamWriter<W>> {
        let recipient = self.0.to_public();
        age::Encryptor::with_recipients(iter::once(&recipient as &dyn age::Recipient))
            .expect("one recipient")
            .wrap_output(output)
    }

    /// Streams a file of this item back to plaintext.
    pub(crate) fn decrypt_from<R: Read>(&self, input: R) -> Result<age::stream::StreamReader<R>> {
        age::Decryptor::new(input)
            .and_then(|decryptor| decryptor.decrypt(iter::once(&self.0 as &dyn age::Identity)))
            .map_err(|error| VaultError::Age(error.to_string()))
    }
}

fn encrypt_bytes(recipient: &age::x25519::Recipient, plain: &[u8]) -> Vec<u8> {
    age::encrypt(recipient, plain).expect("encrypting to an x25519 recipient into memory")
}

fn decrypt_bytes(identity: &age::x25519::Identity, sealed: &[u8]) -> Result<Vec<u8>> {
    age::decrypt(identity, sealed).map_err(|error| match error {
        age::DecryptError::NoMatchingKeys => VaultError::WrongKey,
        other => VaultError::Age(other.to_string()),
    })
}

/// The five sensitive fields of an encrypted item, one age message in `item.sealed`
/// (docs/age-locked-folders.md §2). The plaintext columns are empty meanwhile.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SealedFields {
    pub name: String,
    pub annotation: String,
    pub url: String,
    pub exif: Option<String>,
    pub palette: Option<String>,
}

impl SealedFields {
    pub(crate) fn seal(&self, key: &ItemKey) -> Vec<u8> {
        let json = Zeroizing::new(serde_json::to_vec(self).expect("plain strings serialize"));
        key.seal(&json)
    }

    pub(crate) fn open(sealed: &[u8], key: &ItemKey) -> Result<Self> {
        let json = Zeroizing::new(key.open(sealed)?);
        serde_json::from_slice(&json).map_err(|_| VaultError::Corrupt("sealed fields"))
    }
}

/// The recovery secret of a folder: 128 random bits, shown once (§7). Never stored.
pub(crate) struct RecoverySecret(Zeroizing<[u8; 16]>);

/// Crockford's base32: no I, L, O or U, so a written key reads back unambiguously.
const BASE32: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

impl RecoverySecret {
    pub(crate) fn generate() -> Self {
        Self(Zeroizing::new(random()))
    }

    /// The key as written down: 26 base32 characters and 2 check characters, in groups of
    /// four (`XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX`).
    pub(crate) fn key_text(&self) -> String {
        let bytes: &[u8; 16] = &self.0;
        let mut chars = base32(bytes);
        chars.extend(check_chars(bytes));
        chars
            .chunks(4)
            .map(|group| group.iter().map(|&c| c as char).collect::<String>())
            .collect::<Vec<_>>()
            .join("-")
    }

    /// The same bits as 12 words of the BIP39 English list.
    pub(crate) fn phrase(&self) -> String {
        bip39::Mnemonic::from_entropy(self.0.as_ref())
            .expect("16 bytes are valid BIP39 entropy")
            .to_string()
    }

    /// Reads a key or a phrase as the user typed it: case, spaces and dashes do not matter,
    /// and O, I and L read as 0, 1 and 1.
    pub(crate) fn parse(text: &str) -> Result<Self> {
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.len() >= 12 {
            let mnemonic = bip39::Mnemonic::parse(words.join(" ").to_lowercase())
                .map_err(|_| VaultError::BadRecoveryText)?;
            let entropy = Zeroizing::new(mnemonic.to_entropy());
            let bytes: [u8; 16] = entropy
                .as_slice()
                .try_into()
                .map_err(|_| VaultError::BadRecoveryText)?;
            return Ok(Self(Zeroizing::new(bytes)));
        }
        let mut symbols = Vec::with_capacity(28);
        for c in text.chars().filter(|c| !c.is_whitespace() && *c != '-') {
            let c = match c.to_ascii_uppercase() {
                'O' => '0',
                'I' | 'L' => '1',
                other => other,
            };
            let value = BASE32
                .iter()
                .position(|&b| b as char == c)
                .ok_or(VaultError::BadRecoveryText)?;
            symbols.push(value as u8);
        }
        if symbols.len() != 28 {
            return Err(VaultError::BadRecoveryText);
        }
        let bytes = from_base32(&symbols[..26]).ok_or(VaultError::BadRecoveryText)?;
        let expected = check_chars(&bytes);
        let given: Vec<u8> = symbols[26..].iter().map(|&v| BASE32[v as usize]).collect();
        if given != expected {
            return Err(VaultError::BadRecoveryText);
        }
        Ok(Self(Zeroizing::new(bytes)))
    }
}

/// 16 bytes as 26 base32 characters (the last one carries 3 bits).
fn base32(bytes: &[u8; 16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(26);
    let (mut buffer, mut bits) = (0u32, 0u32);
    for &byte in bytes {
        buffer = (buffer << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(BASE32[((buffer >> bits) & 31) as usize]);
        }
    }
    if bits > 0 {
        out.push(BASE32[((buffer << (5 - bits)) & 31) as usize]);
    }
    out
}

fn from_base32(symbols: &[u8]) -> Option<[u8; 16]> {
    let mut out = [0u8; 16];
    let (mut buffer, mut bits, mut at) = (0u32, 0u32, 0usize);
    for &value in symbols {
        buffer = (buffer << 5) | u32::from(value);
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            *out.get_mut(at)? = (buffer >> bits) as u8;
            at += 1;
        }
    }
    // The 2 bits left over must be zero, so each key has one spelling.
    (at == 16 && buffer & ((1 << bits) - 1) == 0).then_some(out)
}

/// Two characters from a hash of the secret: a typo is caught before the slow unwrap.
fn check_chars(bytes: &[u8; 16]) -> [u8; 2] {
    let hash = blake3::hash(bytes);
    let value = u16::from_be_bytes([hash.as_bytes()[0], hash.as_bytes()[1]]) >> 6;
    [
        BASE32[(value >> 5) as usize & 31],
        BASE32[value as usize & 31],
    ]
}

/// A password from the UI, wiped when dropped.
pub(crate) fn password(text: String) -> SecretString {
    SecretString::from(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrap_key(text: &str, salt: &[u8]) -> WrapKey {
        WrapKey::from_password(&password(text.into()), salt, KdfParams::FAST).unwrap()
    }

    #[test]
    fn the_password_unwraps_the_identity_of_its_own_folder_only() {
        let salt = new_salt();
        let key = FolderKey::generate();
        let blob = key.wrap(&wrap_key("correct horse", &salt), WrapRole::Password, "F1");
        let opened = FolderKey::unwrap(
            &blob,
            &wrap_key("correct horse", &salt),
            WrapRole::Password,
            "F1",
        )
        .unwrap();
        assert_eq!(opened.recipient().to_string(), key.recipient().to_string());
        let wrong = FolderKey::unwrap(&blob, &wrap_key("wrong", &salt), WrapRole::Password, "F1");
        assert_eq!(wrong.err(), Some(VaultError::WrongKey));
        // The same wrap moved to another folder or role does not open.
        let moved = FolderKey::unwrap(
            &blob,
            &wrap_key("correct horse", &salt),
            WrapRole::Password,
            "F2",
        );
        assert_eq!(moved.err(), Some(VaultError::WrongKey));
        let role = FolderKey::unwrap(
            &blob,
            &wrap_key("correct horse", &salt),
            WrapRole::Recovery,
            "F1",
        );
        assert_eq!(role.err(), Some(VaultError::WrongKey));
        assert!(matches!(
            FolderKey::unwrap(&[9, 1, 2], &wrap_key("x", &salt), WrapRole::Password, "F1"),
            Err(VaultError::Corrupt(_))
        ));
    }

    #[test]
    fn a_new_salt_gives_another_key_for_the_same_password() {
        let key = FolderKey::generate();
        let blob = key.wrap(&wrap_key("pw", &[1; 16]), WrapRole::Password, "F");
        let other = FolderKey::unwrap(&blob, &wrap_key("pw", &[2; 16]), WrapRole::Password, "F");
        assert_eq!(other.err(), Some(VaultError::WrongKey));
    }

    #[test]
    fn kdf_parameters_round_trip_and_absurd_ones_are_refused() {
        let json = KdfParams::DEFAULT.to_json();
        assert_eq!(
            json,
            r#"{"algorithm":"argon2id","memoryKib":65536,"iterations":3,"parallelism":1}"#
        );
        assert_eq!(KdfParams::from_json(&json).unwrap(), KdfParams::DEFAULT);
        let huge = json.replace("65536", "999999999");
        assert!(KdfParams::from_json(&huge).is_err());
        assert!(KdfParams::from_json("{}").is_err());
    }

    #[test]
    fn the_recovery_secret_has_a_key_and_a_phrase_for_the_same_bits() {
        let secret = RecoverySecret::generate();
        let text = secret.key_text();
        assert_eq!(text.len(), 34, "{text}");
        assert_eq!(text.split('-').count(), 7);
        let phrase = secret.phrase();
        assert_eq!(phrase.split(' ').count(), 12);
        for form in [
            text.clone(),
            text.to_lowercase(),
            text.replace('-', " "),
            phrase.clone(),
            format!("  {}  ", phrase.to_uppercase()),
        ] {
            let parsed = RecoverySecret::parse(&form).unwrap();
            assert_eq!(*parsed.0, *secret.0, "{form}");
        }
    }

    #[test]
    fn a_typo_in_the_recovery_key_is_caught() {
        let secret = RecoverySecret::generate();
        let text = secret.key_text();
        let first = text.chars().next().unwrap();
        let other = if first == 'A' { 'B' } else { 'A' };
        let typo = format!("{other}{}", &text[1..]);
        assert_eq!(
            RecoverySecret::parse(&typo).err(),
            Some(VaultError::BadRecoveryText)
        );
        assert!(RecoverySecret::parse("not a key").is_err());
        assert!(RecoverySecret::parse(&text[..20]).is_err());
        let mut words: Vec<String> = secret.phrase().split(' ').map(String::from).collect();
        words.swap(0, 1);
        // A swapped phrase fails its checksum (almost always) or gives other bits.
        if let Ok(parsed) = RecoverySecret::parse(&words.join(" ")) {
            assert_ne!(*parsed.0, *secret.0);
        }
    }

    #[test]
    fn the_recovery_wrap_opens_with_the_secret_only() {
        let key = FolderKey::generate();
        let secret = RecoverySecret::generate();
        let blob = key.wrap(&WrapKey::from_recovery(&secret), WrapRole::Recovery, "F");
        let parsed = RecoverySecret::parse(&secret.phrase()).unwrap();
        let opened = FolderKey::unwrap(
            &blob,
            &WrapKey::from_recovery(&parsed),
            WrapRole::Recovery,
            "F",
        )
        .unwrap();
        assert_eq!(opened.recipient().to_string(), key.recipient().to_string());
        let other = RecoverySecret::generate();
        assert!(
            FolderKey::unwrap(
                &blob,
                &WrapKey::from_recovery(&other),
                WrapRole::Recovery,
                "F"
            )
            .is_err()
        );
    }

    #[test]
    fn an_item_key_wrapped_for_folder_a_does_not_open_with_folder_b() {
        let (a, b) = (FolderKey::generate(), FolderKey::generate());
        let item = ItemKey::generate();
        let wrap_a = item.wrap_for(&a.recipient());
        let sealed = SealedFields {
            name: "Секретный план".into(),
            annotation: "note".into(),
            url: "https://example.com".into(),
            exif: Some("{}".into()),
            palette: None,
        };
        let blob = sealed.seal(&item);
        let opened = ItemKey::unwrap(&wrap_a, &a).unwrap();
        assert_eq!(SealedFields::open(&blob, &opened).unwrap(), sealed);
        assert!(matches!(
            ItemKey::unwrap(&wrap_a, &b),
            Err(VaultError::WrongKey)
        ));
        assert!(blob.starts_with(AGE_MAGIC));
        assert!(
            !blob
                .windows(6)
                .any(|w| w == "Секрет".as_bytes()[..6].as_ref())
        );
    }

    #[test]
    fn files_stream_through_age() {
        let item = ItemKey::generate();
        let plain: Vec<u8> = (0..200_000u32).map(|n| (n % 251) as u8).collect();
        let mut encrypted = Vec::new();
        let mut writer = item.encrypt_to(&mut encrypted).unwrap();
        writer.write_all(&plain).unwrap();
        writer.finish().unwrap();
        assert!(encrypted.starts_with(AGE_MAGIC));
        let mut back = Vec::new();
        item.decrypt_from(&encrypted[..])
            .unwrap()
            .read_to_end(&mut back)
            .unwrap();
        assert_eq!(back, plain);
        let other = ItemKey::generate();
        assert!(other.decrypt_from(&encrypted[..]).is_err());
    }

    #[test]
    fn base32_is_crockford_and_has_one_spelling() {
        let bytes = [0xffu8; 16];
        let symbols = base32(&bytes);
        assert_eq!(symbols.len(), 26);
        let values: Vec<u8> = symbols
            .iter()
            .map(|&c| BASE32.iter().position(|&b| b == c).unwrap() as u8)
            .collect();
        assert_eq!(from_base32(&values), Some(bytes));
        // Non-zero padding bits are another spelling of the same key: refused.
        let mut padded = values.clone();
        padded[25] |= 1;
        assert_eq!(from_base32(&padded), None);
    }
}
