# frozen_string_literal: true

require_relative "lib/agentdb/version"

Gem::Specification.new do |spec|
  spec.name    = "agentdb"
  spec.version = AgentDB::VERSION
  spec.authors = ["AgentDB Contributors"]
  spec.email   = []

  spec.summary     = "Ruby SDK for AgentDB — an AI-native embedded database"
  spec.description = <<~DESC
    AgentDB is a single-file embedded database for AI agents. It combines
    SQLite-backed relational storage with HNSW vector search, full-text search,
    hybrid graph+vector queries, memory graphs, conversation history, workflow
    state machines, and reasoning traces. This gem wraps the C shared library
    via the ffi gem.
  DESC

  spec.homepage = "https://github.com/hvrcharon1/agentdb"
  spec.license  = "Unlicense"

  spec.required_ruby_version = ">= 2.7.0"

  spec.metadata = {
    "homepage_uri"    => spec.homepage,
    "source_code_uri" => spec.homepage,
    "bug_tracker_uri" => "#{spec.homepage}/issues",
  }

  # Only ship files tracked by git or explicitly listed.
  spec.files = Dir.glob(
    %w[
      lib/**/*.rb
      agentdb.gemspec
      Gemfile
      README.md
    ]
  ).select { |f| File.file?(f) }

  spec.require_paths = ["lib"]

  # Runtime dependency: the ffi gem for loading the C shared library.
  spec.add_dependency "ffi", "~> 1.15"

  # Development dependencies.
  spec.add_development_dependency "rspec", "~> 3.0"
  spec.add_development_dependency "rake",  "~> 13.0"
end
