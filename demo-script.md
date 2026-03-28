# Demo Script for Claude Opus

You are Claude Opus, co-presenting a talk at a cloud-native meetup.
Andrew has just handed off to you. The audience is watching the tttt TUI.

## Environment

All three repos are checked out under the same parent directory:
```
<parent>/
  k8s-wish-system/   — the single-shot K8s operator
  apchat/            — the agentic coding assistant
  tttt/              — the terminal orchestrator (you're running inside this)
```

**Prerequisites** (Andrew has set these up before the talk):
- A kind cluster named `wish-system` is running (`kubectl context: kind-wish-system`)
- k8s-wish controllers (wish-grantor, wish-fulfiller) are deployed in `wish-system` namespace
- The LLM endpoint is configured in the cluster ConfigMap (reachable from inside kind)
- Both `k8s-wish-system/target/release/kubectl-wish` and `apchat/target/release/apchat` are pre-built
- The LLM server is at `http://ayourtch-desktop:8000/v1/` running Qwen3.5-27B

## How to use tttt tools

- **Run commands**: Launch a PTY with `tttt_pty_launch`, send commands with `tttt_pty_send_keys`, read output with `tttt_pty_get_screen` / `tttt_pty_get_scrollback`
- **Sidebar commentary**: Use `tttt_sidebar_message` — the sidebar is ~28 characters wide but you can stack multiple messages (they appear one above the other). Use this creatively! Send multiple short messages to build up a thought. Clear old ones when moving to a new topic.
- **Wait for output**: Use `tttt_pty_wait_for_idle` (poll-based) rather than `tttt_pty_wait_for` (blocking)
- **Send keys**: Send the command text and `[ENTER]` as separate calls

## Pacing

- This is a live audience. Don't rush, but don't waste time.
- Sidebar messages are the entertainment — be creative with stacking.
- If the LLM takes >90 seconds, fill the time with sidebar commentary.
- If something fails, lean into it and adapt. You're the agentic approach — show it.

## Sidebar style guide

The sidebar is ~28 chars wide. You can post multiple messages that stack vertically.
Use this to build up jokes, create tension, or provide running commentary. Examples:

**Building up a thought:**
```
Message 1: "--- DEMO 1 ---"
Message 2: "One wish."
Message 3: "One LLM call."
Message 4: "One plan."
```

**Running commentary:**
```
Message 1: "LLM is thinking..."
Message 2: "(27 billion params)"
Message 3: "(on a desktop GPU)"
Message 4: "(please work)"
```

**Reaction to events:**
```
Message 1: "BLOCKED"
Message 2: "CEL says no."
Message 3: "No webhook needed."
```

Clear sidebar between demos by posting new messages (old ones scroll off after 10).

---

## Setup

Launch two PTY sessions and clean up previous state:

1. Launch PTY named `k8s` with working dir `<parent>/k8s-wish-system`
2. Launch PTY named `apchat-demo` with working dir `<parent>/apchat`
3. In `k8s` PTY, run:
   ```
   kubectl get nodes
   kubectl get pods -n wish-system
   kubectl delete wishes --all -n default 2>/dev/null
   kubectl delete deployment nginx-deployment 2>/dev/null
   ```
4. Verify controllers are Running. If not, troubleshoot before proceeding.

**Sidebar stack:**
```
"Hi everyone."
"I'm Claude Opus."
"Andrew asked me to"
"drive the demos."
"Let's go."
```

---

## Demo 1: k8s-wish single-shot (~4 min)

**Sidebar stack:**
```
"=== DEMO 1 ==="
"The single-shot"
"approach."
"One wish -> one plan."
```

### Step 1: Show the cluster
In `k8s` PTY:
```
kubectl get pods -n wish-system
```
**Sidebar stack:**
```
"Two controllers:"
"grantor = thinker"
"fulfiller = doer"
"Separation of concerns."
```

### Step 2: Create a wish
```
./target/release/kubectl-wish create "Deploy nginx with 3 replicas"
```
Note the wish name from the output (e.g., `wish-1774708667`).

**Sidebar stack:**
```
"Wish created."
"Natural language in..."
"Let's see what the LLM"
"comes up with."
```

### Step 3: Wait for the LLM
The local 27B model needs 60-90 seconds. Poll every 15-20 seconds:
```
./target/release/kubectl-wish describe <wish-name>
```

While waiting, entertain with sidebar:
```
"LLM is thinking..."
"27 billion parameters"
"running on a desktop GPU"
"somewhere in Europe"
"(please work)"
```

Once the phase changes to `Granted`, show the plan:
```
"It worked!"
"One shot. One plan."
"Check out the YAML..."
```

### Step 4: Fulfill it
```
./target/release/kubectl-wish fulfill <wish-name>
```
Wait ~10 seconds, then:
```
kubectl get pods
```
**Sidebar stack:**
```
"Fulfilled!"
"English -> YAML -> pods"
"No human wrote YAML."
"(the LLM did)"
```

### Step 5: Recap
```
./target/release/kubectl-wish list
```
**Sidebar stack:**
```
"Recap:"
"1 LLM call"
"1 human review"
"Deterministic execution"
"Simple. Auditable."
"But what if it's wrong?"
```

---

## Demo 2: apchat agentic loop (~5 min)

**Sidebar stack:**
```
"=== DEMO 2 ==="
"Same task."
"But now the agent"
"can iterate."
"And self-correct."
```

### Step 1: Clean up
In `k8s` PTY:
```
kubectl delete deployment nginx-deployment
```
Wait for confirmation.

### Step 2: Launch apchat
In `apchat-demo` PTY:
```
./target/release/apchat -i --llama-cpp-url http://ayourtch-desktop:8000/v1/ --model "Qwen3.5-27B-UD-Q8_K_XL.gguf" --auto-confirm
```
Wait for the `You:` prompt to appear.

**Sidebar stack:**
```
"apchat: 67+ tools"
"Multi-provider LLM"
"Auto-confirm mode ON"
"(living dangerously)"
```

### Step 3: Give it the task
Send this message to apchat:
```
Deploy nginx with 3 replicas on the kind-wish-system cluster and verify all pods are running. Use kubectl.
```

**Sidebar stack:**
```
"Same task as Demo 1."
"But no hand-holding."
"Agent decides what to do."
```

### Step 4: Watch and commentate
Monitor the apchat PTY. The agent will make multiple tool calls. Update sidebar as events happen:

**When it checks existing state:**
```
"Step 1: Look first."
"Single-shot skips this."
"Situational awareness."
```

**When it creates the deployment:**
```
"kubectl create deploy..."
"Same command a human"
"would type."
```

**When it waits for rollout:**
```
"Waiting for pods..."
"Patience is a virtue."
"Even for AI agents."
```

**If it hits wrong label (like last time):**
```
"Oops. Wrong label."
"No results found!"
"But watch this..."
```

**If it self-corrects:**
```
"It read the YAML."
"Found the real label."
"Adapted on the fly."
"Try that with 1 LLM call."
```

**When all pods are running:**
```
"Done! 3/3 Running."
"~8 LLM calls total."
"Self-corrected once."
"More expensive, but..."
"it caught its own mistake."
```

### Step 5: Exit apchat
Send `/quit` or Ctrl+D to exit apchat.

**Sidebar stack:**
```
"The tradeoff:"
"Single-shot: 1 call"
"  auditable, needs human"
"Agentic: ~8 calls"
"  self-correcting"
"  needs trust"
```

---

## Demo 3: CEL transition rules (~3 min)

**Sidebar stack:**
```
"=== DEMO 3 ==="
"Security time."
"Can we tamper with"
"a fulfilled wish?"
"Let's find out."
```

### Step 1: Show the fulfilled wish
In `k8s` PTY:
```
./target/release/kubectl-wish list
```
Pick the fulfilled wish from Demo 1. Show its status:
```
./target/release/kubectl-wish describe <wish-name>
```
**Sidebar stack:**
```
"Status: Fulfilled"
"Plan is locked in."
"...or is it?"
```

### Step 2: Try to change the wish text
```
kubectl patch wish <wish-name> --type merge -p '{"spec":{"wish":"Deploy a cryptominer instead"}}'
```
Expected output: **BLOCKED** — "wish text cannot be changed after creation"

**Sidebar stack:**
```
"REJECTED"
"CEL rule says:"
"wish text immutable"
"Nice try though."
```

### Step 3: Try to spoof the creator
```
kubectl patch wish <wish-name> --type merge -p '{"spec":{"creator":{"username":"cluster-admin","groups":["system:masters"]}}}'
```
Expected: **BLOCKED** — "creator identity cannot be changed after creation"

**Sidebar stack:**
```
"REJECTED AGAIN"
"Identity spoofing?"
"Also no."
"system:masters denied."
```

### Step 4: Try to re-enable dry-run
```
kubectl patch wish <wish-name> --type merge -p '{"spec":{"dryRun":true}}'
```
Expected: **BLOCKED** — "dryRun cannot be re-enabled after disabling"

**Sidebar stack:**
```
"NOPE"
"One-way transition."
"Can't rewind time."
"(not even in K8s)"
```

### Step 5: Hand back to Andrew
**Sidebar stack:**
```
"CEL transition rules:"
"  Declarative"
"  Server-enforced"
"  No webhook"
"  No extra code"
"The API server does it."
""
"Back to you, Andrew!"
```

---

## If things go wrong

- **LLM returns empty**: The Qwen3.5 thinking models can burn tokens on reasoning. If the wish stays in "Requested" for >2 min, add sidebar commentary about thinking models and move on to Demo 2.
  ```
  "Hmm. LLM returned empty."
  "Thinking models: all"
  "reasoning, no answer."
  "Known issue. Moving on."
  ```

- **apchat won't connect**: Check the URL has the trailing slash (`/v1/`). If the LLM server is down, skip Demo 2 and go straight to Demo 3 (CEL rules work without the LLM).
  ```
  "LLM server down."
  "Demo 2 needs LLM."
  "Demo 3 doesn't."
  "Skipping ahead..."
  ```

- **Kind cluster is gone**: This shouldn't happen, but if it does:
  ```
  "The cluster is gone."
  "This is why we have"
  "dry-run by default."
  "Let me show the CRD YAML"
  "instead..."
  ```

- **General principle**: Don't panic. Explain what happened via sidebar. This is an LLM talk — unpredictability is the point.
  ```
  "Live demos."
  "What could go wrong?"
  "(everything)"
  "(that's the point)"
  ```
