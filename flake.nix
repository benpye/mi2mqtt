{
  description = "A flake for building mi2mqtt.";

  inputs.flake-utils.url = "github:numtide/flake-utils";

  outputs = { self, nixpkgs, flake-utils }:
    with import nixpkgs;
    let
      name = "mi2mqtt";
      src = self;
    in
    {
      overlay = self: super: {
        ${name} = super.rustPlatform.buildRustPackage {
          inherit name src;
          nativeBuildInputs = [ super.pkg-config ];
          buildInputs = [ super.dbus ];
          version = "2023-04-30";
          cargoHash = "sha256-91m6QZI2TbDc/vqtyGww05hGmggIM/gqABP856QKyz0=";
        };
      };
    } // (
      flake-utils.lib.eachDefaultSystem (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ self.overlay ];
          };

          package = pkgs.${name};
        in {
          packages.${name} = package;
          defaultPackage = package;
        }
      )
    );
}
