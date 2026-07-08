"""AgentDB-backed LlamaIndex ChatStore implementation."""

from __future__ import annotations

import uuid
from typing import Any, Dict, List, Optional

from llama_index.core.llms import ChatMessage, MessageRole
from llama_index.core.storage.chat_store.base import BaseChatStore


# ---------------------------------------------------------------------------
# Role mapping helpers
# ---------------------------------------------------------------------------

def _llama_role_to_str(role: MessageRole) -> str:
    """Convert a LlamaIndex ``MessageRole`` to an AgentDB role string."""
    return role.value if hasattr(role, "value") else str(role)


def _str_to_llama_role(role_str: str) -> MessageRole:
    """Convert an AgentDB role string back to a LlamaIndex ``MessageRole``."""
    try:
        return MessageRole(role_str)
    except ValueError:
        # Fall back to USER for unrecognised roles so content is not lost.
        return MessageRole.USER


def _row_to_chat_message(row: Dict[str, Any]) -> ChatMessage:
    """Reconstruct a ``ChatMessage`` from an AgentDB message row dict."""
    role = _str_to_llama_role(row.get("role", "user"))
    content = row.get("content", "")
    # Restore any extra kwargs stored in message metadata.
    additional = row.get("metadata") or {}
    return ChatMessage(role=role, content=content, additional_kwargs=additional)


class AgentDBChatStore(BaseChatStore):
    """A LlamaIndex ``BaseChatStore`` backed by AgentDB conversations.

    Each *key* passed to this store maps to one AgentDB conversation.  Messages
    are persisted with their role and content and retrieved in insertion order.

    AgentDB supports the following roles out of the box: ``"user"``,
    ``"assistant"``, ``"system"``, and ``"tool"``.  Any ``MessageRole`` value
    is accepted and stored as its string representation.

    Args:
        db_path: Path to the AgentDB database file (created if absent).

    Example::

        from llamaindex_agentdb import AgentDBChatStore
        from llama_index.core.llms import ChatMessage, MessageRole

        chat_store = AgentDBChatStore(db_path="agent.agentdb")

        chat_store.add_message(
            "session-1",
            ChatMessage(role=MessageRole.USER, content="Hello!"),
        )
        messages = chat_store.get_messages("session-1")
        for msg in messages:
            print(msg.role, msg.content)
    """

    # -----------------------------------------------------------------------
    # Pydantic / BaseChatStore plumbing
    # -----------------------------------------------------------------------

    # BaseChatStore (LlamaIndex >= 0.11) is itself a Pydantic BaseModel.
    # We store the db_path as a proper field so it survives serialisation and
    # the ``model_json_schema`` / ``class_name`` machinery works correctly.

    db_path: str = ""

    # Private attributes — managed via object.__setattr__ to avoid Pydantic
    # field conflicts.
    _db: Any

    def __init__(self, db_path: str, **kwargs: Any) -> None:
        try:
            import agentdb as _agentdb
        except ImportError as exc:  # pragma: no cover
            raise ImportError(
                "The 'datacules-agentdb' package is required.  "
                "Install it with: pip install datacules-agentdb"
            ) from exc

        super().__init__(db_path=db_path, **kwargs)
        object.__setattr__(self, "_db", _agentdb.AgentDB.open(db_path))

    @classmethod
    def class_name(cls) -> str:
        return "AgentDBChatStore"

    # -----------------------------------------------------------------------
    # Core interface — BaseChatStore abstract methods
    # -----------------------------------------------------------------------

    def set_messages(self, key: str, messages: List[ChatMessage]) -> None:
        """Replace all messages for ``key`` with the given list.

        The existing conversation is deleted and re-created so the new messages
        become the canonical history.

        Args:
            key: Conversation key / session identifier.
            messages: Replacement message list.
        """
        # Delete existing conversation (and its messages) if present.
        try:
            self._db.delete_conversation(key)
        except Exception:
            pass

        self._db.create_conversation(key)
        for message in messages:
            self._db.add_message(
                key,
                _llama_role_to_str(message.role),
                message.content if isinstance(message.content, str) else str(message.content),
            )

    def get_messages(self, key: str) -> List[ChatMessage]:
        """Retrieve all messages for ``key`` in chronological order.

        Returns an empty list when the key does not exist.

        Args:
            key: Conversation key / session identifier.

        Returns:
            List of ``ChatMessage`` objects.
        """
        try:
            rows = self._db.get_messages(key)
        except Exception:
            return []
        return [_row_to_chat_message(row) for row in rows]

    def add_message(
        self,
        key: str,
        message: ChatMessage,
        idx: Optional[int] = None,
    ) -> None:
        """Append a single message to the conversation identified by ``key``.

        The conversation is created automatically if it does not yet exist.
        The ``idx`` parameter is accepted for interface compatibility but
        ignored — AgentDB always appends in insertion order.

        Args:
            key: Conversation key / session identifier.
            message: The ``ChatMessage`` to store.
            idx: Ignored.  Present for ``BaseChatStore`` interface compatibility.
        """
        # Ensure the conversation exists; silently ignore if already present.
        try:
            self._db.create_conversation(key)
        except Exception:
            pass

        content = message.content if isinstance(message.content, str) else str(message.content)
        self._db.add_message(
            key,
            _llama_role_to_str(message.role),
            content,
        )

    def delete_messages(self, key: str) -> Optional[List[ChatMessage]]:
        """Delete all messages for ``key`` and return them.

        The conversation record itself is also removed from the database.

        Args:
            key: Conversation key / session identifier.

        Returns:
            The list of messages that were stored, or ``None`` if the key did
            not exist.
        """
        try:
            messages = self.get_messages(key)
            self._db.delete_conversation(key)
            return messages if messages else None
        except Exception:
            return None

    def delete_message(self, key: str, idx: int) -> Optional[ChatMessage]:
        """Delete the message at position ``idx`` and return it.

        Because AgentDB does not support positional deletes, this method loads
        the full message list, removes the message at ``idx``, and replaces the
        conversation with the remainder.

        Args:
            key: Conversation key / session identifier.
            idx: Zero-based index of the message to delete.

        Returns:
            The deleted ``ChatMessage``, or ``None`` when ``idx`` is out of range.
        """
        messages = self.get_messages(key)
        if idx < 0 or idx >= len(messages):
            return None
        removed = messages.pop(idx)
        self.set_messages(key, messages)
        return removed

    def delete_last_message(self, key: str) -> Optional[ChatMessage]:
        """Remove and return the last message in the conversation.

        Args:
            key: Conversation key / session identifier.

        Returns:
            The removed ``ChatMessage``, or ``None`` when the conversation is
            empty or does not exist.
        """
        messages = self.get_messages(key)
        if not messages:
            return None
        return self.delete_message(key, len(messages) - 1)

    def get_keys(self) -> List[str]:
        """Return all conversation keys stored in this database.

        Returns:
            List of conversation ID strings.
        """
        try:
            convos = self._db.list_conversations()
            return [c["id"] for c in convos]
        except Exception:
            return []

    # -----------------------------------------------------------------------
    # Utility
    # -----------------------------------------------------------------------

    def __repr__(self) -> str:
        return f"AgentDBChatStore(db_path={self.db_path!r})"
