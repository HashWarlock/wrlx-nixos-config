# CVM Agent Skills Development Guide

This guide explains how to add new automation skills to the CVM Agent. The trait-based registry architecture enables adding new actions in under 30 seconds.

## Quick Start (30 Seconds)

```bash
# 1. Copy the template
cp src/skills/actions/template.rs.example src/skills/actions/myaction.rs

# 2. Edit myaction.rs:
#    - Rename MyNewAction -> MyAction
#    - Change "my.action" -> "myservice.mymethod"
#    - Implement execute()

# 3. Register in src/skills/actions/mod.rs:
#    - Add: mod myaction;
#    - Add: pub use myaction::MyAction;
#    - In register_all(): registry.register(MyAction)?;

# 4. Build and test
cargo build
cargo test
```

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         SkillRegistry                           │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │ GitStatus   │  │ NixRebuild  │  │ YourAction  │  ...        │
│  │  Action     │  │   Action    │  │             │             │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘             │
│         │                │                │                     │
│         └────────────────┴────────────────┘                     │
│                          │                                      │
│              ┌───────────┴───────────┐                          │
│              │    SkillAction Trait  │                          │
│              │  - name() -> &str     │                          │
│              │  - execute(ctx, args) │                          │
│              └───────────────────────┘                          │
└─────────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│                      ServiceContext                              │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐                          │
│  │   DB    │  │   LLM   │  │ Config  │  (static)                │
│  │  Pool   │  │ Client  │  │::get()  │                          │
│  └─────────┘  └─────────┘  └─────────┘                          │
└─────────────────────────────────────────────────────────────────┘
```

## The SkillAction Trait

Every action implements this trait:

```rust
use crate::skills::actions::prelude::*;

pub struct MyAction;

#[async_trait]
impl SkillAction for MyAction {
    /// Unique identifier in "service.method" format
    fn name(&self) -> &'static str {
        "myservice.dowork"
    }

    /// Execute the action with service context and JSON arguments
    async fn execute(
        &self,
        ctx: &ServiceContext,
        args: JsonValue
    ) -> Result<ActionResult> {
        // Your implementation
        Ok(ActionResult::success("Done!"))
    }
}
```

## Available Services

### Configuration (Config)

Access via `Config::get()`:

```rust
let config = Config::get();

// Paths
let working_dir = &config.paths.working_dir;  // /app
let flake_ref = &config.paths.flake_ref;      // /app#phala-cvm

// Database
let db_path = &config.db.path;

// Server
let listen_addr = &config.server.listen_addr;
```

### Database (ctx.db())

SQLite connection pool for persistent storage:

```rust
let db = ctx.db();
// Use with rusqlite for queries
```

### LLM Client (ctx.llm())

Access to the Redpill/Anthropic API:

```rust
let llm = ctx.llm();
// Use for AI-powered reasoning (if configured)
```

## Action Results

Return one of these from `execute()`:

```rust
// Simple success with message
ActionResult::success("Task completed")

// Success with structured data
let mut data = HashMap::new();
data.insert("key".to_string(), serde_json::json!("value"));
ActionResult::success_with_data("Task completed", data)

// Failure with error message
ActionResult::failure("Something went wrong")
```

## Parsing Arguments

Actions receive arguments as `serde_json::Value`:

```rust
// Required argument
let name = args
    .get("name")
    .and_then(|v| v.as_str())
    .ok_or_else(|| anyhow::anyhow!("Missing: name"))?;

// Optional with default
let count = args
    .get("count")
    .and_then(|v| v.as_i64())
    .unwrap_or(10);

// Array argument
let paths: Vec<&str> = args
    .get("paths")
    .and_then(|p| p.as_array())
    .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
    .unwrap_or_default();
```

## Running Shell Commands

Use `tokio::process::Command` for async execution:

```rust
let output = tokio::process::Command::new("git")
    .args(["status", "--porcelain"])
    .current_dir(&Config::get().paths.working_dir)
    .output()
    .await?;

if output.status.success() {
    Ok(ActionResult::success(
        String::from_utf8_lossy(&output.stdout).to_string()
    ))
} else {
    Ok(ActionResult::failure(
        String::from_utf8_lossy(&output.stderr).to_string()
    ))
}
```

## Testing Actions

Include tests in your action file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_memory_db;
    use crate::llm::RedpillClient;
    use std::sync::Arc;

    fn test_context() -> ServiceContext {
        let db = init_memory_db().expect("test db");
        let llm = Arc::new(RedpillClient::new_dummy());
        ServiceContext::new(db, llm)
    }

    #[tokio::test]
    async fn test_action() {
        let action = MyAction;
        let ctx = test_context();

        let result = action
            .execute(&ctx, serde_json::json!({"key": "value"}))
            .await
            .unwrap();

        assert!(result.success);
    }
}
```

Run tests:
```bash
cargo test myaction
```

## Using Actions in Workflows

Actions are called from YAML workflow files:

```yaml
name: my-workflow
description: Example workflow
triggers:
  - deploy
  - update

steps:
  - id: check_status
    action: git.status
    description: Check for changes

  - id: do_work
    action: myservice.dowork
    args:
      name: "{{ parameters.name }}"
      count: 5
    condition: steps.check_status.has_changes

  - id: notify
    action: chat.respond
    message: "Completed: {{ steps.do_work.output }}"
```

## Built-in Actions Reference

| Action | Description | Arguments |
|--------|-------------|-----------|
| `git.status` | Check git status | none |
| `git.add` | Stage files | `paths`: array of paths |
| `git.commit` | Create commit | `message`: commit message |
| `git.push` | Push to remote | none |
| `nixops.rebuild` | Rebuild NixOS | `action`: switch/boot/test |
| `nixops.rollback` | Rollback generation | `generation`: optional |
| `nixops.list_generations` | List generations | none |
| `prompt.confirm` | Auto-confirm | none |
| `chat.respond` | Output message | `message`: text |

## File Structure

```
src/skills/
├── actions/
│   ├── mod.rs              # Module exports and register_all()
│   ├── prelude.rs          # Common imports for actions
│   ├── template.rs.example # Copy this to create new actions
│   ├── git.rs              # Git operations
│   ├── nixops.rs           # NixOS operations
│   └── prompt.rs           # Prompt/chat actions
├── executor.rs             # Workflow execution engine
├── loader.rs               # YAML skill file loader
├── matcher.rs              # Trigger matching
├── registry.rs             # SkillAction trait & registry
└── types.rs                # Skill type definitions
```

## Naming Conventions

- **Action names**: `service.method` format (e.g., `docker.build`, `k8s.deploy`)
- **Struct names**: PascalCase with `Action` suffix (e.g., `DockerBuildAction`)
- **File names**: lowercase, matching service (e.g., `docker.rs`)

## Best Practices

1. **Fail fast**: Validate arguments early, return clear error messages
2. **Be idempotent**: Actions should be safe to retry
3. **Log sparingly**: Use `tracing::info!` for important events only
4. **Return data**: Prefer `success_with_data()` to enable workflow chaining
5. **Handle timeouts**: Set appropriate timeouts for external commands
6. **Document thoroughly**: Include usage examples in doc comments
