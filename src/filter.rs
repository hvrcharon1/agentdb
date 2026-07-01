use serde_json::Value;

/// Evaluate a metadata filter against a JSON document.
///
/// Supports exact match, comparison, and logical operators:
/// `$eq`, `$ne`, `$gt`, `$gte`, `$lt`, `$lte`, `$in`, `$nin`, `$exists`,
/// `$contains` (substring match), `$regex` (regex pattern match),
/// `$and`, `$or`, `$not`.
///
/// Field paths support dot notation: `{ "user.name": { "$eq": "alice" } }`.
pub fn matches(metadata: &Value, filter: &Value) -> bool {
    let filter_obj = match filter {
        Value::Object(f) => f,
        _ => return false,
    };
    for (key, condition) in filter_obj {
        match key.as_str() {
            "$and" => {
                if let Value::Array(clauses) = condition {
                    if !clauses.iter().all(|c| matches(metadata, c)) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            "$or" => {
                if let Value::Array(clauses) = condition {
                    if !clauses.iter().any(|c| matches(metadata, c)) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            "$not" => {
                if matches(metadata, condition) {
                    return false;
                }
            }
            field => {
                let field_value = get_nested(metadata, field);
                match condition {
                    Value::Object(ops) => {
                        for (op, operand) in ops {
                            if !apply_op(op, field_value, operand) {
                                return false;
                            }
                        }
                    }
                    expected => {
                        if field_value != Some(expected) {
                            return false;
                        }
                    }
                }
            }
        }
    }
    true
}

/// Resolve a possibly dot-separated field path into the nested JSON value.
fn get_nested<'a>(val: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = val;
    for segment in path.split('.') {
        current = current.get(segment)?;
    }
    Some(current)
}

fn apply_op(op: &str, field: Option<&Value>, operand: &Value) -> bool {
    match op {
        "$eq" => field == Some(operand),
        "$ne" => field != Some(operand),
        "$exists" => field.is_some() == operand.as_bool().unwrap_or(true),
        "$gt" => cmp_num(field, operand, |a, b| a > b),
        "$gte" => cmp_num(field, operand, |a, b| a >= b),
        "$lt" => cmp_num(field, operand, |a, b| a < b),
        "$lte" => cmp_num(field, operand, |a, b| a <= b),
        "$in" => match (field, operand) {
            (Some(v), Value::Array(arr)) => arr.contains(v),
            _ => false,
        },
        "$nin" => match (field, operand) {
            (Some(v), Value::Array(arr)) => !arr.contains(v),
            _ => true,
        },
        // Substring containment check (literal, not a regex pattern).
        "$contains" => match (field, operand.as_str()) {
            (Some(Value::String(s)), Some(pattern)) => s.contains(pattern),
            _ => false,
        },
        // Regex pattern match. Invalid patterns never match (returns false).
        "$regex" => match (field, operand.as_str()) {
            (Some(Value::String(s)), Some(pattern)) => regex::Regex::new(pattern)
                .map(|re| re.is_match(s))
                .unwrap_or(false),
            _ => false,
        },
        _ => false,
    }
}

fn cmp_num<F>(field: Option<&Value>, operand: &Value, cmp: F) -> bool
where
    F: Fn(f64, f64) -> bool,
{
    match (field, operand) {
        (Some(Value::Number(a)), Value::Number(b)) => match (a.as_f64(), b.as_f64()) {
            (Some(av), Some(bv)) => cmp(av, bv),
            _ => false,
        },
        _ => false,
    }
}
