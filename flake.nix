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
      url = "github:natimerry/retoc-rivals/00a9468510902bfb078393718e484d6c5a66a1af";
      flake = false;
    };
    uasset-mesh-patch-rivals = {
      url = "github:natimerry/uasset-mesh-patch-rivals/93f78118b661dde469f41361a2ebf004eb4f4ca2";
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

      packages.uasset-api = {pkgs, ...}: pkgs.buildDotnetModule rec {
        name = "UAssetAPI-Kawaii";
        version = "95b4053";
        src = pkgs.fetchFromGitHub {
          owner = "natimerry";
          repo = "UAssetAPI-Kawaii";
          rev = version;
          hash = "sha256-tee/HD2mPiCOfpLbbbFG6lo6tGfOSVXJVr1o0fWLxQY=";
        };
        dotnet-sdk = pkgs.dotnet-sdk_8;
        nugetDeps = ./nix/deps.json;
      };

      packages.default = {pkgs, ...}: let
        UAssetAPI = self.packages.${pkgs.system}.uasset-api;
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
          NIX_UASSET_API_PATH = UAssetAPI;
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

          buildInputs = [pkgs.stdenv.cc.cc.lib UAssetAPI];

          NIX_UASSET_API_PATH = UAssetAPI;
          patches = [ ./nix/retoc-build.patch ];

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
      devShell = {pkgs, ...}: {
        inputsFrom = [ self.packages.${pkgs.system}.default ];
        packages = [
          (pkgs.fenix.complete.withComponents [
            "cargo"
            "clippy"
            "rust-src"
            "rustc"
            "rustfmt"
            "rust-analyzer"
          ])
          pkgs.dotnet-sdk_8
        ];
        env = {
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (with pkgs; [
            stdenv.cc.cc.lib
            libX11
            libXcursor
            libxi
            libxkbcommon
            mesa
            libGL
          ]);
        };
      };
    };
}
