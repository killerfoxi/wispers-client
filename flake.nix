{
  description = "Wispers Connect development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { nixpkgs, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        # Core dependencies needed for the Rust library
        coreDeps = with pkgs; [
          cmake
          protobuf
          libclang
        ];
      in
      {
        devShells = {
          # Default: core Rust library development
          default = pkgs.mkShell {
            buildInputs = coreDeps;
            env.LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
          };

          # Go wrapper development
          go = pkgs.mkShell {
            buildInputs = coreDeps ++ [ pkgs.go ];
            env.LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
          };

          # Python wrapper development
          py = pkgs.mkShell {
            buildInputs = coreDeps ++ [
              pkgs.python313
              pkgs.python313Packages.pytest
            ];
            env.LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
          };

          # Kotlin/Android wrapper development
          kt = pkgs.mkShell {
            buildInputs = coreDeps ++ [
              pkgs.jdk17
              pkgs.gradle
            ];
            env.LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
          };
        };
      }
    );
}
