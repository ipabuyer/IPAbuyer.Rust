//! 购买记录状态归一化。
//!
//! 移植自主仓库 `PurchaseRecordStatus`。"已拥有"已合并进"已购买"：
//! 历史 owned 值（含本地化形态）统一归一化为 [`PURCHASED`]。

/// 已购买记录的唯一规范状态（数据库存储值）。
pub const PURCHASED: &str = "purchased";

/// 归一化历史状态值；接受规范 token 与历史本地化形态（已购买/已拥有/owned 等）。
/// 返回 `None` 表示无法识别（调用方按"未购买"处理）。
pub fn try_normalize(status: Option<&str>) -> Option<&'static str> {
    let value = status?.trim();
    if value.is_empty() {
        return None;
    }

    let is_purchased = value.eq_ignore_ascii_case(PURCHASED) || value == "已购买";
    let is_legacy_owned = value.eq_ignore_ascii_case("owned")
        || value.eq_ignore_ascii_case("Already owned")
        || value == "已拥有";

    if is_purchased || is_legacy_owned {
        Some(PURCHASED)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_normalize_accepts_canonical_and_legacy_purchase_records() {
        let cases = [
            ("purchased", PURCHASED),
            ("Purchased", PURCHASED),
            ("已购买", PURCHASED),
            ("owned", PURCHASED),
            ("Already owned", PURCHASED),
            ("已拥有", PURCHASED),
        ];

        for (status, expected) in cases {
            assert_eq!(try_normalize(Some(status)), Some(expected));
        }
    }

    #[test]
    fn try_normalize_rejects_non_record_statuses() {
        for status in [
            None,
            Some(""),
            Some("  "),
            Some("Not purchased"),
            Some("available for purchase"),
            Some("Unable to purchase"),
            Some("未购买"),
            Some("可购买"),
            Some("无法购买"),
            Some("unknown"),
        ] {
            assert_eq!(try_normalize(status), None);
        }
    }

    #[test]
    fn try_normalize_trims_whitespace_and_ignores_case() {
        let cases = [
            ("  purchased  ", PURCHASED),
            ("OWNED", PURCHASED),
            (" ALREADY OWNED ", PURCHASED),
            (" 已购买 ", PURCHASED),
            (" 已拥有 ", PURCHASED),
        ];

        for (status, expected) in cases {
            assert_eq!(try_normalize(Some(status)), Some(expected));
        }
    }
}
