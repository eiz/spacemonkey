use anyhow::bail;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct ChatResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub choices: Vec<ChatChoice>,
    pub usage: ChatUsage,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct ChatChoice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct ChatUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

pub async fn gpt_basic_data(
    openai_key: impl Into<String>,
    prompt: impl Into<String>,
    data: impl Into<String>,
) -> anyhow::Result<String> {
    let client = reqwest::Client::new();
    let query = ChatRequest {
        model: "gpt-3.5-turbo".to_owned(),
        temperature: Some(0.2),
        top_p: Some(1.0),
        messages: vec![
            ChatMessage {
                role: "system".to_owned(),
                content: prompt.into(),
            },
            ChatMessage {
                role: "user".to_owned(),
                content: data.into(),
            },
        ],
        ..Default::default()
    };

    let body = serde_json::to_string(&query)?;
    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(openai_key.into())
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await?;
    let status = response.status();
    let response_text = response.text().await?;
    if !status.is_success() {
        bail!("{}", response_text);
    }
    let result: ChatResponse = serde_json::from_str(&response_text)?;

    Ok(result.choices[0].message.content.to_owned())
}
