//! The YubiKey 5C as a PIV card (docs/age-locked-folders.md §9).
//!
//! A connected card does P-256 ECDH in a retired slot, the same operation
//! `age-plugin-yubikey` asks of the PIV applet. With no card, every call refuses and the
//! folder password and recovery secret are left to open the folder. Tests of the wrap use a
//! software P-256 key that produces the same `piv-p256` stanza; they do not come through here.

use library_core::{PivPolicy, PivPrompt, SourceError};
use library_native::{PivCard, recipient_from_public_key};
use p256::PublicKey;
use yubikey::piv::{self, AlgorithmId, RetiredSlotId, SlotId};
use yubikey::{MgmKey, PinPolicy, Serial, TouchPolicy, YubiKey};

const ABSENT: &str =
    "No YubiKey is connected. The password and the recovery key still open this folder.";

const RETIRED: [RetiredSlotId; 20] = [
    RetiredSlotId::R1,
    RetiredSlotId::R2,
    RetiredSlotId::R3,
    RetiredSlotId::R4,
    RetiredSlotId::R5,
    RetiredSlotId::R6,
    RetiredSlotId::R7,
    RetiredSlotId::R8,
    RetiredSlotId::R9,
    RetiredSlotId::R10,
    RetiredSlotId::R11,
    RetiredSlotId::R12,
    RetiredSlotId::R13,
    RetiredSlotId::R14,
    RetiredSlotId::R15,
    RetiredSlotId::R16,
    RetiredSlotId::R17,
    RetiredSlotId::R18,
    RetiredSlotId::R19,
    RetiredSlotId::R20,
];

pub fn present() -> bool {
    open_any().is_ok()
}

pub fn enroll(policy: PivPolicy, pin: Option<&str>) -> Result<PivCard, SourceError> {
    let mut yubikey = open_any()?;
    authenticate_management(&mut yubikey)?;
    if let Some(pin) = pin {
        yubikey.verify_pin(pin.as_bytes()).map_err(map_pin_error)?;
    }

    let slot = empty_retired_slot(&mut yubikey)?;
    let (pin_policy, touch_policy) = match policy {
        PivPolicy::Touch => (PinPolicy::Never, TouchPolicy::Always),
        PivPolicy::Pin => (PinPolicy::Always, TouchPolicy::Never),
    };
    let spki = piv::generate(
        &mut yubikey,
        SlotId::Retired(slot),
        AlgorithmId::EccP256,
        pin_policy,
        touch_policy,
    )
    .map_err(|error| {
        SourceError::Storage(format!("Could not create a key on the YubiKey: {error}"))
    })?;
    let public = PublicKey::from_sec1_bytes(spki.subject_public_key.raw_bytes()).map_err(|_| {
        SourceError::Storage("The YubiKey returned an unreadable P-256 public key.".into())
    })?;
    let recipient = recipient_from_public_key(&public);
    let serial = yubikey.serial().to_string();
    PivCard::new(serial, u8::from(slot), policy, recipient)
}

pub fn agree(
    serial: &str,
    slot: u8,
    prompt: &PivPrompt,
    ephemeral: &[u8],
) -> Result<[u8; 32], SourceError> {
    let wanted: Serial = serial
        .parse()
        .map_err(|_| SourceError::Invalid("A YubiKey serial is up to 10 digits.".into()))?;
    let retired = RetiredSlotId::try_from(slot).map_err(|_| {
        SourceError::Invalid("A YubiKey wrap uses a retired PIV slot, 82 to 95.".into())
    })?;
    let mut yubikey = open_by_serial(wanted)?;
    match prompt {
        PivPrompt::Touch => {}
        PivPrompt::Pin { pin } => {
            yubikey.verify_pin(pin.as_bytes()).map_err(map_pin_error)?;
        }
    }
    let shared = piv::decrypt_data(
        &mut yubikey,
        ephemeral,
        AlgorithmId::EccP256,
        SlotId::Retired(retired),
    )
    .map_err(|_| SourceError::WrongPassword)?;
    let shared: &[u8] = shared.as_ref();
    let bytes: [u8; 32] = shared.try_into().map_err(|_| SourceError::WrongPassword)?;
    Ok(bytes)
}

fn open_any() -> Result<YubiKey, SourceError> {
    YubiKey::open().map_err(|_| SourceError::NotSupported(ABSENT.into()))
}

fn open_by_serial(serial: Serial) -> Result<YubiKey, SourceError> {
    YubiKey::open_by_serial(serial).map_err(|_| SourceError::NotSupported(ABSENT.into()))
}

fn authenticate_management(yubikey: &mut YubiKey) -> Result<(), SourceError> {
    match yubikey.authenticate(MgmKey::default()) {
        Ok(()) => Ok(()),
        Err(_) => {
            let _ = yubikey.verify_pin(b"123456");
            match MgmKey::get_protected(yubikey) {
                Ok(mgm) => yubikey.authenticate(mgm).map_err(|_| {
                    SourceError::NotSupported(
                        "The YubiKey management key is not the factory default. Reset PIV management, or use the password to open the folder.".into(),
                    )
                }),
                Err(_) => Err(SourceError::NotSupported(
                    "The YubiKey management key is not the factory default. Reset PIV management, or use the password to open the folder.".into(),
                )),
            }
        }
    }
}

fn empty_retired_slot(yubikey: &mut YubiKey) -> Result<RetiredSlotId, SourceError> {
    for slot in RETIRED {
        match piv::metadata(yubikey, SlotId::Retired(slot)) {
            Ok(meta) if meta.public.is_some() => continue,
            Ok(_) | Err(_) => return Ok(slot),
        }
    }
    Err(SourceError::NotSupported(
        "Every retired PIV slot on this YubiKey already has a key.".into(),
    ))
}

fn map_pin_error(error: yubikey::Error) -> SourceError {
    match error {
        yubikey::Error::WrongPin { .. } | yubikey::Error::PinLocked => SourceError::WrongPassword,
        _ => SourceError::NotSupported(ABSENT.into()),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "needs a YubiKey 5C plugged in"]
    fn a_plugged_in_yubikey_is_present() {
        assert!(
            super::present(),
            "Windows PC/SC sees a YubiKey, but present() is false"
        );
    }
}
