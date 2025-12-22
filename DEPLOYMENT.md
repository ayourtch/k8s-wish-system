# Deployment Guide - No Registry Required!

The wish-system can now be deployed **without any external container registry**. The controllers use the Kubernetes API directly - no kubectl binary needed!

## What Changed

### Controllers No Longer Need kubectl
- ✅ **wish-grantor**: Uses Kubernetes API client (kube-rs) - never needed kubectl
- ✅ **wish-fulfiller**: **Refactored** to use Kubernetes API instead of shelling out to kubectl
  - Parses YAML manifests
  - Uses kube dynamic API to apply resources
  - Server-side apply for idempotent operations
  - No external dependencies!

### Single Runtime Image
Both controllers are built into one Docker image (`wish-system-runtime:latest`):
- Smaller footprint (shared dependencies)
- Faster builds (single compilation)
- Same image, different entry points

## Installation Methods

### Method 1: One Command (kind clusters)

Complete installation including controllers:

```bash
make kind-deploy
```

This will:
1. Create a kind cluster
2. Build the unified runtime image
3. Load the image into kind
4. Install CRD, RBAC, ConfigMaps
5. Deploy both controllers
6. Wait for them to be ready

### Method 2: Existing Cluster with Docker

If you have Docker and can load images into your cluster:

```bash
# Build the runtime image
make build-runtime

# Push to your registry (if needed)
docker tag wish-system-runtime:latest YOUR_REGISTRY/wish-system-runtime:latest
docker push YOUR_REGISTRY/wish-system-runtime:latest

# Update image in k8s/deployments-runtime.yaml
# Then install
kubectl apply -f k8s/install.yaml
kubectl apply -f k8s/deployments-runtime.yaml
```

### Method 3: Install Without Controllers

For clusters where you can't easily load images, install the infrastructure first:

```bash
kubectl apply -f k8s/install.yaml
```

This installs:
- `wish-system` namespace
- CRD (Custom Resource Definition)
- RBAC (ServiceAccounts, ClusterRoles, ClusterRoleBindings)
- ConfigMaps (LLM config, permissions)
- Secret (for API keys)

Then deploy controllers separately using your preferred method (CI/CD, GitOps, etc.)

## Architecture Benefits

### No External Dependencies
- **Old**: wish-fulfiller shelled out to `kubectl apply`
- **New**: wish-fulfiller uses Kubernetes API client directly
- **Result**: Simpler, faster, more reliable, no kubectl binary needed

### API-Native Operations
```rust
// Old approach (shell out)
ProcessCommand::new("kubectl").args(["apply", "-f", "-"]).spawn()

// New approach (API native)
let api: Api<DynamicObject> = Api::namespaced_with(client, ns, &ar);
api.patch(&name, &PatchParams::apply("wish-fulfiller"), &patch).await
```

Benefits:
- Better error handling
- Type-safe operations
- Async/await native
- No shell injection risks
- Structured logging

## Makefile Targets

### Building
```bash
make build                # Build Rust binaries locally
make build-runtime        # Build Docker runtime image
```

### Kind (local development)
```bash
make kind-cluster         # Create kind cluster
make kind-load-runtime    # Load runtime image into kind
make kind-deploy          # Complete deployment (cluster + install + controllers)
make kind-delete          # Delete cluster
```

### Installation
```bash
make generate-install     # Regenerate k8s/install.yaml from components
make install-all          # Install CRD, RBAC, ConfigMaps (no controllers)
make uninstall-all        # Remove everything including namespace
```

### Controllers
```bash
kubectl apply -f k8s/deployments-runtime.yaml    # Deploy controllers
kubectl delete -f k8s/deployments-runtime.yaml   # Remove controllers
```

## Files Overview

- `Dockerfile.runtime` - Multi-stage build for both controllers
- `k8s/install.yaml` - All-in-one installation (CRD + RBAC + Config)
- `k8s/deployments-runtime.yaml` - Controller deployments using runtime image
- `k8s/namespace.yaml` - Dedicated wish-system namespace
- `k8s/crd.yaml` - Wish Custom Resource Definition
- `k8s/rbac-grantor.yaml` - RBAC for wish-grantor
- `k8s/rbac-fulfiller.yaml` - RBAC for wish-fulfiller
- `k8s/config.yaml` - ConfigMaps and Secrets

## Configuration

### LLM Configuration
Edit `k8s/config.yaml` before installation or update after:

```bash
kubectl edit configmap wish-grantor-config -n wish-system
```

### Permission Configuration
Control what wishes can do:

```bash
kubectl edit configmap wish-fulfiller-permissions -n wish-system
```

## Verification

```bash
# Check namespace
kubectl get namespace wish-system

# Check CRD
kubectl get crd wishes.wish.ayourt.ch

# Check controllers
kubectl get pods -n wish-system

# Check controller logs
kubectl logs -l app=wish-grantor -n wish-system
kubectl logs -l app=wish-fulfiller -n wish-system

# Create a test wish
kubectl wish create "deploy nginx" --name test-nginx
```

## Troubleshooting

### Controllers not starting
```bash
# Check pod status
kubectl get pods -n wish-system

# Check events
kubectl get events -n wish-system --sort-by='.lastTimestamp'

# Check logs
kubectl logs deployment/wish-grantor -n wish-system
kubectl logs deployment/wish-fulfiller -n wish-system
```

### Image pull errors
If using a registry, verify:
```bash
# Check image pull secrets
kubectl get secrets -n wish-system

# Verify image exists
docker images | grep wish-system-runtime

# For kind, ensure image was loaded
kind load docker-image wish-system-runtime:latest --name wish-system
```

## Next Steps

1. **Test the installation**: `make kind-deploy`
2. **Create your first wish**: See [QUICKSTART.md](QUICKSTART.md)
3. **Configure permissions**: Edit ConfigMaps for your security requirements
4. **Set up LLM**: Configure your LLM endpoint (Ollama, OpenAI, etc.)

## Summary

The wish-system now has **zero external dependencies** beyond the Kubernetes API. The controllers are pure Kubernetes-native applications that:
- Use the API client for all operations
- Run in a simple Debian container
- Require no special tooling
- Can be deployed anywhere Kubernetes runs

This makes it perfect for air-gapped environments, restrictive security policies, or any cluster where installing kubectl in pods would be problematic.
