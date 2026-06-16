'use strict';

// Platform-aware loader for the pre-built native addon.
// When running from source (npm run build), the .node file sits in
// the crate root. When installed from npm, napi-rs CLI places
// platform-specific binaries as optional dependencies and this
// file selects the correct one at runtime.

const { platform, arch } = process;

const PLATFORM_MAP = {
  'linux-x64':   'agentdb.linux-x64-gnu.node',
  'linux-arm64': 'agentdb.linux-arm64-gnu.node',
  'darwin-x64':  'agentdb.darwin-x64.node',
  'darwin-arm64':'agentdb.darwin-arm64.node',
  'win32-x64':   'agentdb.win32-x64-msvc.node',
};

function loadNative() {
  const key = `${platform}-${arch}`;
  const filename = PLATFORM_MAP[key];

  // 1. Try the pre-built platform-specific file (npm install path).
  if (filename) {
    try { return require(`./${filename}`); } catch (_) {}
    // Scoped platform package published by napi-rs CLI alongside @datacules/agentdb
    try { return require(`@datacules/agentdb-${key}`); } catch (_) {}
  }

  // 2. Fall back to a local debug build (cargo build / napi build --debug).
  try { return require('./agentdb.node'); } catch (_) {}

  throw new Error(
    `AgentDB: no native addon found for ${key}.\n` +
    `Run \`npm run build\` in the nodejs/ directory to compile from source,\n` +
    `or install a pre-built package from npm: npm install @datacules/agentdb`
  );
}

module.exports = loadNative();
