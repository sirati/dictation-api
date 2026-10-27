{
  lib,
  rustPlatform,
  stdenv,
}:
rustPlatform.buildRustPackage {
  pname = "dictation-api-node";
  version = "0.1.0";
  src = lib.cleanSource ../.;
  cargoLock.lockFile = ../Cargo.lock;
  cargoBuildFlags = [
    "-p"
    "dictation-api-node"
  ];
  doCheck = false; # Node-API requires a JavaScript runtime; the E2E suite loads the artifact in Node.
  installPhase =
    let
      arch = if stdenv.hostPlatform.isAarch64 then "arm64" else "x64";
      platform = if stdenv.hostPlatform.isDarwin then "darwin-${arch}" else "linux-${arch}-gnu";
      library =
        if stdenv.hostPlatform.isDarwin then "libdictation_api_node.dylib" else "libdictation_api_node.so";
    in
    ''
      runHook preInstall
      mkdir -p "$out/lib"
      install -m755 target/${stdenv.hostPlatform.rust.rustcTarget}/release/${library} "$out/lib/dictation-api.${platform}.node"
      runHook postInstall
    '';
}
