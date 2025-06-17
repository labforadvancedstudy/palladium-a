#!/bin/bash
# Palladium Build Script

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default action
ACTION=${1:-compile}
FILE=${2:-}

# Check if Rust compiler exists
if [ ! -f "./target/release/pdc" ]; then
    echo -e "${YELLOW}Building Rust compiler first...${NC}"
    cargo build --release
fi

case $ACTION in
    compile)
        if [ -z "$FILE" ]; then
            echo -e "${RED}Error: No file specified${NC}"
            echo "Usage: ./build.sh compile <file.pd>"
            exit 1
        fi
        echo -e "${GREEN}Compiling $FILE...${NC}"
        ./target/release/pdc compile "$FILE"
        ;;
    
    test)
        echo -e "${GREEN}Running all tests...${NC}"
        cargo test
        # Add Palladium tests here
        ;;
    
    bootstrap)
        echo -e "${GREEN}Building bootstrap compiler...${NC}"
        cd bootstrap/v3_incremental
        ./build_minimal.sh
        cd ../..
        ;;
    
    clean)
        echo -e "${YELLOW}Cleaning build artifacts...${NC}"
        cargo clean
        rm -rf build_output/*
        rm -rf archive/build_outputs/*
        ;;
    
    *)
        echo "Usage: ./build.sh [compile|test|bootstrap|clean] [file.pd]"
        exit 1
        ;;
esac