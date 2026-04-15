# Changelog

## v0.8.0 — Open Beta

The first generally available release of Wispers Connect. Previously limited to
trusted testers with invite codes, this release opens the platform to everyone.

### Highlights

A lot of these changes were inspired by tester feedback. Thank you!

- **Open registration** — Invite codes are no longer required to create a
  Wispers Connect account. Anyone can sign up at https://connect.wispers.dev.

- **Security fixes**:
  - **Roster construction fix**: Fixed a vulnerability in roster construction
    and verification that had drifted from the original design.
  - **Protocol change**: Reworked StartConnection signing to be
    forward-compatible. The previous approach made every new proto field a
    wire-breaking change.
  - **Activation codes**: Changed from 10 to 11 base36 characters. Added
    activation code expiry (calibrated to the secret length to limit brute-force
    window). Allow up to 100 concurrent activations per node.

- **Better developer experience**:
  - **Nix**: Added `flake.nix` with development shells for each wrapper language.
  - **Published wrappers to package registries** — The library and wrappers are
    now available as standard dependencies in all supported languages.
  - **Tightened public API**: Removed unnecessary `pub` exports from the library
    to clarify the supported API surface. Added module-level documentation.
  - **CI**: Added GitHub Actions for linting, testing (Linux, macOS, Windows),
    and wrapper builds.

### Wrappers

- **New: Swift** wrapper. SPM package wrapping a prebuilt XCFramework, published
  via GitHub Releases.
- **Go**: Now published with prebuilt static libraries for macOS and Linux,
  downloaded automatically via `go run .../cmd/fetch-lib`.
- **Kotlin/Android**: Now published to Maven Central with bundled `.so` files.
  No Rust toolchain or cargo-ndk needed for consumers.
- **Python**: Now published to PyPI with platform-specific wheels (macOS arm64
  & x86, Linux x86 & arm64). `pip install` just works.

### Backend

- **Scalability improvements**: The backend infrastructure can now easily scale
  horizontally, so we should be able to react to more load by throwing more
  resources at the problem.
