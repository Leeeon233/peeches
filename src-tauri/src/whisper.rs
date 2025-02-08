// https://github.com/thewh1teagle/vad-rs/blob/main/examples/whisper/src/main.rs

use ringbuffer::{AllocRingBuffer, RingBuffer};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

#[derive(Clone)]
pub struct Whisper {
    // vad: Arc<Mutex<Vad>>,
    speech_buf: Arc<Mutex<AllocRingBuffer<f32>>>,
    whisper_ctx: Arc<Mutex<WhisperState>>,
    samples_count: Arc<AtomicUsize>,
    // normalizer: Arc<Mutex<Normalizer>>,
}

impl Whisper {
    pub fn new(whisper_model_path: &str) -> Self {
        // let vad = Vad::new(vad_model_path, 16000).unwrap();
        // let normalizer = Normalizer::new(1, 16000);
        let ctx = WhisperContext::new_with_params(
            whisper_model_path,
            WhisperContextParameters {
                use_gpu: true,
                flash_attn: false,
                ..Default::default()
            },
        )
        .unwrap();
        let state = ctx.create_state().expect("failed to create key");

        Self {
            // vad: Arc::new(Mutex::new(vad)),
            whisper_ctx: Arc::new(Mutex::new(state)),
            speech_buf: Arc::new(Mutex::new(AllocRingBuffer::new(16000 * 3))),
            samples_count: Arc::new(AtomicUsize::new(0)),
            // params: Arc::new(Mutex::new(params)),
            // normalizer: Arc::new(Mutex::new(normalizer)),
        }
    }

    pub fn add_new_samples(&self, samples: &[f32], sample_rate: u32, channels: u16) {
        let mut resampled: Vec<f32> = audio_resample(samples, sample_rate, 16000, channels);

        if channels > 1 {
            resampled = stereo_to_mono(&resampled).unwrap();
        }
        self.samples_count.fetch_add(1, Ordering::SeqCst);
        self.speech_buf.lock().unwrap().extend(resampled.clone());
    }

    pub fn can_transcribe(&self) -> bool {
        self.samples_count.load(Ordering::SeqCst) > (16000.0 / 320. * 0.6) as usize
    }

    pub fn transcribe(&self) -> Option<String> {
        let speech_buf = self.speech_buf.lock().unwrap();
        let min_samples = (1.0 * 16_000.0) as usize;
        if speech_buf.len() <= min_samples {
            println!("Less than 1s. Skipping...");
            return None;
        }
        let samples = speech_buf.to_vec();
        drop(speech_buf);

        let mut state = self.whisper_ctx.lock().unwrap();
        let mut params = FullParams::new(SamplingStrategy::default());

        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);
        params.set_print_timestamps(false);
        params.set_debug_mode(false);
        params.set_language(Some("en"));
        state.full(params, &samples).ok()?;
        let text = state.full_get_segment_text_lossy(0).unwrap();
        self.samples_count.store(0, Ordering::SeqCst);
        Some(text)
    }
}

pub fn audio_resample(
    data: &[f32],
    sample_rate0: u32,
    sample_rate: u32,
    channels: u16,
) -> Vec<f32> {
    use samplerate::{convert, ConverterType};
    convert(
        sample_rate0 as _,
        sample_rate as _,
        channels as _,
        ConverterType::SincBestQuality,
        data,
    )
    .unwrap_or_default()
}

pub fn stereo_to_mono(stereo_data: &[f32]) -> anyhow::Result<Vec<f32>> {
    let mut mono_data = Vec::with_capacity(stereo_data.len() / 2);

    for chunk in stereo_data.chunks_exact(2) {
        let average = (chunk[0] + chunk[1]) / 2.0;
        mono_data.push(average);
    }

    Ok(mono_data)
}
