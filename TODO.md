# Remove App/Profile Namespacing from Library

Push namespace responsibility to storage implementations. The library should manage
"a node" without caring about how the storage is namespaced - that's the app's job.

## Motivation

- Storage is already abstracted for mobile integration
- Mobile platforms have native isolation (iOS Keychain groups, Android KeyStore aliases)
- Library managing namespaces creates duplication with platform-native mechanisms
- Simpler API: storage just stores one node's state

## Status: DONE

All phases completed:

- [x] Phase 1: Simplified storage trait and implementations
- [x] Phase 2: Updated state machine (state.rs)
- [x] Phase 3: Updated FFI (Rust + C header)
- [x] Phase 4: Updated CLI (wconnect)
- [x] Phase 5: Updated tests

## Summary of Changes

### Storage trait (`storage/mod.rs`)
```rust
pub trait NodeStateStore: Send + Sync + 'static {
    type Error;
    fn load(&self) -> Result<Option<NodeState>, Self::Error>;
    fn save(&self, state: &NodeState) -> Result<(), Self::Error>;
    fn delete(&self) -> Result<(), Self::Error>;
}
```

### NodeState (`types.rs`)
- Removed `app_namespace` and `profile_namespace` fields
- Removed `AppNamespace` and `ProfileNamespace` types
- Simplified to just `root_key` and `registration`

### State API
- `NodeStorage::restore_or_init_node_state()` - no longer takes namespace params
- `NodeStorage::read_registration()` - no longer takes namespace params
- Removed `registration_url()` method (was unused)
- Removed `app_namespace()` and `profile_namespace()` accessors from state types

### FFI
- Callbacks no longer receive namespace strings
- `ctx` pointer carries all host context (including any namespace info)
- Removed `wispers_pending_state_registration_url()`

### CLI
- Constructs store path explicitly: `config_dir/wconnect/default/`
