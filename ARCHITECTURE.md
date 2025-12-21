# Wish System Architecture Overview

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         User Interaction                         │
│                                                                   │
│  kubectl wish create "deploy nginx"  ←→  kubectl CLI Plugin     │
└────────────────────────────┬────────────────────────────────────┘
                             │
                             ↓
┌─────────────────────────────────────────────────────────────────┐
│                    Kubernetes API Server                         │
│                                                                   │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  Wish CRD (magic.k8s.io/v1alpha1)                        │  │
│  │                                                            │  │
│  │  apiVersion: magic.k8s.io/v1alpha1                       │  │
│  │  kind: Wish                                               │  │
│  │  spec:                                                     │  │
│  │    wish: "deploy nginx"                                   │  │
│  │    dryRun: true                                           │  │
│  │    autoFulfill: false                                     │  │
│  │  status:                                                   │  │
│  │    phase: Requested → Granted → Fulfilled                │  │
│  │    plan: { commands: [...], reasoning: "..." }           │  │
│  └──────────────────────────────────────────────────────────┘  │
└────────────┬────────────────────────────────┬──────────────────┘
             │                                 │
             │ Watch                           │ Watch
             │ (phase=Requested)               │ (phase=Granted)
             ↓                                 ↓
┌──────────────────────────┐    ┌─────────────────────────────────┐
│   wish-grantor           │    │    wish-fulfiller               │
│   Controller             │    │    Controller                   │
│                          │    │                                 │
│  1. Watch for Requested  │    │  1. Watch for Granted wishes    │
│  2. Load LLM config      │    │  2. Check permissions           │
│  3. Call LLM API ────────┼───→│  3. Execute kubectl/shell       │
│  4. Parse response       │    │  4. Update status=Fulfilled     │
│  5. Update status=Granted│    │                                 │
│                          │    │  ServiceAccount:                │
│  ServiceAccount:         │    │  - Elevated permissions         │
│  - Read-only k8s API     │    │  - Create/update resources      │
│  - Read ConfigMaps       │    │  - Subject to permission rules  │
│  - Update Wish status    │    │                                 │
└──────────┬───────────────┘    └─────────────────────────────────┘
           │
           │ HTTP POST
           ↓
┌──────────────────────────────────────────────────────────────────┐
│                    LLM Endpoint                                   │
│                                                                    │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  POST /v1/chat/completions                                 │ │
│  │  {                                                          │ │
│  │    "model": "llama3.2:latest",                            │ │
│  │    "messages": [                                           │ │
│  │      {"role": "system", "content": "Kubernetes expert"},  │ │
│  │      {"role": "user", "content": "deploy nginx"}          │ │
│  │    ]                                                        │ │
│  │  }                                                          │ │
│  │                                                             │ │
│  │  Response:                                                  │ │
│  │  {                                                          │ │
│  │    "name": "nginx-deployment",                            │ │
│  │    "reasoning": "Creating Deployment...",                 │ │
│  │    "commands": [...]                                       │ │
│  │  }                                                          │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                    │
│  Options:                                                          │
│  - Ollama (http://localhost:11434)                               │
│  - OpenAI API                                                      │
│  - Any OpenAI-compatible endpoint                                 │
└────────────────────────────────────────────────────────────────────┘
```

## Component Details

### 1. Wish CRD
- **Purpose:** Define the schema for wishes
- **Location:** `k8s/crd.yaml`
- **Key Fields:**
  - `spec.wish`: Natural language request
  - `spec.dryRun`: Safety flag (default: true)
  - `spec.autoFulfill`: Automatic execution flag
  - `status.phase`: Lifecycle state
  - `status.plan`: Generated execution plan

### 2. wish-grantor Controller
- **Language:** Rust
- **Location:** `src/bin/wish-grantor.rs`
- **Responsibilities:**
  - Watch for wishes in Requested phase
  - Load LLM configuration from ConfigMap or wish spec
  - Call LLM API with wish text + k8s context
  - Parse LLM response (JSON)
  - Update wish status to Granted with execution plan
- **RBAC:** Read-only access to k8s API for schema inspection
- **Configuration:** ConfigMap `wish-grantor-config`

### 3. wish-fulfiller Controller
- **Language:** Rust
- **Location:** `src/bin/wish-fulfiller.rs`
- **Responsibilities:**
  - Watch for wishes in Granted phase with dryRun=false
  - Load permission rules from ConfigMap
  - Validate commands against permission rules
  - Execute kubectl commands or shell scripts
  - Update wish status to Fulfilled or Failed
- **RBAC:** Elevated permissions for resource manipulation
- **Configuration:** ConfigMap `wish-fulfiller-permissions`

### 4. kubectl-wish Plugin
- **Language:** Rust
- **Location:** `src/bin/kubectl-wish.rs`
- **Commands:**
  - `create`: Create new wish
  - `list`: List all wishes
  - `describe`: Show wish details and plan
  - `fulfill`: Mark wish for execution
  - `delete`: Delete wish
- **Installation:** Copy to `/usr/local/bin/kubectl-wish`

## Data Flow

### Wish Creation Flow
```
User → kubectl wish create
  ↓
kubectl plugin creates Wish resource
  ↓
K8s API Server stores Wish (phase=Requested)
  ↓
wish-grantor detects new Wish
  ↓
Load LLM config (ConfigMap or env)
  ↓
Call LLM API with wish text
  ↓
Parse JSON response
  ↓
Update Wish status (phase=Granted, plan={...})
  ↓
User reviews with kubectl wish describe
  ↓
User approves with kubectl wish fulfill
  ↓
Wish spec updated (dryRun=false)
  ↓
wish-fulfiller detects change
  ↓
Load permission config
  ↓
Validate each command
  ↓
Execute kubectl/shell commands
  ↓
Update Wish status (phase=Fulfilled, fulfilled=true)
```

### Auto-Fulfill Flow
```
User → kubectl wish create --auto-fulfill --no-dry-run
  ↓
Wish created with autoFulfill=true, dryRun=false
  ↓
wish-grantor processes → status=Granted
  ↓
wish-fulfiller immediately detects and executes
  ↓
status=Fulfilled (no manual approval needed)
```

## Security Model

### Multi-Layer Protection

1. **Dry-Run Default**
   - All wishes start with dryRun=true
   - Requires explicit fulfill action
   - Prevents accidental execution

2. **Separate RBAC**
   - wish-grantor: Read-only permissions
   - wish-fulfiller: Controlled write permissions
   - Different service accounts

3. **Permission ConfigMap**
   ```yaml
   allowedNamespaces: "default,staging"
   allowedResources: "pods,deployments,services"
   forbiddenOperations: "delete:namespaces"
   ```

4. **Immutable Fulfillment**
   - `status.fulfilled` can only be set once
   - Prevents re-execution
   - Audit trail preserved

5. **LLM Validation**
   - JSON schema validation
   - Command sanitization
   - Operation validation against rules

## Configuration Management

### wish-grantor Configuration
```yaml
# ConfigMap: wish-grantor-config
llmEndpoint: "http://localhost:11434/v1"
llmModel: "llama3.2:latest"
credentialsSecretName: "llm-credentials"  # Optional
credentialsSecretKey: "apiKey"
```

### wish-fulfiller Configuration
```yaml
# ConfigMap: wish-fulfiller-permissions
allowedNamespaces: "default,staging,production"
allowedResources: "pods,deployments,services,configmaps,secrets"
forbiddenOperations: "delete:namespaces,delete:persistentvolumes"
```

### Per-Wish Override
```yaml
apiVersion: magic.k8s.io/v1alpha1
kind: Wish
spec:
  wish: "deploy app"
  llmConfig:
    endpoint: "http://custom-llm:8080/v1"
    model: "custom-model"
    credentialsSecretRef:
      name: "custom-secret"
      key: "apiKey"
```

## Extensibility Points

### 1. Custom LLM Providers
- Any OpenAI-compatible API
- Local models (Ollama, LM Studio)
- Cloud providers (OpenAI, Anthropic, etc.)

### 2. Command Types
Currently: `kubectl`, `shell`
Easy to add: `helm`, `terraform`, `ansible`

### 3. Validation Hooks
- Add webhook for wish validation
- Custom admission controllers
- Policy engines (OPA)

### 4. Status Reporting
- Add custom metrics
- Integration with observability platforms
- Notification systems

## Deployment Patterns

### Development
```
Local machine:
- Controllers run via cargo run
- kubectl plugin installed locally
- Local Ollama instance
```

### Staging
```
Kubernetes cluster:
- Controllers as Deployments (1 replica each)
- Shared LLM endpoint
- Relaxed permissions for testing
```

### Production
```
Kubernetes cluster:
- Controllers as Deployments (multiple replicas)
- Leader election for wish-fulfiller
- Strict RBAC and permissions
- Dedicated LLM instance or API
- Full observability stack
- Audit logging
```

## Future Enhancements

1. **Multi-LLM Support**
   - Route to different models based on complexity
   - Fallback mechanisms

2. **Approval Workflows**
   - Integration with approval systems
   - Multi-stage approvals

3. **Rollback Support**
   - Store undo commands
   - Automatic rollback on failure

4. **Cost Tracking**
   - Track LLM API costs per wish
   - Resource usage monitoring

5. **Learning System**
   - Store successful wishes as examples
   - Fine-tune local models

6. **Multi-Cluster**
   - Execute wishes across clusters
   - Cluster affinity rules

## Technology Choices

### Why Rust?
- **Performance:** Fast execution, low memory footprint
- **Safety:** Memory safety prevents common bugs
- **Ecosystem:** Excellent k8s support (kube-rs)
- **Async:** First-class async/await support

### Why Separate Controllers?
- **Security:** Different permission levels
- **Scalability:** Independent scaling
- **Reliability:** Failure isolation
- **Clarity:** Single responsibility principle

### Why OpenAI-Compatible API?
- **Flexibility:** Works with any provider
- **Local-First:** Support for Ollama, LM Studio
- **Standard:** Well-documented, widely adopted
- **Future-Proof:** Easy to switch providers

## File Structure
```
wish-system/
├── Cargo.toml                 # Rust dependencies
├── Makefile                   # Build automation
├── Dockerfile                 # Container image
├── build-images.sh            # Image build script
├── README.md                  # Main documentation
├── QUICKSTART.md              # Quick start guide
├── TESTING.md                 # Testing guide
├── ARCHITECTURE.md            # This file
├── src/
│   ├── lib.rs                 # Shared types & CRD
│   └── bin/
│       ├── wish-grantor.rs    # Grantor controller
│       ├── wish-fulfiller.rs  # Fulfiller controller
│       └── kubectl-wish.rs    # CLI plugin
└── k8s/
    ├── crd.yaml               # CRD definition
    ├── rbac-grantor.yaml      # Grantor RBAC
    ├── rbac-fulfiller.yaml    # Fulfiller RBAC
    ├── config.yaml            # ConfigMaps & Secrets
    ├── deployments.yaml       # Controller deployments
    └── examples.yaml          # Example wishes
```
