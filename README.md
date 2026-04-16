# sysgen

**SysGen** is a SysML v2-driven spec-to-code toolchain written in Rust. It parses your SysML v2 requirement specifications and uses a large language model to generate Rust implementation code that is automatically validated against compilation, linting, testing, and bidirectional traceability.

## Features

- **SysML v2 parsing** — reads `.sysml` files and extracts `requirement def` declarations, short names, documentation strings, and `satisfy`/`verify` relationships
- **LLM-powered code generation** — drives an agent loop (Anthropic Claude, OpenAI, Google Gemini, or Ollama) to produce idiomatic Rust that satisfies every requirement
- **Automated validation pipeline** — after each generation pass runs `cargo build`, `cargo clippy -- -D warnings`, and `cargo test`; re-prompts the model on any failure
- **Bidirectional traceability** — enforces `#[implements("...")]` and `#[verifies("...")]` annotations on every item, reporting uncovered requirements before declaring success
- **Spec file integrity** — sets `.sysml` files read-only at the OS level and verifies SHA-256 hashes during generation so the model cannot silently mutate your spec
- **Circuit breakers** — detects stuck loops (same error three times) or regressions (error count rising three times) and aborts rather than spinning forever
- **Multi-provider** — swap between Anthropic, OpenAI, Google, and Ollama via an environment variable or CLI flag

## Installation

```bash
cargo install --git https://github.com/gaarutyunov/sysgen sysgen
```

To also install the standalone traceability checker:

```bash
cargo install --git https://github.com/gaarutyunov/sysgen cargo-sysgen-trace
```

## Quick Start

```bash
# 1. Create a new project
sysgen init my-project
cd my-project

# 2. Edit the generated spec to describe your requirements
$EDITOR spec/requirements.sysml

# 3. Set your API key
export ANTHROPIC_API_KEY=sk-ant-...

# 4. Generate implementation code
sysgen gen
```

`sysgen gen` will loop until all requirements are implemented, tested, and traced — or until the maximum number of iterations is reached.

## SysML Spec Format

Requirements are written in standard SysML v2 syntax. SysGen recognises `requirement def` blocks with optional short names (`<'ID'>`) and `doc` comments:

```sysml
package VehicleRequirements {
    private import ScalarValues::*;

    requirement def <'R1'> MassRequirement {
        subject vehicle : Vehicle;
        doc /* Vehicle total mass shall not exceed massLimit */
        require constraint { vehicle.mass <= massLimit }
    }

    requirement def <'R2'> PerformanceRequirement {
        subject vehicle : Vehicle;
        doc /* Vehicle shall achieve minimum top speed */
    }

    part def Vehicle {
        attribute mass : Real;
        attribute topSpeed : Real;
        satisfy requirement : VehicleRequirements::MassRequirement;
    }

    verification def <'V1'> MassVerification {
        subject vehicle : Vehicle;
        verify requirement : VehicleRequirements::MassRequirement;
    }
}
```

## Traceability Macros

Add `sysgen-macros` to your `Cargo.toml` (done automatically by `sysgen init`):

```toml
[dependencies]
sysgen-macros = { git = "https://github.com/gaarutyunov/sysgen", package = "sysgen-macros" }
```

Annotate implementations and tests:

```rust
use sysgen_macros::{implements, verifies};

#[implements("VehicleRequirements::MassRequirement")]
pub fn check_mass(vehicle: &Vehicle) -> bool {
    vehicle.mass <= MASS_LIMIT
}

#[verifies("VehicleRequirements::MassRequirement")]
#[test]
fn test_mass_within_limit() {
    let v = Vehicle { mass: 1200.0, top_speed: 200.0 };
    assert!(check_mass(&v));
}
```

The macros emit `#[doc = "sysgen:..."]` attributes that the traceability collector parses without any runtime overhead.

## Commands

### `sysgen init [PATH]`

Scaffolds a new SysGen project (default: current directory). Creates:

```
my-project/
├── Cargo.toml          (with sysgen-macros dependency)
├── .gitignore
├── .gooseignore
├── spec/
│   └── requirements.sysml
└── src/
    └── lib.rs
```

### `sysgen gen`

Runs the code generation loop.

| Flag | Default | Description |
|------|---------|-------------|
| `--spec-dir` | `spec` | Directory containing `.sysml` files |
| `--project-root` | `.` | Directory containing `Cargo.toml` |
| `--provider` | `anthropic` | LLM provider (`anthropic`, `openai`, `google`, `ollama`) |
| `--model` | provider default | Override the model name |
| `--max-iterations` | `20` | Maximum agent loop iterations |
| `--dry-run` | — | Print the initial prompt and exit |
| `--report-output` | — | Write traceability report to a JSON file |

### `sysgen check`

Checks traceability without running code generation.

| Flag | Default | Description |
|------|---------|-------------|
| `--spec-dir` | `spec` | SysML files directory |
| `--src-dir` | `src` | Source files directory |
| `--format` | `text` | Output format (`text` or `json`) |
| `--no-fail` | — | Exit 0 even when gaps exist |

### `cargo sysgen-trace`

Standalone traceability checker (installed separately).

```bash
cargo sysgen-trace --spec-dir spec --src-dir src --format json
```

## Supported LLM Providers

| Provider | Environment variable | Default model |
|----------|---------------------|---------------|
| Anthropic | `ANTHROPIC_API_KEY` | `claude-sonnet-4-6` |
| OpenAI | `OPENAI_API_KEY` | `gpt-4o` |
| Google | `GOOGLE_API_KEY` | `gemini-2.0-flash` |
| Ollama | — | `qwen2.5-coder:32b` |

Override the provider or model globally:

```bash
export SYSGEN_PROVIDER=openai
export SYSGEN_MODEL=gpt-4-turbo
sysgen gen
```

## How It Works

```
1. Parse .sysml files → extract requirements list

2. Prompt LLM with:
   - Project structure and Cargo.toml
   - Full requirements list (ID, name, doc)
   - Rules: spec files are read-only, every item must be annotated
   - Annotation syntax examples

3. Agent loop (up to --max-iterations):
   a. LLM writes/edits Rust source files
   b. cargo build          — re-prompt on compiler errors
   c. cargo clippy -D warnings — re-prompt on lint errors
   d. cargo test           — re-prompt on test failures
   e. traceability check   — re-prompt on uncovered requirements
   f. All checks pass → SUCCESS ✓

4. Circuit breakers abort the loop early if:
   - Same error repeats 3 times (stuck)
   - Error count rises 3 times in a row (regression)
   - A spec file write is attempted (integrity violation)
```

## Project Structure

```
sysgen/
├── crates/
│   ├── sysgen/          # Main CLI binary and library
│   │   └── src/
│   │       ├── cli.rs
│   │       ├── commands/   # init, gen, check, watch
│   │       ├── agent/      # LLM agent integration and prompt building
│   │       ├── parser/     # SysML parsing via syster-base
│   │       ├── traceability/  # #[implements] / #[verifies] collector
│   │       ├── validation/ # cargo build/clippy/test runner
│   │       └── freeze/     # Spec file integrity guard
│   ├── sysgen-macros/   # Procedural macros: #[implements], #[verifies]
│   └── sysgen-trace/    # Standalone cargo-sysgen-trace binary
└── spec/
    └── example.sysml
```

## License

MIT
