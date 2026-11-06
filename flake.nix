{
  inputs = {
    flakelight.url = "github:nix-community/flakelight";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    uasset-api = {
      url = "github:natimerry/UAssetAPI-kawaii";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    retoc-rivals = {
      url = "github:natimerry/retoc-rivals";
      flake = false;
    };
  };

  outputs = {
    flakelight,
    crane,
    fenix,
    self,
    uasset-api,
    retoc-rivals,
    ...
  }:
    flakelight ./. {
      withOverlays = [
        fenix.overlays.default
      ];

      packages = {
        default = {pkgs, ...}: let
          binding = uasset-api.packages.${pkgs.system}.kawaii-physics-binding;
          dotnetRuntime = pkgs.dotnetCorePackages.runtime_8_0;
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
          src = self;
          postPatch = ''
            rm -rf retoc-rivals
            ln -s ${retoc-rivals} retoc-rivals
          '';
          retocDummy = ''
            mkdir -p $out/retoc-rivals/src
            cp ${retoc-rivals}/Cargo.toml $out/retoc-rivals/Cargo.toml
            : > $out/retoc-rivals/src/lib.rs
          '';
          cargoArtifacts = craneLib.buildDepsOnly {
            inherit src;
            extraDummyScript = retocDummy;
            pname = "repak-rivals";
            doCheck = false;
          };
        in
          craneLib.buildPackage {
            pname = "repak-rivals";
            inherit src postPatch cargoArtifacts;
            doCheck = false;
            nativeBuildInputs = with pkgs; [
              stdenv.cc.cc.lib
              makeWrapper
            ];
            RETOC_KAWAII_BINDING_DIR = "${binding}/lib/KawaiiPhysicsBinding";
            buildInputs = [pkgs.stdenv.cc.cc.lib];

            postInstall = ''
              wrapProgram $out/bin/repak-gui \
                --set DOTNET_ROOT ${dotnetRuntime}/share/dotnet \
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
              wrapProgram $out/bin/retoc-rivals-cli \
                --set DOTNET_ROOT ${dotnetRuntime}/share/dotnet

            '';
          };
      };
      apps.default = packages: {
        type = "app";
        program = "${packages.default}/bin/repak-gui";
      };
      devShell = {pkgs, ...}: {
        inputsFrom = [self.packages.${pkgs.system}.default];
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
