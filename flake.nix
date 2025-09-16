{
  description = "Build a cargo project without extra checks";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane.url = "github:ipetkov/crane";

    flake-utils.url = "github:numtide/flake-utils";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      flake-utils,
      rust-overlay,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };
        inherit (pkgs) lib;

        # Tady přepíšeme defaultní toolchain, který Crane použije
        craneLib = (crane.mkLib pkgs).overrideToolchain (
          p:
          p.rust-bin.stable."1.88.0".default.override {
            targets = [ "x86_64-unknown-linux-gnu" ];
          }
        );
        src = craneLib.cleanCargoSource ./.;

        # Common arguments can be set here to avoid repeating them later
        # Note: changes here will rebuild all dependency crates
        commonArgs = {
          src = craneLib.cleanCargoSource ./.;
          strictDeps = true;

          buildInputs = with pkgs; [
            expat
            fontconfig
            freetype
            freetype.dev
            libGL
            pkg-config
            xorg.libX11
            xorg.libXcursor
            xorg.libXi
            xorg.libXrandr
            wayland
            libxkbcommon
          ];
        };

        # Build *just* the cargo dependencies (of the entire workspace),
        # so we can reuse all of that work (e.g. via cachix) when running in CI
        # It is *highly* recommended to use something like cargo-hakari to avoid
        # cache misses when building individual top-level-crates
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        individualCrateArgs = commonArgs // {
          inherit cargoArtifacts;
          inherit (craneLib.crateNameFromCargoToml { inherit src; }) version;
        };

        fileSetForCrate =
          crate:
          lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions [
              ./Cargo.toml
              ./Cargo.lock
              (craneLib.fileset.commonCargoSources ./ekkles_data)
              (craneLib.fileset.commonCargoSources crate)
            ];
        };

        ekkles = craneLib.buildPackage (
          individualCrateArgs
          // {
            pname = "ekkles";
            cargoExtraArgs = "-p ekkles";
            src = fileSetForCrate ./.;
          }
          // {
            DATABASE_URL = builtins.trace "Cesta k db je: sqlite://${./ekkles_data/db/db_skeletion.sqlite3}" "sqlite://${./ekkles_data/db/db_skeletion.sqlite3}";

            # Jinak vybuchne kompozitor
            nativeBuildInputs = [ pkgs.makeWrapper ];
            postInstall = ''
              makeWrapper $out/bin/ekkles $out/bin/wrapped --set LD_LIBRARY_PATH ${builtins.toString (pkgs.lib.makeLibraryPath [
                  pkgs.expat
                  pkgs.fontconfig
                  pkgs.freetype
                  pkgs.freetype.dev
                  pkgs.libGL
                  pkgs.vulkan-headers
                  pkgs.vulkan-loader
                  pkgs.vulkan-extension-layer
                  pkgs.vulkan-validation-layers
                  pkgs.xorg.libX11
                  pkgs.xorg.libXcursor
                  pkgs.xorg.libXi
                  pkgs.xorg.libXrandr
                  pkgs.wayland
                  pkgs.libxkbcommon
                  pkgs.vulkan-loader
              ])} --set RUST_LOG info
            '';
          }
        );
      in
      {
        checks = {
          inherit ekkles;
        };

        packages.default = ekkles;

        apps.default = flake-utils.lib.mkApp {
          drv = ekkles;
        };

        devShells.default = craneLib.devShell {
          # Inherit inputs from checks.
          checks = self.checks.${system};

          # Additional dev-shell environment variables can be set directly
          # MY_CUSTOM_DEVELOPMENT_VAR = "something else";

          # Extra inputs can be added here; cargo and rustc are provided by default.
          packages = [
            # pkgs.ripgrep
          ];
        };
      }
    );
}
