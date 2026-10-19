//! DartForge Native's own HIR (docs/NATIVO.md and PLANO.md § Increment 28).
//!
//! The HIR is an SSA CFG (Control Flow Graph) representation where:
//! - Calls are resolved (static, interface selector, or dynamic).
//! - Implicit casts have been made explicit.
//! - Syntactic sugar has been desugared.
//! - The LLVM backend (and in the future Cranelift) consumes this structure directly.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueId(pub u32);

/// Primitive and managed types of the native HIR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Type {
    /// Signed 64-bit integer with modular overflow (Dart `int`).
    I64,
    /// 8-bit byte (for booleans in the Rust runtime).
    I8,
    /// IEEE 754 64-bit floating point (Dart `double`).
    F64,
    /// 1-bit boolean (Dart `bool`).
    I1,
    /// Handle of a managed object in the heap (tracked by the GC, 0 = null).
    Ref,
    /// Retorno vazio.
    Void,
    /// Address of an `alloca` (a local in memory, R6). Never a Dart value.
    Ptr,
    /// Unboxed `Float32x4`: `<4 x float>` (`docs/SIMD-NATIVO.md`). Only in
    /// locals and temporaries of synchronous functions; at the boundaries (fields,
    /// parameters, returns, collections) the value is the `Ref` box.
    V4F32,
    /// Unboxed `Int32x4`: `<4 x i32>`.
    V4I32,
    /// Unboxed `Float64x2`: `<2 x double>`.
    V2F64,
}

impl Type {
    /// Tipo LLVM IR textual correspondente.
    pub fn llvm_ir(self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::I8 => "i8",
            Self::F64 => "double",
            Self::I1 => "i1",
            Self::Ref => "i64",
            Self::Void => "void",
            Self::Ptr => "ptr",
            Self::V4F32 => "<4 x float>",
            Self::V4I32 => "<4 x i32>",
            Self::V2F64 => "<2 x double>",
        }
    }

    /// One of the unboxed SIMD vectors?
    pub fn is_vector(self) -> bool {
        matches!(self, Self::V4F32 | Self::V4I32 | Self::V2F64)
    }
}

/// A SIMD operation on unboxed vectors, with the VM's lane-by-lane
/// semantics (the same as `runtime/src/simd.rs`). The vector type is that of the
/// instruction or that of the first operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpSimd {
    /// `a op b` lane by lane (`+ - * /` on the floating point ones; `+ -` with
    /// wraparound on overflow and `& | ^` on `Int32x4`).
    Add,
    Sub,
    Mul,
    Div,
    And,
    Or,
    Xor,
    /// `a < b ? a : b` and `a > b ? a : b` (the VM's `Utils::Minimum`/`Maximum`:
    /// with NaN, the second operand).
    Min,
    Max,
    /// `max(min(a, hi), lo)`.
    Clamp,
    Neg,
    Abs,
    Sqrt,
    /// `1 / x` and `sqrt(1 / x)`.
    Recip,
    RecipSqrt,
    /// `v * s`, with `s` a `double` (rounded to `float` in `Float32x4`).
    Scale,
    /// A `Float32x4` comparison that gives the `Int32x4` mask (-1/0).
    Cmp(FCmpOp),
    /// Lane `i` as a `double` (`int` in `Int32x4`).
    Hint(u8),
    /// The vector with lane `i` replaced.
    WithHint(u8),
    /// Lane `i` of the `Int32x4` as a `bool` (≠ 0).
    Flag(u8),
    /// The `Int32x4` with lane `i` at -1/0 by the `bool`.
    WithFlag(u8),
    /// The vector of the given lanes (`double`, `int` or `bool`).
    Builds,
    /// `Int32x4.bool(x, y, z, w)`: -1/0 per lane.
    BuildsFlags,
    Splat,
    Zero,
    /// The sign bits of the lanes, lane 0 in bit 0.
    SignMask,
    /// `shuffle(m)` / `shuffleMix(b, m)` with the constant mask `m`.
    Shuffle(u8),
    ShuffleMix(u8),
    /// `Int32x4.select(t, f)`: bitwise, `t` where the mask has 1.
    Select,
    /// The same bits in another type (`fromInt32x4Bits`, `fromFloat32x4Bits`).
    Bits,
    /// `Float64x2.fromFloat32x4` (lanes 0 and 1) and `Float32x4.fromFloat64x2`
    /// (`[x, y, 0, 0]`).
    Converts,
    /// The vector at address `args[0]` + 16 × `args[1]` (SIMD list).
    Loads,
    /// Stores `args[2]` at address `args[0]` + 16 × `args[1]`.
    Writes,
    // `Int32x4` API of Dart 3.14 (`docs/SIMD-NATIVO.md` §6), through the
    // extension marked `@pragma('dartforge:simd-api', '3.14')`. `Mul`,
    // `Min`, `Max`, `Neg` and `Abs` also serve `Int32x4` (integer, with
    // wraparound on overflow; `abs(-2^31)` is `-2^31`).
    /// Signed lane-by-lane comparison of `Int32x4`: a -1/0 mask.
    CmpInt(ICmpOp),
    /// `a & ~b`.
    AndNot,
    /// `~a`.
    Not,
    /// `a << (s & 31)` (`false`) or arithmetic `a >> (s & 31)` (`true`),
    /// with `s` an `int`.
    Shifts(bool),
    /// Alguma pista ≠ 0 (`anyTrue`) / todas ≠ 0 (`allTrue`): `bool`.
    Any,
    All,
    /// `Float32x4(v.x.toDouble(), v.y.toDouble(), v.z.toDouble(),
    /// v.w.toDouble())` of an `Int32x4` `v` (`lower/simd.rs`): `sitofp`
    /// lane by lane — the same value, since the 32-bit `int` becomes a `double`
    /// without loss and the `double` becomes a `float` with a single rounding.
    IntToFloat,
    /// `fptosi` lane by lane of a `Float32x4` whose lanes are all in
    /// `[-2^31, 2^31)` (checked before by [`OpSimd::InInt32Range`]): the
    /// `toInt()` of each lane, which then neither throws nor saturates.
    FloatToInt,
    /// All the lanes of the `Float32x4` in `[-2^31, 2^31)` (NaN is not):
    /// `bool`.
    InInt32Range,
}

/// Constants supported in the HIR.
#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    Int(i64),
    Double(f64),
    Bool(bool),
    String(String),
    /// WTF-8 bytes of a Dart literal; preserves lone surrogates.
    StringWtf8(Vec<u8>),
    Null,
    /// The address of the function `symbol` as an `i64` (`ptrtoint`): the same in
    /// all the isolates of the process.
    Function(String),
}

/// Instruction operand: a direct constant or a previous SSA result.
#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    Constant(Constant),
    Val(ValueId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ICmpOp {
    Eq,
    Ne,
    Slt,
    Sle,
    Sgt,
    Sge,
    /// Unsigned less-than: `i u< n` tests `0 <= i < n` at once.
    Ult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FCmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Classified owning slot; does not accept an arbitrary native pointer.
///
/// Frames are scalar IDs of the ARC ABI, with Ref slots initially null.
/// The runtime checks identity, ownership and the index bound.
/// Heap/native slots still require their own descriptors.
///
/// ```
/// use dartforge_emit_native::hir::*;
/// let slot = StrongSlot::Frame { frame: Operand::Val(ValueId(0)), index: 2 };
/// assert!(matches!(slot, StrongSlot::Frame { index: 2, .. }));
/// ```
#[derive(Debug, Clone)]
pub enum StrongSlot {
    /// Ref slot in an owning frame opened by the ABI, identified by an I64 SSA value.
    Frame { frame: Operand, index: u32 },
    /// Ref global storage declared exactly once in the module.
    /// Lazy initialization remains the responsibility of the lowering.
    ///
    /// ```
    /// use dartforge_emit_native::hir::*;
    /// let slot = StrongSlot::Global { symbol: "dfg.resource".into() };
    /// assert!(matches!(slot, StrongSlot::Global { .. }));
    /// ```
    Global { symbol: String },
}

/// How to publish a reference in the owning slot.
///
/// ```
/// use dartforge_emit_native::hir::StrongStoreMode;
/// assert_ne!(StrongStoreMode::Copy, StrongStoreMode::Move);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrongStoreMode {
    /// Retains an independent occurrence; does not consume the SSA token.
    Copy,
    /// Consumes the SSA token on success, transferring its occurrence.
    Move,
}

/// Identity of the resolved instance-field declaration, independent of receiver layout.
/// This is a declaration fact, not a physical class or stored-value certificate.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldDeclaration {
    pub library_uri: String,
    pub owner: String,
    pub name: String,
    pub is_final: bool,
    pub is_late: bool,
    /// Position from the declaring class layout; not a receiver-class certificate.
    pub layout_index: usize,
    /// Declared storage representation before conversion to the runtime bits ABI.
    pub storage_type: Type,
}

#[cfg(test)]
impl FieldDeclaration {
    /// Semantic identity excludes flags and layout premises, which a proof must reconcile.
    pub(crate) fn identity(&self) -> (&str, &str, &str) {
        (&self.library_uri, &self.owner, &self.name)
    }
}

/// Lowering premise for a resolved, simple enum declaration with certified equality.
/// Consumers must separately audit the current getter/cache bodies and all writers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct SimpleEnumConstant {
    pub(crate) library_uri: String,
    pub(crate) unit_uri: String,
    pub(crate) owner: String,
    pub(crate) declaration: u32,
    pub(crate) index: usize,
    pub(crate) name: String,
    pub(crate) class_id: u32,
    pub(crate) getter: String,
    pub(crate) global: String,
    pub(crate) root: u32,
}

/// Instructions that produce a value (or perform a write effect).
#[derive(Debug, Clone)]
pub enum Instruction {
    /// Loads a constant into an SSA identifier.
    Const(Constant),

    /// Copies an already evaluated reference and produces an independent owned token.
    /// Retains without collecting. The operand is an SSA Ref or null; does not create boxes.
    ///
    /// ```
    /// use dartforge_emit_native::hir::*;
    /// let copy = Instruction::ArcCopy { value: Operand::Val(ValueId(0)) };
    /// assert!(matches!(copy, Instruction::ArcCopy { .. }));
    /// ```
    ArcCopy { value: Operand },
    /// Consumes an owned token, without collecting or running Dart.
    /// The token verifier must prove availability and single consumption.
    ///
    /// ```
    /// use dartforge_emit_native::hir::*;
    /// let drop = Instruction::ArcDrop { value: Operand::Val(ValueId(1)) };
    /// assert!(matches!(drop, Instruction::ArcDrop { .. }));
    /// ```
    ArcDrop { value: Operand },
    /// Transfers the token to the Ref result without changing the physical RC.
    /// Consumes the logical origin; keep the operation until ownership is verified.
    ///
    /// ```
    /// use dartforge_emit_native::hir::*;
    /// let movement = Instruction::ArcMove { value: Operand::Val(ValueId(1)) };
    /// assert!(matches!(movement, Instruction::ArcMove { .. }));
    /// ```
    ArcMove { value: Operand },

    /// Loads a classified slot and produces an owned token independent of the slot.
    ///
    /// ```
    /// use dartforge_emit_native::hir::*;
    /// let slot = StrongSlot::Frame { frame: Operand::Val(ValueId(0)), index: 0 };
    /// let load = Instruction::ArcLoadStrong { slot };
    /// assert!(matches!(load, Instruction::ArcLoadStrong { .. }));
    /// ```
    ArcLoadStrong { slot: StrongSlot },
    /// Publishes the already evaluated reference before releasing the old content.
    ///
    /// ```
    /// use dartforge_emit_native::hir::*;
    /// let slot = StrongSlot::Frame { frame: Operand::Val(ValueId(0)), index: 0 };
    /// let store = Instruction::ArcStoreStrong {
    ///     slot, value: Operand::Constant(Constant::Null), mode: StrongStoreMode::Copy,
    /// };
    /// assert!(matches!(store, Instruction::ArcStoreStrong { .. }));
    /// ```
    ArcStoreStrong { slot: StrongSlot, value: Operand, mode: StrongStoreMode },

    // Integer arithmetic and logic
    Add(Operand, Operand),
    Sub(Operand, Operand),
    Mul(Operand, Operand),
    SDiv(Operand, Operand), // ~/
    SRem(Operand, Operand), // %
    Shl(Operand, Operand),
    AShr(Operand, Operand),
    /// `>>>`: logical shift.
    LShr(Operand, Operand),
    And(Operand, Operand),
    Or(Operand, Operand),
    Xor(Operand, Operand),
    Neg(Operand),
    Not(Operand), // ~

    // Floating point arithmetic
    FAdd(Operand, Operand),
    FSub(Operand, Operand),
    FMul(Operand, Operand),
    FDiv(Operand, Operand), // /
    FNeg(Operand),

    // Comparisons
    ICmp(ICmpOp, Operand, Operand),
    FCmp(FCmpOp, Operand, Operand),
    LNot(Operand), // ! (booleano)

    // Primitive conversions
    IntToDouble(Operand),
    DoubleToInt(Operand),
    ZExt { op: Operand, from: Type, to: Type },
    Trunc { op: Operand, from: Type, to: Type },
    /// Reinterprets the bits between `i64` and `double` (a value read from the heap or
    /// written to it). It is not a numeric conversion: that is `IntToDouble`.
    Bitcast { op: Operand, to: Type },
    /// A scalar (`I64`/`F64`/`I1`) in a `Ref` position: a box in the heap (R3).
    Box { op: Operand, from: Type },
    /// A box back to the scalar; null or another type throws `TypeError`
    /// (a pending exception — the emitter checks it, as in a call).
    Unbox { op: Operand, to: Type },

    // Heap allocations
    AllocObject {
        class_id: u32,
        fields: Vec<Operand>,
    },
    AllocList {
        elements: Vec<(Operand, u8)>,
    },
    AllocRecord {
        elements: Vec<(Operand, u8)>,
    },
    AllocCell {
        value: Operand,
    },
    AllocEnv {
        values: Vec<Operand>,
    },
    /// The interpolation `'a$b c'`: the parts in a single string, with one allocation
    /// (`dartforge_string_join_typed`), not one per concatenation. Each
    /// part is a text (`Ref`) or an unboxed `int` (`I64`), written in
    /// decimal directly into the result.
    ConcatStrings {
        parts: Vec<Operand>,
    },
    AllocClosure {
        code_symbol: String,
        env: Operand,
    },
    /// A closure whose body has a typed ABI (`lower/closures.rs`): besides the
    /// uniform entry `code_symbol`, the `typed` body
    /// `(env, p0…) -> r` in the HIR representations, with their code `abi`,
    /// which the typed call checks.
    AllocClosureTyped {
        code_symbol: String,
        env: Operand,
        typed: String,
        abi: i64,
        /// `env` is the only captured value, kept in place of the environment
        /// (`lower/closures.rs`, direct environment): it is not an environment.
        direct: bool,
    },
    /// Call of the typed body of a closure: `target` is the address (`I64`,
    /// from `dartforge_closure_typed`), the arguments go in the HIR
    /// representations (the first is the environment) and the result comes back in `ret`. The
    /// exception leaves through the pending flag, as in a Dart call.
    TypedCall {
        target: Operand,
        args: Vec<(Operand, Type)>,
        ret: Type,
    },

    // Local memory and stack pointers
    Alloca(Type),
    Load {
        ptr: Operand,
        ty: Type,
    },
    Store {
        ptr: Operand,
        val: Operand,
    },

    // Access to properties and positions
    GetField {
        object: Operand,
        index: usize,
    },
    SetField {
        object: Operand,
        index: usize,
        value: Operand,
    },
    GetListElement {
        list: Operand,
        index: Operand,
    },
    SetListElement {
        list: Operand,
        index: Operand,
        value: Operand,
    },
    CellGet {
        cell: Operand,
    },
    CellSet {
        cell: Operand,
        value: Operand,
    },
    EnvGet {
        env: Operand,
        index: usize,
    },

    // Chamadas
    CallStatic {
        symbol: String,
        args: Vec<Operand>,
        ret_ty: Type,
    },
    CallInterface {
        receiver: Operand,
        selector_id: u32,
        selector_name: String,
        args: Vec<Operand>,
        ret_ty: Type,
    },
    CallDynamic {
        receiver: Operand,
        selector_name: String,
        args: Vec<Operand>,
        ret_ty: Type,
    },
    /// Call of a function value (closure, tear-off) through the uniform
    /// convention: all arguments `Ref`, the positional ones first and then
    /// the named ones in the order of `names` (sorted); the result is `Ref`. The
    /// emitter builds the argument vector and the descriptor
    /// (`[n_positional, n_named, hash(name)…]`) and calls the uniform
    /// entry of the closure through the address kept in it (`@df_clo_invalido`
    /// when the value is not a closure). As in the call by selector, the vector
    /// has one extra slot, after the arguments, with the RTI tuple of the
    /// type arguments (`tuple_types`, `I64`; `0` when the call
    /// passes none): it is from it that a generic closure reads its own.
    CallClosure {
        closure: Operand,
        args: Vec<Operand>,
        names: Vec<String>,
        ret_ty: Type,
        tuple_types: Operand,
    },
    CallRuntime {
        name: String,
        args: Vec<(Operand, Type)>,
        ret_ty: Type,
    },
    /// Checked tracing field operation with its resolved declaration attached.
    /// Runtime ABI, exceptions, rooting and side effects remain identical to CallRuntime.
    /// Unannotated operations are unknown to any declaration writer certificate.
    DeclaredFieldCall {
        declaration: FieldDeclaration,
        name: String,
        args: Vec<(Operand, Type)>,
        ret_ty: Type,
    },

    // Type check and cast
    CheckNotNull(Operand),
    IsClass {
        object: Operand,
        class_id: u32,
    },

    // Module globals (top-level variables and static fields — N6)
    LoadGlobal {
        symbol: String,
        ty: Type,
    },
    /// `root`: id of the `Ref` global, which the runtime keeps as a permanent
    /// root (N6/G6); `None` for the flag and for scalars.
    StoreGlobal {
        symbol: String,
        val: Operand,
        ty: Type,
        root: Option<u32>,
    },

    /// Unboxed SIMD operation (`OpSimd`); the type of the result is that of the
    /// instruction.
    Simd {
        op: OpSimd,
        args: Vec<Operand>,
    },

    // Phi node
    Phi {
        incoming: Vec<(BlockId, Operand)>,
        ty: Type,
    },

    // --- Closures (P1, docs/NATIVO-PLANO.md §7.4) -----------------------
    /// The canonical tear-off of the function whose uniform entry is `code_symbol`
    /// (always the same handle: `identical(f, f)`).
    TearOff {
        code_symbol: String,
    },
    /// `base[index]` of a vector of `i64` (arguments or descriptor of the
    /// uniform convention). The recorded type says the representation read
    /// (`Ref` for an argument, `I64` for a field of the descriptor).
    LoadIndexed {
        base: Operand,
        index: Operand,
    },
    /// Address (`Ptr`) of a constant vector of `i64`, an immutable module
    /// global: uniform signatures and tables of ARC physical layouts.
    /// The words are native data; they are not Ref slots of the heap.
    ConstArray(Vec<i64>),
    /// Address (`Ptr`) of a constant vector with the address of each function
    /// (`@df.fns.<k>`, a module global): the getters of the constants of a
    /// large literal that becomes a table (`lower/literals.rs`,
    /// `fill_from_table`), which the runtime calls.
    FunctionTable(Vec<String>),

    // --- P5c (source SDK, δ) --------------------------------------------
    /// Call of an instance member through the **selector** (`c:m`, `g:x`,
    /// `s:x`, `lower/sdk_source.rs`), in the open world: the implementation comes from the
    /// method table of the dynamic class of the receiver (registered in the
    /// runtime per class), with a cache per call site. The convention is
    /// the uniform one of closures: all arguments `Ref`, positional and
    /// then named in the order of `names`; the result is `Ref`. A receiver without
    /// the member: a pending `NoSuchMethodError`.
    CallSelector {
        selector: String,
        recv: Operand,
        args: Vec<Operand>,
        names: Vec<String>,
        /// RTI tuple of the generic method, in the hidden slot after the arguments.
        tuple_types: Operand,
    },
    /// Calls the function value `closure` passing on the argument vector and the
    /// descriptor already built (the parameters of a uniform entry): the
    /// `c:x` adapter of a getter or field calls the read value.
    CallClosureForward {
        /// Owned return through the ARC adapter, with a certified uniform entry.
        arc_owned: bool,
        closure: Operand,
        args: Operand,
        desc: Operand,
    },
    /// `recv.<selector>` passing on the argument vector and the descriptor already
    /// built (those of a uniform entry, with the type tuple in the slot
    /// after the arguments): the entry of the tear-off of a method calls the
    /// implementation of the receiver, which checks the arguments and completes the
    /// optional ones with its defaults. Result `Ref`.
    CallSelectorForward {
        /// Uses the ARC adapter with an Owned return and a pending guard.
        /// It can only be emitted together with the certified AOT inventory.
        arc_owned: bool,
        selector: String,
        recv: Operand,
        args: Operand,
        desc: Operand,
    },
    /// Call to a native (C) function at address `target` (`i64`), with the
    /// C ABI of the target (`dart:ffi`, `lower/ffi.rs`). The arguments come in the
    /// HIR representation — integers and pointers `I64`, floating point
    /// `F64`, `bool` `I1` — and are converted to the C type; the result comes back
    /// the same way (integers extended to 64 bits by the sign of the C type).
    NativeCall {
        target: Operand,
        args: Vec<(Operand, CType)>,
        ret: CType,
    },
    /// The element `index` (C type `ty`) of the memory at `address` (`I64`):
    /// the fast path of a typed list, with the bounds already checked.
    /// The result comes in the Dart representation of the type (`I64`, `F64`).
    NativeLoad {
        address: Operand,
        index: Operand,
        ty: CType,
    },
    /// Stores `value` (Dart representation) as the element `index` of C type
    /// `ty` at `address`.
    NativeWrite {
        address: Operand,
        index: Operand,
        ty: CType,
        value: Operand,
    },
    /// A native call with structs or unions by value, or variadic: the operand of a
    /// compound is the address (`I64`) of its bytes, and a compound return is
    /// written to `dest` (the result of the instruction is then `Void`). The
    /// emitter applies the C ABI of the target (`llvm/c_abi.rs`).
    CompositeNativeCall {
        target: Operand,
        args: Vec<(Operand, NativeType)>,
        ret: NativeType,
        dest: Option<Operand>,
        /// Variadic function (`VarArgs`): how many parameters are fixed; the
        /// following ones go with the C argument promotions.
        variadic_flag: Option<usize>,
    },
}

/// A type at the native boundary: primitive or a compound by value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NativeType {
    Prim(CType),
    Composite(LayoutC),
}

/// The C layout of a struct or union: size, alignment and the primitive
/// leaves (offset, type), with nested structs and arrays
/// flattened — what the ABI classification examines.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LayoutC {
    pub size: usize,
    pub alignment: usize,
    pub leaves: Vec<(usize, CType)>,
}

impl LayoutC {
    /// Some leaf outside its natural alignment (`@Packed`).
    pub fn misaligned(&self) -> bool {
        self.leaves.iter().any(|(o, t)| {
            let n = t.c_size();
            n > 1 && o % n != 0
        })
    }
}

impl NativeType {
    /// The HIR representation of the corresponding Dart value (a compound
    /// arrives as an address).
    pub fn hir_type(&self) -> Type {
        match self {
            NativeType::Prim(t) => t.hir_type(),
            NativeType::Composite(_) => Type::I64,
        }
    }
}

/// A C type at the boundary of a native call (the `dart:ffi` primitives,
/// with the ABI-specific integers already resolved).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CType {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
    Bool,
    Ptr,
    /// `Handle`: a Dart object at the native boundary, as the opaque pointer
    /// of a runtime cell (the VM's `Dart_Handle`).
    Handle,
    Void,
}

impl CType {
    /// The LLVM type of the C value.
    pub fn llvm(self) -> &'static str {
        match self {
            CType::I8 | CType::U8 => "i8",
            CType::I16 | CType::U16 => "i16",
            CType::I32 | CType::U32 => "i32",
            CType::I64 | CType::U64 => "i64",
            CType::F32 => "float",
            CType::F64 => "double",
            CType::Bool => "i1",
            CType::Ptr | CType::Handle => "ptr",
            CType::Void => "void",
        }
    }

    /// The HIR representation of the corresponding Dart value.
    pub fn hir_type(self) -> Type {
        match self {
            CType::F32 | CType::F64 => Type::F64,
            CType::Bool => Type::I1,
            CType::Void => Type::Void,
            _ => Type::I64,
        }
    }

    /// The extension attribute of a narrow parameter (the caller extends:
    /// required by Apple's ABI on arm64 and harmless on the others).
    pub fn extension(self) -> &'static str {
        match self {
            CType::I8 | CType::I16 => "signext ",
            CType::U8 | CType::U16 | CType::Bool => "zeroext ",
            _ => "",
        }
    }

    /// Bytes of the C type (0 for `void`).
    pub fn c_size(self) -> usize {
        match self {
            CType::I8 | CType::U8 | CType::Bool => 1,
            CType::I16 | CType::U16 => 2,
            CType::I32 | CType::U32 | CType::F32 => 4,
            CType::I64 | CType::U64 | CType::F64 | CType::Ptr | CType::Handle => 8,
            CType::Void => 0,
        }
    }

    /// The letter of the type in the key of a native signature.
    pub fn letter(self) -> char {
        match self {
            CType::I8 => 'a',
            CType::U8 => 'h',
            CType::I16 => 's',
            CType::U16 => 't',
            CType::I32 => 'i',
            CType::U32 => 'j',
            CType::I64 => 'l',
            CType::U64 => 'm',
            CType::F32 => 'f',
            CType::F64 => 'd',
            CType::Bool => 'b',
            CType::Ptr => 'p',
            CType::Handle => 'H',
            CType::Void => 'v',
        }
    }
}

/// Control flow terminator of a basic block.
#[derive(Debug, Clone)]
pub enum Terminator {
    Return(Option<Operand>),
    Branch(BlockId),
    CondBranch {
        cond: Operand,
        then_block: BlockId,
        else_block: BlockId,
    },
    Switch {
        val: Operand,
        default: BlockId,
        cases: Vec<(i64, BlockId)>,
    },
    Throw(Operand),
    Unreachable,
}

/// Basic block in SSA.
#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub id: BlockId,
    pub instructions: Vec<(ValueId, Instruction, Type)>,
    pub terminator: Terminator,
}

/// Function compiled into the HIR.
#[derive(Debug, Clone)]
pub struct Function {
    pub symbol: String,
    pub name: String,
    pub params: Vec<(ValueId, String, Type)>,
    pub return_ty: Type,
    pub blocks: Vec<BasicBlock>,
    /// The source positions, when the compilation asks for debug
    /// information (`CompileOptions::debug`, J05).
    pub debug_info: Option<Box<FunctionDebugInfo>>,
}

/// The source of a function and the position of each instruction (that of the statement that
/// generated it), for the line tables of the debugger (`llvm/debug_info.rs`).
/// An instruction without a position (created by an optimization pass) inherits that of the
/// previous one in the block.
#[derive(Debug, Clone, Default)]
pub struct FunctionDebugInfo {
    /// The absolute path of the `.dart` file (or the URI, without a file).
    pub file: String,
    /// The url of the script in the VM's stack trace (§13.14, `Context::stack_trace_url`).
    pub url: String,
    /// The line of the first statement with a position (that of the `DISubprogram`).
    pub line: u32,
    /// `(line, column)` of each instruction, starting at 1.
    pub positions: std::collections::HashMap<ValueId, (u32, u32)>,
    /// That of the terminator of each block (the `return` and the `break` emit no
    /// instruction).
    pub exits: std::collections::HashMap<BlockId, (u32, u32)>,
    // --- the stack trace in the VM's format (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.14)
    /// The position of the function token (the VM's `Function.token_pos`: the name,
    /// or the start of a closure): that of the frame of a closure that listens to a
    /// `Future` (`kFutureListenerPcOffset`).
    pub token: (u32, u32),
    /// The functions copied into this one by HIR inlining (`optimize::inline`),
    /// so that the stack trace shows the frame of each one as the VM shows the inlined
    /// frames (`GetInlinedFunctionsAtReturnAddress`).
    pub embedded_set: Vec<Embedded>,
    /// `(line, column, context)` of the instructions copied by inlining: the
    /// position in the copied function and its index in `embedded_set`. It stays out of
    /// `positions`, which belongs to the line tables (the file is that of this function).
    pub embedded_positions: std::collections::HashMap<ValueId, (u32, u32, u32)>,
    /// The stack trace marks ([`stack_trace_marks`]).
    pub marks: u8,
    /// `async` body (the state machine): the entry of its closures and the
    /// position of each `await`.
    pub async_body: Option<TraceAsyncBody>,
    /// The uniform entry (`$ent`), when this function is the body of a
    /// closure: the code that the closure keeps, through which the runtime finds the
    /// function of a closure that listens to a `Future`.
    pub closure_entry: Option<String>,
    /// The capture marked `@pragma('vm:awaiter-link')` of a closure: the link
    /// to the awaiter (the VM's `ClosureData::awaiter_link`).
    pub chain_link: Option<WaitLink>,
}

impl Function {
    /// The name of the function in the stack trace in the VM's format (the
    /// `QualifiedUserVisibleName`): the Dart qualified one, taken from the stable
    /// symbol `df.<library>.<class>.<member>`, with the parts that the
    /// lowering adds replaced as the VM writes them: `$clo<k>` becomes
    /// `.<anonymous closure>`, `$<name>` of a local function becomes `.<name>`, and the
    /// fingerprints and the body suffixes (`$e…`, `$q…`, `$async`,
    /// `$ent`, `$new`, the repetition `$<k>`) are dropped.
    pub fn stack_trace_name(&self) -> String {
        let base = match self.symbol.strip_prefix("dart_main") {
            Some(rest) => format!("main{rest}"),
            None => self
                .symbol
                .strip_prefix("df.")
                .and_then(|rest| rest.split_once('.'))
                .map(|(_, name)| name.trim_start_matches('.').to_string())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| self.name.clone()),
        };
        let printing = |s: &str, p: char| s.len() == 9 && s.starts_with(p) && s[1..].bytes().all(|b| b.is_ascii_hexdigit());
        let mut parts = base.split('$');
        let mut name = parts.next().unwrap_or_default().to_string();
        for p in parts {
            if p.is_empty() || p == "async" || p == "ent" || p == "new" || printing(p, 'e') || printing(p, 'q') || p.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            if let Some(k) = p.strip_prefix("clo")
                && !k.is_empty()
                && k.bytes().all(|b| b.is_ascii_digit())
            {
                name.push_str(".<anonymous closure>");
                continue;
            }
            name.push('.');
            name.push_str(p);
        }
        name
    }
}

/// A function copied by HIR inlining.
#[derive(Debug, Clone)]
pub struct Embedded {
    /// The name of the copied function, as the stack trace writes it.
    pub name: String,
    /// The url of its script.
    pub url: String,
    /// The position of the call that copied it, in the outer function (the context
    /// `parent`, or the function itself).
    pub call: (u32, u32),
    /// The outer context (index in `embedded_set`); `None`: the function itself.
    pub parent: Option<u32>,
    /// The stack trace marks of the copied function.
    pub marks: u8,
}

/// The stack trace part of an `async` body.
#[derive(Debug, Clone, Default)]
pub struct TraceAsyncBody {
    /// The uniform entry of the body (the code of the closure of the body, whose
    /// environment keeps the frame at position 0).
    pub entry: String,
    /// The position of the `await` of each state: index `k − 1` for state `k`.
    pub waits: Vec<(u32, u32)>,
}

/// Where the capture that is the link to the awaiter lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaitLink {
    /// The position in the environment (without `direct`).
    pub index: usize,
    /// The environment is the captured value itself (`lower/closures.rs`).
    pub direct: bool,
    /// The capture lives in a cell.
    pub cell: bool,
}

/// The stack trace marks of a function ([`FunctionDebugInfo::marks`]).
pub mod stack_trace_marks {
    /// The body of an `async` function (the state machine): its frame,
    /// resumed, starts the chain of awaiters.
    pub const ASYNC_BODY: u8 = 1;
    /// Outside the stack trace: the stub of an `async` function (the VM has a single frame
    /// for it) and the helpers of `async_patch` (`@pragma("dartforge:hidden-frame")`).
    pub const HIDDEN: u8 = 2;
    /// `_FutureListener.handleValue`: its listener starts the chain
    /// (`MethodRecognizer::kFutureListenerHandleValue`).
    pub const LISTEN: u8 = 4;
    /// With [`ASYNC_BODY`], the body of an `async*` generator: its frame on the
    /// stack always counts as already suspended (`WasPreviouslySuspended`), and the
    /// next one in the chain is the controller.
    pub const ASYNC_GENERATOR: u8 = 8;
}

/// Class definition in the HIR.
#[derive(Debug, Clone)]
pub struct ClassDef {
    pub id: u32,
    pub name: String,
    pub field_count: usize,
    /// Map of selector_id -> symbol of the implemented function
    pub vtable: Vec<(u32, String)>,
    pub to_string_symbol: Option<String>,
}

/// Globally registered call selector.
#[derive(Debug, Clone)]
pub struct SelectorDef {
    pub id: u32,
    pub name: String,
    pub arity: usize,
}

/// A struct/union of the program for the runtime: the class (RTI and heap), where
/// `_typedDataBase`/`_offsetInBytes` sit in the object, and the measure in the ABI.
#[derive(Debug, Clone)]
pub struct FfiComposite {
    pub rti: i64,
    pub class: i64,
    pub fields: i64,
    pub base_index: i64,
    pub offset_index: i64,
    pub size: i64,
    pub alignment: i64,
}

/// A native callback of a signature: the key, the HIR body (context and
/// arguments in the Dart representation → Dart return; a compound goes and comes back
/// as the address of its bytes) and the native types.
#[derive(Debug, Clone)]
pub struct FfiCallback {
    pub key: String,
    pub body: String,
    pub ret: NativeType,
    pub params: Vec<NativeType>,
}

/// A field of the layout of a program class (J03).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutField {
    pub name: String,
    /// Stable declaring library/class identity, independent of physical slot
    /// and class-number assignment. Absent only for legacy metadata.
    pub owner: Option<String>,
    /// `null` is a value of the declared type (`T?`, `dynamic`, `Object?`…).
    pub nullable: bool,
    pub late: bool,
    /// Readable declared type for rejection diagnostics.
    pub ty: String,
    /// Versioned semantic type identity, independent of aliases and arena IDs.
    /// Missing identity cannot justify preserving a live field on reload.
    pub type_identity: Option<String>,
}

/// How a `Return` exits when the exception may be pending, with exceptions by
/// tables (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13): a Dart function
/// never returns to its caller with the exception pending — it unwinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceptionExit {
    /// The exception is pending on every path to this `Return`: unwinds
    /// (`@df.throw`) instead of returning.
    Throws,
    /// Resumes the native object received in this Itanium cleanup landing pad.
    /// Only admits ARC drops and LIFO closing of local frames before the Return.
    /// Does not collect, call Dart or publish a new exception.
    Resume,
    /// May be pending: checks the pending flag and unwinds or returns.
    Guard,
}

/// What the pass of exceptions by tables (`optimize/tables.rs`) decided
/// for a function: which call is an `invoke`, where it lands and how each `Return` exits. The
/// emitter (`llvm/mod.rs`) only reads it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FunctionTables {
    /// The instruction (a Dart call) that becomes an `invoke`, and its landing pad block.
    pub invocations: std::collections::HashMap<ValueId, BlockId>,
    /// The landing pad blocks: they start with the `landingpad` and the restoration of the
    /// top of the shadow stack; only the `invoke` reaches them.
    pub landing_pads: std::collections::HashSet<BlockId>,
    /// The blocks that end in a `Return` with the exception (maybe) pending.
    pub exits: std::collections::HashMap<BlockId, ExceptionExit>,
    /// The function calls Dart code: checks the stack in the prologue
    /// (`StackOverflowError`). In the check model, the read of the
    /// pending flag after each call already gave the function the context where the
    /// stack check lives; here the read disappears, and without this mark
    /// `f() => f()` would overflow the system stack.
    pub checks_stack: bool,
    /// Certified inventory of owners for foreign cleanup; recompute after mutations.
    pub foreign_cleanup: Option<crate::optimize::arc::ForeignCleanup>,
    /// Native roles of shared frames to check Resume/cleanup.
    /// Does not replace the owners certificate nor prove unwinding by itself.
    pub shared_frames: Option<crate::optimize::arc::SharedFramesCfgContract>,
}

/// Semantic role of the native vectors of the uniform entry ABI.
/// The descriptor contains counts/hashes; arguments may contain Ref handles.
/// The address of the vector is neither a handle nor an owning slot of the heap.
///
/// ```
/// use dartforge_emit_native::hir::DartEntryVector;
/// assert_ne!(DartEntryVector::Arguments, DartEntryVector::Descriptor);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DartEntryVector {
    Arguments,
    Descriptor,
}

/// Source-level inlining request. A preference remains subject to safety and budgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlinePolicy {
    Prefer,
    Never,
}

/// Registered ancestry from the exact SDK images linked with the program.
/// These nominal facts do not certify allocation identity or field layout.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ArcExternalAncestry {
    pub nominal_classes: std::collections::HashSet<u32>,
    pub ambiguous_classes: std::collections::HashSet<u32>,
    pub parent_edges: Vec<(u32, u32)>,
}

/// Complete HIR module representing a compilable Dart program.
#[derive(Debug, Default, Clone)]
pub struct Module {
    /// J05: the positions of the functions become line tables (`--debug`).
    /// Without this, any positions there are belong only to the symbolic stack trace (§13.14).
    pub dwarf: bool,
    /// The exceptions of the module are by tables (`--exceptions=tables`): the
    /// `optimize::tables` pass ran and [`Module::tables`] holds. False: the usual
    /// model (pending flag checked after each call).
    pub table_exceptions: bool,
    /// The memory of the module is ARC (`--memory=arc`, `target::arc_memory`): the
    /// entry turns ARC on in the runtime and the reference stores into an object
    /// that may be old go through the runtime (docs/ARC-IMPLEMENTACAO.md).
    pub arc_memory: bool,
    /// With [`Module::table_exceptions`], the decision of the pass for each
    /// function, in the order of [`Module::functions`].
    pub tables: Vec<FunctionTables>,
    /// `dart:ffi`: (RTI id of the class, letter of the C type) of each native type
    /// and (signature key, trampoline symbol) (`lower/ffi.rs`).
    pub ffi_types: Vec<(i64, char)>,
    pub ffi_trampolines: Vec<(String, String)>,
    /// `dart:ffi`: the C entries of the native callbacks, one per signature
    /// (`lower/ffi.rs`; the emitter generates the entry with the C ABI).
    pub ffi_callbacks: Vec<FfiCallback>,
    /// `dart:ffi`: the structs and unions of the program (`lower/ffi.rs`).
    pub ffi_composites: Vec<FfiComposite>,
    pub functions: Vec<Function>,
    /// Declaration pragmas by body symbol. Generated closures and ABI adapters
    /// keep their own policy; an async declaration also labels its state machine.
    pub inline_policies: std::collections::HashMap<String, InlinePolicy>,
    /// Scalar parameters declared in the source, by symbol/SSA ID.
    /// The lowering records only unboxed int/double/bool. Hidden/native i64
    /// parameters are not classified by width. These facts
    /// still need to feed the ARC plans and survive the passes.
    pub dart_scalar_parameters: std::collections::HashMap<String, std::collections::HashSet<ValueId>>,
    /// Native IDs of the canonical RTI universe, produced explicitly.
    /// They are neither Ref nor Dart scalars; the life of the universe/pins requires another protocol.
    pub dart_rti_parameters: std::collections::HashMap<String, std::collections::HashSet<ValueId>>,
    /// Vectors of the uniform ABI registered explicitly by the lowering.
    /// Life/bounds depend on the protocol of the caller. Descriptors are immutable;
    /// the first two slots are native counts, without heap references.
    pub dart_vector_parameters: std::collections::HashMap<String, std::collections::HashMap<ValueId, DartEntryVector>>,
    /// Typed bodies of source Dart functions whose ABI returns an Owned Ref.
    /// Nominal provenance of the lowering; does not automatically include uniform
    /// entries, FFI callbacks or HIR functions supplied by another producer.
    pub dart_ref_returns: std::collections::HashSet<String>,
    /// Storages of Dart globals whose representation, by the Dart type, is an
    /// unboxed scalar (`int`, `double`, `bool`): the content is never a handle.
    pub dart_scalar_globals: std::collections::HashSet<String>,
    /// Uniform entries declared by the lowering for the ARC ABI with an Owned return.
    /// They are premises to prepare the whole body, not certificates of indirect
    /// dispatch or of C callbacks; the boundaries require their own verification.
    pub arc_uniform_returns: std::collections::HashSet<String>,
    /// Ports of a fixed AOT image, after verifying the bodies and dependencies.
    /// Later changes invalidate the inventory; does not certify JIT reload.
    pub arc_aot_ports: Option<crate::optimize::arc::ArcAotPorts>,
    /// The ARC ownership preparation published these bodies. Without it, an ARC
    /// image follows the legacy regime (direct calls, factories without an owner of the
    /// code); with it, the emission requires the AOT ports certificate.
    pub arc_prepared: bool,
    /// AOT registration keeps this image's ancestry facts fixed.
    /// Reloadable images leave this false: older generations may add edges.
    pub arc_fixed_ancestry: bool,
    /// Selected canonical SDK double identity/hash overlays were independently verified.
    pub checked_double_identity: bool,
    /// Experimental selected-source/ABI premise for primitive clamp. Consumers
    /// must separately prove fixed registration and absence of native exposure.
    pub(crate) primitive_clamp_certificate: bool,
    /// Actual resolved canonical clamp body symbols, never inferred by spelling.
    pub(crate) primitive_clamp_targets: std::collections::HashSet<String>,
    /// Proven allocated class tables for experimental closed reachability.
    /// None preserves the original eager registration; metadata stays separate.
    pub(crate) closed_class_tables: Option<std::collections::HashSet<u32>>,
    /// Registration facts from the same compiled SDK summaries used at link time.
    pub arc_external_ancestry: ArcExternalAncestry,
    /// Artifact-bound allocation return facts from the exact selected linked SDK.
    pub(crate) imported_allocation_returns: Option<crate::pruning::receiver_provenance::allocation_certificates::Imports>,
    /// Snapshot-bound SDK allocation/exposure facts; unknown records never authorize pruning.
    pub(crate) imported_allocation_inventory: Option<std::sync::Arc<crate::pruning::allocation_inventory::imports::Inventory>>,
    /// The ARC preparation accepts direct calls to Dart functions of another
    /// already prepared image (the prepared source SDK), with the summary taken
    /// from the call itself: arguments by the representation of the operands,
    /// possible failure and an Owned Ref result (the convention of Dart functions).
    pub arc_external_callees: bool,
    /// Caches of native RTI IDs declared by the lowering, with an implicit `$ok` flag.
    /// All writes and representations require a check before producing contracts.
    pub arc_rti_caches: std::collections::HashSet<String>,
    /// Shareable definitions declared nominally by the ARC producer.
    /// Does not certify equivalence of the copies nor emission dependencies.
    pub arc_shared_functions: std::collections::HashSet<String>,
    /// Direct local functions declared by the lowering, closed to external linkage.
    /// Pruning still checks calls, materialized addresses and published entries.
    pub arc_direct_local_functions: std::collections::HashSet<String>,
    /// Pre-optimization nominal origin of the allocations recognized by the ARC lowering.
    /// SSA IDs index the current version; copies of the inliner keep the origin
    /// site. Specialized contexts and reload dependencies are still missing.
    pub arc_sites: std::collections::HashMap<String, std::collections::HashMap<ValueId, crate::optimize::arc::analysis::model::ArcSite>>,
    /// Declared representations of the fields per class/position, only under ARC.
    /// Includes inheritance, mixins and the enum prefix; late with an initializer stays Ref.
    /// Does not certify the type of the receiver, the physical shape of the heap or the reload version/pins.
    pub arc_field_layouts: std::collections::HashMap<u32, Vec<Type>>,
    /// The declared layouts of the classes of the prepared source SDK (another
    /// image), in the same form as `arc_field_layouts`: only to check
    /// open stores and reads of the program prepared alone
    /// (`arc_external_callees`); they do not declare classes of this module.
    pub arc_external_layouts: std::collections::HashMap<u32, Vec<Type>>,
    /// Producer-declared managed slots by physical CID; None covers synthetic/unknown slots.
    /// This does not infer stored values or close writers. Missing/ambiguous maps are unknown.
    pub declared_field_layouts: std::collections::HashMap<u32, Vec<Option<FieldDeclaration>>>,
    pub classes: Vec<ClassDef>,
    pub selectors: Vec<SelectorDef>,
    pub subtyping_edges: Vec<(u32, u32)>,
    /// Nominal classes whose complete runtime parent edges were emitted from
    /// resolved declarations. Generated runtime identities remain unknown.
    pub pruning_class_ids: std::collections::HashSet<u32>,
    /// Shared nominal CID aliases are open, even when one declaration has a
    /// complete ancestry. Their union is not a unique receiver identity.
    pub pruning_ambiguous_class_ids: std::collections::HashSet<u32>,
    /// SDK instance bodies: exact formal `this` SSA parameter and its nominal
    /// runtime class. Captured receivers and synthetic adapters are not inferred.
    pub pruning_this_receivers: std::collections::HashMap<String, (ValueId, u32)>,
    pub entry_symbol: Option<String>,
    /// How many parameters `main` declares: with one, it receives the command
    /// line arguments (`List<String>`); the second (`message`, from
    /// `Isolate.spawnUri`) is `null`.
    pub entry_params: usize,
    /// User globals: (variable id, representation). Each one becomes
    /// User globals: (id of the root in the runtime, representation, stable
    /// symbol of the value `dfg.<path>`); the initialization flag is
    /// `<symbol>$ok`.
    pub globals: Vec<(u32, Type, String)>,
    /// Enum declaration premises, not canonical value or successful-return certificates.
    pub(crate) simple_enum_constants: std::collections::HashMap<String, SimpleEnumConstant>,
    /// Constructs that the lowering cannot lower (N1). Non-empty = the
    /// program does not compile; the emitter produces only the message.
    pub errors: Vec<String>,
    // --- P5c (source SDK, δ) ---
    /// The module is part of a program with the SDK compiled from source: the
    /// code shared between modules (tear-off entries, canonical
    /// constants) comes out in a `comdat`, and symbols of other modules are
    /// declared.
    pub sdk_mode: bool,
    /// The module is an SDK library (without `dartforge_entry` or the
    /// `toString` dispatch, which belong to the program).
    pub sdk_library: bool,
    /// Method tables of the classes of this module, registered in the runtime
    /// at startup (P5c): (class id, [(selector, adapter symbol)]).
    pub method_tables: Vec<(u32, String, Vec<(String, String)>)>,
    /// The function that returns the method table of each compiled concrete
    /// class (of any module), by id: the allocation of an object of the
    /// class registers the table (`dartforge_object_new_t`).
    pub table_functions: std::collections::HashMap<u32, String>,
    /// SDK library: the name of the function that registers its classes
    /// (names, subtypes, method tables) in the runtime.
    pub record: Option<String>,
    /// Program with the source SDK: the registration functions of the SDK
    /// libraries, called by `dartforge_entry` before those of the program.
    pub sdk_records: Vec<String>,
    /// The `dart:async` fields that the stack trace walks to find the awaiter
    /// (§13.14): `(class id, "Class.field", position)`, registered at
    /// startup with the symbolic stack trace.
    pub trace_fields: Vec<(u32, String, usize)>,
    /// The tear-off entries that the stack trace recognizes in the stream branches
    /// (§13.14): `(entry symbol, kind)`, kind 1 for
    /// `_StreamIterator._onData` and 2 for `_StreamController._add`.
    pub stack_trace_tearoffs: Vec<(String, i64)>,
    /// The ids of the program classes, `(id, library, class)`: written
    /// in the IR (`; df.class …`) so that the next generation of a JIT
    /// reload gives the same id to the same class (J03, `Context::with_previous_ids`).
    pub program_ids: Vec<(u32, String, String)>,
    /// The layout of the objects of each program class, for the migration of the
    /// live instances on a JIT reload (J03): `(id, fields before the
    /// declared ones — those of enums —, fields)`, written in the IR as
    /// `; df.fields …`.
    pub program_fields: Vec<(u32, usize, Vec<LayoutField>)>,
    /// Generated frame allocations: allocator symbol and SSA site, fingerprinted
    /// resume entry, and field count. A shared async CID does not imply a shared field layout.
    pub(crate) async_frame_layouts: Vec<(String, ValueId, String, usize)>,
    /// Program with the source SDK: the class ids of the values that the
    /// runtime represents (`Null`, `_Smi`, `_Mint`, `_Double`, `bool`,
    /// `_OneByteString`, `_TwoByteString`, `_GrowableList`, `_List`,
    /// `_ImmutableList`, `_Closure`, `_Record`), in the order of
    /// `runtime/src/selectors.rs`.
    pub runtime_cids: Vec<i64>,
    /// Program with the source SDK: the version of the compiled Dart SDK (its
    /// `version` file), which `Platform.version` reports.
    pub sdk_version: Option<String>,
    /// Source SDK members rejected in this module: (symbol, reason).
    /// Each one became a function that warns at runtime.
    pub rejected: Vec<(String, String)>,
    /// Dart functions that the runtime calls by name (`_dartforge*` of the
    /// overlay): (name, symbol), registered by the registration of the module.
    pub helpers: Vec<(String, String)>,
    // --- P6 (source libraries, `source.rs`) -------------------------------
    /// Diagnostics of the functions of SDK libraries compiled from source,
    /// by the symbol of the function that produced them: they only become `errors` if pruning
    /// (`source::prune`) keeps the function.
    pub source_errors: Vec<(String, Vec<String>)>,
    /// Source functions that the runtime calls without the program referencing them
    /// (pruning roots, besides the functions of the program).
    pub source_roots: Vec<String>,
    /// The function that the runtime event loop uses to call a
    /// closure without arguments (`dartforge_event_loop`, after
    /// `main`). `None`: the program does not use `dart:async` and there is no loop.
    pub call_dart: Option<String>,
    /// RTI: the function that registers the type universe before `main`
    /// (`lower::rti::register_universe`); `None` without recipes.
    pub start_rti: Option<String>,
}

impl Module {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers or looks up a global selector_id for a name and arity.
    pub fn get_or_register_selector(&mut self, name: &str, arity: usize) -> u32 {
        if let Some(pos) = self.selectors.iter().position(|s| s.name == name && s.arity == arity) {
            return pos as u32;
        }
        let id = self.selectors.len() as u32;
        self.selectors.push(SelectorDef {
            id,
            name: name.to_string(),
            arity,
        });
        id
    }
}

impl Instruction {
    /// Attaches declaration metadata only when the complete checked field ABI agrees.
    /// This checks a producer layout fact; it does not certify receiver identity or writers.
    pub(crate) fn with_field_declaration(self, declaration: Option<FieldDeclaration>) -> Self {
        match (self, declaration) {
            (Self::CallRuntime { name, args, ret_ty }, Some(declaration))
                if declaration.storage_type != Type::Void
                    && declaration.storage_type != Type::Ptr
                    && !declaration.storage_type.is_vector()
                    && args.first().is_some_and(|(_, ty)| *ty == Type::Ref)
                    && args.get(1).is_some_and(|(index, ty)| *ty == Type::I64
                        && matches!(index, Operand::Constant(Constant::Int(i))
                            if usize::try_from(*i).ok() == Some(declaration.layout_index)))
                    && ((name == "dartforge_object_get" && args.len() == 2
                        && ret_ty == if declaration.storage_type == Type::Ref { Type::Ref } else { Type::I64 })
                        || (name == "dartforge_object_set" && args.len() == 4
                            && ret_ty == Type::Void && args[2].1 == Type::I64 && args[3].1 == Type::I8
                            && matches!(&args[3].0, Operand::Constant(Constant::Int(tag))
                                if *tag == i64::from(declaration.storage_type == Type::Ref)))) =>
            {
                Self::DeclaredFieldCall { declaration, name, args, ret_ty }
            }
            (instruction, _) => instruction,
        }
    }
}

#[cfg(test)]
mod declared_field_tests {
    use super::*;

    fn declaration(owner: &str) -> FieldDeclaration {
        FieldDeclaration { library_uri: "package:fixture/base.dart".into(), owner: owner.into(),
            name: "_encoder".into(), is_final: true, is_late: false,
            layout_index: 0, storage_type: Type::Ref }
    }

    #[test]
    fn identity_is_not_an_incidental_slot_or_subclass() {
        assert_ne!(declaration("Base"), declaration("Unrelated"));
        let mut other_library = declaration("Base");
        other_library.library_uri = "package:other/base.dart".into();
        assert_ne!(declaration("Base"), other_library);
        assert_eq!(declaration("Base"), declaration("Base").clone());
        let original = declaration("Base");
        let mut contradictory_schema = original.clone();
        contradictory_schema.layout_index = 3;
        assert_eq!(original.identity(), contradictory_schema.identity());
        assert_ne!(original, contradictory_schema);
    }

    #[test]
    fn attachment_preserves_checked_arguments_and_unknown_operations() {
        let call = Instruction::CallRuntime { name: "dartforge_object_get".into(),
            args: vec![(Operand::Val(ValueId(1)), Type::Ref),
                (Operand::Constant(Constant::Int(0)), Type::I64)], ret_ty: Type::Ref };
        let annotated = call.clone().with_field_declaration(Some(declaration("Base")));
        assert!(matches!(annotated, Instruction::DeclaredFieldCall { ref name, ref args,
            ret_ty: Type::Ref, .. } if name == "dartforge_object_get" && args.len() == 2));
        assert!(matches!(call.with_field_declaration(None), Instruction::CallRuntime { .. }));
        let unrelated = Instruction::CallRuntime { name: "untrusted_getter".into(),
            args: vec![], ret_ty: Type::Ref };
        assert!(matches!(unrelated.with_field_declaration(Some(declaration("Base"))),
            Instruction::CallRuntime { .. }));
    }

    #[test]
    fn declaration_attachment_refuses_incompatible_abi_and_slot() {
        let get = Instruction::CallRuntime { name: "dartforge_object_get".into(),
            args: vec![(Operand::Val(ValueId(1)), Type::Ref),
                (Operand::Constant(Constant::Int(0)), Type::I64)], ret_ty: Type::Ref };
        let mut invalid = Vec::new();
        for index in [-1, 1] {
            let mut call = get.clone();
            if let Instruction::CallRuntime { args, .. } = &mut call {
                args[1].0 = Operand::Constant(Constant::Int(index));
            }
            invalid.push(call);
        }
        let mut wrong_receiver = get.clone();
        if let Instruction::CallRuntime { args, .. } = &mut wrong_receiver { args[0].1 = Type::I64; }
        invalid.push(wrong_receiver);
        let mut wrong_result = get.clone();
        if let Instruction::CallRuntime { ret_ty, .. } = &mut wrong_result { *ret_ty = Type::I64; }
        invalid.push(wrong_result);
        for tag in [0, 2] {
            invalid.push(Instruction::CallRuntime { name: "dartforge_object_set".into(),
                args: vec![(Operand::Val(ValueId(1)), Type::Ref),
                    (Operand::Constant(Constant::Int(0)), Type::I64),
                    (Operand::Val(ValueId(2)), Type::I64),
                    (Operand::Constant(Constant::Int(tag)), Type::I8)], ret_ty: Type::Void });
        }
        for call in invalid {
            assert!(matches!(call.with_field_declaration(Some(declaration("Base"))),
                Instruction::CallRuntime { .. }));
        }
    }

    #[test]
    fn cloned_field_call_keeps_identity_and_remaps_all_arguments() {
        let call = Instruction::CallRuntime { name: "dartforge_object_set".into(),
            args: vec![(Operand::Val(ValueId(1)), Type::Ref),
                (Operand::Constant(Constant::Int(0)), Type::I64),
                (Operand::Val(ValueId(3)), Type::I64),
                (Operand::Constant(Constant::Int(1)), Type::I8)], ret_ty: Type::Void };
        let annotated = call.clone().with_field_declaration(Some(declaration("Base")));
        let mut cloned = annotated.clone();
        crate::optimize::operands::operands_mut(&mut cloned, &mut |operand| {
            if let Operand::Val(value) = operand { value.0 += 10; }
        });
        let mut arguments = Vec::new();
        crate::optimize::operands::operands(&cloned, &mut |operand| {
            if let Operand::Val(value) = operand { arguments.push(value.0); }
        });
        assert_eq!(arguments, vec![11, 13]);
        assert!(matches!(cloned, Instruction::DeclaredFieldCall { declaration: identity, .. }
            if identity == declaration("Base")));
        assert!(matches!(annotated, Instruction::DeclaredFieldCall { ref args, .. }
            if matches!(args[0].0, Operand::Val(ValueId(1)))));
    }

    #[test]
    fn llvm_keeps_exact_checked_field_emission() {
        for (name, args, ret_ty) in [
            ("dartforge_object_get", vec![(Operand::Constant(Constant::Null), Type::Ref),
                (Operand::Constant(Constant::Int(0)), Type::I64)], Type::Ref),
            ("dartforge_object_set", vec![(Operand::Constant(Constant::Null), Type::Ref),
                (Operand::Constant(Constant::Int(0)), Type::I64),
                (Operand::Constant(Constant::Int(0)), Type::I64),
                (Operand::Constant(Constant::Int(1)), Type::I8)], Type::Void),
        ] {
            let call = Instruction::CallRuntime { name: name.into(), args, ret_ty };
            let module = |instruction| Module { functions: vec![Function {
                symbol: "field_witness".into(), name: "field_witness".into(), params: vec![],
                return_ty: ret_ty, debug_info: None, blocks: vec![BasicBlock {
                    id: BlockId(0), instructions: vec![(ValueId(0), instruction, ret_ty)],
                    terminator: Terminator::Return((ret_ty != Type::Void).then_some(Operand::Val(ValueId(0)))),
                }],
            }], ..Module::default() };
            let plain = module(call.clone());
            let annotated = module(call.with_field_declaration(Some(declaration("Base"))));
            assert_eq!(crate::llvm::LlvmEmitter::new(&plain).emit_all(),
                crate::llvm::LlvmEmitter::new(&annotated).emit_all());
        }
    }
}
