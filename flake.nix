{
  description = "OpenTTD (JGR's patchpack fork, C++/Rust) development shell";

  inputs = {
    # Fetched from nixos.org and over git rather than GitHub tarballs, which the
    # cloud sessions' network policy blocks. Update with `nix flake update`.
    nixpkgs.url = "https://channels.nixos.org/nixpkgs-26.05-darwin/nixexprs.tar.xz";
    rust-overlay = {
      url = "git+https://github.com/oxalica/rust-overlay?shallow=1";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { nixpkgs, rust-overlay, ... }:
    let
      # Apple silicon is the supported macOS platform; x86_64-linux covers Linux and the cloud sessions.
      systems = [ "aarch64-darwin" "x86_64-linux" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f (import nixpkgs {
        inherit system;
        overlays = [ rust-overlay.overlays.default ];
      }));
    in
    {
      devShells = forAllSystems (pkgs:
        let
          # The same LLVM major version as the pinned rustc (LLVM 22 for Rust 1.97), so that
          # C++ and Rust can be optimised together at link time.
          llvm = pkgs.llvmPackages_22;
          # On Linux, clang's wrapper links through Nix's wrapped lld, which adds the run-time
          # search paths (e.g. for libstdc++) that a bare lld would leave out. On macOS, lld's
          # `ld` name means the ELF linker, so the default linker setup stays and
          # -fuse-ld=lld picks ld64.lld from llvm.lld.
          stdenv =
            if pkgs.stdenv.hostPlatform.isLinux
            then pkgs.overrideCC llvm.stdenv (llvm.clang.override { bintools = llvm.bintools; })
            else llvm.stdenv;
          # The Rust toolchain pinned in rust-toolchain.toml. rust-overlay makes it propagate
          # nixpkgs' default C compiler, as the linker for Rust, which comes before the dev
          # shell's own compiler in PATH. On macOS that is an older clang, which CMake and
          # Cargo would then use instead of this one; without it, Rust links through `cc`
          # from the dev shell. (On Linux it is gcc, which does not shadow `clang`.)
          rustToolchain =
            let toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
            in
            if pkgs.stdenv.hostPlatform.isDarwin
            then toolchain.overrideAttrs (_: { depsHostHostPropagated = [ ]; propagatedBuildInputs = [ ]; })
            else toolchain;
        in
        {
          default = (pkgs.mkShell.override { inherit stdenv; }) {
            packages = with pkgs; [
              # Build tools; configure with -DCMAKE_EXE_LINKER_FLAGS=-fuse-ld=lld
              cmake
              ninja
              ccache
              pkg-config
              llvm.lld
              python3

              # The Rust toolchain, and the dependency policy checker
              rustToolchain
              cargo-deny

              # Optional libraries the game uses (see COMPILING.md)
              zlib
              xz
              libpng
              zstd
              lzo
              curl
            ] ++ lib.optionals stdenv.hostPlatform.isLinux [
              # The Linux GUI build (macOS uses Cocoa and CoreText instead)
              SDL2
              libGL
              freetype
              fontconfig
              harfbuzz
              icu
              libogg
              libopus
              opusfile
            ];
          };
        });
    };
}
