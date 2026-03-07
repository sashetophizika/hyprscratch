{
  lib,
  rustPlatform,
}: let
  cargo = lib.importTOML ../Cargo.toml;
in
  rustPlatform.buildRustPackage {
    pname = "hyprscratch";
    inherit (cargo.package) version;

    src = lib.cleanSource ../.;

    cargoLock.lockFile = ../Cargo.lock;

    doCheck = false;

    meta = {
      description = "Improved scratchpad functionality for Hyprland";
      homepage = "https://github.com/sashetophizika/hyprscratch";
      license = lib.licenses.mit;
      mainProgram = "hyprscratch";
    };
  }
