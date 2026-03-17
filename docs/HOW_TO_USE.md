# How to use it

This guide shows how to integrate Wispers Connect into your application. There
are two main approaches: embedding the library directly, or using `wconnect` as
a sidecar process.

## Integration patterns

### Embed the library

Link the wispers-connect library into your application for full control over the
node lifecycle, serving, and peer-to-peer connections. This is the right choice
when your app needs to manage connections directly — for example, a file sync
app or a collaborative editor.

The library is written in Rust and exposes a C FFI. Wrappers exist for
Kotlin/Android and Go; more are planned (Swift, Python). See
[Building](../README.md#building) for setup instructions.

### Use wconnect as a sidecar

Run `wconnect` alongside your application to get port forwarding and HTTP
proxying without linking any library. This is the right choice when you have an
existing web app or TCP service and just want to make it reachable across
devices.

<!-- TODO: brief pointer to the sidecar section below -->

## Using the library

### Prerequisites

<!-- TODO: what you need before integrating:
     - A Wispers Connect domain (from the web UI)
     - An API key for creating connectivity groups and registration tokens
     - The client library built for your platform -->

### Storage

<!-- TODO: explain the NodeStateStore interface.
     The library needs persistent storage for root keys and registration.
     Cover:
     - What's stored (root key, registration protobuf)
     - Built-in options (in-memory for testing, file-based for CLI)
     - Implementing custom storage (e.g. Android SharedPreferences, Keychain)
     - Point to wrapper-specific examples in examples/ -->

### Node lifecycle

The typical flow is: register, activate, serve, connect.

<!-- TODO: explain each step at a conceptual level (not per-wrapper code).
     Reference HOW_IT_WORKS.md for the protocol details.

     #### Registration
     - Integrator backend creates a registration token via REST API
     - Token is handed to the app (deep link, QR code, paste)
     - App calls register(token)

     #### Activation
     - Bootstrap: first two nodes pair with each other
     - Endorsement: an activated node endorses a new one
     - (Future: hub-trusted mode skips activation entirely)

     #### Serving
     - What it does (connects to Hub, makes node reachable)
     - ServingHandle vs ServingSession (see INTERNALS.md)
     - Generating activation codes for new nodes

     #### Connecting to peers
     - UDP: low-latency, fire-and-forget
     - QUIC: reliable, multiplexed streams
     - Opening connections, sending/receiving data

     #### Logout
     - What it does at each state (deregister, self-revoke, delete local state)
-->

### Error handling

<!-- TODO: cover common error scenarios:
     - Hub unreachable
     - Unauthenticated (node removed server-side)
     - Invalid activation code
     - Peer rejected / unavailable
     - State-inappropriate operations (InvalidState)
-->

### Examples

Complete, runnable examples for each wrapper live in the `examples/` directory.

<!-- TODO: create examples/ with at minimum:
     - examples/rust/    — simple echo service
     - examples/go/      — same in Go
     - examples/kotlin/  — Android integration
     Each should be self-contained and buildable. -->

## Using wconnect as a sidecar

<!-- TODO: explain the sidecar pattern:
     - wconnect runs as a separate process alongside your app
     - Your app doesn't link the library at all
     - wconnect handles registration, activation, serving

     ### Port forwarding
     - Forward a local TCP port to a peer node's port
     - Example: expose a dev server to a teammate's laptop

     ### HTTP proxying
     - Proxy HTTP requests to a peer node
     - Example: access an internal web app from outside the office

     ### Running as a daemon
     - `wconnect serve -d` for background operation
     - Status, shutdown via Unix socket
     - See INTERNALS.md for daemon architecture details
-->

## Real-world examples

These show how the pieces fit together in actual deployments.

### Wispers Files (library integration)

<!-- TODO: describe the Files architecture:
     - Desktop app (Tauri) and Android app both embed the library
     - Registration via files.wispers.dev web UI + deep links
     - Serving runs in the background for file sync
     - QUIC streams for reliable file transfer
     - Point to the Files source as a reference -->

### Internal web app (wconnect sidecar)

<!-- TODO: describe a concrete scenario:
     - A team runs an internal web app (e.g. wiki, dashboard)
     - One team member runs `wconnect serve` + `wconnect proxy` on the server
     - Other team members run `wconnect` on their laptops
     - The web app is now accessible across NATs without a VPN
     - No code changes to the web app needed -->
