# PixNya 首个正式版发布清单

> 状态：`1.0.0` 正式候选准备中，生产签名升级基线为 `0.29.0`
> 计划正式版本：`1.0.0`；完成发布闸门前不发布 stable
> 正式支持：Windows x64（NSIS）、Linux x64（AppImage）、Android ARM64（APK）

候选生命周期：同一套生产密钥签名的 `0.29.0` 基线和本地全量自动检查完成后，源码允许进入 `1.0.0` Draft 候选，以生成真实升级验收所需的签名安装包；这次版本迁移不等于 stable 发布。只有固定候选提交、公开仓库、三平台升级与失败路径验证全部完成后，才允许把 Draft 切换为 stable。

## 1. 正式版边界

- 首个正式版只承诺已经实现并有自动回归覆盖的登录、浏览、搜索、账号与作品操作、图片/Ugoira、小说、下载、离线资料库、本机历史、三种连接模式和自动更新。
- Android 最低 API 29；首发只提供 `arm64-v8a`。ARMv7 调试构建入口保留，但不进入正式 Release。
- 投稿、私信、直播、多账号、完整站内通知、评论删除与更多回复管理等能力保持明确的“不支持”状态，统一记录在[备选功能计划](OPTIONAL_FEATURES_PLAN.md)。
- PixNya 是非官方客户端，不是 Pixiv 官方产品；账号密码只在 Pixiv 官方登录页输入。

## 2. 已自动化的发布闸门

- [x] 发布工作流只能从 `main` 分支的固定提交触发，并校验 npm、Cargo、Tauri 与请求版本一致。
- [x] 签名构建开始前运行 `npm run test:full`，覆盖全部 Node 回归、Svelte 检查、Rust 格式、Clippy 与 workspace tests。
- [x] 发布前阻断运行时 npm 低危以上告警、全部 npm 高危以上告警、RustSec advisory，并用 OSV 扫描 ARM64 APK 的 `arm64ReleaseRuntimeClasspath`；runtime 零例外。构建工具图使用重新扫描确认的 82 条精确、限 scope、限版本、限期的临时 OSV 基线（1 条 Critical 于 2026-08-23 到期，其余 81 条于 2026-09-08 到期），新增、变化或到期即失败，原始报告随每个 Release 归档。
- [x] Windows、Linux 与 Android 构建均要求完整的生产构建参数和签名 Secret，缺少任意一项立即失败。
- [x] Android Release 只允许一个 ARM64 APK，并用 `apksigner` 反查实际 APK 证书与受保护 keystore 一致。
- [x] Draft Release 创建前，用公开密钥重新验证 Windows/Linux updater 签名和 Android 清单签名。
- [x] Draft Release 只接受唯一的桌面 updater 归档，并生成 `SHA256SUMS.txt` 与记录源提交的 `BUILD-PROVENANCE.txt`。
- [x] Draft Release 的 SPDX 2.3 SBOM 与逐依赖许可证归档覆盖 npm、Cargo 和最终 Android Gradle/Maven 锁图；清理无效配置后重新解析的 343 组件 Gradle 图及其 378 份组件/父 POM 证据已重建并离线复核。
- [x] 所有平台和附件验证成功后，Release job 才通过 Git refs API 原子创建 tag 并回读确认其指向已验证 artifact 的源码 SHA；若上传中断，只允许复用同一源码 SHA 且尚无 Release/仍为 Draft 的幂等续传，已发布或不同源码 SHA 一律失败，上传后再次核对 tag 与 18 个附件。

## 3. 发布前阻塞项

### 3.1 固定可复现源码

- [x] 审查当前候选改动，确认正式包中没有临时原型和调试残留。
- [x] 在本机执行 `npm run test:full`。
- [ ] 从固定候选提交在 GitHub Actions 确认 Linux 验证通过。
- [ ] 提交并推送完整的候选源码，确认 `HEAD`、`origin/main` 与准备发布的提交 SHA 一致。
- [x] 所有用户可见版本元数据统一改为 `1.0.0` Draft 候选。
- [ ] 从完成审查与自动检查的固定提交触发 `1.0.0` Draft 发布工作流。

### 3.2 长期签名材料

- [x] 提供不把密码写入文件或命令行、使用工作树外 staging 原子落盘、并可从既有恢复目录重新验密和幂等上传全部 Secrets 的交互式初始化脚本 `scripts/provision-release-signing.ps1`。
- [x] Tauri updater 长期密钥已生成，私钥已保存两份离线备份。
- [x] Android Release keystore 已生成，alias、证书 SHA-256 和恢复说明已记录，并已保存两份离线备份。
- [x] 独立的 Android 更新清单签名密钥已生成，并已保存两份离线备份。
- [x] GitHub 已创建 `production-release` 环境并只允许 `main` 分支部署。
- [x] 13 个生产构建与签名 Secrets 已配置到 `production-release` 环境，并核对名称完整。
- [ ] 主仓库公开后为 `production-release` 启用维护者审核；当前私人仓库套餐不支持 required reviewers。
- [ ] 为 `v*` 配置禁止更新和删除的 tag ruleset；工作流只负责原子创建，仓库规则负责阻止创建后的 tag 被移动。
- [ ] 用一份故意错误的公钥运行发布验证，确认工作流 fail closed。

### 3.3 匿名更新源

- [ ] 在发布 stable 前公开主仓库，使源码、tag 与更新附件处于同一匿名可读信任边界。
- [ ] 不在应用中内置 GitHub 私人访问令牌。
- [ ] 确认桌面与 Android 正式端点均为经过固定验证的 HTTPS 地址。
- [x] 更新客户端、重定向校验与清单生成器统一绑定编译期 `owner/repository`，仓库错配会被拒绝。

### 3.4 真实安装与升级验收

- [x] 已使用同一套生产密钥发布并保存 `0.29.0` 签名基线安装包。
- [ ] Windows x64：从基线升级到 `1.0.0`，验证 NSIS、重启、登录状态、设置、数据库和离线文件。
- [ ] Linux x64：在至少一个支持的发行版从基线 AppImage 升级，验证 WebKitGTK 登录和数据保留。
- [ ] Android ARM64：真机覆盖安装，验证系统安装器、未知来源授权、APK 证书、版本码和数据保留。
- [ ] 三个平台验证断网、错误签名、损坏清单、下载中断、空间不足、取消安装与重试。
- [ ] 标准、ECH、兼容三种连接模式各完成一次低频登录与图片加载冒烟测试。

### 3.5 公开发布治理

- [x] 已在 `docs/PUBLIC_DISTRIBUTION_DECISION.md` 记录上游 App API、OAuth 参数与公开分发决定；公开 Release 仍必须复述其中的风险边界。
- [x] 标准 GPL-3.0 正文、第三方清单、逐依赖归档与可复现 SPDX SBOM 已在最终 Gradle 锁图上完成 Maven 许可证证据闭环；非标准化条款以明确的 `LicenseRef` 和上游声明证据保留。
- [x] 增加 `PRIVACY.md` 与 `SECURITY.md`，说明本地数据、无遥测、低安全连接风险和私密漏洞报告方式。
- [ ] Windows 若没有 Authenticode 证书，在下载页明确 SmartScreen 提示；取得证书后再加入代码签名。

## 4. Draft 到 stable 的人工步骤

1. 按 [`RELEASE_NOTES_TEMPLATE.md`](RELEASE_NOTES_TEMPLATE.md) 填写公开分发、许可与校验信息；从 `main` 的固定 SHA 手动运行 “Draft signed release”。因为升级测试需要签名附件，Draft 中三平台结果、失败路径和已知限制可暂时明确写成 `PENDING after Draft artifacts`，但不能保留 `{{...}}` 模板占位符。
2. 下载 Draft 的全部附件，核对文件名、大小、SHA-256、签名、ABI、应用 ID 与版本。
3. 在三平台完成上节的原地升级和数据保留测试，并记录测试设备、系统版本、源版本、目标版本与结果。
4. 确认匿名环境能够读取 `latest.json`、`android-latest.json` 和对应安装包。
5. 把真实设备、系统、`PASS`、失败路径结果和已知限制写回 Draft 说明，再从相同 `main` SHA 运行 “Publish verified stable release”。该工作流会重新下载并严格验证 18 个附件、SHA-256、来源记录、tag、发布说明和三套签名；只有 Draft 在验证期间未变化才会切换为 stable。
6. 任一检查失败时保留 Draft 供诊断，不移动 stable 更新入口；禁止在 GitHub 页面绕过最终化工作流直接点击发布。

## 5. 当前结论

源码已经迁移到 `1.0.0` 正式候选，生产签名材料和 `0.29.0` Release 基线已经完成。stable 发布仍需固定并验证候选提交、公开主仓库，以及补齐 Windows、Linux、Android 三平台真实升级证据。
