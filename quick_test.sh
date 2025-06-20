#!/bin/bash
# Quick test script for Palladium

echo "🧪 Palladium Quick Test"
echo "====================="

# Test 1: Simple program
echo -e "\n1️⃣ Testing simple program..."
cat > test1.pd << 'EOF'
fn main() {
    print("Test 1 passed!");
    print_int(42);
}
EOF

./target/debug/pdc compile test1.pd -o test1 > /dev/null 2>&1
if [ $? -eq 0 ]; then
    gcc build_output/test1.c runtime/palladium_runtime.c -o test1_exe 2>/dev/null
    if [ $? -eq 0 ]; then
        ./test1_exe
        echo "✅ Simple program test passed"
    else
        echo "❌ Linking failed"
    fi
else
    echo "❌ Compilation failed"
fi

# Test 2: Variables and arithmetic
echo -e "\n2️⃣ Testing variables..."
cat > test2.pd << 'EOF'
fn main() {
    let a = 10;
    let b = 20;
    let c = a + b;
    print("a + b = ");
    print_int(c);
}
EOF

./target/debug/pdc compile test2.pd -o test2 > /dev/null 2>&1
if [ $? -eq 0 ]; then
    gcc build_output/test2.c runtime/palladium_runtime.c -o test2_exe 2>/dev/null
    if [ $? -eq 0 ]; then
        ./test2_exe
        echo "✅ Variables test passed"
    else
        echo "❌ Linking failed"
    fi
else
    echo "❌ Compilation failed"
fi

# Test 3: Functions
echo -e "\n3️⃣ Testing functions..."
cat > test3.pd << 'EOF'
fn add(x: i32, y: i32) -> i32 {
    return x + y;
}

fn main() {
    let result = add(5, 7);
    print("5 + 7 = ");
    print_int(result);
}
EOF

./target/debug/pdc compile test3.pd -o test3 > /dev/null 2>&1
if [ $? -eq 0 ]; then
    gcc build_output/test3.c runtime/palladium_runtime.c -o test3_exe 2>/dev/null
    if [ $? -eq 0 ]; then
        ./test3_exe
        echo "✅ Functions test passed"
    else
        echo "❌ Linking failed"
    fi
else
    echo "❌ Compilation failed"
fi

# Cleanup
rm -f test*.pd test*_exe

echo -e "\n✨ Tests complete!"