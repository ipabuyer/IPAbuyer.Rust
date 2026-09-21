# 构建与发布

> IPAbuyer 开发参考文档，入口见 [DEVELOPMENT.md](../DEVELOPMENT.md)。覆盖构建调试、发布与版本管理、内置 ipatool sidecar。

## 构建与调试

### 前置条件

- Node.js + pnpm（`@tauri-apps/cli`，版本由 `packageManager` 字段固定）
- Rust (MSVC) 1.89+（`Cargo.toml` 的 `rust-version`，随依赖树 MSRV 提升）；arm64 交叉编译需 LLVM/clang 与 VS ARM64 生成工具
- Windows SDK（`makeappx.exe`，脚本按 `WindowsSdkDir` 环境变量 → Program Files → 各固定盘根目录下的 `Windows Kits\10\bin` 顺序自动查找）
- 运行时依赖 WebView2 Evergreen Runtime（Win10/11 一般已内置）

### 常用命令

| 命令 | 作用 |
| --- | --- |
| `pnpm install` | 安装前端依赖（首次 / 锁文件变更后） |
| `pnpm dev` | 仅启动 Vite（浏览器调试前端布局，Tauri API 不可用） |
| `pnpm tauri dev` | **日常开发主模式**：Vite + debug 构建 + 热重载 + DevTools（F12） |
| `pnpm build` | `tauri build --no-bundle`，产出 `src-tauri/target/release/IPAbuyer.exe` |
| `cargo test`（src-tauri 下） | 运行并入的 core 单元测试（119 项） |
| `pnpm msix` | 打包 `msix/out/IPAbuyer_<版本>_x64.msixbundle` |
| `run.ps1` | 一键构建并启动（自动杀实例 / touch 重编 / custom-protocol）；`-SkipBuild` 只启动、`-Msix` 顺带打包 |

### 已知构建坑（务必遵守）

1. **直接 `cargo build` 必须 `--features custom-protocol`**（Cargo.toml 已声明该 feature）；否则 exe 走 `devUrl`（localhost）而非内嵌资产。`tauri build` 会自动启用。
2. **前端 dist 变化不会触发 cargo 重编**（`generate_context!` 编译期嵌入资产）。构建 release 前先 `touch src/lib.rs` 强制重编，或确认构建日志出现 `Compiling ipabuyer`。
3. **构建前先杀掉运行中的 IPAbuyer.exe**，否则链接器报 os error 5（拒绝访问）。
4. **构建失败要看到 tail 输出**，不要只 grep 单个关键词——一次静默失败会让后续所有验证建立在旧 exe 上。
5. vite 的 watcher 已排除 `src-tauri/target`（会扫到被文件锁占用的 exe 导致 EBUSY 崩溃）。

### 前端错误排查（CDP，纯命令行）

不依赖截图和 DevTools 界面，通过 WebView2 远程调试端口直接获取：

```powershell
# 带调试端口启动（dev 或 release 均可）
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9224'
pnpm tauri dev          # 或直接启动 release exe

# 监听页面异常与 console.error（常驻）
node scripts/watch-webview-errors.mjs 9224

# 在页面里执行任意表达式（如检查状态、调 invoke）
node scripts/eval-webview.mjs 9224 "document.body.innerText.slice(0,200)"
node scripts/eval-webview.mjs 9224 "window.__TAURI_INTERNALS__.invoke('settings_get')"
```

## 发布与版本管理

1. 最终发布至 Microsoft Store；上传 `.msixbundle` 无需本地签名（商店自动重签）。
2. 版本号采用 `年.月.日(.0)` CalVer，唯一来源为 `package.json` 的 version 字段；`version.ps1` 将其同步到 `src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` 与 `Cargo.lock`（`-Check` 仅检查）；MSIX 版本由 `scripts/make-msix.ps1 -Version x.y.z.w` 指定，**必须严格大于商店已发布版本**（发布 tag 要求四段式，见 `tag.ps1`）。
3. bundle 版本经 `makeappx bundle /bv` 显式指定为与包内版本一致——缺省时 makeappx 会用当前 UTC 时间生成版本（表现为 `2026.914.940.0` 之类的乱象）。
4. 打包流程：`pnpm build` → `pnpm msix`；产物 `msix/out/`（已 gitignore）。包内容：`IPAbuyer.exe`（前端已内嵌，无外部资源文件）、`ipatool.exe`、清单与商店图标。
5. 前端不生成 `resources.pri`，清单直接引用 `Assets/` 原始文件名（scale-100）。
6. **GitHub Actions 自动构建（`.github/workflows/release.yml`）**：推送 `vX.Y.Z.W` 格式的 tag 触发，构建 x64 + arm64 双架构（`cargo build --target`，arm64 交叉编译依赖 runner 自带的 clang），合并为单一 `IPAbuyer_<版本>.msixbundle` 并发布到 GitHub Release。本地 `make-msix.ps1` 默认仍为 x64 单架构；`-TargetArch x64,arm64 -RustTarget <triple列表>` 可本地复现双架构打包（arm64 交叉编译需 clang）。
7. 本地旁加载（sideload）安装测试需自行签名：`signtool sign /fd SHA256 /a <包>`，证书 Subject 必须为 `CN=68F867E4-B304-4B5D-9818-31B1910E0771`；上传 Partner Center 无需签名（商店自动重签）。

## 内置 ipatool 可执行文件

1. 来源：上游正式版 `2.6.0`，`scripts/fetch-ipatool.ps1` 下载 `amd64`/`arm64` tar.gz、校验 SHA-256 与 PE 头，写入 `src-tauri/binaries/ipatool-<target-triple>.exe`（Tauri sidecar 命名，gitignore，不入 git）。
2. `tauri.conf.json` 以 `bundle.externalBin` 声明；`tauri build` 会将其复制到输出目录为 `ipatool.exe`，MSIX 打包脚本原样收进包内。
3. 路径解析（`src-tauri/src/resolver.rs`）：自定义路径（flavor=custom 且文件存在）> 应用同目录 `ipatool.exe` > PATH 兜底。
4. 自定义 ipatool 要求版本 ≥ `2.5.0`（已购买功能依赖 2.5.0 引入的新逻辑）；macOS 平台需要 `--platform` 参数（2.6.0 新增），自定义 2.5.0 时 macOS 相关能力不可用（同步的 macOS 轮失败会被跳过并记入日志）。

命令行模板与超时约定见 [ipatool-cli.md](ipatool-cli.md)。
