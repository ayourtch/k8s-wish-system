# Wish System for Kubernetes

A Kubernetes operator that allows you to express your infrastructure desires in natural language and have them automatically translated into kubectl commands and YAML manifests.

## Overview

The Wish System consists of three components:

1. **Wish CRD**: Custom resource definition for wishes
2. **wish-grantor**: Controller that interprets wishes using an LLM
3. **wish-fulfiller**: Controller that executes granted wishes
4. **kubectl-wish**: CLI plugin for managing wishes

## Architecture

```
User creates Wish
    ↓
wish-grantor reads wish
    ↓
Calls LLM with wish + k8s schema
    ↓
Updates Wish status to "Granted" with execution plan
    ↓
User reviews and approves (kubectl wish fulfill)
    ↓
wish-fulfiller executes plan
    ↓
Updates Wish status to "Fulfilled"
```

## Features

- **Natural Language Interface**: Express infrastructure needs in plain English
- **Dry-run by Default**: All wishes start in dry-run mode for safety
- **LLM Integration**: Uses OpenAI-compatible API (supports local models like Ollama)
- **RBAC Protection**: Separate permissions for planning vs execution
- **Permission Controls**: Fine-grained control over allowed operations
- **Kubectl Plugin**: Convenient CLI for wish management
- **Namespace Separation**: Controllers in wish-system, resources in target namespace

## Quick Start

Get started in under 5 minutes with our interactive installer:

```bash
# Clone the repository
git clone https://github.com/yourusername/wish-system
cd wish-system

# Run the interactive installer
./scripts/install.sh

# Create your first wish
./target/release/kubectl-wish create "Deploy nginx with 3 replicas"
```

The installer will:
- ✓ Check prerequisites
- ✓ Set up your Kubernetes cluster
- ✓ Auto-detect local LLM services (Ollama, LM Studio)
- ✓ Guide you through configuration

See [QUICKSTART.md](QUICKSTART.md) for detailed quick start guide.

## Installation

### Interactive Installation (Recommended)

```bash
./scripts/install.sh
```

This is the easiest way to get started. The script will guide you through:
- Cluster detection or creation
- Installation method selection
- LLM configuration with auto-detection
- Verification

### Manual Installation

#### Prerequisites

- Kubernetes cluster (1.28+)
- kubectl
- Docker (for building images)
- Recent stable Rust toolchain (optional, for building from source)

#### Build

```bash
# Build Rust binaries
cargo build --release

# Build Docker images
./build-images.sh

# Load into kind cluster (if using kind)
kind load docker-image wish-grantor:latest
kind load docker-image wish-fulfiller:latest
```

### Deploy to Kubernetes

```bash
# Install CRD
kubectl apply -f k8s/crd.yaml

# Create RBAC
kubectl apply -f k8s/rbac-grantor.yaml
kubectl apply -f k8s/rbac-fulfiller.yaml

# Create ConfigMaps
kubectl apply -f k8s/config.yaml

# Deploy controllers
kubectl apply -f k8s/deployments.yaml
```

### Install kubectl plugin

```bash
# Copy to PATH
sudo cp target/release/kubectl-wish /usr/local/bin/

# Verify
kubectl wish --help
```

## Configuration

### LLM Configuration

Edit `k8s/config.yaml` to configure your LLM endpoint:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: wish-grantor-config
data:
  llmEndpoint: "http://localhost:11434/v1"
  llmModel: "llama3.2:latest"
```

For authenticated endpoints, create a secret:

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: llm-credentials
stringData:
  apiKey: "your-api-key"
```

Then reference it in the ConfigMap:

```yaml
data:
  llmEndpoint: "https://api.openai.com/v1"
  llmModel: "gpt-4"
  credentialsSecretName: "llm-credentials"
  credentialsSecretKey: "apiKey"
```

### Permission Configuration

Edit the `wish-fulfiller-permissions` ConfigMap:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: wish-fulfiller-permissions
data:
  allowedNamespaces: "default,staging,production"
  allowedResources: "pods,deployments,services,configmaps"
  forbiddenOperations: "delete:namespaces,delete:persistentvolumes"
```

## Usage

### Using kubectl plugin

```bash
# Create a wish (dry-run by default)
kubectl wish create "create an nginx pod"

# Create with auto-fulfill
kubectl wish create "deploy redis with 3 replicas" --auto-fulfill --no-dry-run

# List wishes
kubectl wish list

# Describe a wish
kubectl wish describe wish-1234567890

# Fulfill a wish (execute the plan)
kubectl wish fulfill wish-1234567890

# Delete a wish
kubectl wish delete wish-1234567890
```

### Using kubectl directly

```bash
# Create wish
cat <<EOF | kubectl apply -f -
apiVersion: magic.k8s.io/v1alpha1
kind: Wish
metadata:
  name: my-wish
spec:
  wish: "Create an nginx deployment with 3 replicas"
  dryRun: true
  autoFulfill: false
EOF

# Check status
kubectl get wishes
kubectl describe wish my-wish

# Fulfill wish
kubectl patch wish my-wish --type=merge -p '{"spec":{"dryRun":false}}'
```

## Examples

### Example 1: Simple Pod

```bash
kubectl wish create "create a pod named test-pod running nginx:latest"
```

The LLM will generate something like:

```yaml
commands:
  - type: kubectl
    command: kubectl apply -f -
    yaml: |
      apiVersion: v1
      kind: Pod
      metadata:
        name: test-pod
      spec:
        containers:
        - name: nginx
          image: nginx:latest
```

### Example 2: Full Application Stack

```bash
kubectl wish create "deploy a complete wordpress stack with mysql database, persistent volumes, and a service"
```

### Example 3: Scaling

```bash
kubectl wish create "scale my-deployment to 5 replicas"
```

## Workflow

1. **Create**: User creates a wish using natural language
2. **Grant**: `wish-grantor` calls LLM to interpret and create execution plan
3. **Review**: User examines the plan via `kubectl wish describe`
4. **Fulfill**: User approves with `kubectl wish fulfill`
5. **Execute**: `wish-fulfiller` runs the commands

## Safety Features

- **Dry-run Default**: All wishes start in dry-run mode
- **Immutable Fulfillment**: Each wish can only be fulfilled once
- **Permission Controls**: ConfigMap-based restrictions on namespaces and resources
- **Separate RBAC**: Different permissions for planning (grantor) vs execution (fulfiller)
- **Manual Approval**: By default, requires explicit fulfill action

## Troubleshooting

### Check controller logs

```bash
# wish-grantor logs
kubectl logs -l app=wish-grantor -f

# wish-fulfiller logs
kubectl logs -l app=wish-fulfiller -f
```

### Common Issues

**Wish stuck in Requested state:**
- Check wish-grantor logs
- Verify LLM endpoint is accessible
- Check ConfigMap configuration

**Wish failed to fulfill:**
- Check wish-fulfiller logs
- Verify RBAC permissions
- Review permission ConfigMap settings
- Check if operation is forbidden

**LLM connection failed:**
- Verify endpoint in ConfigMap
- Check if credentials secret exists (if needed)
- Test LLM endpoint manually: `curl http://localhost:11434/v1/models`

## Development

### Running locally

```bash
# Run wish-grantor locally
RUST_LOG=info cargo run --bin wish-grantor

# Run wish-fulfiller locally
RUST_LOG=info cargo run --bin wish-fulfiller

# Run kubectl plugin locally
cargo run --bin kubectl-wish -- create "test wish"
```

### Testing with local LLM (Ollama)

```bash
# Install Ollama
curl -fsSL https://ollama.com/install.sh | sh

# Pull model
ollama pull llama3.2

# Ollama runs on localhost:11434 by default
```

## Security Considerations

1. **LLM Trust**: The LLM generates kubectl commands that will be executed. Ensure your LLM endpoint is trusted.
2. **RBAC**: Review and adjust the RBAC permissions in `k8s/rbac-fulfiller.yaml`
3. **Permissions**: Customize the permission ConfigMap for your environment
4. **Dry-run**: Always review wishes before fulfilling
5. **Audit**: Monitor wish creation and fulfillment via kubectl events and logs

## Architecture Decisions

- **Rust**: Chosen for performance, safety, and excellent Kubernetes ecosystem (kube-rs)
- **Separate Controllers**: Grantor and fulfiller separated for security and scalability
- **OpenAI-compatible API**: Supports any LLM provider with OpenAI-style endpoints
- **Dry-run Default**: Safety-first approach
- **Status Subresource**: Proper Kubernetes controller pattern

## License

MIT

## Contributing

Contributions welcome! Please ensure:
- Code compiles with `cargo build`
- Follow Rust conventions
- Update documentation
- Test with local cluster
