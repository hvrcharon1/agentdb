{
  description = "AgentDB — single-file embedded database for AI agents";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustToolchain = pkgs.rust-bin.stable.latest.default;
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "agentdb";
          version = "0.3.3";

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          buildAndTestSubdir = ".";

          # Only build the CLI binary, skip python/nodejs sub-crates
          cargoBuildFlags = [ "--package" "datacules-agentdb" "--bin" "agentdb" ];
          cargoTestFlags = [ "--package" "datacules-agentdb" ];

          meta = with pkgs.lib; {
            description = "Single-file embedded database for AI agents — SQL + Vector Search + FTS + Graphs";
            homepage = "https://github.com/hvrcharon1/agentdb";
            license = licenses.unlicense;
            maintainers = [ ];
            mainProgram = "agentdb";
          };
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [
            rustToolchain
            pkgs.rust-analyzer
          ];
        };
      });
}
