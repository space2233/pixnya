# PixNya GitHub 项目文件与发布内容说明

> 仓库：<https://github.com/space2233/pixnya>
>
> 更新日期：2026-08-28
>
> 当前 Latest Stable：`v1.5.0`

本文只说明 GitHub 上的两类内容：**源码仓库**和 **Releases**。GitHub 源码页不包含本机依赖、编译缓存、私密配置或签名私钥。

开发电脑端对应内容见 [本地项目文件与目录说明](LOCAL_PROJECT_STRUCTURE.md)。

## 1. GitHub 上有哪几层内容

| 区域 | 内容 | 用途 |
|---|---|---|
| `main` 分支 | 受版本控制的源码、文档、锁文件和工作流 | 开发、审查和复现构建 |
| Git tags | 如 `v1.5.0`，直接指向发布源码提交 | 将 Release 与不可变源码版本绑定 |
| GitHub Actions | CI 日志和有保留期的中间 artifacts | 测试、跨平台签名构建、Draft 汇总、Stable 复验 |
| GitHub Releases | 面向用户的安装包、自动更新清单和验证包 | 下载、自动更新和发布审计 |
| Issues/PR（启用时） | 问题、讨论和改动审查 | 协作记录，不属于安装包 |

## 2. 源码仓库目录

`PROJECT_PLAN.md` 保持本地忽略；当前仓库共有 378 个受跟踪文件。主要分组如下。

| 路径 | 用途 |
|---|---|
| `.github/workflows/` | GitHub Actions 工作流，见第 4 节 |
| `.vscode/` | 推荐编辑器扩展和项目设置 |
| `crates/` | Rust 业务模块：API、认证、网络、下载、离线资料库、本地目录、缓存等 |
| `docs/` | 项目计划、发布模板与记录、安全/供应链说明、研究和测试文档 |
| `messages/` | 英文、简体中文和繁体中文 i18n 源消息 |
| `project.inlang/settings.json` | Paraglide/Inlang 多语言项目配置 |
| `scripts/` | 构建、测试、发布、供应链、签名初始化、空间审计和校验脚本 |
| `src/` | SvelteKit 前端页面、组件和状态逻辑 |
| `src-tauri/` | Tauri/Rust 主应用、桌面配置、Android 原生源码与可复现 Gradle 工程 |
| `static/` | 随应用打包的静态资源 |
| `artifacts/README.md` | 说明本地 `artifacts/` 的用途；实际二进制不提交 |

### 根目录受控文件

| 文件 | 用途 |
|---|---|
| `README.md` | GitHub 主页、项目性质、支持平台和下载说明 |
| `PRIVACY.md` / `SECURITY.md` | 公开隐私与安全承诺 |
| `LICENSE` | GPL-3.0-only 项目许可证 |
| `THIRD_PARTY_NOTICES.md` | 锁定依赖的第三方许可证说明 |
| `package.json` / `package-lock.json` | Node 依赖、脚本和精确锁定版本 |
| `Cargo.toml` / `Cargo.lock` | Rust workspace 与精确锁定版本 |
| `deny.toml` | cargo-deny/RustSec 审计策略 |
| `gradle-license-review.json` | Android Maven/Gradle 许可证评审证据 |
| `.env.example` | 无真实秘密的环境变量模板 |
| `.gitignore` | 阻止缓存、成品、凭据和本机配置进入仓库 |

## 3. Rust、前端与 Android 源码分工

### `crates/`

- `api`：Pixiv API。
- `auth`：OAuth/PKCE 与认证模型。
- `network`：标准、ECH、兼容连接和网络策略。
- `download-queue`、`library`：下载和离线内容。
- `local-catalog`、`local-history`：本地整理与历史。
- `local-backup`：凭据无关的本地数据备份格式、校验与流式恢复。
- `media-cache`、`storage-policy`：缓存与空间保护。
- `domain`、`diagnostic-log`：共享类型和脱敏诊断。

### `src/`

- `lib/components/`：可复用 UI。
- `lib/*.ts`：会话、导航、缓存的前端状态边界。
- `routes/`：首页、搜索、详情、小说阅读、通知、离线、设置、首次连接选择等页面。

### `src-tauri/`

- `src/`：Tauri 命令和 Rust 主应用逻辑。
- `capabilities/`：允许的 Tauri 能力。
- `icons/`：各平台打包图标。
- `gen/android/`：提交必要的 Gradle 构建文件、依赖锁、SHA-256 verification metadata、Android Kotlin/Java 源码和资源。
- Android 的 `.gradle/`、`build/`、生成 `.so`、`tauri.properties` 等不进入仓库。

## 4. GitHub Actions 工作流

| 文件 | 触发方式 | 用途 |
|---|---|---|
| `.github/workflows/linux.yml` | Push/PR 等仓库验证 | Linux 依赖、完整测试和 Tauri Linux 编译验证 |
| `.github/workflows/android-build-tool-audit.yml` | 定时/手动 | 扫描 Android build-only 锁图，按精确临时基线阻断新增或到期风险 |
| `.github/workflows/release.yml` | 手动 | 完整 preflight、Rust 审计、Windows x64/ARM64、Linux x64、Android ARM64/ARM32 签名构建，生成 10 个 Draft 附件 |
| `.github/workflows/publish-release.yml` | 手动 | 重新下载 Draft，复验 tag、来源、哈希、桌面签名、Android manifest 和 APK 证书，然后公开为 Latest Stable |

Actions 的中间 artifacts（例如 `windows-arm64` 或 `android-armeabi-v7a`）服务于同一次流水线，有 GitHub 保留期；它们不是长期面向用户的 Release 附件。

## 5. v1.5.0 Release 的 10 个附件

公开页面：<https://github.com/space2233/pixnya/releases/tag/v1.5.0>

### 用户安装包

| 文件 | 大小（约） | 用途 |
|---|---:|---|
| `PixNya_1.5.0_x64-setup.exe` | 6.53 MiB | Windows x64 NSIS 安装包 |
| `PixNya_1.5.0_arm64-setup.exe` | 5.75 MiB | Windows ARM64 原生应用安装包 |
| `PixNya_1.5.0_amd64.AppImage` | 84.02 MiB | Linux x64 AppImage |
| `pixnya-1.5.0-android-arm64-v8a.apk` | 30.95 MiB | Android ARM64 split APK，Android 10+ |
| `pixnya-1.5.0-android-armeabi-v7a.apk` | 23.76 MiB | Android ARM32 split APK，Android 10+ |

普通用户只需按设备下载上述一个文件。

### 自动更新与验证文件

| 文件 | 用途 | 普通用户是否需要手动下载 |
|---|---|---:|
| `latest.json` | Tauri 桌面更新清单；按 `windows-x86_64`、`windows-aarch64`、`linux-x86_64` 选择安装包并携带签名 | 否 |
| `android-latest.json` | Android 更新清单；按 `arm64-v8a` / `armeabi-v7a` 选择 APK，并记录大小、SHA-256、包名和证书摘要 | 否 |
| `android-latest.json.minisig` | Android 更新清单的 Ed25519/minisign 签名 | 否 |
| `pixnya-1.5.0-verification.tar.gz` | 发布审计包：来源证明、SBOM、许可证归档、构建工具扫描结果和桌面签名 | 仅审计时需要 |
| `SHA256SUMS.txt` | 覆盖其余 9 个公开附件的 SHA-256 | 验证下载时可用 |

Release 页面自动显示的 `Source code (zip)` / `Source code (tar.gz)` 由 GitHub 根据 tag 生成，不计入上述 10 个上传附件。

## 6. `verification.tar.gz` 内部内容

v1.5.0 的验证包固定包含 12 项：

1. `BUILD-PROVENANCE.txt`：源码仓库、源码提交、发布工作流提交和 run 来源。
2. `LICENSE.txt`：项目 GPL-3.0-only 许可证。
3. Android build-tool OSV 原始报告。
4. Android Gradle 依赖清单。
5. Android ARM 共同 runtime SPDX SBOM。
6. 精确源码归档。
7. 第三方许可证正文归档。
8. 主 SPDX SBOM。
9. Linux AppImage 的 Tauri `.sig`。
10. Windows x64 安装包的 Tauri `.sig`。
11. Windows ARM64 安装包的 Tauri `.sig`。
12. `THIRD_PARTY_NOTICES.md`。

这些资料打成一个验证包，是为了保持 Release 页面简洁；Stable 发布工作流仍会在公开前逐项验证。

## 7. GitHub 明确不包含什么

以下内容不会进入源码仓库：

- `target/`、`node_modules/`、`.svelte-kit/`、`build/`。
- Android `.gradle/`、`app/build/`、生成的 JNI `.so` 和本机 `local.properties`。
- 本地 `artifacts/` 中的二进制副本（仅说明文件进入 Git）。
- `.env.oauth.local` 和任何 `.env.*` 私密配置。
- 本地开发计划 `PROJECT_PLAN.md`；该文件保留在开发电脑并由 `.gitignore` 排除。
- Tauri updater 私钥、Android keystore、Android manifest 私钥及其密码。
- 用户登录 token、Cookie、缓存、离线资料库、历史或诊断导出。
- `F:\ACM\.release-secrets\pixnya` 及其离线备份。

GitHub Actions Secrets 只以加密 Secret 的形式供受保护的 `production-release` environment 使用，不会出现在源码或 Release 附件中。

## 8. 源码仓库与 Release 的关系

```text
main 上的固定提交
  -> 完整测试和供应链检查
  -> 五个平台签名构建
  -> Draft Release（10 个附件）
  -> 独立重新下载和验签
  -> tag v1.5.0 + Latest Stable
```

`v1.5.0` tag 当前直接指向发布源码提交 `b5de41654e84389542ee8f1c3f7259c224f2d935`。安装包不提交到 `main`，而是放在 Release；源码、构建规则和验证脚本则保存在 `main` 和 tag 中。本轮未再跑 provenance 恢复 finalizer；Draft 工作流提交与 tag 目标均为该源码 SHA。
