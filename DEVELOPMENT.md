# IPAbuyer 开发指南（Tauri 2 版）

本文是 IPAbuyer Tauri 2 重写版的开发指南入口，鼓励修改以同步最新开发进度。业务行为约定继承自 WinUI 3 版（E:\ipabuyer），技术实现为 Tauri 2。专题细节拆分在 [`references/`](references/) 目录。

## 参考文档

| 文档 | 内容 |
| --- | --- |
| [references/build-and-release.md](references/build-and-release.md) | 构建与调试（已知构建坑、CDP 排查）、发布与版本管理、内置 ipatool 可执行文件 |
| [references/ipatool-cli.md](references/ipatool-cli.md) | ipatool 命令行参考与超时约定 |
| [references/account-and-database.md](references/account-and-database.md) | 账户页与登录流程、加密密钥（keychain-passphrase）处理、数据库 |
| [references/ui.md](references/ui.md) | UI 总体规范（系统主题桥接、深色模式防白屏、标题栏、窗口几何） |
| [references/features.md](references/features.md) | 功能模块：主页与购买状态、ipatool 页、日志系统、设置页、搜索功能、下载队列 |
| [references/localization.md](references/localization.md) | 本地化（前端 i18next + 后端 Fluent） |
| [references/testing.md](references/testing.md) | 测试（Rust / 前端 / CDP 实机兜底） |
| [references/winui3-differences.md](references/winui3-differences.md) | 与 WinUI3 版的差异清单、参考链接 |

## 项目概述

IPAbuyer 是一款发布至 Microsoft Store 的桌面应用，帮助用户浏览、购买（仅限免费 App）并下载 App Store 中的 App（覆盖 iOS 与 Mac App Store）。本仓库为 Tauri 2 重写版，用于替代 WinUI 3 版。

- 底层工具：[majd/ipatool](https://github.com/majd/ipatool) 2.6.0，所有认证、购买、下载经其完成
- 代码仓库：<https://github.com/ipabuyer/IPAbuyer.Rust>
- 开发者网站：<https://ipa.blazesnow.com>
- 商店身份：`IPAbuyer.IPAbuyer` / `CN=68F867E4-B304-4B5D-9818-31B1910E0771`（与 WinUI3 版一致，PFN `IPAbuyer.IPAbuyer_kr1hdvrv6tpd0`）

包标识详情（Identity 与已发布的 WinUI3 版保持一致，使商店识别为同一应用）：

| 项 | 值 |
| --- | --- |
| Identity Name | `IPAbuyer.IPAbuyer` |
| Publisher | `CN=68F867E4-B304-4B5D-9818-31B1910E0771` |
| DisplayName / PublisherDisplayName | IPAbuyer |
| 体系结构 | x64（CI 另构建 arm64） |
| 最低系统 | Windows 10 1809 (10.0.17763.0) |

## 通用约束

1. 所有文件均以 UTF-8 存储、读取和修改；`.ps1` 脚本必须保留 UTF-8 BOM（PowerShell 5.1 会按 GBK 解析无 BOM 的脚本）。
2. UI 文本一律经 i18next 键引用（`src/locales/`），禁止硬编码；core 返回的本地化消息为键名 + 位置参数，渲染约定见[本地化](references/localization.md)。
3. 允许并鼓励编译运行测试验证改动；packaged 行为验证见[构建与调试](references/build-and-release.md)。
4. 终端注意 GBK 与 UTF-8 编码差异；PowerShell 脚本内调用外部命令优先用绝对路径（PATH 中的 Git GNU tar 会把 `C:\` 误判为远程主机）。
5. 不得在仓库中硬编码盘符路径（SDK 探测、资源引用均需可移植）。
6. 二进制图片（`*.png`/`*.ico`/`*.icns`）走 Git LFS（`.gitattributes`）。

## 技术栈与工程结构

- 桌面框架：Tauri 2（WebView2，最低 Windows 10 1809）。
- 前端：React 19 + TypeScript 7 + Vite 8 + Tailwind CSS v4 + shadcn/ui；状态用 zustand；i18n 用 i18next。
- 后端：Rust（edition 2024，≥1.85），业务核心 `src-tauri/src/core/` 整体并入自 [IPAbuyer.Core](https://github.com/ipabuyer/IPAbuyer.Core)（该仓库已淘汰，不再以 crate/DLL 依赖）。

| 位置 | 职责 |
| --- | --- |
| `ui` → `src/` | 前端：`pages/`（主页/账户/ipatool/设置）、`components/`（shadcn + `SettingsCard` 等业务封装）、`stores/`（zustand）、`locales/`（zh-Hans/en-US JSON）、`lib/`（api 封装、类型、消息渲染） |
| `src-tauri/src/core/` | 业务核心：ipatool 命令构建与执行、响应解析、认证分类、搜索目录、购买、已购同步、下载队列、SQLite、日志缓冲（原 ffi 模块未并入） |
| `src-tauri/src/commands/` | Tauri 命令层：`settings.rs` / `auth.rs`（购买、下载、同步命令在 M3/M4 扩展）；返回给前端的本地化消息统一为 `JsMessage`（`{type:"key",key,args}` 或 `{type:"raw",text}`） |
| `src-tauri/src/state.rs` | 配置存储（settings.json）+ 会话 + 已购数据库 + 密钥（keyring） |
| `src-tauri/src/resolver.rs` | ipatool.exe 路径解析与 cookies.lock 清理 |
| `src-tauri/src/storefront.rs` | Apple storefront 目录（175 项，校验国家码） |
| `src-tauri/capabilities/default.json` | Tauri 2 权限：窗口操作、对话框、外部链接白名单 |
| `msix/` | MSIX 清单模板（`@VERSION@` 占位）与商店图标资源（scale-100） |
| `scripts/` | `make-msix.ps1`（打包）、`fetch-ipatool.ps1`（sidecar 下载）、`migrate-resw.mjs`（resw→i18n 迁移）、CDP 调试脚本 |
