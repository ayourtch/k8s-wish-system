# Demo Install Script

Run this before the talk to set up the full demo environment.
Prompt: "Please follow demo-install.md to set up the demo environment"

## What this does
1. Verifies prerequisites (docker, kubectl, kind, cargo, LLM endpoint)
2. Builds k8s-wish-system from source (Rust binaries + Docker image)
3. Builds apchat from source
4. Creates a kind cluster and deploys k8s-wish controllers
5. Configures the LLM endpoint (Qwen3.5-27B via llama.cpp)
6. Sets up a TCP proxy if the LLM is on a Tailscale/remote host
7. Runs a smoke test to verify everything works end-to-end

## Environment assumptions
- All three repos are checked out under the same parent directory
- Working directory is k8s-wish-system
- The sibling `apchat/` directory exists
- Docker daemon is running (OrbStack, Docker Desktop, etc.)
- LLM server is at `http://ayourtch-desktop:8000/v1/` running Qwen3.5-27B
  (adjust the URL if using a different endpoint)

---

## Step 1: Verify prerequisites

Launch a PTY and check that all required tools are installed:

```
which kubectl docker kind cargo rustc curl
docker info --format '{{.ServerVersion}}'
kind version
rustc --version
```

All must succeed. If Docker isn't running, start it:
```
open -a OrbStack
```
(or `open -a Docker` for Docker Desktop) — wait ~10 seconds and re-check.

## Step 2: Verify LLM endpoint

```
curl -s http://ayourtch-desktop:8000/v1/models | head -1
```

Should return JSON with model info. If it fails, the LLM server needs to be started separately.

## Step 3: Build k8s-wish-system

In the k8s-wish-system directory:
```
cargo build --release --bins
```
This compiles `wish-grantor`, `wish-fulfiller`, and `kubectl-wish`. Takes ~2 min on first build.

Verify:
```
ls -la target/release/kubectl-wish target/release/wish-grantor target/release/wish-fulfiller
```

## Step 4: Build apchat

In the sibling apchat directory:
```
cd ../apchat
cargo build --release --no-default-features
cd ../k8s-wish-system
```
Takes ~2 min. Verify:
```
ls -la ../apchat/target/release/apchat
```

## Step 5: Create kind cluster and deploy

First, delete any stale kind cluster with the same name:
```
kind delete cluster --name wish-system 2>/dev/null
```

Then build the Docker image, create the cluster, and deploy:
```
make build-runtime
kind create cluster --name wish-system
kind load docker-image wish-system-runtime:latest --name wish-system
make install-all
kubectl apply -f k8s/deployments-runtime.yaml
```

Wait for controllers to be ready:
```
kubectl wait --for=condition=available --timeout=120s deployment/wish-grantor -n wish-system
kubectl wait --for=condition=available --timeout=120s deployment/wish-fulfiller -n wish-system
kubectl get pods -n wish-system
```

Both should show `1/1 Running`.

## Step 6: Configure LLM endpoint

The LLM server at `ayourtch-desktop:8000` is on a Tailscale IP, not reachable from inside kind.
We need a TCP proxy on the host so kind pods can reach it via `host.docker.internal`.

Start a Python TCP proxy (in background):
```
python3 -c "
import socket, threading
def proxy(src, dst):
    try:
        while d := src.recv(65536):
            dst.sendall(d)
    except: pass
    src.close(); dst.close()
s = socket.socket(); s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
s.bind(('0.0.0.0', 8001)); s.listen(5)
print('Proxy listening on :8001', flush=True)
while True:
    c, _ = s.accept()
    r = socket.create_connection(('ayourtch-desktop', 8000))
    threading.Thread(target=proxy, args=(c,r), daemon=True).start()
    threading.Thread(target=proxy, args=(r,c), daemon=True).start()
" &
```

Verify the proxy works from inside kind:
```
kubectl run test-proxy --rm -it --restart=Never --image=curlimages/curl -- curl -s --connect-timeout 5 http://host.docker.internal:8001/v1/models 2>&1 | head -1
```
Should return model JSON.

Configure the wish-grantor to use the proxied endpoint:
```
kubectl patch configmap wish-grantor-config -n wish-system --type merge \
  -p '{"data":{"llmEndpoint":"http://host.docker.internal:8001/v1","llmModel":"Qwen3.5-27B-UD-Q8_K_XL.gguf"}}'
kubectl rollout restart deployment/wish-grantor -n wish-system
kubectl wait --for=condition=available --timeout=60s deployment/wish-grantor -n wish-system
```

## Step 7: Smoke test

Create a test wish, wait for it to be granted, fulfill it, verify pods:
```
./target/release/kubectl-wish create "Create a test nginx pod"
```

Wait ~60-90 seconds for the local LLM, then check:
```
./target/release/kubectl-wish list
```

If the wish shows `Granted`, fulfill it:
```
./target/release/kubectl-wish fulfill <wish-name>
```

Wait ~10 seconds, then verify:
```
kubectl get pods
```

Should see the nginx pod running.

## Step 8: Clean up smoke test

```
kubectl delete wishes --all -n default
kubectl delete deployment --all -n default 2>/dev/null
kubectl delete pod --all -n default 2>/dev/null
```

## Done

The environment is ready for the talk. Summary:
- kind cluster `wish-system` running with both controllers
- LLM reachable from inside kind via proxy on port 8001
- `kubectl-wish` binary at `./target/release/kubectl-wish`
- `apchat` binary at `../apchat/target/release/apchat`
- apchat connects directly to `http://ayourtch-desktop:8000/v1/` (no proxy needed, runs on host)

**Important**: The Python TCP proxy runs in the background. If the PTY is killed, it dies too.
Consider running it in a separate PTY or using `nohup`.
