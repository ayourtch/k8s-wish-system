# Quick Start Guide

Get started with wish-system in under 5 minutes!

## One-Line Install (Interactive)

```bash
./scripts/install.sh
```

This interactive script will:
- ✓ Check prerequisites (kubectl, docker)
- ✓ Detect or create a Kubernetes cluster
- ✓ Install the wish-system components
- ✓ Auto-detect local LLM services (Ollama, LM Studio)
- ✓ Guide you through LLM configuration
- ✓ Verify the installation

## Prerequisites

- **kubectl** - Kubernetes command-line tool
- **docker** - Container runtime
- **Kubernetes cluster** - Any cluster (Kind, Minikube, GKE, EKS, etc.)

### Optional (for building from source)
- **Rust** - https://rustup.rs/ (only if building from source)
- **Kind** - https://kind.sigs.k8s.io/ (only for local testing)

## Quick Install Steps

### 1. Clone the repository

```bash
git clone https://github.com/yourusername/wish-system
cd wish-system
```

### 2. Run the installer

```bash
./scripts/install.sh
```

Follow the interactive prompts to:
- Choose installation method (manifests or build from source)
- Configure your LLM endpoint

### 3. Create your first wish

```bash
# Build kubectl-wish (if building from source)
cargo build --release --bin kubectl-wish

# Create a wish
./target/release/kubectl-wish create "Deploy a simple nginx web server"

# Or use kubectl directly
kubectl apply -f - <<EOF
apiVersion: wish.ayourt.ch/v1alpha1
kind: Wish
metadata:
  name: my-first-wish
spec:
  wish: "Deploy a simple nginx web server"
  autoFulfill: false
  dryRun: true
  targetNamespace: default
EOF
```

### 4. Monitor the wish

```bash
# List all wishes
./target/release/kubectl-wish list

# Describe a specific wish
./target/release/kubectl-wish describe my-first-wish

# Watch the status
watch -n 2 './target/release/kubectl-wish list'
```

### 5. Fulfill the wish (if in dry-run mode)

```bash
# Review the execution plan
./target/release/kubectl-wish describe my-first-wish

# If satisfied, fulfill it
./target/release/kubectl-wish fulfill my-first-wish
```

## Standalone LLM Configuration

If you skipped LLM configuration during install:

```bash
# Interactive configuration
./scripts/configure-llm.sh

# Or manual configuration
./target/release/kubectl-wish configure \
  --endpoint "http://localhost:11434/v1" \
  --model "llama3.2:latest"

# For OpenAI
./target/release/kubectl-wish configure \
  --endpoint "https://api.openai.com/v1" \
  --model "gpt-4" \
  --api-key "sk-..."

# View current configuration
./target/release/kubectl-wish configure --show
```

## Common LLM Endpoints

### Ollama (Local)
```bash
# Install Ollama: https://ollama.ai/
ollama pull llama3.2:latest

./target/release/kubectl-wish configure \
  --endpoint "http://localhost:11434/v1" \
  --model "llama3.2:latest"
```

### LM Studio (Local)
```bash
# Install LM Studio: https://lmstudio.ai/
# Load a model in LM Studio and start the server

./target/release/kubectl-wish configure \
  --endpoint "http://localhost:1234/v1" \
  --model "your-model-name"
```

### OpenAI
```bash
./target/release/kubectl-wish configure \
  --endpoint "https://api.openai.com/v1" \
  --model "gpt-4" \
  --api-key "$OPENAI_API_KEY"
```

### Azure OpenAI
```bash
./target/release/kubectl-wish configure \
  --endpoint "https://your-resource.openai.azure.com/openai/deployments/your-deployment" \
  --model "gpt-4" \
  --api-key "$AZURE_OPENAI_KEY"
```

## Install kubectl-wish Plugin

For easier command-line usage:

```bash
# Copy to PATH
sudo cp target/release/kubectl-wish /usr/local/bin/

# Now you can use:
kubectl wish create "Deploy redis"
kubectl wish list
kubectl wish describe my-wish
```

## Example Wishes

Try these example wishes to get started:

```bash
# Simple deployment
kubectl wish create "Deploy nginx with 3 replicas"

# With configuration
kubectl wish create "Deploy a Redis instance with persistence enabled"

# Multiple resources
kubectl wish create "Create a complete web application with nginx frontend, backend API, and PostgreSQL database"

# With specific requirements
kubectl wish create "Deploy Prometheus monitoring with persistent storage in monitoring namespace" \
  --target-namespace monitoring

# Auto-fulfill (skip dry-run)
kubectl wish create "Create a ConfigMap named app-config with key environment=production" \
  --no-dry-run --auto-fulfill
```

## Example Session

```bash
# Create a wish
$ kubectl wish create "create a deployment with 3 nginx replicas"
Wish created: wish-1703123456
Status: Requested
Mode: Dry-run (will not execute automatically)
Use 'kubectl wish fulfill wish-1703123456' to execute after review

# Wait a moment for the grantor to process
$ sleep 5

# Check what the LLM planned
$ kubectl wish describe wish-1703123456
Name:      wish-1703123456
Namespace: default

Spec:
  Wish:        create a deployment with 3 nginx replicas
  Auto-fulfill: false
  Dry-run:     true

Status:
  Phase:     Granted
  Name:      nginx-deployment-3-replicas

  Execution Plan:
    Reasoning: Creating a Deployment resource with 3 replicas running nginx
    Commands (1):
      1. Type: Kubectl
         Command: kubectl apply -f -
         YAML:
           apiVersion: apps/v1
           kind: Deployment
           metadata:
             name: nginx-deployment
           spec:
             replicas: 3
             selector:
               matchLabels:
                 app: nginx
             template:
               metadata:
                 labels:
                   app: nginx
               spec:
                 containers:
                 - name: nginx
                   image: nginx:latest
                   ports:
                   - containerPort: 80

# Looks good! Fulfill it
$ kubectl wish fulfill wish-1703123456
Wish 'wish-1703123456' marked for fulfillment
The wish-fulfiller controller will execute it shortly

# Verify
$ kubectl get deployments
NAME               READY   UP-TO-DATE   AVAILABLE   AGE
nginx-deployment   3/3     3            3           10s
```

## Verification

Check that everything is running:

```bash
# Check controller pods
kubectl get pods -n wish-system

# Should see:
# wish-grantor-xxx    Running
# wish-fulfiller-xxx  Running

# Check CRD
kubectl get crd wishes.wish.ayourt.ch

# Check configuration
kubectl wish configure --show
```

## Troubleshooting

### Controllers not starting
```bash
# Check logs
kubectl logs -n wish-system deployment/wish-grantor
kubectl logs -n wish-system deployment/wish-fulfiller

# Check events
kubectl get events -n wish-system --sort-by='.lastTimestamp'
```

### LLM connection issues
```bash
# Test LLM endpoint manually
curl -X POST http://localhost:11434/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama3.2:latest",
    "messages": [{"role": "user", "content": "Hello"}]
  }'

# Reconfigure
./scripts/configure-llm.sh
```

### Wish stuck in "Requested" phase
```bash
# Check grantor logs
kubectl logs -n wish-system deployment/wish-grantor -f

# Check LLM configuration
kubectl wish configure --show
kubectl get configmap wish-grantor-config -n wish-system -o yaml
```

## Uninstall

```bash
# Delete all wishes first
kubectl delete wishes --all -n default

# Uninstall wish-system
kubectl delete -f k8s/install.yaml

# For Kind cluster
make kind-delete
```

## Next Steps

- Read [INSTALL.md](INSTALL.md) for detailed installation options
- Read [DEPLOYMENT.md](DEPLOYMENT.md) for production deployment guidance
- Check [README.md](README.md) for architecture and design details

## Getting Help

- Check logs: `kubectl logs -n wish-system deployment/wish-grantor`
- View events: `kubectl get events -n wish-system`
- Describe wish: `kubectl wish describe <wish-name>`
- GitHub Issues: https://github.com/yourusername/wish-system/issues
