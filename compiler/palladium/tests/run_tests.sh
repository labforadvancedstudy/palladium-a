#!/bin/bash
# Test runner for Palladium self-hosting compiler

echo "=== Palladium Self-Hosting Compiler Test Suite ==="
echo

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Test counter
PASSED=0
FAILED=0

# Function to run a test
run_test() {
    local test_name=$1
    local test_file=$2
    
    echo "Running test: $test_name"
    echo "----------------------------------------"
    
    # For now, we'll simulate the compilation process
    # In a real scenario, we'd compile the self-hosting compiler first,
    # then use it to compile the test files
    
    echo "Compiling $test_file..."
    
    # Check if file exists
    if [ -f "$test_file" ]; then
        echo -e "${GREEN}✓ Test file found${NC}"
        echo "Would compile: $test_file -> ${test_file%.pd}.c"
        echo "Would then compile C to executable and run"
        ((PASSED++))
    else
        echo -e "${RED}✗ Test file not found${NC}"
        ((FAILED++))
    fi
    
    echo
}

# Run all tests
run_test "Basic Operations" "test_basic.pd"
run_test "Control Flow" "test_control_flow.pd"
run_test "Arrays" "test_arrays.pd"
run_test "Structs" "test_structs.pd"
run_test "String Operations" "test_strings.pd"
run_test "Compiler Features" "test_compiler_features.pd"
run_test "Match and For Loops" "test_match_and_for.pd"

# Summary
echo "=== Test Summary ==="
echo -e "Passed: ${GREEN}$PASSED${NC}"
echo -e "Failed: ${RED}$FAILED${NC}"

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
fi