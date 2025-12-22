# Installation Guide

## Quick Installation

Install the wish-system in your existing Kubernetes cluster with a single command:

```bash
kubectl apply -f k8s/install.yaml
```

This will:
- Create a dedicated `wish-system` namespace
- Install the Wish CRD (Custom Resource Definition)
- Set up RBAC (Service Accounts, ClusterRoles, ClusterRoleBindings)
- Create ConfigMaps for LLM and permission configuration
- Create a Secret for LLM API credentials (if needed)

## What Gets Installed

The installation creates resources in the `wish-system` namespace:

- **Namespace**: `wish-system` - Dedicated namespace for the wish system
- **CRD**: `wishes.wish.ayourt.ch` - Custom Resource Definition for Wish objects
- **Service Accounts**: `wish-grantor`, `wish-fulfiller` - For controller authentication
- **RBAC**: ClusterRoles and ClusterRoleBindings with appropriate permissions
- **ConfigMaps**:
  - `wish-grantor-config` - LLM endpoint and model configuration
  - `wish-fulfiller-permissions` - Execution permission controls
- **Secret**: `llm-credentials` - For storing LLM API keys

## Configuration

Before deploying the controllers, you may want to customize the configuration:

### LLM Configuration

Edit the LLM configuration in `k8s/config.yaml` or after installation:

```bash
kubectl edit configmap wish-grantor-config -n wish-system
```

Key settings:
- `llmEndpoint`: URL of your LLM API (default: `http://localhost:11434/v1` for Ollama)
- `llmModel`: Model to use (default: `llama3.2:latest`)
- `credentialsSecretName`: Optional secret name for API key
- `credentialsSecretKey`: Key within the secret (default: `apiKey`)

### Permission Configuration

Edit permission settings:

```bash
kubectl edit configmap wish-fulfiller-permissions -n wish-system
```

Key settings:
- `allowedNamespaces`: Comma-separated list of namespaces where wishes can create resources
- `allowedResources`: Comma-separated list of allowed resource types
- `forbiddenOperations`: Operations that are explicitly forbidden (e.g., `delete:namespaces`)

### LLM API Credentials

If using a remote LLM that requires authentication:

```bash
kubectl create secret generic llm-credentials \
  --from-literal=apiKey=YOUR_API_KEY \
  -n wish-system \
  --dry-run=client -o yaml | kubectl apply -f -
```

## Deploying Controllers

The `install.yaml` does **not** include the controller Deployments, as these require Docker images.

For local development with kind:

```bash
# Build the binaries
make build

# Build Docker images
make build-images

# Load images into kind cluster
make kind-load

# Deploy the controllers
kubectl apply -f k8s/deployments.yaml
```

For production clusters, build and push images to your container registry, then update `k8s/deployments.yaml` with your image locations.

## Installing the kubectl Plugin

Install the `kubectl-wish` plugin:

```bash
# Build and install
make install-plugin

# Or manually
sudo cp target/release/kubectl-wish /usr/local/bin/
```

Verify installation:

```bash
kubectl wish --help
```

## Verification

Check that everything is installed:

```bash
# Check namespace
kubectl get namespace wish-system

# Check CRD
kubectl get crd wishes.wish.ayourt.ch

# Check service accounts
kubectl get sa -n wish-system

# Check RBAC
kubectl get clusterrole | grep wish
kubectl get clusterrolebinding | grep wish

# Check ConfigMaps
kubectl get configmap -n wish-system

# Check controllers (if deployed)
kubectl get pods -n wish-system
```

## Uninstallation

To completely remove the wish-system:

```bash
# Using Makefile
make uninstall-all

# Or manually
kubectl delete -f k8s/install.yaml
```

This will remove the namespace and all resources within it, including the CRD, RBAC, and any created wishes.

## Updating the Installation Manifest

If you modify any of the component files in `k8s/`, regenerate `install.yaml`:

```bash
make generate-install
```

This ensures `install.yaml` stays in sync with the individual component files.

## Installation from GitHub

Once committed to a repository, you can install directly from GitHub:

```bash
kubectl apply -f https://raw.githubusercontent.com/YOUR_USERNAME/wish-system/main/k8s/install.yaml
```

Replace `YOUR_USERNAME` with your actual GitHub username.
