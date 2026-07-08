"""AgentDB-backed LangChain chat memory implementation."""

from __future__ import annotations

import uuid
from typing import Any, Dict, List, Optional, Sequence

from langchain_core.chat_history import BaseChatMessageHistory
from langchain_core.messages import (
    AIMessage,
    BaseMessage,
    HumanMessage,
    SystemMessage,
    messages_from_dict,
    messages_to_dict,
)


# Role constants used when persisting messages to AgentDB.
_ROLE_HUMAN = "human"
_ROLE_AI = "ai"
_ROLE_SYSTEM = "system"

_LANGCHAIN_ROLE_MAP = {
    "human": _ROLE_HUMAN,
    "ai": _ROLE_AI,
    "system": _ROLE_SYSTEM,
}

_AGENTDB_TO_LC_ROLE: Dict[str, str] = {v: k for k, v in _LANGCHAIN_ROLE_MAP.items()}


def _message_to_role(message: BaseMessage) -> str:
    """Map a LangChain message type to an AgentDB role string."""
    if isinstance(message, HumanMessage):
        return _ROLE_HUMAN
    if isinstance(message, AIMessage):
        return _ROLE_AI
    if isinstance(message, SystemMessage):
        return _ROLE_SYSTEM
    # Fall back to the ``type`` field for any custom message subclasses.
    return message.type


def _row_to_message(row: Dict[str, Any]) -> BaseMessage:
    """Reconstruct a LangChain ``BaseMessage`` from an AgentDB message row."""
    role = row.get("role", "")
    content = row.get("content", "")
    if role == _ROLE_HUMAN:
        return HumanMessage(content=content)
    if role == _ROLE_AI:
        return AIMessage(content=content)
    if role == _ROLE_SYSTEM:
        return SystemMessage(content=content)
    # Unknown roles: surface as HumanMessage so the conversation is not lost.
    return HumanMessage(content=content)


class AgentDBChatMessageHistory(BaseChatMessageHistory):
    """Persistent chat message history backed by AgentDB conversations.

    Each ``AgentDBChatMessageHistory`` instance corresponds to a single
    *conversation* in the AgentDB database.  Messages are stored with their
    role (``"human"``, ``"ai"``, or ``"system"``) and retrieved in insertion
    order.

    This class implements the ``BaseChatMessageHistory`` interface introduced
    in ``langchain-core >= 0.2`` and is the preferred way to plug AgentDB
    into ``RunnableWithMessageHistory`` chains.

    Args:
        db_path: Path to the AgentDB database file.
        conversation_id: A unique identifier for this conversation thread.
            A new UUID is generated automatically when not provided.
        title: Optional human-readable title stored on first creation.

    Example::

        from langchain_agentdb import AgentDBChatMessageHistory
        from langchain_core.runnables.history import RunnableWithMessageHistory

        history = AgentDBChatMessageHistory(
            db_path="agent.agentdb",
            conversation_id="session-42",
        )
        history.add_user_message("What is AgentDB?")
        history.add_ai_message("AgentDB is a fast embedded AI database.")
        print(history.messages)
    """

    def __init__(
        self,
        db_path: str,
        conversation_id: Optional[str] = None,
        title: Optional[str] = None,
    ) -> None:
        try:
            import agentdb as _agentdb
        except ImportError as exc:  # pragma: no cover
            raise ImportError(
                "The 'datacules-agentdb' package is required.  "
                "Install it with: pip install datacules-agentdb"
            ) from exc

        self._db_path = db_path
        self._conversation_id = conversation_id or str(uuid.uuid4())
        self._title = title

        self._db = _agentdb.AgentDB.open(db_path)

        # Ensure the conversation exists in the database.  The call is
        # idempotent — AgentDB will not raise if it already exists.
        try:
            self._db.create_conversation(
                self._conversation_id,
                title=self._title,
            )
        except RuntimeError:
            # Conversation may already exist; that is fine.
            pass

    # ------------------------------------------------------------------
    # BaseChatMessageHistory interface
    # ------------------------------------------------------------------

    @property
    def messages(self) -> List[BaseMessage]:
        """Return all messages in this conversation in chronological order."""
        rows = self._db.get_messages(self._conversation_id)
        return [_row_to_message(row) for row in rows]

    def add_message(self, message: BaseMessage) -> None:
        """Persist a single ``BaseMessage`` to the conversation.

        Args:
            message: The message to store.
        """
        role = _message_to_role(message)
        content = message.content if isinstance(message.content, str) else str(message.content)
        self._db.add_message(
            self._conversation_id,
            role,
            content,
        )

    def add_messages(self, messages: Sequence[BaseMessage]) -> None:
        """Persist multiple messages in one call.

        Args:
            messages: Sequence of messages to store.
        """
        for message in messages:
            self.add_message(message)

    def clear(self) -> None:
        """Delete all messages by removing and re-creating the conversation."""
        self._db.delete_conversation(self._conversation_id)
        self._db.create_conversation(
            self._conversation_id,
            title=self._title,
        )

    # ------------------------------------------------------------------
    # Convenience helpers
    # ------------------------------------------------------------------

    def add_user_message(self, message: str) -> None:  # type: ignore[override]
        """Append a human/user message.

        Args:
            message: The user's message text.
        """
        self.add_message(HumanMessage(content=message))

    def add_ai_message(self, message: str) -> None:  # type: ignore[override]
        """Append an AI/assistant message.

        Args:
            message: The assistant's message text.
        """
        self.add_message(AIMessage(content=message))

    @property
    def conversation_id(self) -> str:
        """The AgentDB conversation ID for this history."""
        return self._conversation_id

    def __len__(self) -> int:
        return len(self.messages)

    def __repr__(self) -> str:
        return (
            f"AgentDBChatMessageHistory("
            f"db_path={self._db_path!r}, "
            f"conversation_id={self._conversation_id!r})"
        )


# ---------------------------------------------------------------------------
# Legacy BaseMemory wrapper
# ---------------------------------------------------------------------------
# LangChain's ``BaseMemory`` interface is still widely used in LCEL chains
# that pre-date ``RunnableWithMessageHistory``.  We expose a thin wrapper so
# users on older chains don't have to migrate.

try:
    from langchain_core.memory import BaseMemory  # type: ignore[attr-defined]

    class AgentDBChatMemory(BaseMemory):
        """LangChain ``BaseMemory`` backed by AgentDB.

        Wraps ``AgentDBChatMessageHistory`` and exposes the ``load_memory_variables``
        / ``save_context`` interface required by ``BaseMemory``.

        Args:
            db_path: Path to the AgentDB database file.
            conversation_id: Unique conversation identifier.
            memory_key: Key under which chat history is returned by
                ``load_memory_variables``.  Defaults to ``"history"``.
            input_key: The chain input key to treat as the human turn.
                Defaults to ``"input"``.
            output_key: The chain output key to treat as the AI turn.
                Defaults to ``"output"``.
            return_messages: When ``True`` the ``"history"`` value is a list of
                ``BaseMessage`` objects; when ``False`` (default) it is a
                formatted string.
        """

        db_path: str
        conversation_id: str = ""
        memory_key: str = "history"
        input_key: str = "input"
        output_key: str = "output"
        return_messages: bool = False

        class Config:
            arbitrary_types_allowed = True

        def __init__(self, **data: Any) -> None:
            if not data.get("conversation_id"):
                data["conversation_id"] = str(uuid.uuid4())
            super().__init__(**data)
            object.__setattr__(
                self,
                "_history",
                AgentDBChatMessageHistory(
                    db_path=self.db_path,
                    conversation_id=self.conversation_id,
                ),
            )

        @property
        def memory_variables(self) -> List[str]:
            return [self.memory_key]

        def load_memory_variables(self, inputs: Dict[str, Any]) -> Dict[str, Any]:
            messages = self._history.messages  # type: ignore[attr-defined]
            if self.return_messages:
                return {self.memory_key: messages}
            # Format as a simple human/AI transcript string.
            lines = []
            for msg in messages:
                prefix = "Human" if isinstance(msg, HumanMessage) else "AI"
                lines.append(f"{prefix}: {msg.content}")
            return {self.memory_key: "\n".join(lines)}

        def save_context(
            self, inputs: Dict[str, Any], outputs: Dict[str, Any]
        ) -> None:
            human_text = inputs.get(self.input_key, "")
            ai_text = outputs.get(self.output_key, "")
            if human_text:
                self._history.add_user_message(str(human_text))  # type: ignore[attr-defined]
            if ai_text:
                self._history.add_ai_message(str(ai_text))  # type: ignore[attr-defined]

        def clear(self) -> None:
            self._history.clear()  # type: ignore[attr-defined]

except ImportError:
    # langchain_core version does not expose BaseMemory — skip the legacy class.
    AgentDBChatMemory = None  # type: ignore[assignment,misc]
