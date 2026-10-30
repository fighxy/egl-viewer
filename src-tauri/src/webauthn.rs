//! The app-login WebAuthn bridge (docs/spec.md §13.2).
//!
//! A YubiKey 5C touch creates or asserts a credential through the Windows WebAuthn
//! API (`webauthn.dll`). The app stores the credential id and the P-256 public key and
//! checks the assertion; the touch does not unwrap a folder identity and login does not
//! ask for a PIN. With no authenticator, enrollment and login refuse.
//! Tests assert a software P-256 credential through the same check, without this bridge.

#![allow(unsafe_code)]

use std::slice;

use app_store::{Authenticator, WebAuthnAssertion, WebAuthnCredential};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use library_core::SourceError;
use p256::elliptic_curve::sec1::{FromEncodedPoint, ToEncodedPoint};
use p256::{EncodedPoint, PublicKey};
use windows::Win32::Foundation::HWND;
use windows::Win32::Networking::WindowsWebServices::{
    WEBAUTHN_ASSERTION, WEBAUTHN_AUTHENTICATOR_ATTACHMENT_CROSS_PLATFORM,
    WEBAUTHN_AUTHENTICATOR_GET_ASSERTION_OPTIONS,
    WEBAUTHN_AUTHENTICATOR_GET_ASSERTION_OPTIONS_CURRENT_VERSION,
    WEBAUTHN_AUTHENTICATOR_MAKE_CREDENTIAL_OPTIONS,
    WEBAUTHN_AUTHENTICATOR_MAKE_CREDENTIAL_OPTIONS_CURRENT_VERSION, WEBAUTHN_CLIENT_DATA,
    WEBAUTHN_CLIENT_DATA_CURRENT_VERSION, WEBAUTHN_COSE_ALGORITHM_ECDSA_P256_WITH_SHA256,
    WEBAUTHN_COSE_CREDENTIAL_PARAMETER, WEBAUTHN_COSE_CREDENTIAL_PARAMETERS,
    WEBAUTHN_CREDENTIAL_ATTESTATION, WEBAUTHN_CREDENTIAL_EX, WEBAUTHN_CREDENTIAL_LIST,
    WEBAUTHN_CTAP_TRANSPORT_USB, WEBAUTHN_RP_ENTITY_INFORMATION,
    WEBAUTHN_RP_ENTITY_INFORMATION_CURRENT_VERSION, WEBAUTHN_USER_ENTITY_INFORMATION,
    WEBAUTHN_USER_ENTITY_INFORMATION_CURRENT_VERSION,
    WEBAUTHN_USER_VERIFICATION_REQUIREMENT_DISCOURAGED, WebAuthNAuthenticatorGetAssertion,
    WebAuthNAuthenticatorMakeCredential, WebAuthNFreeAssertion, WebAuthNFreeCredentialAttestation,
};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
use windows::core::{HSTRING, PCWSTR};

use crate::yubikey;

const ORIGIN: &str = "https://egl-viewer.local";
const RP_ID: &str = "egl-viewer.local";
const RP_NAME: &str = "egl-viewer";
const USER_NAME: &str = "egl-viewer";
const USER_DISPLAY: &str = "egl-viewer";
const TIMEOUT_MS: u32 = 60_000;
const ABSENT: &str = "Connect a YubiKey 5C and touch it. This login does not unlock folders.";
const ABSENT_LOGIN: &str = "Touch the YubiKey to log in. No YubiKey is connected.";

pub struct SystemWebAuthn;

impl Authenticator for SystemWebAuthn {
    fn register(&mut self, challenge: &[u8]) -> Result<WebAuthnCredential, SourceError> {
        require_key(ABSENT)?;
        let client_json = client_data_json("webauthn.create", challenge);
        let hash_alg = HSTRING::from("SHA-256");
        let rp_id = HSTRING::from(RP_ID);
        let rp_name = HSTRING::from(RP_NAME);
        let user_name = HSTRING::from(USER_NAME);
        let user_display = HSTRING::from(USER_DISPLAY);
        let cred_type = HSTRING::from("public-key");
        let mut user_id = *b"egl-viewer-user\0\0";
        let client_data = WEBAUTHN_CLIENT_DATA {
            dwVersion: WEBAUTHN_CLIENT_DATA_CURRENT_VERSION,
            cbClientDataJSON: client_json.len() as u32,
            pbClientDataJSON: client_json.as_ptr() as *mut u8,
            pwszHashAlgId: PCWSTR(hash_alg.as_ptr()),
        };
        let rp = WEBAUTHN_RP_ENTITY_INFORMATION {
            dwVersion: WEBAUTHN_RP_ENTITY_INFORMATION_CURRENT_VERSION,
            pwszId: PCWSTR(rp_id.as_ptr()),
            pwszName: PCWSTR(rp_name.as_ptr()),
            pwszIcon: PCWSTR::null(),
        };
        let user = WEBAUTHN_USER_ENTITY_INFORMATION {
            dwVersion: WEBAUTHN_USER_ENTITY_INFORMATION_CURRENT_VERSION,
            cbId: user_id.len() as u32,
            pbId: user_id.as_mut_ptr(),
            pwszName: PCWSTR(user_name.as_ptr()),
            pwszIcon: PCWSTR::null(),
            pwszDisplayName: PCWSTR(user_display.as_ptr()),
        };
        let mut cose = WEBAUTHN_COSE_CREDENTIAL_PARAMETER {
            dwVersion: 1,
            pwszCredentialType: PCWSTR(cred_type.as_ptr()),
            lAlg: WEBAUTHN_COSE_ALGORITHM_ECDSA_P256_WITH_SHA256,
        };
        let params = WEBAUTHN_COSE_CREDENTIAL_PARAMETERS {
            cCredentialParameters: 1,
            pCredentialParameters: &mut cose,
        };
        let options = WEBAUTHN_AUTHENTICATOR_MAKE_CREDENTIAL_OPTIONS {
            dwVersion: WEBAUTHN_AUTHENTICATOR_MAKE_CREDENTIAL_OPTIONS_CURRENT_VERSION,
            dwTimeoutMilliseconds: TIMEOUT_MS,
            dwAuthenticatorAttachment: WEBAUTHN_AUTHENTICATOR_ATTACHMENT_CROSS_PLATFORM,
            dwUserVerificationRequirement: WEBAUTHN_USER_VERIFICATION_REQUIREMENT_DISCOURAGED,
            ..Default::default()
        };
        let hwnd = foreground_hwnd();
        let attestation = unsafe {
            WebAuthNAuthenticatorMakeCredential(
                hwnd,
                &rp,
                &user,
                &params,
                &client_data,
                Some(&options),
            )
            .map_err(|_| SourceError::NotSupported(ABSENT.into()))?
        };
        let result = unsafe { read_registration(attestation) };
        unsafe {
            WebAuthNFreeCredentialAttestation(Some(attestation));
        }
        result
    }

    fn authenticate(
        &mut self,
        challenge: &[u8],
        credential_id: &[u8],
    ) -> Result<WebAuthnAssertion, SourceError> {
        require_key(ABSENT_LOGIN)?;
        let client_json = client_data_json("webauthn.get", challenge);
        let hash_alg = HSTRING::from("SHA-256");
        let rp_id = HSTRING::from(RP_ID);
        let cred_type = HSTRING::from("public-key");
        let mut cred_id = credential_id.to_vec();
        let client_data = WEBAUTHN_CLIENT_DATA {
            dwVersion: WEBAUTHN_CLIENT_DATA_CURRENT_VERSION,
            cbClientDataJSON: client_json.len() as u32,
            pbClientDataJSON: client_json.as_ptr() as *mut u8,
            pwszHashAlgId: PCWSTR(hash_alg.as_ptr()),
        };
        let mut allow = WEBAUTHN_CREDENTIAL_EX {
            dwVersion: 1,
            cbId: cred_id.len() as u32,
            pbId: cred_id.as_mut_ptr(),
            pwszCredentialType: PCWSTR(cred_type.as_ptr()),
            dwTransports: WEBAUTHN_CTAP_TRANSPORT_USB,
        };
        let mut allow_ptr: *mut WEBAUTHN_CREDENTIAL_EX = &mut allow;
        let mut allow_list = WEBAUTHN_CREDENTIAL_LIST {
            cCredentials: 1,
            ppCredentials: &mut allow_ptr,
        };
        let options = WEBAUTHN_AUTHENTICATOR_GET_ASSERTION_OPTIONS {
            dwVersion: WEBAUTHN_AUTHENTICATOR_GET_ASSERTION_OPTIONS_CURRENT_VERSION,
            dwTimeoutMilliseconds: TIMEOUT_MS,
            dwAuthenticatorAttachment: WEBAUTHN_AUTHENTICATOR_ATTACHMENT_CROSS_PLATFORM,
            dwUserVerificationRequirement: WEBAUTHN_USER_VERIFICATION_REQUIREMENT_DISCOURAGED,
            pAllowCredentialList: &mut allow_list,
            ..Default::default()
        };
        let hwnd = foreground_hwnd();
        let assertion = unsafe {
            WebAuthNAuthenticatorGetAssertion(
                hwnd,
                PCWSTR(rp_id.as_ptr()),
                &client_data,
                Some(&options),
            )
            .map_err(|_| SourceError::NotSupported(ABSENT_LOGIN.into()))?
        };
        let result = unsafe { read_assertion(assertion, &client_json) };
        unsafe {
            WebAuthNFreeAssertion(assertion);
        }
        result
    }
}

fn require_key(message: &str) -> Result<(), SourceError> {
    if yubikey::present() {
        Ok(())
    } else {
        Err(SourceError::NotSupported(message.into()))
    }
}

fn foreground_hwnd() -> HWND {
    unsafe { GetForegroundWindow() }
}

fn client_data_json(webauthn_type: &str, challenge: &[u8]) -> String {
    let challenge = URL_SAFE_NO_PAD.encode(challenge);
    format!(
        r#"{{"type":"{webauthn_type}","challenge":"{challenge}","origin":"{ORIGIN}","crossOrigin":false}}"#
    )
}

unsafe fn read_registration(
    attestation: *mut WEBAUTHN_CREDENTIAL_ATTESTATION,
) -> Result<WebAuthnCredential, SourceError> {
    let attestation = unsafe { &*attestation };
    let id = unsafe {
        slice::from_raw_parts(
            attestation.pbCredentialId,
            attestation.cbCredentialId as usize,
        )
    }
    .to_vec();
    let auth_data = if attestation.cbAuthenticatorData > 0 {
        unsafe {
            slice::from_raw_parts(
                attestation.pbAuthenticatorData,
                attestation.cbAuthenticatorData as usize,
            )
        }
    } else {
        let object = unsafe {
            slice::from_raw_parts(
                attestation.pbAttestationObject,
                attestation.cbAttestationObject as usize,
            )
        };
        auth_data_from_attestation_object(object)?
    };
    let public_key = public_key_from_auth_data(auth_data)?;
    Ok(WebAuthnCredential { id, public_key })
}

unsafe fn read_assertion(
    assertion: *mut WEBAUTHN_ASSERTION,
    client_data_json: &str,
) -> Result<WebAuthnAssertion, SourceError> {
    let assertion = unsafe { &*assertion };
    let id = unsafe {
        slice::from_raw_parts(
            assertion.Credential.pbId,
            assertion.Credential.cbId as usize,
        )
    }
    .to_vec();
    let authenticator_data = unsafe {
        slice::from_raw_parts(
            assertion.pbAuthenticatorData,
            assertion.cbAuthenticatorData as usize,
        )
    }
    .to_vec();
    let signature =
        unsafe { slice::from_raw_parts(assertion.pbSignature, assertion.cbSignature as usize) }
            .to_vec();
    Ok(WebAuthnAssertion {
        id,
        authenticator_data,
        client_data_json: client_data_json.as_bytes().to_vec(),
        signature,
    })
}

fn auth_data_from_attestation_object(object: &[u8]) -> Result<&[u8], SourceError> {
    let map = Cbor::parse(object).map_err(|_| SourceError::NotSupported(ABSENT.into()))?;
    let Cbor::Map(entries) = map else {
        return Err(SourceError::NotSupported(ABSENT.into()));
    };
    for (key, value) in entries {
        if matches!(&key, Cbor::Text(text) if *text == "authData")
            && let Cbor::Bytes(bytes) = value
        {
            return Ok(bytes);
        }
    }
    Err(SourceError::NotSupported(ABSENT.into()))
}

fn public_key_from_auth_data(auth_data: &[u8]) -> Result<Vec<u8>, SourceError> {
    if auth_data.len() < 37 {
        return Err(SourceError::NotSupported(ABSENT.into()));
    }
    let flags = auth_data[32];
    if flags & 0x40 == 0 {
        return Err(SourceError::NotSupported(ABSENT.into()));
    }
    let mut offset = 37;
    if auth_data.len() < offset + 18 {
        return Err(SourceError::NotSupported(ABSENT.into()));
    }
    offset += 16; // aaguid
    let cred_len = u16::from_be_bytes([auth_data[offset], auth_data[offset + 1]]) as usize;
    offset += 2;
    if auth_data.len() < offset + cred_len {
        return Err(SourceError::NotSupported(ABSENT.into()));
    }
    offset += cred_len;
    let cose =
        Cbor::parse(&auth_data[offset..]).map_err(|_| SourceError::NotSupported(ABSENT.into()))?;
    let Cbor::Map(entries) = cose else {
        return Err(SourceError::NotSupported(ABSENT.into()));
    };
    let mut x = None;
    let mut y = None;
    for (key, value) in entries {
        match (key, value) {
            (Cbor::Int(-2), Cbor::Bytes(bytes)) if bytes.len() == 32 => x = Some(bytes),
            (Cbor::Int(-3), Cbor::Bytes(bytes)) if bytes.len() == 32 => y = Some(bytes),
            _ => {}
        }
    }
    let (x, y) = match (x, y) {
        (Some(x), Some(y)) => (x, y),
        _ => return Err(SourceError::NotSupported(ABSENT.into())),
    };
    let mut uncompressed = Vec::with_capacity(65);
    uncompressed.push(0x04);
    uncompressed.extend_from_slice(x);
    uncompressed.extend_from_slice(y);
    let point = EncodedPoint::from_bytes(&uncompressed)
        .map_err(|_| SourceError::NotSupported(ABSENT.into()))?;
    let public = Option::<PublicKey>::from(PublicKey::from_encoded_point(&point))
        .ok_or_else(|| SourceError::NotSupported(ABSENT.into()))?;
    Ok(public.to_encoded_point(true).as_bytes().to_vec())
}

/// Tiny CBOR reader for WebAuthn attestation maps and COSE keys.
enum Cbor<'a> {
    Int(i128),
    Bytes(&'a [u8]),
    Text(&'a str),
    Map(Vec<(Cbor<'a>, Cbor<'a>)>),
    Other,
}

impl<'a> Cbor<'a> {
    fn parse(input: &'a [u8]) -> Result<Self, ()> {
        let (value, rest) = Self::read(input)?;
        if !rest.is_empty() && matches!(value, Self::Map(_)) {
            // Attestation objects are a single map; trailing padding is refused.
            // COSE keys may be followed by nothing we care about when sliced from authData.
        }
        let _ = rest;
        Ok(value)
    }

    fn read(input: &'a [u8]) -> Result<(Self, &'a [u8]), ()> {
        let (first, mut rest) = input.split_first().ok_or(())?;
        let major = first >> 5;
        let mut additional = (first & 0x1f) as u64;
        match additional {
            24 => {
                let (b, next) = rest.split_first().ok_or(())?;
                additional = u64::from(*b);
                rest = next;
            }
            25 => {
                if rest.len() < 2 {
                    return Err(());
                }
                additional = u64::from(u16::from_be_bytes([rest[0], rest[1]]));
                rest = &rest[2..];
            }
            26 => {
                if rest.len() < 4 {
                    return Err(());
                }
                additional = u64::from(u32::from_be_bytes(rest[..4].try_into().unwrap()));
                rest = &rest[4..];
            }
            27 => {
                if rest.len() < 8 {
                    return Err(());
                }
                additional = u64::from_be_bytes(rest[..8].try_into().unwrap());
                rest = &rest[8..];
            }
            28..=31 => return Err(()),
            _ => {}
        }
        match major {
            0 => Ok((Self::Int(additional as i128), rest)),
            1 => Ok((Self::Int(-1 - additional as i128), rest)),
            2 => {
                let len = additional as usize;
                if rest.len() < len {
                    return Err(());
                }
                Ok((Self::Bytes(&rest[..len]), &rest[len..]))
            }
            3 => {
                let len = additional as usize;
                if rest.len() < len {
                    return Err(());
                }
                let text = std::str::from_utf8(&rest[..len]).map_err(|_| ())?;
                Ok((Self::Text(text), &rest[len..]))
            }
            5 => {
                let mut entries = Vec::with_capacity(additional as usize);
                let mut cursor = rest;
                for _ in 0..additional {
                    let (key, next) = Self::read(cursor)?;
                    let (value, next) = Self::read(next)?;
                    entries.push((key, value));
                    cursor = next;
                }
                Ok((Self::Map(entries), cursor))
            }
            _ => Ok((Self::Other, rest)),
        }
    }
}
