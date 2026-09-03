# PixNya 本地项目文件与目录说明

> 适用工作区：`F:\ACM\pixiv-client`
>
> 盘点日期：2026-08-13；版本信息更新于 2026-08-28
>
> 当前源码版本：`1.5.0`

本文说明开发电脑上实际存在的内容，包括 Git 源码、依赖、缓存、构建结果和本机私有配置。目录大小会随构建变化；盘点时工作区约 **102.5 GiB / 22.6 万个文件**，其中 `target/` 约 **98.4 GiB**，是主要空间占用。

GitHub 端对应内容见 [GitHub 项目文件与发布内容说明](GITHUB_PROJECT_STRUCTURE.md)。

## 1. 先看结论

| 类别 | 典型内容 | GitHub 是否包含 | 是否可重新生成 | 处理建议 |
|---|---|---:|---:|---|
| 源码与配置 | `src/`、`src-tauri/` 的受控文件、`crates/`、`scripts/` | 是 | 否 | 必须保留；由 Git 和远端仓库备份 |
| 依赖锁与审计证据 | `Cargo.lock`、`package-lock.json`、Gradle lock、许可证评审 | 是 | 部分可生成，但需审阅 | 必须保留并提交 |
| 下载依赖 | `node_modules/`、Cargo/Gradle 全局缓存 | 否 | 是 | 可删除，下一次构建会重新下载 |
| 编译缓存 | `target/`、`.svelte-kit/`、Android `build/`/`.gradle/` | 否 | 是 | 可清理；会让下一次构建变慢 |
| 本地成品 | `artifacts/` | 仅 `README.md` | 是 | 便于测试；确认已发布/已备份后可删 |
| 本机私密配置 | `.env.oauth.local`、Android `local.properties` | 否 | 需人工恢复 | 不得上传；密码、签名私钥另做双备份 |
| Git 元数据 | `.git/` | GitHub 保存提交内容，不保存本地对象目录 | 可重新 clone | 不要手工删；要迁移完整工作区时保留 |

## 2. 根目录

| 路径 | 用途 | Git 状态 | 删除/备份建议 |
|---|---|---|---|
| `.git/` | 本地 Git 对象、分支、索引和远端信息 | 本机专用 | 不应手工清理；损坏时可从 GitHub 重新 clone |
| `.github/` | GitHub Actions 构建、审计、Draft 与 Stable 发布工作流 | 已跟踪 | 必须保留 |
| `.vscode/` | 推荐扩展和编辑器工作区设置 | 已跟踪 | 可保留；不影响应用运行 |
| `.build-logs/` | 本机构建和测试日志，盘点时约 7 MiB | 已忽略 | 排错结束后可删 |
| `.svelte-kit/` | SvelteKit 同步、类型和中间构建结果 | 已忽略 | 可删；`npm run check/build` 会重建 |
| `artifacts/` | 本地收集的 EXE、APK、GitHub 下载件和供应链产物，盘点时约 3.62 GiB | 仅 `artifacts/README.md` 已跟踪，其余忽略 | 成品确认发布或另行备份后可删 |
| `backups/` | 脚本修改前建立的本地备份 | 已忽略 | 确认新版本稳定后可归档或删除 |
| `build/` | Vite/Svelte 静态前端输出 | 已忽略 | 可删；`npm run build` 会重建 |
| `crates/` | Rust 工作区的深模块 | 已跟踪 | 必须保留，见第 3 节 |
| `docs/` | 计划、研究、测试、供应链、发布记录和本说明 | 已跟踪 | 必须保留 |
| `messages/` | 英文、简体中文、繁体中文源消息目录 | 已跟踪 | 必须保留；生成代码在 `src/lib/paraglide/` |
| `node_modules/` | npm 下载的前端/Tauri CLI 依赖，盘点时约 94.4 MiB | 已忽略 | 可删；`npm ci` 按 lockfile 恢复 |
| `project.inlang/` | Paraglide/Inlang 多语言配置；`.meta.json` 等为工具本机数据 | `settings.json` 已跟踪，其余忽略 | 配置保留；忽略的工具元数据可重建 |
| `public/` | 当前预留的公共静态资源目录 | 当前为空 | 可保留为空 |
| `scripts/` | 构建、检查、清理、签名初始化、发布和回归测试脚本 | 已跟踪 | 必须保留 |
| `src/` | Svelte 5/SvelteKit 前端源码、页面和共享组件 | 已跟踪；生成的 `paraglide/` 忽略 | 必须保留源文件 |
| `src-tauri/` | Tauri 应用、Rust IPC/backend、桌面配置和 Android 工程 | 源码/锁文件已跟踪，生成物忽略 | 必须保留受控文件，见第 5 节 |
| `static/` | 随前端打包的图标/静态资源 | 已跟踪 | 必须保留正在使用的资源 |
| `target/` | Cargo 的 Windows、Android ARM64、Android ARMv7 编译缓存与产物，盘点时约 **98.4 GiB / 19.5 万文件** | 已忽略 | 最大清理目标；优先使用复用优先脚本，整删后会完整重编译 |

### 根目录关键文件

| 文件 | 用途 |
|---|---|
| `README.md` | GitHub 项目主页和下载入口 |
| `PROJECT_PLAN.md` | 本地开发计划与未公开决策；已忽略，不进入 GitHub，需由开发者自行备份 |
| `PRIVACY.md` / `SECURITY.md` | 隐私与安全边界 |
| `LICENSE` | GPL-3.0-only 完整许可证 |
| `THIRD_PARTY_NOTICES.md` | 当前锁定依赖的第三方许可证清单 |
| `package.json` / `package-lock.json` | Node 脚本、前端依赖和精确版本锁 |
| `Cargo.toml` / `Cargo.lock` | Rust workspace 和精确依赖锁 |
| `deny.toml` | RustSec/cargo-deny 的精确临时审计策略 |
| `gradle-license-review.json` | Android Gradle 依赖许可证人工评审结果 |
| `.env.example` | 可公开的环境变量模板，不含真实凭据 |
| `.env.oauth.local` | 本机 OAuth 构建值；被忽略，**不得上传或公开** |
| `.gitignore` | 定义不会进入 GitHub 源码仓库的本机内容 |

## 3. `crates/`：Rust 业务模块

| 目录 | 用途 |
|---|---|
| `api/` | Pixiv API 请求、响应类型、分页和接口封装 |
| `auth/` | OAuth/PKCE、token 类型和认证逻辑 |
| `domain/` | 跨模块共享领域类型 |
| `diagnostic-log/` | 脱敏诊断日志模型和本机导出边界 |
| `download-queue/` | 可恢复下载队列及 SQLite 状态机 |
| `library/` | 离线资料库文件操作和事务/隔离删除 |
| `local-catalog/` | 本地收藏夹、标签、筛选和重复检测 |
| `local-history/` | 本地浏览历史 |
| `media-cache/` | 在线媒体缓存、LRU 和容量限制 |
| `network/` | 标准、ECH、兼容连接路线和媒体/登录网络策略 |
| `storage-policy/` | 可用空间、保留空间和存储策略 |
| `downloads/` | 当前为空的历史/预留目录，不属于 Cargo workspace；如确认不再使用可后续删除 |

## 4. `src/`：前端

- `src/lib/components/`：卡片、图片、评论、连接选择、Ugoira 播放等可复用 Svelte 组件。
- `src/lib/*.ts`：会话、导航返回、收藏同步、搜索历史、更新、i18n 等前端状态边界。
- `src/routes/`：首页、搜索、作品/小说详情与系列、通知、离线资料库、个人主页、设置及首次连接选择页面。
- `src/routes/prototype/`：当前只是本机未跟踪的空目录；Git 不保存空目录，重新 clone 后不会出现。今后若建立一次性原型，也不应直接进入正式应用功能面。
- `src/lib/paraglide/`：由 `messages/` + `project.inlang/` 生成的多语言代码，已忽略，可重建。
- `src/app.css` / `src/app.html`：全局样式和 HTML 外壳。

## 5. `src-tauri/`：原生应用与 Android 工程

| 路径 | 用途 | 处理建议 |
|---|---|---|
| `src-tauri/src/` | Tauri 命令、会话、安全存储、更新、离线下载、Ugoira 导出等 Rust 主应用 | 必须保留 |
| `src-tauri/capabilities/` | Tauri IPC/窗口能力边界 | 必须保留 |
| `src-tauri/icons/` | Windows/Linux/Tauri 打包图标 | 必须保留 |
| `src-tauri/tauri.conf.json` | 通用产品、窗口和打包配置 | 必须保留 |
| `src-tauri/tauri.release.conf.json` | Release 构建覆盖配置 | 必须保留 |
| `src-tauri/gen/android/` 的 Gradle 脚本、锁文件、verification metadata、Android 源码和资源 | 可复现 Android 工程及供应链信任边界 | 已跟踪部分必须保留 |
| `src-tauri/gen/android/app/build/`、根 `build/`、`.gradle/`、`buildSrc/build/` | Android/Gradle 编译缓存与输出 | 已忽略，可删 |
| `src-tauri/gen/android/app/src/main/jniLibs/` | Tauri 构建时生成的 Rust `.so` 链接/副本 | 已忽略，可重建；不要当源码备份 |
| `src-tauri/gen/android/app/src/main/assets/`、`generated/` | Tauri 自动生成配置和 Java/Kotlin glue | 已忽略，可重建 |
| `src-tauri/gen/android/app/tauri.properties` | Tauri 生成的 Android versionName/versionCode | 已忽略，构建时生成 |
| `src-tauri/gen/android/local.properties`（若存在） | 本机 Android SDK 路径 | 已忽略，不可上传；换电脑需重新生成 |
| `src-tauri/gen/schemas/` | Tauri 自动生成 schema | 已忽略，可重建 |

## 6. 本地成品与缓存如何处理

### `artifacts/`

- `windows/`：本机构建或收集的 Windows EXE。
- `android/`：本机构建或收集的 APK。
- `github/`：从 GitHub Actions/Release 下载用于核验的副本。
- `supply-chain/`：本机生成的 SBOM、许可证包、来源归档等。
- `SHA256SUMS.txt`：本地收集产物的校验和。

这些二进制不是源码。GitHub Release 已有同版本且校验无误时可删除；若是尚未发布的唯一测试包，应先复制到独立备份盘。

### `target/`

- `debug/`：Windows 本机 Debug 产物与共享依赖。
- `aarch64-linux-android/`：Android ARM64 Rust 编译缓存。
- `armv7-linux-androideabi/`：Android ARM32 Rust 编译缓存。
- `tmp/`：Cargo 临时构建内容。

建议先运行：

```powershell
npm run storage:audit
npm run storage:cleanup:preview
```

不要把整个项目目录作为普通“备份”反复复制；其中绝大多数小文件来自 `target/`。源码以 GitHub + Git 提交为主，签名材料使用独立的离线双备份，本地私有环境文件单独备份。

## 7. 必须独立备份、绝不能提交的内容

1. 生产 Tauri updater 私钥和密码。
2. Android Release keystore、alias、store/key 密码。
3. Android manifest 签名私钥和密码。
4. `.env.oauth.local` 中的实际 OAuth 构建值。
5. 尚未导出的用户本地数据（缓存本身通常不需要备份）。

生产签名材料当前位于项目目录外的专用目录，并已按此前流程做双备份。**不要为了“备份项目”把它们复制回仓库。**

### 应用用户数据不在源码目录中

安装后产生的用户资料由 Tauri 放在操作系统的 `app_data_dir` / `app_cache_dir`，不属于 `F:\ACM\pixiv-client`。需要迁移收藏整理、浏览历史、下载队列或离线内容时，应另外备份应用数据目录，重点包括：

- `local-catalog-v1.sqlite3`：本地收藏夹、标签和整理状态。
- `browsing-history-v1.sqlite3`：浏览历史。
- `download-queue-v1.sqlite3`：下载队列状态。
- `offline-library/`：已下载的离线作品和小说。
- `media-v1/`：在线媒体缓存，通常不必备份，可重新下载。进程内复用同一缓存实例；打开时仍会清除旧原图缓存。清除媒体缓存不会触及相邻的 `offline-library/`。

登录凭据还会进入系统安全存储，不能仅靠复制源码目录或上述 SQLite 文件完成账号迁移。

## 8. 从 GitHub 恢复开发环境

```powershell
git clone https://github.com/space2233/pixnya.git
cd pixnya
npm ci
cargo fetch --locked
```

之后再恢复本机 `.env.oauth.local`、Android SDK/NDK/JDK 路径和需要的签名材料。`target/`、`node_modules/`、`.svelte-kit/` 与 Android `build/` 不需要从备份恢复。
