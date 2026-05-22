use serde_json::Value;

/// Evaluate a metadata filter against a JSON document.
///
/// Supports:
/// - Exact match: `{ "key": value }`
/// - `$eq`, `$ne`
/// - `$gt`, `$gte`, `$lt`, `$lte` (numeric)
/// - `$in`, `$nin` (array membership)
/// - `$exists` (field presence)
pub fn matches(metadata: &Value, filter: &Value) -> bool {
    let (meta_obj, filter_obj) = match (metadata, filter) {
        (Value::Object(m), Value::Object(f)) => (m, f),
        _ => return false,
    };
    for (key, condition) in filter_obj {
        let field = meta_obj.get(key);
        match condition {
            Value::Object(ops) => {
                for (op, operand) in ops {
                    if !apply_op(op, field, operand) {
                        return false;
                    }
                }
            }
            expected => {
                if field != Some(expected) {
                    return false;
                }
            }
        }
    }
    true
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
        _ => false,
    }
}

fn cmp_num<F>(field: Option<&Value>, operand: &Value, cmp: F) -> bool
where
    F: Fn(f64, f64) -> bool,
{
    match (field, operand) {
        (Some(Value::Number(a)), Value::Number(b)) => {
            matches!((a.as_f64(), b.as_f64()), (Some(av), Some(bv)) if cmp(av, bv))
        }
        _ => false,
    }
}
