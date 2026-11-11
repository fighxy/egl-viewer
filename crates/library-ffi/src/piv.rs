//! A YubiKey for locked folders, for an app that talks to the card itself (the iPhone through
//! YubiKit, D-126). The core never sees the card: it says what the card must compute
//! (`egl_piv_challenge`), and takes the card's ECDH answer (`egl_unlock_with_piv`). Binding a
//! card takes the public key the card made in a retired slot. Bytes travel as lowercase hex.

use std::os::raw::c_char;

use library_core::PivPolicy;
use library_native::PivCard;
use serde::{Deserialize, Serialize};

use crate::{EglLibrary, EglStatus, arg, fail, parse, reply, reply_unit, tri, with_lib};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(text.get(at..at + 2)?, 16).ok())
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Challenge {
    slot: u8,
    policy: PivPolicy,
    /// The wrap's ephemeral P-256 point, uncompressed SEC1, hex.
    ephemeral: String,
}

/// The YubiKeys bound to locked folder `id`: `PivWrap[]` (`{"serial","slot","policy"}`).
/// Nothing secret is in the list.
///
/// # Safety
/// `lib` is a live handle; the string is NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_piv_wraps(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let id = tri!(arg(handle, id_utf8, "folder id"));
            reply(
                handle,
                out_json,
                handle.service.piv_wraps(id),
                "YubiKey wraps",
            )
        })
    }
}

/// What card `serial` must compute to open folder `id`: `{"slot":130,"policy":"touch"|"pin",
/// "ephemeral":"04…"}`. The app asks the card for P-256 ECDH in that slot with that point
/// (after its PIN, or while the user touches it) and passes the 32-byte answer to
/// `egl_unlock_with_piv`. `EGL_ERR_NOT_FOUND` when the card is not bound to the folder.
///
/// # Safety
/// `lib` is a live handle; the strings are NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_piv_challenge(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    serial_utf8: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, strings and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let id = tri!(arg(handle, id_utf8, "folder id"));
            let serial = tri!(arg(handle, serial_utf8, "YubiKey serial"));
            let challenge = handle
                .service
                .piv_challenge(id, serial)
                .map(|challenge| Challenge {
                    slot: challenge.slot,
                    policy: challenge.policy,
                    ephemeral: hex(&challenge.ephemeral),
                });
            reply(handle, out_json, challenge, "YubiKey challenge")
        })
    }
}

/// Opens folder `id` with card `serial`'s ECDH answer to `egl_piv_challenge`, 32 bytes as hex.
/// `EGL_ERR_WRONG_PASSWORD` when the answer does not open the wrap.
///
/// # Safety
/// `lib` is a live handle; the strings are NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_unlock_with_piv(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    serial_utf8: *const c_char,
    shared_hex: *const c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and strings.
    unsafe {
        with_lib(lib, |handle| {
            let id = tri!(arg(handle, id_utf8, "folder id"));
            let serial = tri!(arg(handle, serial_utf8, "YubiKey serial"));
            let Some(shared) = unhex(tri!(arg(handle, shared_hex, "YubiKey answer"))) else {
                return fail(handle, EglStatus::Invalid, "the YubiKey answer is not hex");
            };
            reply_unit(
                handle,
                handle.service.unlock_with_piv_secret(id, serial, &shared),
            )
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BindRequest {
    password: String,
    serial: String,
    slot: u8,
    policy: PivPolicy,
    /// SEC1 public key of the slot, hex.
    public_key: String,
}

/// Binds a card as a third way to open folder `id` (D-126). `request_json`:
/// `{"password":"…","serial":"12345678","slot":130,"policy":"touch"|"pin","publicKey":"04…"}`:
/// the folder's password, and the card's key the app made in a retired slot with that policy.
///
/// # Safety
/// `lib` is a live handle; the strings are NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_bind_piv(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    request_json: *const c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and strings.
    unsafe {
        with_lib(lib, |handle| {
            let id = tri!(arg(handle, id_utf8, "folder id"));
            let request: BindRequest = tri!(parse(handle, request_json, "YubiKey bind JSON"));
            let Some(public) = unhex(&request.public_key) else {
                return fail(handle, EglStatus::Invalid, "the public key is not hex");
            };
            let result =
                PivCard::from_public_key(request.serial, request.slot, request.policy, &public)
                    .and_then(|card| handle.service.bind_piv(id, request.password, &card));
            reply_unit(handle, result)
        })
    }
}

/// Unbinds card `serial` from folder `id`; the folder's password opens it as before.
///
/// # Safety
/// `lib` is a live handle; the strings are NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_unbind_piv(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    password_utf8: *const c_char,
    serial_utf8: *const c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and strings.
    unsafe {
        with_lib(lib, |handle| {
            let id = tri!(arg(handle, id_utf8, "folder id"));
            let password = tri!(arg(handle, password_utf8, "password"));
            let serial = tri!(arg(handle, serial_utf8, "YubiKey serial"));
            reply_unit(
                handle,
                handle.service.unbind_piv(id, password.to_owned(), serial),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{hex, unhex};

    #[test]
    fn hex_round_trips_and_refuses_junk() {
        assert_eq!(hex(&[0x04, 0xab, 0x00]), "04ab00");
        assert_eq!(unhex("04AB00"), Some(vec![0x04, 0xab, 0x00]));
        assert_eq!(unhex("0"), None);
        assert_eq!(unhex("zz"), None);
        assert_eq!(unhex("é0"), None);
    }
}
