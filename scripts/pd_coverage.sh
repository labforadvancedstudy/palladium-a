#!/bin/bash
# Palladium Code Coverage Analysis Script

set -e

echo "🔍 Palladium Code Coverage Analysis"
echo "==================================="

# Colors
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Find all .pd files
PD_FILES=$(find . -name "*.pd" -not -path "./target/*" -not -path "./.git/*" -not -path "./archive/*" | sort)

# Total stats
TOTAL_FILES=0
TOTAL_LINES=0
TESTED_FILES=0
UNTESTED_FILES=0

# Create coverage report directory
mkdir -p coverage_report/palladium

# Create HTML report header
cat > coverage_report/palladium/index.html << 'EOF'
<!DOCTYPE html>
<html>
<head>
    <title>Palladium Code Coverage Report</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; }
        h1 { color: #333; }
        table { border-collapse: collapse; width: 100%; }
        th, td { border: 1px solid #ddd; padding: 8px; text-align: left; }
        th { background-color: #4CAF50; color: white; }
        tr:nth-child(even) { background-color: #f2f2f2; }
        .tested { color: green; font-weight: bold; }
        .untested { color: red; }
        .summary { margin: 20px 0; padding: 10px; background-color: #e7f3fe; border-left: 4px solid #2196F3; }
    </style>
</head>
<body>
    <h1>Palladium Code Coverage Report</h1>
    <div class="summary" id="summary">
        <h2>Summary</h2>
        <p>Loading...</p>
    </div>
    <h2>File Details</h2>
    <table>
        <tr>
            <th>File Path</th>
            <th>Lines</th>
            <th>Test Status</th>
            <th>Notes</th>
        </tr>
EOF

# Function to check if a file has associated tests
has_tests() {
    local file=$1
    local basename=$(basename "$file" .pd)
    local dirname=$(dirname "$file")
    
    # Check for direct test files
    if [[ -f "${dirname}/test_${basename}.pd" ]] || [[ -f "${dirname}/${basename}_test.pd" ]]; then
        return 0
    fi
    
    # Check if file is in examples/ and has corresponding output
    if [[ "$file" == *"examples/"* ]]; then
        if ls "${file%.pd}"*.out 2>/dev/null | grep -q .; then
            return 0
        fi
    fi
    
    # Check if file is in bootstrap/ and has test scripts
    if [[ "$file" == *"bootstrap/"* ]]; then
        local test_script="${dirname}/test_$(basename "$file" .pd).sh"
        if [[ -f "$test_script" ]]; then
            return 0
        fi
    fi
    
    # Check if file is itself a test
    if [[ "$basename" == test_* ]] || [[ "$basename" == *_test ]]; then
        return 0
    fi
    
    return 1
}

# Analyze each .pd file
for file in $PD_FILES; do
    TOTAL_FILES=$((TOTAL_FILES + 1))
    lines=$(wc -l < "$file" 2>/dev/null || echo 0)
    TOTAL_LINES=$((TOTAL_LINES + lines))
    
    if has_tests "$file"; then
        TESTED_FILES=$((TESTED_FILES + 1))
        status="tested"
        status_html="<span class='tested'>✓ Tested</span>"
        notes="Has associated tests"
    else
        UNTESTED_FILES=$((UNTESTED_FILES + 1))
        status="untested"
        status_html="<span class='untested'>✗ No tests</span>"
        notes="Missing test coverage"
    fi
    
    # Add to HTML report
    echo "        <tr>" >> coverage_report/palladium/index.html
    echo "            <td>$file</td>" >> coverage_report/palladium/index.html
    echo "            <td>$lines</td>" >> coverage_report/palladium/index.html
    echo "            <td>$status_html</td>" >> coverage_report/palladium/index.html
    echo "            <td>$notes</td>" >> coverage_report/palladium/index.html
    echo "        </tr>" >> coverage_report/palladium/index.html
done

# Calculate coverage percentage
if [[ $TOTAL_FILES -gt 0 ]]; then
    COVERAGE_PERCENT=$((TESTED_FILES * 100 / TOTAL_FILES))
else
    COVERAGE_PERCENT=0
fi

# Complete HTML report
cat >> coverage_report/palladium/index.html << EOF
    </table>
    <script>
        document.getElementById('summary').innerHTML = \`
            <h2>Summary</h2>
            <p><strong>Total Files:</strong> $TOTAL_FILES</p>
            <p><strong>Total Lines:</strong> $TOTAL_LINES</p>
            <p><strong>Tested Files:</strong> $TESTED_FILES</p>
            <p><strong>Untested Files:</strong> $UNTESTED_FILES</p>
            <p><strong>Coverage:</strong> ${COVERAGE_PERCENT}%</p>
        \`;
    </script>
</body>
</html>
EOF

# Print summary to console
echo ""
echo "📊 Palladium Code Coverage Summary"
echo "=================================="
echo "Total .pd files: $TOTAL_FILES"
echo "Total lines: $TOTAL_LINES"
echo -e "Tested files: ${GREEN}$TESTED_FILES${NC}"
echo -e "Untested files: ${RED}$UNTESTED_FILES${NC}"
echo -e "Coverage: ${YELLOW}${COVERAGE_PERCENT}%${NC}"
echo ""
echo "✅ HTML report generated: coverage_report/palladium/index.html"

# List untested files if any
if [[ $UNTESTED_FILES -gt 0 ]]; then
    echo ""
    echo "❌ Files without tests:"
    for file in $PD_FILES; do
        if ! has_tests "$file"; then
            echo "   - $file"
        fi
    done
fi