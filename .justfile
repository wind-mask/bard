# 使用nu shell跨平台
set shell := ["nu", "-c"]
# 加载.env文件
set dotenv-load := true
# 默认只是列出所有的recipe
default:
    @just --list --unsorted --justfile {{justfile()}}
fmt:
    @cargo fmt --all
check:fmt
    @cargo check --all
clippy:check
    @cargo clippy --all -- -D warnings
clean:
    @cargo clean
run:check
    @cargo run --manifest-path crates/waybar-bard/Cargo.toml --release
build-waybar-bard: check
    @cargo build --manifest-path crates/waybar-bard/Cargo.toml --release
