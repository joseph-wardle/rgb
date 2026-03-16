//! Web Audio API audio output.
//!
//! [`WebAudioSink`] implements [`AudioSink`] using a `ScriptProcessorNode`
//! backed by the browser's Web Audio API.  Samples are buffered in a simple
//! ring buffer and drained by the audio processing callback.

use rgb_frontend::AudioSink;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::{AudioContext, AudioContextOptions, ScriptProcessorNode};

/// Ring buffer shared between the emulator thread (push) and the audio
/// callback (drain).  On WASM everything is single-threaded so `Rc<RefCell>`
/// is sufficient.
type SharedBuffer = Rc<RefCell<VecDeque<f32>>>;

/// Capacity in individual f32 samples (stereo interleaved).
/// ~100 ms at 44,100 Hz stereo ≈ 8,820 samples.
const BUFFER_CAPACITY: usize = 8192;

/// Web Audio API audio sink using a ScriptProcessorNode.
pub struct WebAudioSink {
    buffer: SharedBuffer,
    _context: AudioContext,
    _processor: ScriptProcessorNode,
}

impl WebAudioSink {
    /// Create a new Web Audio sink at 44,100 Hz stereo.
    ///
    /// Returns `None` if the browser does not support the Web Audio API or
    /// if context creation fails.
    pub fn open() -> Option<Self> {
        let opts = AudioContextOptions::new();
        opts.set_sample_rate(44_100.0);
        let context = AudioContext::new_with_context_options(&opts).ok()?;

        // ScriptProcessorNode with a 2048-sample buffer, 0 inputs, 2 outputs.
        let processor = context
            .create_script_processor_with_buffer_size_and_number_of_input_channels_and_number_of_output_channels(
                2048, 0, 2,
            )
            .ok()?;

        let buffer: SharedBuffer = Rc::new(RefCell::new(VecDeque::with_capacity(BUFFER_CAPACITY)));
        let cb_buffer = Rc::clone(&buffer);

        // The onaudioprocess callback drains samples from the shared buffer
        // into the output AudioBuffer.
        let callback = Closure::<dyn FnMut(web_sys::AudioProcessingEvent)>::new(
            move |event: web_sys::AudioProcessingEvent| {
                let output = event.output_buffer().unwrap();
                let length = output.length() as usize;

                // Build channel Vecs from the shared buffer, then write them
                // back via copy_to_channel.  get_channel_data() returns a
                // *copy* of the data, so modifying it has no effect — we must
                // call copy_to_channel() to commit the filled buffers.
                let mut left = vec![0.0f32; length];
                let mut right = vec![0.0f32; length];

                let mut buf = cb_buffer.borrow_mut();
                for i in 0..length {
                    // Samples are interleaved: left, right, left, right, …
                    left[i] = buf.pop_front().unwrap_or(0.0);
                    right[i] = buf.pop_front().unwrap_or(0.0);
                }
                drop(buf);

                let _ = output.copy_to_channel(&mut left, 0);
                let _ = output.copy_to_channel(&mut right, 1);
            },
        );

        processor.set_onaudioprocess(Some(callback.as_ref().unchecked_ref()));
        callback.forget(); // prevent GC — lives for the lifetime of the page

        // Connect processor → destination to start audio output.
        processor
            .connect_with_audio_node(&context.destination())
            .ok()?;

        Some(Self {
            buffer,
            _context: context,
            _processor: processor,
        })
    }
}

impl AudioSink for WebAudioSink {
    fn push_samples(&mut self, samples: &[f32]) {
        let mut buf = self.buffer.borrow_mut();
        for &sample in samples {
            if buf.len() < BUFFER_CAPACITY {
                buf.push_back(sample);
            }
            // Drop samples if buffer is full (non-blocking).
        }
    }
}
