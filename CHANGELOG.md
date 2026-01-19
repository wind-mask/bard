## [unreleased]

### 🚀 Features

- Add lofty dependency and implement lyrics fetching from audio files
- Implement signal handling to toggle output visibility and add rendering functions
- *(dms-bard)* 增加了dms插件；README文件

### 🐛 Bug Fixes

- *(fix-some-parse)* Parse lyrics with <timing>
- *(时间戳对齐)* 改进了时间戳的对齐

### 💼 Other

- Migrate lyrics go project to rust

### 🚜 Refactor

- Clean up unused code and improve lyrics handling
- Remove bard crate and clean up related configurations and code
- *(清除不必要的依赖)* 在添加网络功能前不再依赖
- *(简化)* 去除了我不需要的部分；现在仅使用元数据歌词优化了waybar-bard的循环
