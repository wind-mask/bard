## [unreleased]

## [0.10.0] - 2026-07-17

### ⚠️ Breaking Changes

- Waybar JSON class 从 `no-song` 拆分为 `no-player` / `paused`
- 无同步歌词状态从 `has-song` 更名为 `no-lyrics`
- DMS 插件同步升级到 1.1.0，不再兼容旧 class 名称

### 🚀 Features

- 增加单一 coordinator、粘滞播放器选择与暂停感知 PlaybackClock
- 增加 `--offset-ms` 全局歌词校准参数，默认 100ms
- 支持多时间戳、`[offset:]`、常见元数据及行级增强 LRC
- 同一时间戳的翻译/罗马音去重后以 ` / ` 合并
- DMS 异常退出采用指数退避重启，并强制纯文本渲染
- Add lofty dependency and implement lyrics fetching from audio files
- Implement signal handling to toggle output visibility and add rendering functions
- *(dms-bard)* 增加了 DMS 插件

### 🐛 Bug Fixes

- 修复暂停后歌词仍继续推进
- 修复 TrackChanged 到达早于 Position 更新导致的歌词错位
- 修复重复 Seeked 信号及旧播放器迟到事件覆盖当前状态
- 修复负 LRC offset 饱和后错误合并不同歌词行
- *(fix-some-parse)* Parse lyrics with word timing tags
- *(时间戳对齐)* 改进时间戳对齐

### 🚜 Refactor

- 将多线程共享 `RwLock<AppState>` 改为 watcher 发送事件、coordinator 独占状态
- 使用 `Duration` 统一播放器和歌词时间模型
- 删除未接入的 Tidal 认证代码
- Clean up unused code and improve lyrics handling
- Remove bard crate and clean up related configurations and code
- *(简化)* 仅使用本地文件元数据歌词
