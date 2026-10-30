# 许可证与软件物料清单

PixNya 自身使用 `GPL-3.0-only`。仓库根目录的 [`LICENSE`](../LICENSE) 是未经改写的 GNU GPL version 3 完整正文；依赖项仍分别遵循其上游许可证，不能因为 PixNya 使用 GPL 而忽略上游的署名、通知或源码提供义务。

## 仓库中的文件

- [`THIRD_PARTY_NOTICES.md`](../THIRD_PARTY_NOTICES.md)：与 npm、Cargo 和 Android Gradle 三套锁图指纹绑定的依赖版本和上游声明许可证清单。
- [`gradle-license-review.json`](../gradle-license-review.json)：从受 SHA-256 验证的本机 Maven POM 生成并提交审阅的许可证证据快照；必须与 Android Gradle 指纹及全部锁定坐标完全一致。
- `scripts/generate-supply-chain-artifacts.mjs`：仅使用锁文件、已提交的 Gradle 许可证审阅、本机 npm/Cargo 缓存与固定版本 SPDX 正文数据生成清单、SBOM 和许可证证据目录，不访问网络，也不执行依赖代码。
- `scripts/generate-gradle-license-review.mjs`：仅在 Gradle 依赖发生变化时，从本机缓存递归解析 POM 父链；任何 POM 缺少 tracked SHA-256、许可证名称未审阅或证据不完整都会失败。
- `artifacts/supply-chain/pixnya-<version>.spdx.json`：生成的 SPDX 2.3 JSON SBOM；`artifacts/` 是构建产物目录，因此该文件不提交到 Git。
- `artifacts/supply-chain/pixnya-<version>-third-party-licenses/`：逐依赖保存上游 `LICENSE`/`COPYING`/`NOTICE`、审计元数据和必要的 SPDX 标准正文；Release 会把它打成同名 `.tar.gz`。
- `scripts/supply-chain-regression.test.mjs`：校验 GPL 正文摘要、锁文件指纹、离线参数、锁文件解析、SPDX 结构、许可证目录和未知许可证的 fail-closed 行为。

`THIRD_PARTY_NOTICES.md` 是依赖审计索引，不替代依赖包自身携带的 `LICENSE`、`COPYING` 或 `NOTICE`。许可证证据目录优先逐字保留 npm/Cargo 上游文件；发布包没有携带标准许可证正文时，使用固定的 `spdx-license-list@6.12.0` 正文。Gradle/Maven 组件同时携带逐组件 `MAVEN-LICENSE-DECLARATION.json` 与 SPDX 正文。少数没有标准 SPDX 标识的 Maven 声明使用显式 `LicenseRef`，标为 `upstream-metadata-only` 并保存原名称、URL、POM 摘要和人工分类，绝不伪装成完整标准正文。新增未知声明、缺失证据或不可读取正文都会终止生成。

## 首次准备本机缓存

生成器不会主动下载依赖。新工作区需要联网准备一次锁定依赖：

```powershell
npm ci
cargo fetch --locked
node scripts/check-android-gradle-supply-chain.mjs --check
```

已提交的 `gradle-license-review.json` 使干净克隆不必在正式生成阶段重新下载 Maven POM。只有 Android Gradle 锁图变化时，才应先解析锁定依赖、补齐 `verification-metadata.xml` 中每个直接及父 POM 的 SHA-256，再在本机运行：

```powershell
node scripts/complete-gradle-pom-verification.mjs --write
npm run supply-chain:gradle-poms:check
npm run supply-chain:gradle-review
```

`--write` 只把当前本机 Maven 缓存中的精确 POM 摘要加入 Gradle verification metadata；它建立新的信任基线，因此必须审查 XML diff，不能在 CI 中自动接受。随后审阅生成的许可证名称、URL、SPDX 映射和所有 `LicenseRef` 后再提交。完成后可以断网运行下面的生成和检查命令。

## 离线生成

```powershell
node scripts/generate-supply-chain-artifacts.mjs
```

生成器执行 `cargo metadata --locked --offline --format-version 1`，从 npm v3 锁文件读取精确版本、下载地址、完整性摘要和许可证字段，并用 Android Gradle checker 的指纹验证 Maven 审阅快照。主 SPDX 2.3 SBOM、通知清单和许可证目录都覆盖 npm、Cargo、Gradle/Maven 三类依赖；单独的 `android-gradle-dependencies.json` 只是附加的锁图与 artifact 摘要清单，不能替代主 SBOM 或许可证证据。

两个没有在 `package-lock.json` 声明许可证的固定 npm 版本使用其已安装包内的 MIT `LICENSE` 作为证据；脚本同时锁定证据文件 SHA-256，版本或正文变化都会拒绝生成。跨平台 optional npm 包在当前系统可能不会解包，此时仍须有锁文件声明和可解析的 SPDX 标准正文，否则同样失败。

为可复现的 SBOM 时间戳设置 `SOURCE_DATE_EPOCH`：

```powershell
$env:SOURCE_DATE_EPOCH = "1786233600"
node scripts/generate-supply-chain-artifacts.mjs
```

也可以指定输出路径：

```powershell
node scripts/generate-supply-chain-artifacts.mjs `
  --notices THIRD_PARTY_NOTICES.md `
  --sbom artifacts/supply-chain/pixnya-1.0.0.spdx.json `
  --licenses-dir artifacts/supply-chain/pixnya-1.0.0-third-party-licenses
```

## 离线检查

下列 `--check` 命令不调用 Cargo 元数据，也不需要 `node_modules`、Cargo 或 Maven 包缓存；它校验完整 GPL 正文、npm/Cargo 锁文件、Android Gradle 锁图、Gradle 许可证审阅和已提交通知清单的组合指纹，因此适合放在快速测试的最前面：

```powershell
node scripts/generate-supply-chain-artifacts.mjs --check
node --test scripts/supply-chain-regression.test.mjs
```

只要任一锁文件、Gradle wrapper、verification metadata 或许可证审阅变化，检查就会要求重新审阅并生成 `THIRD_PARTY_NOTICES.md`。生成阶段如果出现 `NOASSERTION`、未覆盖 Maven 坐标、未验证 POM、未知许可证名称或缺失 LicenseRef 证据，脚本都会失败，不允许用猜测继续制作正式产物。

## Android 漏洞扫描边界

正式发布的 OSV 阻断扫描使用 `scripts/generate-android-runtime-sbom.mjs` 从 `app/gradle.lockfile` 精确筛选 `arm64ReleaseRuntimeClasspath` 与 `armReleaseRuntimeClasspath`。当前两套锁图各含 79 个实际 APK runtime Maven 包且集合必须完全相同；缺少任一图或出现分叉都会失败关闭。Release 工作流把这份共同的 Android ARM runtime SBOM 交给固定版本的 OSV Scanner，并在命中漏洞时失败。它不能替代完整物料清单。

AGP、Kotlin Gradle plugin、UTP、buildscript 和 buildSrc 等构建工具仍全部进入严格依赖锁、SHA-256 verification、`android-gradle-dependencies.json`、主 SPDX SBOM、`THIRD_PARTY_NOTICES.md` 与逐依赖许可证归档。它们通过直接插件升级审查和构建环境治理控制；不能把仅存在于 UTP/buildscript 的 advisory 误报成 APK runtime 漏洞，也不能因此从完整供应链记录中删除。

2026-10-06 已通过升级依赖消除原有 88 条构建期例外，以及后续发现的 Bouncy Castle、Jackson 和 Commons Lang 告警。Bouncy Castle 三模块使用 `1.85`，Netty 使用 `4.1.137.Final`，Protobuf 使用 `3.25.5`，Commons Compress 使用 `1.26.0`，Commons Lang 使用 `3.18.0`，jose4j 使用 `0.9.6`，JDOM 使用 `2.0.6.1`，Jackson core/databind 使用 `2.22.3`。Kotlin Gradle Plugin 升级到正式版 `2.4.20`；Tauri 2.11.5、AGP 8.11.0、Gradle 8.14.3 与 JDK 17 保持原版本。

`gradle/tauri-kotlin-compat.gradle` 在每次 Gradle 配置时，仅把锁定 Tauri 模块的旧 `kotlinOptions` 转为 `compilerOptions`，保持 JVM 1.8 目标。原始 Cargo 包源码和资源保持原位置；兼容脚本在该模块自身的 `build/` 目录中按需重建，支持 Cargo 缓存与项目位于不同 Windows 盘符。旧配置不再精确匹配时会失败，要求重新审阅桥接。此转换同时覆盖 CI、本地脚本和直接执行 Tauri Android 构建的入口。

`docs/android-gradle-osv-risk-baseline.json` 当前 `exceptions` 为空。扫描只有覆盖三套锁图的每一个 Maven 包、无新增漏洞且与空基线一致时才通过；空报告、漏扫描、未知坐标或损坏结果都会失败。ARM64 与 ARM32 APK runtime 继续保持零例外、零 ignore。未来确需构建期例外时，仍要求精确 advisory、版本、scope、人工理由和 30 天内复核期；Critical 最长 14 天，运行时不得使用例外。

每次候选 Release 都会重新扫描三套 Gradle 锁，并把未经裁剪的 `pixnya-<version>-android-build-tools-osv.json` 作为附件归档；独立的每周工作流也运行同一基线检查并保留原始报告。到期条目只能通过升级/移除依赖或经过新的人工风险审查后显式更新，不能自动续期。

2026-10-30 发布复验发现新收录的 `GHSA-xxph-c9ww-hj94`，已按 [Guava 33.7.2 官方修补说明](https://github.com/google/guava/releases/tag/v33.7.2) 将 buildscript、buildSrc 和 UTP 构建图统一到 `33.7.2-jre`。同步更新其必要的 failureaccess、Error Prone、J2ObjC 和 JSpecify 依赖；15 个新增 artifact 的 SHA-256 已与 Maven Central 原文件独立比对，既有校验值保持不变。326 个锁定 Maven 组件对应 359 个组件及父 POM 已通过校验，新增许可证声明均为 Apache-2.0。三套锁图 OSV 扫描零告警，两个 ARM APK runtime 仍为共同的 79 包、零告警；不增加漏洞例外。

## Rust 停止维护公告边界

Release 的 `cargo-deny` 门以 `unmaintained = "all"` 和 `unsound = "all"` 检查完整传递图，继续阻断全部 RustSec 漏洞、新增的未定义行为公告、新增的停止维护公告和未审阅例外。`deny.toml` 只临时接受当前 Tauri 2.11.5 上游依赖图中仍会被扫描到的 7 条公告：1 条是 Linux gtk-rs 宏停止维护公告，5 条是 `tauri-utils 2.9.3 -> urlpattern 0.3 -> rust-unic 0.9` 停止维护公告，另 1 条是 `glib 0.18.5` 的 `VariantStrIter` 未定义行为公告。2026-09 起 gtk-rs GTK3 绑定仓库取消归档，RustSec 撤回了 `RUSTSEC-2024-0411`–`0420` 停止维护公告；`unused-ignored-advisory = "deny"` 要求从忽略名单删除这些已不再命中的条目。

`RUSTSEC-2024-0429` 的官方修复要求 `glib >=0.20`，与 Tauri 2 的 GTK3 0.18 图不兼容。对当前锁定依赖进行人工调用点复核后，PixNya 与已检查的 Tauri 消费路径没有使用 `VariantStrIter`/`array_iter_str`；因此私人候选版把它作为短期风险例外，而不是关闭 `unsound` 检查。上游修复记录为 [gtk-rs-core#1343](https://github.com/gtk-rs/gtk-rs-core/pull/1343)。

后续迁移继续参考 [Tauri GTK4 议题](https://github.com/tauri-apps/tauri/issues/11942)和已合入的 [urlpattern 0.6 迁移记录](https://github.com/tauri-apps/tauri/pull/15660)。2026-10-06 重新扫描 583 个锁定 Rust 包，仍只命中上述 7 条 RustSec 公告；锁图仍为 Tauri 2.11.5、`tauri-utils 2.9.3`、`urlpattern 0.3.0`、`glib 0.18.5`。逐包检查锁定源码后，受影响迭代器只出现在 glib 自身定义、文档和测试，未发现非测试消费方调用。现有 7 条例外据此人工复核至 2026-11-05，不新增或扩大例外。GTK3 停止维护公告 `RUSTSEC-2024-0411`–`0420` 已撤回，不再列入忽略名单。`unused-ignored-advisory = "deny"`、全部未维护和未定义行为检查继续生效：公告变化或到期未复核仍会失败。此 Rust 复核用于解除本次修复推送的既有 CI 到期阻塞；每周 Android 审计已经使用空例外基线。

## 正式发布要求

每个 Draft Release 至少应包含：

1. 与对应源码提交一致的完整 `LICENSE` 和 `THIRD_PARTY_NOTICES.md`；
2. 使用相同 npm、Cargo 与 Gradle 锁图生成的 `pixnya-<version>.spdx.json`，其中必须包含全部锁定 Maven 组件；
3. 一个 `pixnya-<version>-verification.tar.gz`，统一包含 npm、Cargo 与 Gradle/Maven 逐依赖正文、声明、通知、SBOM、来源记录和审计元数据；
4. 安装包、更新清单、SBOM、许可证归档和通知文件的 SHA-256；
5. 对新增或变更许可证的人工复核记录。

Release 附件还必须包含上述 Android build-tool OSV 原始报告；它是临时风险基线的审计证据，不能替代 runtime SBOM 或把 build-only 例外扩展到 APK 运行时。

SPDX 清单覆盖所有锁定的跨平台和构建依赖，因此某个组件出现在 SBOM 中不表示它一定被链接进 Windows、Linux 或 Android 的每一个产物。若以后需要按最终二进制精确裁剪，应在各平台完成构建后追加二进制组成分析，而不是删除当前完整锁图清单。
