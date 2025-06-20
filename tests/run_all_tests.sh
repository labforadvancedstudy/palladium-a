#!/bin/bash
# Run all Palladium language tests

echo "=== Running All Palladium Language Tests ==="
echo ""

# Color codes
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Counter
PASSED=0
FAILED=0

# Function to run a test
run_test() {
    local test_file=$1
    local test_name=$(basename "$test_file" .pd)
    
    echo -n "Running $test_name... "
    
    if ./target/release/pdc run "$test_file" > /tmp/test_output.txt 2>&1; then
        echo -e "${GREEN}PASSED${NC}"
        ((PASSED++))
    else
        echo -e "${RED}FAILED${NC}"
        echo "Error output:"
        tail -10 /tmp/test_output.txt
        echo ""
        ((FAILED++))
    fi
}

# Change to project root
cd "$(dirname "$0")/.."

# Run all tests in order
for test in tests/*.pd; do
    if [ -f "$test" ]; then
        run_test "$test"
    fi
done

echo ""
echo "=== Test Summary ==="
echo -e "Passed: ${GREEN}$PASSED${NC}"
echo -e "Failed: ${RED}$FAILED${NC}"

if [ $FAILED -eq 0 ]; then
    echo -e "\n${GREEN}All tests passed!${NC}"
    exit 0
else
    echo -e "\n${RED}Some tests failed!${NC}"
    exit 1
fi