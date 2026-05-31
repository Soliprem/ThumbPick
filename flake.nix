{
  description = "A simple Rust project";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs =
    {
      self,
      nixpkgs,
      naersk,
      fenix,
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          fenixLib = fenix.packages.${system};
          rustToolchain = fenixLib.default.toolchain;

          # GTK4 and dependencies
          buildInputs = with pkgs; [
            gtk4
            glib
            graphene
            gdk-pixbuf
            cairo
            pango
            harfbuzz
          ];

          nativeBuildInputs = with pkgs; [
            pkg-config
            rustToolchain
          ];
        in
        {
          default =
            let
              unwrapped =
                (naersk.lib.${system}.override {
                  cargo = rustToolchain;
                  rustc = rustToolchain;
                }).buildPackage
                  {
                    pname = "thumbpick";
                    version = "0.1.1";
                    src = nixpkgs.lib.cleanSource self;

                    inherit buildInputs nativeBuildInputs;
                  };
            in
            pkgs.runCommand "thumbpick"
              {
                nativeBuildInputs = [ pkgs.makeWrapper ];
              }
              ''
                mkdir -p $out/bin
                cp ${unwrapped}/bin/thumbpick $out/bin/
                wrapProgram $out/bin/thumbpick \
                  --prefix LD_LIBRARY_PATH : "${pkgs.lib.makeLibraryPath buildInputs}"
              '';
        }
      );

      devShells = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          fenixLib = fenix.packages.${system};
          rustToolchain = fenixLib.default.toolchain;

          buildInputs = with pkgs; [
            gtk4
            glib
            graphene
            gdk-pixbuf
            cairo
            pango
            harfbuzz
          ];

          nativeBuildInputs = with pkgs; [
            pkg-config
            rustToolchain
          ];
        in
        {
          default = pkgs.mkShell {
            inherit buildInputs nativeBuildInputs;

            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath buildInputs;
          };
        }
      );
    };
}
