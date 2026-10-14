// Metadata registrations do not publish executable pointers. Publication and
// retirement share this lock, so an owner cannot be retired while a trampoline
// is being installed. Published code remains leased after callback close.
#[derive(Default)]
struct CallbackCodeRegistry {
    next_owner: i64,
    sequence: u64,
    owners: crate::hash::HashMap<i64, CallbackCodeOwner>,
    entries: crate::hash::HashMap<String, Vec<CallbackCodeEntry>>,
}

#[derive(Default)]
struct CallbackCodeOwner {
    published: bool,
    retiring: bool,
}

struct CallbackCodeEntry {
    owner: i64,
    address: usize,
    sequence: u64,
}

impl CallbackCodeRegistry {
    fn new_owner(&mut self) -> i64 {
        let Some(id) = self.next_owner.checked_add(1) else {
            return 0;
        };
        self.next_owner = id;
        self.owners.insert(id, CallbackCodeOwner::default());
        id
    }

    fn register(&mut self, key: String, address: usize, owner: i64) -> bool {
        if address == 0 || (owner != 0 && !self.owners.get(&owner).is_some_and(|s| !s.retiring)) {
            return false;
        }
        let entries = self.entries.entry(key).or_default();
        // A generation's adapter is immutable. Re-registering during a program
        // reset cannot redirect an already published trampoline.
        if let Some(entry) = entries.iter_mut().find(|e| e.owner == owner) {
            if owner != 0 && entry.address != address {
                return false;
            }
            self.sequence = self
                .sequence
                .checked_add(1)
                .expect("callback registration sequence exhausted");
            entry.address = address;
            entry.sequence = self.sequence;
        } else {
            self.sequence = self
                .sequence
                .checked_add(1)
                .expect("callback registration sequence exhausted");
            entries.push(CallbackCodeEntry {
                owner,
                address,
                sequence: self.sequence,
            });
        }
        true
    }

    fn select(&self, key: &str) -> Option<&CallbackCodeEntry> {
        self.entries
            .get(key)?
            .iter()
            .filter(|e| e.owner == 0 || self.owners.get(&e.owner).is_some_and(|s| !s.retiring))
            .max_by_key(|e| e.sequence)
    }

    fn publish(&mut self, key: &str) -> Option<usize> {
        let entry = self.select(key)?;
        let address = entry.address;
        // Signature lookup is process-wide. The selected adapter can belong
        // to a different session from the callback's managed closure, whose
        // code dependencies are not encoded in this ABI. Lease every active
        // owner rather than incorrectly retaining only the selected adapter.
        for owner in self.owners.values_mut().filter(|s| !s.retiring) {
            owner.published = true;
        }
        Some(address)
    }

    fn prepare_retire(&mut self, owner: i64) -> bool {
        let Some(state) = self.owners.get_mut(&owner) else {
            return false;
        };
        if state.published {
            return false;
        }
        state.retiring = true;
        true
    }

    fn cancel_retire(&mut self, owner: i64) -> bool {
        let Some(state) = self.owners.get_mut(&owner) else {
            return false;
        };
        if !state.retiring || state.published {
            return false;
        }
        state.retiring = false;
        true
    }

    fn finish_retire(&mut self, owner: i64) -> bool {
        if !self
            .owners
            .get(&owner)
            .is_some_and(|s| s.retiring && !s.published)
        {
            return false;
        }
        self.entries.retain(|_, entries| {
            entries.retain(|e| e.owner != owner);
            !entries.is_empty()
        });
        self.owners.remove(&owner);
        true
    }
}

fn callback_entries() -> &'static std::sync::Mutex<CallbackCodeRegistry> {
    static REGISTRY: std::sync::OnceLock<std::sync::Mutex<CallbackCodeRegistry>> =
        std::sync::OnceLock::new();
    REGISTRY.get_or_init(Default::default)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_callback_owner_new() -> i64 {
    callback_entries()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .new_owner()
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_callback_owner_published(owner: i64) -> u8 {
    let registry = callback_entries().lock().unwrap_or_else(|e| e.into_inner());
    u8::from(registry.owners.get(&owner).is_none_or(|s| s.published))
}

// SAFETY: key is a readable module-owned UTF-8/ASCII constant of nonnegative
// length. The entry must remain mapped until finish_retire succeeds, or forever
// once published. Returning zero requires the caller to fail closed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_ffi_register_callback_owned(
    key: *const u8,
    length: i64,
    entry: usize,
    owner: i64,
) -> u8 {
    if length < 0 || key.is_null() || owner <= 0 {
        return 0;
    }
    let bytes = unsafe { std::slice::from_raw_parts(key, length as usize) };
    let key = String::from_utf8_lossy(bytes).into_owned();
    u8::from(
        callback_entries()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .register(key, entry, owner),
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_callback_owner_prepare_retire(owner: i64) -> u8 {
    u8::from(
        callback_entries()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .prepare_retire(owner),
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_callback_owner_cancel_retire(owner: i64) -> u8 {
    u8::from(
        callback_entries()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .cancel_retire(owner),
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_ffi_callback_owner_finish_retire(owner: i64) -> u8 {
    u8::from(
        callback_entries()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .finish_retire(owner),
    )
}

#[cfg(test)]
mod callback_code_owner_tests {
    use super::CallbackCodeRegistry;

    #[test]
    fn metadata_can_retire_but_published_adapter_cannot() {
        let mut registry = CallbackCodeRegistry::default();
        let owner = registry.new_owner();
        assert!(registry.register("i(i)".into(), 17, owner));
        assert!(registry.prepare_retire(owner));
        assert!(registry.publish("i(i)").is_none());
        assert!(!registry.register("i(i)".into(), 17, owner));
        assert!(registry.cancel_retire(owner));
        assert_eq!(registry.publish("i(i)"), Some(17));
        assert!(!registry.prepare_retire(owner));
        assert!(!registry.finish_retire(owner));
    }

    #[test]
    fn generations_cannot_redirect_old_adapters_and_cancel_preserves_order() {
        let mut registry = CallbackCodeRegistry::default();
        let old = registry.new_owner();
        let next = registry.new_owner();
        assert!(registry.register("i(i)".into(), 17, old));
        assert!(!registry.register("i(i)".into(), 18, old));
        assert!(registry.prepare_retire(old));
        assert!(registry.register("i(i)".into(), 19, next));
        assert!(registry.cancel_retire(old));
        assert_eq!(registry.select("i(i)").map(|e| e.address), Some(19));
        assert!(registry.prepare_retire(old));
        assert!(registry.finish_retire(old));
        assert_eq!(registry.publish("i(i)"), Some(19));
    }

    #[test]
    fn retirement_restores_an_unowned_adapter_without_reusing_owner_ids() {
        let mut registry = CallbackCodeRegistry::default();
        assert!(registry.register("i(i)".into(), 11, 0));
        let owner = registry.new_owner();
        assert!(registry.register("i(i)".into(), 17, owner));
        assert!(registry.prepare_retire(owner));
        assert_eq!(registry.publish("i(i)"), Some(11));
        assert!(registry.finish_retire(owner));
        let next = registry.new_owner();
        assert!(next > owner);
        assert!(!registry.register("i(i)".into(), 18, owner));
    }
    #[test]
    fn publication_leases_closure_producing_owners_as_well_as_adapter_owner() {
        let mut registry = CallbackCodeRegistry::default();
        let closure_owner = registry.new_owner();
        let adapter_owner = registry.new_owner();
        assert!(registry.register("i(i)".into(), 17, adapter_owner));
        assert_eq!(registry.publish("i(i)"), Some(17));
        assert!(!registry.prepare_retire(closure_owner));
        assert!(!registry.prepare_retire(adapter_owner));
        // A later callback-free owner can still retire independently.
        let later = registry.new_owner();
        assert!(registry.prepare_retire(later));
        assert!(registry.finish_retire(later));
    }

    #[test]
    fn publication_and_retirement_race_has_only_two_safe_outcomes() {
        for _ in 0..32 {
            let mut initial = CallbackCodeRegistry::default();
            let owner = initial.new_owner();
            assert!(initial.register("i(i)".into(), 17, owner));
            let registry = std::sync::Arc::new(std::sync::Mutex::new(initial));
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
            let producer_registry = registry.clone();
            let producer_barrier = barrier.clone();
            let producer = std::thread::spawn(move || {
                producer_barrier.wait();
                producer_registry.lock().unwrap().publish("i(i)")
            });
            barrier.wait();
            let sealed = registry.lock().unwrap().prepare_retire(owner);
            let published = producer.join().unwrap();
            assert_eq!(sealed, published.is_none());
            if sealed {
                assert!(registry.lock().unwrap().finish_retire(owner));
            }
        }
    }
}
