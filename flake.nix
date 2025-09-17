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
        };

        # Build *just* the cargo dependencies (of the entire workspace),
        # so we can reuse all of that work (e.g. via cachix) when running in CI
        # It is *highly* recommended to use something like cargo-hakari to avoid
        # cache misses when building individual top-level-crates
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        individualCrateArgs = commonArgs // {
          # URL k databázi, aby se pomocí sqlx při kompilaci ověřily dotazy vzhledem ke struktuře tabulek
          DATABASE_URL = "sqlite://${./ekkles_data/db/db_skeletion.sqlite3}";

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

        # Iced potřebuje pro GUI funkcionalitu při runtime další závislosti
        icedRuntimeDeps = with pkgs; [
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

        desktopFile = pkgs.makeDesktopItem {
          name = "ekkles";
          exec = "ekkles";
          desktopName = "Ekkles";
          comment = "Prezentační program pro Bohoslužby";
          icon = "ekkles";
        };

        # Samotná GUI Ekkles aplikace
        ekkles = craneLib.buildPackage (
          individualCrateArgs
          // {
            pname = "ekkles";
            cargoExtraArgs = "-p ekkles";
            src = fileSetForCrate ./.;
            buildInputs = icedRuntimeDeps;
          }
          // {
            nativeBuildInputs = with pkgs; [ makeWrapper ];
            postInstall = ''
              # Protože winit používá dl_open(), aby dynamicky otevřel knihovny,
              # wrapneme program a natvrdo nastavíme cestu ke knihovnám, které zkusí otevřít
              wrapProgram $out/bin/ekkles --set LD_LIBRARY_PATH ${builtins.toString (pkgs.lib.makeLibraryPath icedRuntimeDeps)}

              # Překopírujeme desktop file, aby to šlo pohodlně otevřít na ploše
              mkdir -p $out/share/applications
              cp ${desktopFile}/share/applications/ekkles.desktop $out/share/applications/ekkles.desktop

              # Překopírujeme ikonku
              mkdir -p $out/share/pixmaps
              cp "${./pkg/logo.png}" $out/share/pixmaps/ekkles.png
            '';
          }
        );

        # CLI Nástroj pro import souborů do databáze Ekklesu
        ekkles-cli = craneLib.buildPackage (
          individualCrateArgs
          // {
            pname = "ekkles_cli";
            cargoExtraArgs = "-p ekkles_cli";
            src = fileSetForCrate ./.;
          }
        );
      in
      {
        checks = {
          inherit ekkles ekkles-cli;
        };

        packages = {
          inherit ekkles ekkles-cli;
          default = ekkles;
        };

        apps = {
          ekkles = flake-utils.lib.mkApp {
            drv = ekkles;
          };
          ekkles-cli = flake-utils.lib.mkApp {
            drv = ekkles-cli;
          };
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
