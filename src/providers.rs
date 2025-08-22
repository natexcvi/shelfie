use anyhow::{Result, anyhow};
use dialoguer::{Input, Select, theme::ColorfulTheme};
use rig::client::ProviderClient;
use rig::client::builder::{BoxAgentBuilder, DynClientBuilder};
use rig::providers::{anthropic, ollama, openai};
use serde::{Deserialize, Serialize};
use std::env;

use crate::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Provider {
    OpenAI,
    Anthropic,
    Ollama,
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Provider::OpenAI => write!(f, "OpenAI"),
            Provider::Anthropic => write!(f, "Anthropic"),
            Provider::Ollama => write!(f, "Ollama (Local)"),
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

#[derive(Debug, Deserialize)]
struct AnthropicModel {
    name: String,
    display_name: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicModelsResponse {
    models: Vec<AnthropicModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
}

#[derive(Debug, Deserialize)]
struct OllamaModelsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Clone)]
pub struct LLMProvider {
    provider: Provider,
    model_name: String,
}

impl LLMProvider {
    pub async fn new() -> Result<Self> {
        // Try to load existing config first
        if let Some(config) = Config::load()? {
            println!(
                "Using saved configuration: {} with model {}",
                format!("{:?}", config.provider),
                config.model_name
            );

            Self::validate_ai_provider_config(&config.provider).await?;

            return Ok(Self {
                provider: config.provider,
                model_name: config.model_name,
            });
        }

        // If no config exists, prompt user and save the selection
        let providers = vec![Provider::OpenAI, Provider::Anthropic, Provider::Ollama];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select LLM Provider")
            .items(&providers)
            .interact()?;

        let provider = providers[selection].clone();
        let model_name = Self::select_model(&provider).await?;

        // Save the configuration
        let config = Config {
            provider: provider.clone(),
            model_name: model_name.clone(),
        };
        config.save()?;

        Ok(Self {
            provider,
            model_name,
        })
    }

    async fn validate_ai_provider_config(provider: &Provider) -> Result<()> {
        match provider {
            Provider::OpenAI => {
                env::var("OPENAI_API_KEY").map_err(|err| {
                    anyhow!("OPENAI_API_KEY environment variable is not set: {}", err)
                })?;
            }
            Provider::Anthropic => {
                env::var("ANTHROPIC_API_KEY").map_err(|err| {
                    anyhow!("ANTHROPIC_API_KEY environment variable is not set: {}", err)
                })?;
            }
            Provider::Ollama => {
                env::var("OLLAMA_API_BASE_URL").map_err(|err| {
                    anyhow!(
                        "OLLAMA_API_BASE_URL environment variable is not set: {}",
                        err
                    )
                })?;
            }
        }
        Ok(())
    }

    pub async fn new_interactive() -> Result<Self> {
        // Force new provider selection (ignore existing config)
        let providers = vec![Provider::OpenAI, Provider::Anthropic, Provider::Ollama];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select LLM Provider")
            .items(&providers)
            .interact()?;

        let provider = providers[selection].clone();
        let model_name = Self::select_model(&provider).await?;

        Ok(Self {
            provider,
            model_name,
        })
    }

    async fn select_model(provider: &Provider) -> Result<String> {
        let mut models = Self::list_models(provider).await?;

        if models.is_empty() {
            return Err(anyhow!("No models available for {:?}", provider));
        }

        // Ask for filter if there are many models
        if models.len() > 10 {
            let filter: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Filter models (e.g., 'gpt-5', 'claude-4', or press Enter for all)")
                .allow_empty(true)
                .interact_text()?;

            if !filter.is_empty() {
                models.retain(|model| model.to_lowercase().contains(&filter.to_lowercase()));

                if models.is_empty() {
                    return Err(anyhow!("No models match filter '{}'", filter));
                }
            }
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
            Provider::Anthropic => Self::list_anthropic_models().await,
            Provider::Ollama => Self::list_ollama_models().await,
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

    async fn list_anthropic_models() -> Result<Vec<String>> {
        let api_key =
            env::var("ANTHROPIC_API_KEY").map_err(|_| anyhow!("ANTHROPIC_API_KEY not set"))?;

        let client = reqwest::Client::new();
        let response = client
            .get("https://api.anthropic.com/v1/models")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .send()
            .await;

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    let models: AnthropicModelsResponse = resp.json().await?;
                    Ok(models.models.iter().map(|m| m.name.clone()).collect())
                } else {
                    Ok(vec![
                        "claude-4-sonnet-latest".to_string(),
                        "claude-4-haiku-latest".to_string(),
                        "claude-4-opus-latest".to_string(),
                    ])
                }
            }
            _ => Ok(vec![
                "claude-4-sonnet-latest".to_string(),
                "claude-4-haiku-latest".to_string(),
                "claude-4-opus-latest".to_string(),
            ]),
        }
    }

    async fn list_ollama_models() -> Result<Vec<String>> {
        let base_url = env::var("OLLAMA_API_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());
        let client = reqwest::Client::new();
        let response = client.get(format!("{}/api/tags", base_url)).send().await;

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    let models: OllamaModelsResponse = resp.json().await?;
                    if models.models.is_empty() {
                        Err(anyhow!(
                            "No models installed in Ollama. Run 'ollama pull <model>' first"
                        ))
                    } else {
                        Ok(models.models.iter().map(|m| m.name.clone()).collect())
                    }
                } else {
                    Err(anyhow!(
                        "Cannot connect to Ollama. Make sure it's running (ollama serve)"
                    ))
                }
            }
            _ => Err(anyhow!(
                "Cannot connect to Ollama. Make sure it's running (ollama serve)"
            )),
        }
    }

    pub fn get_openai_client(&self) -> Result<openai::Client> {
        Ok(openai::Client::from_env())
    }

    pub fn get_anthropic_client(&self) -> Result<anthropic::Client> {
        Ok(anthropic::Client::from_env())
    }

    pub fn get_agent(&self) -> Result<BoxAgentBuilder> {
        Ok(match self.get_provider() {
            Provider::OpenAI => DynClientBuilder::new().agent("openai", self.get_model_name())?,
            Provider::Anthropic => {
                DynClientBuilder::new().agent("anthropic", self.get_model_name())?
            }
            Provider::Ollama => DynClientBuilder::new().agent("ollama", self.get_model_name())?,
        })
    }

    pub fn get_ollama_client(&self) -> Result<ollama::Client> {
        Ok(ollama::Client::from_env())
    }

    pub fn get_model_name(&self) -> &str {
        &self.model_name
    }

    pub fn get_provider(&self) -> &Provider {
        &self.provider
    }
}
