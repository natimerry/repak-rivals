{
  inputs = {
    self.submodules = true;
    flakelight.url = "github:nix-community/flakelight";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    flakelight,
    crane,
    fenix,
    ...
  }:
    flakelight ./. {
      # make you able to access pkgs.fenix.complete.xyz
      withOverlays = [
        fenix.overlays.default
      ];

      packages.default = {pkgs, ...}: let
        craneLib = (crane.mkLib pkgs).overrideToolchain (
          p:
            p.fenix.complete.withComponents [
              "cargo"
              "clippy"
              "rust-src"
              "rustc"
              "rustfmt"
            ]
        );
      in
        craneLib.buildPackage {
          pname = "repak-rivals"; # i added this so crane wont spam my fucking terminal
          doCheck = false; # disable tests
          src = ./.;
          nativeBuildInputs = with pkgs; [
            stdenv.cc.cc.lib
            makeWrapper
          ];

          buildInputs = [pkgs.stdenv.cc.cc.lib];

          # banger postInstall right here
          postInstall = ''
            wrapProgram $out/bin/repak-gui \
              --prefix LD_LIBRARY_PATH : ${with pkgs; lib.makeLibraryPath [stdenv.cc.cc.lib libX11 libXcursor libXcursor libxi libxkbcommon mesa libGL]}
          '';
        };
      apps.default = packages: {
        type = "app";
        program = "${packages.default}/bin/repak-gui";
      };
      devShell.packages = {pkgs, ...}: [
        (pkgs.fenix.complete.withComponents [
          "cargo"
          "clippy"
          "rust-src"
          "rustc"
          "rustfmt"
          "rust-analyzer"
        ])
      ];
    };
}
