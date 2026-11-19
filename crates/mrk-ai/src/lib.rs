//! Optional AI-assisted statement extraction (ADR-0006).
//!
//! AI only *proposes* rows; Rust validates them and the user approves them in staging.
//! Providers (local Ollama/LM Studio or the user's own API key) arrive in M6.

/// Where extraction runs. Local is the recommended, $0 and private option.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Provider {
    Disabled,
    LocalOpenAiCompatible { base_url: String, model: String },
    Anthropic { model: String },
    OpenAiCompatible { base_url: String, model: String },
}

impl Provider {
    /// Whether statement text would leave the user's machine.
    pub fn sends_data_off_device(&self) -> bool {
        match self {
            Provider::Disabled => false,
            Provider::LocalOpenAiCompatible { base_url, .. } => !is_loopback(base_url),
            Provider::Anthropic { .. } | Provider::OpenAiCompatible { .. } => true,
        }
    }
}

fn is_loopback(url: &str) -> bool {
    let rest = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url);
    if rest.starts_with("[::1]") {
        return true;
    }
    let host = rest.split(['/', ':']).next().unwrap_or("");
    matches!(host, "localhost" | "127.0.0.1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_providers_stay_on_device() {
        let p = Provider::LocalOpenAiCompatible {
            base_url: "http://localhost:11434/v1".into(),
            model: "qwen".into(),
        };
        assert!(!p.sends_data_off_device());
        let p = Provider::LocalOpenAiCompatible {
            base_url: "http://[::1]:11434".into(),
            model: "qwen".into(),
        };
        assert!(!p.sends_data_off_device());
        assert!(!Provider::Disabled.sends_data_off_device());
    }

    #[test]
    fn remote_providers_are_flagged() {
        let p = Provider::LocalOpenAiCompatible {
            base_url: "http://192.168.1.20:11434".into(),
            model: "qwen".into(),
        };
        assert!(p.sends_data_off_device());
        assert!(
            Provider::Anthropic {
                model: "claude-haiku-5-5".into()
            }
            .sends_data_off_device()
        );
    }
}
