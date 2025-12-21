#!/bin/bash
set -e

echo "Building wish-grantor image..."
docker build --build-arg BINARY_NAME=wish-grantor -t wish-grantor:latest .

echo "Building wish-fulfiller image..."
docker build --build-arg BINARY_NAME=wish-fulfiller -t wish-fulfiller:latest .

echo "Build complete!"
echo ""
echo "To load into kind cluster:"
echo "  kind load docker-image wish-grantor:latest"
echo "  kind load docker-image wish-fulfiller:latest"
