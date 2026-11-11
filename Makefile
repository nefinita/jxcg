# ============================================================================
# JXCG 构建矩阵（Tauri v2 + Rust workspace + React 19 + bun）
#
# 常用：
#   make setup        前端依赖安装（bun install，锁文件 bun.lock）
#   make frontend     前端构建（tsc -b && vite build → examples/jxcg_app/dist/）
#   make frontend-test 前端测试（vitest）
#   make build        Rust workspace 构建（含 Tauri app）
#   make dev          Tauri 开发模式（bun run tauri dev）
#   make test         cargo test --workspace + 前端测试
#   make check-all    fmt + clippy + 构建 + 测试 + 前端构建
#   make data-export  重新导出 JX3BOX 数据（P2 后可用）
# ============================================================================

SHELL := /bin/bash

# 沙箱环境：HOME 只读，cargo/bun 缓存统一放工作区 .cache/（见 AGENTS.md）
export CARGO_HOME := $(CURDIR)/.cache/cargo
export BUN_INSTALL_CACHE_DIR := $(CURDIR)/.cache/bun
APP_DIR := examples/jxcg_app
CARGO ?= cargo
BUN ?= bun
BUILD ?= debug

.PHONY: help setup frontend frontend-test build dev test fmt fmt-check clippy check-all data-export clean

help:
	@sed -n '3,17p' Makefile | sed 's/^# \{0,1\}//'

setup:
	cd $(APP_DIR) && $(BUN) install --frozen-lockfile

frontend:
	cd $(APP_DIR) && $(BUN) run build

frontend-test:
	cd $(APP_DIR) && $(BUN) run test

build:
	$(CARGO) build --workspace

dev:
	cd $(APP_DIR) && $(BUN) run tauri dev

test:
	$(CARGO) test --workspace

fmt:
	$(CARGO) fmt --all

fmt-check:
	$(CARGO) fmt --all -- --check

clippy:
	$(CARGO) clippy --workspace --all-targets

check-all: fmt-check clippy build test frontend-test frontend

data-export:
	$(CARGO) run -p jx3box-export -- --out data

clean:
	$(CARGO) clean
	rm -rf $(APP_DIR)/dist $(APP_DIR)/node_modules
