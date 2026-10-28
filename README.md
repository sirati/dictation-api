# Dictation API

Applications retain the insertion target when dictation starts. The broker routes the completed transcript to that same application even after focus moves elsewhere. It never simulates global keyboard input.

## Develop

```sh
nix develop
cargo test -p dictation-broker
cargo check --workspace
cargo run -p xtask              # regenerate TypeScript protocol and C header
cargo run -p dictation-broker -- run
```

The flake follows the development shells in the neighboring Rust projects. Linux and Darwin shells provide Rust, Python, maturin, Node, cbindgen and cross-compilation tools. Use `DICTATION_API_HOME` to isolate a broker for testing.

## Session contract

A receiving application registers an ID, display name, optional icon and process ID. It can report focus explicitly with `activate()`, which returns an activation lease. Pass that lease to `deactivate()`. A newer activation replaces the previous owner. An old deactivation cannot clear newer focus. Native focus observers can also identify a registered application by process ID; activating an unregistered application clears the receiving target.

A provider calls `begin(audio_features)`. Only the focused application receives an offer. It must capture its field, selection and document identity before accepting. Acceptance negotiates the intersection of the provider's available visualization data and the application's requested data. The provider receives `ready`, including the application's name and icon. A decline or five-second offer timeout produces `ready` without an application; the broker does not offer the session to a different application.

Audio frames contain optional normalized level and up to 128 normalized frequency-band magnitudes ordered low to high. These are visualization values, not microphone samples. Values must be finite and between zero and one. Providers must send only negotiated fields.

When microphone capture ends, the provider calls `processing(session)` after
`ready`, before waiting for transcription or a queued recognition slot. The
captured application receives `{kind:"processing", session}` and can replace
its recording indicator with a processing indicator. No more audio frames
are accepted for that session. Repeating `processing` refreshes the five-minute
session timeout without emitting another event; providers should refresh it
at least once a minute during long waits. Completion remains valid without
this call for older providers.

New client SDKs register with `processing:true`. Older clients that omit the
flag do not receive the new event and retain their previous protocol behavior.
Providers using the new request need a broker built from this revision or later.
Cancellation is terminal and sends `cancelled` to the accepting application
immediately. An Undo action begins a new broker session for the retained audio.

Completion delivers `{text, submit}` to the captured application. The application applies it to the captured target and acknowledges success or failure. A missing or destroyed target must fail instead of inserting into the newly focused field. `submit` means submit the original target after insertion; do not globally press Enter. The provider receives `finished` after acknowledgement. Disconnect, cancellation and delivery timeout terminate the session without retargeting.

## Language interfaces

- Rust: `dictation-api` exposes `Client` and `Provider`, plus the protocol types.
- C: `bindings/c` builds shared/static libraries and `cargo run -p xtask` generates `dictation_api.h` using cbindgen. Request and event payloads use the documented protocol JSON. Free every returned string with `dictation_string_free`; close each connection exactly once.
- Python: `bindings/python` uses PyO3 and maturin. Run `maturin develop` in that directory inside a virtual environment. Import `Client` and `Provider` from `dictation_api`. Blocking calls release the GIL.
- JavaScript/TypeScript: `bindings/node` uses napi-rs and ts-rs. Run `npm install && npm run build` there. Both JavaScript and TypeScript import `Client` and `Provider` from `@dictation-api/client`. Node-API binaries work in Electron's main process. Expose a narrow application-specific preload bridge to sandboxed renderers.

Every connection has one event consumer. Do not run simultaneous `next_event` calls on a connection. Each receiving application should keep its own session-to-target map.

## Transport and desktop integration

The SDK authenticates to a loopback-only socket using a random secret from the broker's per-user endpoint file. Unix directories and endpoint files are private to the user. The endpoint has a protocol version, address and token. Do not share it with other users or log its token. This API follows the user's cooperative focus model; explicit activation reports are not compositor-authenticated security claims.

On Linux, the broker also exposes `org.dictationapi.Broker1` at `/org/dictationapi/Broker1` on the session D-Bus. `Request(s json) -> s json` accepts a protocol request, `NextEvent(u timeout_ms) -> s json` waits for an event or returns `null`, and `Close()` releases the caller connection. Each unique D-Bus sender has its own identity. The AT-SPI observer subscribes to window activation/deactivation events on the accessibility bus. Availability depends on the desktop and application accessibility implementation. Explicit reports continue to work when accessibility is unavailable.

Windows observes the native foreground window and owning process. macOS observes NSWorkspace's frontmost application. They run in the interactive user's desktop session.

## User service

Build and install the broker in a permanent executable location before running:

```sh
dictation-broker install
dictation-broker status
# Later, remove the service:
dictation-broker uninstall
```

Linux installs a systemd user unit. macOS installs a LaunchAgent. Windows installs an interactive, least-privilege ONLOGON scheduled task. These run as the current user. Do not run the broker as a privileged system service, which cannot own the user's GUI focus and per-user API endpoint.

Run `cargo test -p dictation-broker` for ownership and socket tests. `node tests/node-e2e.mjs` exercises the napi-rs SDK against a real broker. `python tests/python-c-e2e.py` exercises a Python provider against a C receiving application after building and installing their bindings. Platform focus and service validation are separate from these transport tests.

## Declarative Nix user service

On NixOS and macOS, prefer the module over `dictation-broker install`. Home Manager manages the user service without an imperative installation step:

```nix
# flake.nix
inputs.dictation-api.url = "github:sirati/dictation-api";

# Home Manager configuration, with inputs passed through extraSpecialArgs:
{ inputs, ... }: {
  imports = [ inputs.dictation-api.homeManagerModules.default ];
  services.dictation-api.enable = true;
}
```

Alternatively import `inputs.dictation-api.nixosModules.default` in the NixOS configuration and set `services.dictation-api.enable = true`. This declares a systemd **user** unit for graphical user sessions, rather than a privileged system daemon. Choose either the Home Manager or NixOS module for a given user. Both expose `package` and `environment` options. The broker starts with `graphical-session.target` and stops when that session ends. The desktop must activate that standard target.

On macOS, the Home Manager module instead declares a LaunchAgent that starts at login in the interactive user session. The same `package` and `environment` options apply.

`overlays.default` also provides `pkgs.dictation-broker`. The flake locks the package source and dependencies; update the input to select a newer revision.
