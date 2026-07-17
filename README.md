# 在 Waybar / DMS 中显示同步歌词

![banner](banner.png)

基于 [puszkarek/bard](https://github.com/puszkarek/bard) 精简而来。项目只读取本地音乐文件元数据中的同步 LRC，不请求在线歌词服务；无时间戳的纯文本歌词会被视为“无同步歌词”。

## 功能

- 通过 MPRIS D-Bus 协议跟踪播放器
- 从本地音频标签读取同步歌词
- 支持多时间戳、`[offset:]`、1～3 位小数及常见增强 LRC 逐词标签
- 同一时间戳的附加文本作为翻译/罗马音显示
- Waybar JSON-lines 输出
- DMS 桌面歌词插件

## 构建

需要 Rust 2024 edition 工具链：

```bash
git clone https://github.com/wind-mask/bard.git
cd bard
just build-waybar-bard
# 或
cargo build --release
```

完整本地验证：

```bash
just ci
```

`just ci` 会依次执行格式检查、所有目标的 check/test、严格 Clippy 和 release 构建。

## 命令行

```text
waybar-bard [--offset-ms <MILLISECONDS>]
```

`--offset-ms` 是应用在 LRC 文件 `[offset:]` 之后的全局校准值，可以为负数，默认值为 `100`。例如提前 250ms 显示：

```bash
waybar-bard --offset-ms -250
```

## Waybar 集成

在 `~/.config/waybar/config.jsonc` 中加入持续运行的自定义模块：

```jsonc
{
  "custom/bard": {
    "exec": "/path/to/waybar-bard --offset-ms 100",
    "format": "{}\n<span font='11' fgalpha='50%' style='italic'>{alt}</span>",
    "return-type": "json",
    "restart-interval": 5,
    "tooltip": true,
    "escape": true,
    "hide-empty-text": true,
    "on-click": "pkill -USR1 -x waybar-bard"
  }
}
```

`escape: true` 会把歌词作为纯文本处理，避免音频标签中的 `<`、`>`、`&` 被解释为 Pango 标记。

Waybar 的 `signal` 配置表示 `SIGRTMIN+N`，不会把 `SIGUSR1` 发送给持续运行的子进程，因此不应使用旧的 `"signal": 1` 示例。上面的 `on-click` 会向所有名为 `waybar-bard` 的实例发送 `SIGUSR1`；Waybar 与 DMS 同时运行时，两者会一起切换隐藏状态。

示例 CSS：

```css
#custom-bard {
  background-color: @surface_container;
  color: @on_surface_variant;
}

#custom-bard.hidden,
#custom-bard.no-player,
#custom-bard.paused {
  background: transparent;
}
```

### JSON class 契约

| class | 含义 | text |
| --- | --- | --- |
| `hidden` | SIGUSR1 手动隐藏 | 空 |
| `no-player` | 没有可用播放器或 Position | 空 |
| `paused` | 当前播放器暂停 | 空 |
| `no-lyrics` | 正在播放但无同步歌词 | `Artist - Title` |
| `has-lyrics` | 正在显示同步歌词 | 当前歌词 |

`alt` 优先输出当前行的翻译/罗马音；没有附加文本时输出下一句歌词。

## DMS 集成

确保 `waybar-bard` 在 `PATH` 中，并将 `dms-bard` 目录复制到 DMS 插件目录，然后添加 Desktop Lyrics 桌面部件。

DMS 默认执行：

```qml
command: ["waybar-bard"]
```

如需校准偏移，可手工改为：

```qml
command: ["waybar-bard", "--offset-ms", "250"]
```

插件将歌词强制按纯文本显示。异常退出时采用 1、2、4、8、16、30 秒的指数退避重启；正常退出不会自动重启。

## 从旧 class 迁移

0.10.0 的输出 class 有破坏性调整：

```text
no-song  -> no-player / paused
has-song -> no-lyrics
```

`hidden` 与 `has-lyrics` 保持不变。

## 开发

```bash
just fmt       # 格式化
just check     # 全目标检查
just test      # 全目标测试
just clippy    # 严格 Clippy
just ci        # 完整本地验证
```

## 许可证

MIT License
