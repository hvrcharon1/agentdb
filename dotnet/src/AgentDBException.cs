using System;

namespace Datacules.AgentDB
{
    /// <summary>
    /// The exception thrown when an AgentDB native operation fails.
    /// The <see cref="Exception.Message"/> property contains the text returned
    /// by <c>agentdb_last_error()</c> on the calling thread.
    /// </summary>
    public class AgentDBException : Exception
    {
        /// <inheritdoc />
        public AgentDBException(string message) : base(message) { }

        /// <inheritdoc />
        public AgentDBException(string message, Exception innerException)
            : base(message, innerException) { }
    }
}
