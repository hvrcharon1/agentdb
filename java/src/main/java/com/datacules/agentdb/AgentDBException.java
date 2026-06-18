package com.datacules.agentdb;

/**
 * Thrown when an AgentDB native operation fails.
 *
 * <p>The message is taken directly from {@code agentdb_last_error()} on the
 * calling thread, so it describes the actual native failure.
 */
public class AgentDBException extends RuntimeException {

    private static final long serialVersionUID = 1L;

    public AgentDBException(String message) {
        super(message);
    }

    public AgentDBException(String message, Throwable cause) {
        super(message, cause);
    }
}
