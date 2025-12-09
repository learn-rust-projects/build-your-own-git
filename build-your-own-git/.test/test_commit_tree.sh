#!/usr/bin/env bash
set -euo pipefail

bold() { echo -e "\033[1m$*\033[0m"; }
info() { echo -e "\033[36m[INFO]\033[0m $*"; }
ok() { echo -e "\033[32m[OK]\033[0m $*"; }
fail() { echo -e "\033[31m[FAIL]\033[0m $*" >&2; exit 1; }
print_step() { echo -e "\033[33m▶ $*\033[0m"; }

PROGRAM="$1"
TEST_DIR="test_commit_tree_$(date +%s)"
COMMIT_MESSAGE="Test commit message"

mkdir -p "$TEST_DIR" && cd "$TEST_DIR"

# ========= 用我们的程序 =========
print_step "使用 $PROGRAM 初始化并创建提交"
"$PROGRAM" init

echo "hello world" > file1
mkdir dir1 && echo "hello world" > dir1/file_in_dir_1

# 创建tree对象
TREE_SHA=$($PROGRAM write-tree)
info "创建的tree哈希: $TREE_SHA"

# 创建提交对象
OUR_COMMIT_SHA=$($PROGRAM commit-tree "$TREE_SHA" -m "$COMMIT_MESSAGE")
info "我们的程序生成的commit哈希: $OUR_COMMIT_SHA"

# ========= 用官方git =========
print_step "清理并切换到官方git创建相同的提交"

# 删除所有遗留文件，确保干净的测试环境
rm -rf .git
rm -f file1
rm -rf dir1

# 重新创建文件结构
echo "hello world" > file1
mkdir dir1 && echo "hello world" > dir1/file_in_dir_1

git init 

# 添加到索引
git add .

# 创建提交
GIT_TREE_SHA=$(git write-tree)
GIT_COMMIT_SHA=$(echo "$COMMIT_MESSAGE" | git commit-tree "$GIT_TREE_SHA")
info "官方git生成的commit哈希: $GIT_COMMIT_SHA"

# ========= 比较结果 =========
print_step "比较我们实现和官方git的commit-tree输出"
if [[ "$OUR_COMMIT_SHA" == "$GIT_COMMIT_SHA" ]]; then
    ok "✓ 我们的commit哈希与官方git完全一致"
else
    fail "✗ commit哈希不一致，请检查实现\n我们的: $OUR_COMMIT_SHA\n官方git: $GIT_COMMIT_SHA"
fi

# ========= 验证commit内容 =========
print_step "验证commit对象内容"
OUR_COMMIT_CONTENT=$(git cat-file -p "$OUR_COMMIT_SHA")
GIT_COMMIT_CONTENT=$(git cat-file -p "$GIT_COMMIT_SHA")

# 比较关键字段（忽略时间戳差异）
OUR_TREE_LINE=$(echo "$OUR_COMMIT_CONTENT" | grep "^tree ")
GIT_TREE_LINE=$(echo "$GIT_COMMIT_CONTENT" | grep "^tree ")

OUR_MESSAGE_LINE=$(echo "$OUR_COMMIT_CONTENT" | grep -A1 "^$" | tail -1)
GIT_MESSAGE_LINE=$(echo "$GIT_COMMIT_CONTENT" | grep -A1 "^$" | tail -1)

if [[ "$OUR_TREE_LINE" == "$GIT_TREE_LINE" && "$OUR_MESSAGE_LINE" == "$GIT_MESSAGE_LINE" ]]; then
    ok "✓ commit对象关键内容与官方git一致"
else
    info "我们的commit内容:"
    echo "$OUR_COMMIT_CONTENT"
    info "官方git commit内容:"
    echo "$GIT_COMMIT_CONTENT"
    fail "✗ commit对象内容不一致，请检查实现"
fi

# ========= 清理 =========
cd ..
rm -rf "$TEST_DIR"
bold "\n✅ commit-tree 测试完成！"