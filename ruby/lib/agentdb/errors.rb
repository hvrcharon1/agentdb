# frozen_string_literal: true

module AgentDB
  # Base error class for all AgentDB exceptions.
  class Error < StandardError; end

  # Raised when the native library cannot be loaded.
  class LibraryNotFoundError < Error; end

  # Raised when an FFI call returns a NULL pointer or error code.
  class FFIError < Error; end

  # Raised when a database cannot be opened.
  class DatabaseError < Error; end
end
