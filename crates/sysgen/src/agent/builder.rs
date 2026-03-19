use anyhow::Result;

/// Configuration for the SysGen agent.
pub struct AgentConfig {
    /// LLM provider name: "anthropic", "openai", "google", "ollama"
    pub provider: String,
    /// Model identifier, e.g. "claude-sonnet-4-20250514" or "gpt-4o"
    pub model: String,
    /// Working directory (the generated project root)
    pub working_dir: std::path::PathBuf,
    /// Session identifier used to correlate agent.reply() calls across iterations.
    /// Defaults to a unique hex timestamp string.
    pub session_id: String,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            working_dir: std::env::current_dir().unwrap_or_default(),
            session_id: format!("sysgen-{}", unique_id()),
        }
    }
}

/// Build a Goose Agent configured for SysGen code generation.
///
/// The agent is configured with the `developer` extension restricted to
/// `shell` and `text_editor` tools — sufficient for file I/O and command
/// execution without exposing unnecessary capabilities.
///
/// # Errors
///
/// Returns an error if:
/// - The required API key environment variable is not set
/// - The provider cannot be created
/// - The session cannot be initialized
/// - The developer extension cannot be added
pub async fn build_agent(config: &AgentConfig) -> Result<goose::agents::Agent> {
    use goose::agents::{Agent, ExtensionConfig};
    use goose::providers::create_with_named_model;
    use goose::session::{SessionManager, SessionType};

    assert_provider_env_var(&config.provider)?;

    let agent = Agent::new();

    let session = SessionManager::instance()
        .create_session(
            config.working_dir.clone(),
            config.session_id.clone(),
            SessionType::User,
        )
        .await?;

    // Provider reads API key from environment:
    //   Anthropic: ANTHROPIC_API_KEY
    //   OpenAI:    OPENAI_API_KEY
    //   Google:    GOOGLE_API_KEY
    //   Ollama:    no key needed (local)
    let provider = create_with_named_model(&config.provider, &config.model, vec![])
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create provider {}: {}", config.provider, e))?;

    agent
        .update_provider(provider, &session.id)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to set provider on agent: {}", e))?;

    // Add developer extension with ONLY shell and text_editor tools.
    // This gives the agent: file read/write and shell command execution.
    agent
        .add_extension(
            ExtensionConfig::Builtin {
                name: "developer".to_string(),
                description: String::new(),
                display_name: Some("Developer".to_string()),
                timeout: Some(300), // 5 minute timeout per tool call
                bundled: None,
                available_tools: vec!["shell".to_string(), "text_editor".to_string()],
            },
            &session.id,
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to add developer extension: {}", e))?;

    Ok(agent)
}

/// Validate that the required API key environment variable is set for the given provider.
///
/// Returns `Ok(())` for `ollama` (no key required) and unknown providers.
pub fn assert_provider_env_var(provider: &str) -> Result<()> {
    let key = match provider {
        "anthropic" => "ANTHROPIC_API_KEY",
        "openai" => "OPENAI_API_KEY",
        "google" => "GOOGLE_API_KEY",
        "ollama" => return Ok(()),
        other => {
            tracing::warn!("Unknown provider {other}, cannot verify API key env var");
            return Ok(());
        }
    };

    if std::env::var(key).is_err() {
        anyhow::bail!(
            "Provider {provider} requires environment variable {key} to be set.\n\
             Export it before running sysgen:\n  export {key}=your_api_key_here"
        );
    }
    Ok(())
}

/// Generate a unique session identifier using the current Unix timestamp in milliseconds.
fn unique_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("{ts:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assert_provider_env_var_ollama_no_key_needed() {
        assert!(assert_provider_env_var("ollama").is_ok());
    }

    #[test]
    fn assert_provider_env_var_unknown_provider_passes() {
        assert!(assert_provider_env_var("unknown-provider").is_ok());
    }

    #[test]
    fn assert_provider_env_var_anthropic_missing_key() {
        // Temporarily unset so the test is deterministic regardless of CI env.
        let key = "ANTHROPIC_API_KEY";
        let saved = std::env::var(key).ok();
        // SAFETY: single-threaded test; env mutation is safe here.
        unsafe { std::env::remove_var(key) };

        let result = assert_provider_env_var("anthropic");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("ANTHROPIC_API_KEY"),
            "error should mention the key name"
        );
        assert!(msg.contains("export"), "error should show export hint");

        if let Some(v) = saved {
            unsafe { std::env::set_var(key, v) };
        }
    }

    #[test]
    fn assert_provider_env_var_anthropic_with_key() {
        let key = "ANTHROPIC_API_KEY";
        let saved = std::env::var(key).ok();
        unsafe { std::env::set_var(key, "test-key") };

        assert!(assert_provider_env_var("anthropic").is_ok());

        match saved {
            Some(v) => unsafe { std::env::set_var(key, v) },
            None => unsafe { std::env::remove_var(key) },
        }
    }

    #[test]
    fn agent_config_default_uses_anthropic() {
        let cfg = AgentConfig::default();
        assert_eq!(cfg.provider, "anthropic");
        assert!(cfg.model.contains("claude"));
    }

    #[test]
    fn unique_id_is_nonempty_hex() {
        let id = unique_id();
        assert!(!id.is_empty());
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
