#!/bin/bash
# Benchmark runner for Palladium
# Compares performance with C

echo "=== Palladium Performance Benchmarks ==="
echo

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

# Check if palladium compiler exists
PDC="../target/release/pdc"
if [ ! -f "$PDC" ]; then
    echo -e "${RED}Error: Palladium compiler not found at $PDC${NC}"
    echo "Please run 'cargo build --release' first"
    exit 1
fi

# Function to run benchmark
run_benchmark() {
    local name=$1
    local pd_file="palladium/${name}.pd"
    local c_file="c/${name}.c"
    
    echo -e "${YELLOW}Running benchmark: $name${NC}"
    echo "================================"
    
    # Compile and time C version
    if [ -f "$c_file" ]; then
        echo "C version:"
        gcc -O3 "$c_file" -o "c/${name}_c" 2>/dev/null
        if [ $? -eq 0 ]; then
            C_TIME=$( { time ./c/${name}_c > /dev/null; } 2>&1 | grep real | awk '{print $2}' )
            echo -e "  Execution time: ${GREEN}$C_TIME${NC}"
        else
            echo -e "  ${RED}Compilation failed${NC}"
        fi
    fi
    
    # Compile and time Palladium version (C backend)
    if [ -f "$pd_file" ]; then
        echo
        echo "Palladium version (C backend):"
        $PDC compile "$pd_file" -o "palladium/${name}_pd" 2>/dev/null
        if [ $? -eq 0 ]; then
            # Compile generated C code
            gcc -O3 "../build_output/${name}.c" -o "palladium/${name}_pd" 2>/dev/null
            if [ $? -eq 0 ]; then
                PD_TIME=$( { time ./palladium/${name}_pd > /dev/null; } 2>&1 | grep real | awk '{print $2}' )
                echo -e "  Execution time: ${GREEN}$PD_TIME${NC}"
            else
                echo -e "  ${RED}C compilation failed${NC}"
            fi
        else
            echo -e "  ${RED}Palladium compilation failed${NC}"
        fi
    fi
    
    # Compile and time Palladium version (LLVM backend)
    if [ -f "$pd_file" ]; then
        echo
        echo "Palladium version (LLVM backend):"
        $PDC compile "$pd_file" --llvm 2>/dev/null
        if [ $? -eq 0 ]; then
            # Check if LLVM tools are available
            if command -v llc &> /dev/null; then
                llc -O3 "../build_output/${name}.ll" -o "../build_output/${name}.o" 2>/dev/null
                if [ $? -eq 0 ]; then
                    gcc "../build_output/${name}.o" -o "palladium/${name}_llvm" 2>/dev/null
                    if [ $? -eq 0 ]; then
                        LLVM_TIME=$( { time ./palladium/${name}_llvm > /dev/null; } 2>&1 | grep real | awk '{print $2}' )
                        echo -e "  Execution time: ${GREEN}$LLVM_TIME${NC}"
                    else
                        echo -e "  ${RED}Linking failed${NC}"
                    fi
                else
                    echo -e "  ${RED}LLVM compilation failed${NC}"
                fi
            else
                echo -e "  ${YELLOW}LLVM tools not found (skipping)${NC}"
            fi
        else
            echo -e "  ${RED}Palladium compilation failed${NC}"
        fi
    fi
    
    echo
}

# Run benchmarks
echo "Running all benchmarks..."
echo

run_benchmark "fibonacci"
run_benchmark "matrix_multiply"
run_benchmark "string_concat"
run_benchmark "bubble_sort"

# Summary
echo "=== Benchmark Summary ==="
echo
echo "Note: Lower times are better"
echo "Goal: Palladium performance within 10% of C"
echo
echo "To add more benchmarks:"
echo "1. Create benchmark in palladium/ and c/ directories"
echo "2. Run this script again"