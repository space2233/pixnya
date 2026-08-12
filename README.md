# PixNya

一个面向 Windows、Linux 和 Android 的非官方、开源、侧载 Pixiv 客户端。

## 中文下载说明

请前往 [PixNya v1.2.0 正式版](https://github.com/space2233/pixnya/releases/tag/v1.2.0)，按设备选择一个安装包：

- Windows 64 位：`PixNya_1.2.0_x64-setup.exe`
- Linux 64 位：`PixNya_1.2.0_amd64.AppImage`
- Android 10 及以上、ARM64：`pixnya-1.2.0-android-arm64-v8a.apk`

Release 中的 JSON 和签名文件供自动更新使用，普通用户无须下载。

当前公开稳定版为 `1.2.0`；当前源码版本 `1.2.0`。

## 项目性质

PixNya 是个人维护的独立项目，与 pixiv Inc. 没有隶属、授权、认可或技术支持关系。

项目不托管 Pixiv 内容，也不绕过登录、付费、年龄、地区或账号权限。用户应使用自己的账号，并自行遵守所在地法律、Pixiv 使用条款和作品版权要求。Pixiv 名称、标志和站内作品归各自权利人所有；本仓库的 GPL-3.0-only 许可证只覆盖 PixNya 自身代码。

隐私数据、本地存储和清除边界见 [隐私说明](PRIVACY.md)。安全问题请按 [安全政策](SECURITY.md) 私密报告，不要在公开 Issue 中粘贴令牌、Cookie 或登录截图。

## 主要功能

- 插画、漫画、小说、作者、排行榜、发现、关注、收藏与搜索。
- 多图作品查看、作品与小说系列页面及连续浏览。
- 插画和小说评论、回复、官方贴图及本人评论删除。
- 只读站内通知与安全的应用内资源跳转。
- Ugoira 播放，以及 GIF、APNG 和 WebM 后台导出。
- SQLite 下载队列、离线资料库、浏览历史和阅读进度。
- 本地收藏夹、标签、组合筛选、批量整理与重复内容报告。
- 账号屏蔽、用户静音和标签静音管理。
- Windows、Linux 和 Android 的自动更新支持。

投稿、个人资料编辑、通知写操作、私信、直播和多账号不在当前功能范围内。

## 支持平台

- Windows x64
- Linux x64
- Android ARM64，Android 10 / API 29 及以上

ARMv7 只保留手动兼容性调试入口，不提供正式 Release。

## 界面语言

PixNya 支持简体中文、繁體中文和 English。默认跟随操作系统，也可以在应用设置中固定语言。

Pixiv 返回的作品标题、作者名、标签、评论和小说正文保持原文，客户端不会自动翻译。语言资源由 Paraglide JS 根据 `messages/` 中的词典生成。

## 本地开发

安装依赖并启动开发环境：

```powershell
npm install
npm run check
npm run tauri dev
```

构建 Windows 调试客户端：

```powershell
npm run build:desktop:debug
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-windows-standalone.ps1
```

构建 Android ARM64 调试 APK：

```powershell
npm run build:android:arm64:debug
```

运行完整验证：

```powershell
npm run test:full
```

对外测试产物统一保存到：

- `artifacts\windows`：Windows EXE
- `artifacts\android`：Android APK
- `artifacts\SHA256SUMS.txt`：校验值

Linux 由 `scripts/check-linux.sh` 和 GitHub Actions 在 Ubuntu 22.04 上执行验证与构建。

## 许可证

项目代码按 [GNU GPL-3.0-only](LICENSE) 发布。第三方依赖清单、SPDX SBOM、离线许可证归档和发布要求见 [许可证与软件物料清单](docs/SUPPLY_CHAIN.md)。
