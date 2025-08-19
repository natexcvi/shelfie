use anyhow::{anyhow, Result};
use dialoguer::{theme::ColorfulTheme, Select};
use rig::client::ProviderClient;
use rig::providers::openai;
use serde::Deserialize;
use std::env;

#[derive(Debug, Clone)]
pub enum Provider {
    OpenAI,
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Provider::OpenAI => write!(f, "OpenAI"),
        }
    }
}

#[derive(Debug, Deserialize)]
struct OpenAIModel {
    id: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIModelsResponse {
    data: Vec<OpenAIModel>,
}

pub struct LLMProvider {
    provider: Provider,
    model_name: String,
}

impl LLMProvider {
    pub async fn new() -> Result<Self> {
        let provider = Provider::OpenAI;
        let model_name = Self::select_model(&provider).await?;

        Ok(Self {
            provider,
            model_name,
        })
    }

    async fn select_model(provider: &Provider) -> Result<String> {
        let models = Self::list_models(provider).await?;

        if models.is_empty() {
            return Err(anyhow!("No models available for {:?}", provider));
        }

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select Model")
            .items(&models)
            .interact()?;

        Ok(models[selection].clone())
    }

    async fn list_models(provider: &Provider) -> Result<Vec<String>> {
        match provider {
            Provider::OpenAI => Self::list_openai_models().await,
        }
    }

    async fn list_openai_models() -> Result<Vec<String>> {
        let api_key = env::var("OPENAI_API_KEY").map_err(|_| anyhow!("OPENAI_API_KEY not set"))?;

        let client = reqwest::Client::new();
        let response = client
            .get("https://api.openai.com/v1/models")
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await?;

        let models: OpenAIModelsResponse = response.json().await?;

        let mut model_names: Vec<String> = models
            .data
            .iter()
            .filter(|m| m.id.contains("gpt"))
            .map(|m| m.id.clone())
            .collect();

        model_names.sort();
        model_names.dedup();

        if model_names.is_empty() {
            model_names = vec!["gpt-5".to_string(), "gpt-5-mini".to_string()];
        }

        Ok(model_names)
    }

    pub fn get_openai_client(&self) -> Result<openai::Client> {
        Ok(openai::Client::from_env())
    }

    pub fn get_model_name(&self) -> &str {
        &self.model_name
    }

    pub fn get_provider(&self) -> &Provider {
        &self.provider
    }
}
