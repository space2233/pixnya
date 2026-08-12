# PixEz 连接实现源码调查

> **后续项目决策（2026-08-02）：**本项目保留本文对 PixEz 兼容模式风险的判断，但按产品决策增加一个明确标为“低安全”的可选实现。它默认关闭、逐次确认、限制在 Pixiv 白名单域名，并禁止用于网页登录、OAuth 和 token 交换；ECH/标准模式失败时也不会静默回退到它。

调查日期：2026-08-02
主要对象：[Notsfsssf/pixez-flutter](https://github.com/Notsfsssf/pixez-flutter)
当前源码基线：[`6388dd88d40315d6de1b610cae7e1b48ea80d221`](https://github.com/Notsfsssf/pixez-flutter/tree/6388dd88d40315d6de1b610cae7e1b48ea80d221)

本文只把仓库源码、提交记录和直接依赖文档当作证据。为避免把名称当成能力，以下状态严格区分为：

- **已实现**：存在可到达的完整代码路径。
- **仅接口/配置**：UI、枚举或 MethodChannel 存在，但底层没有实现。
- **已注释或 TODO**：源码明确不执行。
- **推断**：由多段源码组合得出，文中会明确标出。

## 结论摘要

1. 当前 PixEz 的 `standard / ech / compat` 三模式首先是 **Rust HTTP 客户端模式**，不是覆盖整个应用（尤其不是覆盖系统浏览器或 WebView）的全局网络栈。[`NetworkMode`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/network/network_mode.dart#L1-L24) 定义三种模式；API、OAuth 和账户客户端才把它们转换成 rhttp 设置。
2. **ECH API 路径已真实实现**：PixEz 固定把 API/OAuth/Account 域名连接到 Cloudflare IP，向 AliDNS 查询 `cloudflare-ech.com` 的 HTTPS RR，取出 `ech` 参数，再用 rustls `EchConfig` 构建 TLS 1.3 ECH 客户端。它不是简单“改 hosts”。
3. **兼容直连已真实实现，但安全性很弱**：它把目标域名解析成单个预置 IP，同时关闭 SNI 和服务器证书验证。这并不是“连接 IP、仍以原域名校验证书”的安全 Host override，而是明确接受任意证书。
4. **普通模式没有 PixEz 自己的特殊解析**：传给 rhttp 的设置为 `null`，使用 rhttp/reqwest 默认 TLS、DNS 与代理行为。仓库也有自定义代理类型，但 PixEz 三模式配置没有给它传入代理 URL，不能把“标准模式”表述成一个由 PixEz 实现的代理模块。
5. **网页登录没有完整接入后两种路径**。Android/Windows 的 Weiss 本地代理启动和 WebView 代理覆盖代码已被注释或仅为 TODO；Linux 没找到对应实现。iOS、macOS 又走各自独立的 WebView/系统浏览器。因此“ECH 登录”和“兼容直连登录”在当前源码中都不能视为已实现。
6. 图片请求也不是 ECH：`ech` 和 `compat` 对 `i.pximg.net` 都复用兼容模式，即关闭 SNI和证书验证；`standard` 才使用默认网络栈。

## 1. 当前总体调用链

```text
用户设置 networkMode / oauthNetworkMode
  ├─ API: ApiClient.createDioClient()
  ├─ OAuth: OAuthClient.createDioClient()
  └─ Account: AccountClient.createDioClient()
       ↓
PixezNetworkSettings.forHost(host, mode)
       ↓
RhttpCompatibleClient.create(settings)
       ↓
dio_compatibility_layer.ConversionLayerAdapter
       ↓
vendored rhttp (Rust) → reqwest → rustls
```

API 侧的适配入口是 [`ApiClient.createDioClient`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/network/api_client.dart#L71-L98)，OAuth 侧是 [`OAuthClient.createDioClient`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/network/oauth_client.dart#L56-L83)。它们保留 Dio 的上层 API，只替换 `httpClientAdapter`，所以业务请求无需知道底层是标准、ECH 还是兼容传输。

每次 Rust 请求都会先解析 URL，再调用 `client_for_url()` 选择普通或 ECH 客户端，最后用选中的 reqwest client 执行请求；参见 [`make_http_request_helper`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/plugins/rhttp/rhttp/rust/src/api/http.rs#L340-L365) 和执行位置 [`execute`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/plugins/rhttp/rhttp/rust/src/api/http.rs#L448-L465)。

## 2. ECH 路径：已实现

### 2.1 Dart 侧配置

[`PixezNetworkSettings.forHost`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/network/pixez_network_settings.dart#L14-L34) 在 `ech` 模式下设置：

- `enableEch: true`
- `requireEch: true`
- 保持 `verifyCertificates: true`
- 使用内置 WebPKI 根证书
- 保持 `sni: true`
- 将 `app-api.pixiv.net`、`oauth.secure.pixiv.net`、`accounts.pixiv.net` 都覆盖到 `104.18.10.118` 和 `104.18.11.118`

所以这里同时做了两件事：**连接地址覆盖**和 **ECH TLS**。它没有修改操作系统 hosts，也没有把 URL 的域名替换成 IP；reqwest 仍拿原域名构造请求和 TLS 内层名称。

### 2.2 Rust 侧配置获取与握手

vendored rhttp 把 AliDNS 端点固定为 `https://223.5.5.5/resolve`，把 ECH bootstrap 名固定为 `cloudflare-ech.com`；见 [`client.rs` 常量](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/plugins/rhttp/rhttp/rust/src/api/client.rs#L25-L40)。实际流程为：

1. `client_for_url()` 判断是否为 HTTPS、域名不是 IP、SNI 开启且没有强制 TLS 1.2。
2. 以请求域名作为 cache key 查 ECH client；缓存有效期采用 DNS 回答 TTL。
3. 无缓存时调用 `lookup_ech_config()`。
4. `lookup_ech_config()` **忽略实际请求 host**，固定查询 `cloudflare-ech.com` 的 `HTTPS` RR。
5. 从 AliDNS JSON 的 `Answer[].data` 中提取 `ech="..."`，Base64 解码。
6. 用 `EchConfig::new(...)` 和 AWS-LC 支持的 HPKE suites 构造配置，再调用 rustls `with_ech(EchMode::from(ech_config))`。
7. 把该 rustls 配置交给 reqwest，并继续应用 Dart 侧的静态 DNS 覆盖。

关键源码分别是 [`client_for_url` 和 fail-closed 分支](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/plugins/rhttp/rhttp/rust/src/api/client.rs#L182-L252)、[`build_ech_tls_config`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/plugins/rhttp/rhttp/rust/src/api/client.rs#L479-L590)，以及 [AliDNS 查询和解析](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/plugins/rhttp/rhttp/rust/src/api/client.rs#L754-L875)。

`requireEch` 的精确含义是：ECH 配置查询失败、缺失或 ECH client 构建失败时，不回落到普通 client。仓库没有调用 rustls 的 `ech_status()`，因此应用层没有“Accepted/Rejected”遥测；但其使用的是 rustls `EchMode::Enable`，不是 GREASE。rustls 0.23 文档说明 ECH 配置应来自目标服务器域名的 HTTPS RR，并且该配置只适合提供它的域名；见 [rustls `EchConfig`](https://docs.rs/rustls/0.23.40/rustls/client/struct.EchConfig.html) 和 [`with_ech`](https://docs.rs/rustls/0.23.40/rustls/struct.ConfigBuilder.html#method.with_ech)。PixEz 固定跨域使用 `cloudflare-ech.com` 配置是其专门的前置域设计，而不是通用 ECH 解析器。

### 2.3 ECH 没覆盖图片

[`forImages`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/network/pixez_network_settings.dart#L37-L41) 对任何非 `standard` 模式都返回 `compatible()`，且只在 host 为 `i.pximg.net` 时生效。因此 `ech` 模式下的图片下载仍是下节的“关 SNI + 关证书验证”路径，而不是 ECH。

## 3. Host / 自定义 DNS / 兼容直连：已实现，但不安全

[`compatible()`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/network/pixez_network_settings.dart#L43-L63) 做法很直接：

- `verifyCertificates: false`
- `sni: false`
- 动态 DNS resolver 先查 `Hoster`；不认识的域名才调用 `InternetAddress.lookup()`

`Hoster` 的默认映射是：API/OAuth → `210.140.139.155`，图片 → `210.140.139.133`；见 [`Hoster._constMap`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/er/hoster.dart#L11-L30)。它还会从本地偏好读取缓存 IP，并用固定 IP 访问 Cloudflare DoH；当前 `dnsQueryAll()` / `dnsQueryFetcher()` 只刷新两个图片域名，选取回答中 TTL 最大的单个 IPv4 地址并持久化，见 [`Hoster` 查询实现](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/er/hoster.dart#L32-L96)。

这条路径没有候选 IP 池、并发竞速、健康评分或证书域名校验。`assets/json/host.json` 只是与 `_constMap` 相同的静态数据；真正被上述调用链读取的是 `Hoster` 内存映射与偏好缓存。

安全边界必须明确：关闭证书验证意味着中间人只要能截获连接就可伪装服务器。它不应成为默认路径，也不适合承载授权码或 token。研究阶段原本建议登录也保持原 URL、SNI 和证书域名校验；真实网络证明该路线会被主动断开后，项目所有者明确接受了第 8 节所述的逐次确认风险方案。该实现选择不改变这里的安全结论。

## 4. 标准/系统代理路径：仅默认行为，没有独立 PixEz 模块

`standard` 在 [`forHost`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/network/pixez_network_settings.dart#L14-L16) 和 `forImages()` 中都返回 `null`，于是 rhttp 创建默认 reqwest client。rhttp 本身定义了 `NoProxy` 和自定义代理列表，并在构建 client 时调用 reqwest 的代理 API；见 [`ProxySettings`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/plugins/rhttp/rhttp/rust/src/api/client.rs#L43-L65) 与 [应用位置](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/plugins/rhttp/rhttp/rust/src/api/client.rs#L304-L328)。

但是 PixEz 的三模式配置没有传 `proxySettings`，所以能从项目源码确定的只有“未覆盖 reqwest 默认行为”。不要据此宣称 PixEz 实现了 Windows/Android 系统代理同步；源码中没有这样的同步层。

## 5. 官方网页登录 / WebView：路径分裂，后两种模式未真正接通

### 5.1 授权协议本身已实现

[`OAuthClient.generateWebviewUrl`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/network/oauth_client.dart#L131-L156) 生成 PKCE verifier/challenge，打开 Pixiv 的 `app-api.pixiv.net/web/v1/login`。页面最终跳到 `pixiv://...`；内置 WebView 在导航回调中拦截该 scheme，交给 `Leader.pushWithUri()`，再由 `code2Token()` 通过 OAuth Rust client 换 token。这个“授权页面 → code → token”的逻辑是已实现的。

### 5.2 页面由谁打开

[`LoginPage._launch`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/page/login/login_page.dart#L164-L190) 的平台分支是：

| 平台/模式 | 页面容器 | 是否接入 Rust ECH/兼容解析 |
|---|---|---|
| iOS，所有模式 | Flutter `WebViewPage` | 否；在进入模式判断前已返回 |
| macOS，所有模式 | `url_launcher` 系统浏览器 | 否 |
| Android `standard` | Android Custom Tab | 否；浏览器自己的网络栈 |
| Windows/Linux `standard` | `url_launcher` 系统浏览器 | 否；浏览器自己的网络栈 |
| Android/Windows/Linux，`ech` 或 `compat` | 调 Weiss 后打开 Flutter WebView | 设计上想走本地代理，当前底层未实现 |

这里有一个容易误读的细节：[`usesCompatibleConnection`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/network/network_mode.dart#L23-L24) 对 `ech` 和 `compat` 都返回 `true`。所以选择 ECH 并不会让 WebView 使用 Rust ECH client，只会进入与兼容模式相同的 Weiss 分支。

### 5.3 Weiss 状态：当前是空实现

Dart 侧 [`WeissPlugin`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/weiss_plugin.dart#L1-L30) 会请求启动 `127.0.0.1:9876` 并设置 WebView proxy；但是：

- Android 的 `weiss.Weiss.start/close`、`ProxyController.setProxyOverride` 全部被注释，方法只返回成功：[Android `Weiss.kt`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/android/app/src/main/kotlin/com/perol/pixez/plugin/Weiss.kt#L31-L59)。对应的 `:weiss` module 和 `androidx.webkit` 依赖也处于注释状态：[Android `build.gradle.kts`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/android/app/build.gradle.kts#L141-L149)。因此这不是“运行时探测失败”，而是当前构建根本没有编入代理实现。
- Windows 的 `Start/Stop/Proxy` 都只有 `TODO`：[Windows `weiss_plugin.cpp`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/windows/runner/plugins/weiss_plugin.cpp#L43-L55)。
- 当前仓库没有 Linux Weiss 实现。

因此登录 WebView 的“直连代理”目前是**仅接口/已删除实现残迹**，不能算可用功能。WebView 本身还会在非标准模式对 `accounts.pixiv.net` 页面注入 JavaScript，隐藏 POST 表单和社交登录按钮；见 [`WebViewPage`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/page/webview/webview_page.dart#L25-L74)。这降低了误在不安全链路输入凭据的概率，但不等于建立了直连网络路径。

标准 Android 登录使用 [`CustomTabsIntent`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/android/app/src/main/kotlin/com/perol/pixez/plugin/CustomTab.kt#L13-L39)。成功后 `pixiv://...` intent 由原生 `DeepLinkPlugin` 接收；[`AndroidHelloPage.initPlatform()`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/page/hello/android_hello_page.dart#L432-L445) 读取最新链接并订阅 `uriLinkStream`，再交给 `Leader.pushWithUri()` 换 token。因此当前源码中的标准 Custom Tab 回调闭环是完整的。内置 WebView 则直接在 `NavigationDelegate` 中拦截同一 `pixiv` scheme。

还需区分“网页登录页面”和“回调后的 token 请求”：Android WebView 由 `webview_flutter` 的 `WebViewController.loadRequest()` 直接加载 URL，没有接入 rhttp；所以 PixEz 的 ECH 不覆盖页面 TLS。只有回调被 `Leader.pushWithUri()` 处理后，`OAuthClient.code2Token()` 发出的 `/auth/token` 请求才使用 `oauthNetworkMode` 对应的 rhttp ECH/兼容/标准设置。

## 6. 历史版本：能证明什么，不能证明什么

- [`0.9.88` 对应提交 `52a422ff…`](https://github.com/Notsfsssf/pixez-flutter/tree/52a422ff96293e1343f84c7d48b347a82d9b4064) 的 `pubspec.yaml` 使用发布版 `rhttp 0.15.1`；其 [`ApiClient`](https://github.com/Notsfsssf/pixez-flutter/blob/52a422ff96293e1343f84c7d48b347a82d9b4064/lib/network/api_client.dart#L66-L109) 只有 `disableBypassSni` 开关、关闭证书验证/SNI和单 IP resolver，没有 `enableEch` 调用。也就是说，该 tag 的仓库源码不能证明 ECH 已启用。
- 首个可由提交历史直接定位的公开 ECH 集成是 2026-04-05 的 [`50bfa20 "rhttp ech"`](https://github.com/Notsfsssf/pixez-flutter/commit/50bfa20b914d8be9cd75187071857a37ca98fe99)。它给 API client 增加 `enableEch: true` 并把修改后的 rhttp 纳入仓库。该提交中的 Rust 代码已经通过 DoH/AliDNS动态取得 ECH 参数，并非一段固定的 `ECHConfigList` 字节数组。
- 因而，如果某个 release 文案把 0.9.88 描述成“硬编码 ECH”，最多只能视为发布说明或构建产物描述；**公开 tag 源码无法复原该实现**。可能存在未提交补丁、不同构建分支或措辞不准确，但这都属于推断，不能写成源码事实。

## 7. 对本项目可借鉴与不应照搬的部分

值得借鉴：

- 用一个稳定的 `NetworkMode` 领域枚举控制 API、OAuth、账户和图片客户端。
- 通过 Dio/HTTP adapter 隔离业务 API 与底层传输。
- 把 ECH 配置获取、TLS client 构建和 TTL cache 放在 Rust 层。
- 登录采用官方网页 + PKCE，并只拦截最终回调，不读取用户密码。

原安全评估认为不应照搬（其中第一项后来仅在第 8 节的严格边界内获准采用）：

- 兼容模式关闭证书验证和 SNI。
- 用单个永久 IP 代替候选池与健康探测。
- 把 ECH 配置固定为第三方 bootstrap 域，而不验证目标域的 HTTPS RR策略。
- 把 WebView 代理当成已有能力；PixEz 当前正说明 UI/MethodChannel 存在不代表底层已工作。
- 用同一个 `usesCompatibleConnection` 布尔值合并 ECH 和兼容模式；两者的安全属性与登录能力不同，应该分别建模。

对我们的客户端，三种模式必须分别给出“API/图片/登录页”的能力矩阵，不能因为 Rust API 的 ECH 成功就把 Android WebView 标成 ECH。原先建议登录仅使用不解密 TLS 的 CONNECT 代理；真实网络测试证明 Android WebView 的可见 SNI 仍会被主动断开后，项目所有者于 2026-08-02 明确选择加入逐次确认的低安全兼容桥。下面记录最终实现，避免把新决策误认为 PixEz 当前源码本身已有的能力。

## 8. 本项目采用的 Android 低安全登录桥

本项目复用 PixEz 兼容路径的**上游行为**：命中封闭 Pixiv 白名单时连接固定 IP，并关闭上游 SNI 与服务器证书链验证。由于 Android WebView 没有公开接口直接配置这两个 TLS 属性，具体实现不是照抄 PixEz 当前空置的 Weiss 插件，而是在应用内补上一层 TLS 终止桥：

1. Rust 代理只监听随机的 `127.0.0.1` 端口，只接受 HTTP `CONNECT`，只允许 `443`。
2. 每次登录新生成一张自签名证书和私钥；证书与私钥只存在于本次 `LoginProxy` 生命周期的内存中。
3. Rust 把叶证书 SHA-256 指纹随登录启动参数传给 Android。
4. Android 只在 `compatible` 模式、请求 URL 属于固定 Pixiv 白名单、证书指纹与本次会话完全一致时调用 `SslErrorHandler.proceed()`；标准与 ECH 模式及其他所有证书错误继续取消。
5. 代理解开 WebView TLS 后，以固定 IP 建立第二段 rustls 连接；使用 IP 类型 `ServerName`、显式关闭 SNI，并跳过上游证书链验证，但仍验证 TLS 握手签名。
6. 两段 TLS 之间仅用 `copy_bidirectional` 转发字节；不解析、不修改、不记录 HTTP 正文。尽管如此，登录数据仍以明文经过应用进程内存，不能称为端到端安全。
7. 未命中固定 IP 表的第三方域名保持普通端到端 CONNECT 隧道，不继承低安全 TLS。
8. ECH 模式会用 Rust 对 API 目标做 `Accepted` 验证；Android 系统 WebView 无法把该验证绑定到页面自身的 TLS 连接，因此 Android 的 ECH 登录明确失败关闭，不映射到普通 TLS 或低安全桥。
9. callback 后的 authorization code/token 交换不得经过该桥；接入时必须强制使用已验证的 Rust ECH 路线并单独测试。

实时回归测试已经通过本地桥请求 `https://app-api.pixiv.net/web/v1/login`，用于证明“回环 CONNECT → 一次性本地 TLS → 固定 Pixiv IP → 无 SNI 上游 TLS”链路能够取得官方页面响应。此测试不包含账号、密码、二步验证码或 token。
