# PixNya `target/` 空间清理计划

> 审计日期：2026-08-04
> 当前授权范围：只处理 `F:\ACM\pixiv-client\target\`
> 当前状态：仅完成只读审计和计划，**尚未删除任何文件**
> 容量口径：文件逻辑大小之和，单位为 GiB/MiB

## 1. 本轮边界

本轮只把 Rust/Cargo/Tauri 的 `target/` 作为清理对象。以下目录即使可重建，也全部暂时保留：

- `artifacts/` 中的 APK、EXE 和 GitHub 上传准备目录；
- `node_modules/`、`.svelte-kit/` 和前端 `build/`；
- `src-tauri/gen/android/` 下的 Gradle 输出与 Android 定制源码；
- `backups/`、`.build-logs/` 及其余项目目录。

不执行 `cargo clean`，因为它会清空整个 `target/`，无法满足“需要复用的先保留”。也禁止使用 `git clean -fdx`。

## 2. 当前总量

`target/` 当前共 **138.464 GiB、241,707 个文件**：

| 路径 | 占用 | 文件数 | 主要用途 | 当前决定 |
|---|---:|---:|---|---|
| `target/debug/` | 116.013 GiB | 143,724 | Windows 调试构建、测试、依赖和增量缓存 | 细分保留与清理 |
| `target/aarch64-linux-android/` | 14.455 GiB | 64,638 | 当前 Android ARM64 构建缓存 | 细分保留与清理 |
| `target/armv7-linux-androideabi/` | 7.964 GiB | 32,634 | 已暂停的 Android ARM32 构建缓存 | 整体列入清理 |
| `target/webview-e2e-*` | 32.70 MiB | 694 | WebView 端到端测试的临时运行目录 | 整体列入清理 |
| `target/windows-standalone-runtime-*` | 0.08 MiB | 15 | 独立启动回归测试的临时运行目录 | 整体列入清理 |
| `target/tmp/` | 0 | 0 | Cargo 临时目录 | 空目录，可清理 |
| `.rustc_info.json`、`CACHEDIR.TAG` | 很小 | 2 | rustc 环境信息和缓存标记 | 保留 |

## 3. Windows `target/debug/` 细分

| 路径/文件 | 占用 | 文件数 | 是什么 | 复用判断 |
|---|---:|---:|---|---|
| `incremental/` | 69.350 GiB | 109,631 | rustc 为工作区 crate 保存的增量对象，共 1,909 个编译会话目录 | 可加速本地 crate 重编，但不是构建所必需 |
| `deps/` | 35.130 GiB | 9,127 | 已编译依赖、工作区库、测试程序及调试符号 | 当前依赖可直接复用，应优先保留 |
| `build/` | 9.235 GiB | 9,918 | 各依赖 `build.rs` 的输出，例如 Tauri、SQLite、TLS 库 | 当前配置可复用，应优先保留 |
| 根目录旧应用输出 | 1.963 GiB | 11 | 旧 `pixiv-client` EXE/PDB 和 `pixiv_client_lib` DLL/LIB/RLIB | 已被 PixNya 包名替代，可清理 |
| 当前 `pixnya.exe` + `pixnya.pdb` | 269.3 MiB | 2 | 当前 Windows 调试程序和符号 | 保留，便于直接运行和调试 |
| `examples/` | 69.0 MiB | 5 | `connectivity_probe` 示例程序及 PDB | 测试时可重建，列入清理 |
| `.fingerprint/` | 4.9 MiB | 15,026 | Cargo 判断产物是否新鲜的元数据 | 当前条目保留，只清旧应用精确条目 |
| `.cargo-*-lock` | 很小 | 3 | Cargo 构建并发锁 | 不主动处理；确认无构建进程后可自动消失或重建 |

`deps/` 的 35.130 GiB 主要不是“重复源码”，而是 Debug 构建产物：PDB 约 19.008 GiB、RLIB 约 9.798 GiB、LIB 约 2.421 GiB、测试/工具 EXE 约 1.783 GiB、RMETA 约 1.657 GiB。它们占用很大，但能避免第三方依赖和部分测试目标重新编译，因此本轮不整目录删除。

### 3.1 Windows 增量缓存按 crate 聚合

| crate | 会话目录数 | 占用 | 判断 |
|---|---:|---:|---|
| `pixiv_client_lib` | 168 | 37.900 GiB | 旧应用主库名，精确清理 |
| `pixnya_lib` | 43 | 15.400 GiB | 当前应用主库，复用优先，暂留 |
| `pixiv_client_network` | 163 | 6.676 GiB | 当前内部网络模块，暂留 |
| `pixiv_client_api` | 164 | 3.128 GiB | 当前内部 API 模块，暂留 |
| `pixiv_client_auth` | 165 | 1.893 GiB | 当前内部鉴权模块，暂留 |
| 其余工作区 crate、示例和构建脚本 | 1,206 | 4.353 GiB | 当前或测试配置缓存，暂留 |

这里必须区分：品牌和主应用 crate 已改为 `pixnya`/`pixnya_lib`，但 `pixiv-client-api`、`pixiv-client-network`、`pixiv-client-auth` 等仍是项目当前使用的内部包名。不能按文件名包含 `pixiv` 进行模糊删除，只能精确匹配已经消失的旧主应用 `pixiv-client` 和 `pixiv_client_lib`。

## 4. Android ARM64 细分

| 路径/文件 | 占用 | 文件数 | 是什么 | 复用判断 |
|---|---:|---:|---|---|
| `debug/incremental/` | 11.228 GiB | 51,865 | ARM64 工作区 crate 的 372 个增量编译会话 | 可加速本地 crate 重编，暂留当前名称部分 |
| `debug/deps/` | 2.489 GiB | 2,632 | 已编译的 ARM64 Rust 依赖 | 下一次 APK 构建可直接复用，保留当前部分 |
| `debug/build/` | 75.9 MiB | 5,923 | ARM64 构建脚本输出，主要包含 TLS 和 SQLite 原生库 | 保留当前部分 |
| 当前 `libpixnya_lib.a/.rlib/.so` | 342.8 MiB | 3 | 当前 ARM64 Rust 静态库、中间库和 JNI 动态库 | 保留 |
| 旧 `libpixiv_client_lib.*` | 333.7 MiB | 4 | 旧主应用库名的 ARM64 输出 | 可清理 |
| `.fingerprint/` | 2.0 MiB | 4,205 | ARM64 Cargo 新鲜度元数据 | 当前条目保留 |

ARM64 的增量缓存中，旧 `pixiv_client_lib` 占 **5.112 GiB（29 个会话目录）**；当前 `pixnya_lib` 占 **3.503 GiB（13 个会话目录）**；当前内部模块合计约 **2.613 GiB**。本轮复用优先方案只清旧主应用部分。

## 5. Android ARM32 细分

ARM32 已明确暂停支持，因此 `target/armv7-linux-androideabi/` 当前没有需要继续复用的构建状态，可以整体清理：

| 子目录/文件 | 占用 | 用途 |
|---|---:|---|
| `debug/incremental/` | 5.525 GiB | ARM32 增量编译缓存 |
| `debug/deps/` | 1.816 GiB | ARM32 依赖库和测试产物 |
| `debug/build/` | 25.1 MiB | ARM32 构建脚本输出 |
| `libpixnya_lib.*` | 309.9 MiB | 最近一次 ARM32 主库输出 |
| `libpixiv_client_lib.*` | 302.5 MiB | 旧名称 ARM32 主库输出 |
| `.fingerprint/` 及锁文件 | 约 1 MiB | Cargo 元数据 |

删除该三元组不会影响 ARM64 APK 和 Windows 构建。以后恢复 ARM32 时，Cargo 会重新生成它。

## 6. 可精确清理的旧主应用残留

旧包名曾是 `pixiv-client`，旧主库名曾是 `pixiv_client_lib`。源代码当前已经使用 `pixnya` 和 `pixnya_lib`，因此下列精确残留没有复用价值：

| 平台 | `incremental` | `deps` | `build` | 根输出 | 合计 |
|---|---:|---:|---:|---:|---:|
| Windows Debug | 37.900 GiB | 7.347 GiB | 5.229 GiB | 1.963 GiB | 52.440 GiB |
| Android ARM64 Debug | 5.112 GiB | 0.329 GiB | 0.006 GiB | 0.326 GiB | 5.773 GiB |
| 合计 | 43.012 GiB | 7.676 GiB | 5.235 GiB | 2.289 GiB | **58.213 GiB** |

Cargo 指纹中的旧主应用条目不足 2 MiB，也一并列入精确清理。实际执行时应先生成完整路径清单并复核，再对每个已解析的路径使用 `-LiteralPath`；不能使用 `*pixiv*` 之类宽泛通配符。

## 7. 两档执行方案

### 7.1 复用优先方案（本轮推荐）

清理：

1. 整个 `target/armv7-linux-androideabi/`：约 7.964 GiB；
2. Windows 和 ARM64 中旧主应用的精确残留：约 58.213 GiB；
3. `target/webview-e2e-*`、`windows-standalone-runtime-*`、空 `tmp/`：约 32.78 MiB；
4. `target/debug/examples/`：约 69.0 MiB。

预计共回收约 **66.28 GiB**，`target/` 从 138.464 GiB 降至约 **72.19 GiB**。当前 Windows/ARM64 的 `deps`、`build`、`.fingerprint`、PixNya 输出和当前名称增量缓存均保留，后续构建速度受影响最小。

### 7.2 平衡空间方案（需再次确认）

在复用优先方案基础上，再整体清理：

- `target/debug/incremental/`；
- `target/aarch64-linux-android/debug/incremental/`。

两个目录合计 80.578 GiB，其中 43.012 GiB 已包含在复用优先方案的旧主应用残留中，因此相对复用优先方案还可多回收约 37.57 GiB。总回收约 **103.84 GiB**，最终保留约 **34.62 GiB**。

该方案仍保留当前 Windows/ARM64 的第三方依赖、构建脚本输出和 Cargo 指纹；下一次会重编 PixNya 与工作区本地 crate，但通常不必从头编译所有第三方依赖。它比完整 `cargo clean` 更符合“兼顾复用”的要求。

### 7.3 不采用：完整清空 `target/`

完整删除可立即回收 138.464 GiB，而且不会丢源码，但下一次 Windows 检查和 ARM64 APK 构建都要从零开始。当前明确要求优先保留可复用内容，因此不列为本轮执行项。

## 8. 执行步骤（待用户确认后再做）

- [ ] 确认 Cargo、Tauri、Gradle、Android Studio 和 PixNya 测试程序均已关闭。
- [ ] 重新统计目标路径，防止构建期间数字和目录结构发生变化。
- [ ] 生成“将删除的绝对路径 + 大小”清单，不直接从模糊通配符执行删除。
- [ ] 先执行复用优先方案，不触碰 `target/` 外的任何路径。
- [ ] 再次统计 `target/`，核对预计回收量和保留目录。
- [ ] 确认 `target/debug/pixnya.exe` 仍存在并可独立启动。
- [ ] 运行 `cargo check --workspace`，确认保留的 Windows 依赖可正常复用。
- [ ] ARM64 库留到下一次真实 APK 构建时验证，避免仅为验证重新制造大量缓存。
- [ ] 是否执行平衡空间方案，等待第二次明确确认。

## 9. 后续防止再次膨胀

1. 发布/验证构建使用 `CARGO_INCREMENTAL=0`，日常开发构建仍可保留增量编译。
2. ARM32 暂停期间，构建脚本不再生成 `armv7-linux-androideabi`。
3. 品牌或 crate 重命名后，立即精确清理旧主 crate 的缓存，不让旧名称继续占用几十 GiB。
4. 增加只读容量审计脚本，分别报告 Windows、ARM64、ARM32、`incremental`、`deps` 和 `build`；删除仍需人工确认。
5. 当 Windows + ARM64 的 `incremental` 再次超过约 40 GiB 时，提示使用“平衡空间方案”，而不是无条件每次构建后清空。

## 10. 本轮明确保留

- `target/debug/deps/`、`build/`、当前 `.fingerprint/`；
- `target/debug/pixnya.exe`、`pixnya.pdb`；
- `target/aarch64-linux-android/debug/deps/`、`build/`、当前 `.fingerprint/`；
- ARM64 的 `libpixnya_lib.a/.rlib/.so`；
- 当前 PixNya 与内部工作区 crate 的增量缓存（复用优先方案下）；
- `target/` 外的所有目录和文件。

本计划只描述后续清理边界。本轮审计没有执行删除。
