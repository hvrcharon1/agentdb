#[cfg(test)]
mod tests {
    use agentdb::AgentDB;
    use serde_json::json;

    fn open() -> AgentDB {
        AgentDB::open(":memory:").expect("failed to open in-memory db")
    }

    // ── create_conversation ───────────────────────────────────────────────────

    #[test]
    fn test_create_conversation_and_list() {
        let db = open();
        let convs = db.conversations();
        convs
            .create_conversation("conv-1", Some("Hello World"), None)
            .unwrap();
        let list = convs.list_conversations().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "conv-1");
        assert_eq!(list[0].title.as_deref(), Some("Hello World"));
    }

    #[test]
    fn test_create_conversation_with_metadata() {
        let db = open();
        let convs = db.conversations();
        convs
            .create_conversation("conv-meta", None, Some(json!({ "agent": "gpt" })))
            .unwrap();
        let list = convs.list_conversations().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].metadata.as_ref().unwrap()["agent"], "gpt");
    }

    #[test]
    fn test_create_conversation_no_title() {
        let db = open();
        let convs = db.conversations();
        convs
            .create_conversation("conv-notitle", None, None)
            .unwrap();
        let list = convs.list_conversations().unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].title.is_none());
    }

    // ── list_conversations ────────────────────────────────────────────────────

    #[test]
    fn test_list_conversations_empty() {
        let db = open();
        let list = db.conversations().list_conversations().unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn test_list_conversations_multiple() {
        let db = open();
        let convs = db.conversations();
        convs
            .create_conversation("c1", Some("First"), None)
            .unwrap();
        convs
            .create_conversation("c2", Some("Second"), None)
            .unwrap();
        convs
            .create_conversation("c3", Some("Third"), None)
            .unwrap();
        let list = convs.list_conversations().unwrap();
        assert_eq!(list.len(), 3);
    }

    // ── add_message / get_messages ────────────────────────────────────────────

    #[test]
    fn test_add_message_returns_id() {
        let db = open();
        let convs = db.conversations();
        convs.create_conversation("conv-1", None, None).unwrap();
        let msg_id = convs.add_message("conv-1", "user", "Hello!", None).unwrap();
        assert!(!msg_id.is_empty());
    }

    #[test]
    fn test_add_and_get_messages() {
        let db = open();
        let convs = db.conversations();
        convs.create_conversation("conv-1", None, None).unwrap();
        convs
            .add_message("conv-1", "system", "You are helpful.", None)
            .unwrap();
        convs
            .add_message("conv-1", "user", "What is 2 + 2?", None)
            .unwrap();
        convs
            .add_message("conv-1", "assistant", "4.", None)
            .unwrap();
        let messages = convs.get_messages("conv-1", None).unwrap();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1].role, "user");
        assert_eq!(messages[2].role, "assistant");
    }

    #[test]
    fn test_get_messages_content() {
        let db = open();
        let convs = db.conversations();
        convs.create_conversation("conv-1", None, None).unwrap();
        convs
            .add_message("conv-1", "user", "Tell me a joke.", None)
            .unwrap();
        let messages = convs.get_messages("conv-1", None).unwrap();
        assert_eq!(messages[0].content, "Tell me a joke.");
        assert_eq!(messages[0].conversation_id, "conv-1");
    }

    #[test]
    fn test_get_messages_with_metadata() {
        let db = open();
        let convs = db.conversations();
        convs.create_conversation("conv-1", None, None).unwrap();
        convs
            .add_message("conv-1", "user", "Hi", Some(json!({ "tokens": 3 })))
            .unwrap();
        let messages = convs.get_messages("conv-1", None).unwrap();
        assert_eq!(messages[0].metadata.as_ref().unwrap()["tokens"], 3);
    }

    #[test]
    fn test_get_messages_limit() {
        let db = open();
        let convs = db.conversations();
        convs.create_conversation("conv-1", None, None).unwrap();
        for i in 0..5 {
            convs
                .add_message("conv-1", "user", &format!("msg {i}"), None)
                .unwrap();
        }
        // Limit to 2: should return exactly 2 messages from the conversation.
        let messages = convs.get_messages("conv-1", Some(2)).unwrap();
        assert_eq!(messages.len(), 2);
        // Verify all returned messages belong to this conversation.
        let contents: Vec<&str> = messages.iter().map(|m| m.content.as_str()).collect();
        for c in &contents {
            assert!(c.starts_with("msg "), "unexpected content: {c}");
        }
    }

    #[test]
    fn test_get_messages_chronological_order() {
        let db = open();
        let convs = db.conversations();
        convs.create_conversation("conv-1", None, None).unwrap();
        convs.add_message("conv-1", "user", "first", None).unwrap();
        convs
            .add_message("conv-1", "assistant", "second", None)
            .unwrap();
        let messages = convs.get_messages("conv-1", None).unwrap();
        assert!(messages[0].created_at <= messages[1].created_at);
    }

    // ── edge cases: empty conversation and nonexistent conversation ───────────

    #[test]
    fn test_get_messages_empty_conversation() {
        let db = open();
        let convs = db.conversations();
        convs.create_conversation("conv-empty", None, None).unwrap();
        let messages = convs.get_messages("conv-empty", None).unwrap();
        assert!(messages.is_empty());
    }

    #[test]
    fn test_get_messages_nonexistent_conversation() {
        let db = open();
        // Querying messages for a conversation that doesn't exist should return
        // an empty vec (no error), because there simply are no matching rows.
        let messages = db.conversations().get_messages("ghost", None).unwrap();
        assert!(messages.is_empty());
    }

    // ── delete_conversation ───────────────────────────────────────────────────

    #[test]
    fn test_delete_conversation_removes_it() {
        let db = open();
        let convs = db.conversations();
        convs
            .create_conversation("conv-1", Some("Temp"), None)
            .unwrap();
        assert_eq!(convs.list_conversations().unwrap().len(), 1);
        convs.delete_conversation("conv-1").unwrap();
        assert!(convs.list_conversations().unwrap().is_empty());
    }

    #[test]
    fn test_delete_conversation_cascades_messages() {
        let db = open();
        let convs = db.conversations();
        convs.create_conversation("conv-1", None, None).unwrap();
        convs
            .add_message("conv-1", "user", "Will be deleted.", None)
            .unwrap();
        convs.delete_conversation("conv-1").unwrap();
        // After deletion the conversation is gone and its messages should
        // no longer be returned (empty result, not an error).
        let messages = convs.get_messages("conv-1", None).unwrap();
        assert!(messages.is_empty());
    }

    #[test]
    fn test_delete_nonexistent_conversation_is_ok() {
        let db = open();
        // Deleting an ID that does not exist should not return an error.
        db.conversations()
            .delete_conversation("no-such-id")
            .unwrap();
    }

    // ── add_message bumps updated_at ──────────────────────────────────────────

    #[test]
    fn test_add_message_bumps_updated_at() {
        let db = open();
        let convs = db.conversations();
        convs.create_conversation("conv-1", None, None).unwrap();
        let before = convs.list_conversations().unwrap()[0].updated_at;
        // Sleep briefly so the timestamp can advance on fast machines.
        std::thread::sleep(std::time::Duration::from_millis(5));
        convs.add_message("conv-1", "user", "bump", None).unwrap();
        let after = convs.list_conversations().unwrap()[0].updated_at;
        assert!(after >= before);
    }
}
