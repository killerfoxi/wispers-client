# Remove App/Profile Namespacing from Library

Push namespace responsibility to storage implementations. The library should manage
"a node" without caring about how the storage is namespaced - that's the app's job.

## Motivation

- Storage is already abstracted for mobile integration
- Mobile platforms have native isolation (iOS Keychain groups, Android KeyStore aliases)
- Library managing namespaces creates duplication with platform-native mechanisms
- Simpler API: storage just stores one node's state

## Phase 1: Simplify Storage Trait

### 1.1 Update `NodeStateStore` trait (`storage/mod.rs`)

Remove namespace parameters:

```rust
pub trait NodeStateStore: Send + Sync + 'static {
    type Error;
    fn load(&self) -> Result<Option<NodeState>, Self::Error>;
    fn save(&self, state: &NodeState) -> Result<(), Self::Error>;
    fn delete(&self) -> Result<(), Self::Error>;
}
```

### 1.2 Simplify `NodeState` (`types.rs`)

Remove namespace fields entirely:

```rust
pub struct NodeState {
    pub(crate) root_key: RootKey,
    pub(crate) registration: Option<NodeRegistration>,
}

impl NodeState {
    pub fn new() -> Self {
        Self {
            root_key: RootKey::generate(),
            registration: None,
        }
    }
}
```

Delete:
- `AppNamespace` type
- `ProfileNamespace` type
- `DEFAULT_PROFILE_NAMESPACE` constant
- `initialize()` and `initialize_with_namespaces()` methods

### 1.3 Update `FileNodeStateStore` (`storage/file.rs`)

Constructor takes full path (already namespaced by caller):

```rust
impl FileNodeStateStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn root_key_path(&self) -> PathBuf {
        self.dir.join("root_key.bin")
    }

    fn registration_path(&self) -> PathBuf {
        self.dir.join("registration.json")
    }
}
```

Remove `with_app_name()` - callers construct paths themselves.

### 1.4 Update `InMemoryNodeStateStore` (`storage/in_memory.rs`)

Store single `Option<NodeState>` instead of `HashMap`:

```rust
pub struct InMemoryNodeStateStore {
    state: Arc<RwLock<Option<NodeState>>>,
}
```

### 1.5 Update `ForeignNodeStateStore` (`storage/foreign.rs`)

Callbacks no longer receive namespace strings:

```rust
pub struct WispersNodeStateStoreCallbacks {
    pub ctx: *mut c_void,
    pub load_root_key: Option<unsafe extern "C" fn(ctx: *mut c_void, out: *mut u8, len: usize) -> WispersStatus>,
    pub save_root_key: Option<unsafe extern "C" fn(ctx: *mut c_void, key: *const u8, len: usize) -> WispersStatus>,
    pub delete_root_key: Option<unsafe extern "C" fn(ctx: *mut c_void) -> WispersStatus>,
    pub load_registration: Option<unsafe extern "C" fn(ctx: *mut c_void, buf: *mut u8, len: usize, out_len: *mut usize) -> WispersStatus>,
    pub save_registration: Option<unsafe extern "C" fn(ctx: *mut c_void, buf: *const u8, len: usize) -> WispersStatus>,
    pub delete_registration: Option<unsafe extern "C" fn(ctx: *mut c_void) -> WispersStatus>,
}
```

The `ctx` pointer now carries all context the host needs (including any namespace info).

## Phase 2: Update State Machine (`state.rs`)

### 2.1 Simplify `NodeStorage` API

```rust
impl<S: NodeStateStore> NodeStorage<S> {
    pub fn new(store: S) -> Self { ... }

    pub fn read_registration(&self) -> Result<Option<NodeRegistration>, ...> {
        // No namespace params
    }

    pub async fn restore_or_init(&self) -> Result<NodeStateStage<S>, ...> {
        // No namespace params
    }
}
```

### 2.2 Remove namespace accessors from state types

Delete from `PendingNodeState`, `RegisteredNodeState`, `ActivatedNode`:
- `app_namespace()` method
- `profile_namespace()` method
- Internal namespace fields

### 2.3 Delete `registration_url()` method

`PendingNodeState::registration_url()` is unused - remove it entirely.

### 2.4 Simplify `logout()` implementations

All `logout()` methods become:
```rust
pub async fn logout(self) -> Result<(), ...> {
    self.store.delete().map_err(...)
}
```

## Phase 3: Update FFI (`ffi/`, `include/wispers_connect.h`)

### 3.1 Update C header

Remove namespace params from callbacks and `restore_or_init`:

```c
typedef struct {
    void *ctx;
    WispersStatus (*load_root_key)(void *ctx, uint8_t *out_key, size_t out_key_len);
    WispersStatus (*save_root_key)(void *ctx, const uint8_t *key, size_t key_len);
    WispersStatus (*delete_root_key)(void *ctx);
    WispersStatus (*load_registration)(void *ctx, uint8_t *buffer, size_t buffer_len, size_t *out_len);
    WispersStatus (*save_registration)(void *ctx, const uint8_t *buffer, size_t buffer_len);
    WispersStatus (*delete_registration)(void *ctx);
} WispersNodeStateStoreCallbacks;

WispersStatus wispers_storage_restore_or_init(
    WispersNodeStorageHandle *handle,
    WispersPendingNodeStateHandle **out_pending,
    WispersRegisteredNodeStateHandle **out_registered
);
```

### 3.2 Delete `wispers_pending_state_registration_url()`

Remove from header and implementation.

### 3.3 Update Rust FFI implementations

- `ffi/manager.rs`: Update restore_or_init binding
- `ffi/handles.rs`: Update any namespace usage

## Phase 4: Update CLI (`wconnect/`)

### 4.1 Update `main.rs`

CLI constructs namespaced path and passes to store:

```rust
fn get_store(app: &str, profile: &str) -> FileNodeStateStore {
    let base = dirs::config_dir()
        .expect("no config dir")
        .join("wconnect")
        .join(app)
        .join(profile);
    FileNodeStateStore::new(base)
}
```

Then calls `storage.restore_or_init()` with no namespace args.

### 4.2 Update daemon mode

Same pattern - construct store with full path, then use simplified API.

## Phase 5: Update Tests

- Update all tests that construct `NodeState` or call `restore_or_init`
- Tests that need multiple "nodes" create multiple store instances
- `InMemoryNodeStateStore` tests simplify significantly

## Migration Notes

- This is a breaking change to the FFI API
- Mobile SDK wrappers will need updates
- The ctx pointer in callbacks now must carry namespace context if the host needs it
- File storage layout unchanged (just constructed differently)
