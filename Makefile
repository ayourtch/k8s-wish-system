.PHONY: build build-images install-crd install-rbac install-config deploy clean test generate-install install-all uninstall-all

# Build Rust binaries
build:
	cargo build --release

# Build Docker images (separate images for each controller)
build-images:
	./build-images.sh

# Build unified runtime image (both controllers in one image)
build-runtime:
	docker build -t wish-system-runtime:latest -f Dockerfile.runtime .

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

# Generate all-in-one installation manifest
generate-install:
	@echo "# Wish System - All-in-One Installation Manifest" > k8s/install.yaml
	@echo "# This file installs the complete wish-system in the wish-system namespace" >> k8s/install.yaml
	@echo "#" >> k8s/install.yaml
	@echo "# Installation:" >> k8s/install.yaml
	@echo "#   kubectl apply -f https://raw.githubusercontent.com/YOUR_USERNAME/wish-system/main/k8s/install.yaml" >> k8s/install.yaml
	@echo "#" >> k8s/install.yaml
	@echo "# Or locally:" >> k8s/install.yaml
	@echo "#   kubectl apply -f k8s/install.yaml" >> k8s/install.yaml
	@echo "#" >> k8s/install.yaml
	@echo "# To uninstall:" >> k8s/install.yaml
	@echo "#   kubectl delete -f k8s/install.yaml" >> k8s/install.yaml
	@echo "#" >> k8s/install.yaml
	@echo "# ============================================================================" >> k8s/install.yaml
	@cat k8s/namespace.yaml >> k8s/install.yaml
	@echo "---" >> k8s/install.yaml
	@cat k8s/crd.yaml >> k8s/install.yaml
	@echo "---" >> k8s/install.yaml
	@cat k8s/rbac-grantor.yaml >> k8s/install.yaml
	@echo "---" >> k8s/install.yaml
	@cat k8s/rbac-fulfiller.yaml >> k8s/install.yaml
	@echo "---" >> k8s/install.yaml
	@cat k8s/config.yaml >> k8s/install.yaml
	@echo "Generated k8s/install.yaml from component files"

# Install using all-in-one manifest (without controllers for existing clusters)
install-all:
	kubectl apply -f k8s/install.yaml

# Uninstall everything including namespace
uninstall-all:
	kubectl delete -f k8s/install.yaml || true

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

kind-load-runtime: build-runtime
	kind load docker-image wish-system-runtime:latest --name wish-system

kind-delete:
	kind delete cluster --name wish-system

# Complete kind deployment with runtime image
kind-deploy: kind-cluster kind-load-runtime install-all
	kubectl apply -f k8s/deployments-runtime.yaml
	@echo "Waiting for controllers to start..."
	kubectl wait --for=condition=available --timeout=60s deployment/wish-grantor -n wish-system || true
	kubectl wait --for=condition=available --timeout=60s deployment/wish-fulfiller -n wish-system || true
	@echo ""
	@echo "Wish system deployed successfully!"
	@echo "Check status with: kubectl get pods -n wish-system"
