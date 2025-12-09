#!/usr/bin/env bash
set -euo pipefail

bold() { echo -e "\033[1m$*\033[0m"; }
info() { echo -e "\033[36m[INFO]\033[0m $*"; }
ok() { echo -e "\033[32m[OK]\033[0m $*"; }
fail() { echo -e "\033[31m[FAIL]\033[0m $*" >&2; exit 1; }
print_step() { echo -e "\033[33m▶ $*\033[0m"; }

PROGRAM="$1"
TEST_DIR="test_clone_$(date +%s)"
REMOTE_REPO="https://github.com/learn-rust-projects/build-your-own-git"
CLONE_REPO="clone_repo_$(date +%s)"

# ========= 用我们的程序clone =========
print_step "使用 $PROGRAM 克隆远程仓库"
OUR_CLONE_DIR="${CLONE_REPO}_our"
"$PROGRAM" clone "$REMOTE_REPO" "$OUR_CLONE_DIR"

# 验证clone结果
if [[ -d "$OUR_CLONE_DIR/.git" ]]; then
    ok "✓ 我们的程序成功创建了.git目录"
else
    fail "✗ 我们的程序未能创建.git目录"
fi

# ========= 用官方git clone =========
print_step "使用官方git克隆相同的仓库"
GIT_CLONE_DIR="${CLONE_REPO}_git"
git clone "$REMOTE_REPO" "$GIT_CLONE_DIR" > /dev/null 2>&1

# ========= 比较结果 =========
print_step "比较clone结果"

# 检查文件结构
OUR_FILES=$(find "$OUR_CLONE_DIR" -type f -not -path '*/\.git/*' | sort | sed "s|^$OUR_CLONE_DIR/||")        
GIT_FILES=$(find "$GIT_CLONE_DIR" -type f -not -path '*/\.git/*' | sort | sed "s|^$GIT_CLONE_DIR/||")


if [[ "$OUR_FILES" == "$GIT_FILES" ]]; then
    ok "✓ 文件结构与官方git一致"
else
    info "我们的文件:"
    echo "$OUR_FILES"
    info "官方git文件:"
    echo "$GIT_FILES"
    fail "✗ 文件结构不一致"
fi

# 检查文件内容
for file in $(find "$OUR_CLONE_DIR" -type f -not -path '*/\.git/*'); do
    rel_path=${file#$OUR_CLONE_DIR/}
    git_file="$GIT_CLONE_DIR/$rel_path"
    
    if [[ -f "$git_file" ]]; then
        OUR_CONTENT=$(cat "$file")
        GIT_CONTENT=$(cat "$git_file")
        
        if [[ "$OUR_CONTENT" == "$GIT_CONTENT" ]]; then
            ok "✓ 文件 '$rel_path' 内容一致"
        else
            fail "✗ 文件 '$rel_path' 内容不一致"
        fi
    fi
done

# 检查HEAD引用
OUR_HEAD=$(cat "$OUR_CLONE_DIR/.git/HEAD")
GIT_HEAD=$(cat "$GIT_CLONE_DIR/.git/HEAD")

if [[ "$OUR_HEAD" == "$GIT_HEAD" ]]; then
    ok "✓ HEAD引用一致"
else
    fail "✗ HEAD引用不一致\n我们的: $OUR_HEAD\n官方git: $GIT_HEAD"
fi

# ========= 清理 =========
print_step "清理测试环境"
rm -rf "$OUR_CLONE_DIR" "$GIT_CLONE_DIR"
ok "✓ 测试环境已清理"

bold "\n✅ clone 测试完成！"