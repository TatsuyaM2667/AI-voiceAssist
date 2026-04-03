use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use anyhow::Result;

pub struct AudioRecorder {
    device: cpal::Device,
    config: cpal::StreamConfig,
}

impl AudioRecorder {
    pub fn new() -> Result<Self> {
        let host = cpal::default_host();
        let device = host.default_input_device()
            .ok_or_else(|| anyhow::anyhow!("No input device found"))?;
        let config = device.default_input_config()?;
        Ok(Self { device, config: config.into() })
    }

    pub fn record(&self, duration_secs: u64) -> Result<Vec<f32>> {
        let recording_data = Arc::new(Mutex::new(Vec::new()));
        let recording_data_clone = Arc::clone(&recording_data);

        let stream = self.device.build_input_stream(
            &self.config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let mut recording = recording_data_clone.lock().unwrap();
                recording.extend_from_slice(data);
            },
            |err| eprintln!("Audio record error: {}", err),
            None,
        )?;

        stream.play()?;
        std::thread::sleep(std::time::Duration::from_secs(duration_secs));
        drop(stream);

        let result = recording_data.lock().unwrap().clone();
        Ok(result)
    }
}
