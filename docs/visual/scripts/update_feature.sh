#!/bin/bash
# Palladium Feature Update Script
# Usage: ./update_feature.sh <feature-name> <progress>

FEATURE=$1
PROGRESS=$2

if [ -z "$FEATURE" ] || [ -z "$PROGRESS" ]; then
    echo "Usage: $0 <feature-name> <progress-percentage>"
    echo "Example: $0 implicit_lifetimes 85"
    exit 1
fi

# Update status.yaml
echo "Updating feature '$FEATURE' to $PROGRESS%..."

# Update the YAML file (using yq or similar tool)
yq eval ".palladium.**.${FEATURE}.progress = $PROGRESS" -i docs/features/status.yaml

# Update the feature's markdown file
FEATURE_MD="docs/features/${FEATURE}.md"
if [ -f "$FEATURE_MD" ]; then
    # Update the status line in markdown
    sed -i "s/## Status: .* [0-9]*% Complete/## Status: ⏳ $PROGRESS% Complete/" "$FEATURE_MD"
    
    # Update status emoji based on progress
    if [ "$PROGRESS" -ge 80 ]; then
        sed -i "s/## Status: ⏳/## Status: ✅/" "$FEATURE_MD"
        yq eval ".palladium.**.${FEATURE}.status = \"✅\"" -i docs/features/status.yaml
    elif [ "$PROGRESS" -lt 20 ]; then
        sed -i "s/## Status: ⏳/## Status: 🔲/" "$FEATURE_MD"
        yq eval ".palladium.**.${FEATURE}.status = \"🔲\"" -i docs/features/status.yaml
    fi
fi

# Update last modified date
yq eval '.last_updated = strenv(TODAY)' -i docs/features/status.yaml

# Generate summary
echo "Feature '$FEATURE' updated to $PROGRESS%"
echo "Don't forget to:"
echo "1. Update implementation notes in $FEATURE_MD"
echo "2. Add any new code examples"
echo "3. Commit with message: 'feat($FEATURE): $PROGRESS% complete'"