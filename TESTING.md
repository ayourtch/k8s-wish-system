# Deployment and Testing Guide

## Building the Project

Since this project uses Rust with external dependencies, you'll need network access to download crates from crates.io.

### Build Steps

1. **Ensure you have a recent stable Rust toolchain:**
   ```bash
   rustup update stable
   ```

2. **Build the project:**
   ```bash
   cd wish-system
   cargo build --release
   ```

3. **Build controller images:**
   ```bash
   ./build-images.sh
   ```

## Local Development Testing

### Option 1: Run Controllers Locally

You can run the controllers locally without deploying to Kubernetes:

```bash
# Terminal 1: Run wish-grantor
RUST_LOG=info cargo run --bin wish-grantor

# Terminal 2: Run wish-fulfiller  
RUST_LOG=info cargo run --bin wish-fulfiller

# Terminal 3: Create wishes
cargo run --bin kubectl-wish -- create "test wish"
```

### Option 2: Deploy to Local Cluster

#### Using Kind

```bash
# Create cluster
make kind-cluster

# Build and load images
make build-images
make kind-load

# Deploy
make deploy

# Install kubectl plugin
make install-plugin
```

#### Using Minikube

```bash
# Start minikube
minikube start

# Use minikube's Docker daemon
eval $(minikube docker-env)

# Build images
make build-images

# Deploy
make deploy

# Install kubectl plugin
make install-plugin
```

## Testing Workflow

### 1. Verify Installation

```bash
# Check CRD is installed
kubectl get crd wishes.magic.k8s.io

# Check controllers are running
kubectl get pods

# Check kubectl plugin
kubectl wish --help
```

### 2. Test Basic Wish

```bash
# Create a simple wish
kubectl wish create "create an nginx pod named test-nginx"

# Wait a few seconds for processing
sleep 5

# Check status
kubectl wish list

# Describe the wish to see the plan
WISH_NAME=$(kubectl get wishes -o jsonpath='{.items[0].metadata.name}')
kubectl wish describe $WISH_NAME
```

### 3. Review and Fulfill

```bash
# Review the execution plan
kubectl wish describe $WISH_NAME

# If it looks good, fulfill it
kubectl wish fulfill $WISH_NAME

# Verify the resource was created
kubectl get pods
```

### 4. Test Dry-Run Mode

```bash
# Dry-run is enabled by default
kubectl wish create "deploy redis with 3 replicas"

# The wish will be granted but not executed
kubectl wish list

# Review what would happen
kubectl wish describe <wish-name>

# If approved, fulfill
kubectl wish fulfill <wish-name>
```

### 5. Test Auto-Fulfill

```bash
# Create with auto-fulfill
kubectl wish create "create a busybox pod" --auto-fulfill --no-dry-run

# Check that it was executed automatically
kubectl get pods
```

## Testing Different Scenarios

### Scenario 1: Simple Resource Creation

```bash
kubectl wish create "create a ConfigMap named app-config with key=value"
```

### Scenario 2: Deployment with Service

```bash
kubectl wish create "deploy nginx with 3 replicas and expose it via a ClusterIP service on port 80"
```

### Scenario 3: Complex Application

```bash
kubectl wish create "create a wordpress installation with mysql database, persistent volumes, and load balancer service"
```

### Scenario 4: Resource Modification

```bash
# First create a deployment
kubectl create deployment test-app --image=nginx

# Then wish to modify it
kubectl wish create "scale test-app deployment to 5 replicas"
```

## Debugging

### Check Controller Logs

```bash
# wish-grantor logs
kubectl logs -l app=wish-grantor -f

# wish-fulfiller logs
kubectl logs -l app=wish-fulfiller -f
```

### Common Issues

1. **Wish stuck in Requested:**
   - Check wish-grantor logs
   - Verify LLM endpoint is reachable
   - Test LLM directly: `curl http://localhost:11434/v1/models`

2. **LLM returns invalid JSON:**
   - Check the prompt in wish-grantor.rs
   - Try a different model
   - Increase temperature or max_tokens

3. **Execution fails:**
   - Check wish-fulfiller logs
   - Verify RBAC permissions
   - Check permission ConfigMap settings
   - Review the generated kubectl command

4. **Permission denied:**
   - Check RBAC: `kubectl auth can-i <verb> <resource> --as=system:serviceaccount:default:wish-fulfiller`
   - Review `k8s/rbac-fulfiller.yaml`
   - Update forbidden operations in ConfigMap

### Manual Testing

You can manually test the LLM integration:

```bash
# Test LLM endpoint
curl http://localhost:11434/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama3.2:latest",
    "messages": [
      {"role": "system", "content": "You are a Kubernetes expert."},
      {"role": "user", "content": "Generate kubectl command to create an nginx pod"}
    ]
  }'
```

## Performance Testing

### Create Multiple Wishes

```bash
# Create several wishes
for i in {1..5}; do
  kubectl wish create "create a pod named test-pod-$i running nginx"
  sleep 2
done

# Monitor processing
watch kubectl wish list
```

### Stress Test

```bash
# Create many wishes rapidly
for i in {1..20}; do
  kubectl wish create "create configmap config-$i" &
done

# Check controller performance
kubectl top pods
```

## Security Testing

### Test Permission Boundaries

```bash
# Try forbidden operation
kubectl wish create "delete all namespaces"

# Check if it's blocked
kubectl wish describe <wish-name>
```

### Test Namespace Isolation

```bash
# Create wish in non-allowed namespace
kubectl create ns restricted
kubectl wish create "create pod in restricted namespace" -n restricted

# Should fail or be blocked
```

## Clean Up

```bash
# Delete all wishes
kubectl delete wishes --all

# Uninstall
make clean

# Delete local cluster (if using kind)
make kind-delete
```

## CI/CD Integration

### GitHub Actions Example

```yaml
name: Test Wish System

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          
      - name: Build
        run: cargo build --release
        
      - name: Run tests
        run: cargo test
        
      - name: Setup Kind
        uses: engineerd/setup-kind@v0.5.0
        
      - name: Build images
        run: ./build-images.sh
        
      - name: Load images
        run: |
          kind load docker-image wish-grantor:latest
          kind load docker-image wish-fulfiller:latest
          
      - name: Deploy
        run: make deploy
        
      - name: Test
        run: |
          kubectl wait --for=condition=ready pod -l app=wish-grantor --timeout=60s
          kubectl wish create "create a test pod"
```

## Production Considerations

1. **High Availability:**
   - Run multiple replicas of wish-grantor
   - Use leader election for wish-fulfiller

2. **Monitoring:**
   - Add Prometheus metrics
   - Set up alerts for failed wishes
   - Monitor LLM latency

3. **Audit:**
   - Log all wish creations and fulfillments
   - Store execution history
   - Track who created wishes

4. **Rate Limiting:**
   - Limit wishes per user/namespace
   - Queue management for LLM calls

5. **Security Hardening:**
   - Implement webhook validation
   - Add resource quotas
   - Use Pod Security Standards
   - Network policies for LLM access
