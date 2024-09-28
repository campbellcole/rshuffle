{
  description = "";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem(system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
      in
      with pkgs;
      {
        devShells.default = pkgs.mkShell rec {
          nativeBuildInputs = [
            pkg-config
            clang
            cargo-msrv
          ];

          buildInputs = [
            (rust-bin.fromRustupToolchainFile ./rust-toolchain.toml)
          ];

          LD_LIBRARY_PATH = lib.makeLibraryPath buildInputs;

          RUST_BACKTRACE = 1;
          RUST_LOG = "rshuffle=trace,info";
        };
      }
    );
}