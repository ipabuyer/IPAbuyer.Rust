# 与 WinUI3 版的差异清单及参考链接

> IPAbuyer 开发参考文档，入口见 [DEVELOPMENT.md](../DEVELOPMENT.md)。

## 与 WinUI3 版的差异清单

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

## 参考链接

- ipatool 仓库：<https://github.com/majd/ipatool>
- 苹果账户官网（获取双重验证码）：<https://account.apple.com/>
- iTunes 搜索 API：`https://itunes.apple.com/search?term=...&entity=software&limit=...&country=...`
- Tauri 2 文档：<https://v2.tauri.app/>
- shadcn/ui：<https://ui.shadcn.com/>
- WinUI3 旧版仓库：<https://github.com/ipabuyer/ipabuyer>（行为规范来源）
- IPAbuyer.Core（已淘汰，逻辑并入本仓库）：<https://github.com/ipabuyer/IPAbuyer.Core>
