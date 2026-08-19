# PixNya 首个正式版发布清单

> 状态：首个稳定版与后续 `1.1.0`–`1.4.3` 已发布
> 当前升级基线：公开稳定版 `1.4.3`
> 正式支持：Windows x64/ARM64（NSIS）、Linux x64（AppImage）、Android ARM64/ARM32（split APK）

候选生命周期：固定 `main` 提交先生成五个平台签名 Draft；完成自动验证和本轮声明的活体验收后，再由独立 Publish 工作流复验并切换为 latest stable。生产签名密钥保持不变。

## 1. 正式版边界

- 当前稳定范围包括登录、浏览、搜索、插画/小说收藏、评论与只读通知、图片/Ugoira、下载、离线资料库、本机历史、数据备份、三种连接模式和自动更新。
- Android 最低 API 29；正式 Release 同时提供 `arm64-v8a` 与 `armeabi-v7a` split APK。
- 投稿、个人资料编辑、通知写操作、私信、直播与多账号保持“不支持”，统一记录在[备选功能计划](OPTIONAL_FEATURES_PLAN.md)。
- PixNya 是非官方客户端，不是 Pixiv 官方产品。

## 2. 已自动化的发布闸门

- [x] 发布工作流只能从 `main` 分支的固定提交触发，并校验 npm、Cargo、Tauri 与请求版本一致。
- [x] 签名构建开始前运行 `npm run test:full`，覆盖全部 Node 回归、Svelte 检查、Rust 格式、Clippy 与 workspace tests。
- [x] 发布前阻断运行时 npm 低危以上告警、全部 npm 高危以上告警、RustSec advisory，并用 OSV 扫描 Android ARM APK 的共同 runtime 锁图；runtime 零例外。构建工具图使用重新扫描确认的 84 条精确、限 scope、限版本、限期的临时 OSV 基线（79 条于 2026-09-08 到期，Kotlin 构建缓存告警于 2026-09-12 到期，Bouncy Castle 的两条 Moderate 告警于 2026-09-16 到期，Netty 的两条 Moderate 告警于 2026-09-17 到期）；此前唯一的 Critical Bouncy Castle 例外已通过升级到 `1.80.2` 消除。新增、变化或到期即失败，原始报告随每个 Release 归档。
- [x] Windows、Linux 与 Android 构建均要求完整的生产构建参数和签名 Secret，缺少任意一项立即失败。
- [x] Android Release 只允许经过验证的 ARM64 与 ARM32 分包 APK，并用 `apksigner` 反查实际 APK 证书与受保护 keystore 一致。
- [x] Draft Release 创建前，用公开密钥重新验证 Windows/Linux updater 签名和 Android 清单签名。
- [x] Draft Release 只公开五个平台安装包、三个更新文件、`SHA256SUMS.txt` 和一个审计资料包；`BUILD-PROVENANCE.txt`、SBOM、许可证与独立签名统一收进 `pixnya-<version>-verification.tar.gz`。
- [x] Draft Release 的 SPDX 2.3 SBOM 与逐依赖许可证归档覆盖 npm、Cargo 和最终 Android Gradle/Maven 锁图；清理无效配置后重新解析的 343 组件 Gradle 图及其 378 份组件/父 POM 证据已重建并离线复核。
- [x] 所有平台和附件验证成功后，Release job 才通过 Git refs API 原子创建 tag 并回读确认其指向已验证 artifact 的源码 SHA；若上传中断，只允许复用同一源码 SHA 且尚无 Release/仍为 Draft 的幂等续传，已发布或不同源码 SHA 一律失败，上传后再次核对 tag 与 10 个公开附件。

## 3. 发布闸门与后续治理

3.1、3.3 与 3.4 是发布闸门；3.2 和 3.5 中明确标为“后续非阻塞”的未勾选项只用于持续加固，不否定已经通过自动检查、签名和活体验收的稳定版。

### 3.1 固定可复现源码

- [x] 审查当前候选改动，确认正式包中没有临时原型和调试残留。
- [x] 在本机执行 `npm run test:full`。
- [x] 固定候选提交在 GitHub Actions 完成 Linux 与全部正式平台验证。
- [x] 提交并推送完整候选源码，发布时确认 `HEAD`、`origin/main` 与来源 SHA 一致。
- [x] 所有用户可见版本元数据由发布边界测试强制一致。
- [x] 从固定提交 `95e7bf74a7f9ab5cc7cbe13f460c29e2a8580705` 触发 `1.4.3` 签名 Draft；GitHub Actions run `32128825447` 完成五平台构建、签名与 10 附件汇总，run `32242090056` 复用同一批不可变 artifacts 更新 provenance，Publish run `32242238581` 复验后公开。

### 3.2 长期签名材料与后续非阻塞治理

- [x] 提供不把密码写入文件或命令行、使用工作树外 staging 原子落盘、并可从既有恢复目录重新验密和幂等上传全部 Secrets 的交互式初始化脚本 `scripts/provision-release-signing.ps1`。
- [x] Tauri updater 长期密钥已生成，私钥已保存两份离线备份。
- [x] Android Release keystore 已生成，alias、证书 SHA-256 和恢复说明已记录，并已保存两份离线备份。
- [x] 独立的 Android 更新清单签名密钥已生成，并已保存两份离线备份。
- [x] GitHub 已创建 `production-release` 环境并只允许 `main` 分支部署。
- [x] 13 个生产构建与签名 Secrets 已配置到 `production-release` 环境，并核对名称完整。
- [ ] 后续非阻塞：为 `production-release` 启用 required reviewers；当前签名工作流已限制为 `main` 与受保护环境。
- [x] `v*` 已配置禁止更新和删除的 tag ruleset；工作流负责原子创建，仓库规则阻止 tag 被移动或删除。
- [ ] 后续非阻塞：在隔离测试仓库用一份故意错误的公钥运行完整发布验证；当前仓库已有错误公钥/签名的自动回归，不用生产 Release 做破坏性演练。

### 3.3 匿名更新源

- [x] 主仓库已公开，源码、tag 与更新附件处于同一匿名可读信任边界。
- [x] 应用不内置 GitHub 私人访问令牌。
- [x] 桌面与 Android 正式端点均为编译期绑定并经过验证的 HTTPS 地址。
- [x] 更新客户端、重定向校验与清单生成器统一绑定编译期 `owner/repository`，仓库错配会被拒绝。

### 3.4 真实安装与升级验收

- [x] 已使用同一套生产密钥发布公开 `1.4.0` 升级基线。
- [x] Windows x64：从 `1.4.0` 升级到 `1.4.3` Draft；NSIS、两版启动、版本注册、非敏感 marker、业务数据库、字号和日志按钮通过。该 Windows 基线未登录且没有现成离线作品，账号凭据与真实离线内容保留由 Android 主设备验收覆盖。
- [x] Android ARM64：`1.4.3` Draft 真机覆盖安装通过，系统安装器、版本码、登录状态、设置、数据库、离线文件、备份功能、ECH 缩略图和新字号布局均由维护者确认正常。
- [x] 标准、ECH、兼容三种连接模式在 Android ARM64 的低频登录、API 与图片加载冒烟测试通过。
- [x] Linux x64、Windows ARM64、Android ARM32 完成 `1.4.3` 签名 CI 构建；本轮不宣称已做活体验收。

### 3.5 公开发布治理

- [x] 已在 `docs/PUBLIC_DISTRIBUTION_DECISION.md`、README、`SECURITY.md` 与 `PRIVACY.md` 集中记录上游 App API、OAuth 参数和风险边界；Release 正文保持简短，只有长期边界发生实质变化时才明确提示并链接更新后的说明。
- [x] 标准 GPL-3.0 正文、第三方清单、逐依赖归档与可复现 SPDX SBOM 已在最终 Gradle 锁图上完成 Maven 许可证证据闭环；非标准化条款以明确的 `LicenseRef` 和上游声明证据保留。
- [x] 增加 `PRIVACY.md` 与 `SECURITY.md`，说明本地数据、无遥测、低安全连接风险和私密漏洞报告方式。
- [ ] 后续非阻塞：取得 Authenticode 证书后加入 Windows 代码签名；按当前产品决定，简洁下载说明不重复 SmartScreen 警告。

## 4. Draft 到 stable 的人工步骤

1. 按 [`RELEASE_NOTES_TEMPLATE.md`](RELEASE_NOTES_TEMPLATE.md) 填写公开变更和支持平台；从 `main` 的固定 SHA 手动运行 “Draft signed release”。升级测试使用 Draft 中的五平台签名附件，但公开稳定版说明不得保留 `PENDING` 或模板占位符。
2. 下载 Draft 的全部附件，核对文件名、大小、SHA-256、签名、ABI、应用 ID 与版本。
3. 在 Windows x64 与 Android ARM64 完成上节的原地升级和数据保留测试，并记录设备、系统版本、源版本、目标版本与结果。
4. 确认匿名环境能够读取 `latest.json`、`android-latest.json` 和对应安装包。
5. 把真实设备、系统、`PASS`、失败路径结果和未活体验收平台写回验收记录，再从相同 `main` SHA 运行 “Publish verified stable release”。该工作流会重新下载并严格验证 10 个公开附件、审计资料包、SHA-256、来源记录、tag、发布说明和三套签名；只有 Draft 在验证期间未变化才会切换为 stable。
6. 任一检查失败时保留 Draft 供诊断，不移动 stable 更新入口；禁止在 GitHub 页面绕过最终化工作流直接点击发布。

## 5. 当前结论

`1.4.3` 已是公开 latest stable；固定候选、五平台签名、10 个附件、Windows x64 与 Android ARM64 升级/数据/界面验收、匿名更新清单及三套签名均已复验。未公开的 `1.4.2`/`1.4.1` Draft Release 已删除；历史 tag 因仓库不可删除 ruleset 保持原 SHA。
