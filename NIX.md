## Nix Installation Insctructions:

### Flake:
```nix
inputs = {
  hyprscratch = {
    url = "github:sashetophizika/hyprscratch";
    inputs.nixpkgs.follows = "nixpkgs";
  };
};
```

### Home Manager:
```nix
{inputs, pkgs, ...}: {
  home.packages = [inputs.hyprscratch.packages.${pkgs.system}.default];

  # or

  imports = [inputs.hyprscratch.homeModules.default];
  programs.hyprscratch = {
    enable = true;
    settings = {
      btop = {
        class = "btop";
        command = "kitty --title btop -e btop";
        rules = "size 85% 85%";
        options = "cover persist sticky";
      };
    };
  };
}
```

### Non-NixOS:
```bash
nix profile install github:sashetophizika/hyprscratch
```

