# Pixiv 登录后 App API 数据接入调查

调查日期：2026-08-03
主要实现参考：[Notsfsssf/pixez-flutter](https://github.com/Notsfsssf/pixez-flutter)
PixEz 源码基线：[`6388dd88d40315d6de1b610cae7e1b48ea80d221`](https://github.com/Notsfsssf/pixez-flutter/tree/6388dd88d40315d6de1b610cae7e1b48ea80d221)（提交时间 2026-07-30）

Pixiv 没有为这套移动端 App API 提供稳定的公开开发者契约。本文只记录下列证据，不把第三方博客或记忆中的旧参数当作事实：

- PixEz 当前源码实际构造的请求、模型与分页路径；
- 2026-08-03 对 Pixiv 第一方端点的无账号实时探测；
- Pixiv 官方帮助中心与 Google Play 中由 pixiv Inc. 发布的信息。

文中的状态定义如下：

- **源码已证实**：可由固定提交中的完整调用路径确认。
- **实时已观察**：调查当天由官方域名直接返回；由于 API 无公开兼容承诺，未来可能改变。
- **推断/建议**：为了本项目安全、容错而提出，不冒充官方要求。

本文没有记录任何真实 access token、refresh token、OAuth client secret 或其他账号凭据，也没有用真实账号请求推荐数据。

## 结论摘要

1. PixEz 当前登录后推荐插画首屏实际调用：

   ```text
   GET https://app-api.pixiv.net/v1/illust/recommended
       ?filter=for_ios
       &include_ranking_label=true
   Authorization: Bearer <access token>
   ```

   这与一些旧示例中的 `filter=for_android&include_ranking_illusts=true&include_privacy_policy=true` 不同；后者不能标成“当前 PixEz 行为”。来源见 [`getRecommend()`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/network/api_client.dart#L197-L201)。
2. 通用 API headers 至少包括 `Authorization: Bearer ...`、`User-Agent`、`Accept-Language`、`App-OS`、`App-OS-Version` 和 `App-Version`；PixEz 还发送 `X-Client-Time`、`X-Client-Hash` 与显式 `Host`。其中只有 `Authorization` 能从源码调用链确认是登录后请求的必要身份材料；不能仅凭 PixEz“发送了某 header”便宣称服务端强制要求它。
3. 当前错误形态不是只能按 `401` 判断。2026-08-03 用故意无效的 Bearer 值请求推荐端点，官方 API 返回 **HTTP 400**，JSON 为顶层 `error` 对象，`error.message` 包含 OAuth 与 `invalid_grant`。应按“状态码 + 结构化错误内容”分类，并最多刷新、重试一次。
4. 插画卡片需要保留 `id/title/type/image_urls/user/page_count/is_bookmarked/x_restrict/sanity_level`。官方无登录 walkthrough 响应在调查当天也包含这些字段，说明它们仍是现行 Illust 结构的一部分；但登录后推荐的成功响应本次未用真实账号验证。
5. `next_url` 应只在 Rust 后端保存和跟随。必须验证 `https`、精确 host、精确 path、端口/用户信息/fragment，并禁用重定向。尤其低安全兼容模式可能被中间人篡改响应，不能照搬 PixEz 的“接收任意字符串后直接 GET”。
6. `i.pximg.net` 缩略图不能当作普通 WebView `<img>` 直链。PixEz 为图片固定添加 `Referer: https://app-api.pixiv.net/`；同一缩略图在本次兼容链路探测中，无 Referer 为 `403`，加该 Referer 为 `206`。图片应由 Rust 媒体层请求并缓存，且绝不能把 Bearer token 发给 pximg。

## 1. 已验证的请求头

### 1.1 PixEz 的 API client 默认头

PixEz 在 [`ApiClient` 构造器](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/network/api_client.dart#L102-L122) 中设置如下信息；Android 启动后又在 [`createDioClient()`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/network/api_client.dart#L75-L91) 中用真实系统版本和设备型号更新 User-Agent 与 `App-OS-Version`。

| Header | 当前 PixEz 行为 | 本项目建议 |
|---|---|---|
| `Authorization` | 除 walkthrough 外，由拦截器添加 `Bearer <access token>` | 只在 Rust 内存中组装；不得序列化给前端、日志或错误对象 |
| `User-Agent` | Android 为 `PixivAndroidApp/5.0.166 (Android <release>; <model>)`；其他平台保留 Pixel C 默认值 | 先集中为单一兼容配置，不在各 command 重复硬编码 |
| `Accept-Language` | 固定 `zh-CN` | 可随应用语言变化，但需输出合法、稳定的语言标签 |
| `App-OS` | `Android` | 若采用 Android 兼容身份，须与 UA、OS version 保持一致 |
| `App-OS-Version` | Android 动态为 `Android <release>`；默认 `Android 10.0` | 同上，不要产生彼此矛盾的 header 组合 |
| `App-Version` | 固定 `5.0.166` | 作为可更新兼容配置；不可散落为 magic string |
| `X-Client-Time` | client 构造时生成一次 | 若复用现有 OAuth 签名模块，应生成真实 UTC 时间，避免把本地时间错误标为 `+00:00` |
| `X-Client-Hash` | 对 time 与一个内置常量拼接后做 MD5 | 复用既有安全封装；本文不记录常量值 |
| `Host` | PixEz 显式写 `app-api.pixiv.net` | reqwest 应从已验证 URL 自动生成，避免手工 Host 与 URL authority 不一致 |

`Authorization` 的完整调用链见 [`RefreshTokenInterceptor.onRequest`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/network/refresh_token_interceptor.dart#L23-L49)：它从当前账户读取 access token，加 `Bearer ` 前缀，并只对 `/v1/walkthrough/illusts` 例外。无 token 时请求会在本地被拒绝。

### 1.2 版本漂移不能靠猜

PixEz 的兼容身份仍是 `5.0.166`。调查当天，[pixiv 官方 Google Play 页面](https://play.google.com/store/apps/details?id=jp.pxv.android)显示的应用版本已经是 `6.191.1`；而第一方 [`/v1/application-info/android`](https://app-api.pixiv.net/v1/application-info/android) 在携带 PixEz 旧版 headers 时返回的 `latest_version` 是 `6.66.1`，同时标记 `update_required=true`。两者并不一致。

因此：

- **已证实**：`5.0.166` 是当前 PixEz 使用且仍能抵达 API 鉴权层的组合；
- **不能推出**：`5.0.166` 是官方最新版本，或服务端永远接受它；
- **实现建议**：把 headers 版本集中配置，并设置协议回归测试。不要每次启动自动把 Google Play 或 `application-info` 的字符串直接写进 UA，因为不同发布通道可能不一致。

## 2. 推荐插画首屏

### 2.1 Endpoint

当前 PixEz 源码明确调用：

```http
GET /v1/illust/recommended?filter=for_ios&include_ranking_label=true HTTP/1.1
Host: app-api.pixiv.net
Authorization: Bearer <access token>
```

入口是 [`ApiClient.getRecommend()`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/network/api_client.dart#L197-L201)。登录后的首页在账户存在时把它作为数据源，见 [Flutter 首页初始化](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/fluent/page/splash/splash_page.dart#L35-L40)。

PixEz 用 [`Recommend`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/models/recommend.dart#L22-L42) 解析响应，已建模的顶层字段是：

- `illusts`：首屏卡片列表；
- `ranking_illusts`：可选；
- `contest_exists`：可选；
- `privacy_policy`：可选；
- `next_url`：可空字符串 URL。

首个垂直切片只应依赖 `illusts` 和 `next_url`，对其他未知顶层字段保持向前兼容并忽略。`include_ranking_label=true` 不等于响应一定含 `ranking_illusts`；不要因名字相似把二者强行绑定。

### 2.2 本次实时验证的边界

本次没有使用真实账户或保存的 token，因而没有把“推荐端点成功返回 200”列为已证实。完成了两项无账号验证：

1. 用故意无效的 Bearer 值请求上述推荐 URL，服务端到达 OAuth 鉴权逻辑并返回本文第 6 节的结构化 400；
2. 请求 PixEz 明确排除鉴权的第一方 [`/v1/walkthrough/illusts`](https://app-api.pixiv.net/v1/walkthrough/illusts)，返回 200，顶层键为 `illusts`、`next_url`，其中首个 Illust 对象含本文第 3 节列出的关键字段。

这足以支持 DTO 与错误分类的离线实现，但正式验收仍需用户在客户端登录后做一次推荐首屏、刷新 token、下一页和图片加载的端到端测试。

## 3. Illustration card 字段

PixEz 的现行模型见 [`Illusts`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/models/illust.dart#L89-L164)、[`ImageUrls`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/models/illust.dart#L179-L193) 和 [`User`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/models/illust.dart#L195-L226)。2026-08-03 walkthrough 实时响应也观察到这些键。

| JSON 字段 | 建议领域类型 | 卡片用途与注意点 |
|---|---|---|
| `id` | `u64`，跨 IPC 转字符串 | 作品稳定标识；JS 数字精度不是当前 ID 的契约，字符串更稳妥 |
| `title` | `String` | 标题可为空，UI 提供占位 |
| `type` | 字符串/非穷尽枚举 | 已知常见值有插画、漫画、动图；未知值不能让整页反序列化失败 |
| `image_urls` | 对象 | 常见 `square_medium`、`medium`、`large`；卡片优先缩略尺寸 |
| `user.id` | `u64`，跨 IPC 转字符串 | 作者标识 |
| `user.name` | `String` | 显示名 |
| `user.account` | `String` | 账号名 |
| `user.profile_image_urls.medium` | `Option<URL>` | 头像，仍需走 pximg 媒体层 |
| `user.is_followed` | `Option<bool>` | API 可能省略；省略不等同于服务端明确返回 false |
| `page_count` | `u32` | 大于 1 时显示多图角标；详情图来自 `meta_pages` |
| `is_bookmarked` | `Option<bool>` | 登录后收藏状态；缺失与 false 最好在 transport 层区分 |
| `x_restrict` | 保留原始非负整数 | 内容分级字段；本文未从第一方公开契约验证所有数值语义 |
| `sanity_level` | 保留原始非负整数 | 内容/审核相关等级；同样不要仅凭旧社区映射丢弃未知值 |

其他建议保留或容错的字段包括 `visible`、`is_muted`、`width`、`height`、`tags`、`illust_ai_type`、`meta_single_page` 和 `meta_pages`。调查当天 walkthrough 的两个响应还出现过可选的 `restriction_attributes` 差异，说明 parser 必须允许新增/缺失字段。

原图位置不能从卡片顶层 `image_urls` 假设得到：

- 单页作品可能把原图放在 `meta_single_page.original_image_url`；
- 多页作品在 `meta_pages[].image_urls.original`；
- 首屏卡片不需要原图，先只传缩略图 URL，可显著降低流量和内存。

对于 `x_restrict`、`sanity_level`，第一阶段应“完整保存、按设置决定展示”，不要在 DTO 映射时静默丢掉成人内容字段。未知值采取保守策略，并在用户设置中提供明确的内容过滤选项。官方 Google Play 页面将 pixiv 标为 Mature 17+，但这不能替代 App API 数值枚举的技术证据。

## 4. 分页 `next_url`

### 4.1 PixEz 当前行为

[`LightingStore`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/lighting/lighting_store.dart#L117-L177) 把响应的 `next_url` 原样存入状态，下一页再交给 `apiClient.getNext()`。[`getNext()`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/network/api_client.dart#L279-L289) 对字符串做 host 文本替换后直接发 GET。

由此可以确认 PixEz 预期 `next_url` 是可直接请求的 URL，而不仅是一个整数 offset。源码附近示例注释展示了 `https://app-api.pixiv.net/...&offset=30` 的形态；推荐端点还可能返回其他游标参数。由于本次没有真实登录响应，**不能把 `offset=30` 写死为推荐分页的唯一结构**。

### 4.2 本项目的安全边界

Rust 接收服务端 `next_url` 后应至少检查：

```text
scheme      == https
host        == app-api.pixiv.net（精确匹配，不允许后缀欺骗）
path        == /v1/illust/recommended
port        == 未显式指定（或实现统一限定有效端口 443）
username    == 空
password    == 无
fragment    == 无
serialized length <= 4096 bytes
```

此外：

- HTTP client 禁止自动跟随重定向；若未来需要跟随，逐跳重复同样的 origin 校验；
- 不把 raw `next_url` 暴露给 Svelte；返回一个 Rust 生成的 opaque cursor；
- 最稳妥的是把验证后的 URL 存在 Rust 会话的 cursor 表，只给前端随机 ID。Base64 只能编码，不能防篡改；若不存表，至少给 cursor 加 MAC 或重新做完整 URL 校验；
- 验证 origin/path 后可把 query 当作服务端 opaque 数据保留，以兼容新增游标键；但应限制总长度和同名参数数量，避免资源滥用；
- 一旦 host、path 或 URL 语法异常，把整页标为 `invalid_response`，不要退化为任意 URL fetch。

这些校验在标准/ECH 模式是纵深防御，在兼容模式则是必要边界：兼容模式关闭证书验证后，中间人能够构造恶意 `next_url`。

## 5. pximg 缩略图加载

### 5.1 源码与实时行为

PixEz 的图片组件把所有图片请求交给 `CachedNetworkImage` 并注入 [`Hoster.header()`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/er/hoster.dart#L139-L145)；实际接线见 [`PixivImage`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/component/pixiv_image.dart#L222-L237)。当前固定值为：

```http
Referer: https://app-api.pixiv.net/
User-Agent: PixivIOSApp/5.8.0
```

Pixiv 官方帮助中心也要求浏览器不要禁用 Referer，见[“pixiv 页面可能会因为某些原因出现以下等问题”](https://www.pixiv.help/hc/zh-cn/articles/7026475470105)。

2026-08-03 实时探测从第一方 walkthrough JSON 读取同一个 `i.pximg.net` medium URL，发 `Range: bytes=0-0`：

| 请求 | 结果 |
|---|---|
| 有 User-Agent、无 Referer | `403` |
| 同样请求，加 `Referer: https://app-api.pixiv.net/` | `206` |

受中国大陆网络条件限制，这两次图片请求使用了项目已明确标为低安全的兼容链路（固定 IP、关闭 SNI/证书验证），所以状态差异不能表述为“经过已验证 TLS 的密码学证据”。它与 PixEz 注入 header 的源码、Pixiv 官方 Referer 提示一致，可作为实现决策依据；标准 TLS 路径应在可访问网络上再回归一次。

### 5.2 对 Tauri/WebView 的影响

浏览器页面不能可靠地为普通 `<img>` 自定义 `Referer`；`referrerpolicy="no-referrer"` 反而会落入已观察的 403 路径。Tauri 页面自身的来源也不是 `https://app-api.pixiv.net/`。

建议实现独立 Rust 媒体层：

1. 只接受经过校验的 `https://i.pximg.net/...` 或 `https://s.pximg.net/...`；
2. 使用当前选择的标准/ECH/兼容媒体 transport；
3. 加 `Referer: https://app-api.pixiv.net/` 和统一 User-Agent；
4. **不添加 Authorization**，防止 access token 泄漏给 CDN；
5. 禁止重定向到非白名单 host，限制响应大小、连接时间和 `Content-Type`；
6. 写入应用 cache 后经受控 asset protocol/文件 URL 交给 WebView，或实现流式自定义 protocol；
7. cache key 使用已验证 URL 的摘要，避免远端路径参与本地文件路径拼接。

## 6. Access token 过期与错误形态

### 6.1 实时已观察

2026-08-03，以一个故意无效、非真实的 Bearer 值请求推荐端点，第一方 API 返回：

```text
HTTP 400
```

响应结构为：

```json
{
  "error": {
    "user_message": "",
    "message": "Error occurred at the OAuth process. Please check your Access Token to fix this. Error Message: invalid_grant",
    "reason": "",
    "user_message_details": {}
  }
}
```

PixEz 对这一结构的模型见 [`ErrorMessage`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/models/error_message.dart#L20-L51)。它在 [`RefreshTokenInterceptor.onError`](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/network/refresh_token_interceptor.dart#L64-L140) 中检查 `statusCode == 400`，再以 `error.message` 是否含 `OAuth` 决定刷新并重试。

### 6.2 推荐的刷新状态机

不要照搬“所有 400 都刷新”。更稳妥的判定和流程是：

1. 请求前若本地 `expires_at` 距当前不足 60 秒，先刷新；
2. 响应为 400 或 401 时尝试解析 `error` 对象；
3. 只有明确出现 OAuth/`invalid_grant` 身份失败时才走 refresh；参数错误、限流与普通 400 原样上报；
4. 同一批并发请求只允许一个刷新任务，其他请求等待结果；
5. 成功后原请求最多重试一次；用新的 access token 更新 Rust 内存，若 refresh token 轮换则原子更新安全存储；
6. 第二次仍失败，终止重试并把 session 标为需要重新登录，避免死循环；
7. 日志只记录结构化分类和 HTTP 状态，不记录 Authorization、完整响应头或 token endpoint body。

当前已观察到的是“故意无效 token”的形态；真实 token 自然过期、撤销、账号风控或 API 将来改成 401 时可能略有差异，所以实现必须兼容 400 与 401，但不能仅按字符串做无限重试。

## 7. 第一阶段落地清单

为了以最小闭环接入登录后数据，建议顺序如下：

1. Rust `PixivApiClient`：推荐首屏、稳定 DTO、严格 URL 校验；
2. Session 层：只向 API client 临时借用内存 access token，不经过 Tauri IPC；
3. 过期前刷新 + 400 OAuth/`invalid_grant` 单次恢复；
4. Rust 媒体下载/cache 层，带正确 Referer；
5. 首页加载态、空态、登录失效态、重试按钮；
6. 用户真实登录后的四项手动验收：首屏、下一页、token 刷新、三种连接模式下的缩略图；
7. 以脱敏 fixture 固化响应解析测试，不把任何真实 token 或用户私有推荐内容提交到仓库。

当前最需要避免的三个错误是：把旧示例参数写成“当前 PixEz 参数”、只处理 HTTP 401、以及让 WebView 直接加载 pximg URL。
