# 测试

> IPAbuyer 开发参考文档，入口见 [DEVELOPMENT.md](../DEVELOPMENT.md)。

1. Rust：`cd src-tauri && cargo test`——core 并入的 119 项 + 应用层（配置序列化兼容、密钥轮换默认值、日志缓冲环形上限与序号游标、平台维度、消息序列化、DTO 映射等）共 167 项；修改 core 或命令层必须保证通过。
2. 前端：`pnpm test`（Vitest 5 + jsdom，`pnpm test:watch` 常驻）——覆盖 lib/ 纯函数（价格/状态策略、URL 拼装、cn）、stores（会话/搜索/队列/日志，mock Tauri invoke 与 event）、消息渲染 hook（{{0}} 插值与键名回退），以及组件与页面测试（SettingsCard/AppSidebar 结构与路由、日志窗口、账户页表单校验与日志按钮、主页卡片渲染/筛选/购买动作与不开窗约定）。新增 UI 文本逻辑或组件时应配套用例。
3. 命令层行为另以 CDP 脚本（eval-webview/screenshot/watch-webview-errors）+ 实机验证兜底。
4. 新增可测纯逻辑（如解析、策略）应补单元测试。
