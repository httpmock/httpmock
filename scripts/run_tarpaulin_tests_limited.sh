#!/bin/bash

# Check if file path is provided
if [ -z "$1" ]; then
  echo "Please provide the file path as an argument."
  exit 1
fi

# File path from the argument
file_path="$1"

# Initialize an array to store the extracted features
features_list=()

# Read the file line by line
while IFS= read -r line; do
  # Check if the line contains 'features ='
  if [[ "$line" =~ features\ = ]]; then
    # Extract the features using sed
    features=$(echo "$line" | sed -n 's/.*features = "\([^"]*\)".*/\1/p' | grep -v "https")

    # Store the extracted features in the array
    features_list+=("$features")
  fi
done < "$file_path"

# List all extracted features
echo "Extracted features:"
for feature in "${features_list[@]}"; do
  echo "$feature"
done

# Run cargo test for each feature set, stop if an error occurs and wait for confirmation
for feature in "${features_list[@]}"; do
  echo "Testing with features: $feature"
  if ! cargo test --features="$feature"; then
    echo "Test failed for features: $feature."
    echo "Press Enter to continue with the next set of features or Ctrl+C to exit."
    read -r
  fi
done

echo "All tests completed."
