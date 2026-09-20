# 账户、密钥与数据库

> IPAbuyer 开发参考文档，入口见 [DEVELOPMENT.md](../DEVELOPMENT.md)。覆盖账户页与登录流程、加密密钥处理、已购数据库。

## 账户页与登录流程

**已实现**（`src/pages/account.tsx` + `src-tauri/src/commands/auth.rs`）。

1. 四个输入框：账户、密码、双重验证码、加密密钥；按钮：登录、查询登录状态、退出登录、打开苹果账户官网。
2. 登录先用占位验证码 `000000` 触发双重验证码下发（core `auth::login`），用户填入真实验证码后再 `auth::verify_auth_code` 完成登录。
3. 收不到验证码时提示打开 <https://account.apple.com/> 获取。
4. 启动时静默执行查询登录状态恢复会话（App.tsx effect）；已登录时输入区禁用（锁定蒙版），邮箱框回填已登录邮箱。
5. 标题栏头像显示登录态：已登录绿色、未登录红色。
6. 测试账户 `test`/`test`：购买、下载、登录直接成功，用于界面测试（core 内识别，同步不执行）。

## 加密密钥（keychain-passphrase）处理

**已实现**（`src-tauri/src/state.rs`）。

1. 密钥显示于账户页输入框；页面初始化时读取已存值，不存在则生成 UUID（32 位十六进制，对齐 `Guid "N"` 格式）填入，**登录成功后才持久化**。
2. 存储位置：Windows 凭据管理器（keyring crate），service `IPAbuyer.ipatool.passphrase`、user `__default__`（与旧版 PasswordVault 同名但存储不同）。**首启自动迁移**：凭据管理器尚无密钥时，从旧版 PasswordVault 同名条目读入（`state.rs::migrate_legacy_passphrase`），商店升级用户无需重新登录。
3. 登录命令的密钥解析顺序：输入框显式传入 > 已存密钥 > 新生成（成功后落库）。
4. 购买、下载、同步等命令不从输入框读取，统一使用已存密钥。
5. 修改密钥：提示用户退出登录，改输入框后重新登录。
6. 退出登录成功且设置 `passphraseRotationEnabled` 为 true 时，自动生成新 UUID 密钥落库。

## 数据库

1. `PurchasedAppDb.db`（SQLite，core rusqlite 承担）存放已购记录（bundleId + 账户 + 平台，状态统一 "purchased"）与 `SyncState` 表（上次成功/尝试同步时间）；schema `user_version` 3（2→3 加 `Platform` 列，历史记录归 ios；同一 bundleId 在 iOS/Mac 商店是不同条目）。
2. 路径：Tauri `app_data_dir`（`%APPDATA%\com.ipabuyer.app\`）；packaged 运行时经 MSIX 虚拟化重定向到包容器，读写一致。
3. `src-tauri/src/state.rs` 的 `AppState::new` 在 setup 时打开，句柄以 Mutex 串行化。
4. 旧版 WinUI3 的数据库在 `%AppData%\Local\Packages\IPAbuyer.IPAbuyer_kr1hdvrv6tpd0\LocalState\PurchasedAppDb.db`，schema 相同可复制导入（设置页提供导入提示，待实现）。
5. 旧版 LocalSettings 设置项不迁移（国家码等需重新设置）；PasswordVault 中的加密密钥已支持首启自动迁移（见[加密密钥（keychain-passphrase）处理](#加密密钥keychain-passphrase处理)）。
