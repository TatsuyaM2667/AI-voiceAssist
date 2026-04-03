use whisper_rs::{WhisperContext, FullParams, SamplingStrategy};
use anyhow::Result;

pub struct WhisperClient {
    ctx: WhisperContext,
}

impl WhisperClient {
    pub fn new(model_path: &str) -> Result<Self> {
        let ctx = WhisperContext::new(model_path)?;
        Ok(Self { ctx })
    }

    pub fn transcribe(&self, audio_data: &[f32]) -> Result<String> {
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some("ja"));
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        let mut state = self.ctx.create_state()?;
        state.full(params, audio_data)?;

        let mut result = String::new();
        let num_segments = state.full_n_segments()?;
        for i in 0..num_segments {
            if let Ok(segment) = state.full_get_segment_text(i) {
                result.push_str(&segment);
            }
        }

        Ok(result)
    }
}
