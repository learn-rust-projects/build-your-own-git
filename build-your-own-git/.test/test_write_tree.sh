#!/usr/bin/env bash
set -euo pipefail

bold() { echo -e "\033[1m$*\033[0m"; }
info() { echo -e "\033[36m[INFO]\033[0m $*"; }
ok() { echo -e "\033[32m[OK]\033[0m $*"; }
fail() { echo -e "\033[31m[FAIL]\033[0m $*" >&2; exit 1; }
print_step() { echo -e "\033[33m▶ $*\033[0m"; }

PROGRAM="$1"
TEST_DIR="test_write_tree_$(date +%s)"

mkdir -p "$TEST_DIR" && cd "$TEST_DIR"

# ========= 用我们的程序 =========
print_step "使用 $PROGRAM 初始化并生成tree"
"$PROGRAM" init

echo "hello world" > file1
mkdir dir1 && echo "hello world" > dir1/file_in_dir_1
mkdir dir2 && echo "hello world" > dir2/file_in_dir_2

OUR_TREE_SHA=$($PROGRAM write-tree)
info "我们的程序生成的tree哈希: $OUR_TREE_SHA"
OUR_TREE_CONTENT=$(git cat-file -p "$OUR_TREE_SHA")

# ========= 用官方git =========
print_step "切换到官方git初始化并生成tree"
rm -rf .git
git init > /dev/null

# 将当前目录下所有文件添加到索引
git add .

GIT_TREE_SHA=$(git write-tree)
info "官方git生成的tree哈希: $GIT_TREE_SHA"

# ========= 比较结果 =========
print_step "比较我们实现和官方git的write-tree输出"
if [[ "$OUR_TREE_SHA" == "$GIT_TREE_SHA" ]]; then
    ok "✓ 我们的tree哈希与官方git完全一致"
else
    fail "✗ tree哈希不一致，请检查实现\n我们的: $OUR_TREE_SHA\n官方git: $GIT_TREE_SHA"
fi

# ========= 验证tree内容 =========
print_step "验证tree对象内容"
GIT_TREE_CONTENT=$(git cat-file -p "$GIT_TREE_SHA")

if [[ "$OUR_TREE_CONTENT" == "$GIT_TREE_CONTENT" ]]; then
    ok "✓ tree对象内容与官方git完全一致"
else
    fail "✗ tree对象内容不一致，请检查实现"
fi

# ========= 清理 =========
cd ..
rm -rf "$TEST_DIR"
bold "\n✅ write-tree 测试完成！"