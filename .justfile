# 使用nu shell跨平台
set shell := ["nu", "-c"]
# 加载.env文件
set dotenv-load := true
#  * Created on Tue Jan 20 2026
# 默认只是列出所有的recipe
default:
    @just --list --unsorted --justfile {{justfile()}}

check:
    @cargo check --all
build-waybar-bard: check
    @cargo build --manifest-path crates/waybar-bard/Cargo.toml --release