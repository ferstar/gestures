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
    crate2nix = {
      url = "github:kolloch/crate2nix";
      flake = false;
    };
  };

  outputs = { nixpkgs, utils, fenix, crate2nix, ... }:
  let 
    name = "gestures";
  in utils.lib.eachSystem
    [
      utils.lib.system.x86_64-linux
    ]
    (system:
      let
        toolchain = fenix.packages.${system}.fromToolchainFile {
          file = ./rust-toolchain.toml;
        };

        pkgs = import nixpkgs {
          inherit system;
        };

        crate2nix' = pkgs.callPackage "${crate2nix}/tools.nix" { };
        project = crate2nix'.appliedCargoNix {
          inherit name;
          src = ./.;

          #inherit buildInputs nativeBuildInputs;
        };

        buildInputs = with pkgs; [ libinput udev xdotool ];
        nativeBuildInputs = with pkgs; [ toolchain pkg-config nixpkgs-fmt ];
        buildEnvVars = {};
      in
      rec {
        packages.${name} = project.rootCrate.build.override {
          crateOverrides = pkgs.defaultCrateOverrides // {
            ${name} = attrs: {
              inherit buildInputs nativeBuildInputs;
            };
          };
        };

        defaultPackage = packages.${name};

        apps.${name} = utils.lib.mkApp {
          inherit name;
          drv = packages.${name};
        };
        defaultApp = apps.${name};

        devShell = pkgs.mkShell
          {
            inherit buildInputs nativeBuildInputs;
            RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
          } // buildEnvVars;
      }
    );
}
