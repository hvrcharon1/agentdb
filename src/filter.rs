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
            Value::Object(ops) => {
                for (op, operand) in ops {
                    if !apply_operator(op, field_val, operand) {
                        return false;
                    }
                }
            }
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
        "$gt" => compare_num(field, operand, |a, b| a > b),
        "$gte" => compare_num(field, operand, |a, b| a >= b),
        "$lt" => compare_num(field, operand, |a, b| a < b),
        "$lte" => compare_num(field, operand, |a, b| a <= b),
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

fn compare_num<F>(field: Option<&Value>, operand: &Value, cmp: F) -> bool
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
