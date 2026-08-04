# PixEz 评论、回复、表情与站内通知实现研究

研究日期：2026-08-04
基准源码：[Notsfsssf/pixez-flutter @ `6388dd88d40315d6de1b610cae7e1b48ea80d221`](https://github.com/Notsfsssf/pixez-flutter/tree/6388dd88d40315d6de1b610cae7e1b48ea80d221)

## 结论摘要

| 功能 | PixEz 当前实现状态 | 核心结论 |
| --- | --- | --- |
| 评论删除 | **未实现（已确认）** | API 客户端没有删除评论方法；评论“更多”菜单只有本地屏蔽和举报。不能从 PixEz 得出一个可靠的删除 endpoint。 |
| 更多回复/回复列表 | **已实现（已确认）** | 插画与小说分别调用 `/v2/illust/comment/replies`、`/v2/novel/comment/replies`，以 `comment_id` 查询，并沿响应的 `next_url` 分页。 |
| 回复发表 | **已实现（已确认）** | 仍调用评论新增接口，并附带 `parent_comment_id`；回复页会把根评论 ID 作为该字段。 |
| Pixiv 站内通知 | **未实现（已确认到当前提交）** | 没有通知 endpoint、响应模型、Store 或页面入口。Android 的通知权限和 Flutter 的 `NotificationListener` 不能证明存在 Pixiv 站内通知。 |
| 评论表情 | **已实现两种展示（已确认）** | 独立 `stamp` 使用响应里的 `stamp_url`；正文内 `(normal)` 等 token 由客户端映射成本地小图。发表时 PixEz 只提交 `comment` 文本，并不提交 `stamp_id`。 |

> 范围说明：本文只陈述在上述提交中能从一手源码确认的行为。Pixiv App API 属于未公开、可能变动的接口；对于 PixEz 没有实现的删除与站内通知，不把网络上的猜测 endpoint 写成事实。

## 公共鉴权与请求环境

`ApiClient` 的基址是 `https://app-api.pixiv.net`，并设置 Pixiv Android 客户端相关请求头。所有普通 App API 请求通过 `RefreshTokenInterceptor` 注入 `Authorization: Bearer <access_token>`；没有令牌时请求会被拒绝。因此本文中的评论读取、回复读取和发表评论都需要有效登录会话。[`api_client.dart` L33-L125](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/network/api_client.dart#L33-L125) [`refresh_token_interceptor.dart` L25-L52](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/network/refresh_token_interceptor.dart#L25-L52)

PixEz 遇到带 OAuth 信息的 `400` 响应时会尝试刷新令牌并重放请求。这是客户端的通用令牌恢复机制，并不是评论模块特有行为。[`refresh_token_interceptor.dart` L69-L139](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/network/refresh_token_interceptor.dart#L69-L139)

## 1. 评论删除

### 已确认：PixEz 没有实现删除

在 `ApiClient` 的评论区域，现有方法只有：

- 获取插画评论：`GET /v3/illust/comments?illust_id=...`
- 获取小说评论：`GET /v3/novel/comments?novel_id=...`
- 获取插画/小说回复
- 新增插画/小说评论或回复

该区域没有 `deleteComment` 方法或评论删除路径。[`api_client.dart` L553-L622](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/network/api_client.dart#L553-L622)

评论行右侧“更多”菜单也只有：

1. `muteStore.insertComment(comment)`：本地屏蔽；
2. `Reporter.show(...)`：举报，并可在完成后本地屏蔽。

没有根据当前用户 ID 显示的删除动作，也没有删除后的列表更新逻辑。[`comment_page.dart` L484-L535](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/page/comment/comment_page.dart#L484-L535)

### endpoint、参数、响应与权限

| 项目 | PixEz 可确认内容 |
| --- | --- |
| endpoint | 无；PixEz 当前没有发起删除请求 |
| 参数 | 无 |
| 响应模型 | 无 |
| 权限判断 | 无；UI 也未判断“评论作者是否为当前账号” |

因此，PixNya **不能以“照搬 PixEz”方式实现删除**。实施前应先用受控账号验证当前官方客户端的实际请求；确认 endpoint、HTTP 方法、表单字段、成功响应和错误码后再编码。UI 至少只应对 `comment.user.id == 当前账号 ID` 的评论显示删除，并让服务端继续做最终授权；删除成功后按 ID 从当前列表移除，`403/404` 则回滚并刷新。不要把尚未抓包确认的路径写死为已支持 API。

## 2. 评论列表、更多回复与分页

### 请求接口

| 场景 | HTTP | endpoint | 参数 |
| --- | --- | --- | --- |
| 插画顶层评论 | GET | `/v3/illust/comments` | query `illust_id` |
| 小说顶层评论 | GET | `/v3/novel/comments` | query `novel_id` |
| 插画某评论的回复 | GET | `/v2/illust/comment/replies` | query `comment_id` |
| 小说某评论的回复 | GET | `/v2/novel/comment/replies` | query `comment_id` |

对应实现位于 `ApiClient.getIllustComments`、`getNovelComments`、`getIllustCommentsReplies`、`getNovelCommentsReplies`。[`api_client.dart` L553-L591](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/network/api_client.dart#L553-L591)

### 响应模型

`CommentResponse` 包含：

- `total_comments: int?`
- `comments: Comment[]`
- `next_url: string?`

`Comment` 包含 `id`、`comment`、`date`、`user`、`parent_comment`、`has_replies`、`stamp`；`User` 带 ID、昵称、账号和头像 URL；`Stamp` 带 `stamp_id` 与 `stamp_url`。[`comment_response.dart` L26-L93](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/models/comment_response.dart#L26-L93)

### 状态与分页

`CommentStore.fetch()` 根据 `isReplay` 和作品类型选择顶层评论或回复接口，解析 `CommentResponse` 后替换列表，并保存 `next_url`。`next()` 在 `next_url` 非空时调用通用 `ApiClient.getNext(nextUrl)`，追加新页；没有 `next_url` 时结束加载。[`comment_store.dart` L71-L127](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/page/comment/comment_store.dart#L71-L127) [`api_client.dart` L279-L290](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/network/api_client.dart#L279-L290)

当条目的 `has_replies == true` 时，PixEz 显示“查看回复”，再打开一个 `CommentPage(isReplay: true, pId: comment.id)`；因此回复列表是独立页面，不会内联展开。[`comment_page.dart` L290-L316](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/page/comment/comment_page.dart#L290-L316)

### 发表回复

| 场景 | HTTP | endpoint | form 参数 |
| --- | --- | --- | --- |
| 插画评论/回复 | POST | `/v1/illust/comment/add` | `illust_id`, `comment`, 可选 `parent_comment_id` |
| 小说评论/回复 | POST | `/v1/novel/comment/add` | `novel_id`, `comment`, 可选 `parent_comment_id` |

请求体是 `application/x-www-form-urlencoded`。[`api_client.dart` L593-L622](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/network/api_client.dart#L593-L622)

在顶层页点“回复”会把所选评论 ID 写入 `parentCommentId`；进入回复页时则默认把根评论 `pId` 作为 `parent_comment_id`。发送成功后清空输入框并整体刷新当前评论页。输入框在客户端限制为 140 字符。[`comment_page.dart` L397-L445](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/page/comment/comment_page.dart#L397-L445) [`comment_page.dart` L484-L500](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/page/comment/comment_page.dart#L484-L500)

### PixEz 的管理限制

- 回复页不会继续显示“回复”动作，避免前端形成任意深度的嵌套回复。
- 回复页也不显示顶层评论的“更多”菜单，因此对回复项没有单独的本地屏蔽/举报入口。
- PixEz 只用 `next_url` 做游标分页，没有页码，也没有回复数量字段；是否有回复只看 `has_replies`。
- 发送后是整页 `fetch()`，不是乐观插入；实现简单但网络较慢时视觉反馈较重。

### PixNya 建议

保留同一份 `CommentPageState`：`items + nextUrl + loading + error + rootCommentId`，顶层评论和回复列表只替换 endpoint。分页必须把服务器返回的 `next_url` 当作不透明游标，不自行拼接 offset。发送回复成功后可先乐观插入，再后台刷新校正；返回栈应保存原列表、滚动位置和已加载游标，避免从回复页返回时重新加载顶层评论。

## 3. Pixiv 站内通知

### 结论：当前 PixEz 没有实现

以下证据共同排除了“PixEz 已实现 Pixiv 站内通知”的判断：

1. `ApiClient` 的全部 App API 路由中没有 notification/notifications 路径，也没有获取、已读、未读数等方法；评论区域之后即进入动图与收藏标签接口。[`api_client.dart` L553-L645](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/network/api_client.dart#L553-L645)
2. Android 主页面实际注册的五个页面是推荐、排行、新作、搜索、设置，没有通知页面。[`android_hello_page.dart` L294-L315](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/page/hello/android_hello_page.dart#L294-L315)
3. Fluent/桌面导航同样只有首页、排行、快捷查看及设置等入口，没有通知 Store 或页面。[`fluent_hello_page.dart` L55-L132](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/fluent/page/hello/fluent_hello_page.dart#L55-L132)
4. `pubspec.yaml` 没有 Firebase Messaging、OneSignal 或 `flutter_local_notifications` 等通知实现依赖。[`pubspec.yaml` L18-L75](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/pubspec.yaml#L18-L75)

Android manifest 的确声明了 `POST_NOTIFICATIONS`，但这只能说明应用申请过系统通知权限，不能证明存在 Pixiv 站内通知数据接口。[`AndroidManifest.xml` L1-L15](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/android/app/src/main/AndroidManifest.xml#L1-L15) 源码中的 `NotificationListener<ScrollNotification>` 是 Flutter 滚动事件监听器，也不是站内通知。

因此该功能在 PixEz 中没有可复用的 endpoint、参数或响应模型。PixNya 若要加入站内通知，应作为独立逆向验证任务：分别确认列表、未读数、标记已读、通知类型到作品/用户/评论的路由，以及分页游标。验证前不要把 Android 本地推送权限或系统通知 UI 与 Pixiv 站内通知混为一谈。

## 4. 评论中的表情与 stamp

PixEz 实际处理两种不同形态。

### A. 独立 stamp 对象

当评论响应的 `stamp` 不为空时，页面直接读取 `comment.stamp.stamp_url`，通过统一的 `PixivImage` 组件显示为 100×100 图片；`stamp_id` 只被模型保存，没有参与发表请求。[`comment_page.dart` L278-L289](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/page/comment/comment_page.dart#L278-L289) [`comment_response.dart` L85-L93](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/models/comment_response.dart#L85-L93)

### B. 正文 token 表情

`emojisMap` 把 `(normal)`、`(surprise)`、`(heart)` 等字符串映射到 `assets/emojis/*.png`。输入面板展示这些本地图片；点击后只是把对应 token 插入文本光标位置。发表时 token 随普通 `comment` 字段提交，并没有额外的 emoji/stamp 参数。[`comment_store.dart` L29-L68](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/page/comment/comment_store.dart#L29-L68) [`comment_page.dart` L102-L138](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/page/comment/comment_page.dart#L102-L138)

显示正文时，`CommentEmojiText` 顺序扫描括号内容：命中 `emojisMap` 就插入 20×20 的本地图片，否则保留原始文字。[`comment_emoji_text.dart` L29-L75](https://github.com/Notsfsssf/pixez-flutter/blob/6388dd88d40315d6de1b610cae7e1b48ea80d221/lib/component/comment_emoji_text.dart#L29-L75)

### PixNya 建议

1. 数据模型同时保留 `commentText` 和可选 `stamp { id, url }`，不要把二者合并。
2. 对 `stamp_url` 复用现有 Pixiv 图片代理、鉴权头与磁盘缓存；加载失败时显示占位，不应让整条评论失败。
3. 正文 token 用一次线性 tokenizer 转成文本/emoji segment；未知 token 原样显示，避免吞字。
4. 发表第一阶段只实现 PixEz 已验证的“token 写入 `comment`”；**不要假设发送 `stamp_id` 可用**。独立 stamp 的发表协议需要另行抓包确认。
5. 不直接复制 PixEz 的表情图片资源。`pixez-flutter` 整体采用 GPL-3.0，图片本身还可能涉及 Pixiv 的素材权利；PixNya 可以复用协议与状态机思路，但应自行确认素材授权或仅使用服务端返回的 `stamp_url`。

## 推荐实施顺序

1. 完成评论/回复统一模型与 `next_url` 游标分页。
2. 加入回复独立页、返回位置恢复和发送后的局部更新。
3. 加入正文 token 表情解析，再接收独立 `stamp_url` 展示。
4. 对评论删除先做请求验证；确认协议后再开放仅本人可见的删除入口。
5. 将 Pixiv 站内通知列为独立研究项；当前不能从 PixEz 源码复用。
