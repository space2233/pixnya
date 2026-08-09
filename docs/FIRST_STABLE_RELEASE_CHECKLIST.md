# PixNya 首个正式版发布清单

> 状态：候选版准备中，当前源码版本仍为 `0.29.0`
> 计划正式版本：`1.0.0`；完成发布闸门前不提前改版本号或发布 stable
> 正式支持：Windows x64（NSIS）、Linux x64（AppImage）、Android ARM64（APK）

## 1. 正式版边界

- 首个正式版只承诺已经实现并有自动回归覆盖的登录、浏览、搜索、账号与作品操作、图片/Ugoira、小说、下载、离线资料库、本机历史、三种连接模式和自动更新。
- Android 最低 API 29；首发只提供 `arm64-v8a`。ARMv7 调试构建入口保留，但不进入正式 Release。
- 投稿、私信、直播、多账号、完整站内通知、评论删除与更多回复管理等能力保持明确的“不支持”状态，统一记录在[备选功能计划](OPTIONAL_FEATURES_PLAN.md)。
- PixNya 是非官方客户端，不是 Pixiv 官方产品；账号密码只在 Pixiv 官方登录页输入。

## 2. 已自动化的发布闸门

- [x] 发布工作流只能从 `main` 分支的固定提交触发，并校验 npm、Cargo、Tauri 与请求版本一致。
- [x] 签名构建开始前运行 `npm run test:full`，覆盖全部 Node 回归、Svelte 检查、Rust 格式、Clippy 与 workspace tests。
- [x] Windows、Linux 与 Android 构建均要求完整的生产构建参数和签名 Secret，缺少任意一项立即失败。
- [x] Android Release 只允许一个 ARM64 APK，并用 `apksigner` 反查实际 APK 证书与受保护 keystore 一致。
- [x] Draft Release 创建前，用公开密钥重新验证 Windows/Linux updater 签名和 Android 清单签名。
- [x] Draft Release 只接受唯一的桌面 updater 归档，并生成 `SHA256SUMS.txt` 与记录源提交的 `BUILD-PROVENANCE.txt`。
- [x] Release tag 明确指向触发工作流的 `github.sha`，所有平台成功后才创建 Draft Release。

## 3. 发布前阻塞项

### 3.1 固定可复现源码

- [ ] 审查当前未提交改动，移除正式包中的临时原型和调试残留。
- [ ] 在本机执行 `npm run test:full`，在 GitHub Actions 确认 Linux 验证通过。
- [ ] 提交并推送完整的候选源码，确认 `HEAD`、`origin/main` 与准备发布的提交 SHA 一致。
- [ ] 所有用户可见版本元数据统一改为 `1.0.0`，再从该固定提交触发发布工作流。

### 3.2 长期签名材料

- [ ] 生成 Tauri updater 长期密钥，私钥至少保存两份离线备份。
- [ ] 生成 Android Release keystore，记录 alias、证书 SHA-256 和恢复说明，并至少保存两份离线备份。
- [ ] 生成独立的 Android 更新清单签名密钥并离线备份。
- [ ] 将所需 Secrets 配置到 GitHub Actions；当前仓库尚未配置任何发布 Secret。
- [ ] 用一份故意错误的公钥运行发布验证，确认工作流 fail closed。

### 3.3 匿名更新源

- [ ] 在发布 stable 前让更新附件可匿名读取：公开主仓库，或使用独立的公开 Release 仓库/静态更新源。
- [ ] 不在应用中内置 GitHub 私人访问令牌。
- [ ] 确认桌面与 Android 正式端点均为经过固定验证的 HTTPS 地址。

### 3.4 真实安装与升级验收

- [ ] 先使用同一套生产密钥生成并保存一个 `0.29.0` 签名基线安装包。
- [ ] Windows x64：从基线升级到 `1.0.0`，验证 NSIS、重启、登录状态、设置、数据库和离线文件。
- [ ] Linux x64：在至少一个支持的发行版从基线 AppImage 升级，验证 WebKitGTK 登录和数据保留。
- [ ] Android ARM64：真机覆盖安装，验证系统安装器、未知来源授权、APK 证书、版本码和数据保留。
- [ ] 三个平台验证断网、错误签名、损坏清单、下载中断、空间不足、取消安装与重试。
- [ ] 标准、ECH、兼容三种连接模式各完成一次低频登录与图片加载冒烟测试。

### 3.5 公开发布治理

- [ ] 明确上游 App API、OAuth 参数与公开分发的风险决定，并写入 Release 说明。
- [ ] 补齐标准 GPL-3.0 许可证正文、第三方依赖/许可证清单。
- [x] 增加 `PRIVACY.md` 与 `SECURITY.md`，说明本地数据、无遥测、低安全连接风险和私密漏洞报告方式。
- [ ] Windows 若没有 Authenticode 证书，在下载页明确 SmartScreen 提示；取得证书后再加入代码签名。

## 4. Draft 到 stable 的人工步骤

1. 从 `main` 的固定 SHA 手动运行 “Draft signed release”，输入与源码一致的版本和最终发布说明。
2. 下载 Draft 的全部附件，核对文件名、大小、SHA-256、签名、ABI、应用 ID 与版本。
3. 在三平台完成上节的原地升级和数据保留测试，并记录测试设备、系统版本、源版本、目标版本与结果。
4. 确认匿名环境能够读取 `latest.json`、`android-latest.json` 和对应安装包。
5. 所有阻塞项关闭后才公开 Release；失败时保留 Draft 供诊断，不移动 stable 更新入口。

## 5. 当前结论

代码结构已经接近正式候选版，但 `1.0.0` 目前仍被四项外部条件阻塞：固定候选提交、生产签名材料、匿名更新源、三平台真实升级证据。完成这些条件之前可以继续完善自动化和清理正式构建边界，但不能把现有 `0.29.0` Debug 产物当作正式更新链起点。
