#!/bin/bash
# User-friendly installation script for wish-system

set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo -e "${BLUE}"
cat << "EOF"
 _       ___       __       _____            __
| |     / (_)____ / /_     / ___/__  _______/ /____  ____ ___
| | /| / / / ___// __ \    \__ \/ / / / ___/ __/ _ \/ __ `__ \
| |/ |/ / (__  )/ / / /   ___/ / /_/ (__  ) /_/  __/ / / / / /
|__/|__/_/____//_/ /_/   /____/\__, /____/\__/\___/_/ /_/ /_/
                              /____/
EOF
echo -e "${NC}"

echo -e "${GREEN}Natural Language Infrastructure Management for Kubernetes${NC}"
echo ""

# Check prerequisites
echo "Checking prerequisites..."
MISSING_DEPS=()

if ! command -v kubectl &> /dev/null; then
    MISSING_DEPS+=("kubectl")
fi

if ! command -v docker &> /dev/null; then
    MISSING_DEPS+=("docker")
fi

if [ ${#MISSING_DEPS[@]} -ne 0 ]; then
    echo -e "${RED}Missing required dependencies: ${MISSING_DEPS[*]}${NC}"
    echo ""
    echo "Please install the missing dependencies and try again."
    exit 1
fi

echo -e "${GREEN}✓ All prerequisites met${NC}"
echo ""

# Check if cluster is accessible
if ! kubectl cluster-info &> /dev/null; then
    echo -e "${RED}Cannot connect to Kubernetes cluster${NC}"
    echo ""
    echo "Please ensure you have a running Kubernetes cluster and valid kubeconfig."
    echo ""
    read -p "Would you like to create a local Kind cluster? [y/N]: " CREATE_KIND
    if [[ $CREATE_KIND =~ ^[Yy]$ ]]; then
        if ! command -v kind &> /dev/null; then
            echo -e "${RED}Kind is not installed${NC}"
            echo "Install from: https://kind.sigs.k8s.io/docs/user/quick-start/#installation"
            exit 1
        fi
        cd "$PROJECT_ROOT"
        make kind-cluster
        echo ""
    else
        exit 1
    fi
fi

# Get cluster info
CLUSTER_NAME=$(kubectl config current-context)
echo -e "Installing to cluster: ${GREEN}${CLUSTER_NAME}${NC}"
echo ""

# Choose installation method
echo "Installation method:"
echo "  1) Install from manifests (recommended for production)"
echo "  2) Build and install from source (requires Rust)"
echo ""
read -p "Select option [1-2]: " INSTALL_METHOD

case $INSTALL_METHOD in
    1)
        # Install from manifests
        echo ""
        echo -e "${GREEN}Installing wish-system from manifests...${NC}"
        cd "$PROJECT_ROOT"
        kubectl apply -f k8s/install.yaml

        # Wait for CRD to be established
        echo ""
        echo "Waiting for CRD to be established..."
        kubectl wait --for condition=established --timeout=60s crd/wishes.wish.ayourt.ch

        # Deploy controllers
        echo ""
        echo "Deploying controllers..."

        # Check if runtime image exists or needs to be built
        if [[ $CLUSTER_NAME == *"kind"* ]]; then
            echo "Detected Kind cluster - building and loading image..."
            make build-runtime
            make kind-load-runtime
        else
            echo -e "${YELLOW}Note: For non-Kind clusters, you need to build and push the image to a registry${NC}"
            echo ""
            read -p "Enter container registry (e.g., docker.io/username): " REGISTRY
            if [ -n "$REGISTRY" ]; then
                docker build -t "$REGISTRY/wish-system-runtime:latest" -f Dockerfile.runtime .
                docker push "$REGISTRY/wish-system-runtime:latest"
                # Update deployment image
                kubectl set image deployment/wish-grantor wish-grantor="$REGISTRY/wish-system-runtime:latest" -n wish-system
                kubectl set image deployment/wish-fulfiller wish-fulfiller="$REGISTRY/wish-system-runtime:latest" -n wish-system
            fi
        fi

        kubectl apply -f k8s/deployments-runtime.yaml
        ;;

    2)
        # Build from source
        echo ""
        if ! command -v cargo &> /dev/null; then
            echo -e "${RED}Rust/Cargo is not installed${NC}"
            echo "Install from: https://rustup.rs/"
            exit 1
        fi

        echo -e "${GREEN}Building from source...${NC}"
        cd "$PROJECT_ROOT"
        cargo build --release --bins

        echo ""
        echo -e "${GREEN}Installing wish-system...${NC}"

        if [[ $CLUSTER_NAME == *"kind"* ]]; then
            make kind-load-runtime
            make install-all
            kubectl apply -f k8s/deployments-runtime.yaml
        else
            kubectl apply -f k8s/install.yaml
            kubectl wait --for condition=established --timeout=60s crd/wishes.wish.ayourt.ch

            echo ""
            read -p "Enter container registry (e.g., docker.io/username): " REGISTRY
            if [ -n "$REGISTRY" ]; then
                make build-runtime
                docker tag wish-system-runtime:latest "$REGISTRY/wish-system-runtime:latest"
                docker push "$REGISTRY/wish-system-runtime:latest"
                kubectl set image deployment/wish-grantor wish-grantor="$REGISTRY/wish-system-runtime:latest" -n wish-system
                kubectl set image deployment/wish-fulfiller wish-fulfiller="$REGISTRY/wish-system-runtime:latest" -n wish-system
            fi

            kubectl apply -f k8s/deployments-runtime.yaml
        fi
        ;;

    *)
        echo -e "${RED}Invalid option${NC}"
        exit 1
        ;;
esac

# Wait for controllers to be ready
echo ""
echo "Waiting for controllers to be ready..."
kubectl wait --for=condition=available --timeout=120s deployment/wish-grantor -n wish-system || true
kubectl wait --for=condition=available --timeout=120s deployment/wish-fulfiller -n wish-system || true

echo ""
echo -e "${GREEN}✓ Wish-system installed successfully!${NC}"
echo ""

# Configure LLM
echo -e "${YELLOW}=== LLM Configuration ===${NC}"
echo ""
echo "The wish-system requires an LLM to function."
echo ""
read -p "Would you like to configure the LLM now? [Y/n]: " CONFIGURE_LLM

if [[ ! $CONFIGURE_LLM =~ ^[Nn]$ ]]; then
    "$SCRIPT_DIR/configure-llm.sh"
else
    echo ""
    echo -e "${YELLOW}Skipping LLM configuration.${NC}"
    echo ""
    echo "You can configure it later with:"
    echo "  kubectl wish configure --endpoint <url> --model <name>"
    echo "  scripts/configure-llm.sh"
fi

echo ""
echo -e "${GREEN}=== Installation Complete ===${NC}"
echo ""
echo "Next steps:"
echo ""
echo "1. Verify installation:"
echo -e "   ${BLUE}kubectl get pods -n wish-system${NC}"
echo ""
echo "2. Create your first wish:"
echo -e "   ${BLUE}kubectl wish create \"Deploy nginx web server\"${NC}"
echo ""
echo "3. List wishes:"
echo -e "   ${BLUE}kubectl wish list${NC}"
echo ""
echo "4. Configure kubectl wish plugin (optional):"
if [ -f "$PROJECT_ROOT/target/release/kubectl-wish" ]; then
    echo -e "   ${BLUE}sudo cp $PROJECT_ROOT/target/release/kubectl-wish /usr/local/bin/${NC}"
else
    echo -e "   ${BLUE}cargo build --release --bin kubectl-wish${NC}"
    echo -e "   ${BLUE}sudo cp target/release/kubectl-wish /usr/local/bin/${NC}"
fi
echo ""
echo "Documentation:"
echo "  - Installation Guide: INSTALL.md"
echo "  - Deployment Guide: DEPLOYMENT.md"
echo "  - README: README.md"
echo ""
