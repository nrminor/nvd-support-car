{
  description = "Development environment for nvd-support-car";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "nvd-support-car";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = with pkgs; [ pkg-config ];
          buildInputs =
            with pkgs;
            [ ]
            ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
              pkgs.darwin.apple_sdk.frameworks.Security
            ];
        };

        devShells.default = pkgs.mkShell {
          buildInputs =
            with pkgs;
            [
              rustup
              bacon
              pkg-config
              just
              pre-commit
              cargo-watch
              cargo-audit
              cargo-outdated
              cargo-binstall
              cargo-zigbuild
              zig

              # Benchmarking
              hyperfine

              # Python for benchmark visualization
              python313
              uv

              # Search and navigation
              ripgrep
              tree

              # TOML formatter
              taplo

              # Documentation
              mdbook

              # For direnv
              direnv
              nix-direnv
            ]
            ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
              darwin.apple_sdk.frameworks.Security
            ];

          shellHook = ''
            echo "nvd-support-car development environment"
            export RUST_BACKTRACE=1
            echo "Run 'just' to see available commands"
          '';
        };
      }
    );
}
