# B.A.R.D

**Ballad Assistant Rhythm Debugger** - 在 Waybar/dms 中显示同步歌词的工具

![banner](banner.png)

## [Upstream](https://github.com/puszkarek/bard)

进行了精简，只使用歌曲文件元数据中的歌词，
也就是说，只支持播放本地音乐。

## 功能特性

- 通过 MPRIS D-Bus 协议与音乐播放器交互
- 自动从音频文件标签读取歌词
- 支持 LRC 时间戳格式 `[MM:SS.CC]`
- 在 Waybar 中实时显示同步歌词
- 在dms中作为bar插件显示歌词

## 安装

### 从源码构建

确保已安装 Rust 工具链：

```bash
# 克隆仓库
git clone https://github.com/wind-mask/bard.git
cd bard
# 构建 release 版本
just build-waybar-bard
# 或
cargo build --release
```

## Waybar 集成

在 Waybar 配置中添加自定义模块：

**~/.config/waybar/config.jsonc**
```jsonc
{
  "custom/bard": {
    "exec": "/path/to/waybar-bard",
    "format": "{}\n <span font='11' fgalpha='50%' style='italic'>{alt}</span>",
    "return-type": "json",
    "restart-interval": 5,
    "signal": 1,
    "tooltip": true,
    "hide-empty-text":true
  }
}
```

**~/.config/waybar/style.css**
```css
#custom-bard {
  background-color: @surface_container;
  color: @on_surface_variant;
}
```
## dms集成
确保`waybar-bard`可执行文件在路径中，并将`dms-bard`放入dms plugin文件夹中。

## 开发

```bash
# 检查代码
just check

# 格式化
just fmt

# Lint
just clippy

```

## 许可证

MIT License
