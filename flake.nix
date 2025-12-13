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
      system = "x86_64-linux";
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
      devShell.${system} = pkgs.mkShell {
        inherit buildInputs nativeBuildInputs;

        # Additional build inputs for dev shell
        packages = with pkgs; [
          nushell
        ];

        shellHook = ''
          exec nu
        '';
      };

      packages.${system}.default =
        let
          unwrapped =
            (naersk.lib.x86_64-linux.override {
              cargo = rustToolchain;
              rustc = rustToolchain;
            }).buildPackage
              {
                pname = "image-gallery-picker";
                version = "0.1.0";
                src = ./.;

                inherit buildInputs nativeBuildInputs;
              };
        in
        pkgs.runCommand "image-gallery-picker"
          {
            nativeBuildInputs = [ pkgs.makeWrapper ];
          }
          ''
            mkdir -p $out/bin
            cp ${unwrapped}/bin/image-gallery-picker $out/bin/
            wrapProgram $out/bin/image-gallery-picker \
              --prefix LD_LIBRARY_PATH : "${pkgs.lib.makeLibraryPath buildInputs}"
          '';
    };
}
