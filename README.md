# PixNya

一个面向 Windows、Linux 和 Android 的非官方、开源、侧载 Pixiv 客户端。

## 项目性质

PixNya 是个人维护的独立项目，与 pixiv Inc. 没有隶属、授权、认可或技术支持关系。“官方网页登录”只表示登录时打开 Pixiv 提供的页面，不表示 PixNya 是官方客户端或正在使用面向第三方公开、稳定承诺的官方 API。

| 能力 | 对接方式 |
|---|---|
| 登录 | 隔离 WebView 中的 Pixiv 官方页面 + OAuth/PKCE；应用没有账号密码输入框 |
| 浏览与交互 | Rust 网络层访问 Pixiv 现有客户端使用的非公开 App API；上游变化可能导致功能失效 |
| 图片与 Ugoira | Rust 媒体管线访问 Pixiv 图片 CDN，前端不接触登录令牌 |

项目不托管 Pixiv 内容，不运营公共代理、VPN、账号中转或镜像服务，也不绕过登录、付费、年龄、地区或账号权限。用户应使用自己的账号，并自行遵守所在地法律、Pixiv 使用条款和作品版权要求。Pixiv 名称、标志和站内作品归各自权利人所有；本仓库的 GPL-3.0-only 许可证只覆盖 PixNya 自身代码。

PixNya 是独立实现，PixEz 等公开项目仅用于研究可观察行为和兼容思路，不是项目依赖，当前仓库未复制或改写 PixEz 源码。完整边界见[项目计划书](PROJECT_PLAN.md#11-项目性质与对接边界)。

隐私数据、网络请求与本机清除边界见[隐私说明](PRIVACY.md)；漏洞请按[安全政策](SECURITY.md)私密报告，不要在公开 Issue 中粘贴令牌、Cookie 或登录截图。

当前开发版已完成登录、主要浏览/交互、小说阅读、Ugoira 与离线资料库。项目没有账号密码输入框；低安全路线默认关闭，用户首次启用前必须确认风险，也可在完整披露后选择停止重复提醒。只有明确确认状态才会允许 Pixiv OAuth、API、图片以及 Android 非标准网页登录关闭上游 SNI 与证书验证。

已经完成的基础能力：

- Tauri 2 + Svelte 5 的 Windows/Android 共用界面；
- 接近 Pixiv 官方信息结构的响应式应用壳：桌面可折叠侧栏，Android 抽屉与底部导航；
- 单一导航模块驱动侧栏、作品标签、顶栏与移动底栏，同一入口的路由和选中状态保持同步；
- 统一设置中心集中管理账号、连接、界面、内容/存储与隐私选项；安全连接偏好可保存，低安全直连始终只作临时选择；
- 首页、插画、漫画、小说、关注新作、关注作者、发现、排行榜、收藏、搜索、个人主页、离线资料库和独立连接设置页面；
- 标准、严格 ECH、低安全直连的真实 Rust 网络探测，以及 `ConnectionPolicy` 模式矩阵测试；
- ECH 通过固定地址访问可信 DoH，动态读取 `HTTPS` 记录并要求 rustls 报告 `Accepted`；
- 低安全直连仅允许内置 Pixiv 域名/IP 白名单，启用前显示中间人攻击警告；
- 独立登录准备页、官方登录 WebView、Rust 内存 PKCE，以及与私有窗口/Activity launch ID 绑定的 callback 校验；
- Windows/Linux 使用独立 Tauri WebView；Android 使用独立原生 Activity并处理系统栏安全区；
- 标准登录走系统网络；Android 的 ECH/兼容网页登录使用仅监听回环地址的一次性低安全 TLS 桥；
- 登录桥为每次会话生成新证书，Android 只对当前 Pixiv 白名单和完全匹配的 SHA-256 指纹放行，其余证书错误仍拒绝；
- ECH 模式对 Rust API 做严格 `Accepted` 预检，但 Android 网页本身如实标记为低安全桥，不冒充 ECH；
- callback 只在原生层与 Rust 之间传递；authorization code 在 Rust 内交换，access token 只留内存；
- refresh token 在 Windows/Linux 使用系统凭据库，在 Android 使用 AndroidKeyStore AES-GCM 加密后保存；
- 启动时 single-flight 恢复会话，登录后展示 token 响应中的头像、昵称、账号与 Premium 状态，并支持安全退出；
- 自动恢复会话完成前不会短暂显示“未登录”引导；只有确认无会话或令牌恢复失败后才显示登录入口；
- 插画/漫画详情、多图、作者资料、相关作品、排行榜、趋势标签、作品/作者/小说搜索、插画与小说收藏、关注及评论/回复；
- 关注中心可在“最新作品”和“关注作者”间切换；作者列表支持公开/非公开范围、安全分页，并从 Rust 会话确定当前账号；
- 作品系列与小说系列拥有独立目录页、安全分页和“从第一部开始”入口；作品详情提供跨分页上一篇/下一篇，小说阅读器使用官方返回的系列相邻篇信息连续阅读；
- 在线与离线作品共用沉浸式原图查看器，支持多图翻页、1–6 倍缩放、键鼠拖动、触控双击及双指缩放；
- 独立 SQLite 浏览历史记录最近查看的作品、小说和作者：同项更新置顶、最多保留 500 条，可筛选、搜索、单条移除、暂停记录或全部清除；
- 小说推荐、关注新作、排行榜、公开/非公开收藏、作者小说列表；小说详情与正文阅读使用独立页面，阅读器提供安全解析、阅读样式、本机进度和系列连续阅读；
- 首页推荐标签会在本机缓存最近一次成功响应，网络失败或重启后不再退回固定演示标签；
- Ugoira ZIP 校验解压与播放，以及插画、小说、Ugoira 的事务式应用私有离线资料库；
- SQLite 本地目录支持离线内容收藏夹与最多 16 个标签，并可按关键字、类型、收藏夹、标签和下载时间/标题/空间组合筛选排序；
- SQLite 持久下载队列：串行执行、退出恢复、登录后自动继续，并支持暂停、继续、失败重试、进度展示和队列记录移除；
- 跨平台存储策略会显示下载安全可写空间，始终保留 512 MiB 系统余量；空间不足时停止离线写入，并支持 128 MiB–1 GiB 的持久缓存上限；
- API、图片与登录路线的脱敏连接诊断，以及按安全范围隔离、带容量上限和 LRU 淘汰的在线媒体缓存；
- 统一的本机脱敏诊断日志：只记录固定事件类别和计数/耗时，保留 7 天且限制为 256 KiB，支持用户主动导出或清除；
- 设置页可强确认清除令牌、登录 Cookie、下载队列、离线内容、缓存、浏览与搜索历史、阅读进度和界面偏好，并逐项报告失败；
- “默认显示 R18”是本机、默认关闭的界面开关，统一控制作品、小说及作者预览的受限内容遮罩，不改变 Pixiv 账号自身的内容范围；
- Pixiv 站内通知、评论删除、投稿、私信、直播与多账号没有可靠的 App API 或不在客户端授权范围内，界面会明确标记而不伪造数据；
- Android API 29；正式发布当前只提供 ARM64 APK，ARMv7 分离调试构建入口继续保留但暂不发布。

三种模式现在都能在 Android 打开官方登录页并完成回调处理：标准模式使用系统 TLS；ECH 和兼容模式经本地桥连接内置 Pixiv IP。ECH 模式的令牌交换重新使用强制 ECH 与证书验证；兼容模式的网页登录和令牌交换均关闭上游 SNI 与证书验证。桥不记录 HTTP 正文，但解密后的页面数据会经过应用内存。

> **安全警告：**低安全直连可能让中间人读取或修改账号密码、二步验证码、authorization code、refresh token、access token 与 API 数据。它不会自动启用、保存为默认模式或在 ECH 失败时静默回退。用户可以停止重复显示警告，但这不会降低风险；提醒可在“连接与安全”中恢复。

## 界面语言

PixNya 的界面支持简体中文、繁體中文和 English。默认跟随操作系统，也可以在“设置 → 界面 → 界面语言”中固定语言；选择会仅保存在本机。Pixiv 返回的作品标题、作者名、标签、评论和小说正文保持原文，不会由客户端自动翻译。

语言资源由 Paraglide JS 根据 `messages/` 中的三个 JSON 词库生成。应用不使用语言前缀 URL；系统语言中的 `zh-Hant`、`zh-TW`、`zh-HK` 与 `zh-MO` 映射为繁体中文，其他中文区域映射为简体中文，非中文环境映射为英文。

## 本地开发

从 `.env.example` 创建被 Git 忽略的 `.env.oauth.local`，填入本地兼容性测试所需的三个构建参数。构建脚本会拒绝生成缺少这些参数、无法完成令牌交换的测试包。参数会被编译进客户端，因此可从 APK/EXE 中提取；不要把测试产物当作能够保密的凭据容器，也不要在完成上游授权与分发审查前公开发布。

```powershell
npm install
npm run check
npm run check:navigation
npm run check:avatar
npm run tauri dev
```

生成可独立双击、无需本地 Vite 服务的 Windows 调试客户端：

```powershell
npm run build:desktop:debug
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-windows-standalone.ps1
```

构建完成后，EXE 会自动复制到 `artifacts\windows`。

不要用普通 `cargo build` 生成供测试人员直接启动的客户端；该命令不会执行前端构建流程，Debug 产物会加载 `devUrl`。

Rust 策略测试：

```powershell
cargo test --workspace
```

## 版本规则

版本号使用 `主版本.功能版本.修复版本`：修复 bug 只增加第三位，新增功能增加第二位，只有不兼容的大改版才增加第一位。当前版本为 `0.29.0`。

## 当前开发路线

当前只保留自动检查更新与自动更新这一项活跃功能：

- [自动检查更新与自动更新计划](docs/AUTO_UPDATE_PLAN.md)
- [暂停的备选功能计划](docs/OPTIONAL_FEATURES_PLAN.md)

备选计划中的功能没有排期，也不计入当前尚未完成的任务。

## Android 环境与构建

本机工具链位于 F 盘：

- JDK 17：`F:\ACM\.toolchains\jdk-17`
- Android SDK/NDK：`F:\ACM\.toolchains\android`
- Gradle 缓存：`F:\ACM\.toolchains\gradle`

首次或环境变量变更后，在新 PowerShell 中执行环境脚本：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File F:\ACM\.toolchains\android\setup-android-env.ps1
```

初始化 Android 工程：

```powershell
npm run tauri android init
```

生成 ARM64 与 ARMv7 分离调试 APK：

```powershell
npm run tauri -- android build --debug --target aarch64 armv7 --split-per-abi --apk --ci
```

只构建 ARM64 并自动收集 APK：

```powershell
npm run build:android:arm64:debug
```

只构建 ARMv7 并自动收集 APK：

```powershell
npm run build:android:armv7:debug
```

该快捷命令会裁剪 Rust 调试符号，避免测试 APK 再次膨胀到数百 MB。

产物位于：

- `src-tauri/gen/android/app/build/outputs/apk/arm64/debug/app-arm64-debug.apk`
- `src-tauri/gen/android/app/build/outputs/apk/arm/debug/app-arm-debug.apk`

对外测试时统一从以下目录取文件：

- `artifacts\windows`：Windows EXE；
- `artifacts\android`：Android APK；
- `artifacts\SHA256SUMS.txt`：校验值。

完整设计与安全约束参见 [PROJECT_PLAN.md](PROJECT_PLAN.md)。

Linux 由 `scripts/check-linux.sh` 执行前端、Rust、测试与 Tauri 桌面编译检查；GitHub Actions 在 Ubuntu 22.04 上运行同一入口。本机是 Windows，因此 Linux 产物必须以该流水线或真实 Linux 主机的结果为准。

## 许可证

项目代码按 [GNU GPL-3.0-only](LICENSE) 发布。Pixiv 名称、商标与服务内容归其各自权利人所有；本项目与 pixiv Inc. 无隶属、授权或认可关系。

关于 PixEz 等参考项目与本项目代码之间的边界，见 [来源与独立实现说明](docs/PROVENANCE.md)。
