# Wish System Quick Start Guide

Get up and running with the Wish System in 10 minutes.

## Prerequisites

- Kubernetes cluster (minikube, kind, or any cluster)
- kubectl configured
- Ollama running locally (or any OpenAI-compatible LLM endpoint)

## Step 1: Install Ollama (if using local LLM)

```bash
# Install Ollama
curl -fsSL https://ollama.com/install.sh | sh

# Pull a model
ollama pull llama3.2

# Verify it's running
curl http://localhost:11434/v1/models
```

## Step 2: Build and Install

```bash
# Clone/navigate to wish-system directory
cd wish-system

# Build binaries
cargo build --release

# Install kubectl plugin
sudo cp target/release/kubectl-wish /usr/local/bin/

# Build controller images (if deploying to cluster)
./build-images.sh

# If using kind, load images
kind load docker-image wish-grantor:latest
kind load docker-image wish-fulfiller:latest
```

## Step 3: Deploy to Kubernetes

```bash
# Install CRD
kubectl apply -f k8s/crd.yaml

# Install RBAC
kubectl apply -f k8s/rbac-grantor.yaml
kubectl apply -f k8s/rbac-fulfiller.yaml

# Install configuration
kubectl apply -f k8s/config.yaml

# Deploy controllers
kubectl apply -f k8s/deployments.yaml

# Verify controllers are running
kubectl get pods
```

## Step 4: Create Your First Wish

```bash
# Create a simple wish
kubectl wish create "create an nginx pod named my-first-nginx"

# Check the wish status
kubectl wish list

# Describe the wish to see the plan
kubectl wish describe <wish-name>
```

## Step 5: Review and Fulfill

```bash
# Review the execution plan
kubectl wish describe <wish-name>

# If the plan looks good, fulfill it
kubectl wish fulfill <wish-name>

# Verify the pod was created
kubectl get pods
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

## Common Wishes to Try

```bash
# Create resources
kubectl wish create "create a redis pod"
kubectl wish create "deploy postgres with persistent storage"
kubectl wish create "create a service for my nginx deployment"

# Modify resources
kubectl wish create "scale my-deployment to 5 replicas"
kubectl wish create "update nginx image to nginx:1.24"

# Complex scenarios
kubectl wish create "create a complete web app with frontend deployment, backend deployment, and mysql database"
```

## Cleanup

```bash
# Delete a wish
kubectl wish delete <wish-name>

# Uninstall everything
kubectl delete -f k8s/deployments.yaml
kubectl delete -f k8s/config.yaml
kubectl delete -f k8s/rbac-fulfiller.yaml
kubectl delete -f k8s/rbac-grantor.yaml
kubectl delete -f k8s/crd.yaml
```

## Next Steps

- Read the full [README.md](README.md) for detailed documentation
- Customize the LLM configuration in `k8s/config.yaml`
- Adjust permissions in `k8s/rbac-fulfiller.yaml`
- Try the example wishes in `k8s/examples.yaml`

## Troubleshooting

**Wish stuck in Requested:**
```bash
# Check grantor logs
kubectl logs -l app=wish-grantor

# Verify LLM endpoint
curl http://localhost:11434/v1/models
```

**Wish failed:**
```bash
# Check fulfiller logs
kubectl logs -l app=wish-fulfiller

# Check wish details
kubectl wish describe <wish-name>
```

**Controllers not starting:**
```bash
# Check pod status
kubectl get pods
kubectl describe pod <pod-name>

# Check RBAC
kubectl auth can-i get wishes --as=system:serviceaccount:default:wish-grantor
```
