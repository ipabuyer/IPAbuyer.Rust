# 功能模块

> IPAbuyer 开发参考文档，入口见 [DEVELOPMENT.md](../DEVELOPMENT.md)。覆盖主页与购买状态、ipatool 页、日志系统、设置页、搜索功能、下载队列。

## 主页与购买状态

主页（搜索结果列表、筛选、下载进度环）**已实现（M3）**：

1. 标题栏搜索框（仅主页可用）经 iTunes Search API 搜索：`https://itunes.apple.com/search?term=名称&entity=software&limit=200&country=国家代码`。
2. 筛选：全部 / 未购买 / 已购买（工具栏分段按钮）+ 平台（全部/iOS/iPad/Mac）与开发者（**独立筛选窗口**，label `filter`，"筛选"按钮打开，行为仿日志窗口：用户关闭即销毁、按钮再开重建；筛选选择与开发者选项保存在后端 AppState，经 `filter-changed` 事件同步两个窗口，开发者选项随每次搜索刷新、失效选择自动回退）；空结果显示空状态提示。
3. 结果卡片（SettingsCard 风格）：App 图标、名称、开发者、版本号、平台徽标（仅 macOS 条目显示 "Mac"）、购买状态文字（已购买绿 / 无法购买红）、操作按钮（未购→购买；已购→下载；无法购买→禁用）、三点菜单（标记已购/未购、复制名称/ID、在 App Store 打开）。
4. 购买状态来自数据库合成；"无法购买"由价格推导不入库；`alreadyOwned` 或 `failed to purchase item with param 'STDQ'` 直接标记已购买不弹窗。
5. 搜索与购买经 core（`core::appcatalog` / purchase 流程）实现；底部 InfoBar → shadcn Alert。

## ipatool 页

**已实现（M4）**：内置版本卡片（release@2.6.0、"当前使用"徽章、导出）、自定义 ipatool.exe 卡片（选择/使用/删除插槽）、版本要求卡片（≥2.5.0）、详细日志开关（`detailedIpatoolLog`）、清空 ipatool 数据（`~/.ipatool/`）、majd/ipatool 仓库链接。来源选择 `ipatoolFlavor`（main/custom）与 `customIpatoolPath` 已在配置结构中就位。

## 日志系统

**已实现（M3）**：

1. 展示形式为**独立日志窗口**（对齐 WinUI3 版 LogViewerWindow）：Rust 命令 `logs_show_window`/`logs_hide_window` 按需创建/隐藏（label `log`，用户关闭即销毁、再开重建并快照回填）；前端 `main.tsx` 按窗口标签分流渲染 `LogWindow`。
2. 格式 `[日期时间] [INFO] 内容`；等级着色；ipatool 输出的等级标修订为 `[ipatool]`；等宽字体深色底。
3. 日志窗口自动展开：主页点击下载、设置页开始/取消同步（刷新已购列表）时自动打开；购买经 toast 反馈不开窗（前端测试约定），账户页提供手动日志按钮。
4. 数据链路：core 命令的日志回调 → 后端 UiLogStore（环形缓冲 1000 行）→ `emit("log-append")` → 前端 store；详细日志开关（`detailedIpatoolLog`）开启时记录命令与完整输出，`$` 前缀为输入命令（敏感值遮蔽）、`<` 前缀为 ipatool 输出行。
5. 长任务（队列、同步）由后端 tokio 任务 200ms 轮询状态并 emit 事件，前端不自行轮询。

## 设置页

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

显示语言切换即时生效（后端保存配置并广播 `language-changed`，所有窗口——主窗口/日志/筛选——各自 `changeLanguage`，"auto" 由各窗口按系统语言解析），无需重启；启动时由 `initialization_script` 注入 `window.__IPABUYER_LANG__` 保证首帧正确（见 `src-tauri/src/lib.rs` 与 `src/i18n.ts`）。

## 搜索功能

见[主页与购买状态](#主页与购买状态)。搜索请求、响应解析与已购状态合成由 core 承担（`core::appcatalog::catalog_service::search_catalog`）；同时检索 iOS（`entity=software`）、iPad（`entity=iPadSoftware`）与 Mac（`entity=macSoftware`）三个 App Store 并按此顺序合并（tvOS/visionOS 因公开搜索 API 无数据源暂不支持），国家码经 `normalize_country_code` 归一化（非法回退 `cn`），合法性由 `storefront::contains` 校验；已购状态按「平台:bundleId」组合键合成。

## 下载队列

**待实现（M3）**，行为约定（沿用 WinUI3 版）：

1. 已购 App 点击下载加入全局队列，队列未运行时自动启动；不做批量操作。
2. 状态：待下载、下载中、成功、失败、已取消；主页仅显示"终止下载"入口（队列级：终止当前下载并结束本轮，剩余条目保留状态可继续）。
3. 队列状态机与输出解析在 core（`core::downloads`）；后端命令 add/remove/start/status/cancel，事件推送见[日志系统](#日志系统)。
4. 详细日志关闭时避免刷屏；开启时显示命令、输出与下载进度片段。
