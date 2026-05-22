use serde_json::Value;

/// Evaluate a metadata filter against a JSON document.
/// Supports: exact match, $eq, $ne, $gt, $gte, $lt, $lte, $in, $nin, $exists
pub fn matches(metadata: &Value, filter: &Value) -> bool {
    let (meta_obj, filter_obj) = match (metadata, filter) {
        (Value::Object(m), Value::Object(f)) => (m, f),
        _ => return false,
    };

    for (key, condition) in filter_obj {
        let field_val = meta_obj.get(key);

        match condition {
            // Nested operator object: { "$gt": 5 } etc.
            Value::Object(ops) => {
                for (op, operand) in ops {
                    if !apply_operator(op, field_val, operand) {
                        return false;
                    }
                }
            }
            // Plain value: exact match
            expected => {
                if field_val != Some(expected) {
                    return false;
                }
            }
        }
    }
    true
}

fn apply_operator(op: &str, field: Option<&Value>, operand: &Value) -> bool {
    match op {
        "$eq" => field == Some(operand),
        "$ne" => field != Some(operand),
        "$exists" => {
            let want = operand.as_bool().unwrap_or(true);
            field.is_some() == want
        }
        "$gt" => compare(field, operand, |a, b| a > b),
        "$gte" => compare(field, operand, |a, b| a >= b),
        "$lt" => compare(field, operand, |a, b| a < b),
        "$lte" => compare(field, operand, |a, b| a <= b),
        "$in" => {
            if let (Some(val), Value::Array(arr)) = (field, operand) {
                arr.contains(val)
            } else {
                false
            }
        }
        "$nin" => {
            if let (Some(val), Value::Array(arr)) = (field, operand) {
                !arr.contains(val)
            } else {
                true
            }
        }
        _ => false,
    }
}

fn compare<F>(field: Option<&Value>, operand: &Value, cmp: F) -> bool
where
    F: Fn(f64, f64) -> bool,
{
    match (field, operand) {
        (Some(Value::Number(a)), Value::Number(b)) => {
            match (a.as_f64(), b.as_f64()) {
                (Some(av), Some(bv)) => cmp(av, bv),
                _ => false,
            }
        }
        (Some(Value::String(a)), Value::String(b)) => {
            // Lexicographic comparison for strings
            let av = a.as_str();
            let bv = b.as_str();
            cmp(
                av.len() as f64,
                bv.len() as f64,
            ) && cmp(
                av.bytes().map(|b| b as f64).sum::<f64>(),
                bv.bytes().map(|b| b as f64).sum::<f64>(),
            )
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_exact_match() {
        let meta = json!({ "role": "user" });
        assert!(matches(&meta, &json!({ "role": "user" })));
        assert!(!matches(&meta, &json!({ "role": "agent" })));
    }

    #[test]
    fn test_eq_operator() {
        let meta = json!({ "score": 5 });
        assert!(matches(&meta, &json!({ "score": { "$eq": 5 } })));
        assert!(!matches(&meta, &json!({ "score": { "$eq": 6 } })));
    }

    #[test]
    fn test_ne_operator() {
        let meta = json!({ "role": "user" });
        assert!(matches(&meta, &json!({ "role": { "$ne": "agent" } })));
        assert!(!matches(&meta, &json!({ "role": { "$ne": "user" } })));
    }

    #[test]
    fn test_gt_gte_lt_lte() {
        let meta = json!({ "score": 7.0 });
        assert!(matches(&meta, &json!({ "score": { "$gt": 5.0 } })));
        assert!(matches(&meta, &json!({ "score": { "$gte": 7.0 } })));
        assert!(matches(&meta, &json!({ "score": { "$lt": 10.0 } })));
        assert!(matches(&meta, &json!({ "score": { "$lte": 7.0 } })));
        assert!(!matches(&meta, &json!({ "score": { "$gt": 8.0 } })));
    }

    #[test]
    fn test_in_operator() {
        let meta = json!({ "role": "user" });
        assert!(matches(&meta, &json!({ "role": { "$in": ["user", "admin"] } })));
        assert!(!matches(&meta, &json!({ "role": { "$in": ["agent", "bot"] } })));
    }

    #[test]
    fn test_nin_operator() {
        let meta = json!({ "role": "user" });
        assert!(matches(&meta, &json!({ "role": { "$nin": ["agent", "bot"] } })));
        assert!(!matches(&meta, &json!({ "role": { "$nin": ["user", "admin"] } })));
    }

    #[test]
    fn test_exists_operator() {
        let meta = json!({ "role": "user" });
        assert!(matches(&meta, &json!({ "role": { "$exists": true } })));
        assert!(matches(&meta, &json!({ "missing": { "$exists": false } })));
        assert!(!matches(&meta, &json!({ "role": { "$exists": false } })));
    }

    #[test]
    fn test_multi_field_filter() {
        let meta = json!({ "role": "user", "ts": 1700000000 });
        assert!(matches(&meta, &json!({
            "role": "user",
            "ts": { "$gte": 1000000000 }
        })));
        assert!(!matches(&meta, &json!({
            "role": "user",
            "ts": { "$gt": 2000000000 }
        })));
    }
}
