{
  description = "A fast libinput-based touchpad gestures program";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
    utils.url = "github:numtide/flake-utils";
    fenix.url = "github:nix-community/fenix";
    crane = {
      url = "github:ipetkov/crane";
    };
  };

  outputs = { nixpkgs, utils, fenix, crane, ... }:
  let 
    name = "gestures";
  in utils.lib.eachSystem
    [
      utils.lib.system.x86_64-linux
    ]
    (system:
      let
        toolchain = fenix.packages.${system}.stable.toolchain;

        pkgs = import nixpkgs {
          inherit system;
        };

        craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;

        buildInputs = with pkgs; [ libinput udev xdotool ];
        nativeBuildInputs = with pkgs; [ pkg-config makeWrapper ];
      in
      rec {
        packages = {
          ${name} = craneLib.buildPackage {
            pname = name;
            src = craneLib.cleanCargoSource ./.;

            inherit buildInputs nativeBuildInputs;

            # Set runtime library paths
            runtimeDependencies = buildInputs;

            # Ensure dynamic libraries can be found at runtime
            postInstall = ''
              wrapProgram $out/bin/${name} \
                --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath buildInputs}
            '';
          };
          default = packages.${name};
        };

        apps = {
          ${name} = utils.lib.mkApp {
            inherit name;
            drv = packages.${name};
          };
          default = apps.${name};
        };

        devShells.default = craneLib.devShell {
          inherit buildInputs;
          nativeBuildInputs = nativeBuildInputs ++ [ toolchain pkgs.nixpkgs-fmt ];
          RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
        };
      }
    );
}
