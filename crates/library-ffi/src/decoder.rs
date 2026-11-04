//! The platform image decoder (D-096): the app lends its own decoder for formats this build of
//! the core cannot read, HEIC on iOS through ImageIO. Process-wide, set once at launch; called
//! on the import's background threads.

use std::ffi::{CString, c_void};
use std::os::raw::c_char;
use std::sync::Arc;

use media::{PlatformImage, set_platform_decoder};

use crate::{EglStatus, guarded};

/// Where a decode callback puts the image: hand it to `egl_decode_sink_put`, at most once.
pub struct EglDecodeSink {
    image: Option<PlatformImage>,
}

/// Decodes `len` bytes of a file of extension `ext_utf8` (lowercase, no dot). On success the
/// callback calls `egl_decode_sink_put(sink, …)` once and returns nonzero; it returns 0 when it
/// cannot decode. The bytes and the extension are valid only during the call.
pub type EglDecodeCallback = Option<
    unsafe extern "C" fn(
        ctx: *mut c_void,
        bytes: *const u8,
        len: usize,
        ext_utf8: *const c_char,
        sink: *mut EglDecodeSink,
    ) -> i32,
>;

/// Sets (or with a null `callback`, clears) the process's platform decoder. It may be called
/// from any thread until it is cleared.
///
/// # Safety
/// `callback` and `ctx` stay valid until the decoder is cleared or replaced.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_set_platform_decoder(
    callback: EglDecodeCallback,
    ctx: *mut c_void,
) -> EglStatus {
    guarded(
        |_| {},
        || {
            let Some(call) = callback else {
                set_platform_decoder(None);
                return EglStatus::Ok;
            };
            // The context is kept as an address: Rust never dereferences it.
            let ctx = ctx as usize;
            set_platform_decoder(Some(Arc::new(move |bytes: &[u8], ext: &str| {
                let ext = CString::new(ext).ok()?;
                let mut sink = EglDecodeSink { image: None };
                // SAFETY: the app registered `call` for `ctx` and keeps both valid while set;
                // the bytes, the extension and the sink live for the call.
                let decoded = unsafe {
                    call(
                        ctx as *mut c_void,
                        bytes.as_ptr(),
                        bytes.len(),
                        ext.as_ptr(),
                        &mut sink,
                    )
                };
                if decoded == 0 { None } else { sink.image }
            })));
            EglStatus::Ok
        },
    )
}

/// Hands the decoded image to the core: the image's displayed size (`width`, `height`, after
/// orientation) and its RGBA pixels, `pixel_width × pixel_height × 4` bytes, which may be
/// smaller than the displayed size. The pixels are copied.
///
/// # Safety
/// `sink` is the one the callback got; `rgba` points at `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_decode_sink_put(
    sink: *mut EglDecodeSink,
    width: u32,
    height: u32,
    pixel_width: u32,
    pixel_height: u32,
    rgba: *const u8,
    len: usize,
) {
    if sink.is_null() || rgba.is_null() {
        return;
    }
    let _ = guarded(
        |_| {},
        || {
            // SAFETY: the caller's sink and buffer, valid for the call.
            let (sink, bytes) = unsafe { (&mut *sink, std::slice::from_raw_parts(rgba, len)) };
            sink.image = Some(PlatformImage {
                width,
                height,
                pixel_width,
                pixel_height,
                rgba: bytes.to_vec(),
            });
            EglStatus::Ok
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::ffi::CStr;

    /// A stand-in for ImageIO: decodes only bytes that start with `PLATFORM`.
    unsafe extern "C" fn fake(
        ctx: *mut c_void,
        bytes: *const u8,
        len: usize,
        ext: *const c_char,
        sink: *mut EglDecodeSink,
    ) -> i32 {
        // SAFETY: the core's arguments, valid for the call.
        let (bytes, ext) = unsafe { (std::slice::from_raw_parts(bytes, len), CStr::from_ptr(ext)) };
        assert_eq!(ctx as usize, 7);
        if ext.to_bytes() != b"heic" || !bytes.starts_with(b"PLATFORM") {
            return 0;
        }
        let rgba = [120u8; 8 * 6 * 4];
        // SAFETY: the sink of this call and a live buffer.
        unsafe { egl_decode_sink_put(sink, 800, 600, 8, 6, rgba.as_ptr(), rgba.len()) };
        1
    }

    #[test]
    fn the_core_renders_heic_through_the_platform_decoder() {
        // SAFETY: a static function and a context that is never dereferenced.
        assert_eq!(
            unsafe { egl_set_platform_decoder(Some(fake), 7 as *mut c_void) },
            EglStatus::Ok
        );
        let rendered = media::render_bytes(b"PLATFORM heic bytes", "heic").unwrap();
        assert_eq!((rendered.width, rendered.height), (800, 600));
        // Bytes the decoder declines go to the built-in path, which refuses junk.
        assert!(media::render_bytes(b"junk", "heic").is_err());
        // SAFETY: clearing needs no pointers.
        assert_eq!(
            unsafe { egl_set_platform_decoder(None, std::ptr::null_mut()) },
            EglStatus::Ok
        );
    }
}
