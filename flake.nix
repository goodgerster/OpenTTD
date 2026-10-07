{
  description = "OpenTTD (JGR's patchpack fork, C++/Rust) development shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-26.05-darwin";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { nixpkgs, rust-overlay, ... }:
    let
      # Apple silicon is the supported macOS platform; x86_64-linux mirrors the cloud sessions.
      systems = [ "aarch64-darwin" "x86_64-linux" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f (import nixpkgs {
        inherit system;
        overlays = [ rust-overlay.overlays.default ];
      }));
    in
    {
      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            # Build tools
            cmake
            ninja
            ccache
            pkg-config
            python3

            # The Rust toolchain pinned in rust-toolchain.toml, and the dependency policy checker
            (rust-bin.fromRustupToolchainFile ./rust-toolchain.toml)
            cargo-deny

            # Optional libraries the game uses (see COMPILING.md)
            zlib
            xz
            libpng
            zstd
            lzo
            curl
          ] ++ lib.optionals stdenv.hostPlatform.isLinux [
            # The cloud sessions build headless (-DOPTION_DEDICATED=ON) and link with mold.
            mold
          ];
        };
      });
    };
}
