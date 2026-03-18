/// Stub implementation of the goose agent framework.
/// Mirrors the block/goose public API for local compilation.
/// The real implementation is pulled via git dependency (patched here for development).

// Top-level re-exports for backward compatibility with existing code.
pub use agents::agent::Agent;
pub use agents::agent::AgentBuilder;

pub mod agents {
    pub mod agent {
        use std::sync::Arc;

        use crate::agents::extension::ExtensionConfig;
        use crate::providers::factory::StubProvider;
        use crate::session::SessionManager;

        /// The main Goose agent struct.
        pub struct Agent;

        impl Agent {
            pub fn new(_provider: Arc<StubProvider>, _session: Arc<SessionManager>) -> Self {
                Agent
            }

            pub async fn add_extension(&self, _config: ExtensionConfig) -> anyhow::Result<()> {
                Ok(())
            }
        }

        /// Builder pattern for Agent construction (legacy API).
        #[derive(Default)]
        pub struct AgentBuilder;

        impl AgentBuilder {
            pub fn new() -> Self {
                AgentBuilder
            }

            pub fn build(self) -> Agent {
                Agent
            }
        }
    }

    pub mod extension {
        /// Extension (tool) configuration for the Goose agent.
        pub enum ExtensionConfig {
            Builtin {
                name: String,
                display_name: Option<String>,
                timeout: Option<u64>,
                available_tools: Option<Vec<String>>,
                bundled: Option<bool>,
            },
        }
    }
}

pub mod providers {
    pub mod factory {
        /// Stub provider — replaced by the real LLM client at runtime.
        pub struct StubProvider;

        /// Create a provider by name and model string.
        ///
        /// The provider reads its API key from the environment:
        /// - Anthropic: `ANTHROPIC_API_KEY`
        /// - OpenAI:    `OPENAI_API_KEY`
        /// - Google:    `GOOGLE_API_KEY`
        /// - Ollama:    no key required (local)
        pub fn create_provider(_provider: &str, _model: &str) -> anyhow::Result<StubProvider> {
            Ok(StubProvider)
        }
    }
}

pub mod session {
    /// Manages conversation history (SQLite-backed in real implementation).
    pub struct SessionManager;

    impl SessionManager {
        pub fn new() -> anyhow::Result<Self> {
            Ok(SessionManager)
        }
    }

    impl Default for SessionManager {
        fn default() -> Self {
            SessionManager
        }
    }
}

pub mod message {
    /// A conversation message with role and content parts.
    pub struct Message;
}
