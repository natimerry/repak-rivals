{
  inputs = {
    flakelight.url = "github:nix-community/flakelight";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    retoc-rivals = {
      url = "github:natimerry/retoc-rivals/c5b00c2";
      flake = false;
    };
    uasset-mesh-patch-rivals = {
      url = "github:natimerry/uasset-mesh-patch-rivals/a9e9f33";
      flake = false;
    };
  };

  outputs = {
    flakelight,
    crane,
    fenix,
    self,
    uasset-mesh-patch-rivals,
    retoc-rivals,
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
        # instead of making $out become $self, i just make a folder and then put the $self content into $out, seems to fix it... idk how
        src = pkgs.runCommand "source-with-submodules" {} ''
          mkdir -p $out
          cp -r ${self}/. $out/
          chmod -R u+w $out
          mkdir -p $out/retoc-rivals
          mkdir -p $out/uasset-mesh-patch-rivals
          cp -r ${retoc-rivals}/. $out/retoc-rivals/
          cp -r ${uasset-mesh-patch-rivals}/. $out/uasset-mesh-patch-rivals/
        '';
        # i am tired to build dependencies so i added this
        cargoArtifacts = craneLib.buildDepsOnly {
          inherit src;
          pname = "repak-rivals";
          doCheck = false;
        };
      in
        craneLib.buildPackage {
          pname = "repak-rivals"; # i added this so crane wont spam my fucking terminal
          doCheck = false; # disable tests

          inherit src cargoArtifacts;
          nativeBuildInputs = with pkgs; [
            stdenv.cc.cc.lib
            makeWrapper
          ];

          buildInputs = [pkgs.stdenv.cc.cc.lib];

          # banger postInstall right here
          postInstall = ''
            wrapProgram $out/bin/repak-gui \
              --prefix LD_LIBRARY_PATH : ${
              with pkgs;
                lib.makeLibraryPath [
                  stdenv.cc.cc.lib
                  libX11
                  libXcursor
                  libXcursor
                  libxi
                  libxkbcommon
                  mesa
                  libGL
                ]
            }
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
