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
        ];
    in
    {
      packages = forSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};

          uiPackage = pkgs.stdenv.mkDerivation {
            pname = "ui-dist";
            version = "0.1.0";
            src = ./ui;

            nativeBuildInputs = with pkgs; [
              cargo
              rustc
              trunk-ng
              wasm-bindgen-cli_0_2_106
              lld
              rustPlatform.cargoSetupHook
            ];

            cargoDeps = pkgs.rustPlatform.importCargoLock {
              lockFile = ./ui/Cargo.lock;
            };

            buildPhase = ''
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

              contents = [ dynamicrouterPackage ];

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
