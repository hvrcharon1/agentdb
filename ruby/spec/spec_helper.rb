# frozen_string_literal: true

require "agentdb"

RSpec.configure do |config|
  # Use the documentation formatter for output
  config.formatter = :documentation

  # Allow only the new "expect" syntax
  config.expect_with :rspec do |expectations|
    expectations.include_chain_clauses_in_custom_matcher_descriptions = true
  end

  config.mock_with :rspec do |mocks|
    mocks.verify_partial_doubles = true
  end

  config.shared_context_metadata_behavior = :apply_to_host_groups

  # Run specs in random order to surface order-dependent failures.
  config.order = :random
  Kernel.srand config.seed

  # ── Shared helpers ─────────────────────────────────────────────────────

  # Open a temporary in-memory database for the duration of an example.
  def with_db
    db = AgentDB::Database.new(":memory:")
    yield db
  ensure
    db&.close
  end

  # Generate a normalised random embedding of the given dimensionality.
  def random_embedding(dim = 4)
    raw = Array.new(dim) { rand(-1.0..1.0) }
    mag = Math.sqrt(raw.sum { |x| x**2 })
    mag > 0 ? raw.map { |x| x / mag } : raw
  end
end
