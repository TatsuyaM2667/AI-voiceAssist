use vosk::{Model, Recognizer};
use anyhow::Result;
use serde_json::Value;

pub struct VoskClient {
    model: Model,
}

impl VoskClient {
    pub fn new(model_path: &str) -> Result<Self> {
        let model = Model::new(model_path).ok_or_else(|| anyhow::anyhow!("Failed to load Vosk model"))?;
        Ok(Self { model })
    }

    pub fn transcribe(&self, audio_data: &[f32], sample_rate: f32) -> Result<String> {
        let mut recognizer = Recognizer::new(&self.model, sample_rate)
            .ok_or_else(|| anyhow::anyhow!("Failed to create recognizer"))?;

        // Convert f32 to i16 (PCM)
        let pcm_data: Vec<i16> = audio_data.iter()
            .map(|&x| (x * 32767.0) as i16)
            .collect();

        let _ = recognizer.accept_waveform(&pcm_data);
        let result = recognizer.final_result().single().ok_or_else(|| anyhow::anyhow!("No transcription result"))?;
        
        // Parse JSON result from Vosk
        let v: Value = serde_json::from_str(result.text)?;
        let text = v["text"].as_str().unwrap_or("").to_string();
        
        Ok(text)
    }
}
