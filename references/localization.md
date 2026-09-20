# 本地化

> IPAbuyer 开发参考文档，入口见 [DEVELOPMENT.md](../DEVELOPMENT.md)。

1. 语言资源：`src/locales/zh-Hans.json`（默认）与 `en-US.json`，由 `scripts/migrate-resw.mjs` 从 WinUI3 版 resw 迁移（553 key，键名原样保留含 `.Content` 等属性后缀）。
2. i18next 配置：`keySeparator: false`、`nsSeparator: false`（键原样查找）、`escapeValue: false`；占位符为 i18next 插值 `{{0}}`（与 core 消息位置参数数组契约一致，渲染时传 `{ 0: value }`）。
3. core 消息（`Message::Key{key,args}` / `NormalizedText::Keyed`）经后端 `JsMessage` 序列化，前端 `useRenderMessage()`（`src/lib/messages.ts`）渲染；无对应键时 i18next 回退显示键名。
4. 新增 UI 文本必须同时在两个语言 JSON 中补键；resw 重迁移需重跑脚本（会覆盖手改内容，迁移后手改应落到 JSON）。
5. **后端自有文案（Fluent）**：Rust 侧生成的用户可见文本（命令错误、子窗口标题）由 Fluent 本地化——资源 `src-tauri/locales/{zh-Hans,en-US}/main.ftl`，实现 `src-tauri/src/i18n.rs` 的 `Lang`（语言偏好同 settings 的 display_language，auto 按系统 locale 解析，缺失键回退 zh-Hans）。core 消息仍为键名由前端渲染，两者键空间独立；命令错误以原文（已本地化）返回前端展示。
