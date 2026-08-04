# GitHub 上 Pixiv 接入实现调研

调研日期：2026-08-01

## 范围与结论

本次只研究个人使用、开源、侧载客户端所需的接入方式，不把现有项目中公开的令牌、客户端密钥或签名常量复制到本文或未来代码中。

当前最可行的技术路线是：

1. 使用 Flutter 编写共享 UI 和应用核心。
2. 把未公开的 Pixiv App API 封装在单独的 `PixivSource` adapter 中，不让界面和本地数据库依赖远端 JSON。
3. 第一阶段允许用户导入 refresh token；第二阶段再为 Android、Windows、Linux 分别实现交互式 PKCE 登录。
4. Web AJAX/Cookie 只能作为有限回退 adapter，不与 App API 细节混在应用核心中。
5. 不复制现有项目中的官方客户端凭据，不记录 OAuth 请求体、响应体或 Authorization 请求头。

这里的 App API、Web AJAX 接口和登录方法均不是 Pixiv 面向普通第三方开发者公开承诺的稳定接口，随时可能失效。

## 主要参考项目

| 项目 | 值得研究的部分 | 不应照搬的部分 | 许可证 |
|---|---|---|---|
| [PixEz Flutter](https://github.com/Notsfsssf/pixez-flutter) | Flutter 产品结构、App API 调用、PKCE 登录、Windows/Android 平台接入、Ugoira UI | 全局网络对象、逐端点膨胀的客户端、硬编码上游凭据、可能记录 OAuth 响应的调试日志 | [GPL-3.0](https://github.com/Notsfsssf/pixez-flutter/blob/master/LICENSE) |
| [PixivPy](https://github.com/upbit/pixivpy) | App API 的接口族、响应结构、分页参数、refresh token 登录 | 把其 Python 类直接翻译成同样庞大的 Dart 类 | [Unlicense](https://github.com/upbit/pixivpy/blob/master/LICENSE) |
| [gallery-dl](https://github.com/mikf/gallery-dl) | 仍在维护的 Pixiv extractor、分页、限流等待、图片和 Ugoira 回退、Cookie/AJAX 补充路径 | 下载器特有的批量抓取模型、GPL 源码直接复制 | [GPL-2.0](https://github.com/mikf/gallery-dl/blob/master/LICENSE) |
| [Pixiv-Shaft](https://github.com/CeuiLiSA/Pixiv-Shaft) | Android 下载队列、Room/MMKV 本地存储、图片浏览、功能范围 | Android 专属 UI/网络实现不能作为三端共享核心 | [MIT](https://github.com/CeuiLiSA/Pixiv-Shaft/blob/classic/LICENSE) |
| [pixiv-viewer-app](https://github.com/asadahimeka/pixiv-viewer-app) | Windows/Linux/Android 分发、RefreshToken/OAuth/Cookie 多种登录入口、自定义 source 的产品思路 | 依赖第三方代理 API 作为默认基础设施 | [MIT](https://github.com/asadahimeka/pixiv-viewer-app/blob/main/LICENSE) |

如果项目希望直接复用 PixEz 或 gallery-dl 的实现代码，应先选择兼容的 GPL 许可证并保留版权信息。若希望使用 MIT、Apache-2.0 等宽松许可证，则只研究外部行为和接口形状，独立实现代码。

## 已观察到的接入机制

### 1. OAuth 与会话

PixEz 的当前代码展示了以下流程：

- 生成 PKCE `code_verifier` 和 S256 `code_challenge`；
- 在 Pixiv 登录页面完成登录；
- 拦截固定的 HTTPS callback 中的 authorization code；
- 用 authorization code 换取 access token 和 refresh token；
- 后续用 refresh token 获取新的短期 access token。

来源：[PixEz OAuthClient](https://github.com/Notsfsssf/pixez-flutter/blob/master/lib/network/oauth_client.dart)、[Pixiv OAuth flow 参考实现](https://gist.github.com/ZipFile/c9ebedb224406f4f11845ab700124362)

PixivPy 已移除账号密码登录，README 明确要求 refresh token；这说明客户端不应收集或保存 Pixiv 密码。来源：[PixivPy README](https://github.com/upbit/pixivpy#pixivpy3)

架构要求：

- `SessionManager` 独占 token，其他模块只能请求“已认证的 HTTP 操作”，不能读取 token 字符串；
- access token 只保存在内存，refresh token 进入系统安全存储；
- 并发请求遇到会话失效时只允许一个 refresh 操作，其余请求等待同一个 future；
- refresh 成功后原子替换可能轮换的 refresh token；
- refresh 失败转成 `ReauthenticationRequired`，不能无限重试；
- OAuth host、Authorization、Cookie、请求体和响应体全部经过日志脱敏。

### 2. App API

现有实现主要访问 `app-api.pixiv.net`，使用 Bearer access token，并附带模拟官方移动客户端环境的请求头。PixEz 的 `ApiClient` 和 gallery-dl 的 `PixivAppAPI` 都显示了作品详情、推荐、排行榜、搜索、收藏、关注、评论、小说及 Ugoira metadata 等接口族。

来源：[PixEz ApiClient](https://github.com/Notsfsssf/pixez-flutter/blob/master/lib/network/api_client.dart)、[gallery-dl PixivAppAPI](https://github.com/mikf/gallery-dl/blob/master/gallery_dl/extractor/pixiv.py#L1024)

这些端点不应原样暴露给 UI。建议把调用收敛为应用概念：

```dart
abstract interface class PixivSource {
  Future<Page<ArtworkSummary>> list(
    ArtworkCollection collection,
    PageCursor? cursor,
  );

  Future<Artwork> artwork(ArtworkId id);

  Future<SearchPage> search(
    SearchQuery query,
    PageCursor? cursor,
  );

  Future<MutationReceipt> apply(AccountAction action);
}
```

远端 endpoint、query 参数、请求头、版本号及 JSON DTO 全部留在 adapter 内部。

### 3. 分页和限流

App API 的列表响应通常返回 `next_url`。gallery-dl 只解析其中 query，并继续调用同一个 endpoint；应用核心应该把它保存为不透明的 `PageCursor`，而不是让 UI 拼接 offset。来源：[gallery-dl 分页实现](https://github.com/mikf/gallery-dl/blob/master/gallery_dl/extractor/pixiv.py#L1221)

gallery-dl 在识别到 rate limit 后会长时间等待。交互式客户端更适合：

- API GET 并发上限初始设为 4；
- 图片 CDN 使用独立的并发池，初始设为 6；
- 写操作串行或低并发；
- 仅对 GET、HEAD 等幂等操作自动重试；
- 429 和 5xx 使用带 jitter 的指数退避，并优先遵守 `Retry-After`；
- 后台批量下载允许暂停，不用前台请求抢占所有额度。

具体并发数必须通过真实账号的小流量测试校准，不能假定为 Pixiv 官方限额。

### 4. 图片与 Ugoira

作品 JSON 可能通过 `meta_single_page` 或 `meta_pages` 提供单页和多页图片。Ugoira 则由 ZIP 帧文件和每帧 delay metadata 组成。gallery-dl 同时实现了 App API metadata 和 Web AJAX metadata 的回退路径。来源：[gallery-dl 图片提取](https://github.com/mikf/gallery-dl/blob/master/gallery_dl/extractor/pixiv.py#L102)、[gallery-dl Ugoira 提取](https://github.com/mikf/gallery-dl/blob/master/gallery_dl/extractor/pixiv.py#L167)

建议单独建立 `MediaPipeline`：

- 为缩略图、预览图、原图设置不同缓存 key；
- 图片下载统一设置正确 Referer，界面不直接加载远端 URL；
- Ugoira ZIP 下载到文件缓存，不一次性把所有帧常驻内存；
- 播放时使用小型解码环形缓冲区，并按每帧 delay 调度；
- 导出 GIF/APNG/WebM 属于后续功能，不进入第一版播放核心；
- 下载文件保留作品 ID、页码和来源元数据，避免仅依赖可变标题作为文件名。

### 5. Web AJAX/Cookie 回退

gallery-dl 会在部分内容受限或 App API 数据不足时访问 `www.pixiv.net/ajax/...`，并在需要时使用 `PHPSESSID` Cookie。来源：[gallery-dl `_request_ajax`](https://github.com/mikf/gallery-dl/blob/master/gallery_dl/extractor/pixiv.py#L213)

这一方式的风险更高：Cookie 通常拥有完整网页登录权限，Web JSON 也更容易随页面改版。因此：

- 默认不要求用户粘贴 Cookie；
- Cookie 必须存入安全存储且禁止导出到日志；
- Web adapter 只实现明确缺失的读取能力；
- 收藏、关注、评论等写操作优先走 App API；
- 不使用未经用户明确选择的公共图片/API 代理。

## 推荐的第一版登录方案

### 阶段 A：验证核心

提供“导入 refresh token”入口，用于开发者和个人用户测试：

1. 用户在外部工具中完成 Pixiv 登录并取得 refresh token；
2. 客户端立即把它写入系统安全存储；
3. 输入框清空，界面和日志不再显示完整 token；
4. 客户端 refresh 后获取账号资料，并进行一个低风险读取请求；
5. 失败时区分无效 token、网络问题、限流和上游结构变化。

### 阶段 B：交互式登录

建立 `InteractiveLogin` seam，并提供平台 adapter：

- Android：受控 WebView/浏览器页，拦截固定 HTTPS callback；
- Windows：WebView2 adapter；
- Linux：WebKitGTK adapter；
- 所有平台保留“粘贴 callback URL/refresh token”的故障回退。

因为 redirect 地址不是项目自有域名，不能假定普通系统浏览器能通过自定义 scheme 自动返回客户端。该行为需要在三端分别做最小原型验证。

## 不能从参考项目直接继承的做法

1. 不提交任何来自官方客户端的 client secret、hash secret 或真实 token。
2. 不在 Debug 模式记录 OAuth 响应体；PixEz 当前 `LogInterceptor` 的配置说明这是一个需要主动规避的风险。来源：[PixEz OAuthClient](https://github.com/Notsfsssf/pixez-flutter/blob/master/lib/network/oauth_client.dart)
3. 不把每个远端 endpoint 做成供页面直接调用的方法；这会把未公开接口的变化扩散到整个应用。
4. 不通过关闭 TLS 验证实现“直连”。网络模式只能是系统网络、用户配置的 HTTP/SOCKS 代理，或保持 hostname/SNI 和证书校验的自定义 DNS。
5. 不默认使用第三方公共代理保存、转换或转发用户访问的作品。
6. 不自动批量抓取整个关注列表或作者图库；下载队列必须可见、可暂停且有限流。

## 建议的验证顺序

1. `SessionManager`：导入 refresh token、刷新、日志脱敏、并发 single-flight。
2. `PixivSource.artwork(id)`：作品详情和单图/多图映射。
3. `PixivSource.list(...)`：推荐或排行榜的一页及 `next_url` cursor。
4. `MediaPipeline`：缩略图、原图和磁盘缓存。
5. Ugoira metadata、ZIP 下载和基础播放。
6. 搜索、收藏与关注写操作。
7. 三个平台的交互式登录 adapter。
8. 下载队列、历史和本地资料库。

## 尚未验证

- 2026-08-01 当天实际账号登录是否仍对所有地区和账号类型工作；本次未使用任何用户凭据做在线调用。
- Pixiv 是否会在未来更换 App API 请求签名、客户端版本要求或 refresh token 策略。
- Linux WebKitGTK 对登录页面、第三方账号登录、passkey 和二步验证的兼容性。
- 不同网络环境下图片 CDN 的 Referer、HTTP/2、代理和 DNS 行为。
- 大规模下载的实际限流阈值；项目不应通过压测 Pixiv 来推断该阈值。
