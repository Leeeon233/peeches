// https://github.com/thewh1teagle/vad-rs/blob/main/examples/whisper/src/main.rs

use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

pub struct Whisper {
    // vad: Arc<Mutex<Vad>>,
    whisper_ctx: WhisperState,
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
            whisper_ctx: state,
            // params: Arc::new(Mutex::new(params)),
            // normalizer: Arc::new(Mutex::new(normalizer)),
        }
    }

    pub fn transcribe(&mut self, samples: Vec<f32>) -> anyhow::Result<String> {
        let mut params = FullParams::new(SamplingStrategy::default());
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);
        params.set_print_timestamps(false);
        params.set_debug_mode(false);
        params.set_language(Some("en"));
        params.set_duration_ms(3000);
        params.set_logprob_thold(-2.0);
        params.set_temperature(0.2);
        params.set_suppress_non_speech_tokens(true);
        self.whisper_ctx.full(params, &samples)?;
        let text = self.whisper_ctx.full_get_segment_text_lossy(0)?;
        Ok(text)
    }
}
