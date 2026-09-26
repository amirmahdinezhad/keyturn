{
  description = "Menubar API-key manager and quota monitor for AI coding CLIs";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        version = "0.1.0";
        src = self;
      in
      {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            nodejs_22
            rustc
            cargo
            rust-analyzer
            pkg-config
            openssl
          ];
          shellHook = ''
            echo "Keyturn dev shell — npm install && npm run tauri dev"
          '';
        };

        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "keyturn";
          inherit version src;
          cargoRoot = "src-tauri";
          buildAndTestSubdir = "src-tauri";
          cargoLock.lockFile = ./src-tauri/Cargo.lock;

          # The frontend bundle is built first; `dist/` is then embedded by
          # tauri-build. Refresh `npmDepsHash` when package-lock.json changes
          # (nix prints the expected value on mismatch).
          npmDepsHash = pkgs.lib.fakeHash;

          nativeBuildInputs = with pkgs; [
            nodejs_22
            pkg-config
            cargo-tauri
            makeWrapper
          ];
          buildInputs = with pkgs; [
            openssl
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.WebKit
            pkgs.darwin.apple_sdk.frameworks.AppKit
          ];

          buildPhase = ''
            export HOME=$TMPDIR
            npm ci --ignore-scripts
            npm run build
            cargo tauri build --no-bundle
          '';

          installPhase = ''
            mkdir -p $out/Applications
            cp -R src-tauri/target/release/keyturn $out/
          '';

          meta = with pkgs.lib; {
            description = "Menubar API-key manager and quota monitor for AI coding CLIs";
            homepage = "https://github.com/7SkyLab/keyturn";
            license = licenses.mit;
            platforms = platforms.darwin;
            mainProgram = "keyturn";
          };
        };
      });
}
