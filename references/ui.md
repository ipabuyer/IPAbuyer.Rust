# UI 总体规范

> IPAbuyer 开发参考文档，入口见 [DEVELOPMENT.md](../DEVELOPMENT.md)。

**已实现骨架**（`src/App.tsx`、`src/components/`）。

1. 适配系统明暗模式，主色 neutral。WebView2 的 prefers-color-scheme 不保证随系统实时更新，故由后端桥接：`system_theme.rs` 轮询注册表（AppsUseLightTheme），变化时设窗口原生主题并 emit `system-theme` 事件，前端 `main.tsx` 的 `SystemThemeSync` 据此切换文档类并写入内联 `color-scheme`（覆盖 wry 建窗时按"系统模式"写入的内联值，否则浅色下滚动条仍为深色），同时镜像到 `stores/theme.ts` 供 sonner 等取用（绕开不实时更新的媒体查询）；`system_theme` 命令供启动时查询初始值。
2. **深色模式启动防白屏**：页面主题在首绘前应用，不等 React 挂载。三处配合——`lib.rs` 的 initialization_script 在任何页面脚本前注入 `window.__IPABUYER_THEME__`；`index.html` 的内联脚本读取该值同步挂 `dark` 类与 `color-scheme`；Rust setup 时（`apply_startup_theme`）与子窗口创建时（logs/filter 的 builder）将窗口及 WebView 底色按主题设为深/浅色（`system_theme::window_background_color`，对应 `--background`）。日志窗口内容恒为深色底，建窗固定用深色。
3. shadcn/ui Sidebar 侧边栏定位页面，可折叠，折叠按钮在标题栏上。
4. 4 个导航页：主页、账户、ipatool、设置；页面切换用组件状态（无路由库），与 NavigationView 语义一致。
5. 自绘标题栏（`decorations: false` + `data-tauri-drag-region`）：左侧折叠按钮与应用名，主页时居中显示搜索框，右侧登录头像与最小化/最大化/关闭按钮。
6. 窗口权限集中在 `src-tauri/capabilities/default.json`；新增插件能力需同步更新。
7. 业务组件 `SettingsCard`（`src/components/settings-card.tsx`）对应 WinUI3 CommunityToolkit SettingsCard：图标 + 标题/描述 + 右侧操作区。
8. 窗口几何（大小/位置/最大化）由 tauri-plugin-window-state 持久化至 `app_config_dir/.window-state.json`（物理像素）；启动时 `fit_main_window`（`src-tauri/src/lib.rs`）将恢复的几何钳制在显示器工作区（去除任务栏）内，首次启动在工作区居中。
9. 窗口生命周期：**关闭主窗口即退出应用**（`lib.rs` 的 `on_window_event` 拦截 main 窗口 CloseRequested 后 `exit(0)`），日志/筛选子窗口随进程一起结束，无需逐个关闭；子窗口自身的叉号仍是各自关闭（销毁，再开重建）。
10. **单实例**：`tauri-plugin-single-instance`（`lib.rs` 中最先注册，官方要求其互斥锁先于其它初始化）保证同一应用仅运行一个实例；二次启动的新实例立即退出，并唤起已运行实例的主窗口（unminimize + set_focus）。
