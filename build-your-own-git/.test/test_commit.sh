#!/usr/bin/env bash
set -euo pipefail

bold() { echo -e "\033[1m$*\033[0m"; }
info() { echo -e "\033[36m[INFO]\033[0m $*"; }
ok() { echo -e "\033[32m[OK]\033[0m $*"; }
fail() { echo -e "\033[31m[FAIL]\033[0m $*" >&2; exit 1; }
print_step() { echo -e "\033[33m▶ $*\033[0m"; }

PROGRAM="$1"
TEST_DIR="test_commit_$(date +%s)"
COMMIT_MESSAGE="Test commit message"

mkdir -p "$TEST_DIR" && cd "$TEST_DIR"

# ========= 用我们的程序 =========
print_step "使用 $PROGRAM 初始化并创建提交"
"$PROGRAM" init

echo "hello world" > file1
mkdir dir1 && echo "hello world" > dir1/file_in_dir_1

# 创建提交
OUR_COMMIT_SHA=$($PROGRAM commit -m "$COMMIT_MESSAGE")
info "我们的程序生成的commit哈希: $OUR_COMMIT_SHA"

# 验证HEAD引用
OUR_HEAD_REF=$(cat .git/HEAD)
info "我们的HEAD引用: $OUR_HEAD_REF"
OUR_COMMIT_CONTENT=$(git cat-file -p  "$OUR_COMMIT_SHA")
# ========= 用官方git =========

rm -rf .git
rm -rf dir1
rm -rf file1
print_step "使用git初始化并创建提交"
git init > /dev/null

# 重新创建文件结构
echo "hello world" > file1
mkdir dir1 && echo "hello world" > dir1/file_in_dir_1

# 添加到索引并提交
git add .
git commit -m "$COMMIT_MESSAGE"
GIT_COMMIT_SHA=$(git rev-parse HEAD)
info "官方git生成的commit哈希: $GIT_COMMIT_SHA"

# 验证HEAD引用
GIT_HEAD_REF=$(cat .git/HEAD)
info "官方git HEAD引用: $GIT_HEAD_REF"

# ========= 比较结果 =========
print_step "比较我们实现和官方git的commit输出"
if [[ "$OUR_COMMIT_SHA" == "$GIT_COMMIT_SHA" ]]; then
    ok "✓ 我们的commit哈希与官方git完全一致"
else
    fail "✗ commit哈希不一致，请检查实现\n我们的: $OUR_COMMIT_SHA\n官方git: $GIT_COMMIT_SHA"
fi

# ========= 验证commit对象内容 =========
print_step "验证commit对象内容"
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

# ========= 验证引用更新 =========
print_step "验证引用更新"
OUR_BRANCH_REF=$(cat .git/refs/heads/master 2>/dev/null || cat .git/refs/heads/main 2>/dev/null)
GIT_BRANCH_REF=$(cat .git/refs/heads/master 2>/dev/null || cat .git/refs/heads/main 2>/dev/null)

if [[ "$OUR_BRANCH_REF" == "$GIT_BRANCH_REF" ]]; then
    ok "✓ 分支引用更新正确"
else
    fail "✗ 分支引用更新不一致\n我们的: $OUR_BRANCH_REF\n官方git: $GIT_BRANCH_REF"
fi

# ========= 清理 =========
cd ..
rm -rf "$TEST_DIR"
bold "\n✅ commit 测试完成！"