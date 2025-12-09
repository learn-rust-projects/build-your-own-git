# 定义变量
BINARY_NAME = own-git
TARGET_PATH = /tmp/codecrafters-build-git-rust/release/$(BINARY_NAME)
INSTALL_PATH = /usr/local/bin/$(BINARY_NAME)
PROJECT_DIR = .

# 默认目标
.PHONY: all
all: build

# 构建项目
.PHONY: build
build:
	cargo build --release --target-dir=/tmp/codecrafters-build-git-rust --manifest-path "$(PROJECT_DIR)/Cargo.toml"

# 为所有脚本添加执行权限
.PHONY: chmod
chmod:
	find . -name "*.sh" -exec chmod +x {} \;

# 安装项目
.PHONY: install
install: chmod build
	sudo cp "$(TARGET_PATH)" "$(INSTALL_PATH)"
	@echo "✅ Installation completed: $(INSTALL_PATH)"

# 卸载项目
.PHONY: uninstall
uninstall:
	@if [ -f "$(INSTALL_PATH)" ]; then \
		sudo rm -f "$(INSTALL_PATH)"; \
		echo "✅ Uninstallation completed"; \
	else \
		echo "⚠️ Binary not found: $(INSTALL_PATH)"; \
	fi

# 运行测试
.PHONY: test
test: clean chmod build
	# 从target直接运行测试，获取TARGET_PATH的绝对路径并传递给run_tests.sh
	./run_tests.sh "$$(realpath $(TARGET_PATH))"

# 清理构建文件
.PHONY: clean
clean:
	rm -rf /tmp/codecrafters-build-git-rust
	@echo "✅ Clean completed"

# 显示帮助信息
.PHONY: help
help:
	@echo "Available commands:";
	@echo "  make          - Build the project (default)";
	@echo "  make build    - Build the project";
	@echo "  make chmod    - Make all shell scripts executable";
	@echo "  make install  - Build and install the project globally";
	@echo "  make uninstall- Uninstall the project";
	@echo "  make test     - Run tests with build/chmod/install";
	@echo "  make clean    - Remove build files";
	@echo "  make help     - Show this help message";