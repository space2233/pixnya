# PixNya 第一次 GitHub 上传清单

> 仓库：`space2233/pixnya`
> 可见性：Private
> 发布候选：`v0.28.2`，GitHub Pre-release
> 状态：源码和附件已准备；尚未创建提交、推送、标签或 Release

## 上传边界

- Git 仓库只上传源码、锁文件、脚本、Android 定制源码、文档和 CI 配置。
- APK、EXE 和本次专用校验清单只上传到 GitHub Release，不写入 Git 历史。
- GitHub 标签让 GitHub 自动生成 Source code ZIP/TAR，因此不额外上传重复源码压缩包。
- `.env.oauth.local`、OAuth 参数、签名私钥、keystore、令牌、Cookie、`target/`、`node_modules/` 和 Gradle 输出不得上传。

## 已完成检查

- [x] GitHub CLI 已登录 `space2233`，权限包含 `repo` 和 `workflow`。
- [x] 远端 `space2233/pixnya` 是私人空仓库。
- [x] 首次源码清单约 2.3 MiB，不含大文件。
- [x] 高置信度私钥、GitHub token、OpenAI key、AWS key扫描无命中。
- [x] `.env.oauth.local`、二进制、依赖和构建缓存均被忽略。
- [x] 旧品牌文本已修复，版本按规则提升到 0.28.2。
- [x] updater 配置启动 panic 已复现、修复并加入回归测试。
- [x] Windows x64 EXE 和 Android ARM64 APK 已重新构建并验证。

## 待执行的外部写入

1. 将本地分支从 `master` 重命名为 `main`。
2. 复核 `git diff --cached` 后建立首个提交：`feat: publish initial PixNya source snapshot`。
3. 使用 GitHub CLI 配置 HTTPS Git 凭据，并把 `main` 推送到 `origin`。
4. 在私人仓库创建 `v0.28.2` 预发布，标题为 `PixNya v0.28.2 private test build`。
5. 使用 `docs/releases/v0.28.2.md` 作为 Release 正文。
6. 上传 `artifacts/github/v0.28.2/` 中的 EXE、APK 和 `SHA256SUMS.txt`。
7. 从 GitHub 重新下载两个附件并复核哈希，确认私有 Release 权限和附件完整。

## 不使用正式发布工作流的原因

`.github/workflows/release.yml` 用于正式签名发布，会要求 Windows/Linux Tauri updater 密钥、Android 长期 keystore 和 Android 清单签名密钥。第一次私人测试上传尚未配置这些长期凭据，因此本次只手工创建明确标记的 Debug Pre-release，不生成 `latest.json` 或 `android-latest.json`，也不会被稳定自动更新通道采用。

## 上传后检查

- [ ] 仓库默认分支是 `main`，源文件可在 GitHub 浏览。
- [ ] 仓库保持 Private。
- [ ] GitHub 的 Secret scanning/Push protection 保持启用（如私人仓库方案可用）。
- [ ] Release 显示 Pre-release，正文明确说明 Debug、未签名和 OAuth 参数可提取。
- [ ] Release 只有本次两个二进制附件和一个校验清单。
- [ ] GitHub 自动生成的源码压缩包来自同一个 `v0.28.2` 标签。
- [ ] 本地和远端提交 ID、标签目标一致。
