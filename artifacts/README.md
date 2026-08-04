# Build artifacts

构建产物统一收集到这里：

- `windows/`：Windows EXE；
- `android/`：Android APK；
- `SHA256SUMS.txt`：所有已收集产物的 SHA-256。

目录中的二进制文件由 `scripts/collect-artifacts.ps1` 生成，不提交到版本库。
