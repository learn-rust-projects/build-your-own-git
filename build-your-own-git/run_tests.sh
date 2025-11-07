#!/usr/bin/env bash
set -euo pipefail

# 颜色输出函数
bold() { echo -e "\033[1m$*\033[0m"; }
info() { echo -e "\033[36m[INFO]\033[0m $*"; }
ok() { echo -e "\033[32m[OK]\033[0m $*"; }
warn() { echo -e "\033[33m[WARN]\033[0m $*"; }
error() { echo -e "\033[31m[ERROR]\033[0m $*" >&2; }

# 测试统计
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0
CURRENT_TEST=0

# 进度条配置
PROGRESS_BAR_WIDTH=50

# 打印漂亮的分隔线
print_separator() {
    echo -e "\033[34m==========================================================\033[0m"
}

# 打印彩色标题
print_title() {
    local title="$1"
    local color="$2"
    echo -e "$color$title\033[0m"
}

# 打印测试标题
print_test_title() {
    print_separator
    print_title "开始测试: $*" "\033[1;35m"
    print_separator
}

# 打印进度条
print_progress() {
    local current="$1"
    local total="$2"
    local percent=$((current * 100 / total))
    local filled_width=$((percent * PROGRESS_BAR_WIDTH / 100))
    
    # 创建进度条
    local progress_bar=""
    for ((i=0; i<PROGRESS_BAR_WIDTH; i++)); do
        if [ $i -lt $filled_width ]; then
            progress_bar+="▓"
        else
            progress_bar+="░"
        fi
    done
    
    # 移动光标到行首并打印进度条
    echo -ne "\r\033[32m$progress_bar\033[0m \033[1m${percent}%\033[0m ($current/$total)"
}

# 打印测试总结
print_test_summary() {
    echo
    print_separator
    print_title "测试总结" "\033[1;34m"
    bold "  总测试数: $TOTAL_TESTS"
    bold "  通过测试: \033[32m$PASSED_TESTS\033[0m"
    bold "  失败测试: \033[31m$FAILED_TESTS\033[0m"
    
    if [ $TOTAL_TESTS -gt 0 ]; then
        PASS_RATE=$((PASSED_TESTS * 100 / TOTAL_TESTS))
        local rate_color="\033[32m"
        if [ $PASS_RATE -lt 70 ]; then
            rate_color="\033[31m"
        elif [ $PASS_RATE -lt 90 ]; then
            rate_color="\033[33m"
        fi
        bold "  通过率: $rate_color$PASS_RATE%\033[0m"
    fi
    print_separator
}

# 运行单个测试
run_test() {
    local test_name="$1"
    local test_script="$2"
    local program="$3"
    local test_start_time
    local test_end_time
    local test_duration
    
    CURRENT_TEST=$((CURRENT_TEST + 1))
    # 移除重复的 TOTAL_TESTS 计数
    
    # 显示进度条
    print_progress $CURRENT_TEST $TOTAL_TESTS
    
    # 运行测试
    test_start_time=$(date +%s)
    if $test_script "$program"; then
        PASSED_TESTS=$((PASSED_TESTS + 1))
        test_end_time=$(date +%s)
        test_duration=$((test_end_time - test_start_time))
        echo -e "\n\033[32m[✓]\033[0m 测试 '$test_name' 通过! (耗时: ${test_duration}s)"
    else
        FAILED_TESTS=$((FAILED_TESTS + 1))
        test_end_time=$(date +%s)
        test_duration=$((test_end_time - test_start_time))
        error "测试 '$test_name' 失败! (耗时: ${test_duration}s)"
        return 1
    fi
    
    echo
}

# 打印开始信息
print_start_info() {
    local title="$1"
    clear # 清屏以提供更干净的输出
    print_title "$title" "\033[1;36m"
    echo -e "\033[37m==============================================\033[0m"
    echo -e "\033[37m项目: own-git - 自定义 Git 实现\033[0m"
    echo -e "\033[37m日期: $(date '+%Y-%m-%d %H:%M:%S')\033[0m"
    echo -e "\033[37m==============================================\033[0m\n"
}

# 主测试流程
main() {
    # 打印开始信息
    print_start_info "Git 实现测试套件"
        
    # 接受Makefile传递的可执行文件绝对路径参数
    if [ $# -gt 0 ]; then
        PROGRAM="$1"
        info "使用传入的可执行文件路径: $PROGRAM"
    else
        PROGRAM="own-git"
        info "使用默认可执行文件名: $PROGRAM"
    fi
    
    TEST_DIR="test_dir"
    
    # 准备测试环境
    print_title "准备测试环境" "\033[1;34m"
    info "清理旧的测试目录..."
    rm -rf "$TEST_DIR"
    info "创建新的测试目录..."
    mkdir "$TEST_DIR"
    cd "$TEST_DIR"
    ok "测试目录 '$TEST_DIR' 已创建"
    echo
    
    # 定义测试列表 - 使用竖线作为分隔符
    TESTS=("Git 初始化|../.test/test_init.sh"
           "文件内容读取|../.test/test_cat_file.sh"
           "写入树对象|../.test/test_write_tree.sh"
           "读取树对象|../.test/test_read_tree.sh"
           )
    TOTAL_TESTS=${#TESTS[@]}
    
    # 运行所有测试
    print_title "运行测试 ($TOTAL_TESTS)" "\033[1;34m"
    
    CURRENT_TEST=0
    local any_failed=false
    
    for test_case in "${TESTS[@]}"; do
        # 解析测试用例 - 使用竖线作为分隔符
        local test_name=$(echo "$test_case" | cut -d'|' -f1)
        local test_script=$(echo "$test_case" | cut -d'|' -f2)
        
        # 运行测试并检查结果
        if ! run_test "$test_name" "$test_script" "$PROGRAM"; then
            any_failed=true
        fi
    done
    
    # 清理测试环境
    cd ..
    print_title "清理测试环境" "\033[1;34m"
    info "删除测试目录..."
    rm -rf "$TEST_DIR"
    ok "测试目录 '$TEST_DIR' 已清理"
    echo
    
    # 打印测试总结
    print_test_summary
    
    # 返回适当的退出码
    if [ "$any_failed" = true ]; then
        error "❌ 有测试失败!"
        exit 1
    else
        ok "✅ 所有测试通过!"
        exit 0
    fi
}

# 开始执行
main