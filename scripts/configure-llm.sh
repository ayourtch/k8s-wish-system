#!/bin/bash
# Interactive LLM configuration for wish-system

set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== Wish System LLM Configuration ===${NC}"
echo ""

# Detect namespace
NAMESPACE="${WISH_NAMESPACE:-default}"
echo -e "Configuration namespace: ${GREEN}${NAMESPACE}${NC}"
echo ""

# Check if kubectl wish is available
if [ ! -f "./target/release/kubectl-wish" ]; then
    echo -e "${YELLOW}Note: kubectl-wish binary not found in ./target/release/${NC}"
    echo -e "${YELLOW}You can configure manually after building with: ./target/release/kubectl-wish configure${NC}"
    echo ""
fi

# Detect common LLM endpoints
echo "Checking for common LLM services..."
DETECTED_ENDPOINTS=()

# Check for Ollama
if curl -s --connect-timeout 2 http://localhost:11434/api/tags &>/dev/null; then
    DETECTED_ENDPOINTS+=("Ollama (http://localhost:11434/v1)")
    echo -e "  ${GREEN}✓${NC} Ollama detected at http://localhost:11434"
fi

# Check for LM Studio
if curl -s --connect-timeout 2 http://localhost:1234/v1/models &>/dev/null; then
    DETECTED_ENDPOINTS+=("LM Studio (http://localhost:1234/v1)")
    echo -e "  ${GREEN}✓${NC} LM Studio detected at http://localhost:1234"
fi

echo ""

# Prompt for configuration method
echo "Configuration options:"
echo "  1) Use detected service (if any)"
echo "  2) OpenAI API"
echo "  3) Custom endpoint"
echo "  4) Skip (configure later)"
echo ""
read -p "Select option [1-4]: " CONFIG_OPTION

case $CONFIG_OPTION in
    1)
        if [ ${#DETECTED_ENDPOINTS[@]} -eq 0 ]; then
            echo -e "${RED}No services detected. Please choose another option.${NC}"
            exit 1
        fi

        if [ ${#DETECTED_ENDPOINTS[@]} -eq 1 ]; then
            SELECTED_SERVICE="${DETECTED_ENDPOINTS[0]}"
        else
            echo ""
            echo "Detected services:"
            for i in "${!DETECTED_ENDPOINTS[@]}"; do
                echo "  $((i+1))) ${DETECTED_ENDPOINTS[$i]}"
            done
            read -p "Select service [1-${#DETECTED_ENDPOINTS[@]}]: " SERVICE_IDX
            SELECTED_SERVICE="${DETECTED_ENDPOINTS[$((SERVICE_IDX-1))]}"
        fi

        if [[ $SELECTED_SERVICE == *"Ollama"* ]]; then
            ENDPOINT="http://localhost:11434/v1"
            echo ""
            echo "Available Ollama models:"
            curl -s http://localhost:11434/api/tags | grep -o '"name":"[^"]*"' | cut -d'"' -f4 | head -5
            echo ""
            read -p "Enter model name (e.g., llama3.2:latest): " MODEL
            API_KEY=""
        elif [[ $SELECTED_SERVICE == *"LM Studio"* ]]; then
            ENDPOINT="http://localhost:1234/v1"
            read -p "Enter model name: " MODEL
            API_KEY=""
        fi
        ;;

    2)
        ENDPOINT="https://api.openai.com/v1"
        read -p "Enter OpenAI API key: " API_KEY
        read -p "Enter model name (e.g., gpt-4): " MODEL
        ;;

    3)
        read -p "Enter LLM endpoint URL: " ENDPOINT
        read -p "Enter model name: " MODEL
        read -p "Enter API key (leave empty if not needed): " API_KEY
        ;;

    4)
        echo ""
        echo -e "${YELLOW}Skipping LLM configuration.${NC}"
        echo "You can configure later with:"
        echo "  ./target/release/kubectl-wish configure --endpoint <url> --model <name>"
        echo "  kubectl wish configure --endpoint <url> --model <name>"
        exit 0
        ;;

    *)
        echo -e "${RED}Invalid option${NC}"
        exit 1
        ;;
esac

# Apply configuration
echo ""
echo -e "${GREEN}Applying configuration...${NC}"
echo "  Endpoint: $ENDPOINT"
echo "  Model: $MODEL"
echo "  API Key: ${API_KEY:+***configured***}"

# Use kubectl wish if available, otherwise use kubectl directly
if [ -f "./target/release/kubectl-wish" ]; then
    if [ -n "$API_KEY" ]; then
        ./target/release/kubectl-wish configure -n "$NAMESPACE" \
            --endpoint "$ENDPOINT" \
            --model "$MODEL" \
            --api-key "$API_KEY"
    else
        ./target/release/kubectl-wish configure -n "$NAMESPACE" \
            --endpoint "$ENDPOINT" \
            --model "$MODEL"
    fi
else
    # Fallback to direct kubectl commands
    kubectl create configmap wish-grantor-config -n "$NAMESPACE" \
        --from-literal=llmEndpoint="$ENDPOINT" \
        --from-literal=llmModel="$MODEL" \
        --dry-run=client -o yaml | kubectl apply -f -

    if [ -n "$API_KEY" ]; then
        kubectl create secret generic llm-credentials -n "$NAMESPACE" \
            --from-literal=apiKey="$API_KEY" \
            --dry-run=client -o yaml | kubectl apply -f -

        kubectl patch configmap wish-grantor-config -n "$NAMESPACE" --type merge -p '{
            "data": {
                "credentialsSecretName": "llm-credentials",
                "credentialsSecretKey": "apiKey"
            }
        }'
    fi
fi

echo ""
echo -e "${GREEN}✓ LLM configuration complete!${NC}"
echo ""
echo "You can verify the configuration with:"
echo "  ./target/release/kubectl-wish configure --show"
echo "  kubectl wish configure --show"
