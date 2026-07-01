#[cfg(test)]
mod tests {
    use agentdb::AgentDB;
    use serde_json::json;
    use std::collections::HashMap;

    fn open() -> AgentDB {
        AgentDB::open(":memory:").expect("failed to open in-memory db")
    }

    // ── create_template ──────────────────────────────────────────────────────

    #[test]
    fn test_create_template_returns_id() {
        let db = open();
        let id = db
            .prompts()
            .create_template(
                "greeting",
                "Hello, {{name}}!",
                Some("claude-3-opus"),
                Some(4096),
                None,
            )
            .unwrap();
        assert!(!id.is_empty());
    }

    #[test]
    fn test_create_template_auto_increments_version() {
        let db = open();
        let prompts = db.prompts();
        prompts
            .create_template("sys", "You are v1", None, None, None)
            .unwrap();
        prompts
            .create_template("sys", "You are v2", None, None, None)
            .unwrap();
        prompts
            .create_template("sys", "You are v3", None, None, None)
            .unwrap();
        let latest = prompts.get_template("sys").unwrap();
        assert_eq!(latest.version, 3);
        assert_eq!(latest.template, "You are v3");
    }

    #[test]
    fn test_create_template_with_metadata() {
        let db = open();
        let meta = json!({"author": "alice", "tags": ["production"]});
        db.prompts()
            .create_template("prompt_a", "template body", None, None, Some(meta.clone()))
            .unwrap();
        let t = db.prompts().get_template("prompt_a").unwrap();
        assert_eq!(t.metadata.unwrap()["author"], "alice");
    }

    // ── get_template ─────────────────────────────────────────────────────────

    #[test]
    fn test_get_template_returns_latest_version() {
        let db = open();
        let prompts = db.prompts();
        prompts
            .create_template("tmpl", "old", None, None, None)
            .unwrap();
        prompts
            .create_template("tmpl", "new", None, None, None)
            .unwrap();
        let t = prompts.get_template("tmpl").unwrap();
        assert_eq!(t.template, "new");
        assert_eq!(t.version, 2);
    }

    #[test]
    fn test_get_template_not_found() {
        let db = open();
        let result = db.prompts().get_template("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_template_fields() {
        let db = open();
        db.prompts()
            .create_template(
                "qa",
                "Answer {{question}} using {{context}}",
                Some("gpt-4"),
                Some(2048),
                None,
            )
            .unwrap();
        let t = db.prompts().get_template("qa").unwrap();
        assert_eq!(t.name, "qa");
        assert_eq!(t.model_hint.as_deref(), Some("gpt-4"));
        assert_eq!(t.max_tokens, Some(2048));
        assert!(t.created_at > 0);
    }

    // ── get_template_version ─────────────────────────────────────────────────

    #[test]
    fn test_get_template_specific_version() {
        let db = open();
        let prompts = db.prompts();
        prompts
            .create_template("v_test", "version one", None, None, None)
            .unwrap();
        prompts
            .create_template("v_test", "version two", None, None, None)
            .unwrap();
        let v1 = prompts.get_template_version("v_test", 1).unwrap();
        let v2 = prompts.get_template_version("v_test", 2).unwrap();
        assert_eq!(v1.template, "version one");
        assert_eq!(v2.template, "version two");
    }

    #[test]
    fn test_get_template_version_not_found() {
        let db = open();
        db.prompts()
            .create_template("exists", "body", None, None, None)
            .unwrap();
        let result = db.prompts().get_template_version("exists", 99);
        assert!(result.is_err());
    }

    // ── list_templates ───────────────────────────────────────────────────────

    #[test]
    fn test_list_templates_empty() {
        let db = open();
        let list = db.prompts().list_templates().unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn test_list_templates_includes_all_versions() {
        let db = open();
        let prompts = db.prompts();
        prompts
            .create_template("alpha", "a1", None, None, None)
            .unwrap();
        prompts
            .create_template("alpha", "a2", None, None, None)
            .unwrap();
        prompts
            .create_template("beta", "b1", None, None, None)
            .unwrap();
        let list = prompts.list_templates().unwrap();
        assert_eq!(list.len(), 3);
    }

    // ── render ───────────────────────────────────────────────────────────────

    #[test]
    fn test_render_substitutes_placeholders() {
        let db = open();
        let prompts = db.prompts();
        prompts
            .create_template(
                "greet",
                "Hello {{name}}, welcome to {{place}}!",
                None,
                None,
                None,
            )
            .unwrap();
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "Alice".to_string());
        vars.insert("place".to_string(), "Wonderland".to_string());
        let result = prompts.render("greet", &vars).unwrap();
        assert_eq!(result, "Hello Alice, welcome to Wonderland!");
    }

    #[test]
    fn test_render_missing_placeholder_left_intact() {
        let db = open();
        let prompts = db.prompts();
        prompts
            .create_template(
                "partial",
                "Hi {{name}}, your id is {{id}}",
                None,
                None,
                None,
            )
            .unwrap();
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "Bob".to_string());
        let result = prompts.render("partial", &vars).unwrap();
        assert_eq!(result, "Hi Bob, your id is {{id}}");
    }

    #[test]
    fn test_render_uses_latest_version() {
        let db = open();
        let prompts = db.prompts();
        prompts
            .create_template("evolving", "Old: {{x}}", None, None, None)
            .unwrap();
        prompts
            .create_template("evolving", "New: {{x}}!", None, None, None)
            .unwrap();
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), "value".to_string());
        let result = prompts.render("evolving", &vars).unwrap();
        assert_eq!(result, "New: value!");
    }

    #[test]
    fn test_render_nonexistent_template_error() {
        let db = open();
        let vars = HashMap::new();
        let result = db.prompts().render("no_such", &vars);
        assert!(result.is_err());
    }

    // ── delete_template ──────────────────────────────────────────────────────

    #[test]
    fn test_delete_template_removes_all_versions() {
        let db = open();
        let prompts = db.prompts();
        prompts
            .create_template("doomed", "v1", None, None, None)
            .unwrap();
        prompts
            .create_template("doomed", "v2", None, None, None)
            .unwrap();
        prompts.delete_template("doomed").unwrap();
        assert!(prompts.get_template("doomed").is_err());
        assert!(prompts.get_template_version("doomed", 1).is_err());
    }

    #[test]
    fn test_delete_nonexistent_template_no_error() {
        let db = open();
        db.prompts().delete_template("ghost").unwrap();
    }

    // ── stats integration ────────────────────────────────────────────────────

    #[test]
    fn test_stats_counts_prompt_templates() {
        let db = open();
        let prompts = db.prompts();
        prompts
            .create_template("a", "body", None, None, None)
            .unwrap();
        prompts
            .create_template("a", "body v2", None, None, None)
            .unwrap();
        prompts
            .create_template("b", "other", None, None, None)
            .unwrap();
        let stats = db.stats().unwrap();
        assert_eq!(stats.prompt_templates, 3);
    }
}
