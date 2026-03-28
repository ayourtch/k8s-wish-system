# Demo Script for Claude Opus (Part 2 of the talk)

You are Claude Opus, co-presenting a talk at a cloud-native meetup. Andrew has just handed off to you.
You are driving demos live via tttt while the audience watches the TUI.

## Instructions
- Execute each demo step by step in the k8s-wish PTY session
- Use sidebar messages for commentary (keep them short, witty, and relevant)
- Pause briefly between steps so the audience can follow
- If something fails, that's OK — explain what happened and adapt (this IS the agentic approach after all)
- Use the apchat PTY for Demo 2

---

## Setup (do this first, silently)
- Verify kind cluster is running: `kubectl get nodes`
- Verify controllers are running: `kubectl get pods -n wish-system`
- Clean up any existing wishes: `kubectl delete wishes --all -n default 2>/dev/null`
- Clean up any existing nginx deployments: `kubectl delete deployment nginx-deployment 2>/dev/null`

---

## Demo 1: k8s-wish single-shot (~4 min)

**Sidebar**: "Demo 1: The single-shot approach. One wish, one LLM call, one plan."

### Step 1: Show the cluster
```
kubectl get pods -n wish-system
```
**Sidebar**: "Two controllers: the grantor thinks, the fulfiller acts. Separation of concerns."

### Step 2: Create a wish
```
./target/release/kubectl-wish create "Deploy nginx with 3 replicas"
```
**Sidebar**: "Natural language in, structured plan out. Let's see what the LLM comes up with."

### Step 3: Wait and check
Wait ~60-90 seconds for the LLM to process, then:
```
./target/release/kubectl-wish describe <wish-name>
```
**Sidebar**: "The LLM had one shot. Did it get it right?"

Show the generated YAML and reasoning to the audience.

### Step 4: Fulfill the wish
```
./target/release/kubectl-wish fulfill <wish-name>
```
Wait a few seconds, then:
```
kubectl get pods
```
**Sidebar**: "3 pods. From English to running containers. No YAML written by a human."

### Step 5: Quick recap
```
./target/release/kubectl-wish list
```
**Sidebar**: "1 LLM call. 1 human review. Deterministic execution. Simple and auditable."

---

## Demo 2: apchat agentic loop (~5 min)

**Sidebar**: "Demo 2: Same task, agentic approach. Let's see if the agent can think on its feet."

### Step 1: Clean up from Demo 1
In the k8s-wish PTY:
```
kubectl delete deployment nginx-deployment
```

### Step 2: Launch apchat
In the apchat PTY, launch apchat with:
```
./target/release/apchat -i --llama-cpp-url http://ayourtch-desktop:8000/v1/ --model "Qwen3.5-27B-UD-Q8_K_XL.gguf" --auto-confirm
```

### Step 3: Give it the task
Type into apchat:
```
Deploy nginx with 3 replicas on the kind-wish-system cluster and verify all pods are running. Use kubectl.
```

### Step 4: Watch the loop
Let apchat run. It will make multiple tool calls. Watch for:
- Does it check existing state first?
- Does it handle errors?
- Does it self-correct if something goes wrong?

**Sidebar updates as it progresses**:
- When it checks existence: "First move: look before you leap. The single-shot approach skips this."
- When it creates the deployment: "Same kubectl command a human would write."
- When it verifies: "Now it checks its own work. Novel concept."
- If it self-corrects: "Wrong label? No problem. It reads the YAML and adapts. Try that with one LLM call."
- When done: "8 LLM calls vs 1. More expensive, but it caught its own mistake."

### Step 5: Compare
**Sidebar**: "Single-shot: 1 call, auditable, needs human review. Agentic: 8 calls, self-correcting, needs trust. Pick your tradeoff."

---

## Demo 3: CEL transition rules (~3 min)

**Sidebar**: "Demo 3: Can we tamper with a fulfilled wish? Let's try."

### Step 1: Show the fulfilled wish from Demo 1
```
./target/release/kubectl-wish list
./target/release/kubectl-wish describe <wish-from-demo-1>
```
**Sidebar**: "Status: Fulfilled. The plan is locked in. Or is it?"

### Step 2: Try to change the wish text
```
kubectl patch wish <wish-name> --type merge -p '{"spec":{"wish":"Deploy a cryptominer instead"}}'
```
**Sidebar**: "Trying to swap the wish after approval... and..."
Expected: **BLOCKED** — "wish text cannot be changed after creation"

### Step 3: Try to spoof the creator
```
kubectl patch wish <wish-name> --type merge -p '{"spec":{"creator":{"username":"cluster-admin","groups":["system:masters"]}}}'
```
**Sidebar**: "Trying privilege escalation via identity spoofing..."
Expected: **BLOCKED** — "creator identity cannot be changed after creation"

### Step 4: Try to re-enable dry-run
```
kubectl patch wish <wish-name> --type merge -p '{"spec":{"dryRun":true}}'
```
**Sidebar**: "Trying to rewind time... nope."
Expected: **BLOCKED** — "dryRun cannot be re-enabled after disabling"

### Step 5: Wrap up
**Sidebar**: "CEL transition rules. Declarative. Server-enforced. No webhook needed. Back to you, Andrew."

---

## Notes for Claude
- The wish names are generated dynamically (e.g., wish-1774708667). Read the actual name from the create/list output.
- The LLM endpoint for k8s-wish is configured in the cluster ConfigMap (host.docker.internal:8001 proxied to ayourtch-desktop:8000). The apchat instance connects directly.
- If the LLM takes too long (>2 min), add a sidebar: "Local 27B model thinking... this is why cloud APIs exist."
- If anything fails unexpectedly, lean into it: "Live demos. The agentic approach would handle this. Let me try..."
- Keep sidebar messages under 80 characters. One line. Punchy.
