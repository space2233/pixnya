# 来源与独立实现说明

本项目没有引入或复制 PixEz 的实现代码，也没有把 PixEz、Flutter、Dart 包或其仓库作为依赖、子模块或构建输入。

研究文档会引用 PixEz、PixivPy 等公开仓库，用于核对可观察的网络行为、端点名称、参数结构和平台限制。编译代码则使用 Rust、Tauri 与 Svelte 独立编写，并通过项目自己的模型、连接策略、安全边界和测试实现这些能力。

具体边界如下：

- `docs/research/` 中允许出现上游链接、行为摘要和为本项目设计的伪代码；
- `crates/`、`src/` 与 `src-tauri/src/` 不包含 PixEz 仓库引用或 PixEz 包引用；
- `Cargo.toml`、`package.json` 与锁文件不包含 PixEz、Flutter 或 Dart 依赖；
- `scripts/source-boundaries-regression.test.mjs` 会持续检查上述编译边界。

端点字符串、协议字段和服务端返回结构属于互操作所需的接口事实；本项目不据此宣称这些非公开 App API 获得 Pixiv 的稳定性承诺或官方授权。
