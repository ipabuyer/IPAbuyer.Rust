# IPAbuyer.Rust

IPAbuyer 的 Tauri 2 空白工程，用于生成 `.msixbundle` 并上传微软商店测试 Tauri 应用与商店的兼容性。

包标识与已发布的 WinUI3 版保持一致，使商店将其识别为同一应用：

| 项 | 值 |
| --- | --- |
| Identity Name | `IPAbuyer.IPAbuyer` |
| Publisher | `CN=68F867E4-B304-4B5D-9818-31B1910E0771` |
| DisplayName / PublisherDisplayName | IPAbuyer |
| 体系结构 | x64 |
| 最低系统 | Windows 10 1809 (10.0.17763.0) |

## 前置条件

- Node.js + pnpm（`@tauri-apps/cli`）
- Rust (MSVC) 1.77+
- Windows SDK（`makeappx.exe`，脚本按 `WindowsSdkDir` 环境变量 → Program Files → 各固定盘根目录下的 `Windows Kits\10\bin` 的顺序自动查找）
- 运行时依赖 WebView2 Evergreen Runtime（Win10/11 一般已内置）

## 构建并打包

```powershell
pnpm install
pnpm build        # tauri build --no-bundle，产出 src-tauri/target/release/IPAbuyer.exe
pnpm msix         # 打包 msix/out/IPAbuyer_<版本>_x64.msixbundle
```

自定义版本号（商店要求必须高于已发布版本，源工程使用 年.月.日 CalVer）：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/make-msix.ps1 -Version 2026.9.14.0
```

## 目录结构

- `ui/` — 静态前端（编译时内嵌进 exe，包内无需额外文件）
- `src-tauri/` — Tauri 2 Rust 工程
- `src-tauri/icons/icon-source.png` — 图标源文件（1000x1000，黑底无透明通道）；修改后运行 `pnpm tauri icon src-tauri/icons/icon-source.png -o src-tauri/icons` 重新生成整套图标并重新构建
- `msix/AppxManifest.template.xml` — MSIX 清单模板（`@VERSION@` 由打包脚本替换）
- `msix/assets/` — 商店图标资源（scale-100，源自 WinUI3 工程，文件名与清单引用一致）
- `msix/out/` — 打包产物（已 gitignore）
- `scripts/make-msix.ps1` — 暂存 + makeappx pack/bundle 打包脚本

## 关于签名

上传 Partner Center 时**无需本地签名**，商店在发布时会自动用商店证书重新签名。
如需本地旁加载（sideload）安装测试，需用与 Publisher 一致的证书签名
（`signtool sign /fd SHA256 /a <包>`，证书 Subject 必须为 `CN=68F867E4-B304-4B5D-9818-31B1910E0771`）。

## 注意事项

- 版本号必须严格大于商店中已发布的版本，否则上传时会被校验拒绝。
- `msix/out/` 中的 `pack-mapping.txt`、`bundle-mapping.txt`、`verify/` 为脚本中间产物。
- 图标资源（scale-100）取自 WinUI3 工程并收录在 `msix/assets/`；包内未生成 `resources.pri`，故清单直接引用原始文件名。
