# IPAbuyer 开发指南（Tauri 2 版）

本文是 IPAbuyer Tauri 2 重写版的详细开发指南，鼓励修改以同步最新开发进度。业务行为约定继承自 WinUI 3 版（E:\ipabuyer），技术实现为 Tauri 2。

## 目录

1. [项目概述](#1-项目概述)
2. [通用约束](#2-通用约束)
3. [技术栈与工程结构](#3-技术栈与工程结构)
4. [构建与调试](#4-构建与调试)
5. [发布与版本管理](#5-发布与版本管理)
6. [内置 ipatool 可执行文件](#6-内置-ipatool-可执行文件)
7. [ipatool 命令参考](#7-ipatool-命令参考)
8. [账户页与登录流程](#8-账户页与登录流程)
9. [加密密钥（keychain-passphrase）处理](#9-加密密钥keychain-passphrase处理)
10. [数据库](#10-数据库)
11. [UI 总体规范](#11-ui-总体规范)
12. [主页与购买状态](#12-主页与购买状态)
13. [ipatool 页](#13-ipatool-页)
14. [日志系统](#14-日志系统)
15. [设置页](#15-设置页)
16. [搜索功能](#16-搜索功能)
17. [下载队列](#17-下载队列)
18. [本地化](#18-本地化)
19. [测试](#19-测试)
20. [与 WinUI3 版的差异清单](#20-与-winui3-版的差异清单)
21. [参考链接](#21-参考链接)

## 1. 项目概述

IPAbuyer 是一款发布至 Microsoft Store 的桌面应用，帮助用户浏览、购买（仅限免费 App）并下载 App Store 中的 App。本仓库为 Tauri 2 重写版，用于替代 WinUI 3 版。

- 底层工具：[majd/ipatool](https://github.com/majd/ipatool) 2.5.0，所有认证、购买、下载经其完成
- 代码仓库：<https://github.com/ipabuyer/IPAbuyer.Rust>
- 开发者网站：<https://ipa.blazesnow.com>
- 商店身份：`IPAbuyer.IPAbuyer` / `CN=68F867E4-B304-4B5D-9818-31B1910E0771`（与 WinUI3 版一致，PFN `IPAbuyer.IPAbuyer_kr1hdvrv6tpd0`）

## 2. 通用约束

1. 所有文件均以 UTF-8 存储、读取和修改；`.ps1` 脚本必须保留 UTF-8 BOM（PowerShell 5.1 会按 GBK 解析无 BOM 的脚本）。
2. UI 文本一律经 i18next 键引用（`src/locales/`），禁止硬编码；core 返回的本地化消息为键名 + 位置参数，渲染约定见[本地化](#18-本地化)。
3. 允许并鼓励编译运行测试验证改动； packaged 行为验证见[构建与调试](#4-构建与调试)。
4. 终端注意 GBK 与 UTF-8 编码差异；PowerShell 脚本内调用外部命令优先用绝对路径（PATH 中的 Git GNU tar 会把 `C:\` 误判为远程主机）。
5. 不得在仓库中硬编码盘符路径（SDK 探测、资源引用均需可移植）。
6. 二进制图片（`*.png`/`*.ico`/`*.icns`）走 Git LFS（`.gitattributes`）。

## 3. 技术栈与工程结构

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

## 4. 构建与调试

### 常用命令

| 命令 | 作用 |
| --- | --- |
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

## 5. 发布与版本管理

1. 最终发布至 Microsoft Store；上传 `.msixbundle` 无需本地签名（商店自动重签）。
2. 版本号采用 `年.月.日.0` CalVer，维护于 `src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`；MSIX 版本由 `scripts/make-msix.ps1 -Version x.y.z.w` 指定，**必须严格大于商店已发布版本**。
3. bundle 版本经 `makeappx bundle /bv` 显式指定为与包内版本一致——缺省时 makeappx 会用当前 UTC 时间生成版本（表现为 `2026.914.940.0` 之类的乱象）。
4. 打包流程：`pnpm build` → `pnpm msix`；产物 `msix/out/`（已 gitignore）。包内容：`IPAbuyer.exe`（前端已内嵌，无外部资源文件）、`ipatool.exe`、清单与商店图标。
5. 前端不生成 `resources.pri`，清单直接引用 `Assets/` 原始文件名（scale-100）。

## 6. 内置 ipatool 可执行文件

1. 来源：上游正式版 `2.5.0`，`scripts/fetch-ipatool.ps1` 下载 `amd64`/`arm64` tar.gz、校验 SHA-256 与 PE 头，写入 `src-tauri/binaries/ipatool-<target-triple>.exe`（Tauri sidecar 命名，gitignore，不入 git）。
2. `tauri.conf.json` 以 `bundle.externalBin` 声明；`tauri build` 会将其复制到输出目录为 `ipatool.exe`，MSIX 打包脚本原样收进包内。
3. 路径解析（`src-tauri/src/resolver.rs`）：自定义路径（flavor=custom 且文件存在）> 应用同目录 `ipatool.exe` > PATH 兜底。
4. 自定义 ipatool 要求版本 ≥ `2.5.0`（已购买功能依赖 2.5.0 引入的新逻辑）。

## 7. ipatool 命令参考

命令组装与执行在 core（`src-tauri/src/core/ipatool/`），均加 `--format json`；子进程 stdin 启动后立即关闭（交互提示因 EOF 立即结束）。

| 用途 | 命令模板 |
| --- | --- |
| 登录 | `auth login --auth-code 双重验证码 --email 邮箱 --password 密码 --keychain-passphrase 加密密钥` |
| 查询登录状态 | `auth info --keychain-passphrase 加密密钥` |
| 退出登录 | `auth revoke` |
| 列出已拥有 | `list-purchases --max-results 每页数量(≤100) --page 页码 --keychain-passphrase 加密密钥 --format json --non-interactive --verbose` |
| 购买 | `purchase --bundle-identifier APPID --keychain-passphrase 加密密钥 --format json --non-interactive --verbose` |
| 下载 | `download --output 输出位置 --bundle-identifier APPID --keychain-passphrase 加密密钥 --format json --non-interactive --verbose` |

超时约定：登录 60 秒；查询登录状态与购买 2 分钟；下载不设超时（依靠用户终止或应用关闭时终止进程树）。

## 8. 账户页与登录流程

**已实现**（`src/pages/account.tsx` + `src-tauri/src/commands/auth.rs`）。

1. 四个输入框：账户、密码、双重验证码、加密密钥；按钮：登录、查询登录状态、退出登录、打开苹果账户官网。
2. 登录先用占位验证码 `000000` 触发双重验证码下发（core `auth::login`），用户填入真实验证码后再 `auth::verify_auth_code` 完成登录。
3. 收不到验证码时提示打开 <https://account.apple.com/> 获取。
4. 启动时静默执行查询登录状态恢复会话（App.tsx effect）；已登录时输入区禁用（锁定蒙版），邮箱框回填已登录邮箱。
5. 标题栏头像显示登录态：已登录绿色、未登录红色。
6. 测试账户 `test`/`test`：购买、下载、登录直接成功，用于界面测试（core 内识别，同步不执行）。

## 9. 加密密钥（keychain-passphrase）处理

**已实现**（`src-tauri/src/state.rs`）。

1. 密钥显示于账户页输入框；页面初始化时读取已存值，不存在则生成 UUID（32 位十六进制，对齐 `Guid "N"` 格式）填入，**登录成功后才持久化**。
2. 存储位置：Windows 凭据管理器（keyring crate），service `IPAbuyer.ipatool.passphrase`、user `__default__`（与旧版 PasswordVault 同名但存储不同）。**首启自动迁移**：凭据管理器尚无密钥时，从旧版 PasswordVault 同名条目读入（`state.rs::migrate_legacy_passphrase`），商店升级用户无需重新登录。
3. 登录命令的密钥解析顺序：输入框显式传入 > 已存密钥 > 新生成（成功后落库）。
4. 购买、下载、同步等命令不从输入框读取，统一使用已存密钥。
5. 修改密钥：提示用户退出登录，改输入框后重新登录。
6. 退出登录成功且设置 `passphraseRotationEnabled` 为 true 时，自动生成新 UUID 密钥落库。

## 10. 数据库

1. `PurchasedAppDb.db`（SQLite，core rusqlite 承担）存放已购记录（bundleId + 账户 + 状态统一 "purchased"）与 `SyncState` 表（上次成功/尝试同步时间）；schema `user_version` 2。
2. 路径：Tauri `app_data_dir`（`%APPDATA%\com.ipabuyer.app\`）；packaged 运行时经 MSIX 虚拟化重定向到包容器，读写一致。
3. `src-tauri/src/state.rs` 的 `AppState::new` 在 setup 时打开，句柄以 Mutex 串行化。
4. 旧版 WinUI3 的数据库在 `%AppData%\Local\Packages\IPAbuyer.IPAbuyer_kr1hdvrv6tpd0\LocalState\PurchasedAppDb.db`，schema 相同可复制导入（设置页提供导入提示，待实现）。
5. 旧版 LocalSettings 设置项不迁移（国家码等需重新设置）；PasswordVault 中的加密密钥已支持首启自动迁移（见第 9 节）。

## 11. UI 总体规范

**已实现骨架**（`src/App.tsx`、`src/components/`）。

1. 适配系统明暗模式，主色 neutral。WebView2 的 prefers-color-scheme 不保证随系统实时更新，故由后端桥接：`system_theme.rs` 轮询注册表（AppsUseLightTheme），变化时设窗口原生主题并 emit `system-theme` 事件，前端 `main.tsx` 的 `SystemThemeSync` 据此切换文档类；`system_theme` 命令供启动时查询初始值。
2. shadcn/ui Sidebar 侧边栏定位页面，可折叠，折叠按钮在标题栏上。
3. 4 个导航页：主页、账户、ipatool、设置；页面切换用组件状态（无路由库），与 NavigationView 语义一致。
4. 自绘标题栏（`decorations: false` + `data-tauri-drag-region`）：左侧折叠按钮与应用名，主页时居中显示搜索框，右侧登录头像与最小化/最大化/关闭按钮。
5. 窗口权限集中在 `src-tauri/capabilities/default.json`；新增插件能力需同步更新。
6. 业务组件 `SettingsCard`（`src/components/settings-card.tsx`）对应 WinUI3 CommunityToolkit SettingsCard：图标 + 标题/描述 + 右侧操作区。
7. 窗口几何（大小/位置/最大化）由 tauri-plugin-window-state 持久化至 `app_config_dir/.window-state.json`（物理像素）；启动时 `fit_main_window`（`src-tauri/src/lib.rs`）将恢复的几何钳制在显示器工作区（去除任务栏）内，首次启动在工作区居中。

## 12. 主页与购买状态

主页（搜索结果列表、筛选、下载进度环）**已实现（M3）**：

1. 标题栏搜索框（仅主页可用）经 iTunes Search API 搜索：`https://itunes.apple.com/search?term=名称&entity=software&limit=200&country=国家代码`。
2. 筛选：全部 / 未购买 / 已购买 + 开发者下拉筛选；空结果显示空状态提示。
3. 结果卡片（SettingsCard 风格）：App 图标、名称、开发者、版本号、购买状态文字（已购买绿 / 无法购买红）、操作按钮（未购→购买；已购→下载；无法购买→禁用）、三点菜单（标记已购/未购、复制名称/ID、在 App Store 打开）。
4. 购买状态来自数据库合成；"无法购买"由价格推导不入库；`alreadyOwned` 或 `failed to purchase item with param 'STDQ'` 直接标记已购买不弹窗。
5. 搜索与购买经 core（`core::appcatalog` / purchase 流程）实现；底部 InfoBar → shadcn Alert。

## 13. ipatool 页

**已实现（M4）**：内置版本卡片（release@2.5.0、"当前使用"徽章、导出）、自定义 ipatool.exe 卡片（选择/使用/删除插槽）、版本要求卡片（≥2.5.0）、详细日志开关（`detailedIpatoolLog`）、清空 ipatool 数据（`~/.ipatool/`）、majd/ipatool 仓库链接。来源选择 `ipatoolFlavor`（main/custom）与 `customIpatoolPath` 已在配置结构中就位。

## 14. 日志系统

**已实现（M3）**：

1. 展示形式为**独立日志窗口**（对齐 WinUI3 版 LogViewerWindow）：Rust 命令 `logs_show_window`/`logs_hide_window` 按需创建/隐藏（label `log`，用户关闭即销毁、再开重建并快照回填）；前端 `main.tsx` 按窗口标签分流渲染 `LogWindow`。
2. 格式 `[日期时间] [INFO] 内容`；等级着色；ipatool 输出的等级标修订为 `[ipatool]`；等宽字体深色底。
3. 执行购买、登录、查询登录状态、下载、终止下载、刷新已购列表时自动展开。
4. 数据链路：core 命令的日志回调 → 后端 UiLogStore（环形缓冲 1000 行）→ `emit("log-append")` → 前端 store；详细日志开关（`detailedIpatoolLog`）开启时记录命令与完整输出。
5. 长任务（队列、同步）由后端 tokio 任务 200ms 轮询状态并 emit 事件，前端不自行轮询。

## 15. 设置页

**已实现全部项**（`src/pages/settings.tsx` + `commands/settings.rs` / `commands/sync.rs` / `commands/ipatool.rs`）。

设置持久化为 `app_data_dir/settings.json`（serde `#[serde(default)]`，新增字段向后兼容），无旧版 LocalSettings 迁移。

| settings.json 键 | 含义 | 默认值 |
| --- | --- | --- |
| `country_code` | App Store 国家/地区代码（ISO 3166-1 Alpha-2，storefront 目录校验） | `cn` |
| `download_directory` | 下载目录（null = ~/Downloads） | null |
| `display_language` | 显示语言：`auto` / `zh-Hans` / `en-US` | `auto`（按系统语言） |
| `detailed_ipatool_log` | ipatool 详细日志 | false |
| `passphrase_rotation_enabled` | 退出登录后自动轮换密钥 | true |
| `ipatool_flavor` | ipatool 来源：`main` / `custom` | `main` |
| `custom_ipatool_path` | 自定义 ipatool.exe 路径 | null |

显示语言切换即时生效（`i18n.changeLanguage`），无需重启；启动时由 `initialization_script` 注入 `window.__IPABUYER_LANG__` 保证首帧正确（见 `src-tauri/src/lib.rs` 与 `src/i18n.ts`）。

## 16. 搜索功能

见[主页与购买状态](#12-主页与购买状态)。搜索请求、响应解析与已购状态合成由 core 承担（`core::appcatalog::catalog_service::search_catalog`）；国家码经 `normalize_country_code` 归一化（非法回退 `cn`），合法性由 `storefront::contains` 校验。

## 17. 下载队列

**待实现（M3）**，行为约定（沿用 WinUI3 版）：

1. 已购 App 点击下载加入全局队列，队列未运行时自动启动；不做批量操作。
2. 状态：待下载、下载中、成功、失败、已取消；主页仅显示"终止下载"入口（队列级：终止当前下载并结束本轮，剩余条目保留状态可继续）。
3. 队列状态机与输出解析在 core（`core::downloads`）；后端命令 add/remove/start/status/cancel，事件推送见[日志系统](#14-日志系统)。
4. 详细日志关闭时避免刷屏；开启时显示命令、输出与下载进度片段。

## 18. 本地化

1. 语言资源：`src/locales/zh-Hans.json`（默认）与 `en-US.json`，由 `scripts/migrate-resw.mjs` 从 WinUI3 版 resw 迁移（553 key，键名原样保留含 `.Content` 等属性后缀）。
2. i18next 配置：`keySeparator: false`、`nsSeparator: false`（键原样查找）、`escapeValue: false`；占位符为 i18next 插值 `{{0}}`（与 core 消息位置参数数组契约一致，渲染时传 `{ 0: value }`）。
3. core 消息（`Message::Key{key,args}` / `NormalizedText::Keyed`）经后端 `JsMessage` 序列化，前端 `useRenderMessage()`（`src/lib/messages.ts`）渲染；无对应键时 i18next 回退显示键名。
4. 新增 UI 文本必须同时在两个语言 JSON 中补键；resw 重迁移需重跑脚本（会覆盖手改内容，迁移后手改应落到 JSON）。

## 19. 测试

1. Rust：`cd src-tauri && cargo test`——core 并入的 119 项 + 应用层（配置序列化兼容、密钥轮换默认值、日志缓冲环形上限、消息序列化、DTO 映射等）共 138 项；修改 core 或命令层必须保证通过。
2. 前端：`pnpm test`（Vitest 5 + jsdom，`pnpm test:watch` 常驻）——覆盖 lib/ 纯函数（价格/状态策略、URL 拼装、cn）、stores（会话/搜索/队列/日志，mock Tauri invoke 与 event）、消息渲染 hook（{{0}} 插值与键名回退），以及组件冒烟测试（SettingsCard 结构、AppSidebar 导航与外链标识）。新增 UI 文本逻辑或组件时应配套用例。
3. 命令层行为另以 CDP 脚本（eval-webview/screenshot/watch-webview-errors）+ 实机验证兜底。
3. 新增可测纯逻辑（如解析、策略）应补单元测试。

## 20. 与 WinUI3 版的差异清单

| 项 | WinUI3 版 | Tauri 2 版 |
| --- | --- | --- |
| 技术栈 | WinUI 3 / .NET 10 | Tauri 2 / React 19 + shadcn/ui |
| 业务核心 | Rust DLL 经 C ABI FFI | Rust crate 直接并入 `src-tauri/src/core/` |
| 设置存储 | LocalSettings（Settings.dat） | `settings.json`（app_data_dir） |
| 密钥存储 | Windows PasswordVault | Windows 凭据管理器（keyring），旧 PasswordVault 密钥首启自动迁移 |
| 日志展示 | 独立窗口 LogViewerWindow | 独立日志窗口（label `log`，按需创建） |
| 语言切换 | AppInstance.Restart 重启生效 | i18next 即时切换，首帧语言经 initialization_script 注入 |
| 数据目录 | 包 LocalState | app_data_dir（packaged 虚拟化），旧库可复制导入 |
| 更新机制 | 依赖商店 | 依赖商店（无应用内更新） |
| 深链/文件关联/自启 | 无 | 无 |

## 21. 参考链接

- ipatool 仓库：<https://github.com/majd/ipatool>
- 苹果账户官网（获取双重验证码）：<https://account.apple.com/>
- iTunes 搜索 API：`https://itunes.apple.com/search?term=...&entity=software&limit=...&country=...`
- Tauri 2 文档：<https://v2.tauri.app/>
- shadcn/ui：<https://ui.shadcn.com/>
- WinUI3 旧版仓库：<https://github.com/ipabuyer/ipabuyer>（行为规范来源）
- IPAbuyer.Core（已淘汰，逻辑并入本仓库）：<https://github.com/ipabuyer/IPAbuyer.Core>
