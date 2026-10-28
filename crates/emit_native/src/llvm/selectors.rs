//! Emission of the source SDK (P5c, δ): the call by selector, the method tables
//! of the classes and the declarations of what lives in another module.
//!
//! **Why per-class tables and not the global dispatch table.** The
//! *global dispatch table* of the AOT VM (offset per selector + class
//! id) assumes the whole program: the offsets are chosen seeing
//! every class. With the SDK compiled **once**, before any
//! program, the SDK offsets cannot depend on the classes of the
//! program, and each program would have to redefine a global per selector that the
//! SDK uses (thousands) — or COFF would have to resolve weak symbols, which it
//! does not resolve the way ELF does. What the VM does in the JIT solves the
//! same problem: each class has its own table (selector → code), the lookup
//! happens on the first call at a site and stays in a cache of the site
//! (*inline cache*). The selector is identified by the 64-bit FNV-1a hash of its
//! text (`c:m`, `g:x`, `s:x`; private ones with `@library`), the same in
//! any module, without a combined numbering. The class registers its table at
//! startup (`dartforge_register_methods`).

use super::LlvmEmitter;
use crate::hir::*;
use std::fmt::Write;

#[cfg(test)]
#[path = "table_batch_tests.rs"]
mod table_batch_tests;

/// The hash of a selector (the same as `lower::sdk_source::selector_hash`).
pub fn selector_hash(text: &str) -> i64 {
    crate::lower::closures::hash_name(text)
}

/// Escapes bytes for an LLVM `c"…"`.
pub(super) fn llvm_bytes(s: &str) -> String {
    let mut t = String::new();
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric()
            || b == b' '
            || b == b'_'
            || b == b'.'
            || b == b':'
            || b == b'@'
        {
            t.push(b as char);
        } else {
            t.push_str(&format!("\\{b:02X}"));
        }
    }
    t
}

impl LlvmEmitter<'_> {
    /// `%v = recv.<selector>(args)`: the entry comes from
    /// `dartforge_selector(cache, recv, hash, name, n)` — the call-site cache
    /// (class id, entry), the lookup in the class table on a miss,
    /// and the `NoSuchMethodError` entry when the class does not have the member.
    pub(super) fn emit_selector_call(
        &mut self,
        v: u32,
        selector: &str,
        recv: &Operand,
        args: &[Operand],
        names: &[String],
        tuple_types: &Operand,
    ) {
        let k = self.vector_of[&Self::descriptor(args.len(), names)];
        let n = args.len() + 1;
        for (i, a) in args.iter().enumerate() {
            let s = self.coerce(a, Type::I64);
            writeln!(
                self.out,
                "  %sa{v}_{i} = getelementptr [{n} x i64], ptr %sargs{v}, i64 0, i64 {i}"
            )
            .unwrap();
            writeln!(self.out, "  store i64 {s}, ptr %sa{v}_{i}").unwrap();
        }
        let tuple = self.coerce(tuple_types, Type::I64);
        writeln!(
            self.out,
            "  %sat{v} = getelementptr [{n} x i64], ptr %sargs{v}, i64 0, i64 {}",
            args.len()
        )
        .unwrap();
        writeln!(self.out, "  store i64 {tuple}, ptr %sat{v}").unwrap();
        let r = self.coerce(recv, Type::Ref);
        let name = match self.selector_names.iter().position(|s| s == selector) {
            Some(j) => j,
            None => {
                self.selector_names.push(selector.to_string());
                self.selector_names.len() - 1
            }
        };
        self.selector_caches += 1;
        let h = selector_hash(selector);
        let slot = self.cache_slot();
        writeln!(
            self.out,
            "  %sic{v} = getelementptr i64, ptr %area, i64 {slot}"
        )
        .unwrap();
        if let Operand::Val(value) = recv
            && let Some(classes) = self.current_exact_receivers.get(value)
        {
            let classes = classes
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",");
            writeln!(self.out, "  ; df.selector-exact {classes}").unwrap();
        } else if let Some((parameter, cid)) = self.current_receiver_cone.as_ref()
            && recv == &Operand::Val(*parameter)
        {
            writeln!(self.out, "  ; df.selector-cone {cid}").unwrap();
        }
        if self.compact_selector {
            // The hash, the name and the length in a static descriptor
            // (`@df.seld.<k>`): three arguments instead of five.
            writeln!(
                self.out,
                "  %sf{v} = call ptr @df.selector_d(ptr %sic{v}, i64 {r}, ptr @df.seld.{name})"
            )
            .unwrap();
        } else {
            writeln!(
                self.out,
                "  %sf{v} = call ptr @df.selector(ptr %sic{v}, i64 {r}, i64 {h}, ptr @df.seln.{name}, i64 {})",
                selector.len()
            )
            .unwrap();
        }
        if self.arc_uniform_runtime() {
            writeln!(self.out, "  %sc{v} = ptrtoint ptr %sf{v} to i64").unwrap();
            writeln!(self.out, "  %sav{v} = ptrtoint ptr %sargs{v} to i64").unwrap();
            writeln!(
                self.out,
                "  %sdv{v} = ptrtoint ptr @{} to i64",
                self.vector_symbols[k]
            )
            .unwrap();
            writeln!(self.out, "  %v{v} = call i64 @dartforge_arc_invoke_uniform_v1(i64 %sc{v}, i64 {r}, i64 %sav{v}, i64 %sdv{v})").unwrap();
        } else {
            writeln!(
                self.out,
                "  %v{v} = call i64 %sf{v}(i64 {r}, ptr %sargs{v}, ptr @{})",
                self.vector_symbols[k]
            )
            .unwrap();
        }
    }

    /// `%v = recv.<selector>` with the argument vector and the descriptor of a
    /// uniform entry forwarded as they came (`CallSelectorForward`); the
    /// lookup of the entry is that of [`Self::emit_selector_call`].
    pub(super) fn emit_selector_forward(
        &mut self,
        v: u32,
        selector: &str,
        recv: &Operand,
        args: &Operand,
        desc: &Operand,
        arc_owned: bool,
    ) {
        let r = self.coerce(recv, Type::Ref);
        let a = self.operand_str(args);
        let d = self.operand_str(desc);
        let name = match self.selector_names.iter().position(|s| s == selector) {
            Some(j) => j,
            None => {
                self.selector_names.push(selector.to_string());
                self.selector_names.len() - 1
            }
        };
        self.selector_caches += 1;
        let h = selector_hash(selector);
        let slot = self.cache_slot();
        writeln!(
            self.out,
            "  %sic{v} = getelementptr i64, ptr %area, i64 {slot}"
        )
        .unwrap();
        if let Operand::Val(value) = recv
            && let Some(classes) = self.current_exact_receivers.get(value)
        {
            let classes = classes
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",");
            writeln!(self.out, "  ; df.selector-exact {classes}").unwrap();
        } else if let Some((parameter, cid)) = self.current_receiver_cone.as_ref()
            && recv == &Operand::Val(*parameter)
        {
            writeln!(self.out, "  ; df.selector-cone {cid}").unwrap();
        }
        if self.compact_selector {
            // The hash, the name and the length in a static descriptor
            // (`@df.seld.<k>`): three arguments instead of five.
            writeln!(
                self.out,
                "  %sf{v} = call ptr @df.selector_d(ptr %sic{v}, i64 {r}, ptr @df.seld.{name})"
            )
            .unwrap();
        } else {
            writeln!(
                self.out,
                "  %sf{v} = call ptr @df.selector(ptr %sic{v}, i64 {r}, i64 {h}, ptr @df.seln.{name}, i64 {})",
                selector.len()
            )
            .unwrap();
        }
        if arc_owned {
            assert!(
                self.module.arc_memory && self.module.arc_aot_ports.is_some(),
                "ARC forwarding requires certified AOT ports"
            );
        }
        if arc_owned && self.arc_uniform_runtime() {
            writeln!(self.out, "  %sc{v} = ptrtoint ptr %sf{v} to i64").unwrap();
            writeln!(self.out, "  %sav{v} = ptrtoint ptr {a} to i64").unwrap();
            writeln!(self.out, "  %sdv{v} = ptrtoint ptr {d} to i64").unwrap();
            writeln!(self.out, "  %v{v} = call i64 @dartforge_arc_invoke_uniform_v1(i64 %sc{v}, i64 {r}, i64 %sav{v}, i64 %sdv{v})").unwrap();
        } else {
            writeln!(
                self.out,
                "  %v{v} = call i64 %sf{v}(i64 {r}, ptr {a}, ptr {d})"
            )
            .unwrap();
        }
    }

    /// The names of the call-trace functions (`DARTFORGE_CALL_TRACE=1`).
    pub(super) fn emit_stack_trace_names(&mut self) {
        for (k, s) in std::mem::take(&mut self.stack_trace_names)
            .iter()
            .enumerate()
        {
            writeln!(
                self.out,
                "@df.call_trace.{k} = private unnamed_addr constant [{} x i8] c\"{}\"",
                s.len(),
                llvm_bytes(s)
            )
            .unwrap();
        }
    }

    /// The call-site caches and the names of the selectors.
    pub(super) fn emit_selector_globals(&mut self) {
        if self.selector_caches == 0 {
            return;
        }
        // The per-call-site caches (class id, entry) live in the
        // isolate's global area; here only the names of the selectors.
        for (j, s) in self.selector_names.iter().enumerate() {
            writeln!(
                self.out,
                "@df.seln.{j} = private unnamed_addr constant [{} x i8] c\"{}\"",
                s.len(),
                llvm_bytes(s)
            )
            .unwrap();
            if self.compact_selector {
                writeln!(
                    self.out,
                    "@df.seld.{j} = private unnamed_addr constant {{ i64, ptr, i64 }} {{ i64 {}, ptr @df.seln.{j}, i64 {} }}",
                    selector_hash(s),
                    s.len()
                )
                .unwrap();
            }
        }
        self.out.push('\n');
    }

    /// The `<record>` function: the names of the classes of the module, the subtype
    /// edges and the method tables, handed to the runtime.
    pub(super) fn emit_registration(&mut self, record: &str) {
        let module = self.module;
        let mut body = String::new();
        if module.arc_aot_ports.is_some() {
            writeln!(
                body,
                "  call void @{}()",
                self.arc_ports_registration_symbol()
            )
            .unwrap();
        }
        for class in &module.classes {
            let idx = self.string_const_index(class.name.as_bytes()).unwrap_or(0);
            writeln!(
                body,
                "  call void @dartforge_register_class_name(i64 {}, ptr @.str.{idx}, i64 {})",
                class.id,
                class.name.len()
            )
            .unwrap();
        }
        // The name of each function tear-off of the module, for the closure's
        // `toString` (`Module::tearoff_names`); an entry the pruning removed has
        // no closure to describe.
        let defined: std::collections::HashSet<&str> =
            module.functions.iter().map(|f| f.symbol.as_str()).collect();
        for (symbol, (name, is_static)) in &module.tearoff_names {
            if !defined.contains(symbol.as_str()) {
                continue;
            }
            let idx = self.string_const_index(name.as_bytes()).unwrap_or(0);
            writeln!(
                body,
                "  call void @dartforge_register_tearoff(ptr @{}, ptr @.str.{idx}, i64 {}, i8 {})",
                symbol,
                name.len(),
                u8::from(*is_static)
            )
            .unwrap();
        }
        for symbol in &module.instantiation_entries {
            if defined.contains(symbol.as_str()) {
                writeln!(
                    body,
                    "  call void @dartforge_register_instantiation(ptr @{symbol})"
                )
                .unwrap();
            }
        }
        for (sub, sup) in &module.subtyping_edges {
            writeln!(
                body,
                "  call void @dartforge_register_subclass(i64 {sub}, i64 {sup})"
            )
            .unwrap();
        }
        body.push_str(&self.emit_method_tables());
        // The section of the static objects of this image (docs/NATIVO-ESPACO-UNIFICADO.md
        // §2.11): one startup root of the complete SDK bundle registers its
        // linked image. Standalone/program modules retain their own registration.
        // Keep the marker protocol even for delegated modules: their strings
        // still contribute to the same image section and source census.
        if self.static_objects {
            let (globals, start, end) = crate::target::image_markers();
            self.out.push_str(&globals);
            if self.static_image_registration {
                writeln!(
                    body,
                    "  call void @dartforge_register_image(ptr {start}, ptr {end})"
                )
                .unwrap();
            }
        }
        // Roots by maps: the stack maps of the image this module
        // was linked into (the registration is idempotent, per image), in its
        // format. The JIT finds and registers the map of each object.
        if self.maps && !self.maps_in_jit {
            body.push_str(Self::maps_registration_call());
        }
        // The symbolic stack trace (§13.14): the table of the image, idempotent.
        if let Some(c) = self.trace_registration_call() {
            body.push_str(&c);
            body.push_str(&self.trace_field_registration_calls());
        }
        // The program classes (and the record shapes) register their table
        // at startup. The SDK's `_StackTrace` needs it too: the runtime creates the
        // first trace directly, without going through `object_new_t`, which is the
        // lazy registration point of the other SDK classes.
        let tables: Vec<_> = module
            .method_tables
            .iter()
            .filter(|(cid, _, _)| {
                let runtime_trace = module.sdk_library
                    && module
                        .classes
                        .iter()
                        .any(|cls| cls.id == *cid && cls.name == "_StackTrace");
                let active = module
                    .closed_class_tables
                    .as_ref()
                    .is_none_or(|classes| classes.contains(cid));
                (!module.sdk_library || runtime_trace) && active
            })
            .collect();
        // Amortize the array and loop overhead for larger program startups.
        // Generated table getters are pure and return their own embedded CID.
        if !module.sdk_library && tables.len() >= crate::pruning::assembled_registration::BATCH_MIN
        {
            let array = format!("df.table_batch.{record}");
            let pointers = tables
                .iter()
                .map(|(_, symbol, _)| format!("ptr @{symbol}"))
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(
                self.out,
                "@{array} = private unnamed_addr constant [{} x ptr] [{pointers}], align 8",
                tables.len()
            )
            .unwrap();
            writeln!(
                body,
                "  call void @dartforge_register_tables(ptr @{array}, i64 {})",
                tables.len()
            )
            .unwrap();
        } else {
            for (cid, symbol, _) in &tables {
                writeln!(
                    body,
                    "  call void @dartforge_register_table(i64 {cid}, ptr @{symbol})"
                )
                .unwrap();
            }
        }
        if self.tables_at_link {
            for (_, symbol, _) in tables {
                self.annotate_external(symbol, Type::Ptr, &[]);
            }
        }
        for (k, (name, symbol)) in module.helpers.iter().enumerate() {
            writeln!(
                self.out,
                "@df.ajn.{k} = private unnamed_addr constant [{} x i8] c\"{}\"",
                name.len(),
                llvm_bytes(name)
            )
            .unwrap();
            writeln!(
                body,
                "  call void @dartforge_register_helper(ptr @df.ajn.{k}, i64 {}, ptr @{symbol})",
                name.len()
            )
            .unwrap();
        }
        for (k, name) in self.argument_names.iter().enumerate() {
            writeln!(
                self.out,
                "@df.argname.{k} = private unnamed_addr constant [{} x i8] c\"{}\"",
                name.len(),
                llvm_bytes(name)
            )
            .unwrap();
            writeln!(
                body,
                "  call void @dartforge_register_argument_name(ptr @df.argname.{k}, i64 {})",
                name.len()
            )
            .unwrap();
        }
        writeln!(self.out, "define void @{record}() {{\nb0:").unwrap();
        self.out.push_str(&body);
        self.out.push_str("  ret void\n}\n\n");
    }

    /// `dartforge_dispatch_toString` (the runtime asks for the text of a value) through the
    /// `c:toString` selector: the `toString()` of the dynamic class, of the program
    /// or of the source SDK.
    pub(super) fn emit_to_string_by_selector(&mut self) {
        let s = "c:toString";
        let name = match self.selector_names.iter().position(|x| x == s) {
            Some(j) => j,
            None => {
                self.selector_names.push(s.to_string());
                self.selector_names.len() - 1
            }
        };
        self.selector_caches += 1;
        let k = self.vector_of[&vec![0, 0]];
        let h = selector_hash(s);
        self.current_function = "dartforge_dispatch_toString".to_string();
        self.cache_in_function = 0;
        let slot = self.cache_slot();
        let descriptor = &self.vector_symbols[k];
        writeln!(
            self.out,
            "define i64 @dartforge_dispatch_toString(i64 %obj) {{\nb0:\n  %a = alloca [1 x i64]\n  \
             store i64 0, ptr %a\n  \
             %area = call ptr @df.get_area()\n  \
             %ic = getelementptr i64, ptr %area, i64 {slot}\n  \
             %f = call ptr @df.selector(ptr %ic, i64 %obj, i64 {h}, ptr @df.seln.{name}, i64 {})\n  \
             %r = call i64 %f(i64 %obj, ptr %a, ptr @{descriptor})\n  ret i64 %r\n}}\n",
            s.len()
        )
        .unwrap();
    }

    /// The method tables of the classes of the module, as data, and the public
    /// function of each one, `df.mt.<library>.<Class>`, which returns its
    /// address. The table is registered in the runtime **on the first
    /// allocation** of an object of the class (`dartforge_object_new_t`): a
    /// class the program never instantiates does not have its table reached, and the
    /// linker (`/OPT:REF`, in the production profile) removes the table, the
    /// adapters and the methods that only it reached. The classes of the
    /// runtime values are registered by the program entry.
    ///
    /// With the tables at link time (the production SDK,
    /// docs/NATIVO-PODA-DE-TABELAS.md §3.2) nothing comes out here: the program
    /// module defines the SDK tables, with only the live pairs
    /// (`pruning::assemble`).
    fn emit_method_tables(&mut self) -> String {
        if self.tables_at_link {
            return String::new();
        }
        let module = self.module;
        for (cid, symbol, methods) in &module.method_tables {
            if module
                .closed_class_tables
                .as_ref()
                .is_some_and(|active| !active.contains(cid))
            {
                continue;
            }
            let pairs = crate::pruning::table_pairs(*cid, methods);
            for (_, _, f) in &pairs {
                self.annotate_external(f, Type::Ref, &[Type::Ref, Type::Ptr, Type::Ptr]);
            }
            let items: Vec<(i64, String)> = pairs
                .iter()
                .map(|(h, _, f)| (*h, format!("@{f}")))
                .collect();
            self.out.push_str(&crate::pruning::table_line(
                &format!("@{symbol}$d = private unnamed_addr constant "),
                i64::from(*cid),
                &items,
            ));
            writeln!(
                self.out,
                "define ptr @{symbol}() {{\nb0:\n  ret ptr @{symbol}$d\n}}"
            )
            .unwrap();
        }
        String::new()
    }
    /// `define` of a function that may appear in more than one module
    /// (tear-off entries, canonical constants): `linkonce_odr` in its own
    /// `comdat` — the linker keeps one. The other symbols do not
    /// change.
    pub(super) fn linkage_of(&mut self, symbol: &str) -> (&'static str, String) {
        if !self.module.sdk_mode {
            return ("", String::new());
        }
        let shared = if self.module.arc_memory {
            self.module.arc_shared_functions.contains(symbol)
        } else {
            symbol.ends_with("$tear")
                || symbol.ends_with("$tt")
                || symbol.ends_with("$tearm")
                || symbol.starts_with("dfc.")
        };
        if !shared {
            return ("", String::new());
        }
        if !crate::target::has_comdat() {
            // Mach-O: the `linkonce_odr` definition is already weak, without a `comdat`.
            return ("linkonce_odr ", String::new());
        }
        self.comdats.push(symbol.to_string());
        ("linkonce_odr ", format!(" comdat($\"{symbol}\")"))
    }

    /// Declarations of what the module uses and another module defines (P5c): Dart
    /// functions called directly, closure entries, runtime natives
    /// outside the extern table. Without the source SDK the module is the whole
    /// program, and nothing comes out here.
    /// The generic Owned thunks (`optimize::arc::natives`): the base runs in a
    /// native scope (the tokens that Dart ports return stay in it), and the
    /// result gets a code owner before the scope closes; with a
    /// pending exception, null without an owner — the same protocol as
    /// `owner_em_escopo_nativo` in the runtime.
    fn emit_owned_thunks(&mut self) {
        // A virtual token has no runtime owner to create.
        let retain_line = if self.module.arc_virtual_tokens {
            String::new()
        } else {
            format!("  call void @{}(i64 %r)\n", self.arc_retain_target())
        };
        let thunks = std::mem::take(&mut self.owned_thunks);
        for (name, (base, types)) in thunks {
            let params: Vec<String> = types
                .iter()
                .enumerate()
                .map(|(i, t)| format!("{} %a{i}", t.llvm_ir()))
                .collect();
            let params = params.join(", ");
            let _ = write!(
                self.out,
                "define internal i64 @{name}({params}) {{\n\
                 entrada:\n\
                 \x20 %e = call i64 @dartforge_arc_native_scope_open_v1()\n\
                 \x20 %r = call i64 @{base}({params})\n\
                 \x20 %p = call i8 @dartforge_exception_pending()\n\
                 \x20 %z = icmp ne i8 %p, 0\n\
                 \x20 br i1 %z, label %fim, label %ok\n\
                 ok:\n\
                 {retain_line}\
                 \x20 br label %fim\n\
                 fim:\n\
                 \x20 %res = phi i64 [ 0, %entrada ], [ %r, %ok ]\n\
                 \x20 call void @dartforge_arc_native_scope_close_v1(i64 %e)\n\
                 \x20 ret i64 %res\n\
                 }}\n"
            );
        }
    }

    /// The native scope thunks (`optimize::arc::natives`): the base runs
    /// in a native scope, and the result (scalar, borrowed or none) and the
    /// pending state stay as the base left them.
    fn emit_scope_thunks(&mut self) {
        let thunks = std::mem::take(&mut self.scope_thunks);
        for (name, (base, types, ret)) in thunks {
            let params: Vec<String> = types
                .iter()
                .enumerate()
                .map(|(i, t)| format!("{} %a{i}", t.llvm_ir()))
                .collect();
            let params = params.join(", ");
            let r = ret.llvm_ir();
            let (call, ret_val) = if ret == Type::Void {
                (
                    format!("call void @{base}({params})"),
                    "ret void".to_string(),
                )
            } else {
                (
                    format!("%r = call {r} @{base}({params})"),
                    format!("ret {r} %r"),
                )
            };
            let _ = write!(
                self.out,
                "define internal {r} @{name}({params}) {{\n\
                 entrada:\n\
                 \x20 %e = call i64 @dartforge_arc_native_scope_open_v1()\n\
                 \x20 {call}\n\
                 \x20 call void @dartforge_arc_native_scope_close_v1(i64 %e)\n\
                 \x20 {ret_val}\n\
                 }}\n"
            );
        }
    }

    /// A call to a JavaScript snippet (`crate::js_interop`): the import, with
    /// each Dart value converted to its type at the boundary and the result
    /// back to the HIR's (`Kind`).
    pub(super) fn emit_snippet_call(
        &mut self,
        v: u32,
        name: &str,
        args: &[(Operand, Type)],
        ret_ty: Type,
    ) {
        use crate::js_interop::{Kind, Snippet};
        let snippet = Snippet::from_symbol(name)
            .unwrap_or_else(|| panic!("compiler bug: malformed snippet {name:.60}"));
        assert_eq!(
            snippet.params.len(),
            args.len(),
            "compiler bug: snippet {} with {} arguments",
            snippet.signature(),
            args.len()
        );
        self.js_snippets = true;
        self.externals
            .entry(name.to_string())
            .or_insert_with(|| snippet.declaration());
        let mut parts = Vec::with_capacity(args.len());
        for (i, ((op, ty), kind)) in args.iter().zip(&snippet.params).enumerate() {
            let value = self.coerce(op, *ty);
            let part = match (kind, ty) {
                (Kind::Number, Type::F64) => format!("double {value}"),
                (Kind::Integer, Type::I64) => {
                    writeln!(self.out, "  %js{v}.{i} = sitofp i64 {value} to double").unwrap();
                    format!("double %js{v}.{i}")
                }
                (Kind::Int64, Type::I64) => format!("i64 {value}"),
                (Kind::Value | Kind::Text, Type::I64) => {
                    writeln!(self.out, "  %js{v}.{i} = trunc i64 {value} to i32").unwrap();
                    format!("i32 %js{v}.{i}")
                }
                (Kind::Boolean | Kind::JsBoolean, Type::I8) => {
                    writeln!(self.out, "  %js{v}.{i} = zext i8 {value} to i32").unwrap();
                    format!("i32 %js{v}.{i}")
                }
                _ => panic!("compiler bug: snippet argument {i} of kind {kind:?} as {ty:?}"),
            };
            parts.push(part);
        }
        let call = format!(
            "call {} @{name}({})",
            snippet.ret.wasm_result(),
            parts.join(", ")
        );
        match (snippet.ret, ret_ty) {
            (Kind::Void, Type::Void) => writeln!(self.out, "  {call}").unwrap(),
            (Kind::Number, Type::F64) | (Kind::Int64, Type::I64) => {
                writeln!(self.out, "  %v{v} = {call}").unwrap()
            }
            (Kind::Value | Kind::Text | Kind::Integer, Type::I64) => {
                writeln!(self.out, "  %js{v}.r = {call}").unwrap();
                writeln!(self.out, "  %v{v} = sext i32 %js{v}.r to i64").unwrap();
            }
            (Kind::Boolean, Type::I8) => {
                writeln!(self.out, "  %js{v}.r = {call}").unwrap();
                writeln!(self.out, "  %js{v}.b = icmp ne i32 %js{v}.r, 0").unwrap();
                writeln!(self.out, "  %v{v} = zext i1 %js{v}.b to i8").unwrap();
            }
            (kind, ty) => panic!("compiler bug: snippet result of kind {kind:?} as {ty:?}"),
        }
    }

    /// The runtime entries the adapter calls (`crate::js_interop::EXPORTS`) in
    /// a module that calls snippets: weak definitions, so the copies of several
    /// objects become one export each, kept through LTO by `@llvm.used`
    /// (nothing in the module calls them).
    fn emit_js_exports(&mut self) {
        if !self.js_snippets {
            return;
        }
        let mut used = Vec::new();
        for (k, (name, ret, params)) in crate::js_interop::EXPORTS.iter().enumerate() {
            let formals: Vec<String> = params
                .iter()
                .enumerate()
                .map(|(i, t)| format!("{t} %a{i}"))
                .collect();
            let actuals = formals.join(", ");
            // `dartforge_js_throw` is in the runtime's table (`externs.rs`).
            if *name != crate::js_interop::THROW_EXPORT {
                writeln!(self.out, "declare {ret} @{name}({})", params.join(", ")).unwrap();
            }
            let call = format!("call {ret} @{name}({actuals})");
            let body = if *ret == "void" {
                format!("  {call}\n  ret void")
            } else {
                format!("  %r = {call}\n  ret {ret} %r")
            };
            writeln!(
                self.out,
                "define weak {ret} @df.js.export.{k}({}) \"wasm-export-name\"=\"{name}\" {{\n{body}\n}}",
                formals.join(", ")
            )
            .unwrap();
            used.push(format!("ptr @df.js.export.{k}"));
        }
        writeln!(
            self.out,
            "@llvm.used = appending global [{} x ptr] [{}], section \"llvm.metadata\"",
            used.len(),
            used.join(", ")
        )
        .unwrap();
    }

    pub(super) fn emit_external_declarations(&mut self) {
        self.emit_js_exports();
        self.emit_owned_thunks();
        self.emit_scope_thunks();
        self.emit_stack_trace_names();
        // `DARTFORGE_EFFECTS=check`: the names of the checked externs.
        for (k, s) in std::mem::take(&mut self.checked_externs).iter().enumerate() {
            writeln!(
                self.out,
                "@df.efn.{k} = private unnamed_addr constant [{} x i8] c\"{}\"",
                s.len(),
                llvm_bytes(s)
            )
            .unwrap();
        }
        for c in std::mem::take(&mut self.comdats) {
            writeln!(self.out, "$\"{c}\" = comdat any").unwrap();
        }
        let mut defined: std::collections::HashSet<&str> = self
            .module
            .functions
            .iter()
            .map(|f| f.symbol.as_str())
            .collect();
        // The functions of the method tables of the module (`emit_method_tables`).
        // With the tables at link time, they belong to the program module.
        if !self.tables_at_link {
            defined.extend(self.module.method_tables.iter().map(|(_, s, _)| s.as_str()));
        }
        let runtime_declared: std::collections::HashSet<&str> = super::externs::EXTERNS
            .iter()
            .filter_map(|e| {
                let i = e.decl.find('@')? + 1;
                let f = e.decl[i..].find('(')? + i;
                Some(&e.decl[i..f])
            })
            .collect();
        let mut exit = std::collections::BTreeMap::new();
        for (symbol, decl) in &self.externals {
            if defined.contains(symbol.as_str()) || runtime_declared.contains(symbol.as_str()) {
                continue;
            }
            // What the emitter itself writes (`dartforge_dispatch_toString`,
            // `dartforge_entry`, the registrations) is not external.
            if (symbol.starts_with("dartforge_") && !symbol.starts_with("dartforge_native_"))
                || symbol == "df.register.program"
            {
                continue;
            }
            exit.insert(symbol.clone(), decl.clone());
        }
        if exit.is_empty() {
            return;
        }
        self.out
            .push_str("; Defined in another module (source SDK) or in the runtime\n");
        for (symbol, d) in exit {
            self.out.push_str(&d);
            if self.maps {
                let effects = super::externs::effects_of(&symbol);
                if (!effects.allocates && !effects.calls_dart)
                    || crate::target::sabotaged_leaf(&symbol)
                {
                    // Match the effects used by root placement, including
                    // native declarations absent from the fixed extern table.
                    self.out.push_str(" \"gc-leaf-function\"");
                }
            }
            self.out.push('\n');
        }
        self.out.push('\n');
    }

    /// Records the signature of a call to a symbol, to declare it if
    /// it is not defined in the module.
    pub(super) fn annotate_external(&mut self, symbol: &str, ret: Type, args: &[Type]) {
        if self.externals.contains_key(symbol) {
            return;
        }
        let a: Vec<&str> = args.iter().map(|t| t.llvm_ir()).collect();
        self.externals.insert(
            symbol.to_string(),
            format!("declare {} @{symbol}({})", ret.llvm_ir(), a.join(", ")),
        );
    }
}
