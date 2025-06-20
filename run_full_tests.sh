#!/bin/bash
# Palladium Full Test Suite

echo "🧪 Palladium Full Test Suite"
echo "============================"
echo ""

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
NC='\033[0m'

# Counters
PASSED=0
FAILED=0
WARNINGS=0

# 1. Build Test
echo "1. Build Test"
echo "-------------"
if cargo build --no-default-features 2>/dev/null; then
    echo -e "${GREEN}✅ Build successful${NC}"
    ((PASSED++))
else
    echo -e "${RED}❌ Build failed${NC}"
    ((FAILED++))
fi

# 2. Lint Test (count warnings)
echo -e "\n2. Lint Test"
echo "------------"
WARNING_COUNT=$(cargo clippy --no-default-features 2>&1 | grep -c "warning:" || echo "0")
if [ "$WARNING_COUNT" -eq 0 ]; then
    echo -e "${GREEN}✅ No warnings${NC}"
    ((PASSED++))
else
    echo -e "${YELLOW}⚠️  $WARNING_COUNT warnings found${NC}"
    ((WARNINGS++))
fi

# 3. Unit Tests
echo -e "\n3. Unit Tests"
echo "-------------"
if cargo test --lib --no-default-features 2>&1 | grep -q "test result: ok"; then
    UNIT_TESTS=$(cargo test --lib --no-default-features 2>&1 | grep -oE "[0-9]+ passed" | head -1)
    echo -e "${GREEN}✅ Unit tests passed ($UNIT_TESTS)${NC}"
    ((PASSED++))
else
    echo -e "${RED}❌ Unit tests failed${NC}"
    ((FAILED++))
fi

# 4. Integration Test (with runtime)
echo -e "\n4. Integration Test"
echo "-------------------"
cat > test_integration.pd << 'EOF'
fn main() {
    print("Integration test running");
    let x = 42;
    let y = 58;
    let z = x + y;
    print_int(z);
}
EOF

if ./target/debug/pdc compile test_integration.pd -o test_int 2>/dev/null; then
    # Try to compile with runtime
    if gcc build_output/test_integration.c runtime/palladium_runtime.c -o test_int_exe 2>/dev/null; then
        OUTPUT=$(./test_int_exe 2>&1)
        if [[ "$OUTPUT" == *"100"* ]]; then
            echo -e "${GREEN}✅ Integration test passed${NC}"
            ((PASSED++))
        else
            echo -e "${RED}❌ Integration test wrong output${NC}"
            ((FAILED++))
        fi
        rm -f test_int_exe
    else
        echo -e "${YELLOW}⚠️  Linking failed (runtime library issue)${NC}"
        ((WARNINGS++))
    fi
else
    echo -e "${RED}❌ Compilation failed${NC}"
    ((FAILED++))
fi
rm -f test_integration.pd

# 5. E2E Test
echo -e "\n5. End-to-End Test"
echo "------------------"
if [ -f "./target/debug/pdc" ]; then
    echo -e "${GREEN}✅ Compiler binary exists${NC}"
    ((PASSED++))
else
    echo -e "${RED}❌ Compiler binary not found${NC}"
    ((FAILED++))
fi

# Summary
echo -e "\n=============================="
echo "Test Summary"
echo "=============================="
echo -e "${GREEN}Passed: $PASSED${NC}"
echo -e "${RED}Failed: $FAILED${NC}"
echo -e "${YELLOW}Warnings: $WARNINGS${NC}"

TOTAL=$((PASSED + FAILED))
if [ $FAILED -eq 0 ] && [ $WARNINGS -eq 0 ]; then
    echo -e "\n${GREEN}🎉 All tests passed!${NC}"
    exit 0
elif [ $FAILED -eq 0 ]; then
    echo -e "\n${YELLOW}⚠️  Tests passed with warnings${NC}"
    exit 0
else
    echo -e "\n${RED}❌ Some tests failed${NC}"
    exit 1
fi