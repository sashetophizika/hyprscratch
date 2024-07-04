{ pkgs ? import <nixpkgs> { } }:
pkgs.rustPlatform.buildRustPackage rec {
  pname = "hyprscratch";
  version = "0.4.1";
  cargoLock.lockFile = ./Cargo.lock;
  src = pkgs.lib.cleanSource ./.;
}

