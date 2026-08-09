# PixNya 测试流程

PixNya 使用分层测试入口。目标是让日常修改尽快得到反馈，同时在提交或发布前保留完整的 Rust 和平台覆盖。

## 1. 快速层

```powershell
npm run test:quick
```

它会依次执行：

- `scripts/` 中全部 `*.test.mjs` 回归测试。
- Svelte 类型、可访问性和模板诊断；

Node 回归测试本身很快，因此快速层始终运行全部测试，不按改动文件猜测测试集合。两者不并行，是因为源码边界测试和 Svelte 检查同时扫描项目会在 Windows 上造成磁盘争用；实测顺序执行约 6 秒，并行反而约 20 秒。日常界面、路由、脚本和配置修改后优先运行这一层。

## 2. Rust 层

```powershell
npm run test:rust
```

它依次执行：

1. `cargo fmt --all -- --check`；
2. `cargo clippy --workspace --all-targets -- -D warnings`；
3. `cargo test --workspace`。

该入口设置 `CARGO_INCREMENTAL=0`，继续复用 `deps/` 和 `build/`，但不会为一次性验证持续制造大型增量缓存。

## 3. 完整层

```powershell
npm run test:full
```

完整层先运行快速层，再运行 Rust 层。提交功能、修复 bug、准备推送或准备构建测试包前使用这一入口。

## 4. 平台构建验证

完整测试通过后，只构建本次真正需要的平台：

```powershell
# Windows x64 调试客户端
npm run build:desktop:debug

# Android ARM64 调试 APK
npm run build:android:arm64:debug
```

Linux 使用：

```bash
bash scripts/check-linux.sh
```

Linux GitHub Actions 会先执行快速层。只有快速层通过后才安装 WebKitGTK 等系统依赖，并继续 Rust 严格检查和真实 Tauri 桌面编译，从而让常见的前端或回归错误更早失败。

## 5. 建议使用时机

| 时机 | 最小检查 |
|---|---|
| 调整页面、样式、路由或普通脚本 | `npm run test:quick` |
| 修改 Rust、Tauri 命令、网络、存储或鉴权 | `npm run test:full` |
| 准备提交或推送 | `npm run test:full` |
| 生成给测试人员使用的文件 | 完整层通过后，仅构建目标平台 |
| 正式发布 | GitHub Release 工作流的 Windows、Linux、Android 签名构建 |

真实账号登录、三种连接模式、Android 系统安装器和自动更新仍属于设备/网络冒烟测试，不能由静态回归测试替代。
