#!/bin/bash
# Script to compile LLVM IR to native executable
# Usage: ./compile_llvm.sh <file.ll>

if [ $# -eq 0 ]; then
    echo "Usage: $0 <file.ll>"
    exit 1
fi

INPUT=$1
BASENAME=$(basename "$INPUT" .ll)
OUTPUT_DIR="build_output"

# Check if LLVM tools are available
if ! command -v llc &> /dev/null; then
    echo "Error: llc not found. Please install LLVM tools."
    echo "On macOS: brew install llvm"
    echo "On Ubuntu: apt-get install llvm"
    exit 1
fi

if ! command -v clang &> /dev/null; then
    echo "Error: clang not found. Please install clang."
    exit 1
fi

echo "Compiling $INPUT..."

# Step 1: Compile LLVM IR to assembly
echo "  Generating assembly..."
llc -filetype=asm -o "$OUTPUT_DIR/$BASENAME.s" "$INPUT" || exit 1

# Step 2: Compile LLVM IR to object file
echo "  Generating object file..."
llc -filetype=obj -o "$OUTPUT_DIR/$BASENAME.o" "$INPUT" || exit 1

# Step 3: Link with C runtime
echo "  Linking..."
clang -o "$OUTPUT_DIR/$BASENAME" "$OUTPUT_DIR/$BASENAME.o" -lm || exit 1

echo "Success! Executable created: $OUTPUT_DIR/$BASENAME"
echo ""
echo "To run: ./$OUTPUT_DIR/$BASENAME"