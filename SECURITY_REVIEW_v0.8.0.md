# Security review — `connect/client` (pre v0.8.0)

**Reviewer:** Claude (Opus 4.6)
**Date:** 2026-04-10
**Scope:** `connect/client/wispers-connect/` (Rust core), `connect/client/wispers-connect/src/ffi/`. Wrappers, `wconnect`, `wcadm` not reviewed in depth.
**Threat model assumed (per author):**
- Hub is "generally benevolent" but **may be taken over by an attacker**. The advertised value prop is that the hub cannot eavesdrop or inject malicious nodes — it can only DoS.
- On-path network attackers are in scope.
- Compromised activated nodes are in scope but accepted (mitigated by easy revocation).
- Rooted devices are out of scope; another local user account is in scope.

This review focused on: cryptographic primitives, the activation protocol end-to-end, roster verification (author-flagged), the hub trust boundary, encryption layer, FFI memory safety, and storage. `cargo audit` could not be run (`cargo-audit` not installed). Wrappers and CLI tools were not exhaustively reviewed.

---

## Summary

| ID | Severity | Title | Blocks v0.8.0? |
|---|---|---|---|
| C-1 | **Critical** | Bootstrap activation does not authenticate the endorser's public key | **Yes** |
| H-1 | High | Pairing secret has ~51.7 bits of effective entropy (offline-crackable) | **Yes** |
| M-1 | Medium | FFI `&mut self` exposed via raw pointers without internal synchronization | No, but document loudly |
| M-2 | Medium | P2P signed messages lack domain separation | No |
| L-1 | Low | No anti-replay window at UDP `Decrypter` | No (intentional) |
| L-2 | Low | No hub TLS certificate pinning | No |
| L-3 | Low | File-store permission/atomicity race in `write_private` | No |
| L-4 | Low | Root key bytes linger in non-zeroizing `Vec<u8>` during load | No |
| I-1 | Info | `cargo audit` not run for this review | Run before tag |
| I-2 | Info | Hand-rolled constant-time eq; `derive_mac_key` is HMAC-as-PRF (unusual but sound) | No |

**Bottom line on the v0.8.0 tag:** **C-1 alone blocks it.** It directly invalidates the "you don't have to trust the hub" guarantee for the very first activation in any new connectivity group. H-1 is independently bad enough that I would also block on it: it lets a determined hub adversary break activation security via offline brute force, regardless of bootstrap-vs-not. The two findings combined would let a malicious hub completely own a new connectivity group from day one.

The good news: the **non-bootstrap** activation path is structurally sound — the `base_version_hash` chain in `roster.rs` correctly anchors trust back through history and prevents the same kind of substitution attack. The roster verification logic, which the author flagged for doubts, is largely correct (the "rewrite cleaner" instinct is reasonable for readability, but the algorithm itself works).

---

## C-1 — Bootstrap activation does not authenticate the endorser's public key (CRITICAL)

**Files:** `wispers-connect/src/node.rs:570-698`, `wispers-connect/src/roster.rs:336-345`

### What's wrong

In the activation flow, the new node receives the endorser's public key via the HMAC-authenticated `pair_nodes` exchange (`node.rs:608-618`). For **non-bootstrap** activations (`current_roster.version > 0`), the new node correctly uses that paired key as the trust anchor when verifying the existing roster:

```rust
// node.rs:637-644
if current_roster.version > 0 {
    verify_roster(
        &current_roster,
        endorser_node_number,
        &response_payload.public_key_spki, // ← HMAC-paired key as anchor
    )?;
}
```

After submitting the activation, however, the cosigned roster returned by the hub is verified using **only the new node's own key** as the trust anchor:

```rust
// node.rs:687-693
verify_roster(
    &cosigned_roster,
    registration.node_number,                  // ← self
    &self.signing_key.public_key_spki(),       // ← self
)?;
```

For non-bootstrap, this is fine because the chain walk through `verify_roster_chain` → `verify_base_hash` re-derives the previous roster from the cosigned one and verifies its hash matches `payload.base_version_hash` in each addendum, all the way back to version 1. Since the new node signed `payload.base_version_hash = hash(real current_roster)`, any substitution by the hub would produce a hash mismatch.

For **bootstrap** (`current_roster.version == 0`):
- `verify_activation` skips the "endorser must have been active before" check (`roster.rs:336-345`, `is_bootstrap` branch).
- `verify_base_hash` is skipped entirely (`roster.rs:357`, `if expected_version > 1`).
- The HMAC-paired endorser key is used to **build** the bootstrap roster (`node.rs:660-666`), but is **never compared** to whatever the hub returns.

### Concrete attack

A malicious hub can perform the following without breaking any cryptographic primitive:

1. Observe the `pair_nodes` flow — the hub routes both messages, so it sees the new node's nonce, the (real) endorser's nonce, and both public keys (it just can't forge HMACs without the secret).
2. The new node sends `update_roster(bootstrap_roster_v1)` with the **real** endorser key in `nodes[]` and its **real** signature on the activation payload.
3. The hub does **not** forward to the real endorser. Instead it:
   - Generates a fresh Ed25519 key pair `fake_endorser_key`.
   - Constructs `fake_cosigned_roster` with `nodes = [new_node (real key), endorser_node_number (FAKE key)]` and the same activation payload (which contains only node numbers and nonces — no keys).
   - Signs the activation payload with `fake_endorser_key` and uses that as `endorser_signature`.
   - Returns `fake_cosigned_roster` to the new node.
4. New node runs `verify_roster(fake_cosigned_roster, self_node_number, self_key)`:
   - Verifier-in-roster check passes (own entry, own key).
   - `verify_roster_chain` walks back from version 1: bootstrap activation, `is_bootstrap = true`, endorser-active check skipped. New node signature verifies under real new-node key. Endorser signature verifies under `fake_endorser_key` — which is the key sitting in the roster. **Verification passes.**
5. New node transitions to `Activated` and stores `fake_cosigned_roster` as its trusted roster.
6. The next time the new node calls `connect_udp` / `connect_quic` for the endorser, `find_peer_in_roster` returns the entry with `fake_endorser_key`. The Ed25519 signature verification on the answerer's `StartConnectionResponse` (`node.rs:817`) is checked **against `fake_endorser_key`**. The hub answers the connection (impersonating the endorser) and signs with `fake_endorser_key`. Connection establishes; the hub now sits on a confidential P2P session the new node believes is with the endorser.

This is end-to-end impersonation by the hub for any node that bootstraps a new connectivity group. The **first** activation in every group is vulnerable. Subsequent activations are fine because the chain anchor catches them.

### Why the existing nonce binding doesn't save you

The activation payload contains `new_node_nonce` and `endorser_nonce`, which the endorser's serving session checks against its `pending_endorsements` state (`serving.rs:272-277`). This is a defense against the hub *replaying* old payloads to the endorser — but in this attack the hub never contacts the real endorser at all. The nonces are visible to the hub on the wire (the `pair_nodes` payload is HMAC-authenticated, not encrypted), so the hub can copy them into the fake activation payload it signs with `fake_endorser_key`.

### Fix

After verifying the cosigned bootstrap roster with the new node's own key as the anchor, also check that the endorser's entry in the roster matches the HMAC-paired public key:

```rust
// in node.rs::activate(), after the verify_roster call at line 687:
if current_roster.version == 0 {
    let endorser_in_roster = cosigned_roster
        .nodes
        .iter()
        .find(|n| n.node_number == endorser_node_number)
        .ok_or(NodeStateError::RosterVerificationFailed(
            RosterVerificationError::VerifierNotInRoster(endorser_node_number),
        ))?;
    if endorser_in_roster.public_key_spki != response_payload.public_key_spki {
        return Err(NodeStateError::RosterVerificationFailed(
            RosterVerificationError::VerifierKeyMismatch(endorser_node_number),
        ));
    }
}
```

A more principled refactor (which I'd recommend if you do touch `verify_roster` for cleanliness) is to give it a list of trust anchors instead of one, and require *all* of them to match. The bootstrap case then naturally passes both `(self, self_key)` and `(endorser, paired_key)`. The non-bootstrap case can also be made to pass `(endorser, paired_key)` for symmetry, though it isn't strictly required there.

I would also strongly recommend adding a regression test that constructs exactly the fake-cosigned-roster attack above and asserts that bootstrap activation rejects it. Without such a test the same class of bug could come back during a refactor.

---

## H-1 — Pairing secret has ~51.7 bits of effective entropy (HIGH)

**Files:** `wispers-connect/src/crypto.rs:14-199`

### What's wrong

`PairingSecret` is documented and implemented as 7 bytes = 56 bits of randomness. The base36 encoding to 10 characters caps the actual codomain at `36^10 ≈ 2^51.7` distinct values. The `generate()` function then deliberately round-trips through base36 to keep the in-memory representation consistent with anything a user might paste, and `encode_base36` truncates to the **last 10 characters** when the BigUint is longer than 10 base36 digits, which happens for most random 7-byte inputs (since `36^10 < 2^56`):

```rust
// crypto.rs:115-122
pub fn generate() -> Self {
    let mut bytes = [0u8; PAIRING_SECRET_LEN]; // 7 bytes
    rand::thread_rng().fill_bytes(&mut bytes);
    let base36 = encode_base36(&bytes);
    let bytes = decode_base36(&base36).expect("just encoded");
    Self { bytes }
}

// crypto.rs:177-185
fn encode_base36(bytes: &[u8]) -> String {
    let s = BigUint::from_bytes_be(bytes).to_str_radix(36);
    match s.len().cmp(&10) {
        ...
        std::cmp::Ordering::Greater => s[s.len() - 10..].to_string(), // ← clips
    }
}
```

So the effective entropy is `log2(36^10) ≈ 51.7 bits`, and the nominal "56 bits" claim is incorrect by ~4 bits.

### Why this matters under your threat model

The `PairNodesMessage` flow routes through the hub. The hub sees:
- The plaintext `PairNodesMessage.Payload` bytes.
- The 16-byte HMAC-SHA256 tag computed as `HMAC(derive_mac_key(secret), payload_bytes)`, where `derive_mac_key` is itself two HMAC-SHA256 calls keyed by the secret.

With the hub considered actively malicious, this is enough to mount an offline brute force: for each candidate secret in `[0, 36^10)` (~2^52 candidates), derive the MAC key (2 HMACs), HMAC the payload (1 HMAC), and compare 16 bytes. Roughly 3 SHA256 compressions per HMAC ≈ 9 SHA256 ops per candidate ≈ `2^52 * 2^3 = 2^55` SHA256 ops total.

A single modern GPU sustains roughly 10⁹–10¹⁰ SHA256/s. That's ~1–10 hours per attacked secret on one GPU. Cloud-rented at modest scale (10–100 GPUs, ~$10–$100 per attack) it's minutes. Once the secret is recovered, the hub can fabricate either side of the pairing exchange — i.e., MITM the activation — because the only thing protecting the public-key exchange is HMAC under that secret.

Compounded with C-1, an attacker with hub control can fully own the activation flow: H-1 for non-bootstrap (cracks pairing → MITMs the public-key exchange → hijacks activation), C-1 for bootstrap (no offline crack needed at all).

### Fix

Increase `PAIRING_SECRET_LEN` to **at least 10 bytes** (80 bits), preferably 12 (96 bits). Two reasonable encodings:

- **12 base36 characters** (`36^12 ≈ 2^62`): cleaner UX than 10 chars only marginally, decent margin against offline crack.
- **16 base32 characters** (`32^16 = 2^80`): still finger-typeable, ~80 bits is the conventional floor for a key that gets brute-forced. Crockford base32 is friendly to manual entry (no ambiguous chars).

Either way:
1. Drop the round-trip-through-base36 trick in `generate()`. Generate the random bits at the desired length and encode forward only.
2. Fix `encode_base36` to **not clip** — assert or pad, never truncate.
3. Add a test that asserts `2^(8*PAIRING_SECRET_LEN) <= base_radix^encoded_len` so this regression can't recur silently.
4. Update the wrappers and any UI that displays the activation code length.

Note that this is a **breaking change** for the activation code wire format. If you'd rather avoid coordinating a bump, the minimum patch is to fix the encoding clipping (keep 7 bytes = 56 bits, which is borderline acceptable but at least lives up to the comment) — but I recommend doing the full upgrade to ≥80 bits now while you're tagging anyway.

---

## M-1 — FFI exposes `&mut self` via raw pointers without internal synchronization (MEDIUM)

**Files:** `wispers-connect/src/ffi/node.rs:208-302, 401-420`

`wispers_node_register_async` and `wispers_node_activate_async` take `*mut WispersNodeHandle`, wrap it in `SendableNodePtr`, ship it into a spawned tokio task, and call `.get_mut()` on it to obtain `&mut WispersNodeHandle` for `Node::register` / `Node::activate` (both `&mut self`).

```rust
// ffi/node.rs:401-419
struct SendableNodePtr(*mut WispersNodeHandle);
unsafe impl Send for SendableNodePtr {}

impl SendableNodePtr {
    unsafe fn get_mut(&self) -> &mut WispersNodeHandle {
        unsafe { &mut *self.0 }
    }
}
```

The doc comment says "The caller must ensure the handle remains valid for the lifetime of the spawned task and is not accessed concurrently from other threads." This is correct as a contract, but it places an unsoundness footgun on the wrapper authors:

- A second `wispers_node_register_async` call before the first's callback fires creates two `&mut WispersNodeHandle` aliases simultaneously → instant UB.
- `wispers_node_state` (`ffi/node.rs:194`) takes `&*handle` (`&self`) — calling it while a register/activate task is in flight is `&self` aliasing `&mut self` → UB.
- A wrapper using async APIs (Kotlin coroutines, Swift Tasks) is going to be tempted to dispatch these concurrently.

This isn't theoretical — in pathological cases the optimizer can reorder and reorder loads/stores assuming exclusive access. It also shows up under MIRI / TSan instantly.

### Fix

Either:
- (Simpler) Wrap the inner `Node` in a `tokio::sync::Mutex` (or `parking_lot::Mutex`, since the held duration is FFI-call scope) inside `WispersNodeHandle` and acquire it for the duration of any async operation. Concurrent calls will then serialize harmlessly.
- (Lower overhead) Refactor `Node::register` / `Node::activate` to take `&self` and move state mutation behind the `RwLock` you already have for `roster`. Then `SendableNodePtr` only needs `get` (`&self`), which is at least sound under aliasing rules — but you still want callback-frame validity guarantees and a way to refuse concurrent register/activate.

Given the bug class and the known JNA pitfalls already in your project memory, I'd recommend the Mutex approach. The cost is negligible compared to a network round trip and removes a whole category of latent UB across all wrappers.

---

## M-2 — P2P signed messages lack domain separation (MEDIUM)

**Files:** `wispers-connect/src/node.rs:779-791, 877-890`, `wispers-connect/src/serving.rs:691-751`

The caller and answerer signing constructions in `connect_udp`, `connect_quic`, and `handle_start_connection_request` concatenate fields without a prefixed domain tag and without length encoding:

```rust
// caller (node.rs:779-783)
let mut message_to_sign = Vec::new();
message_to_sign.extend_from_slice(&peer_node_number.to_le_bytes()); // 4 bytes
message_to_sign.extend_from_slice(&encryption_key.public_key());    // 32 bytes
message_to_sign.extend_from_slice(caller_sdp.as_bytes());           // variable

// answerer (serving.rs:747-751)
let mut message_to_sign = Vec::new();
message_to_sign.extend_from_slice(&connection_id.to_le_bytes());    // 8 bytes
message_to_sign.extend_from_slice(&encryption_key.public_key());    // 32 bytes
message_to_sign.extend_from_slice(answerer_sdp.as_bytes());         // variable
```

The same Ed25519 signing key is used for both. As coded, replay between the two contexts is prevented in practice because (a) the field widths differ (4 vs 8 bytes for the ID prefix) and (b) the ephemeral X25519 keys make accidental collisions vanishingly unlikely. **No exploitable issue today.** But:

- Adding any new signed message in the future risks a confusion attack if the prefix happens to align.
- The lack of length-prefixing means an attacker who can get the system to sign over user-controlled SDP could in principle craft a message that, when parsed in another context, has a meaningful first-N-bytes interpretation.

### Fix

Add a single-byte (or single-string) domain tag at the start of each signed message:

```rust
const SIG_CTX_CALLER: &[u8] = b"wc/sig/caller-v1\0";
const SIG_CTX_ANSWERER: &[u8] = b"wc/sig/answerer-v1\0";
// caller:
message_to_sign.extend_from_slice(SIG_CTX_CALLER);
// answerer:
message_to_sign.extend_from_slice(SIG_CTX_ANSWERER);
```

Length-prefix the variable-width SDP field while you're there. This is a wire-breaking change (existing peers won't verify each other's signatures), so bundle it with the v0.8.0 tag if you do it.

---

## L-1 — No anti-replay window at the UDP `Decrypter` (LOW)

**File:** `wispers-connect/src/encryption.rs:80-117`

`Decrypter::decrypt` accepts any `seqno` an attacker presents and uses it only for nonce derivation. There is no sliding window, no high-water-mark, no dedup. An on-path attacker who captures a UDP packet can replay it later and the application will see the plaintext as a fresh packet.

The current comment on the type says "Does not enforce sequence ordering — packets can arrive out of order or be lost." That's true and correct as a *transport-layer* design choice — but the doc should also explicitly state that there is **no replay protection**, so wrappers/integrators don't assume there is.

For Wispers Files specifically: my recollection is that QUIC is the actual transport for the file flows, and QUIC has its own replay/order semantics on top. So the practical risk is contained. But anything using raw `UdpConnection` needs to know.

### Fix
- Documentation-only is acceptable: add a clear "NO REPLAY PROTECTION" note to `UdpConnection` and `Decrypter`, and mention it in `HOW_IT_WORKS.md`.
- Or implement a sliding window (e.g. 64 entries, IPsec-style) inside `Decrypter` keyed by seqno. This is straightforward but adds shared mutable state.

---

## L-2 — No hub TLS certificate pinning (LOW)

**File:** `wispers-connect/src/hub.rs:90-109`

`HubClient::connect` uses platform / webpki roots and accepts any certificate that chains to a trusted CA for the hub host. A compromised CA — or a state-level adversary that obtains a misissued cert for `hub.connect.wispers.dev` — can fully MITM the gRPC stream.

The activation/connection security model **does not depend on hub TLS** (the cryptographic chain is independent), so a successful TLS MITM can't impersonate nodes or read P2P traffic. But it can:
- Read all metadata (who is in which group, who is online, who is connecting to whom).
- Selectively drop messages, including roster updates and pair_nodes flows — i.e., DoS with much better granularity than a network adversary.
- Combine with C-1 to fully own bootstrap activations from a position outside the hub.

### Fix
Pin the hub's leaf cert public key (SPKI hash) at compile time, with the option to override for tests. The pin can be rotated by shipping new client versions before rotating the cert.

---

## L-3 — File-store permission/atomicity race (LOW)

**File:** `wispers-connect/src/storage/file.rs:90-131`

`write_private` calls `fs::write(path, data)` and *then* `fs::set_permissions(0o600)`. There's a brief window where the file exists with the process umask's default mode (commonly 0o644). Similarly, `save()` only sets the directory mode 0o700 *if* it creates the directory; an existing directory with looser permissions is left alone.

### Fix

```rust
use std::os::unix::fs::OpenOptionsExt;
let mut f = std::fs::OpenOptions::new()
    .write(true).create(true).truncate(true)
    .mode(0o600)
    .open(path)?;
f.write_all(data)?;
f.sync_all()?;
```

Tighten the directory mode unconditionally on `save()` (or warn if it's looser).

---

## L-4 — Root key bytes linger in non-zeroizing `Vec<u8>` during load (LOW)

**File:** `wispers-connect/src/storage/file.rs:55-62`

```rust
let root_key_bytes = fs::read(&root_key_path)?;     // ← Vec<u8>, NOT zeroized
if root_key_bytes.len() != ROOT_KEY_LEN { ... }
let mut key_array = [0u8; ROOT_KEY_LEN];
key_array.copy_from_slice(&root_key_bytes);
```

`RootKey` correctly zeroizes on drop (`types.rs:35-39`), but the intermediate `Vec` from `fs::read` does not. After the `copy_from_slice`, the bytes remain in heap memory until the Vec is dropped at end of function — and even then, deallocation does not zero the bytes. A process memory dump (or another tenant after free + reuse) could still recover the root key.

### Fix

```rust
use zeroize::Zeroizing;
let root_key_bytes: Zeroizing<Vec<u8>> = Zeroizing::new(fs::read(&root_key_path)?);
```

Or read directly into a stack array:

```rust
let mut key_array = [0u8; ROOT_KEY_LEN];
let mut f = std::fs::File::open(&root_key_path)?;
f.read_exact(&mut key_array)?;
// optional: assert EOF to catch wrong-length files
```

The same pattern applies to `ForeignNodeStateStore::call_load_root_key` (`storage/foreign.rs:66-75`) — the stack `[u8; ROOT_KEY_LEN]` there does get moved into the (zeroizing) `RootKey`, so the heap risk is lower, but consider zeroizing on the buffer if the load fails.

---

## I-1 — `cargo audit` was not run

`cargo-audit` is not installed in the review environment, so I could not check the dependency graph against the RustSec advisory database. Please run before tagging:

```bash
cargo install cargo-audit
cd connect/client && cargo audit
```

The dependency list in `wispers-connect/Cargo.toml` looks healthy at a glance — `aes-gcm 0.10`, `ed25519-dalek 2.x`, `x25519-dalek 2.x`, `hkdf 0.12`, `sha2 0.10`, `tonic 0.12`, `quiche 0.24`, `boring 4` are all current. No obvious red flags but `cargo audit` is the authoritative check.

---

## I-2 — Sound but unconventional crypto idioms (informational)

These aren't bugs — flagging them so a future reader doesn't have to re-derive that they're OK:

- **`crypto.rs:265-271` constant_time_eq** is correctly written (length check then XOR-OR fold). Consider switching to `subtle::ConstantTimeEq` for clarity and to make sure a future compiler can't outsmart it.
- **`crypto.rs:141-148` derive_mac_key** uses `HMAC(secret, "wispers-pairing-v1" || "wispers-pairing-v1|mac")` rather than HKDF-Extract/Expand. Functionally fine — HMAC is a PRF — but unconventional. The comment correctly notes it matches the Go implementation. If the Go side ever switches to HKDF, switch both.
- **`encryption.rs:190-211` derive_direction_keys** uses `connection_id` as HKDF salt. Hub-controlled salt is harmless for HKDF security, and even if the hub picked the same `connection_id` for two sessions, the underlying X25519 shared secret is fresh per session, so AEAD keys are still distinct. Good.
- **`quic.rs::derive_psk`** ties the QUIC PSK to a separate domain string (`"wispers-connect-quic-v1"` / `"tls13-psk"`) from the UDP encryption HKDF. No key reuse between layers. Good.

---

## What I did NOT review (and recommend before any "audited" claim)

- The wrappers (`wrappers/kotlin`, `wrappers/swift`, `wrappers/go`, `wrappers/python`) — only the Rust FFI surface. Wrappers may have their own callback-lifetime, JNA-padding, and concurrency bugs. The pre-existing project memory already lists several historical issues here.
- `wconnect` and `wcadm` CLIs — credential handling, profile state file permissions, and how they print/log secrets. The library-level review covers everything they call into, but the CLIs themselves can leak (e.g. activation codes in shell history, in logs, on screen).
- `juice.rs` libjuice bindings (27 unsafe blocks) — I confirmed the unsafe surface is concentrated in the FFI module as expected, but did not audit the libjuice bindings line by line. libjuice itself is a vendored C dependency and has had its own CVEs historically.
- Protobuf parsing trust boundary — `prost` is generally safe but I did not exhaustively check that every untrusted decode (e.g. roster fetched from hub before verification) handles malformed/maliciously-large input gracefully. Worth fuzzing.
- The Connect backend's removal of the invite-code requirement. This review confirmed the *library* contains zero references to "invite", so the backend change has no client-side surface — but the backend itself is out of scope.

A `cargo fuzz` target around `verify_roster` and the `pair_nodes` handlers would be very high value before claiming any kind of audited status. The bootstrap finding (C-1) would have shown up in a few minutes of fuzzing-with-a-hostile-hub.

---

## Recommended ordering before tagging v0.8.0

1. **Block tag on:** fix C-1 and H-1. Add regression tests for both.
2. **Strongly recommend before tag:** fix M-1 (wrap inner Node in a Mutex), document M-2, run `cargo audit`.
3. **Can ship in v0.8.1:** L-1 documentation, L-3, L-4, L-2 (cert pinning is a meaningful design decision worth deliberate scheduling), M-2 wire-breaking domain separation if not bundled with the tag.

If you'd like, I can implement the C-1 and H-1 fixes plus tests in a follow-up — they're both small surgical changes (~30 lines each) and the regression tests are the more interesting part.
