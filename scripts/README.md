# Installation Scripts

This directory contains user-friendly installation and configuration scripts for wish-system.

## Scripts

### `install.sh`

**Main installation script** - Interactive installer that guides you through the entire setup process.

```bash
./scripts/install.sh
```

Features:
- Checks prerequisites (kubectl, docker)
- Detects or helps create Kubernetes cluster
- Offers installation from manifests or source
- Detects Kind clusters and handles image loading automatically
- Prompts for container registry (for non-Kind clusters)
- Integrates LLM configuration
- Verifies installation

### `configure-llm.sh`

**LLM configuration helper** - Interactive script to configure the LLM endpoint.

```bash
./scripts/configure-llm.sh
```

Features:
- Auto-detects Ollama (http://localhost:11434)
- Auto-detects LM Studio (http://localhost:1234)
- Provides presets for OpenAI, Azure OpenAI
- Supports custom endpoints
- Can be run during install or standalone

Environment variables:
- `WISH_NAMESPACE` - Namespace for configuration (default: "default")

## Usage Examples

### Complete fresh install

```bash
# Run the installer
./scripts/install.sh

# Follow prompts to:
# 1. Choose installation method
# 2. Configure LLM
```

### Reconfigure LLM after install

```bash
# Use the configuration script
./scripts/configure-llm.sh

# Or use kubectl wish directly
./target/release/kubectl-wish configure \
  --endpoint "http://localhost:11434/v1" \
  --model "llama3.2:latest"
```

### Install for different namespaces

```bash
# Configure LLM for wish-system namespace
WISH_NAMESPACE=wish-system ./scripts/configure-llm.sh
```

## Supported LLM Providers

The scripts support configuration for:

1. **Ollama** (local)
   - Auto-detected at http://localhost:11434
   - Lists available models

2. **LM Studio** (local)
   - Auto-detected at http://localhost:1234

3. **OpenAI**
   - Endpoint: https://api.openai.com/v1
   - Requires API key

4. **Custom**
   - Any OpenAI-compatible endpoint
   - Optional API key

## Prerequisites

### All scripts
- bash
- kubectl
- Kubernetes cluster access

### install.sh additional
- docker (for building images)
- cargo/rust (if building from source)
- kind (if creating local cluster)

## Troubleshooting

### "kubectl not found"
Install kubectl: https://kubernetes.io/docs/tasks/tools/

### "Cannot connect to Kubernetes cluster"
```bash
# Check cluster access
kubectl cluster-info

# If using Kind, create cluster
kind create cluster --name wish-system
```

### "No LLM services detected"
```bash
# Install and run Ollama
curl -fsSL https://ollama.com/install.sh | sh
ollama pull llama3.2:latest

# Or install LM Studio
# Download from: https://lmstudio.ai/
```

### Scripts fail with "Permission denied"
```bash
# Make scripts executable
chmod +x scripts/*.sh
```
