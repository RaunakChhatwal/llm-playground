{
  description = "Native frontend to GPT and Claude.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-utils.url = "github:numtide/flake-utils";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs = { self, nixpkgs, crane, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        rustWithWasmTarget = pkgs.rust-bin.nightly.latest.default.override {
          targets = [ "wasm32-unknown-unknown" ];
        };

        # NB: we don't need to overlay our custom toolchain for the *entire*
        # pkgs (which would require rebuidling anything else which uses rust).
        # Instead, we just want to update the scope that crane will use by appending
        # our specific toolchain there.
        craneLib = (crane.mkLib pkgs).overrideToolchain rustWithWasmTarget;

        buildInputs = with pkgs; ([
          openssl
          zlib
          at-spi2-atk
        ] ++ (if stdenv.isLinux then [
          webkitgtk
          gtk3
        ] else [
          iconv
          darwin.apple_sdk.frameworks.Security
          darwin.apple_sdk.frameworks.CoreServices
          darwin.apple_sdk.frameworks.CoreFoundation
          darwin.apple_sdk.frameworks.Foundation
          darwin.apple_sdk.frameworks.AppKit
          darwin.apple_sdk.frameworks.WebKit
          darwin.apple_sdk.frameworks.Cocoa
        ]));

        llm-playground = craneLib.mkCargoDerivation (with pkgs; rec {
          pname = "llm-playground";
          version = "0.1.0";

          src = lib.cleanSource ./.;
          strictDeps = false;
          buildPhaseCargoCommand = (if stdenv.isLinux then "" else ''
            export RUSTFLAGS="-L ${iconv}/lib"
          '') + ''
            export HOME=/tmp/homeless-shelter
            [[ -e $HOME ]] || mkdir $HOME
            unset SSL_CERT_FILE
            cargo-tauri build || [[ -e ./target/release/llm-playground ]]
          '';

          libPath = lib.makeLibraryPath buildInputs;
          XDG_DATA_DIRS = lib.concatStringsSep ":" [
            "${gsettings-desktop-schemas}/share/gsettings-schemas/${gsettings-desktop-schemas.name}"
            "${gtk3}/share/gsettings-schemas/${gtk3.name}"
            "$XDG_DATA_DIRS"];
          installPhase = if stdenv.isLinux then ''
            mkdir -p $out/bin
            mv ./target/release/llm-playground $out/bin
            patchelf --set-rpath ${libPath} \             # why is this necessary??
              --set-interpreter $(cat $NIX_CC/nix-support/dynamic-linker) \
              $out/bin/llm-playground
            mv ./bundle/share $out
            cat << EOF > "$out/share/applications/llm-playground.desktop"
            [Desktop Entry]
            Name=LLM Playground
            Icon=$out/share/icons/llm-playground.ico
            Exec=XDG_DATA_DIRS=${XDG_DATA_DIRS} \
              GIO_MODULE_DIR="${glib-networking}/lib/gio/modules/" llm-playground
            Type=Application
            Terminal=false
            Type=Application
            EOF
          '' else ''
            mkdir ./bundle/Contents/MacOS && cp ./target/release/llm-playground ./bundle/Contents/MacOS
            APPDIR="$out/Applications/llm-playground.app"
            mkdir -p $APPDIR
            mv ./bundle/Contents $APPDIR
            mkdir -p $out/bin
            mv ./target/release/llm-playground $out/bin
          '';
          doInstallCargoArtifacts = false;
          cargoArtifacts = ./.;

          nativeBuildInputs = with pkgs; [
            cargo-tauri
            trunk
            wasm-bindgen-cli
            tailwindcss
            python3
            pkg-config
          ] ++ (if stdenv.isLinux then [
            autoPatchelfHook    # this hook breaks on macos due to mach-o
          ] else []);

          inherit buildInputs;
        });
      in {
        checks = {
          inherit llm-playground;
        };

        packages.default = llm-playground;

        devShells.default = pkgs.mkShell (with pkgs; {
          buildInputs = buildInputs ++ [
            trunk
            tailwindcss
            wasm-bindgen-cli
            pkg-config
          ];
        });
      }
    );
}
