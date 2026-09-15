//! 开发者筛选。
//!
//! 移植自主仓库 `DeveloperFilter`。排序差异：C# 使用 CurrentCultureIgnoreCase
//! 的文化感知排序，Rust 侧以小写比较近似（ASCII 场景行为一致）。

use std::collections::HashMap;

/// 开发者筛选项：显示名（首次出现的规范化拼写）与出现次数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeveloperFilterOption {
    pub display_name: String,
    pub count: i64,
}

/// 汇总开发者选项：忽略空白名、大小写不敏感分组、按次数降序后按显示名升序。
pub fn build_options(developer_names: &[Option<&str>]) -> Vec<DeveloperFilterOption> {
    let mut developers: HashMap<String, (String, i64)> = HashMap::new();
    for name in developer_names {
        let developer = name.unwrap_or("").trim();
        if developer.is_empty() {
            continue;
        }

        developers
            .entry(developer.to_lowercase())
            .and_modify(|(_, count)| *count += 1)
            .or_insert_with(|| (developer.to_string(), 1));
    }

    let mut options: Vec<DeveloperFilterOption> = developers
        .into_values()
        .map(|(display_name, count)| DeveloperFilterOption {
            display_name,
            count,
        })
        .collect();
    options.sort_by(|left, right| {
        right.count.cmp(&left.count).then_with(|| {
            left.display_name
                .to_lowercase()
                .cmp(&right.display_name.to_lowercase())
        })
    });
    options
}

/// 判断结果是否匹配当前筛选：筛选为空匹配全部；开发者名先裁剪再大小写不敏感比较。
pub fn matches(developer_name: Option<&str>, selected_developer: Option<&str>) -> bool {
    let Some(selected) = selected_developer else {
        return true;
    };
    if selected.trim().is_empty() {
        return true;
    }

    developer_name.is_some_and(|developer| developer.trim().eq_ignore_ascii_case(selected))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_options_ignores_blank_names_and_groups_case_insensitively() {
        let names = [
            None,
            Some(""),
            Some("  "),
            Some(" Acme "),
            Some("acme"),
            Some("ACME"),
            Some("Beta"),
        ];

        let options = build_options(&names);

        assert_eq!(
            options,
            vec![
                DeveloperFilterOption {
                    display_name: "Acme".to_string(),
                    count: 3
                },
                DeveloperFilterOption {
                    display_name: "Beta".to_string(),
                    count: 1
                },
            ]
        );
    }

    #[test]
    fn build_options_preserves_first_normalized_spelling() {
        let names = [Some("  Zebra Labs "), Some("zebra labs")];

        let options = build_options(&names);

        assert_eq!(
            options,
            vec![DeveloperFilterOption {
                display_name: "Zebra Labs".to_string(),
                count: 2
            }]
        );
    }

    #[test]
    fn build_options_orders_same_counts_by_display_name() {
        let names = [Some("Beta"), Some("Acme"), Some("Zebra")];

        let display_names: Vec<String> = build_options(&names)
            .into_iter()
            .map(|option| option.display_name)
            .collect();

        assert_eq!(display_names, ["Acme", "Beta", "Zebra"]);
    }

    #[test]
    fn matches_applies_selection_and_developer_normalization() {
        let cases = [
            (None, None, true),
            (Some("Acme"), Some(""), true),
            (Some(" Acme "), Some("acme"), true),
            (Some("Acme"), Some("Beta"), false),
            (None, Some("Acme"), false),
            // 选中的筛选值不做裁剪：带空格的选值与裁剪后的开发者名不相等（对齐 C#）。
            (Some("Acme"), Some(" Acme "), false),
        ];

        for (developer_name, selected_developer, expected) in cases {
            assert_eq!(matches(developer_name, selected_developer), expected);
        }
    }
}
