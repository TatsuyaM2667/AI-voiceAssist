use anyhow::Result;
use reqwest::Client;

pub struct VoicevoxClient {
    client: Client,
    base_url: String,
}

impl VoicevoxClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
        }
    }

    pub async fn tts(&self, text: &str, speaker_id: u32) -> Result<Vec<u8>> {
        // Step 1: Create Query
        let query_url = format!("{}/audio_query?text={}&speaker={}", self.base_url, urlencoding::encode(text), speaker_id);
        let query = self.client.post(&query_url).send().await?.text().await?;

        // Step 2: Synthesis
        let synthesis_url = format!("{}/synthesis?speaker={}", self.base_url, speaker_id);
        let response = self.client.post(&synthesis_url)
            .header("Content-Type", "application/json")
            .body(query)
            .send()
            .await?
            .bytes()
            .await?;

        Ok(response.to_vec())
    }
}
