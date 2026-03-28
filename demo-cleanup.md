# Demo Cleanup Script

Run this after the talk (or before re-running demo-install) to tear everything down.
Prompt: "Please follow demo-cleanup.md to clean up the demo environment"

## What this does
1. Deletes all wishes and workloads from the kind cluster
2. Deletes the kind cluster
3. Kills the TCP proxy (if running)
4. Optionally cleans up Docker images

---

## Step 1: Clean up K8s resources

If the kind cluster is still running:
```
kubectl delete wishes --all -n default 2>/dev/null
kubectl delete wishes --all --all-namespaces 2>/dev/null
kubectl delete deployments --all -n default 2>/dev/null
kubectl delete pods --all -n default 2>/dev/null
```

## Step 2: Delete the kind cluster

```
kind delete cluster --name wish-system
```

Verify no clusters remain:
```
kind get clusters
```
Should say "No kind clusters found."

## Step 3: Kill the TCP proxy

Find and kill the Python proxy process:
```
pkill -f "python3.*TCP-LISTEN\|python3.*8001.*listen" 2>/dev/null
# More reliable: find the exact process
ps aux | grep "python3.*8001" | grep -v grep
```
Kill by PID if found:
```
kill <pid>
```

Or kill all background Python proxy processes from this session:
```
kill %1 2>/dev/null
```

## Step 4: (Optional) Clean up Docker images

Remove the wish-system runtime image:
```
docker rmi wish-system-runtime:latest 2>/dev/null
```

Remove kind node images (only if you don't use kind for other things):
```
docker rmi kindest/node:v1.35.0 2>/dev/null
```

## Step 5: Verify clean state

```
kind get clusters
docker ps -a | grep wish
kubectl config get-contexts | grep wish
```

All should be empty/clean. If a stale kubectl context remains:
```
kubectl config delete-context kind-wish-system 2>/dev/null
kubectl config delete-cluster kind-wish-system 2>/dev/null
```

## Done

Environment is clean. Run `demo-install.md` to set it up again.
