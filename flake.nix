{
  description = "Dynamicrouter";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    {
      self,
      nixpkgs,
    }:
    let
      forSystems = nixpkgs.lib.genAttrs [
        "x86_64-linux"
        "aarch64-linux"
      ];
      sharedNativeDeps =
        pkgs: with pkgs; [
          sqlx-cli
          sqlite
        ];
    in
    {
      packages = forSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};

          interFont = pkgs.fetchurl {
            url = "https://cdn.jsdelivr.net/npm/@fontsource-variable/inter@5.1.1/files/inter-latin-wght-normal.woff2";
            sha256 = "1bsnvz125508mnvgn9ihj0ilscdwa1qmd958mcizv3bjqd2fwlph";
          };

          outfitFont = pkgs.fetchurl {
            url = "https://cdn.jsdelivr.net/npm/@fontsource-variable/outfit@5.1.1/files/outfit-latin-wght-normal.woff2";
            sha256 = "1433qk8d6xmbfah5zzfv50ngdz07r92691phgzka87lr8lmpli25";
          };

          jetbrainsMonoFont = pkgs.fetchurl {
            url = "https://cdn.jsdelivr.net/npm/@fontsource-variable/jetbrains-mono@5.1.1/files/jetbrains-mono-latin-wght-normal.woff2";
            sha256 = "0zka3xjxb7nfb2r4ml67a9kshhbzk8j4maac0xn27p5z4hklbghq";
          };

          uiPackage = pkgs.stdenv.mkDerivation {
            pname = "ui-dist";
            version = "0.1.0";
            src = ./ui;

            nativeBuildInputs = with pkgs; [
              cargo
              rustc
              trunk-ng
              wasm-bindgen-cli
              dioxus-cli
              lld
              rustPlatform.cargoSetupHook
            ];

            cargoDeps = pkgs.rustPlatform.importCargoLock {
              lockFile = ./ui/Cargo.lock;
            };

            buildPhase = ''
              mkdir -p fonts
              cp ${interFont} fonts/inter.woff2
              cp ${outfitFont} fonts/outfit.woff2
              cp ${jetbrainsMonoFont} fonts/jetbrains-mono.woff2
              trunk-ng build --release --offline --filehash false
            '';

            installPhase = ''
              mkdir -p $out
              cp -r dist/* $out/
            '';
          };

          dynamicrouterPackage = pkgs.rustPlatform.buildRustPackage {
            pname = "dynamicrouter";
            version = "0.1.0";
            src = ./.;

            nativeBuildInputs = sharedNativeDeps pkgs;

            cargoLock = {
              lockFile = ./Cargo.lock;
            };

            preBuild = ''
              mkdir -p ui/dist
              cp -r ${uiPackage}/* ui/dist/
              touch build.db
              export DATABASE_URL=sqlite:build.db
              sqlx migrate run
            '';

            doCheck = false;
          };

          dynamicrouter =
            pkgs.dockerTools.buildLayeredImage {
              name = "dynamicrouter";
              tag = "latest";

              contents = [
                dynamicrouterPackage
              ]
              ++ sharedNativeDeps pkgs;

              config = {
                Cmd = [
                  "${dynamicrouterPackage}/bin/dynamicrouter"
                ];
                ExposedPorts = {
                  "8080/tcp" = { };
                };
                Env = [
                  "RUST_LOG=info"
                ];
              };
            }
            // {
              meta.image-format = "docker";
            };
        in
        {
          inherit dynamicrouterPackage dynamicrouter;
        }
      );

      devShells = forSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell {
            nativeBuildInputs =
              with pkgs;
              [
                cargo
                rustc
                rustfmt
                clippy
                lld
                trunk-ng
                wasm-bindgen-cli
                dioxus-cli
              ]
              ++ sharedNativeDeps pkgs;

            # Unset LD_LIBRARY_PATH to prevent host environment pollution
            shellHook = ''
              unset LD_LIBRARY_PATH
            '';
          };
        }
      );
    };
}
