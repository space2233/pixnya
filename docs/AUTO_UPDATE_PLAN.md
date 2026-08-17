# PixNya 自动检查更新与自动更新计划

> 状态：生产签名更新流程已投入使用；当前公开稳定版为 `1.4.0`，维护候选为 `1.4.1`
> 更新日期：2026-08-17
> 当前正式平台：Windows x64/ARM64、Linux x64、Android ARM64/ARM32

首个正式版的逐项发布状态与人工验收记录见[首个正式版发布清单](FIRST_STABLE_RELEASE_CHECKLIST.md)。

已确认：产品名为 **PixNya**；最终应用 ID 为 `io.github.space2233.pixnya`；更新源使用公开的 GitHub Releases；Android 交给系统安装组件，并始终保留用户确认。Windows x64/ARM64、Linux x64、Android ARM64/ARM32 均已进入正式签名矩阵。

## 1. 产品定义

本计划中的“自动更新”分为三个独立动作：

1. **自动检查**：应用启动完成后检查一次，此后最多每 24 小时检查一次；设置页保留“立即检查”。
2. **自动下载**：发现稳定版后，根据用户设置下载正确平台和架构的产物，并显示进度、大小和发布说明。
3. **安装更新**：Windows/Linux 在用户确认后交给桌面 updater 安装；Android 交给系统安装器，始终准备处理用户确认和“允许此来源安装应用”的系统设置。

默认建议：自动检查开启；自动下载关闭；从不静默安装。首个稳定版只提供 `stable` 通道，不做灰度、测试通道和降级。

## 2. 平台路线

| 平台 | 检查与下载 | 安装 | 首个稳定版限制 |
|---|---|---|---|
| Windows x64/ARM64 | Tauri updater + GitHub Releases `latest.json` | NSIS `passive`，应用内确认后执行 | `latest.json` 按 `windows-x86_64` / `windows-aarch64` 选择独立安装包 |
| Linux x64 | Tauri updater + 同一桌面清单 | AppImage 更新后重启 | deb/rpm 只提示前往发布页，暂不自更新 |
| Android ARM64/ARM32 | 项目自有签名清单 + 两个 split APK | Android `PackageInstaller`/安装 Intent | 按 `arm64-v8a` / `armeabi-v7a` 选择；可能需要未知来源授权和系统确认 |

Tauri 官方 updater 只支持桌面平台，并要求使用内置公钥验证更新签名；Android 需要独立 Adapter。Android 接受覆盖安装至少要求 application ID、签名证书一致，且 `versionCode` 不低于已安装版本。

## 3. 模块设计

建立一个深模块 `UpdateManager`。页面不接触 GitHub URL、清单 JSON、签名格式、ABI、安装权限或临时文件，只使用以下 interface：

```rust
pub enum UpdateTrigger {
    Startup,
    Scheduled,
    Manual,
}

pub enum UpdateState {
    UpToDate,
    Available(UpdateSummary),
    Downloading(UpdateProgress),
    ReadyToInstall(UpdateSummary),
    AwaitingSystemAction,
    Failed(UpdateFailure),
}

pub trait UpdateManager {
    async fn check(&self, trigger: UpdateTrigger) -> Result<UpdateState, UpdateError>;
    async fn download(&self) -> Result<UpdateState, UpdateError>;
    async fn install(&self) -> Result<UpdateState, UpdateError>;
}
```

外部 interface 的不变量：

- 同一时间只有一个检查、下载或安装任务；
- 只接受高于当前版本的稳定版，不允许远端请求降级；
- 更新流量永远使用系统 DNS/代理和经过验证的 HTTPS，不使用 Pixiv ECH 或低安全直连；
- 检查失败不影响应用启动和现有 Pixiv 功能；
- UI 只能展示归一化状态，不读取远端原始清单。

内部在安装 seam 上保留两个真实 Adapter：

- `DesktopTauriUpdaterAdapter`：包装 Tauri updater，负责 Windows/Linux 签名包下载、安装和重启。
- `AndroidPackageInstallerAdapter`：选择 ABI APK，验证后调用 Android 系统安装器。

测试使用内存 Adapter，覆盖新版本、无更新、损坏签名、错误 ABI、下载中断、空间不足、用户取消和重启恢复。

## 4. 发布源与清单

首个稳定版使用 GitHub Releases，不建设自有更新服务器。每个稳定版发布以下文件：

- Windows x64/ARM64 NSIS 更新包及各自的 Tauri `.sig`；
- Linux AppImage 及 Tauri `.sig`；
- `latest.json`，供桌面 Tauri updater 使用；
- ARM64 与 ARM32（`armeabi-v7a`）Release split APK；
- `android-latest.json` 与其 Ed25519 签名；
- `SHA256SUMS.txt`、发布说明和构建来源信息。

Android 清单至少包含：schema 版本、`versionName`、`versionCode`、发布日期、发布说明、最低 Android 版本，以及每个 ABI 的 URL、大小、SHA-256、包名和预期 APK 签名证书 SHA-256。

发布流程必须先创建 GitHub Draft Release，上传并验证全部产物，最后再原子性地公开 Release，避免客户端看到不完整版本。

## 5. 密钥与安全规则

- Tauri updater 私钥和 Android Release keystore 是两套独立长期密钥，均不得进入仓库或普通构建机备份目录。
- 客户端只内置 Tauri updater 公钥、Android 清单公钥和预期 Android Release 证书摘要。
- 当前 Debug APK 的调试证书不能作为正式更新链起点；首个可自动更新版本必须改用稳定 Release keystore。
- Android 下载完成后依次验证清单签名、目标版本、ABI、文件大小、SHA-256、包名和 APK 签名证书，再交给系统安装器。
- 更新 URL 只允许经过验证的 HTTPS 和固定发布源；重定向后重新检查 scheme 与主机。
- 更新文件写入应用私有临时目录，使用现有存储保留区策略；失败或取消后安全清理。
- 不记录完整下载 URL 查询、设备标识、账号状态或 Pixiv 会话数据。

## 6. 用户界面与设置

设置中心新增“应用更新”区域：

- 当前版本、通道和上次检查时间；
- “自动检查更新”开关，默认开启；
- “自动下载更新”开关，默认关闭；
- “仅在非计费网络自动下载”，Android 默认开启；
- “立即检查更新”；
- 可用版本、发布日期、大小和发布说明；
- 下载进度、重试、取消和“安装并重启/打开系统安装器”；
- 检查失败只显示可重试状态，不阻塞正常使用。

首个稳定版不提供“自动静默安装”开关。安全更新可以提高提示显著性，但仍不绕过用户确认。

## 7. 实施顺序

### 阶段 A：发布基础

- 确定 GitHub 仓库与 Release 地址。
- 生成并离线备份 Tauri updater 私钥和 Android Release keystore。
- 建立 Windows NSIS、Linux AppImage、Android Release APK 构建。
- CI 从同一版本生成签名产物、两个更新清单和校验和。

### 阶段 B：只检查不安装

- 实现 `UpdateManager.check`、版本比较、频率限制和设置页状态。
- 使用本地 HTTP/TLS fixture 测试有效、过期、错误签名和不完整清单。
- 先在 Debug 包中指向测试 Release，禁止连接生产更新通道。

### 阶段 C：桌面自动更新

- 接入仅桌面编译的 Tauri updater 与 process/relaunch 插件。
- Windows 验证 NSIS 更新；Linux 验证 AppImage 更新。
- 覆盖下载中断、安装取消、应用重启和旧版本数据迁移。

### 阶段 D：Android 自动更新

- 建立 Kotlin/Tauri Android 更新插件。
- 实现 ABI 选择、断点下载、全套验证和系统安装流程。
- 覆盖未知来源未授权、用户取消、签名不一致、空间不足和安装成功后的版本检查。

### 阶段 E：发布闸门

- 从 `0.29.0` 签名候选基线分别更新到 `1.0.0`，验证 Windows x64、Linux x64 和 Android ARM64。
- 确认签名错误、HTTP、降级和错误架构全部 fail closed。
- 确认更新检查不会携带 Pixiv token、Cookie 或账号标识。

## 8. 验收标准

- [x] 启动检查最多每日一次，手动检查不受此限制。
- [x] 无网络、GitHub 不可达或清单损坏不会影响应用启动。
- [x] 桌面安装前必须通过 Tauri updater 签名验证。
- [x] Android 安装前完成清单、哈希、包名、ABI 和 APK 证书验证。
- [x] 自动下载可关闭、取消和失败重试；设置可跨重启保存。
- [x] Android 正确引导未知来源授权并处理系统安装确认。
- [x] 不允许降级、跨通道更新、Debug 包进入 stable 通道或低安全更新传输。
- [ ] 旧版本数据库、登录状态、离线资料库和用户设置在更新后保持可用。

## 9. 当前决策状态

1. [x] 更新源使用 GitHub Releases。
2. [x] 源码仓库与匿名更新源统一为公开仓库 `space2233/pixnya`，本地 `origin` 已连接；应用不内置 GitHub 私人访问令牌。
3. [x] Windows 以 NSIS 安装包作为正式分发和更新格式。
4. [x] Linux 当前只使用 AppImage 进行应用内自动更新。
5. [x] Android 使用“自动检查，可选自动下载，系统确认安装”，不追求静默安装。
6. [x] 自动检查默认开启、自动下载默认关闭。

## 10. 当前实现进度

- [x] 产品名、窗口标题、包名与构建产物统一为 PixNya，自动更新功能版本从 `0.25.0` 开始提供。
- [x] `UpdateManager` 提供归一化状态、持久设置、单任务检查与 24 小时自动检查限流。
- [x] 设置中心提供更新状态、手动检查、自动检查、自动下载和 Android 非计费网络设置。
- [x] 桌面检查 Adapter 接入 Tauri updater，且只接受编译期写入的 GitHub HTTPS 地址和签名公钥。
- [x] Android 注册系统安装 Adapter，限制 APK 位于应用私有更新目录，并引导未知来源授权与系统确认。
- [x] 正式端点使用编译期配置，并建立只创建 Draft Release 的签名发布工作流。
- [x] Draft Release 在签名构建前强制运行前端/Rust 全量检查，并反查 APK 包名、版本、ABI、证书及桌面/Android 签名公私钥配对。
- [x] 更新仓库由编译期 `PIXNYA_UPDATE_REPOSITORY` 绑定；端点、下载重定向和两类清单拒绝跨仓库资源。
- [x] 发布流水线阻断 npm 高危和 RustSec advisory，并附带 GPL 正文、第三方许可证清单、逐依赖许可证正文归档、SPDX SBOM 与固定提交源码归档。
- [x] Tauri updater 私钥、Android Release keystore 与 Android 清单签名密钥已生成，已上传受保护的环境 Secrets，并已完成两份加密离线备份。
- [x] 完成桌面下载/安装以及 Android 清单验证、下载与系统安装链路。
- [ ] 使用生产签名从公开 `1.4.0` 升级到 `1.4.1` Draft，完成 Windows x64 与 Android ARM64 的数据保留回归；Linux x64、Windows ARM64 与 Android ARM32 本轮只承诺签名 CI 构建。

## 11. 发布配置

### 生产发布前需要配置的 GitHub Environment Secrets

所有生产签名与 OAuth 参数放在 `production-release` 环境中，而不是普通仓库级 Secret。Windows、Linux、Android 和最终 Draft job 都显式引用该环境。该环境只允许 `main` 分支；是否启用 required reviewers 由仓库维护者在 GitHub Environment 中单独管理。

- Pixiv 构建参数：`PIXIV_OAUTH_CLIENT_ID`、`PIXIV_OAUTH_CLIENT_SECRET`、`PIXIV_OAUTH_HASH_SALT`。
- 桌面更新签名：`TAURI_SIGNING_PRIVATE_KEY`、`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`、`PIXNYA_UPDATER_PUBKEY`。
- Android APK 签名：`PIXNYA_ANDROID_KEYSTORE_BASE64`、`PIXNYA_ANDROID_KEYSTORE_PASSWORD`、`PIXNYA_ANDROID_KEY_ALIAS`、`PIXNYA_ANDROID_KEY_PASSWORD`。
- Android 清单签名：`PIXNYA_ANDROID_MANIFEST_PRIVATE_KEY_BASE64`、`PIXNYA_ANDROID_MANIFEST_PRIVATE_KEY_PASSWORD`、`PIXNYA_ANDROID_UPDATE_PUBKEY`。其中公钥使用完整 minisign 公钥文件的 Base64，避免多行 Secret 注入出错。

在项目根目录运行以下交互式脚本，可在 Git 工作树之外的临时 staging 目录生成并验密三套长期签名材料，全部成功后才原子移动到恢复目录；添加 `-UploadSecrets` 后会读取 `.env.oauth.local` 并通过标准输入上传全部 `production-release` 环境 Secrets。四个密码只在内存与进程环境中短暂存在，必须由维护者保存到密码管理器；生成目录随后至少复制到两份加密离线介质。

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\provision-release-signing.ps1 -UploadSecrets
```

若密钥已经成功落盘、但 GitHub 上传中途失败，不要重新生成或删除密钥。使用相同四个密码运行恢复模式；脚本会重新签名验密 Tauri 私钥、导入验证 Android 私钥、核对证书指纹与公钥格式，再幂等覆盖全部 13 个环境 Secret 并检查名称完整性：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\provision-release-signing.ps1 -UploadSecrets -UploadExisting
```

官方工作流只接受手动触发、先创建当前源码仓库的 Draft Release，并将源码、tag、显式源码归档和匿名更新附件保持在同一信任边界。独立的 Publish 工作流会重新下载并验证附件、签名、清单、校验和与来源提交，全部通过后才把 Draft 切换为 latest stable。`PIXNYA_UPDATE_REPOSITORY` 仍供分支或复刻项目在自有工作流中编译期配置。

## 12. 官方依据

- [Tauri updater](https://v2.tauri.app/plugin/updater/)
- [Tauri updater 官方仓库](https://github.com/tauri-apps/plugins-workspace/tree/v2/plugins/updater)
- [Android 应用更新规则](https://developer.android.com/google/play/app-updates)
- [Android `canRequestPackageInstalls`](https://developer.android.com/reference/android/content/pm/PackageManager.html#canRequestPackageInstalls())
- [Android `PackageInstaller.SessionParams`](https://developer.android.com/reference/android/content/pm/PackageInstaller.SessionParams)
