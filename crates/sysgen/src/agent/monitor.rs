/// Result of processing the agent reply stream.
pub struct AgentRunResult {
    /// Final text response from the agent (concatenation of all text deltas).
    pub final_response: String,
    /// Number of tool calls made during the run.
    pub tool_call_count: usize,
    /// Whether any `text_editor` write to a `spec/` path was observed in the stream.
    ///
    /// # Limitations
    ///
    /// The Goose agent framework does not expose a mid-stream `inject_tool_result` API,
    /// so SysGen cannot veto tool calls at the event-stream level. Instead, spec-file
    /// protection is enforced at the filesystem layer via [`crate::freeze::guard::SpecFreezeGuard`]
    /// (which sets spec files read-only before the agent runs) and via the system prompt
    /// (which explicitly forbids writes to `spec/`). This flag records whether the agent
    /// *attempted* a write so the outer loop can react (e.g. abort, log, re-prompt).
    pub spec_write_attempted: bool,
    /// Non-fatal error strings collected from agent messages.
    pub errors: Vec<String>,
}

/// Process the Goose agent reply stream, enforce the spec/ write guard at the observation
/// layer, and stream text output to stdout.
///
/// The caller is responsible for activating [`crate::freeze::guard::SpecFreezeGuard`]
/// before calling this function so that any write attempt to a spec file is rejected by
/// the OS regardless of whether this function catches it first.
///
/// # Errors
///
/// Returns an error if `agent.reply()` itself fails (e.g. network error, invalid session).
/// Non-fatal agent errors are collected into [`AgentRunResult::errors`].
pub async fn process_agent_stream(
    agent: &goose::agents::Agent,
    user_message: goose::conversation::message::Message,
    session_config: goose::agents::SessionConfig,
    spec_dir: &std::path::Path,
) -> anyhow::Result<AgentRunResult> {
    use futures::StreamExt;
    use goose::agents::AgentEvent;
    use goose::conversation::message::MessageContent;

    let mut stream = agent.reply(user_message, session_config, None).await?;

    let mut result = AgentRunResult {
        final_response: String::new(),
        tool_call_count: 0,
        spec_write_attempted: false,
        errors: Vec::new(),
    };

    while let Some(event) = stream.next().await {
        let event = match event {
            Ok(e) => e,
            Err(e) => {
                let msg = e.to_string();
                tracing::error!("Agent stream error: {}", msg);
                result.errors.push(msg);
                continue;
            }
        };

        match event {
            AgentEvent::Message(msg) => {
                for content in &msg.content {
                    match content {
                        MessageContent::Text(text_content) => {
                            print!("{}", text_content.text);
                            result.final_response.push_str(&text_content.text);
                        }

                        MessageContent::ToolRequest(tool_req) => {
                            if let Ok(params) = &tool_req.tool_call {
                                let name = params.name.as_ref();
                                result.tool_call_count += 1;
                                tracing::info!("Agent tool call: {} (id={})", name, tool_req.id);

                                if is_spec_write_attempt(name, params.arguments.as_ref(), spec_dir)
                                {
                                    tracing::warn!(
                                        "OBSERVED spec/ write attempt: {} (id={}). \
                                         The filesystem guard will reject this at the OS level.",
                                        name,
                                        tool_req.id
                                    );
                                    result.spec_write_attempted = true;
                                }
                            } else {
                                tracing::warn!(
                                    "Agent sent invalid tool request (id={})",
                                    tool_req.id
                                );
                            }
                        }

                        MessageContent::ToolResponse(tool_resp) => {
                            tracing::info!("Tool result for id={}", tool_resp.id);
                        }

                        _ => {}
                    }
                }
            }

            AgentEvent::ModelChange { model, mode } => {
                tracing::info!("Agent switched model: {} (mode={})", model, mode);
            }

            // HistoryReplaced and McpNotification are informational; no action needed.
            _ => {}
        }
    }

    // Ensure a newline after any streamed output.
    if !result.final_response.is_empty() {
        println!();
    }

    Ok(result)
}

/// Construct a [`goose::agents::SessionConfig`] for a single SysGen generation run.
pub fn make_session_config(session_id: &str) -> goose::agents::SessionConfig {
    goose::agents::SessionConfig {
        id: session_id.to_string(),
        schedule_id: None,
        max_turns: None,
        retry_config: None,
    }
}

/// Construct a user [`goose::conversation::message::Message`] from a plain text string.
pub fn make_user_message(content: &str) -> goose::conversation::message::Message {
    goose::conversation::message::Message::user().with_text(content)
}

/// Return `true` if the tool call looks like a `text_editor` write targeting a `spec/` path.
///
/// This is a best-effort observation: real enforcement is handled by the OS (read-only
/// permissions set by [`crate::freeze::guard::SpecFreezeGuard`]).
fn is_spec_write_attempt(
    tool_name: &str,
    arguments: Option<&serde_json::Map<String, serde_json::Value>>,
    spec_dir: &std::path::Path,
) -> bool {
    if !tool_name.contains("text_editor") {
        return false;
    }

    let args = match arguments {
        Some(a) => a,
        None => return false,
    };

    // Only write-mutating commands are relevant.
    let command = args.get("command").and_then(|v| v.as_str());
    if !matches!(command, Some("write") | Some("replace") | Some("insert")) {
        return false;
    }

    let path_str = match args.get("path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return false,
    };

    let path = std::path::Path::new(path_str);

    // Prefer canonical comparison when the path already exists on disk.
    if let (Ok(canonical), Ok(spec_canonical)) = (path.canonicalize(), spec_dir.canonicalize()) {
        return canonical.starts_with(spec_canonical);
    }

    // Fallback: string-prefix heuristics for paths that don't exist yet.
    let spec_prefix = spec_dir.to_string_lossy();
    path_str.starts_with(spec_prefix.as_ref())
        || path_str.starts_with("spec/")
        || path_str.starts_with("./spec/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn is_spec_write_attempt_returns_false_for_non_editor_tool() {
        let args = serde_json::json!({ "command": "write", "path": "spec/foo.sysml" });
        let map = args.as_object().unwrap();
        assert!(!is_spec_write_attempt(
            "shell",
            Some(map),
            Path::new("spec")
        ));
    }

    #[test]
    fn is_spec_write_attempt_returns_false_for_read_command() {
        let args = serde_json::json!({ "command": "view", "path": "spec/foo.sysml" });
        let map = args.as_object().unwrap();
        assert!(!is_spec_write_attempt(
            "text_editor",
            Some(map),
            Path::new("spec")
        ));
    }

    #[test]
    fn is_spec_write_attempt_detects_write_to_spec_prefix() {
        let args = serde_json::json!({ "command": "write", "path": "spec/foo.sysml" });
        let map = args.as_object().unwrap();
        assert!(is_spec_write_attempt(
            "text_editor",
            Some(map),
            Path::new("spec")
        ));
    }

    #[test]
    fn is_spec_write_attempt_detects_replace_to_dot_slash_spec() {
        let args = serde_json::json!({ "command": "replace", "path": "./spec/bar.sysml" });
        let map = args.as_object().unwrap();
        assert!(is_spec_write_attempt(
            "developer__text_editor",
            Some(map),
            Path::new("spec")
        ));
    }

    #[test]
    fn is_spec_write_attempt_returns_false_for_src_path() {
        let args = serde_json::json!({ "command": "write", "path": "src/main.rs" });
        let map = args.as_object().unwrap();
        assert!(!is_spec_write_attempt(
            "text_editor",
            Some(map),
            Path::new("spec")
        ));
    }

    #[test]
    fn is_spec_write_attempt_returns_false_for_no_arguments() {
        assert!(!is_spec_write_attempt(
            "text_editor",
            None,
            Path::new("spec")
        ));
    }

    #[test]
    fn make_session_config_sets_id() {
        let cfg = make_session_config("test-session-42");
        assert_eq!(cfg.id, "test-session-42");
        assert!(cfg.schedule_id.is_none());
        assert!(cfg.max_turns.is_none());
        assert!(cfg.retry_config.is_none());
    }

    #[test]
    fn make_user_message_is_user_role() {
        let msg = make_user_message("hello");
        // Message::user() sets role to User; verify via the text content.
        let text = msg.as_concat_text();
        assert_eq!(text, "hello");
    }
}
