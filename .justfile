# 使用nu shell跨平台
set shell := ["nu", "-c"]
# 加载.env文件
set dotenv-load := true
# 默认只是列出所有的recipe
default:
    @just --list --unsorted --justfile {{justfile()}}
fmt:
    @cargo fmt --all
fmt-check:
    @cargo fmt --all -- --check
check: fmt-check
    @cargo check --workspace --all-targets
test: check
    @cargo test --workspace --all-targets
clippy: test
    @cargo clippy --workspace --all-targets -- -D warnings
build-release: clippy
    @cargo build --workspace --release
ci: build-release
clean:
    @cargo clean
run: check
    @cargo run --manifest-path crates/waybar-bard/Cargo.toml --release
build-waybar-bard: check
    @cargo build --manifest-path crates/waybar-bard/Cargo.toml --release
