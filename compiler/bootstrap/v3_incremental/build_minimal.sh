#!/bin/bash
# Build script for minimal Palladium compiler

echo "Building minimal Palladium compiler..."

# Step 1: Concatenate all source files
cat > pdc_complete.pd << 'EOF'
// Complete minimal Palladium compiler
// This file combines lexer, parser, and codegen

EOF

# Add lexer
echo "// === LEXER ===" >> pdc_complete.pd
cat lexer_minimal.pd >> pdc_complete.pd

# Add parser (remove duplicate definitions)
echo -e "\n// === PARSER ===" >> pdc_complete.pd
sed '/^const TOK_/d; /^struct Token/,/^}/d; /^let mut TOKENS/d; /^let mut TOKEN_COUNT/d; /^let mut STRING_TABLE/d; /^fn get_string_from_table/,/^}/d; /^extern {/,/^}/d' parser_minimal.pd >> pdc_complete.pd

# Add codegen (remove duplicate definitions)
echo -e "\n// === CODEGEN ===" >> pdc_complete.pd
sed '/^extern {/,/^}/d' codegen_minimal.pd >> pdc_complete.pd

# Add main driver
echo -e "\n// === MAIN DRIVER ===" >> pdc_complete.pd
cat >> pdc_complete.pd << 'EOF'

fn compile_file(filename: String) -> bool {
    print("Compiling: " + filename);
    
    // Read input file
    let file_handle = file_open(filename);
    if file_handle < 0 {
        print("Error: Cannot open file " + filename);
        return false;
    }
    
    let source = file_read_all(file_handle);
    file_close(file_handle);
    
    // Lexical analysis
    print("Lexing...");
    init_lexer(source);
    lex();
    print("Found " + int_to_string(TOKEN_COUNT) + " tokens");
    
    // Parsing
    print("Parsing...");
    let root_id = parse();
    if root_id < 0 {
        print("Parse error!");
        return false;
    }
    print("Created " + int_to_string(AST_NODE_COUNT) + " AST nodes");
    
    // Code generation
    print("Generating C code...");
    let c_code = generate_code(root_id);
    
    // Write output
    let output_filename = string_concat(filename, ".c");
    let output_handle = file_open(output_filename);
    if output_handle < 0 {
        print("Error: Cannot create output file");
        return false;
    }
    
    let result = file_write(output_handle, c_code);
    file_close(output_handle);
    
    if result {
        print("Output written to: " + output_filename);
        print("Compilation successful!");
        return true;
    } else {
        print("Error writing output file");
        return false;
    }
}

fn main() {
    print("Minimal Palladium Compiler v0.1");
    print("================================");
    
    // Compile a test file
    if !compile_file("ultra_minimal.pd") {
        print("Compilation failed!");
    }
}
EOF

echo "Combined source created: pdc_complete.pd"

# Step 2: Compile with current Palladium compiler
echo "Compiling with Rust-based compiler..."
cd ..
cargo run -- compile bootstrap3/pdc_complete.pd -o bootstrap3/pdc_complete.c

# Step 3: Compile C code
echo "Compiling C code..."
cd bootstrap3
gcc pdc_complete.c -o pdc_minimal

echo "Build complete! Executable: pdc_minimal"