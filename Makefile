.PHONY: build build-images install-crd install-rbac install-config deploy clean test

# Build Rust binaries
build:
	cargo build --release

# Build Docker images
build-images:
	./build-images.sh

# Install CRD
install-crd:
	kubectl apply -f k8s/crd.yaml

# Install RBAC
install-rbac:
	kubectl apply -f k8s/rbac-grantor.yaml
	kubectl apply -f k8s/rbac-fulfiller.yaml

# Install configuration
install-config:
	kubectl apply -f k8s/config.yaml

# Deploy controllers
deploy: install-crd install-rbac install-config
	kubectl apply -f k8s/deployments.yaml

# Install kubectl plugin
install-plugin: build
	sudo cp target/release/kubectl-wish /usr/local/bin/

# Full installation
install: build build-images deploy install-plugin
	@echo "Installation complete!"
	@echo "Verify with: kubectl get wishes"

# Clean Kubernetes resources
clean:
	kubectl delete -f k8s/deployments.yaml || true
	kubectl delete -f k8s/config.yaml || true
	kubectl delete -f k8s/rbac-fulfiller.yaml || true
	kubectl delete -f k8s/rbac-grantor.yaml || true
	kubectl delete -f k8s/crd.yaml || true

# Run tests
test:
	cargo test

# Run wish-grantor locally
run-grantor:
	RUST_LOG=info cargo run --bin wish-grantor

# Run wish-fulfiller locally
run-fulfiller:
	RUST_LOG=info cargo run --bin wish-fulfiller

# Check controller logs
logs-grantor:
	kubectl logs -l app=wish-grantor -f

logs-fulfiller:
	kubectl logs -l app=wish-fulfiller -f

# Apply example wishes
examples:
	kubectl apply -f k8s/examples.yaml

# Kind cluster helpers
kind-cluster:
	kind create cluster --name wish-system

kind-load: build-images
	kind load docker-image wish-grantor:latest --name wish-system
	kind load docker-image wish-fulfiller:latest --name wish-system

kind-delete:
	kind delete cluster --name wish-system
