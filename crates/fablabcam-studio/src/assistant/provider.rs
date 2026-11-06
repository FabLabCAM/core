//! Explicit provider routing. A model name never selects a different endpoint.
mod ollama_division;
mod request;
mod stream;
use genai::{
    adapter::AdapterKind,
    resolver::{AuthData, Endpoint},
};
pub use request::request;
use serde::{Deserialize, Serialize};
use std::time::Duration;
pub use stream::{chat, TurnResult};

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Provider {
    #[default]
    OpenAi,
    Claude,
    Mistral,
    Ollama,
}

impl Provider {
    pub const ALL: [Self; 4] = [Self::OpenAi, Self::Claude, Self::Mistral, Self::Ollama];
    pub fn label(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
            Self::Claude => "Claude",
            Self::Mistral => "Mistral",
            Self::Ollama => "Ollama",
        }
    }
    pub fn env(self) -> &'static str {
        match self {
            Self::OpenAi => "OPENAI_API_KEY",
            Self::Claude => "ANTHROPIC_API_KEY",
            Self::Mistral => "MISTRAL_API_KEY",
            Self::Ollama => "OLLAMA_API_KEY",
        }
    }
    pub fn endpoint(self) -> &'static str {
        match self {
            Self::OpenAi => "https://api.openai.com/v1/",
            Self::Claude => "https://api.anthropic.com/v1/",
            Self::Mistral => "https://api.mistral.ai/v1/",
            Self::Ollama => "http://127.0.0.1:11434/",
        }
    }
    fn adapter(self) -> AdapterKind {
        match self {
            Self::OpenAi => AdapterKind::OpenAIResp,
            Self::Claude => AdapterKind::Anthropic,
            Self::Mistral => AdapterKind::Mistral,
            Self::Ollama => AdapterKind::Ollama,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Connection {
    pub provider: Provider,
    pub model: String,
    pub endpoint: String,
    /// Explicit declaration, confirmed by the real tool-call connection probe.
    pub tools: bool,
    pub timeout_secs: u64,
    pub max_tokens: u32,
    pub max_turns: u32,
}
impl Default for Connection {
    fn default() -> Self {
        Self::for_provider(Provider::OpenAi)
    }
}
impl Connection {
    pub fn for_provider(provider: Provider) -> Self {
        Self {
            provider,
            model: String::new(),
            endpoint: provider.endpoint().into(),
            tools: false,
            timeout_secs: 90,
            max_tokens: 4096,
            max_turns: 4,
        }
    }
    pub fn check(&self) -> Result<(), String> {
        if self.model.trim().is_empty() || self.model.len() > 200 || self.model.chars().any(char::is_control) {
            return Err("Saisissez un identifiant de modèle valide".into());
        }
        let u = reqwest::Url::parse(&self.endpoint).map_err(|_| "Adresse du fournisseur invalide")?;
        let local = matches!(u.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        if !u.username().is_empty()
            || u.password().is_some()
            || u.query().is_some()
            || u.fragment().is_some()
            || !(u.scheme() == "https" || u.scheme() == "http" && (local || self.provider == Provider::Ollama))
        {
            return Err("Adresse HTTPS requise (HTTP admis pour Ollama ou un test local), sans identifiants ni paramètres".into());
        }
        if !(5..=300).contains(&self.timeout_secs) || !(128..=16384).contains(&self.max_tokens) || !(1..=8).contains(&self.max_turns) {
            return Err("Limites : délai 5–300 s, génération 128–16384 tokens, 1–8 tours".into());
        }
        Ok(())
    }
    pub fn identity(&self) -> String {
        format!("{}|{}|{}", self.provider.label(), self.endpoint.trim_end_matches('/'), self.model)
    }
    pub fn client(&self, secret: &str) -> Result<genai::Client, String> {
        self.check()?;
        if self.provider != Provider::Ollama && secret.is_empty() {
            return Err("Clé API absente : configurez la connexion".into());
        }
        let mut headers = reqwest::header::HeaderMap::new();
        if self.provider == Provider::Ollama && !secret.is_empty() {
            let mut value = reqwest::header::HeaderValue::from_str(&format!("Bearer {secret}")).map_err(|_| "Clé API invalide")?;
            value.set_sensitive(true);
            headers.insert(reqwest::header::AUTHORIZATION, value);
        }
        let http = reqwest::Client::builder()
            .default_headers(headers)
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(self.timeout_secs))
            .no_proxy()
            .build()
            .map_err(|_| "Initialisation réseau impossible")?;
        let auth = if secret.is_empty() { AuthData::None } else { AuthData::from_single(secret) };
        let endpoint = Endpoint::from_owned(format!("{}/", self.endpoint.trim_end_matches('/')));
        genai::Client::builder()
            .with_reqwest(http)
            .with_adapter_kind(self.provider.adapter())
            .append_provider_config(self.provider.adapter(), (endpoint, auth))
            .build()
            .map_err(|_| "Initialisation du fournisseur impossible".into())
    }
}

/// Provider errors may echo request bodies or credentials. Return classified errors only.
pub fn public_error(error: &genai::Error) -> String {
    let text = error.to_string().to_lowercase();
    let message = if text.contains("401") || text.contains("403") {
        "Clé invalide ou accès refusé"
    } else if text.contains("429") {
        "Quota atteint ou trop de requêtes"
    } else if text.contains("404") {
        "Modèle ou adresse introuvable"
    } else if text.contains("timeout") || text.contains("timed out") {
        "Délai de réponse dépassé"
    } else if text.contains("400") || text.contains("422") {
        "Requête refusée : vérifier le modèle et ses capacités d'outils"
    } else if text.contains("connect") {
        "Serveur indisponible ou connexion impossible"
    } else {
        "Réponse fournisseur invalide ou connexion interrompue"
    };
    format!("{message}. Aucune nouvelle modification appliquée.")
}
