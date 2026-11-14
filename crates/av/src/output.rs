//! Sound to the default device through `cpal` (WASAPI, CoreAudio, ALSA/PulseAudio), after the
//! Scrobb player's `player-output-cpal`. A `cpal::Stream` cannot move between threads on every
//! platform, so it lives on a thread of its own; dropping [`AudioOutput`] ends it.
//!
//! The device runs at its own default rate and channel count; FFmpeg's filter graph converts to
//! them, so no resampling happens here. The callback allocates nothing.

use std::sync::mpsc::{self, Sender};
use std::thread;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, SupportedStreamConfig};

use crate::rt::{PcmFormat, PcmSource};

const SCRATCH_FRAMES: usize = 4096;

/// The default device's own format; `None` without a device (CI machines, remote sessions).
pub(crate) fn device_format() -> Option<PcmFormat> {
    let device = cpal::default_host().default_output_device()?;
    let config = device.default_output_config().ok()?;
    Some(PcmFormat {
        sample_rate: config.sample_rate(),
        channels: config.channels().clamp(1, 8),
    })
}

/// A running output stream; drop to stop it.
pub(crate) struct AudioOutput {
    stop: Sender<()>,
}

impl Drop for AudioOutput {
    fn drop(&mut self) {
        let _ = self.stop.send(());
    }
}

/// Opens the default device in `format` and starts pulling from `source`.
pub(crate) fn open(format: PcmFormat, source: Box<dyn PcmSource>) -> Result<AudioOutput, String> {
    let (stop, stopped) = mpsc::channel::<()>();
    let (ready, opened) = mpsc::channel::<Result<(), String>>();
    thread::Builder::new()
        .name("av-audio-out".into())
        .spawn(move || {
            let stream = build(format, source).and_then(|stream| {
                stream.play().map_err(|e| e.to_string())?;
                Ok(stream)
            });
            match stream {
                Ok(stream) => {
                    let _ = ready.send(Ok(()));
                    // Blocks until the player drops its handle (or the sender is gone).
                    let _ = stopped.recv();
                    drop(stream);
                }
                Err(error) => {
                    let _ = ready.send(Err(error));
                }
            }
        })
        .map_err(|e| e.to_string())?;
    opened
        .recv()
        .map_err(|_| "the audio thread stopped".to_string())??;
    Ok(AudioOutput { stop })
}

fn build(format: PcmFormat, source: Box<dyn PcmSource>) -> Result<cpal::Stream, String> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or_else(|| "no audio output device".to_string())?;
    let config = choose_config(&device, format)?;
    match config.sample_format() {
        SampleFormat::F32 => build_typed::<f32>(&device, &config, format, source),
        SampleFormat::F64 => build_typed::<f64>(&device, &config, format, source),
        SampleFormat::I16 => build_typed::<i16>(&device, &config, format, source),
        SampleFormat::I32 => build_typed::<i32>(&device, &config, format, source),
        SampleFormat::U16 => build_typed::<u16>(&device, &config, format, source),
        SampleFormat::I8 => build_typed::<i8>(&device, &config, format, source),
        SampleFormat::U8 => build_typed::<u8>(&device, &config, format, source),
        other => Err(format!("sample format {other} is not supported")),
    }
}

/// A device configuration at the wanted rate: the same channel count first, then `f32`.
fn choose_config(
    device: &cpal::Device,
    format: PcmFormat,
) -> Result<SupportedStreamConfig, String> {
    let rate = format.sample_rate;
    device
        .supported_output_configs()
        .map_err(|e| e.to_string())?
        .filter(|r| r.min_sample_rate() <= rate && rate <= r.max_sample_rate())
        .max_by_key(|r| {
            (
                r.channels() == format.channels,
                r.sample_format() == SampleFormat::F32,
                r.channels() >= format.channels,
            )
        })
        .map(|r| r.with_sample_rate(rate))
        .ok_or_else(|| format!("the device does not support {rate} Hz"))
}

fn build_typed<T>(
    device: &cpal::Device,
    config: &SupportedStreamConfig,
    format: PcmFormat,
    mut source: Box<dyn PcmSource>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let src_ch = usize::from(format.channels.max(1));
    let dev_ch = usize::from(config.channels().max(1));
    let mut scratch = vec![0.0f32; SCRATCH_FRAMES * src_ch];
    let data = move |out: &mut [T], _: &cpal::OutputCallbackInfo| {
        let frames = out.len() / dev_ch;
        let mut done = 0;
        while done < frames {
            let n = (frames - done).min(SCRATCH_FRAMES);
            let buf = &mut scratch[..n * src_ch];
            source.fill(buf);
            let dst = &mut out[done * dev_ch..(done + n) * dev_ch];
            for (src, dst) in buf.chunks_exact(src_ch).zip(dst.chunks_exact_mut(dev_ch)) {
                for (d, sample) in dst.iter_mut().enumerate() {
                    let s = match src_ch {
                        1 => src[0],
                        _ => src.get(d).copied().unwrap_or(0.0),
                    };
                    *sample = T::from_sample(s);
                }
            }
            done += n;
        }
    };
    device
        .build_output_stream::<T, _, _>(config.config(), data, |_| {}, None)
        .map_err(|e| e.to_string())
}
