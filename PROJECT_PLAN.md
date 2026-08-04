# PixNya 跨平台客户端项目计划书

> 状态：当前路线仅保留自动检查更新与自动更新
> 日期：2026-08-04
> 目标平台：Windows、Linux、Android
> 分发方式：个人使用、开源、侧载
> 说明：本项目为非官方客户端，与 pixiv Inc. 无隶属或授权关系。

## 1. 执行摘要

本项目计划开发一款以 Rust 为核心、同时支持 Windows、Linux 和 Android 的 Pixiv 客户端。应用使用官方网页完成交互式登录，通过 PKCE 获取并维护会话；作品、用户、搜索、收藏和媒体能力由独立的 Pixiv 数据源实现提供。

### 1.1 项目性质与对接边界

PixNya 是由个人维护的开源、非官方、侧载客户端，不是 pixiv Inc. 的产品，也未获得 pixiv Inc. 的授权、认可或技术支持。项目名称、界面和仓库说明不得让用户误以为它是官方客户端；“使用 Pixiv 官方页面登录”只描述登录页面来源，不代表本应用或其数据接口具有官方身份。

当前实现不是把 Pixiv 网站嵌入应用后解析 HTML，也不是以网页抓取作为主要数据源，而是由三条边界清晰的路径组成：

| 能力 | 实际对接方式 | 性质与限制 |
|---|---|---|
| 账号登录 | 隔离 WebView 打开 Pixiv 官方登录页，使用 OAuth + PKCE 接收回调 | 用户在官方页面输入密码和二步验证码；PixNya 不提供密码输入框，但仍负责保护回调、Cookie 与令牌 |
| 作品、小说、用户、搜索、收藏、关注和评论 | Rust 网络层访问 Pixiv 现有客户端使用的 App API | 这是未向普通第三方承诺稳定性的非公开接口，不等同于 Pixiv 官方公开 API；端点、字段、参数或鉴权随时可能变化 |
| 图片与动图媒体 | Rust 媒体管线访问 Pixiv 图片 CDN | 页面不直接接触登录令牌；连接、缓存、下载和低安全降级由本地客户端控制 |

项目的责任边界如下：

- 用户使用自己的 Pixiv 账号，并自行遵守所在地法律、Pixiv 使用条款以及作品版权和年龄限制；PixNya 不提供内容、账号或付费权益。
- 项目不运营公共代理、VPN、账号中转或内容镜像服务；网络请求默认从用户设备直接发往 Pixiv 相关服务。
- 三种连接模式只改变到白名单 Pixiv 主机的网络传输方式，不绕过登录、付费、年龄、地区或账号权限控制。低安全路径可能遭受中间人攻击，必须明确披露并允许用户恢复警告。
- Pixiv、pixiv 标志及站内作品归各自权利人所有；GPL-3.0-only 只覆盖 PixNya 自身源代码，不授予第三方内容或商标权利。
- PixNya 是独立实现。PixEz 等公开项目只用于研究可观察行为、兼容思路和架构取舍，不是运行依赖；当前仓库未复制或改写 PixEz 源码。如未来引入第三方代码，必须记录来源、许可证和修改内容。
- 因非公开接口或上游登录流程变化造成的失效属于兼容性风险；仓库不得承诺持续可用，也不得把逆向兼容能力描述为 Pixiv 官方支持。

项目最重要的底层能力是统一网络模块。它需要覆盖 Rust 发起的 OAuth、数据和图片请求，以及平台 WebView 发起的登录请求，并提供三种用户可选连接配置：

1. **标准模式**：使用系统 DNS、系统代理和标准 TLS。
2. **ECH 直连模式**：对可控的 Rust 连接强制使用 TLS 1.3 + ECH，并验证 ECH 是否被接受。
3. **低安全直连模式**：通过内置 Pixiv IP 白名单绕过异常解析，关闭上游 SNI 和证书验证；默认禁用，首次必须确认，之后可由用户选择停止重复提醒。Android 网页登录与 token 交换都可使用该传输，但必须分别披露密码、验证码和令牌泄露风险。

核心浏览版本的成功标准不是功能数量最大化，而是证明以下闭环可靠成立：

- 标准模式安全完成官方网页登录；非标准 Android 登录默认逐次披露中间人风险，用户可在明确确认后选择停止重复提醒；
- 三种连接配置的能力和限制可被检测、展示和诊断；
- 稳定浏览作品、作者、推荐、排行榜和搜索结果；
- 正确加载、缓存和下载普通图片，基础播放 Ugoira；
- 不泄露 token、Cookie、密码、访问历史或未脱敏日志。

### 1.2 当前实现进度（2026-08-04）

- [x] Windows 与 Android 可以从应用内打开隔离的 Pixiv 官方登录页。
- [x] 标准模式使用系统 WebView 网络；ECH 模式先做严格 Rust `Accepted` 预检；Android ECH/兼容登录再使用一次性低安全 TLS 桥连接固定 Pixiv IP。
- [x] Android 等待 `ProxyController` 应用完成后才加载页面；只对当前会话 Pixiv 白名单和匹配的一次性证书指纹放行，其余证书错误取消。
- [x] Windows x64 Debug 与 Android ARM64、ARMv7 Debug 构建通过并归档到 `artifacts/`。
- Linux WebKitGTK 活体测试已暂停并移入[备选功能计划](docs/OPTIONAL_FEATURES_PLAN.md)。
- [x] 接入私有登录表面的 callback、authorization code 交换与平台安全令牌存储。
- [x] 设置中心支持连接诊断、媒体缓存管理、固定字段脱敏日志的本机导出/清除，以及强确认清除全部本机数据，并覆盖安全存储、Cookie、离线资料库和前端偏好边界测试。
- [x] SQLite 持久下载队列支持串行执行、崩溃恢复、登录后自动继续、暂停、继续、失败重试、进度展示和清理边界；作品、小说与 Ugoira 的离线保存入口已接入队列。
- [x] 跨平台存储策略提供剩余空间检查、512 MiB 写入保留区、低空间/临界预警，以及 128 MiB–1 GiB 可持久调整并立即 LRU 收缩的缓存上限。
- [x] 下载导出支持 Windows/Linux 系统文件夹选择与 Android SAF 持久目录授权；应用私有副本先行保存，下载可自动导出，离线资料库也可逐项手动导出，并拒绝覆盖无项目标记的同名用户目录。
- [x] 离线资料库使用独立 SQLite 本地目录管理收藏夹与标签，支持关键字、类型、收藏夹、标签和多种排序组合筛选；删除内容和清除本机数据会同步清理目录记录。
- [x] 在线与离线作品详情共用原图查看器，支持多图翻页、1–6 倍缩放、键鼠拖动、键盘操作、触控双击和双指缩放。
- [x] SQLite 本机浏览历史覆盖作品、小说与作者详情，最多保留 500 条；历史页与设置页联动，支持搜索、类型筛选、暂停记录、单条移除和全部清除。
- [x] 自动恢复会话结束前全局抑制未登录提示，成功恢复不闪现登录入口，失败或无本地会话后再显示。
- [x] 设置中心提供默认关闭的 R18 默认显示开关，并统一应用于作品、小说与作者预览中的受限内容遮罩。
- [x] 关注中心提供最新作品与关注作者双视图；关注作者按公开/非公开范围分页，账号归属由 Rust 登录会话确定。
- [x] 作品系列与小说系列提供独立目录、安全分页、详情入口，以及作品跨页相邻导航和小说官方相邻篇连续阅读。
- [x] 小说元数据详情与正文阅读拆分为独立页面；移动操作区单列回流，首页标签使用最近一次成功数据的本机缓存。
- [x] 低安全连接警告支持用户明确选择“以后不再提醒”，连接设置与登录页共享偏好，并可在设置中恢复；低安全模式本身仍不保存为默认连接方式。
- 三条登录路线的真实账号活体测试已暂停并移入[备选功能计划](docs/OPTIONAL_FEATURES_PLAN.md)。
- 当前唯一活跃功能为[自动检查更新与自动更新](docs/AUTO_UPDATE_PLAN.md)。

本计划采用最新技术决策：**Rust + Tauri 2 + TypeScript 前端**。早期调研文档中提出的 Flutter 路线不再作为默认实现路线。

## 2. 项目目标

### 2.1 产品目标

- 提供适合桌面和移动端的统一 Pixiv 浏览体验。
- 使用 Pixiv 官方网页进行登录，兼容常见二步验证流程。
- 支持标准、ECH 直连和兼容直连三种连接配置。
- 支持首页推荐、排行榜、搜索、作品详情、作者详情、收藏和关注。
- 支持单图、多图、动图和 Ugoira 的浏览。
- 提供可暂停、可恢复、有限流的下载队列。
- 默认本地优先，不要求项目方运营账号、代理或中转服务器。
- 保持模块接口稳定，把上游未公开接口的变化限制在 Pixiv 数据源实现内部。

### 2.2 工程目标

- 共享 Rust 领域逻辑、会话、网络、存储和媒体代码。
- 平台差异仅通过明确的 Adapter 放在 seam 上。
- 网络策略可通过纯函数和 fake Adapter 完整测试。
- 所有发布产物可复现，依赖、许可证和构建来源可审计。
- 上游接口变更时，尽量只修改一个深模块。

### 2.3 第一版不做

- iOS 和 macOS 支持。
- 应用商店上架。
- 内置公共代理、VPN 或项目方运营的流量中转。
- 绕过付费、年龄、地区或账号权限控制。
- 自动批量抓取整个关注列表或作者全部作品。
- 私信、投稿、直播等高风险或低优先级功能。
- 云同步、社交功能和自建账号系统。
- 自动或静默地在登录中关闭 TLS 证书验证，以及在 OAuth token 交换中使用低安全 TLS。

## 3. 关键约束

### 3.1 上游接口约束

Pixiv 面向现有客户端使用的 App API、Web AJAX 和 OAuth 行为并非面向普通第三方开发者承诺的稳定公开接口。项目必须接受以下事实：

- 上游端点、字段、请求头和登录流程可能随时变化；
- 不能把远端 JSON、URL 或分页参数直接暴露给 UI；
- 不能把兼容参数或真实 token 提交到仓库；本地构建参数只放入被忽略的环境文件；
- 兼容参数会进入最终二进制、无法真正保密；公开分发前必须完成上游授权与法律审查。

### 3.2 开源许可证约束

- 本项目自身源代码当前采用 `GPL-3.0-only`；该许可证不覆盖 Pixiv 内容、商标、账号数据或其他第三方资产。
- 参考其他 GPL 项目时只研究公开文档、外部行为和架构，不复制实现代码；PixEz 当前不是依赖，也没有源代码进入本仓库。
- 未来若复用第三方代码，必须在合入前确认 GPL 兼容性，保留版权声明并记录具体来源与修改内容。
- 使用图片、插画和 Pixiv 商标时遵守其版权和品牌规则。

### 3.3 网络安全约束

- 不自动修改系统 hosts。
- 不安装自定义根证书；Android 低安全登录桥使用每会话随机证书和显式 SHA-256 指纹，不写入系统/应用信任库。
- 不把“连接到指定 IP”误报为“已使用 ECH”。
- ECH 严格路径不得静默降级到明文 SNI。
- 每次跨域重定向都必须重新执行主机分类和连接策略。
- 未知主机不得继承 Pixiv 的固定 IP 或 ECH 配置。
- 低安全直连不得自动启用、保存为默认模式或作为 ECH 的静默回退；默认每次新页面会话确认风险，只有用户在完整披露后主动选择“以后不再提醒”才可跳过重复 UI，Rust 调用仍必须携带明确确认状态。
- 低安全直连只用于明确白名单内的 Pixiv OAuth/API/媒体主机和已经确认风险的 Android 登录桥；UI 必须明确提示 token 同样可能泄露，并允许恢复重复提醒。

## 4. 平台支持基线

| 平台 | 第一版支持范围 | 发布架构 | 备注 |
|---|---|---|---|
| Windows | Windows 10/11 | `x86_64` | 依赖 WebView2 Runtime；ARM64 后续评估 |
| Linux | WebKitGTK 4.1 可用的主流发行版 | `x86_64` | 重点验证 Ubuntu LTS、Fedora；Wayland/X11 均测试 |
| Android | Android 10–16，API 29–36 | `arm64-v8a`、`armeabi-v7a` | ARM64 与 ARMv7 分包发布；`x86_64` 仅用于模拟器/CI |

Android 构建基线：

- `minSdkVersion = 29`
- `targetSdkVersion = 36`
- `compileSdkVersion = 36`
- 发布 ABI：`arm64-v8a`、`armeabi-v7a`
- 测试 ABI：`arm64-v8a`、`armeabi-v7a`、`x86_64`
- Rust Android targets：`aarch64-linux-android`、`armv7-linux-androideabi`、`x86_64-linux-android`
- 默认分别生成 ARM64 和 ARMv7 APK，避免 universal APK 增加下载体积；是否额外提供 universal APK 在候选发布阶段决定
- ARMv7 构建必须通过 CI，并至少在一台真实 32 位设备上完成登录、浏览、图片和 Ugoira 冒烟测试后才能进入正式发布

提高 Android 最低版本的主要原因是 WebView/Chromium 安全更新和登录兼容性，而不是单纯为了放弃 32 位设备。

### 4.1 当前开发环境与首次安装

当前 Windows 开发机的 Rust、MSVC、Node.js、Tauri、Android Studio、SDK/NDK 与 JDK 已安装；大型工具链和缓存统一位于 `F:\ACM\.toolchains`。

推荐安装顺序：

1. 检查并安装 Microsoft C++ Build Tools，启用“使用 C++ 的桌面开发”。
2. 检查 WebView2 Runtime；Windows 10/11 通常已经安装。
3. 通过 `rustup` 安装稳定版 Rust，并使用 MSVC host toolchain。
4. 安装 Node.js LTS，启用 Corepack，项目确定后固定 npm 或 pnpm 版本。
5. 安装稳定版 Android Studio，并通过 SDK Manager 安装 Android SDK Platform 36、Platform-Tools、Build-Tools、Command-line Tools 和 NDK (Side by side)。
6. 设置 `JAVA_HOME`、`ANDROID_HOME` 和 `NDK_HOME`，重开终端后验证。
7. 添加 Rust Android targets：

   ```powershell
   rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
   ```

8. 用 Tauri 最小工程分别完成 Windows 调试构建、Android ARM64 构建和 Android ARMv7 构建。

Android Studio 是本地调试、SDK 管理和模拟器的推荐方案，但不是命令行构建的硬性条件。如果开发机资源有限，可以只安装 Android Command-line Tools、SDK、NDK、JDK 并连接真实 Android 设备。项目在首次成功构建后固定 Rust toolchain、Android Gradle Plugin、SDK、Build-Tools 和 NDK 版本，避免后续使用“本机最新版本”导致不可复现。

## 5. 技术栈

### 5.1 主体技术

| 层次 | 选择 | 用途 |
|---|---|---|
| 应用壳 | Tauri 2 | Windows、Linux、Android 统一应用壳和命令桥接 |
| 核心语言 | Rust | 网络、会话、数据源、存储、缓存和下载 |
| 前端 | TypeScript + Svelte + Vite | 跨平台 UI；最终框架在 UI 原型后锁定 |
| 异步运行时 | Tokio | 网络、下载、缓存和后台任务 |
| TLS | rustls | 标准 TLS、TLS 1.3、ECH 和证书验证 |
| HTTP | reqwest，必要时下沉到 hyper/tokio-rustls | 普通请求和需要精确 TLS 控制的请求 |
| 序列化 | serde | 远端 DTO、配置和本地数据 |
| 本地数据库 | SQLite | 元数据、收藏镜像、历史、下载任务和迁移 |
| 媒体文件 | 应用私有文件目录 | 图片、Ugoira ZIP、缩略图和导出文件 |
| 平台安全存储 | Windows Credential/DPAPI、Linux Secret Service、Android Keystore Adapter | refresh token 和敏感配置 |

### 5.2 暂不锁定的选项

以下选择必须通过阶段 0 原型后再写入架构决策记录：

- Svelte 与 React 的最终选择；
- SQLite 使用 `rusqlite` 还是 `sqlx`；
- reqwest 是否能满足 ECH 状态观测，否则建立 hyper/tokio-rustls 传输实现；
- Linux 安装包优先使用 AppImage、Flatpak、deb 还是组合发布；
- 严格 ECH 登录是否值得引入自带浏览器内核。

## 6. 总体架构

```text
┌──────────────────────────────────────────────────────────┐
│                     TypeScript UI                        │
│ 登录 / 首页 / 搜索 / 作品 / 作者 / 收藏 / 下载 / 设置   │
└──────────────────────────┬───────────────────────────────┘
                           │ Tauri commands/events
┌──────────────────────────▼───────────────────────────────┐
│                       Rust 应用核心                      │
│ PixivSource │ SessionManager │ MediaPipeline │ Repository│
└──────────────────────────┬───────────────────────────────┘
                           │
┌──────────────────────────▼───────────────────────────────┐
│                    NetworkGateway                       │
│ ConnectionPolicy / DNS / ECH / TLS / 连接池 / 限流      │
└───────────────┬──────────────────────────┬───────────────┘
                │                          │
┌───────────────▼──────────────┐  ┌────────▼──────────────┐
│ Rust HTTP Transport         │  │ InteractiveLogin      │
│ OAuth / API / Media         │  │ 平台 WebView Adapter  │
└──────────────────────────────┘  └───────────────────────┘
```

### 6.1 模块与 seam

#### `NetworkGateway`

统一执行连接配置、解析、ECH、TLS、代理、重试、限流和诊断。调用者只提交逻辑请求，不接触 IP、SNI、DoH 或证书配置。

```rust
pub enum ConnectionMode {
    Standard,
    Ech,
    Compatible,
}

pub trait NetworkGateway {
    async fn apply_mode(&self, mode: ConnectionMode)
        -> Result<ConnectionReport, NetworkError>;
    async fn probe(&self)
        -> ConnectionReport;
    async fn execute(&self, request: NetworkRequest)
        -> Result<NetworkResponse, NetworkError>;
}
```

外部 interface 的不变量：

- 请求必须先被归类为 OAuth、Pixiv 数据、媒体或登录 WebView；
- 每次重定向重新归类；
- ECH 模式下要求 ECH 的请求只有在握手确认后才成功；
- Android 低安全登录桥必须由策略明确标记并取得确认状态；默认逐次提示，用户关闭重复提醒后仍由 UI 为每次 Rust 调用传入显式确认。兼容模式的 token 请求也必须被策略标为低安全，禁止隐藏风险；
- 日志输出只包含脱敏后的主机、阶段、错误类别和耗时。

#### `ConnectionPolicy`

纯 Rust 决策模块：输入连接配置、流量类别、域名和平台能力，返回内部 `RoutePlan`。它不执行网络请求，因此可以用表驱动测试覆盖全部组合。

#### `InteractiveLogin`

统一管理 PKCE、内部随机会话标识、登录生命周期、重定向拦截和取消；Windows、Linux、Android 分别提供 WebView Adapter。当前 Pixiv 私有 callback 不回传 `state`，因此只允许从不可导出的独立窗口/Activity 按精确 launch ID 返回；若 callback 携带 `state`，仍必须校验。

#### `SessionManager`

独占 access token 和 refresh token：

- access token 仅保存在内存；
- refresh token 存入平台安全存储；
- 并发刷新使用 single-flight；
- 刷新失败转换为 `ReauthenticationRequired`；
- 其他模块不能取得 token 明文，只能请求经过认证的操作。

#### `PixivSource`

把上游端点和 JSON 转换为稳定的应用领域模型。UI 不能调用具体远端端点。

```rust
pub trait PixivSource {
    async fn artwork(&self, id: ArtworkId) -> Result<Artwork, SourceError>;
    async fn list(&self, query: CollectionQuery) -> Result<Page<ArtworkSummary>, SourceError>;
    async fn search(&self, query: SearchQuery) -> Result<Page<SearchItem>, SourceError>;
    async fn mutate(&self, action: AccountAction) -> Result<MutationReceipt, SourceError>;
}
```

#### `MediaPipeline`

管理缩略图、预览图、原图、Referer、缓存、Ugoira ZIP、帧调度和导出。UI 只请求本地可消费的媒体句柄，不直接加载远端图片 URL。

#### `DownloadManager`

提供持久化队列、并发限制、暂停、恢复、失败重试、文件名清理和剩余空间检查。下载任务必须始终对用户可见。

#### `Repository`

隐藏 SQLite、迁移、文件索引和缓存清理策略。领域模块不直接拼 SQL 或文件路径。

## 7. 三种连接配置

### 7.1 行为矩阵

| 配置 | OAuth / 数据请求 | 图片请求 | 登录 WebView | 失败行为 |
|---|---|---|---|---|
| 标准 | 系统 DNS/代理 + 标准 TLS | 同左 | WebView 默认网络 | 返回分类错误 |
| ECH | DoH `HTTPS` 记录 + TLS 1.3 + ECH，必须确认接受 | 支持 ECH 时使用；否则显示失败 | 桌面由 WebView 内核协商；Android 明示为“ECH 仅预检 + 低安全登录桥” | OAuth/数据不静默降级；Android 登录另行确认 |
| 低安全直连 | 内置 Pixiv IP；关闭 SNI/证书验证；默认逐次提示，可停止重复提醒 | 同左 | Android 使用一次性指纹锁定的低安全 TLS 桥；桌面保留端到端 WebView TLS | 不自动启用，不作为 ECH 回退 |

### 7.2 ECH 实现要求

- 查询目标域名 DNS `HTTPS` 记录并提取 `ECHConfigList`。
- 按 DNS TTL 缓存 ECH 配置，支持服务器返回的 retry config。
- 内置最近已知配置只能作为有明确过期时间的启动回退，不能永久硬编码。
- `rustls::ClientConfig` 按域名和 ECH 配置指纹隔离，连接池不能跨不兼容配置共享。
- 使用 TLS 1.3，完成握手后确认 `EchStatus::Accepted`。
- 区分 `NotOffered`、`Rejected`、配置过期、DNS 失败和证书失败。
- ECH 配置和候选 IP不属于秘密，但更新来源必须可验证，且不能让远端配置关闭证书验证。

### 7.3 低安全直连要求

- 内置主机表只允许 Pixiv 相关的明确域名，不支持任意域名映射。
- 默认关闭；用户首次进入该模式时必须看到中间人攻击、数据篡改和 access token 泄露风险。默认逐次确认，也可在完整披露后选择停止重复提醒，并能随时从设置恢复。
- Rust 客户端连接固定候选 IP，关闭 SNI 与服务器证书验证；UI 必须如实显示这一点，不得称为“安全直连”。
- Android 登录 WebView 与 OAuth token 交换只有在取得明确确认状态后才可使用低安全传输；首次必须分别显示密码、验证码和 token 泄露风险，用户可停止重复提醒并随时恢复。
- ECH 或标准模式失败时不得自动回退到该模式。
- 当前固定 IP 只作为可更新的本地版本配置，不接受远端响应扩大域名白名单。

### 7.4 登录 WebView 的能力边界

普通 CONNECT 代理可以替 WebView 选择 DNS 结果和目标 IP，却无法隐藏 WebView ClientHello 的 SNI。Android 非标准登录因此采用显式低安全 TLS 终止桥，第一版规则为：

- 标准登录保持系统 TLS；Windows/Linux 兼容登录保留端到端 WebView TLS；
- Android ECH/兼容登录取得明确确认状态后使用一次性证书、固定 IP、无上游 SNI/证书验证的本地桥；
- UI 分别报告“Rust 请求 ECH 已确认”和“Android 登录页实际为低安全桥”；
- 不把“已通过直连代理连接”显示成“ECH 已确认”；
- callback 后的 token 交换由 Rust 单独建立连接：标准模式使用系统 TLS，ECH 模式要求 `Accepted`，兼容模式按已确认风险使用固定 IP/无 SNI/无证书验证；
- 若未来要求登录页本身使用可验证 ECH，则单独评估 Android GeckoView 或自带浏览器内核。

Android 使用 `androidx.webkit.ProxyController` 设置进程级 WebView 代理：必须先检查 `PROXY_OVERRIDE`，等待设置完成回调后才能加载页面，并在登录结束后清除覆盖。桥只监听随机回环端口、只允许 `CONNECT :443`，且只有固定 Pixiv 主机终止 TLS；第三方站点保持端到端隧道。由于代理覆盖影响应用进程内全部 WebView，登录期间不得并行创建其他远端 WebView。

### 7.5 网络诊断页

设置页提供不泄密的诊断结果：

- 当前连接配置和平台能力；
- 系统 DNS/DoH 解析是否成功；
- 每个关键主机的候选地址数量；
- TLS 版本、证书验证结果和 ECH 状态；
- WebView 代理是否成功应用；
- API、图片和登录入口的分项连通性；
- 可复制的脱敏诊断报告。

诊断页不得显示 token、Cookie、完整 OAuth URL、查询关键词、作品标题或用户浏览历史。

## 8. 登录与会话流程

### 8.1 交互式登录

1. Rust 生成 PKCE `code_verifier`、S256 `code_challenge` 和内部随机登录会话标识。
2. `InteractiveLogin` 根据当前连接配置准备 WebView 网络 Adapter。
3. 打开独立登录窗口或页面，加载官方 Pixiv 登录 URL。
4. 用户在官方页面输入账号、密码和二步验证码；标准模式不触碰页面正文，低安全桥只转发且不解析/记录正文，但明文会经过应用内存。
5. 导航观察器只拦截预期 callback，验证 scheme、host、path 与私有窗口/Activity launch ID；若上游返回 `state`，同时验证 `state`。
6. Rust 使用 authorization code 和 `code_verifier` 交换 token。
7. `SessionManager` 将 refresh token 写入安全存储，access token 留在内存。
8. 校验 token 响应并保存其中的非敏感账号资料；后续数据源再执行只读 API 验证。
9. 销毁登录 WebView；按平台能力清理临时 Cookie、缓存和代理覆盖。

### 8.2 登录安全要求

- WebView JS bridge 不暴露 token、文件系统或任意 Tauri 命令。
- 只允许经过审核的命令白名单。
- OAuth callback 必须验证 PKCE 与私有登录表面绑定；不能把当前不回传 `state` 的 callback 注册为可被其他应用唤起的公开 deep link。
- 对 Pixiv 相关域名应用选择的连接配置；第三方身份提供商默认使用系统连接。
- 外部帮助、条款和非登录页面转到系统浏览器。
- 记录导航错误类别，但不记录完整 URL 查询和 fragment。
- 支持取消、超时、用户拒绝、二步验证和页面崩溃恢复。

### 8.3 故障回退

- 开发构建保留“导入 refresh token”入口，用于验证核心能力。
- 是否在正式构建保留该入口，在阶段 0 安全评审后决定。
- 不提供账号密码登录，不要求用户把密码交给应用。

## 9. 第一版功能范围

### 9.1 必须具备

- 首次启动与连接配置选择。
- 独立登录页、退出登录和重新认证。
- 推荐、排行榜和关注动态中的至少两类信息流。
- 作品详情：单图、多图、标签、作者和基础统计。
- 作者详情和作品列表。
- 搜索作品、作者和标签。
- 收藏/取消收藏，关注/取消关注。
- 图片查看、缩放、翻页、缓存和原图下载。
- Ugoira 基础播放。
- 持久化下载队列。
- 连接诊断、缓存管理、日志导出和数据清除。

### 9.2 可以延期

- 当前只重新激活自动检查更新与自动更新，实施方案见[自动更新计划](docs/AUTO_UPDATE_PLAN.md)。
- 其余未完成或未进入当前范围的功能、平台扩展与活体验证统一放入[备选功能计划](docs/OPTIONAL_FEATURES_PLAN.md)，不计入当前待办，也没有排期。

## 10. 本地数据设计

### 10.1 SQLite 数据

- schema 版本和迁移记录；
- 账号非敏感资料；
- 作品、作者、标签和分页游标缓存；
- 浏览历史和收藏镜像；
- 下载任务、状态、重试次数和目标文件；
- 媒体缓存索引、大小、最后访问时间；
- ECH 配置、DNS 结果和过期时间；
- 用户设置和平台能力快照。

### 10.2 不进入普通数据库的数据

- refresh token、代理密码和其他长期秘密；
- WebView 会话 Cookie；
- 完整 Authorization 请求头；
- 未脱敏 OAuth callback URL；
- 用户在登录页输入的任何内容。

### 10.3 缓存策略

- 缩略图、预览图、原图使用不同缓存键。
- 缓存键基于稳定 ID、页码、尺寸和来源版本，不依赖作品标题。
- 默认设置总容量上限，并使用 LRU 清理非固定内容。
- 下载文件与临时缓存分开；清理缓存不得删除用户下载。
- Ugoira ZIP 按流式方式写入磁盘，播放采用有限帧缓冲区。

## 11. 性能与可靠性预算

初始值需要通过真实小流量测试校准：

| 项目 | 初始预算 |
|---|---|
| API GET 并发 | 4 |
| 图片 CDN 并发 | 6 |
| 写操作 | 串行或最多 2 |
| 自动重试 | 仅幂等请求，最多 3 次 |
| 重试策略 | 优先 `Retry-After`，否则指数退避 + jitter |
| 前台首屏缓存命中目标 | 300 ms 内可展示 |
| 网络诊断超时 | 单项 5–10 秒，总计可取消 |
| 后台下载 | 必须可暂停，不抢占全部前台连接 |

内存预算、磁盘缓存默认值和 Ugoira 帧缓冲大小在目标设备实测后确定。

## 12. 仓库结构建议

```text
pixiv-client/
├─ app/                         # TypeScript UI
├─ src-tauri/                   # Tauri 壳、命令与权限配置
├─ crates/
│  ├─ domain/                   # 稳定领域类型
│  ├─ network/                  # NetworkGateway 与连接策略
│  ├─ session/                  # SessionManager
│  ├─ pixiv-source/             # 上游 DTO 与数据源实现
│  ├─ media/                    # 缓存、图片与 Ugoira
│  ├─ storage/                  # SQLite、迁移和文件索引
│  └─ downloads/                # 下载队列
├─ plugins/
│  └─ webview-network/          # Windows/Linux/Android WebView Adapter
├─ tests/
│  ├─ fixtures/                 # 清理敏感信息后的固定响应
│  ├─ integration/              # 本地可控网络测试
│  └─ e2e/                      # 桌面与 Android 端到端测试
├─ docs/
│  ├─ adr/                      # 架构决策记录
│  ├─ research/                 # 外部项目与协议调研
│  ├─ security/                 # 威胁模型与披露流程
│  └─ release/                  # 发布和签名说明
└─ PROJECT_PLAN.md
```

## 13. 实施阶段与交付物

本节保留为已经完成的核心客户端建设过程参考，不再充当当前待办。尚未执行的旧阶段项目已迁入[备选功能计划](docs/OPTIONAL_FEATURES_PLAN.md)；当前开发只执行[自动更新计划](docs/AUTO_UPDATE_PLAN.md)。

原工期以单人全职的工程周估算；个人业余开发通常需要 4–7 个月。登录和上游接口若发生变化，工期需重新评估。

### 阶段 0：可行性闸门（2–3 周）

任务：

- 按第 4.1 节安装并验证 Rust、C++ Build Tools、Node.js 和 Android 工具链。
- 建立 Tauri 2 的 Windows、Linux、Android 最小工程。
- 在三个平台显示同一个页面并调用一个 Rust 命令。
- 分别构建 Android `arm64-v8a` 和 `armeabi-v7a` 原生库/APK，尽早识别不支持 ARMv7 的依赖。
- 验证官方登录页、PKCE callback 和 token 交换。
- 确认不需要在仓库或发行包中复制受保护的官方凭据。
- 验证 Android WebView 代理覆盖，验证 Windows/Linux WebView 代理可行性。
- 用 Rust 完成标准、ECH 和兼容连接的最小探测。
- 记录 WebView ECH 的可观测程度。
- 选定前端、SQLite 库和开源许可证。

交付物：

- 三个平台最小可运行包；
- 登录和连接技术报告；
- ADR-001 技术栈、ADR-002 登录、ADR-003 网络模式；
- 明确的 Go/No-go 决策。

验收：

- `rustc`、`cargo`、Node.js、JDK、Android SDK、NDK 和 `adb` 版本检查通过并记录到开发文档；
- Windows 调试包与 Android ARM64、ARMv7 调试包均能成功构建；
- 至少在一个真实测试账号上完成低频只读登录闭环；
- 三个平台标准登录页可加载，兼容直连路径至少完成代理设置和 TLS 验证；
- Rust ECH 测试连接能报告 `Accepted` 或提供明确失败原因；
- 若合法、安全的 token 交换方案不可成立，项目停止进入产品开发。

### 阶段 1：核心基础（2–3 周）

任务：

- 实现 `NetworkGateway`、`ConnectionPolicy` 和诊断模型。
- 实现 `SessionManager` 与三端安全存储 Adapter。
- 建立 SQLite 迁移、Repository 和日志脱敏。
- 实现 `PixivSource.artwork` 与一类列表。
- 建立 fake Adapter、固定响应测试和基础 CI。

验收：

- 连接策略矩阵单元测试完整覆盖；
- 并发 token 刷新只产生一次上游刷新；
- 日志扫描不出现 token、Cookie 或 OAuth 响应体；
- 上游 DTO 不泄漏到 UI。

### 阶段 2：只读 MVP（4–6 周）

任务：

- 首次启动、连接选择、登录和账号状态页面。
- 推荐/排行榜、搜索、作品和作者页面。
- 单图、多图、缩放、缓存和分页。
- 网络诊断页和脱敏报告导出。
- 桌面键鼠、Android 返回键、生命周期和旋转适配。

验收：

- 三个平台完成登录到浏览作品的闭环；
- 网络切换不会泄漏 token 或让旧连接池继续使用错误配置；
- 离线时已缓存页面可以打开，并显示明确的离线状态；
- 图片加载失败可重试，不阻塞整个列表。

### 阶段 3：账号操作与媒体（3–5 周）

任务：

- 收藏、取消收藏、关注和取消关注。
- 下载队列、暂停、恢复、失败重试和目录选择。
- Ugoira ZIP、metadata、帧缓冲和基础播放。
- 缓存上限、清理和存储空间预警。

验收：

- 写操作具有幂等保护和明确结果；
- 下载任务在应用重启后恢复；
- Ugoira 长时间播放无明显持续内存增长；
- 清理缓存不影响用户下载。

### 阶段 4：加固与候选发布（3–5 周）

任务：

- 三个平台回归测试、崩溃恢复和性能优化。
- 完成威胁模型、依赖审计、许可证清单和 SBOM。
- 完成安装包、签名、升级/降级和数据库迁移测试。
- 编写隐私说明、已知限制、故障排查和贡献指南。
- 使用小范围测试用户验证登录和网络配置。

验收：

- 关键路径无阻断级缺陷；
- 所有发布产物来自固定依赖和可审计 CI；
- ECH、兼容直连和 WebView 限制在 UI 与文档中描述一致；
- 删除账号可清除安全存储、Cookie、数据库账号数据和相关缓存索引。

## 14. 测试策略

### 14.1 单元测试

- `ConnectionPolicy` 的模式 × 流量 × 主机 × 平台能力矩阵；
- DNS TTL、ECH 配置过期和 retry config；
- 重定向重新分类与未知主机拒绝；
- PKCE、`state` 验证、超时和取消；
- token single-flight、轮换和重新认证；
- DTO 到领域模型映射；
- 下载状态机、文件名清理和缓存淘汰。

### 14.2 集成测试

- 使用本地可控的 DNS、HTTP、TLS 和代理测试环境；
- 测试标准 TLS、证书错误、ECH 接受/拒绝和连接降级；
- 测试 CONNECT 代理不解密 TLS；
- 测试 SQLite 迁移和崩溃后的下载恢复；
- 上游响应使用脱敏 fixture，不把真实账号响应提交到仓库。

### 14.3 端到端测试

- Windows WebView2、Linux WebKitGTK、Android WebView 各自覆盖登录窗口生命周期；
- Android API 29、当前稳定版本和 API 36；
- 网络断开、切换 Wi-Fi/移动网络、代理失效、DNS 异常；
- 系统深色模式、缩放、窗口尺寸、Android 旋转和返回键；
- 登录活体测试仅由维护者手动触发，不在公共 CI 中保存账号凭据。

### 14.4 安全验证

- 自动扫描日志、崩溃报告和测试快照中的敏感字段；
- 确认证书错误无法被“继续访问”；
- 确认 WebView 导航无法调用未授权 Tauri 命令；
- 确认恶意文件名不能越出下载目录；
- 确认代理配置和诊断导出不会泄露凭据。

## 15. CI、发布与更新

- 使用 GitHub Actions 构建 Windows、Linux 和 Android。
- Android CI 分别编译 `arm64-v8a`、`armeabi-v7a`，并构建 `x86_64` 测试产物；正式发布为 ARM64、ARMv7 独立签名 APK。
- 固定 Cargo 与前端 lockfile，使用 Dependabot 或 Renovate 提交更新。
- 执行格式化、静态检查、单元测试、许可证检查和依赖漏洞扫描。
- 生成 SBOM、校验和与签名发布清单。
- Windows 提供安装包和便携包；Linux 至少提供一个通用包和一个发行版包；Android 提供签名 APK。
- Android 签名密钥离线备份，CI 使用受保护的发布凭据。
- 当前唯一活跃建设项是自动更新：桌面使用 Tauri updater 的签名更新产物，Android 使用独立签名清单、ABI APK 验证和系统安装器，详见[自动更新计划](docs/AUTO_UPDATE_PLAN.md)。
- 首版更新源推荐 GitHub Releases；自动检查默认开启，自动下载默认关闭，所有平台安装前保留用户确认。
- 更新检查和下载只使用系统网络与经过验证的 HTTPS，永不使用 Pixiv 低安全直连。
- 发布页明确列出支持平台、网络模式限制、上游接口非官方性质和数据清理方式。

## 16. 隐私与可观测性

- 默认不收集遥测、不上传访问历史、不运行自有流量中转。
- 诊断日志默认仅本地保存，具有容量和保留时间上限。
- 崩溃报告必须由用户主动导出或明确启用。
- 所有日志通过统一脱敏器处理，禁止模块绕过。
- 对外故障报告模板要求用户检查并删除可能包含账号信息的截图。

## 17. 主要风险与应对

| 风险 | 可能性 | 影响 | 应对 |
|---|---:|---:|---|
| OAuth 或 App API 需要不可安全分发的凭据 | 高 | 阻断 | 阶段 0 优先验证；保留用户导入 token 的开发路径；不可合法解决则停止 |
| 上游接口或请求头变化 | 高 | 高 | `PixivSource` 隔离、fixture 测试、版本探测、快速发布流程 |
| 系统 WebView 无法强制或观测 ECH | 高 | 中 | UI 如实报告；登录直连与严格 ECH 分开；未来评估自带内核 |
| 兼容 IP 或 ECH 配置过期 | 高 | 中 | DoH 动态获取、TTL、retry config、带期限缓存和诊断页 |
| Linux WebKitGTK 登录兼容性差 | 中 | 高 | 阶段 0 实测第三方登录、2FA、Cookie 和 callback；保留故障回退 |
| GPL 代码污染宽松许可证代码库 | 中 | 高 | 禁止直接复制，记录参考来源，合并前许可证检查 |
| token 或 Cookie 进入日志 | 中 | 高 | 集中脱敏、敏感字段类型化、自动扫描和安全评审 |
| 下载导致限流或磁盘耗尽 | 中 | 中 | 可见队列、并发上限、退避、空间检查、暂停和配额 |
| Android 厂商 WebView 行为差异 | 中 | 中 | WebView 能力探测、最低 API 29、真实设备矩阵和清晰错误提示 |
| 证书验证被兼容需求逐步弱化 | 中 | 高 | 把安全不变量写入 interface、测试和发布闸门，禁止运行时远程关闭 |

## 18. 发布完成标准

核心客户端当前已具备登录、会话安全存储、三种连接路线、主要浏览与账号操作、图片/Ugoira、小说、下载队列、离线资料库、本机历史和脱敏诊断。未完成的公开发布矩阵与活体验证已迁入[备选功能计划](docs/OPTIONAL_FEATURES_PLAN.md)，不属于当前活跃功能。

当前发布完成标准只针对自动更新：

- [ ] Windows NSIS、Linux AppImage、Android ARM64/ARMv7 生成可更新的正式签名产物。
- [ ] 桌面更新包通过 Tauri updater 签名验证；Android 通过清单、哈希、包名、ABI 和 APK 证书验证。
- [ ] 自动检查、可选自动下载、用户确认安装和手动检查均可用。
- [ ] 断网、清单损坏、签名错误、下载中断、空间不足、用户取消和重启恢复均经过测试。
- [ ] 更新后数据库、登录状态、离线资料库和设置保持可用。
- [ ] 更新流量不携带 Pixiv token/Cookie，不允许 HTTP、低安全直连、降级或 Debug 包进入 stable 通道。

## 19. 当前需要确认的决策

自动更新开始编码前需要确认：

1. 是否确定使用 GitHub Releases 作为更新源。
2. Windows 是否确定使用 NSIS 作为正式安装和自动更新格式。
3. Linux 是否接受首版仅 AppImage 支持应用内自动安装。
4. Android 是否接受自动检查、可选自动下载、系统确认安装，而不是静默安装。
5. 自动检查默认开启、自动下载默认关闭是否符合预期。

旧决策项和其他未完成功能已迁入[备选功能计划](docs/OPTIONAL_FEATURES_PLAN.md)。

## 20. 下一步

只执行[自动更新计划](docs/AUTO_UPDATE_PLAN.md)，顺序如下：

1. 与维护者确认更新源、正式安装格式、Android 交互和默认设置。
2. 建立长期签名密钥与 GitHub Draft Release 发布流水线。
3. 先完成签名版本检查和设置页状态，不立即开放安装。
4. 接入 Windows NSIS 与 Linux AppImage 的 Tauri updater。
5. 实现 Android ABI APK 下载、验证和系统安装 Adapter。
6. 完成从旧一版更新到当前版的跨平台回归与发布闸门。

## 21. 参考资料

- [自动检查更新与自动更新计划](docs/AUTO_UPDATE_PLAN.md)
- [备选功能计划](docs/OPTIONAL_FEATURES_PLAN.md)
- [项目目录空间清理计划](docs/STORAGE_CLEANUP_PLAN.md)
- [第一次 GitHub 上传清单](docs/FIRST_GITHUB_UPLOAD_CHECKLIST.md)
- [v0.28.2 私人测试版说明](docs/releases/v0.28.2.md)
- [项目内 GitHub 接入调研](docs/research/github-pixiv-integration.md)
- [PixEz 网络模式实现](https://github.com/Notsfsssf/pixez-flutter/blob/master/lib/network/pixez_network_settings.dart)
- [PixEz OAuth 实现](https://github.com/Notsfsssf/pixez-flutter/blob/master/lib/network/oauth_client.dart)
- [rustls `EchConfig`](https://docs.rs/rustls/latest/rustls/client/struct.EchConfig.html)
- [rustls `with_ech`](https://docs.rs/rustls/latest/rustls/struct.ConfigBuilder.html#method.with_ech)
- [rustls `ClientConnection::ech_status`](https://docs.rs/rustls/latest/rustls/client/struct.ClientConnection.html#method.ech_status)
- [AndroidX `ProxyController`](https://developer.android.com/reference/androidx/webkit/ProxyController)
- [AndroidX `WebViewFeature`](https://developer.android.com/reference/androidx/webkit/WebViewFeature)
- [Tauri 2 开发前置条件与 Android targets](https://v2.tauri.app/start/prerequisites/)
- [Android NDK 安装与版本固定](https://developer.android.com/studio/projects/install-ndk)
- [Android Command-line Tools](https://developer.android.com/tools)

## 22. 项目目录空间治理

2026-08-04 的最新只读审计显示，Rust `target/` 已增长到 138.464 GiB、241,707 个文件。本阶段只把 `target/` 作为清理范围；`artifacts/`、Android Gradle 输出、`node_modules/`、备份和其余项目目录全部暂时保留。

清理采用“复用优先”原则：保留当前 Windows/ARM64 的依赖、构建脚本输出、Cargo 指纹和 PixNya 成品，优先移除暂停使用的 ARM32、测试临时目录，以及旧主应用名 `pixiv-client` / `pixiv_client_lib` 的精确残留，预计回收约 66.28 GiB。工作区内部仍使用的 `pixiv-client-api`、`pixiv-client-network` 等 crate 不属于旧品牌残留。详细边界、目录用途和可选的平衡空间方案见[项目目录空间清理计划](docs/STORAGE_CLEANUP_PLAN.md)；当前尚未删除任何文件。
