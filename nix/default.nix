{
  lib,
  rustPlatform,
}:
rustPlatform.buildRustPackage {
  pname = "hyprscratch";
  version = "0.6.2";

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
