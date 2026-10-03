# A small package definition plus a development shell.
{ pkgs ? import <nixpkgs> { }
, lib ? pkgs.lib
, enableDocs ? false
, ...
}:

let
  version = "1.4.2";
  /* Source filtered to exclude build artefacts. */
  src = lib.cleanSourceWith {
    src = ./.;
    filter = path: type: !(type == "directory" && baseNameOf path == "target");
  };
  inherit (pkgs) stdenv;
  optionalDeps = lib.optionals enableDocs [ pkgs.mdbook ];
in
rec {
  package = stdenv.mkDerivation {
    pname = "sample";
    inherit version src;
    nativeBuildInputs = with pkgs; [ pkg-config cmake ] ++ optionalDeps;
    buildInputs = [ pkgs.openssl ];
    cmakeFlags = [ "-DVERSION=${version}" "-DDOCS=${lib.boolToString enableDocs}" ];
    doCheck = true;
    installPhase = ''
      mkdir -p $out/bin
      cp sample $out/bin/
      echo "installed ''${version}" > $out/VERSION
    '';
    meta = {
      description = "Example package\twith a tab";
      license = lib.licenses.mit;
      platforms = lib.platforms.unix;
    };
  };

  shell = pkgs.mkShell {
    inputsFrom = [ package ];
    packages = builtins.attrValues { inherit (pkgs) git ripgrep; };
    shellHook = "export RUST_LOG=${if enableDocs then "debug" else "info"}";
  };

  checks.count = assert builtins.length optionalDeps >= 0; builtins.toString (1 + 2 * 3);
}
