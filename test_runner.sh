#!/bin/bash
# Palladium Test Runner

echo "🧪 Running Palladium Tests"
echo "========================"

# Build
echo "📦 Building..."
cargo build --release --no-default-features
if [ $? -ne 0 ]; then
    echo "❌ Build failed"
    exit 1
fi
echo "✅ Build successful"

# Unit tests
echo ""
echo "🔬 Running unit tests..."
cargo test --no-default-features --lib
if [ $? -ne 0 ]; then
    echo "❌ Unit tests failed"
    exit 1
fi
echo "✅ Unit tests passed"

# Integration test (simple)
echo ""
echo "🔗 Running integration test..."
cat > test_program.pd << 'EOF'
fn main() {
    print("Integration test passed!");
}
EOF

./target/release/pdc compile test_program.pd -o test_output
if [ $? -eq 0 ]; then
    echo "✅ Compilation successful"
    ./build_output/test_output
else
    echo "❌ Compilation failed"
fi

# Cleanup
rm -f test_program.pd

echo ""
echo "🎉 All tests complete!"