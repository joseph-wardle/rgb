//! Audio output via cpal.
//!
//! [`AudioOutput`] opens the default audio device and streams interleaved
//! stereo f32 samples from a ring buffer.  The emulator pushes samples into
//! the producer end once per frame; the audio callback drains the consumer
//! end in a separate thread at the hardware sample rate.
//!
//! If the ring buffer runs empty (the emulator is running slow) the callback
//! outputs silence rather than repeating the last sample, which avoids a
//! buzzing artifact while the emulator catches up.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use ringbuf::traits::{Consumer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};

/// Capacity of the ring buffer in stereo sample pairs.
/// ~100 ms at 44,100 Hz gives a comfortable margin between emulator frames
/// and the audio callback without introducing noticeable latency.
const RING_BUFFER_PAIRS: usize = 4096;

/// Owns the cpal stream and the producer end of the sample ring buffer.
///
/// Drop this to stop audio output; the stream stops when it is dropped.
pub struct AudioOutput {
    /// Push interleaved stereo f32 pairs (left, right) here each frame.
    pub producer: HeapProd<f32>,
    /// Kept alive so the stream is not dropped while the emulator runs.
    _stream: Stream,
}

impl AudioOutput {
    /// Open the default audio device and start the output stream.
    ///
    /// Returns `None` if no audio device is available or the device cannot be
    /// configured — the caller falls back to silent operation in that case.
    pub fn open(sample_rate: u32) -> Option<Self> {
        let host   = cpal::default_host();
        let device = host.default_output_device()?;
        let config = find_config(&device, sample_rate)?;

        let (producer, consumer) = HeapRb::<f32>::new(RING_BUFFER_PAIRS * 2).split();

        let stream = build_stream(&device, &config, consumer)?;
        stream.play().ok()?;

        Some(Self { producer, _stream: stream })
    }
}

/// Find a stereo f32 output config at the requested sample rate.
fn find_config(device: &Device, sample_rate: u32) -> Option<StreamConfig> {
    let supported = device.supported_output_configs().ok()?;
    for range in supported {
        if range.channels() == 2 && range.sample_format() == SampleFormat::F32 {
            let rate = cpal::SampleRate(sample_rate);
            if range.min_sample_rate() <= rate && rate <= range.max_sample_rate() {
                return Some(range.with_sample_rate(rate).config());
            }
        }
    }
    None
}

/// Build an output stream that drains samples from the ring-buffer consumer.
fn build_stream(device: &Device, config: &StreamConfig, mut consumer: HeapCons<f32>) -> Option<Stream> {
    device.build_output_stream(
        config,
        move |output: &mut [f32], _| {
            for sample in output.iter_mut() {
                *sample = consumer.try_pop().unwrap_or(0.0);
            }
        },
        |err| eprintln!("audio stream error: {err}"),
        None,
    ).ok()
}
