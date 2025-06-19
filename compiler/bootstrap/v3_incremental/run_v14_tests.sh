#!/bin/bash
# Script to compile and run the v14 compiler test suite

echo "=== Building v14 compiler ==="
# First compile the v14 compiler itself
cargo run --bin palladium -- bootstrap/v3_incremental/simple_compiler_v14_complete.pd -o build_output/v14_compiler.c
if [ $? -ne 0 ]; then
    echo "Failed to compile v14 compiler to C"
    exit 1
fi

gcc -o build_output/v14_compiler build_output/v14_compiler.c
if [ $? -ne 0 ]; then
    echo "Failed to compile v14 compiler C code"
    exit 1
fi

echo ""
echo "=== Using v14 compiler to compile test suite ==="
# Use the v14 compiler to compile the test suite
./build_output/v14_compiler < bootstrap/v3_incremental/test_v14_compiler.pd > build_output/test_v14_output.c
if [ $? -ne 0 ]; then
    echo "Failed to compile test suite with v14 compiler"
    exit 1
fi

echo ""
echo "=== Compiling test suite C code ==="
gcc -o build_output/test_v14 build_output/test_v14_output.c -lm
if [ $? -ne 0 ]; then
    echo "Failed to compile test suite C code"
    exit 1
fi

echo ""
echo "=== Running test suite ==="
./build_output/test_v14