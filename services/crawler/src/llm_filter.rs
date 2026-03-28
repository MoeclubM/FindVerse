use anyhow::Context;
use serde::Deserialize;
use serde_json::json;

use crate::models::{LlmFilterConfig, LlmPageDecision};

pub async fn evaluate_page(
    client: &reqwest::Client,
    config: &LlmFilterConfig,
    url: &str,
    title: Option<&str>,
    snippet: Option<&str>,
    body: &str,
) -> anyhow::Result<LlmPageDecision> {
    let excerpt = body.chars().take(config.max_body_chars).collect::<String>();
    let response = client
        .post(format!(
            "{}/chat/completions",
            config.base_url.trim_end_matches('/')
        ))
        .headers(build_headers(config)?)
        .json(&json!({
            "model": config.model,
            "temperature": 0,
            "messages": [
                {
                    "role": "system",
                    "content": "You classify web pages for a general search index. Return strict JSON only with keys should_index, should_discover, relevance_score, reason. Favor useful content pages such as documentation, guides, tutorials, articles, product details, changelogs, and high-signal landing pages. Reject thin pages, faceted search pages, login walls, shopping carts, boilerplate navigation shells, spam, empty placeholders, and low-signal tag or archive pages."
                },
                {
                    "role": "user",
                    "content": format!(
                        "URL: {url}\nTitle: {}\nSnippet: {}\nBody excerpt:\n{}\n\nReturn JSON with:\n{{\"should_index\": boolean, \"should_discover\": boolean, \"relevance_score\": number, \"reason\": string}}",
                        title.unwrap_or(""),
                        snippet.unwrap_or(""),
                        excerpt
                    )
                }
            ]
        }))
        .send()
        .await
        .context("failed to call llm filter endpoint")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("llm filter returned {status}: {body}");
    }

    let payload: ChatCompletionResponse = response
        .json()
        .await
        .context("failed to decode llm filter response")?;

    let content = payload
        .choices
        .into_iter()
        .find_map(|choice| choice.message.content.to_plain_text())
        .context("llm filter returned no message content")?;
    let decision = parse_decision(&content)?;

    Ok(LlmPageDecision {
        should_index: decision.should_index && decision.relevance_score >= config.min_score,
        should_discover: decision.should_discover,
        relevance_score: decision.relevance_score.clamp(0.0, 1.0),
        reason: decision.reason.trim().chars().take(180).collect(),
    })
}

fn build_headers(config: &LlmFilterConfig) -> anyhow::Result<reqwest::header::HeaderMap> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_static("application/json"),
    );

    if let Some(api_key) = config
        .api_key
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let value = reqwest::header::HeaderValue::from_str(&format!("Bearer {api_key}"))
            .context("invalid llm api key header")?;
        headers.insert(reqwest::header::AUTHORIZATION, value);
    }

    Ok(headers)
}

fn parse_decision(content: &str) -> anyhow::Result<LlmDecisionResponse> {
    let trimmed = content.trim();
    let json_body = trimmed
        .strip_prefix("```json")
        .and_then(|value| value.strip_suffix("```"))
        .map(str::trim)
        .or_else(|| {
            trimmed
                .strip_prefix("```")
                .and_then(|value| value.strip_suffix("```"))
                .map(str::trim)
        })
        .unwrap_or(trimmed);

    serde_json::from_str(json_body).context("failed to parse llm decision json")
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: ChatContent,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ChatContent {
    Text(String),
    Parts(Vec<ChatContentPart>),
}

impl ChatContent {
    fn to_plain_text(self) -> Option<String> {
        match self {
            Self::Text(text) => Some(text),
            Self::Parts(parts) => {
                let text = parts
                    .into_iter()
                    .filter_map(|part| match part {
                        ChatContentPart::Text { text } => Some(text),
                    })
                    .collect::<String>();
                if text.trim().is_empty() {
                    None
                } else {
                    Some(text)
                }
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ChatContentPart {
    #[serde(rename = "text")]
    Text { text: String },
}

#[derive(Debug, Deserialize)]
struct LlmDecisionResponse {
    should_index: bool,
    should_discover: bool,
    relevance_score: f32,
    reason: String,
}

#[cfg(test)]
mod tests {
    use super::parse_decision;

    #[test]
    fn parses_decision_json_inside_code_fence() {
        let decision = parse_decision(
            "```json\n{\"should_index\":true,\"should_discover\":false,\"relevance_score\":0.8,\"reason\":\"high signal\"}\n```",
        )
        .expect("decision should parse");

        assert!(decision.should_index);
        assert!(!decision.should_discover);
        assert_eq!(decision.reason, "high signal");
    }
}
