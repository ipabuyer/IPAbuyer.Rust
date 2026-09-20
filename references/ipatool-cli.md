# ipatool 命令参考

> IPAbuyer 开发参考文档，入口见 [DEVELOPMENT.md](../DEVELOPMENT.md)。

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
