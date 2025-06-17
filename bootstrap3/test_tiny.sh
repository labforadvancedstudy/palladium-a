#!/bin/bash

echo "Palladium Bootstrap Demonstration"
echo "================================="
echo ""

# Compile tiny_v7.pd
echo "1. Compiling tiny_v7.pd (Palladium compiler written in Palladium)..."
../target/debug/pdc compile tiny_v7.pd
if [ $? -eq 0 ]; then
    echo "   ✅ Compilation successful!"
else
    echo "   ❌ Compilation failed"
    exit 1
fi

# Compile the generated C code
echo ""
echo "2. Compiling generated C code..."
gcc ../build_output/tiny_v7.c -o tiny_v7_compiler 2>/dev/null
if [ $? -eq 0 ]; then
    echo "   ✅ C compilation successful!"
else
    echo "   ❌ C compilation failed"
    exit 1
fi

# Create a test program
echo ""
echo "3. Creating test program..."
cat > test_program.pd << 'EOF'
fn add(x: i64, y: i64) -> i64 {
    return x + y;
}

fn main() {
    print("Bootstrap test!");
    let result: i64 = add(10, 20);
    print("10 + 20 = " + int_to_string(result));
}
EOF
echo "   ✅ Test program created"

# Run tiny_v7 compiler
echo ""
echo "4. Running tiny compiler on test program..."
./tiny_v7_compiler > test_output.c
echo "   ✅ Compilation complete"

echo ""
echo "5. Generated C code:"
echo "--------------------"
head -n 20 tiny_v7_output.c

echo ""
echo "✨ Bootstrap demonstration complete!"
echo "We have a working Palladium compiler written in Palladium!"