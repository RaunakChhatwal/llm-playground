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

        buildInputs = with pkgs; [
          openssl
          zlib
          at-spi2-atk
          webkitgtk
          gtk3
        ];

        llm-playground = craneLib.mkCargoDerivation (with pkgs; rec {
          src = lib.cleanSource ./.;
          strictDeps = false;
          buildPhaseCargoCommand = ''
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
          installPhaseCommand = if stdenv.isLinux then ''
            mkdir -p $out/bin
            mv ./target/release/llm-playground $out/bin
            patchelf --set-rpath ${libPath} \
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
            mv ./target/release/llm-playground ./bundle/Contents/MacOS
            APPDIR="$out/Applications/llm-playground.app"
            mkdir -p $APPDIR
            mv ./bundle/Contents $APPDIR
            cat << EOF > "$out/bin/llm-playground"
            #!${stdenvNoCC.shell}
            open -na "$APP_DIR" --args "$@"
            EOF
            chmod +x "$out/bin/llm-playground"
          '';
          cargoArtifacts = ./.;

          nativeBuildInputs = with pkgs; [
            cargo-tauri
            trunk
            wasm-bindgen-cli
            tailwindcss
            python3
            autoPatchelfHook
            pkg-config
          ];

          inherit buildInputs;
        });
      in {
        checks = {
          inherit llm-playground;
        };

        packages.default = llm-playground;

        devShells.default = pkgs.mkShell (with pkgs; {
          nativeBuildInputs = [
            trunk
            tailwindcss
            wasm-bindgen-cli
            pkg-config
          ];

          inherit buildInputs;
        });
      }
    );
}
