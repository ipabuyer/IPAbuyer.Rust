//! IPAbuyer 跨平台核心库（并入自 github.com/ipabuyer/IPAbuyer.Core，原仓库淘汰）。
//!
//! 模块划分：
//! - 纯逻辑：ipatool 命令构建、响应解析、购买状态策略、已购买页解析
//! - 进程编排：`execution` + `ipatool::client`、登录分类（`auth`）、
//!   同步服务（`purchases::sync_service`）、App Catalog（`appcatalog`）
//! - 存储：数据库（`db`）、下载队列（`downloads`）、日志缓冲（`logging`）
//!
//! 与原仓库的差异：不包含 `ffi` 模块（C ABI 仅服务 C# 宿主），
//! 模块内 `crate::` 路径已改写为 `crate::core::`。

pub mod appcatalog;
pub mod auth;
pub mod db;
pub mod downloads;
pub mod execution;
pub mod ipatool;
pub mod json;
pub mod logging;
pub mod purchases;
