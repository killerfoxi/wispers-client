<center>
  <img src="docs/images/wispers-connect-logo-and-text-trans.svg" width="256" alt="Wispers Connect logo"/>
</center>

## About Wispers Connect

Wispers is an application-level secure overlay network. It connects software running on different devices with secure, NAT-traversing, peer-to-peer connections.

A central rendezvous server coordinates connection setup, but unlike other NAT-traversal systems, you don't have to trust it. Nodes verify each other using a cryptographic roster, so neither the server nor any other infrastructure can eavesdrop or tamper with traffic.

You just link the wispers-connect library (or run a sidecar process), and your software can now connect to other instances, securely and directly. The library is written in Rust and C and has wrappers for a growing set of other languages.

## Quick start

As an introduction to how things work, let's set up two nodes communicating over a peer-to-peer connection.

### 0. Prerequisites

- A local clone of the `wispers-connect` repository (this one)
- A [Rust development environment](https://rust-lang.org/tools/install/). We'll use terminal commands, but feel free to use your IDE of choice.

### 1. Get a Connect account & API key

Establishing connections using NAT-traversal requires a rendezvous server, which we call the "hub" in Wispers. We need to tell the hub about you and your use case. To do that,

1. Get an account at https://connect.wispers.dev. You'll get a personal account with a **domain** named "Default" already set up for you. Domains map to use cases — say you create a new app with Wispers Connect, you'd use a separate domain for that.
2. Create an **API key** for the "Default" domain. Click on the domain in the web UI and the form is right there. Give it a name like "test" and make sure to copy down the key! Wispers doesn't store it.

### 2. Create a connectivity group

At this point, we leave the web UI and take to the terminal to create our first **connectivity group**. These groups are the basic unit of connectivity in Wispers. All nodes in a connectivity group can talk to each other and (after activation) trust each other. In [Wispers Files](https://files.wispers.dev) for example, each user gets their own connectivity group to connect their to devices to each other.

For this introduction, we'll create a single group named "quick-start" using the `wcadm` tool. From the root of the repository, run

```bash
export WC_API_KEY=$YOURKEY # Your API key, "wc_prod_..."
cargo run --bin wcadm add-group --name=quick-start
```

The output will show a UUID for your newly created connectivity group. You'll need that to add nodes to the group.

Note that you don't _have_ to use the `wcadm` tool for this. When writing an application that uses Wispers Connect, you'll often want to use the REST API at https://connect.wispers.dev/api from your own code instead.

### 3. Register nodes

**Nodes** are the things that actually communicate in Wispers You usually create your own by linking the wispers-connect library, but we've also created a command line tool that you can use as a sidecar or for testing, `wconnect`. We'll use it here to demonstrate node registration & activation. If you want to, you can do this from two different devices, but two terminals are sufficient.

Let's start by registering a node with the connectivity group we've just created. First, we need a registration token (Make sure to set `YOUR_GROUP_ID` to the connectivity group ID from step 2).

```bash
cargo run --bin wcadm -- \
    create-registration-token ${YOUR_GROUP_ID} \
    --name="first node"
```

This prints a registration token. Let's use it to actually register the node.

```bash
cargo run --bin wconnect -- \
    --profile="quick-start-1" \
    register ${YOUR_REGISTRATION_TOKEN}
```

In case you're wondering: The `--profile` parameter lets you register multiple nodes on the same computer, under the same user — perfect for our quick-start.

Let's register another node, so we have two we can connect to each other.

```bash
cargo run --bin wcadm -- \
    create-registration-token ${YOUR_GROUP_ID} \
    --name="second node"

# Note the registration token and use it in the next command.

cargo run --bin wconnect -- \
    --profile="quick-start-2" \
    register ${YOUR_REGISTRATION_TOKEN}
```

At this point, the nodes can send each other messages through the hub, but it still requires trusting the Wispers Connect backend not to eavesdrop on messages or impersonate nodes.While we assure you the Wispers team is trustworthy, we don't want you to have take our word for it.

### 4. Activate the nodes

Once nodes are registered, you **activate** them to establish peer-to-peer trust. This is the step that makes it impossible for the hub to eavesdrop or impersonate. Each new node activation iteratively creates a cryptographic roster for the group. Nodes on that roster can communicate directly and securely.

TODO: continue here



<!-- TODO: minimal end-to-end example showing the three steps:
     1. Register a node (get a token from the integrator, call register())
     2. Activate (pair two nodes using a code)
     3. Connect (open a QUIC stream to a peer)
     Show code for one wrapper (Rust or Go) and link to HOW_TO_USE.md for
     all wrappers and more examples. -->

## Documentation

- **[How it works](docs/HOW_IT_WORKS.md)** — Transport, security model,
  and protocol design
- **[How to use it](docs/HOW_TO_USE.md)** — Integration guide with
  examples for each wrapper
- **[Internals](docs/INTERNALS.md)** — Code map, module responsibilities,
  and key types

## Building

<!-- TODO: cover building the Rust library, cross-compiling for Android
     (cargo-ndk), and linking from Go/Kotlin. Keep it short — point to
     HOW_TO_USE.md for wrapper-specific setup. -->

## License

<!-- TODO -->
