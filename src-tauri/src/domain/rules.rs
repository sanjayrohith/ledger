#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnabledRule {
    pub id: String,
    pub name: String,
    pub match_type: String,
    pub pattern: String,
    pub merchant_id: Option<String>,
    pub category_id: Option<String>,
    pub transaction_kind: Option<String>,
    pub priority: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleMatch {
    pub rule_id: String,
    pub merchant_id: Option<String>,
    pub category_id: Option<String>,
    pub transaction_kind: Option<String>,
    pub explanation: String,
}

pub fn first_match(rules: &[EnabledRule], description: &str) -> Option<RuleMatch> {
    let normalized = normalize(description);
    rules.iter().find_map(|rule| {
        let pattern = normalize(&rule.pattern);
        let matches = match rule.match_type.as_str() {
            "contains" => normalized.contains(&pattern),
            "equals" => normalized == pattern,
            "starts_with" => normalized.starts_with(&pattern),
            _ => false,
        };
        matches.then(|| RuleMatch {
            rule_id: rule.id.clone(),
            merchant_id: rule.merchant_id.clone(),
            category_id: rule.category_id.clone(),
            transaction_kind: rule.transaction_kind.clone(),
            explanation: format!(
                "{}: matched description {} `{}`",
                rule.name,
                rule.match_type.replace('_', " "),
                rule.pattern
            ),
        })
    })
}

fn normalize(value: &str) -> String {
    value.trim().to_ascii_uppercase()
}

#[cfg(test)]
mod tests {
    use super::{first_match, EnabledRule};

    fn rule(id: &str, priority: i64, kind: &str, pattern: &str) -> EnabledRule {
        EnabledRule {
            id: id.into(),
            name: format!("Rule {id}"),
            match_type: kind.into(),
            pattern: pattern.into(),
            merchant_id: None,
            category_id: Some(id.into()),
            transaction_kind: Some("expense".into()),
            priority,
        }
    }

    #[test]
    fn precedence_is_the_repository_order_and_explanation_is_stable() {
        let rules = vec![
            rule("first", 10, "contains", "SWIGGY"),
            rule("second", 20, "equals", "SWIGGY"),
        ];
        let result = first_match(&rules, "UPI-Swiggy order").expect("first matching rule");
        assert_eq!(result.rule_id, "first");
        assert_eq!(result.category_id.as_deref(), Some("first"));
        assert_eq!(
            result.explanation,
            "Rule first: matched description contains `SWIGGY`"
        );
    }

    #[test]
    fn matching_is_case_insensitive_and_respects_match_type() {
        assert!(first_match(&[rule("a", 1, "starts_with", "UPI-")], "upi-merchant").is_some());
        assert!(first_match(&[rule("a", 1, "equals", "UPI")], "UPI payment").is_none());
    }
}
