# AGENTS.md - LLM-Assisted Development Guide

This document provides guidance for LLMs (AI assistants) to effectively develop, debug, and maintain the Wish System project.

## Project Overview

The Wish System is a Kubernetes operator written in Rust that allows users to express infrastructure desires in natural language. An LLM interprets these wishes and generates kubectl commands/YAML manifests for execution.

**Key Technologies:**
- Rust (kube-rs ecosystem, recent stable toolchain recommended)
- Kubernetes Custom Resource Definitions (CRDs)
- OpenAI-compatible LLM APIs
- Tokio async runtime

## Project Structure

```
wish-system/
├── src/
│   ├── lib.rs                 # Shared types, CRD definition, LLM client types
│   └── bin/
│       ├── wish-grantor.rs    # Controller: interprets wishes via LLM
│       ├── wish-fulfiller.rs  # Controller: executes approved wishes
│       └── kubectl-wish.rs    # CLI plugin for user interaction
├── k8s/                       # Kubernetes manifests
│   ├── crd.yaml              # Wish CRD definition
│   ├── rbac-*.yaml           # RBAC for controllers
│   ├── config.yaml           # ConfigMaps and Secrets
│   ├── deployments.yaml      # Controller deployments
│   └── examples.yaml         # Example wish resources
├── Cargo.toml                 # Rust dependencies
├── Dockerfile                 # Multi-stage container build
├── Makefile                   # Build automation
└── *.md                       # Documentation
```

## Core Concepts

### 1. Wish Lifecycle
```
Requested → wish-grantor → Granted → wish-fulfiller → Fulfilled
                                    ↓
                                  Failed
```

### 2. Key Data Structures

**WishSpec:**
- `wish: String` - Natural language wish
- `dry_run: bool` - Safety flag (default: true)
- `auto_fulfill: bool` - Auto-execute flag
- `llm_config: Option<LlmConfig>` - Per-wish LLM override

**WishStatus:**
- `phase: Option<WishPhase>` - Current state
- `plan: Option<ExecutionPlan>` - Generated commands
- `fulfilled: bool` - Immutable execution flag
- `error: Option<String>` - Failure reason

### 3. Security Model
- Separate ServiceAccounts for grantor (read-only) and fulfiller (write)
- Permission validation via ConfigMap
- Dry-run default prevents accidental execution
- Immutable fulfillment prevents re-execution

## Development Guidelines for LLMs

### When Adding Features

1. **Update the CRD first** (`src/lib.rs` + `k8s/crd.yaml`)
   - Add new fields to `WishSpec` or `WishStatus`
   - Update JsonSchema derives
   - Regenerate CRD YAML if needed

2. **Update controllers** (`wish-grantor.rs` or `wish-fulfiller.rs`)
   - Follow existing patterns for reconciliation loops
   - Use `kube::runtime::controller::Action` for requeue logic
   - Always update status via `patch_status()`

3. **Update kubectl plugin** (`kubectl-wish.rs`)
   - Add new subcommands to `Commands` enum
   - Implement handler in `main()` match statement
   - Maintain consistent CLI UX

4. **Update documentation**
   - README.md for user-facing features
   - ARCHITECTURE.md for system design changes
   - Add examples to `k8s/examples.yaml`

### When Fixing Bugs

1. **Identify the component:**
   - CRD schema issues → `src/lib.rs` + `k8s/crd.yaml`
   - LLM integration → `wish-grantor.rs`
   - Execution logic → `wish-fulfiller.rs`
   - CLI issues → `kubectl-wish.rs`
   - Kubernetes config → `k8s/*.yaml`

2. **Check common failure points:**
   - JSON parsing from LLM responses
   - RBAC permission errors
   - Status update conflicts
   - Async/await execution order

3. **Add error handling:**
   - Use `Result<T, anyhow::Error>` for fallible operations
   - Log errors with `tracing::error!`
   - Update wish status with error details
   - Return appropriate `Action::requeue()` durations

### Code Style Conventions

**Rust:**
- Use `async`/`await` for all I/O operations
- Prefer `anyhow::Result` for error handling
- Use `tracing::info!`, `error!`, `warn!` for logging
- Follow `rustfmt` defaults
- Use explicit type annotations for clarity

**Kubernetes:**
- Always include RBAC manifests for new permissions
- Use namespaced resources unless cluster-wide needed
- Add `additionalPrinterColumns` to CRDs for `kubectl get`
- Include resource limits in Deployments

**Error Messages:**
- Be specific: "Failed to parse LLM response as JSON: {error}"
- Include context: "While processing wish '{name}' in namespace '{ns}'"
- Suggest remediation when possible

## Common Development Tasks

### Task 1: Add New Command Type

**Files to modify:**
1. `src/lib.rs` - Add variant to `CommandType` enum
2. `wish-grantor.rs` - Update LLM prompt to mention new type
3. `wish-fulfiller.rs` - Add execution handler in `execute_plan()`

**Example:**
```rust
// src/lib.rs
pub enum CommandType {
    Kubectl,
    Shell,
    Helm,  // NEW
}

// wish-fulfiller.rs
match &cmd.command_type {
    CommandType::Kubectl => execute_kubectl(cmd, namespace).await?,
    CommandType::Shell => execute_shell(cmd).await?,
    CommandType::Helm => execute_helm(cmd, namespace).await?,  // NEW
}

async fn execute_helm(cmd: &Command, namespace: &str) -> Result<()> {
    // Implementation
}
```

### Task 2: Add Validation Webhook

**Files to create:**
1. `src/bin/wish-webhook.rs` - Webhook server
2. `k8s/webhook.yaml` - ValidatingWebhookConfiguration
3. `k8s/webhook-cert.yaml` - TLS certificate config

**Steps:**
1. Implement webhook using `kube::core::admission`
2. Add validation logic for wish spec
3. Generate TLS certificates
4. Deploy webhook alongside controllers

### Task 3: Add Metrics

**Files to modify:**
1. `Cargo.toml` - Add `prometheus` dependency
2. `wish-grantor.rs` - Add metric recording
3. `wish-fulfiller.rs` - Add metric recording
4. `k8s/deployments.yaml` - Add metrics port

**Example:**
```rust
// Add to Cargo.toml
prometheus = "0.13"

// In controller
use prometheus::{Counter, Registry};

lazy_static::lazy_static! {
    static ref WISHES_GRANTED: Counter = Counter::new(
        "wishes_granted_total",
        "Total wishes granted"
    ).unwrap();
}

// In reconcile()
WISHES_GRANTED.inc();
```

### Task 4: Support Multi-Cluster

**High-level approach:**
1. Add `targetCluster` field to WishSpec
2. Load multiple kubeconfig contexts
3. Create client for target cluster
4. Execute commands against correct cluster
5. Update RBAC for cross-cluster access

## Debugging Guide for LLMs

### Issue: Wish Stuck in Requested

**Diagnosis:**
```bash
kubectl logs -l app=wish-grantor --tail=50
```

**Common causes:**
- LLM endpoint unreachable
- Invalid JSON response from LLM
- ConfigMap not found
- Secret missing

**Fix locations:**
- `wish-grantor.rs`: `load_llm_config()`, `generate_plan()`
- `k8s/config.yaml`: Verify endpoint and credentials

### Issue: Execution Fails

**Diagnosis:**
```bash
kubectl logs -l app=wish-fulfiller --tail=50
kubectl describe wish <wish-name>
```

**Common causes:**
- RBAC permissions insufficient
- Forbidden operation in ConfigMap
- Invalid kubectl command syntax
- Namespace not allowed

**Fix locations:**
- `wish-fulfiller.rs`: `validate_command()`, `execute_kubectl()`
- `k8s/rbac-fulfiller.yaml`: Add missing permissions
- `k8s/config.yaml`: Update permission rules

### Issue: Invalid CRD

**Diagnosis:**
```bash
kubectl get crd wishes.magic.k8s.io -o yaml
```

**Common causes:**
- Schema validation errors
- Missing required fields
- Type mismatches

**Fix locations:**
- `src/lib.rs`: Update `#[schemars]` derives
- `k8s/crd.yaml`: Regenerate from code or fix manually

### Issue: Status Not Updating

**Diagnosis:**
```bash
kubectl get wishes <name> -o yaml
```

**Common causes:**
- Status subresource not enabled on CRD
- Missing RBAC for status updates
- Conflicting updates (optimistic locking)

**Fix locations:**
- `k8s/crd.yaml`: Ensure `subresources: { status: {} }`
- `k8s/rbac-*.yaml`: Add `wishes/status` permissions
- Controller code: Use `patch_status()` not `patch()`

## Testing Guidelines

### Unit Tests

Add tests to each binary:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_command() {
        let cmd = Command {
            command_type: CommandType::Kubectl,
            command: "kubectl get pods".to_string(),
            yaml: None,
        };
        let perms = PermissionConfig::default();
        assert!(validate_command(&cmd, "default", &perms).is_ok());
    }
}
```

### Integration Tests

```bash
# Start local cluster
kind create cluster

# Deploy
make deploy

# Test
kubectl wish create "create nginx pod"
sleep 5
kubectl wish list
```

### LLM Response Testing

Mock LLM responses for testing:

```rust
#[cfg(test)]
fn mock_llm_response() -> LlmResponse {
    LlmResponse {
        choices: vec![LlmChoice {
            message: LlmMessage {
                role: "assistant".to_string(),
                content: r#"{"name":"test","reasoning":"test","commands":[]}"#.to_string(),
            },
        }],
    }
}
```

## Common Pitfalls to Avoid

1. **Async/Await Issues:**
   - Always `.await` futures
   - Don't block in async functions
   - Use `tokio::spawn` for background tasks

2. **Status Updates:**
   - Use `patch_status()` not `patch()` for status
   - Handle conflicts with retry logic
   - Don't update spec from controller

3. **RBAC:**
   - Test with actual ServiceAccount, not admin
   - Grant minimum required permissions
   - Document new permissions in RBAC files

4. **LLM Integration:**
   - Validate JSON before parsing
   - Handle partial/incomplete responses
   - Set reasonable timeouts
   - Don't assume specific LLM behavior

5. **Resource Cleanup:**
   - Implement finalizers if needed
   - Handle cascading deletes properly
   - Clean up resources on failure

## AI Assistant Best Practices

### When Reading Code

1. **Start with:** `ARCHITECTURE.md` → `README.md` → `src/lib.rs`
2. **Understand the state machine:** Requested → Granted → Fulfilled
3. **Trace data flow:** User → kubectl → CRD → Grantor → LLM → Fulfiller → K8s
4. **Note security boundaries:** Separate RBAC, permission ConfigMap, dry-run default

### When Making Changes

1. **Consider backwards compatibility:** Can existing wishes still work?
2. **Update all affected components:** CRD, controllers, CLI, docs, manifests
3. **Test the happy path AND error cases**
4. **Update examples:** Add to `k8s/examples.yaml`
5. **Document breaking changes:** Note in commit message

### When Explaining Code

1. **Use concrete examples:** Show actual wish → plan → execution flow
2. **Highlight security implications:** Explain why RBAC is separate
3. **Reference architecture diagrams:** Point to ARCHITECTURE.md
4. **Provide debugging commands:** Show how to investigate issues

## Build and Deploy Checklist

Before suggesting code changes, verify:

- [ ] Code compiles: `cargo build --release`
- [ ] Tests pass: `cargo test`
- [ ] CRD validates: `kubectl apply --dry-run=client -f k8s/crd.yaml`
- [ ] Manifests are valid: `kubectl apply --dry-run=client -f k8s/`
- [ ] Documentation updated
- [ ] Examples added/updated

## Reference Commands

```bash
# Development
cargo build --release                    # Build all binaries
cargo run --bin wish-grantor            # Run controller locally
cargo test                              # Run tests
cargo fmt                               # Format code
cargo clippy                            # Lint code

# Docker
docker build --build-arg BINARY_NAME=wish-grantor -t wish-grantor:latest .
kind load docker-image wish-grantor:latest

# Kubernetes
kubectl apply -f k8s/                   # Deploy everything
kubectl get wishes                       # List wishes
kubectl describe wish <name>             # Detailed view
kubectl logs -l app=wish-grantor -f     # Controller logs
kubectl delete -f k8s/                  # Clean up

# Plugin
kubectl wish create "wish text"          # Create wish
kubectl wish list                        # List wishes
kubectl wish describe <name>             # View details
kubectl wish fulfill <name>              # Approve execution
kubectl wish delete <name>               # Delete wish
```

## Version Compatibility

- **Rust:** 1.75+ (edition 2021)
- **Kubernetes:** 1.28+
- **kube-rs:** 0.87
- **k8s-openapi:** 0.20 (v1_28 feature)

When updating dependencies, check compatibility matrix and test thoroughly.

## Contributing Workflow

1. Read `ARCHITECTURE.md` to understand design
2. Check existing issues/feature requests
3. Make changes following this guide
4. Test locally with kind/minikube
5. Update documentation
6. Create PR with clear description

## Getting Help

If stuck, check:
1. Error messages in controller logs
2. `kubectl describe` output for wishes
3. RBAC with `kubectl auth can-i`
4. LLM endpoint with `curl`
5. CRD schema with `kubectl explain wishes.spec`

## Final Notes for LLMs

This project follows standard Kubernetes operator patterns using Rust. The key insight is:
- **wish-grantor** = brain (interprets, plans)
- **wish-fulfiller** = hands (executes, acts)
- **Wish CRD** = contract (state, lifecycle)

When developing:
- Prioritize safety (dry-run, RBAC, validation)
- Maintain separation of concerns (grantor vs fulfiller)
- Document everything (code is read more than written)
- Test with real Kubernetes clusters

The LLM integration is the unique aspect - treat it as an untrusted external service that might return anything. Validate, sanitize, and verify all responses.

Good luck! 🚀
