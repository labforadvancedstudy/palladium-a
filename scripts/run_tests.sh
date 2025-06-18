#!/bin/bash

# Palladium Test Runner Script
# Automated test execution for all Palladium test suites

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
PALLADIUM_BIN="${PALLADIUM_BIN:-./target/release/pdc}"
TEST_DIR="${TEST_DIR:-tests}"
OUTPUT_DIR="${OUTPUT_DIR:-test_output}"
VERBOSE="${VERBOSE:-0}"
FILTER="${FILTER:-}"

# Counters
TOTAL=0
PASSED=0
FAILED=0
SKIPPED=0

# Ensure output directory exists
mkdir -p "$OUTPUT_DIR"

# Print colored message
print_colored() {
    local color=$1
    local message=$2
    echo -e "${color}${message}${NC}"
}

# Print test header
print_header() {
    print_colored "$BLUE" "==================================="
    print_colored "$BLUE" "    Palladium Test Runner"
    print_colored "$BLUE" "==================================="
    echo
}

# Check if Palladium compiler exists
check_compiler() {
    if [ ! -f "$PALLADIUM_BIN" ]; then
        print_colored "$RED" "Error: Palladium compiler not found at $PALLADIUM_BIN"
        print_colored "$YELLOW" "Build it with: cargo build --release"
        exit 1
    fi
}

# Extract test metadata from file
extract_metadata() {
    local file=$1
    local key=$2
    grep "^//@ $key:" "$file" 2>/dev/null | sed "s/^\/\/@ $key: *//" || echo ""
}

# Run a single test
run_test() {
    local test_file=$1
    local test_name=$(basename "$test_file" .pd)
    
    # Apply filter if set
    if [ -n "$FILTER" ] && [[ ! "$test_name" =~ $FILTER ]]; then
        return 0
    fi
    
    ((TOTAL++))
    
    # Extract metadata
    local description=$(extract_metadata "$test_file" "description")
    local should_fail=$(extract_metadata "$test_file" "should-fail")
    local compile_only=$(extract_metadata "$test_file" "compile-only")
    local expected_output=$(extract_metadata "$test_file" "expect-output")
    local expected_exit=$(extract_metadata "$test_file" "expect-exit-code")
    
    # Default values
    [ -z "$expected_exit" ] && expected_exit=0
    
    # Print test info
    echo -n "Testing $test_name... "
    [ -n "$description" ] && [ "$VERBOSE" = "1" ] && echo -e "\n  Description: $description"
    
    # Compile test
    local c_output="$OUTPUT_DIR/${test_name}.c"
    local exe_output="$OUTPUT_DIR/${test_name}"
    local stdout_file="$OUTPUT_DIR/${test_name}.stdout"
    local stderr_file="$OUTPUT_DIR/${test_name}.stderr"
    
    # Run Palladium compiler
    if $PALLADIUM_BIN compile "$test_file" 2> "$stderr_file"; then
        local compile_result=0
        # Copy the generated C file from build_output
        local generated_c="build_output/${test_name}.c"
        if [ -f "$generated_c" ]; then
            cp "$generated_c" "$c_output"
        else
            compile_result=1
        fi
    else
        local compile_result=$?
    fi
    
    # Check compilation result
    if [ "$should_fail" = "true" ]; then
        if [ $compile_result -ne 0 ]; then
            print_colored "$GREEN" "✓ (expected compilation failure)"
            ((PASSED++))
            return 0
        else
            print_colored "$RED" "✗ (expected compilation to fail but it succeeded)"
            ((FAILED++))
            return 1
        fi
    elif [ $compile_result -ne 0 ]; then
        print_colored "$RED" "✗ (compilation failed)"
        [ "$VERBOSE" = "1" ] && cat "$stderr_file"
        ((FAILED++))
        return 1
    fi
    
    # If compile-only, we're done
    if [ "$compile_only" = "true" ]; then
        print_colored "$GREEN" "✓ (compile-only)"
        ((PASSED++))
        return 0
    fi
    
    # Compile C to executable
    if ! gcc "$c_output" -o "$exe_output" 2> "$stderr_file"; then
        print_colored "$RED" "✗ (C compilation failed)"
        [ "$VERBOSE" = "1" ] && cat "$stderr_file"
        ((FAILED++))
        return 1
    fi
    
    # Run the executable
    "$exe_output" > "$stdout_file" 2> "$stderr_file"
    local exit_code=$?
    
    # Check exit code
    if [ "$exit_code" -ne "$expected_exit" ]; then
        print_colored "$RED" "✗ (exit code $exit_code, expected $expected_exit)"
        ((FAILED++))
        return 1
    fi
    
    # Check expected output if specified
    if [ -n "$expected_output" ] && [ -f "$expected_output" ]; then
        if ! diff -q "$stdout_file" "$expected_output" > /dev/null; then
            print_colored "$RED" "✗ (output mismatch)"
            [ "$VERBOSE" = "1" ] && diff "$stdout_file" "$expected_output"
            ((FAILED++))
            return 1
        fi
    fi
    
    print_colored "$GREEN" "✓"
    ((PASSED++))
    return 0
}

# Run tests in a directory
run_test_suite() {
    local suite_name=$1
    local suite_dir=$2
    
    print_colored "$BLUE" "\nRunning $suite_name..."
    echo
    
    # Find all .pd files in the directory
    while IFS= read -r test_file; do
        run_test "$test_file"
    done < <(find "$suite_dir" -name "*.pd" -type f | sort)
}

# Run bootstrap tests
run_bootstrap_tests() {
    print_colored "$BLUE" "\nRunning Bootstrap Tests..."
    echo
    
    # Test tiny compiler self-hosting
    if [ -f "bootstrap/v3_incremental/tiny_v16.pd" ]; then
        echo -n "Testing tiny compiler self-hosting... "
        
        # Compile tiny compiler
        if $PALLADIUM_BIN compile "bootstrap/v3_incremental/tiny_v16.pd" 2>/dev/null &&
           cp "build_output/tiny_v16.c" "$OUTPUT_DIR/tiny_compiler.c" &&
           gcc "$OUTPUT_DIR/tiny_compiler.c" -o "$OUTPUT_DIR/tiny_compiler"; then
            
            # Test self-compilation
            if "$OUTPUT_DIR/tiny_compiler" "bootstrap/v3_incremental/tiny_self_test.pd" "$OUTPUT_DIR/tiny_self_test.c" &&
               gcc "$OUTPUT_DIR/tiny_self_test.c" -o "$OUTPUT_DIR/tiny_self_test" &&
               "$OUTPUT_DIR/tiny_self_test" > /dev/null 2>&1; then
                print_colored "$GREEN" "✓"
                ((PASSED++))
            else
                print_colored "$RED" "✗ (self-compilation failed)"
                ((FAILED++))
            fi
        else
            print_colored "$RED" "✗ (compilation failed)"
            ((FAILED++))
        fi
        ((TOTAL++))
    fi
}

# Generate test report
generate_report() {
    local success_rate=0
    [ $TOTAL -gt 0 ] && success_rate=$((PASSED * 100 / TOTAL))
    
    echo
    print_colored "$BLUE" "==================================="
    print_colored "$BLUE" "         Test Summary"
    print_colored "$BLUE" "==================================="
    echo
    echo "Total tests: $TOTAL"
    print_colored "$GREEN" "Passed: $PASSED"
    [ $FAILED -gt 0 ] && print_colored "$RED" "Failed: $FAILED"
    [ $SKIPPED -gt 0 ] && print_colored "$YELLOW" "Skipped: $SKIPPED"
    echo
    echo "Success rate: ${success_rate}%"
    
    # Generate JSON report
    cat > "$OUTPUT_DIR/test_report.json" << EOF
{
  "total": $TOTAL,
  "passed": $PASSED,
  "failed": $FAILED,
  "skipped": $SKIPPED,
  "success_rate": $success_rate
}
EOF
    
    # Generate JUnit XML report
    cat > "$OUTPUT_DIR/test_report.xml" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<testsuites tests="$TOTAL" failures="$FAILED" skipped="$SKIPPED">
  <testsuite name="Palladium Tests" tests="$TOTAL" failures="$FAILED" skipped="$SKIPPED">
  </testsuite>
</testsuites>
EOF
    
    echo
    echo "Reports generated in $OUTPUT_DIR/"
}

# Main execution
main() {
    print_header
    check_compiler
    
    # Run different test suites
    run_test_suite "Unit Tests" "$TEST_DIR"
    run_test_suite "Integration Tests" "$TEST_DIR/integration"
    run_test_suite "Example Programs" "examples/basic"
    run_test_suite "Standard Library Tests" "examples/stdlib"
    run_bootstrap_tests
    
    # Generate report
    generate_report
    
    # Exit with appropriate code
    if [ $FAILED -eq 0 ]; then
        print_colored "$GREEN" "\nAll tests passed! 🎉"
        exit 0
    else
        print_colored "$RED" "\nSome tests failed. Please check the output above."
        exit 1
    fi
}

# Handle script arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -v|--verbose)
            VERBOSE=1
            shift
            ;;
        -f|--filter)
            FILTER="$2"
            shift 2
            ;;
        -o|--output-dir)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        -b|--binary)
            PALLADIUM_BIN="$2"
            shift 2
            ;;
        -h|--help)
            echo "Usage: $0 [options]"
            echo "Options:"
            echo "  -v, --verbose       Show verbose output"
            echo "  -f, --filter REGEX  Only run tests matching regex"
            echo "  -o, --output-dir    Set output directory (default: test_output)"
            echo "  -b, --binary PATH   Path to Palladium compiler"
            echo "  -h, --help          Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Run main
main