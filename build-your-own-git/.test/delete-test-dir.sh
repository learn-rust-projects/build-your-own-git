#!/bin/sh
#
# Delete the entire test directory.

set -e

# 默认测试目录，可通过参数传入
TEST_DIR="${1:-./test-repo}"

if [ -d "$TEST_DIR" ]; then
  echo "Deleting test directory: $TEST_DIR"
  rm -rf "$TEST_DIR"
  echo "Deleted successfully."
else
  echo "Test directory does not exist: $TEST_DIR"
fi
