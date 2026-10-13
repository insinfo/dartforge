// Native runtime: runtime types (RTI, docs/NATIVO-PLANO.md
// §7.7), following the dart2js design (`sdk/lib/_internal/js_runtime/lib/rti.dart`):
//
// * a **universe** of canonical types per isolate: each type is an id
//   (`i64`) of a *hash-consed* table — the same type always has the
//   same id, so type equality is id equality;
// * **recipes**: the compiler describes a type by a short text (the
//   grammar is in `Reader`) that the runtime reads once (the getter of the
//   recipe's global keeps the id). A recipe can have **variables**:
//   `P<i>`, the i-th type parameter of the class that declares the current
//   code, and `M<i>`, the i-th type argument of the current generic
//   function. `dartforge_rti_eval` replaces the variables with the types of
//   the environment (the type of `this` seen as that class, and the tuple of
//   function arguments) — dart2js's `_Rti._eval`;
// * **supertype rules** per class (dart2js's `_Universe.findRule`):
//   for each class `C<X…>`, its supertypes written in `P<i>`;
//   registered by the generated code at startup;
// * the **type of each generic object** lives in the `metadata` of the block
//   header (`layout::Header`, `id + 1`; docs/NATIVO-ESPACO-UNIFICADO.md
//   §2.15), for every object of the space — lists and closures included; an
//   object without metadata has the raw type of its class (`dynamic` arguments).
//
// Subtyping follows the rules of the specification (Dart 3, no legacy types;
// "Subtypes", dart2js's `_isSubtype`), with a per-pair cache.

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum Type {
    Dynamic,
    Void,
    Never,
    Null,
    /// Class (RTI id) and arguments.
    Interface(i64, Vec<i64>),
    /// `T?` (normalized: `T` is neither nullable nor top).
    Nullable(i64),
    FutureOr(i64),
    /// `R Function<X…>(pos…, [opt…], {named…})`: `n_required` of the
    /// positionals are required; the named ones come sorted by name.
    Function {
        generics: usize,
        ret: i64,
        pos: Vec<i64>,
        n_required: usize,
        named: Vec<(String, i64, bool)>,
    },
    /// Bound variable of a generic function type (De Bruijn index).
    Enabled(usize),
    Record {
        pos: Vec<i64>,
        named: Vec<(String, i64)>,
    },
    /// Recipe variables: class parameter and function argument.
    ClassParam(usize),
    FunctionParam(usize),
    /// The type arguments of a generic call (the `M<i>` environment).
    Tuple(Vec<i64>),
}

/// The SDK classes the runtime needs to know (registered by the
/// generated code): `dartforge_rti_runtime_class`.
#[derive(Default)]
struct RuntimeClasses {
    int: i64,
    double: i64,
    bool_: i64,
    string: i64,
    list: i64,
    map: i64,
    set: i64,
    function: i64,
    record: i64,
    future: i64,
    object: i64,
    string_buffer: i64,
    regexp: i64,
    matchc: i64,
    type_: i64,
}

#[derive(Default)]
struct Universe {
    /// Local copy (without lock) of the types of the [`GlobalUniverse`] that
    /// this isolate has already seen, by id.
    types: Vec<Option<&'static Type>>,
    ix: HashMap<Type, i64>,
    /// Supertypes of each class: (supertype class, model in `P<i>`).
    rules: HashMap<i64, Vec<(i64, i64)>>,
    /// Name and number of type parameters of each class (RTI id).
    classes: HashMap<i64, (String, usize)>,
    /// RTI class of a heap class id that is not the same number (the
    /// runtime's error classes 1000–1012).
    from_heap: HashMap<i64, i64>,
    rt: RuntimeClasses,
    cache_sub: HashMap<(i64, i64), bool>,
    eval_cache: HashMap<(i64, i64, i64, i64), i64>,
    recipes: HashMap<Vec<u16>, i64>,
    /// `Interface(class, [])` already interned, per class: the type of every
    /// `int`, `double`, `String`… of a check, without building and hashing the
    /// `Type` each time (interned ids never change).
    no_args: HashMap<i64, i64>,
    /// The raw type of each class, with the arity it was computed with (a
    /// reload that changes the arity recomputes it).
    raws: HashMap<i64, (usize, i64)>,
    /// Direct-mapped memo (no collision resolution: the new entry takes the slot)
    /// of `v is t`, by the value key ([`value_key`]): `(key, t,
    /// answer)`. Avoids building the value's type and the subtype cache on every
    /// covariance check (`K`/`V` of a map's `[]=`, `E` of a list's
    /// `add`), which the VM does with an inline class test.
    memo_and: Vec<(i64, i64, bool)>,
    /// The same for `dartforge_rti_eval`: `[key of this, model,
    /// class, tuple, result]`.
    memo_eval: Vec<[i64; 5]>,
    /// The same for `dartforge_rti_set` of a runtime collection:
    /// `[concrete class, requested type, stored type]` — the `List<E>` of a
    /// literal seen as `_GrowableList<E>` went through `as_supertype`
    /// (and a new `Vec`) on every list created (N17).
    memo_set: Vec<[i64; 3]>,
}

/// The size of the direct-mapped memos of the [`Universe`] (power of 2).
const MEMO_N: usize = 1 << 10;

/// The slot of a key in the direct-mapped memos.
fn memo_slot(a: i64, b: i64, c: i64) -> usize {
    let h = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ (c as u64).wrapping_mul(0x1656_67B1_9E37_79F9);
    (h >> (64 - MEMO_N.trailing_zeros())) as usize
}

/// A key that determines the type of `h` in [`value_type`] without
/// building it (§2.15): the reified type stored in the object (the metadata, > 0), or,
/// without it, `-16 - cid` — for every value: `null` and `Smi` by cids 1 and 2,
/// the others by the header cid. A positional record without metadata has
/// no key (its type comes from the fields). `None` also for what is not a
/// valid value.
fn value_key(h: i64) -> Option<i64> {
    use crate::layout::{DESLOCAMENTO_DO_HANDLE, cid};
    if h == 0 {
        return Some(-16 - i64::from(cid::NULL));
    }
    if smi::is_smi(h) {
        return Some(-16 - i64::from(cid::SMI));
    }
    if !crate::layout::is_object(h) {
        return None;
    }
    // The metadata and the class in the header, without the heap (the handle is the block + 2;
    // also holds for the image statics).
    // SAFETY: the handle of a live object (the value the generated code
    // passed) points to its block header.
    #[allow(unsafe_code)]
    let c = unsafe { *((h - DESLOCAMENTO_DO_HANDLE) as *const crate::layout::Header) };
    if c.metadata != 0 {
        return Some(i64::from(c.metadata));
    }
    (c.class_id != cid::RECORD).then(|| -16 - i64::from(c.class_id))
}

thread_local! {
    static RTI: RefCell<Universe> = RefCell::new(Universe::new());
}

/// Type ids belong to the PROCESS, not to the isolate (docs/NATIVO-PROJETOS-REAIS.md,
/// C11): an id kept as a raw word — the type argument `T` that a
/// generic closure captured in its context, the type tuple of a call —
/// crosses `Isolate.spawn` copied as a number, and in the new isolate it had to
/// denote the same type. With one universe per isolate, the main isolate's id
/// pointed to another type (or outside the table: the
/// "index out of bounds" panic of `NotificationServiceWorker` in new_sali/backend).
/// The VM sends type arguments as objects. Here the canonical table is
/// a single one, append-only (each type leaked to live for the whole process); each
/// isolate keeps its local copy of those it has already seen, and only a new type goes
/// through the lock.
struct GlobalUniverse {
    types: Vec<&'static Type>,
    ix: HashMap<Type, i64>,
}

fn global_universe() -> &'static std::sync::RwLock<GlobalUniverse> {
    static G: std::sync::OnceLock<std::sync::RwLock<GlobalUniverse>> = std::sync::OnceLock::new();
    G.get_or_init(|| {
        let mut g = GlobalUniverse { types: Vec::new(), ix: HashMap::default() };
        // The fixed ids: T_DINAMICO, T_VAZIO, T_NUNCA, T_NULO.
        for t in [Type::Dynamic, Type::Void, Type::Never, Type::Null] {
            let id = g.types.len() as i64;
            g.types.push(Box::leak(Box::new(t.clone())));
            g.ix.insert(t, id);
        }
        std::sync::RwLock::new(g)
    })
}

/// The type `id` of the process table.
fn global_type(id: i64) -> Option<&'static Type> {
    let g = global_universe().read().unwrap_or_else(|e| e.into_inner());
    usize::try_from(id).ok().and_then(|i| g.types.get(i).copied())
}

/// Slots of the [`FAST_MEMO`] (power of 2).
const FAST_MEMO_N: usize = 256;

thread_local! {
    /// The direct-mapped memo of `v is t` in front of the [`Universe`]
    /// (docs/NATIVO-PLANO.md §13.2, item 7): `[key of the value, t, response]`,
    /// the same key and the same answers as `memo_and`, but in a thread
    /// static with no destructor nor `RefCell`. A hit avoids accessing the
    /// universe, the `borrow` and the `test_without_value` lookups — which weighs
    /// on the two covariance checks of each `map[k] = v`. Forgotten
    /// together with the universe memos ([`Universe::forget_memos`]).
    static FAST_MEMO: std::cell::UnsafeCell<[[i64; 3]; FAST_MEMO_N]> =
        const { std::cell::UnsafeCell::new([[0, -1, 0]; FAST_MEMO_N]) };
}

/// The slot of `(key, t)` in the [`FAST_MEMO`].
#[inline(always)]
fn fast_slot(key: i64, t: i64) -> usize {
    let h = (key as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (t as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    (h >> (64 - FAST_MEMO_N.trailing_zeros())) as usize
}

/// `v is t` stored in the [`FAST_MEMO`] for the value key. No key
/// is 0 and no type is -1: the empty slot never matches.
#[inline(always)]
#[allow(unsafe_code)]
fn fast_memo_read(key: i64, t: i64) -> Option<u8> {
    // SAFETY: the memo belongs to this thread and no reference to it outlives the
    // read.
    let e = FAST_MEMO.with(|m| unsafe { (*m.get())[fast_slot(key, t)] });
    (e[0] == key && e[1] == t).then_some(e[2] as u8)
}

#[allow(unsafe_code)]
fn fast_memo_store(key: i64, t: i64, r: bool) {
    // SAFETY: as in `fast_memo_read`.
    FAST_MEMO.with(|m| unsafe { (*m.get())[fast_slot(key, t)] = [key, t, i64::from(r)] });
}

#[allow(unsafe_code)]
fn fast_memo_forget() {
    // SAFETY: as in `fast_memo_read`.
    FAST_MEMO.with(|m| unsafe { *m.get() = [[0, -1, 0]; FAST_MEMO_N] });
}

impl Type {
    /// The type with each child type replaced by `f(child)`.
    fn with_children(&self, mut f: impl FnMut(i64) -> i64) -> Type {
        match self {
            Type::Interface(c, args) => Type::Interface(*c, args.iter().map(|&x| f(x)).collect()),
            Type::Nullable(x) => Type::Nullable(f(*x)),
            Type::FutureOr(x) => Type::FutureOr(f(*x)),
            Type::Function { generics, ret, pos, n_required, named } => Type::Function {
                generics: *generics,
                ret: f(*ret),
                pos: pos.iter().map(|&x| f(x)).collect(),
                n_required: *n_required,
                named: named.iter().map(|(n, x, r)| (n.clone(), f(*x), *r)).collect(),
            },
            Type::Record { pos, named } => Type::Record {
                pos: pos.iter().map(|&x| f(x)).collect(),
                named: named.iter().map(|(n, x)| (n.clone(), f(*x))).collect(),
            },
            Type::Tuple(args) => Type::Tuple(args.iter().map(|&x| f(x)).collect()),
            Type::Dynamic | Type::Void | Type::Never | Type::Null | Type::Enabled(_) | Type::ClassParam(_) | Type::FunctionParam(_) => self.clone(),
        }
    }
}

/// The types of a message to another isolate: each id of this isolate's
/// universe becomes an index of its own table, with children before
/// parents (type ids are per isolate; class ids, per program).
#[derive(Clone, Default)]
pub struct MessageTypes {
    types: Vec<Type>,
    ix: HashMap<i64, i64>,
}

impl MessageTypes {
    /// The index (in the message table) of this isolate's type `id`.
    pub fn export(&mut self, id: i64) -> i64 {
        if let Some(&i) = self.ix.get(&id) {
            return i;
        }
        let t = RTI.with(|u| u.borrow().tipo(id).clone());
        let local = t.with_children(|child| self.export(child));
        let i = self.types.len() as i64;
        self.types.push(local);
        self.ix.insert(id, i);
        i
    }

    /// The ids, in this isolate's universe, of all types in the table.
    pub fn import(&self) -> Vec<i64> {
        RTI.with(|u| {
            let mut u = u.borrow_mut();
            let mut ids: Vec<i64> = Vec::with_capacity(self.types.len());
            for t in &self.types {
                let t = t.with_children(|child| ids[child as usize]);
                ids.push(u.intern(t));
            }
            ids
        })
    }
}

const T_DYNAMIC: i64 = 0;
const T_VOID: i64 = 1;
const T_NEVER: i64 = 2;
const T_NULL: i64 = 3;

impl Universe {
    fn new() -> Self {
        let mut u = Universe::default();
        for t in [Type::Dynamic, Type::Void, Type::Never, Type::Null] {
            u.intern(t);
        }
        u
    }

    /// Forgets the direct-mapped memos (a new rule, class or runtime shape
    /// can change a stored answer).
    fn forget_memos(&mut self) {
        fast_memo_forget();
        self.memo_and.clear();
        self.memo_eval.clear();
        self.memo_set.clear();
    }

    /// `v is t` stored for the value key.
    fn memo_and_read(&self, key: i64, t: i64) -> Option<bool> {
        let (c, tt, r) = *self.memo_and.get(memo_slot(key, t, 0))?;
        (c == key && tt == t).then_some(r)
    }

    fn memo_and_store(&mut self, key: i64, t: i64, r: bool) {
        if self.memo_and.is_empty() {
            // An empty slot never matches: no key is 0 (metadata
            // is > 0, kinds < 0).
            self.memo_and = vec![(0, 0, false); MEMO_N];
        }
        let i = memo_slot(key, t, 0);
        self.memo_and[i] = (key, t, r);
    }

    /// Stored `dartforge_rti_eval`.
    fn memo_eval_read(&self, k: [i64; 4]) -> Option<i64> {
        let e = self.memo_eval.get(memo_slot(k[0], k[1], k[2] ^ k[3].rotate_left(32)))?;
        (e[..4] == k).then_some(e[4])
    }

    fn memo_eval_store(&mut self, k: [i64; 4], r: i64) {
        if self.memo_eval.is_empty() {
            self.memo_eval = vec![[0, -1, 0, 0, 0]; MEMO_N];
        }
        let i = memo_slot(k[0], k[1], k[2] ^ k[3].rotate_left(32));
        self.memo_eval[i] = [k[0], k[1], k[2], k[3], r];
    }

    fn intern(&mut self, t: Type) -> i64 {
        if let Some(&id) = self.ix.get(&t) {
            return id;
        }
        let (id, r) = {
            let mut g = global_universe().write().unwrap_or_else(|e| e.into_inner());
            match g.ix.get(&t) {
                Some(&id) => (id, g.types[id as usize]),
                None => {
                    let id = g.types.len() as i64;
                    let r: &'static Type = Box::leak(Box::new(t.clone()));
                    g.types.push(r);
                    g.ix.insert(t.clone(), id);
                    (id, r)
                }
            }
        };
        let i = id as usize;
        if self.types.len() <= i {
            self.types.resize(i + 1, None);
        }
        self.types[i] = Some(r);
        self.ix.insert(t, id);
        id
    }

    /// The type `id`, if it exists (the local copy, otherwise the process table).
    fn get_value(&self, id: i64) -> Option<&Type> {
        match usize::try_from(id).ok().and_then(|i| self.types.get(i).copied().flatten()) {
            Some(t) => Some(t),
            None => global_type(id),
        }
    }

    fn tipo(&self, id: i64) -> &Type {
        self.get_value(id).expect("invalid type")
    }

    fn is_top(&self, id: i64) -> bool {
        match self.tipo(id) {
            Type::Dynamic | Type::Void => true,
            Type::Nullable(x) => self.e_object(*x),
            Type::FutureOr(x) => self.is_top(*x),
            _ => false,
        }
    }

    /// `v is t` when the answer does not depend on the value's type: `t` is a
    /// top type, or `Object` and `v` is not null. The case of every `as E` with
    /// `E` = `Object?`/`dynamic` in SDK collections; spares building the value's
    /// type (the heap, the metadata, the universe) and the subtype cache.
    fn test_without_value(&self, v: i64, t: i64) -> Option<bool> {
        if self.is_top(t) {
            return Some(true);
        }
        if v != 0 && self.e_object(t) {
            return Some(true);
        }
        None
    }

    fn e_object(&self, id: i64) -> bool {
        matches!(self.tipo(id), Type::Interface(c, a) if *c == self.rt.object && a.is_empty())
    }

    /// Normalized `T?` (specification, "Type normalization").
    fn nullable(&mut self, t: i64) -> i64 {
        match self.tipo(t).clone() {
            Type::Dynamic | Type::Void | Type::Null | Type::Nullable(_) => t,
            Type::Never => T_NULL,
            Type::FutureOr(x) if self.is_nullable(x) => t,
            _ => self.intern(Type::Nullable(t)),
        }
    }

    fn is_nullable(&self, t: i64) -> bool {
        match self.tipo(t) {
            Type::Dynamic | Type::Void | Type::Null | Type::Nullable(_) => true,
            Type::FutureOr(x) => self.is_nullable(*x),
            _ => false,
        }
    }

    /// Normalized `FutureOr<T>`.
    fn future_or(&mut self, t: i64) -> i64 {
        if self.is_top(t) || self.e_object(t) {
            return t;
        }
        match self.tipo(t) {
            Type::Never => self.interface(self.rt.future, vec![T_NEVER]),
            Type::Null => {
                let f = self.interface(self.rt.future, vec![T_NULL]);
                self.nullable(f)
            }
            _ => self.intern(Type::FutureOr(t)),
        }
    }

    fn interface(&mut self, class: i64, args: Vec<i64>) -> i64 {
        if args.is_empty() {
            if let Some(&id) = self.no_args.get(&class) {
                return id;
            }
            let id = self.intern(Type::Interface(class, args));
            self.no_args.insert(class, id);
            return id;
        }
        self.intern(Type::Interface(class, args))
    }

    /// The RTI class of a heap class id.
    fn heap_class(&self, id: i64) -> i64 {
        self.from_heap.get(&id).copied().unwrap_or(id)
    }

    /// Raw type of a class: `dynamic` arguments.
    fn raw_value(&mut self, class: i64) -> i64 {
        let n = self.classes.get(&class).map_or(0, |c| c.1);
        if let Some(&(arity, id)) = self.raws.get(&class)
            && arity == n
        {
            return id;
        }
        let id = self.interface(class, vec![T_DYNAMIC; n]);
        self.raws.insert(class, (n, id));
        id
    }

    /// Replaces the recipe variables with the environment types.
    fn substitute(&mut self, t: i64, class: &[i64], function: &[i64]) -> i64 {
        match self.tipo(t).clone() {
            Type::ClassParam(i) => class.get(i).copied().unwrap_or(T_DYNAMIC),
            Type::FunctionParam(i) => function.get(i).copied().unwrap_or(T_DYNAMIC),
            Type::Interface(c, args) => {
                if args.is_empty() {
                    return t;
                }
                let a = args.iter().map(|&x| self.substitute(x, class, function)).collect();
                self.interface(c, a)
            }
            Type::Nullable(x) => {
                let y = self.substitute(x, class, function);
                self.nullable(y)
            }
            Type::FutureOr(x) => {
                let y = self.substitute(x, class, function);
                self.future_or(y)
            }
            Type::Function { generics, ret, pos, n_required, named } => {
                let ret = self.substitute(ret, class, function);
                let pos = pos.iter().map(|&x| self.substitute(x, class, function)).collect();
                let named = named
                    .iter()
                    .map(|(n, x, r)| (n.clone(), self.substitute(*x, class, function), *r))
                    .collect();
                self.intern(Type::Function { generics, ret, pos, n_required, named })
            }
            Type::Record { pos, named } => {
                let pos = pos.iter().map(|&x| self.substitute(x, class, function)).collect();
                let named = named
                    .iter()
                    .map(|(n, x)| (n.clone(), self.substitute(*x, class, function)))
                    .collect();
                self.intern(Type::Record { pos, named })
            }
            Type::Tuple(args) => {
                let a = args.iter().map(|&x| self.substitute(x, class, function)).collect();
                self.intern(Type::Tuple(a))
            }
            _ => t,
        }
    }

    /// The arguments of the supertype `target` of `class<args>` (the class itself
    /// or one of its rules).
    fn as_supertype(&mut self, class: i64, args: &[i64], target: i64) -> Option<Vec<i64>> {
        if class == target {
            return Some(args.to_vec());
        }
        let model = self.rules.get(&class)?.iter().find(|(c, _)| *c == target)?.1;
        let s = self.substitute(model, args, &[]);
        match self.tipo(s) {
            Type::Interface(_, a) => Some(a.clone()),
            _ => None,
        }
    }

    fn sub(&mut self, s: i64, t: i64) -> bool {
        if s == t {
            return true;
        }
        if let Some(&r) = self.cache_sub.get(&(s, t)) {
            return r;
        }
        let r = self.sub_without_cache(s, t);
        self.cache_sub.insert((s, t), r);
        r
    }

    fn sub_without_cache(&mut self, s: i64, t: i64) -> bool {
        if self.is_top(t) {
            return true;
        }
        let ts = self.tipo(s).clone();
        let tt = self.tipo(t).clone();
        if matches!(ts, Type::Never) {
            return true;
        }
        if matches!(ts, Type::Dynamic | Type::Void) {
            return false;
        }
        if self.e_object(t) {
            return match ts {
                Type::Null | Type::Nullable(_) => false,
                Type::FutureOr(x) => self.sub(x, t),
                _ => true,
            };
        }
        if matches!(ts, Type::Null) {
            return match tt {
                Type::Null | Type::Nullable(_) => true,
                Type::FutureOr(y) => self.sub(T_NULL, y),
                _ => false,
            };
        }
        if let Type::Nullable(x) = ts {
            return self.sub(T_NULL, t) && self.sub(x, t);
        }
        if let Type::FutureOr(x) = ts {
            let fx = self.interface(self.rt.future, vec![x]);
            return self.sub(fx, t) && self.sub(x, t);
        }
        if let Type::Nullable(y) = tt {
            return self.sub(s, y);
        }
        if let Type::FutureOr(y) = tt {
            if self.sub(s, y) {
                return true;
            }
            let fy = self.interface(self.rt.future, vec![y]);
            return self.sub(s, fy);
        }
        match (ts, tt) {
            (Type::Interface(c1, a1), Type::Interface(c2, a2)) => {
                let Some(seen) = self.as_supertype(c1, &a1, c2) else {
                    return false;
                };
                seen.iter().zip(a2.iter()).all(|(&x, &y)| self.sub(x, y))
            }
            (Type::Function { .. }, Type::Interface(c2, _)) => c2 == self.rt.function,
            (Type::Record { .. }, Type::Interface(c2, _)) => c2 == self.rt.record,
            (
                Type::Function { generics: g1, ret: r1, pos: p1, n_required: o1, named: n1 },
                Type::Function { generics: g2, ret: r2, pos: p2, n_required: o2, named: n2 },
            ) => {
                if g1 != g2 || o1 > o2 || p1.len() < p2.len() {
                    return false;
                }
                if !self.sub(r1, r2) {
                    return false;
                }
                for (i, &q) in p2.iter().enumerate() {
                    if !self.sub(q, p1[i]) {
                        return false;
                    }
                }
                for (name, q, req) in &n2 {
                    match n1.iter().find(|(m, _, _)| m == name) {
                        Some((_, p, _)) => {
                            if !self.sub(*q, *p) {
                                return false;
                            }
                        }
                        None => return false,
                    }
                    let _ = req;
                }
                // Every required named parameter of S must be passed by T.
                n1.iter()
                    .filter(|(_, _, r)| *r)
                    .all(|(m, _, _)| n2.iter().any(|(n, _, r)| n == m && *r))
            }
            (Type::Record { pos: p1, named: n1 }, Type::Record { pos: p2, named: n2 }) => {
                p1.len() == p2.len()
                    && n1.len() == n2.len()
                    && p1.iter().zip(&p2).all(|(&x, &y)| self.sub(x, y))
                    && n1.iter().zip(&n2).all(|((a, x), (b, y))| a == b && self.sub(*x, *y))
            }
            (Type::Enabled(i), Type::Enabled(j)) => i == j,
            _ => false,
        }
    }

    /// The text of a type, like the VM's `Type.toString()`.
    fn text(&self, t: i64) -> String {
        match self.tipo(t) {
            Type::Dynamic => "dynamic".to_string(),
            Type::Void => "void".to_string(),
            Type::Never => "Never".to_string(),
            Type::Null => "Null".to_string(),
            Type::Interface(c, args) => {
                let name = self.classes.get(c).map_or_else(|| format!("Class{c}"), |x| x.0.clone());
                if args.is_empty() {
                    name
                } else {
                    let a: Vec<String> = args.iter().map(|&x| self.text(x)).collect();
                    format!("{name}<{}>", a.join(", "))
                }
            }
            Type::Nullable(x) => format!("{}?", self.text(*x)),
            Type::FutureOr(x) => format!("FutureOr<{}>", self.text(*x)),
            Type::Function { ret, pos, n_required, named, generics } => {
                let mut part_list: Vec<String> = pos[..*n_required].iter().map(|&x| self.text(x)).collect();
                if pos.len() > *n_required {
                    let o: Vec<String> = pos[*n_required..].iter().map(|&x| self.text(x)).collect();
                    part_list.push(format!("[{}]", o.join(", ")));
                }
                if !named.is_empty() {
                    let n: Vec<String> = named
                        .iter()
                        .map(|(m, x, r)| format!("{}{} {m}", if *r { "required " } else { "" }, self.text(*x)))
                        .collect();
                    part_list.push(format!("{{{}}}", n.join(", ")));
                }
                let g = if *generics > 0 {
                    // Like the VM: `<Y0, Y1>(Y0) => Y1`.
                    let v: Vec<String> = (0..*generics).map(|i| format!("Y{i}")).collect();
                    format!("<{}>", v.join(", "))
                } else {
                    String::new()
                };
                format!("{g}({}) => {}", part_list.join(", "), self.text(*ret))
            }
            Type::Record { pos, named } => {
                let mut part_list: Vec<String> = pos.iter().map(|&x| self.text(x)).collect();
                if !named.is_empty() {
                    let n: Vec<String> = named.iter().map(|(m, x)| format!("{} {m}", self.text(*x))).collect();
                    part_list.push(format!("{{{}}}", n.join(", ")));
                }
                format!("({})", part_list.join(", "))
            }
            Type::Enabled(i) => format!("Y{i}"),
            Type::ClassParam(i) => format!("P{i}"),
            Type::FunctionParam(i) => format!("M{i}"),
            Type::Tuple(a) => {
                let v: Vec<String> = a.iter().map(|&x| self.text(x)).collect();
                format!("<{}>", v.join(", "))
            }
        }
    }
}

/// Recipe reader (UTF-16 units of the text). The grammar:
///
/// ```text
/// T := 'D' | 'V' | 'N' | 'U'              dynamic, void, Never, Null
///    | 'C' n [ '<' T (',' T)* '>' ]       class (RTI id) with arguments
///    | 'O' '<' T '>'                      FutureOr
///    | 'P' n | 'M' n | 'B' n              class parameter, function parameter, bound variable
///    | 'F' '<' g ';' T ';' n ';' T* ';' (name ':' T ['!'])* '>'
///                                         function: generics, ret, required count,
///                                         positional, named ('!' = required)
///    | 'R' '<' T* ';' (name ':' T)* '>'   record
///    | 'L' '<' T* '>'                     tuple of type arguments
///    | T '?'                              nullable
/// ```
/// Lists separated by `,`.
struct Reader<'x> {
    u: &'x [u16],
    i: usize,
}

impl Reader<'_> {
    fn peek(&self) -> Option<char> {
        self.u.get(self.i).map(|&c| char::from_u32(u32::from(c)).unwrap_or('\0'))
    }
    fn take(&mut self) -> char {
        let c = self.peek().expect("truncated type recipe");
        self.i += 1;
        c
    }
    fn wait_for(&mut self, c: char) {
        let d = self.take();
        assert!(d == c, "type recipe: expected {c:?}, got {d:?}");
    }
    fn number(&mut self) -> i64 {
        let mut neg = false;
        if self.peek() == Some('-') {
            self.i += 1;
            neg = true;
        }
        let mut n: i64 = 0;
        while let Some(c) = self.peek().filter(char::is_ascii_digit) {
            n = n * 10 + i64::from(c as u8 - b'0');
            self.i += 1;
        }
        if neg { -n } else { n }
    }
    fn name(&mut self) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek().filter(|c| c.is_alphanumeric() || *c == '_' || *c == '$') {
            s.push(c);
            self.i += 1;
        }
        s
    }
    fn list(&mut self, u: &mut Universe, end: char) -> Vec<i64> {
        let mut v = Vec::new();
        if self.peek() == Some(end) {
            return v;
        }
        loop {
            v.push(self.tipo(u));
            if self.peek() == Some(',') {
                self.i += 1;
            } else {
                return v;
            }
        }
    }
    fn tipo(&mut self, u: &mut Universe) -> i64 {
        let mut t = match self.take() {
            'D' => T_DYNAMIC,
            'V' => T_VOID,
            'N' => T_NEVER,
            'U' => T_NULL,
            'C' => {
                let c = self.number();
                let mut args = Vec::new();
                if self.peek() == Some('<') {
                    self.i += 1;
                    args = self.list(u, '>');
                    self.wait_for('>');
                }
                u.interface(c, args)
            }
            'O' => {
                self.wait_for('<');
                let x = self.tipo(u);
                self.wait_for('>');
                u.future_or(x)
            }
            'P' => {
                let n = self.number() as usize;
                u.intern(Type::ClassParam(n))
            }
            'M' => {
                let n = self.number() as usize;
                u.intern(Type::FunctionParam(n))
            }
            'B' => {
                let n = self.number() as usize;
                u.intern(Type::Enabled(n))
            }
            'F' => {
                self.wait_for('<');
                let generics = self.number() as usize;
                self.wait_for(';');
                let ret = self.tipo(u);
                self.wait_for(';');
                let n_required = self.number() as usize;
                self.wait_for(';');
                let pos = self.list(u, ';');
                self.wait_for(';');
                let mut named = Vec::new();
                while self.peek() != Some('>') {
                    let n = self.name();
                    self.wait_for(':');
                    let x = self.tipo(u);
                    let req = self.peek() == Some('!');
                    if req {
                        self.i += 1;
                    }
                    named.push((n, x, req));
                    if self.peek() == Some(',') {
                        self.i += 1;
                    }
                }
                self.wait_for('>');
                named.sort_by(|a, b| a.0.cmp(&b.0));
                u.intern(Type::Function { generics, ret, pos, n_required, named })
            }
            'R' => {
                self.wait_for('<');
                let pos = self.list(u, ';');
                self.wait_for(';');
                let mut named = Vec::new();
                while self.peek() != Some('>') {
                    let n = self.name();
                    self.wait_for(':');
                    let x = self.tipo(u);
                    named.push((n, x));
                    if self.peek() == Some(',') {
                        self.i += 1;
                    }
                }
                self.wait_for('>');
                named.sort_by(|a, b| a.0.cmp(&b.0));
                u.intern(Type::Record { pos, named })
            }
            'L' => {
                self.wait_for('<');
                let a = self.list(u, '>');
                self.wait_for('>');
                u.intern(Type::Tuple(a))
            }
            c => panic!("type recipe: unexpected character {c:?}"),
        };
        while self.peek() == Some('?') {
            self.i += 1;
            t = u.nullable(t);
        }
        t
    }
}

// `smi` for all fragments.
use crate::heap::smi;

/// The type of an unboxed value or one in a `Ref` position.
#[allow(dead_code)]
fn value_type(u: &mut Universe, v: crate::heap::Value) -> i64 {
    use crate::heap::Value;
    match v {
        Value::Int(_) => u.interface(u.rt.int, Vec::new()),
        Value::Double(_) => u.interface(u.rt.double, Vec::new()),
        Value::Bool(_) => u.interface(u.rt.bool_, Vec::new()),
        Value::Ref(r) => ref_type(u, r),
    }
}

/// The type of a `Ref` (null, `Smi` or object), dispatched by cid (§2.15).
fn ref_type(u: &mut Universe, h: i64) -> i64 {
    use crate::layout::cid;
    if h == 0 {
        return T_NULL;
    }
    if smi::is_smi(h) {
        return u.interface(u.rt.int, Vec::new());
    }
    enum Shape {
        Ready(i64),
        Raw(i64),
        Record(Vec<i64>),
    }
    let shape = HEAP.with(|heap| {
        let heap = heap.borrow();
        let c = *heap.header(h);
        if c.metadata != 0 {
            return Shape::Ready(i64::from(c.metadata) - 1);
        }
        match c.class_id {
            cid::MINT => Shape::Raw(u.rt.int),
            cid::DOUBLE => Shape::Raw(u.rt.double),
            cid::BOOL => Shape::Raw(u.rt.bool_),
            cid::ONE_BYTE_STRING | cid::TWO_BYTE_STRING => Shape::Raw(u.rt.string),
            cid::CLOSURE => Shape::Raw(u.rt.function),
            cid::RECORD => Shape::Record(heap.record(h).map(<[i64]>::to_vec).unwrap_or_default()),
            // The runtime's internal classes (context, cell, accumulator,
            // `RegExp` program) never reach the program as values.
            cid::CONTEXTO | cid::CELULA | cid::STRING_ACCUMULATOR | cid::REGEXP_PROGRAM => Shape::Raw(u.rt.object),
            // List without a stored type (literal with dynamic `E`, runtime
            // list), typed list, object: the concrete class
            // (`_GrowableList`, `_List`, `_ImmutableList`…), as in the VM —
            // raw `List` is not a subtype of `ListBase`, and the SDK's `addAll`
            // converts to it.
            c => Shape::Raw(u.heap_class(i64::from(c))),
        }
    });
    match shape {
        Shape::Ready(t) => t,
        Shape::Raw(c) => u.raw_value(c),
        Shape::Record(fields) => {
            let pos = fields.into_iter().map(|c| ref_type(u, c)).collect();
            u.intern(Type::Record { pos, named: Vec::new() })
        }
    }
}

/// The `_List` and `_GrowableList` natives receive the tuple of the constructor's
/// type arguments as the last parameter. Converts `L<E>` into the type of the
/// concrete object (`_List<E>` or `_GrowableList<E>`) that will be stored in the
/// metadata of the heap slot. The concrete class preserves `P0` in the SDK
/// methods declared in those classes, such as `_GrowableList.toList`.
pub(crate) fn tuple_list_type(tuple: i64, concrete_class: Option<i64>) -> Option<i64> {
    RTI.with(|u| {
        let mut u = u.borrow_mut();
        let class = concrete_class.unwrap_or(u.rt.list);
        if class == 0 {
            return None;
        }
        let args = match u.tipo(tuple) {
            Type::Tuple(args) => args.clone(),
            _ => vec![T_DYNAMIC],
        };
        Some(u.intern(Type::Interface(class, args)))
    })
}

/// Does the reified type `meta` of a list (`Interface(List, [E])`) accept an
/// `int` (`code` 1), `double` (2) or `bool` (3) value? Only reads the universe
/// (without interning): `E` is `dynamic`, `void`, `Object`, the class itself, or
/// a nullable or `FutureOr` of them. Any other `E` (`num`, `Comparable`…)
/// answers false, and the caller goes through the SDK's `[]=`, which checks.
pub(crate) fn list_accepts_scalar(meta: i64, code: i64) -> bool {
    RTI.with(|u| {
        let u = u.borrow();
        let target = match code {
            1 => u.rt.int,
            2 => u.rt.double,
            3 => u.rt.bool_,
            _ => return false,
        };
        let Some(Type::Interface(_, args)) = u.get_value(meta) else { return false };
        let mut e = args.first().copied().unwrap_or(T_DYNAMIC);
        loop {
            match u.get_value(e) {
                Some(Type::Dynamic | Type::Void) => return true,
                Some(Type::Nullable(x) | Type::FutureOr(x)) => e = *x,
                Some(Type::Interface(c, a)) => return a.is_empty() && (*c == target || *c == u.rt.object),
                _ => return false,
            }
        }
    })
}

/// The element shape (N14) of a list with reified type `meta`
/// (`Interface(List, [E])`): compact when `E` is exactly non-nullable `int`,
/// `double` or `bool` — then only that scalar enters through the SDK's
/// `[]=`/`add` (covariance checks the value against the reified
/// `E`), and the 8 bytes of the bits are enough; otherwise the general one. Only reads the
/// universe.
pub(crate) fn type_list_shape(meta: i64) -> crate::lists::Element {
    use crate::lists::Element;
    RTI.with(|u| {
        let u = u.borrow();
        let Some(Type::Interface(_, args)) = u.get_value(meta) else { return Element::General };
        let Some(&e) = args.first() else { return Element::General };
        match u.get_value(e) {
            Some(Type::Interface(c, a)) if a.is_empty() && *c != 0 => {
                if *c == u.rt.int {
                    Element::Int
                } else if *c == u.rt.double {
                    Element::Double
                } else if *c == u.rt.bool_ {
                    Element::Bool
                } else {
                    Element::General
                }
            }
            _ => Element::General,
        }
    })
}

/// Puts the list `h` in the shape of its reified type (N14,
/// [`type_list_shape`]): called whenever the metadata of a list
/// is stored. The switch is of the storage (`Heap::list_adjust_shape`,
/// `lists.rs`, §2.15), which converts the existing elements in place,
/// without allocating; a list that does not fit the compact shape yet stays general.
/// None of this is observable.
pub(crate) fn adjust_list_shape(heap: &mut Heap, h: i64) {
    if !crate::layout::is_object(h) || !heap.e_lista(h) {
        return;
    }
    let meta = heap.metadata(h);
    let shape = if meta == 0 { crate::lists::Element::General } else { type_list_shape(meta - 1) };
    if heap.list_shape(h) != shape {
        // The switch can allocate (the boxes of the general shape, §2.16): the list
        // stays rooted meanwhile (the caller has not always rooted it).
        let frame = heap.push_frame_with_slots(1);
        heap.set_root(frame, 0, h);
        heap.list_adjust_shape(h, shape);
        heap.pop_frame(frame);
    }
}

/// Stores the reified type `meta` (the metadata, `type + 1`) of the list `h` and
/// puts it in its shape ([`adjust_list_shape`]).
pub(crate) fn set_list_type(h: i64, meta: i64) {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        heap.set_metadata(h, meta);
        adjust_list_shape(&mut heap, h);
    });
}

/// `List<String>` in the concrete class of a list the runtime creates (the
/// VM's `Dart_NewListOfTypeFilled`).
pub(crate) fn string_list_type(concrete_class: Option<i64>) -> Option<i64> {
    RTI.with(|u| {
        let mut u = u.borrow_mut();
        let class = concrete_class.unwrap_or(u.rt.list);
        if class == 0 || u.rt.string == 0 {
            return None;
        }
        let string = u.rt.string;
        let text = u.interface(string, Vec::new());
        Some(u.intern(Type::Interface(class, vec![text])))
    })
}

/// `_List._sliceInternal` creates another concrete list class, but keeps
/// the receiver's `E` argument. The native receives no separate type tuple.
pub(crate) fn copied_list_type(origin: i64, concrete_class: Option<i64>) -> Option<i64> {
    RTI.with(|u| {
        let mut u = u.borrow_mut();
        let class = concrete_class.unwrap_or(u.rt.list);
        if class == 0 {
            return None;
        }
        let origin = ref_type(&mut u, origin);
        let args = match u.tipo(origin) {
            Type::Interface(_, args) => args.clone(),
            _ => vec![T_DYNAMIC],
        };
        Some(u.intern(Type::Interface(class, args)))
    })
}

// --- Generated code ABI --------------------------------------------------

/// The program's RTI registrations in a table (`lower/rti.rs`): one line
/// per registration — `C<id> <parameters> <name>` (the class name),
/// `R<id> <recipe>` (a supertype rule) and `F<shape> <id>` (the class of
/// a runtime shape) —, read in order. The large table comes compressed
/// (`tabela_rti`).
///
/// # Safety
/// `data` points to the words of the module's constant vector, with at least
/// `len` readable bytes (packed in little-endian): the UTF-8 text or its
/// compressed form. The vector lives for the whole process.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_rti_init_table(data: *const i64, len: i64) {
    // SAFETY: guaranteed by the caller (a module constant).
    let bytes: &'static [u8] = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), usize::try_from(len).unwrap_or(0)) };
    let bytes = if bytes.starts_with(crate::tabela_rti::HEADER) { crate::tabela_rti::decompressed(bytes) } else { bytes };
    let text = std::str::from_utf8(bytes).expect("RTI table in UTF-8");
    for line in text.lines() {
        let (tipo, rest) = line.split_at(1);
        let (a, b) = rest.split_once(' ').expect("RTI registration");
        let id: i64 = a.parse().expect("RTI id");
        match tipo {
            "C" => {
                let (n, name) = b.split_once(' ').unwrap_or((b, ""));
                let n: usize = n.parse().unwrap_or(0);
                RTI.with(|u| {
                    let mut u = u.borrow_mut();
                    u.classes.insert(id, (name.to_string(), n));
                    u.forget_memos();
                });
            }
            "R" => {
                let model = units_recipe(b.encode_utf16().collect());
                dartforge_rti_rule(id, model);
            }
            "F" => dartforge_rti_runtime_class(id, b.parse().expect("RTI id")),
            // The library and the declared name of the class (`dart:mirrors`).
            "B" => {
                let (uri, name) = b.split_once(' ').unwrap_or((b, ""));
                register_class_library(id, uri, name);
            }
            _ => panic!("unknown RTI registration: {line}"),
        }
    }
}

/// Registers a supertype rule: `class` has the supertype `model` (an
/// already read recipe, in `P<i>` of the parameters of `class`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_rule(class: i64, model: i64) {
    RTI.with(|u| {
        let mut u = u.borrow_mut();
        let sup = match u.tipo(model) {
            Type::Interface(c, _) => *c,
            _ => return,
        };
        // Without duplicating: publishing a reload redoes the rules.
        let rules = u.rules.entry(class).or_default();
        if !rules.contains(&(sup, model)) {
            rules.push((sup, model));
            // A new rule (the new generation of a reload) can change an
            // already stored answer.
            u.cache_sub.clear();
            u.forget_memos();
        }
    });
}

/// The RTI class of one of the runtime shapes (0 int, 1 double, 2 bool,
/// 3 String, 4 List, 5 Map, 6 Set, 7 Function, 8 Record, 9 Future,
/// 10 Object, 11 StringBuffer, 12 RegExp, 13 Match, 14 Type) or, with
/// `shape` ≥ 1000, the RTI class of the heap class id `shape`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_runtime_class(shape: i64, class: i64) {
    RTI.with(|u| {
        let mut u = u.borrow_mut();
        u.forget_memos();
        let rt = &mut u.rt;
        match shape {
            0 => rt.int = class,
            1 => rt.double = class,
            2 => rt.bool_ = class,
            3 => rt.string = class,
            4 => rt.list = class,
            5 => rt.map = class,
            6 => rt.set = class,
            7 => rt.function = class,
            8 => rt.record = class,
            9 => rt.future = class,
            10 => rt.object = class,
            11 => rt.string_buffer = class,
            12 => rt.regexp = class,
            13 => rt.matchc = class,
            14 => rt.type_ = class,
            f => {
                u.from_heap.insert(f, class);
            }
        }
    });
}

/// Reads a recipe (a heap `String`; the getter of its global keeps the
/// result).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_recipe(text: i64) -> i64 {
    let unidades: Vec<u16> = HEAP.with(|h| h.borrow().text(text).map(|t| t.unidades().collect()).unwrap_or_default());
    units_recipe(unidades)
}

/// The recipe (in UTF-16) read and interned.
fn units_recipe(unidades: Vec<u16>) -> i64 {
    RTI.with(|u| {
        let mut u = u.borrow_mut();
        if let Some(&t) = u.recipes.get(&unidades) {
            return t;
        }
        let mut l = Reader { u: &unidades, i: 0 };
        let t = l.tipo(&mut u);
        assert!(l.i == unidades.len(), "type recipe with leftover");
        u.recipes.insert(unidades, t);
        t
    })
}

/// The recipe `model` in the environment: `P<i>` is the i-th argument of the type of
/// `this` seen as the class `class` (0 = no `this`), `M<i>` the i-th
/// of the tuple `tuple` (0 = none: `dynamic`).
///
/// The receiver is borrowed, including the children of records without metadata.
/// Only reads the heap; interning and memos use native structures, with no collection,
/// Dart allocation, field writes, handle publication or pending exit.
/// The IDs belong to the RTI universe, not managed references.
///
/// # Panics
/// Inconsistent IDs or a receiver without valid lifetime: internal error, no Dart exception.
///
/// ```
/// use dartforge_runtime::abi::dartforge_rti_eval;
/// assert_eq!(dartforge_rti_eval(0, 0, 0, 0), 0); // dynamic stays dynamic.
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_eval(model: i64, this: i64, class: i64, tuple: i64) -> i64 {
    RTI.with(|u| {
        let mut u = u.borrow_mut();
        // The type of `this` only enters through its key: the direct-mapped memo
        // answers without building it.
        let this_key = if this == 0 { Some(0) } else { value_key(this) };
        let memo = this_key.map(|c| [c, model, class, tuple]);
        if let Some(r) = memo.and_then(|k| u.memo_eval_read(k)) {
            return r;
        }
        let this_type = if this == 0 { 0 } else { ref_type(&mut u, this) };
        let key = (model, this_type, class, tuple);
        if let Some(&r) = u.eval_cache.get(&key) {
            if let Some(k) = memo {
                u.memo_eval_store(k, r);
            }
            return r;
        }
        let class_args: Vec<i64> = match u.tipo(this_type).clone() {
            Type::Interface(c, a) if this != 0 => u.as_supertype(c, &a, class).unwrap_or_default(),
            _ => Vec::new(),
        };
        let function_args: Vec<i64> = if tuple > 0 {
            match u.tipo(tuple) {
                Type::Tuple(a) => a.clone(),
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        };
        let r = u.substitute(model, &class_args, &function_args);
        u.eval_cache.insert(key, r);
        if let Some(k) = memo {
            u.memo_eval_store(k, r);
        }
        r
    })
}

/// [`dartforge_rti_eval`] (without tuple) of a use site with an inline
/// cache (`llvm/mod.rs`, N17): `cache` are the two words of the site in the
/// isolate's globals area — the key of `this` ([`value_key`] of
/// an object of the space: the metadata, or `-16 - class`) and the result. The
/// generated code compares the key read from the header of `this` with the
/// first word and only calls here on a miss; here the answer is stored
/// when `this` is an object of the space (other values do not have the key in the
/// header and always come back to the runtime).
///
/// # Safety
/// `cache` points to two writable words of the isolate area.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn dartforge_rti_eval_cache(cache: *mut i64, model: i64, this: i64, class: i64) -> i64 {
    let r = dartforge_rti_eval(model, this, class, 0);
    if crate::heap::is_object(this)
        && let Some(key) = value_key(this)
        && key != 0
    {
        // SAFETY: the contract above; only the isolate's thread writes the area.
        unsafe {
            *cache = key;
            *cache.add(1) = r;
        }
    }
    r
}

/// Stores the type of a generic object (in the header `metadata`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_set(obj: i64, tipo: i64) {
    if !crate::layout::is_object(obj) {
        return;
    }
    // The lowering of literals reifies `List<E>`, `Map<K,V>` or `Set<E>`, but
    // the objects belong to the SDK's concrete classes. The methods of those
    // classes (and of their mixins) evaluate `P<i>` from the receiver.
    // The class and the metadata in the block header (`layout::Header`), without
    // consulting the heap — as the generated code reads them.
    let header = (obj - crate::layout::DESLOCAMENTO_DO_HANDLE) as *mut crate::layout::Header;
    // SAFETY: the handle of a live object points to its header.
    #[allow(unsafe_code)]
    let (estado, cid) = unsafe { ((*header).estado, (*header).class_id) };
    // An image static (`PERMANENTE`) is read-only and has no type to
    // store (string literals and the `bool` boxes).
    if estado == crate::layout::estado::PERMANENTE {
        return;
    }
    let concrete_class = Some(i64::from(cid));
    let tipo = if let Some(class) = concrete_class {
        RTI.with(|u| {
            let mut u = u.borrow_mut();
            // Only the collection interface of another class changes; the common case
            // (the object's own class: `Future<T>`, `_Map<K, V>`…) leaves
            // without copying the type. Only that case enters the memo, read before.
            let slot = memo_slot(class, tipo, 1);
            if let Some(&[mc, mt, r]) = u.memo_set.get(slot)
                && mc == class
                && mt == tipo
            {
                return r;
            }
            let (c, args) = match u.tipo(tipo) {
                Type::Interface(c, args) if *c != class && [u.rt.list, u.rt.map, u.rt.set].contains(c) => {
                    (*c, args.clone())
                }
                _ => return tipo,
            };
            let r = if u.as_supertype(class, &args, c).as_deref() == Some(args.as_slice()) {
                u.intern(Type::Interface(class, args))
            } else {
                tipo
            };
            if u.memo_set.is_empty() {
                // An empty slot never matches: no requested type is -1.
                u.memo_set = vec![[0, -1, 0]; MEMO_N];
            }
            u.memo_set[slot] = [class, tipo, r];
            r
        })
    } else {
        tipo
    };
    // The metadata (`id + 1`) is not a reference: no barrier.
    // SAFETY: as above; the header belongs to the object and only this thread writes it.
    #[allow(unsafe_code)]
    unsafe {
        (*header).metadata = u32::try_from(tipo + 1).expect("metadata beyond 32 bits");
    }
    // An `<int>[…]` literal (N14): the compact list.
    if crate::layout::cid::e_lista(cid) {
        HEAP.with(|h| adjust_list_shape(&mut h.borrow_mut(), obj));
    }
}

/// Stores the structural type of a record with named fields. The object's
/// fields were already filled in canonical order (positionals, then sorted
/// names); the types come from the actual values, as `Record.runtimeType` requires.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_named_record(obj: i64, npos: i64, names: i64) {
    let npos = usize::try_from(npos).expect("invalid number of positional fields");
    let names = HEAP.with(|h| h.borrow().text(names).map(|t| t.para_string()).unwrap_or_default());
    let fields = HEAP.with(|h| {
        let h = h.borrow();
        h.object(obj).expect("named record expected").to_vec()
    });
    let names: Vec<&str> = names.split(',').collect();
    assert_eq!(fields.len(), npos + names.len(), "shape of the named record");
    let tipo = RTI.with(|u| {
        let mut u = u.borrow_mut();
        let mut fields: Vec<i64> = fields.into_iter().map(|(bits, is_ref)| {
            assert!(is_ref, "record field without reference");
            ref_type(&mut u, bits)
        }).collect();
        let named = names.into_iter().zip(fields.drain(npos..)).map(|(n, t)| (n.to_string(), t)).collect();
        u.intern(Type::Record { pos: fields, named })
    });
    dartforge_rti_set(obj, tipo);
}

/// Text with type arguments for the default `toString` of a value
/// (`Instance of 'Caixa<int>'`, like the VM), or `None` when the value's type
/// has no arguments — then the caller uses the registered class name.
pub(crate) fn text_with_arguments(v: i64) -> Option<String> {
    RTI.with(|u| {
        let mut u = u.borrow_mut();
        let t = ref_type(&mut u, v);
        let has_args = matches!(u.tipo(t), Type::Interface(_, args) if !args.is_empty());
        has_args.then(|| u.text(t))
    })
}

/// The type of a `Ref` value.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_of_value(v: i64) -> i64 {
    RTI.with(|u| ref_type(&mut u.borrow_mut(), v))
}

/// `v is T`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_e(v: i64, t: i64) -> u8 {
    let key = value_key(v);
    if let Some(r) = key.and_then(|c| fast_memo_read(c, t)) {
        return r;
    }
    RTI.with(|u| {
        let mut u = u.borrow_mut();
        let r = if let Some(r) = u.test_without_value(v, t) {
            r
        } else if let Some(r) = key.and_then(|c| u.memo_and_read(c, t)) {
            r
        } else {
            let s = ref_type(&mut u, v);
            let r = u.sub(s, t);
            if debug() {
                eprintln!("[depurar] {} is {} = {r}", u.text(s), u.text(t));
            }
            if let Some(c) = key {
                u.memo_and_store(c, t, r);
            }
            r
        };
        if let Some(c) = key {
            fast_memo_store(c, t, r);
        }
        u8::from(r)
    })
}

/// `S <: T`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_subtype(s: i64, t: i64) -> u8 {
    RTI.with(|u| u8::from(u.borrow_mut().sub(s, t)))
}

/// `v as T`: pending `TypeError` if `v` is not a `T` (the VM message:
/// "type 'S' is not a subtype of type 'T' in type cast").
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_as(v: i64, t: i64) {
    dartforge_rti_as_with_context(v, t, 0, 0);
}

/// [`dartforge_rti_as`] with the check context, which changes the end of the
/// VM message: 0 the `as` (" in type cast"), 1 the implicit assignment of
/// a `dynamic` (no suffix), 2 the parameter `name` (a `String`) on entry
/// to a dynamic or covariant call (" of 'name'").
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_as_with_context(v: i64, t: i64, contexto: i64, name: i64) {
    // The suffix is only built on failure: a passing check (almost
    // all of them) allocates nothing.
    let suffix = || match contexto {
        0 => " in type cast".to_string(),
        1 => String::new(),
        _ => format!(" of '{}'", HEAP.with(|h| h.borrow().text(name).map(|t| t.para_string()).unwrap_or_default())),
    };
    let failure = RTI.with(|u| {
        let mut u = u.borrow_mut();
        if u.test_without_value(v, t) == Some(true) {
            return None;
        }
        let key = value_key(v);
        if key.and_then(|c| u.memo_and_read(c, t)) == Some(true) {
            return None;
        }
        let s = ref_type(&mut u, v);
        let r = u.sub(s, t);
        if let Some(c) = key {
            u.memo_and_store(c, t, r);
        }
        if r {
            None
        } else {
            Some(format!("type '{}' is not a subtype of type '{}'{}", u.text(s), u.text(t), suffix()))
        }
    });
    if let Some(msg) = failure {
        let m = HEAP.with(|h| h.borrow_mut().allocate_str(&msg));
        let e = with_roots(&[m], || dartforge_type_error_with_message(m));
        with_roots(&[e], || dartforge_exception_throw(e, 3));
    }
}

/// Heap class id of the `Type` object (the VM's `_Type`): an object with the
/// type id in field 0, canonical per type (`identical(A, A)`, map
/// key). The compiler registers the class with its `toString`.
const TYPE_CLASS: i64 = 0x3FFF_FF01;

thread_local! {
    static TYPE_OBJECTS: RefCell<HashMap<i64, i64>> = RefCell::new(HashMap::default());
}

/// The canonical `Type` object of type `t` (permanent root, like enum
/// values).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_object_type(t: i64) -> i64 {
    if let Some(h) = TYPE_OBJECTS.with(|m| m.borrow().get(&t).copied()) {
        return h;
    }
    let h = HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let h = heap.new_object(TYPE_CLASS, &[(t, false)]);
        // Permanent root in an id that the program globals (non-negative)
        // and the event loop (small negatives) do not use. The sabotage
        // `tipo_sem_raiz` (D12, docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
        // §7.3) removes the registration: the cache then keeps a handle that
        // collection frees.
        if !crate::heap::sabotage("tipo_sem_raiz") {
            heap.set_global_root(-(1_i64 << 40) - t, h);
        }
        h
    });
    TYPE_OBJECTS.with(|m| m.borrow_mut().insert(t, h));
    h
}

/// [`dartforge_rti_object_type`] under ARC: the canonical `Type` object with a
/// code owner, independent of the cache's permanent root. May allocate
/// on the first lookup; does not throw nor run Dart.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let a = dartforge_arc_rti_object_type_owned_v1(7);
/// let b = dartforge_arc_rti_object_type_owned_v1(7);
/// assert_eq!(a, b);
/// assert_eq!(dartforge_arc_observe_heap_v1(a), 3);
/// dartforge_arc_release(a);
/// dartforge_arc_release(b);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_rti_object_type_owned_v1(t: i64) -> i64 {
    let h = dartforge_rti_object_type(t);
    HEAP.with(|heap| heap.borrow_mut().retain_code_owner(h));
    h
}

/// The text of a type (`Type.toString()`), as a heap `String`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_string(t: i64) -> i64 {
    let s = RTI.with(|u| u.borrow().text(t));
    HEAP.with(|h| h.borrow_mut().allocate_str(&s))
}

/// The text of a type under ARC, with a code owner of the result.
///
/// `t` is an RTI universe ID, not a heap reference. The caller transfers
/// the owner or calls release exactly once. Does not run Dart nor leave a
/// pending exception.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let text = dartforge_arc_rti_string_owned_v1(0);
/// assert_eq!(dartforge_arc_observe_heap_v1(text), 3);
/// dartforge_arc_release(text);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observe_heap_v1(text), 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_rti_string_owned_v1(t: i64) -> i64 {
    let s = RTI.with(|u| u.borrow().text(t));
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let text = heap.allocate_str(&s);
        heap.retain_code_owner(text);
        text
    })
}

/// `TypeError` with the message. In the source SDK, the exception must be the real
/// `_TypeError` class for `e is TypeError` and `toString` dispatch.
fn dartforge_type_error_with_message(message: i64) -> i64 {
    if let Some(f) = ajudante("_dartforgeErroDeTipo") {
        // SAFETY: the helper registered by `dart:core` receives a String and
        // returns the concrete `_TypeError` instance.
        let g = { let dart_target: usize = f; move |a0: i64| -> i64 { dart_r1(dart_target, a0) } };
        return with_roots(&[message], || g(message));
    }
    allocate_error_with_trace(1011, vec![(message, true)])
}

#[cfg(test)]
mod tests_arc_borrowed_evaluation {
    use super::*;

    #[test]
    #[allow(unsafe_code)]
    fn evaluating_types_preserves_fields_owners_heap_and_pending() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(crate::heap::Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            let previous_universe = RTI.with(|u| u.replace(Universe::new()));
            let recipe = |text: &str| units_recipe(text.encode_utf16().collect());
            let object = dartforge_arc_object_owned_v1(32001, 2);
            let child = dartforge_arc_box_int_owned_v1(i64::MAX);
            dartforge_arc_write_field_scalar_v1(object, 0, 42);
            dartforge_arc_write_field_ref_v1(object, 1, child);
            dartforge_rti_set(object, recipe("C32001<C200>"));
            let refs = [child, 0, crate::layout::smi::de(7).unwrap()];
            // SAFETY: live vector of three Refs held by the caller.
            let rec = unsafe { dartforge_record_from_refs(refs.as_ptr(), refs.len() as i64) };
            dartforge_arc_retain(rec);
            let parameter = recipe("P0");
            let method = recipe("M0");
            let tuple = recipe("L<C201>");
            let expected = recipe("C200");
            let expected_method = recipe("C201");
            let stats = HEAP.with(|h| h.borrow().stats());
            let pending = dartforge_exception_pending();
            for _ in 0..64 {
                assert_eq!(dartforge_rti_eval(parameter, object, 32001, 0), expected);
                assert_eq!(dartforge_rti_eval(method, 0, 0, tuple), expected_method);
                assert_eq!(dartforge_rti_eval(parameter, rec, 32001, 0), T_DYNAMIC);
                assert_eq!(dartforge_rti_eval(parameter, crate::layout::smi::de(7).unwrap(), 32001, 0), T_DYNAMIC);
            }
            HEAP.with(|h| {
                let h = h.borrow();
                assert_eq!(h.stats().allocations, stats.allocations);
                assert_eq!(h.stats().collections, stats.collections);
                assert_eq!(h.object(object).unwrap().class_id, 32001);
                assert_eq!(h.object(object).unwrap().get(0), Some((42, false)));
                assert_eq!(h.object(object).unwrap().get(1), Some((child, true)));
            });
            assert_eq!(dartforge_exception_pending(), pending);
            assert_eq!(dartforge_arc_observe_heap_v1(child), 3);
            dartforge_arc_release(rec);
            dartforge_arc_release(object);
            dartforge_arc_release(child);
            HEAP.with(|h| h.replace(previous));
            RTI.with(|u| u.replace(previous_universe));
        }
    }
}
