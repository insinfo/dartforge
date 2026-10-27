//! Body emitter: scopes, local types, rti environment and statements.

use crate::ctx::{Ctx, Member, MemberKind};
use crate::js::{self, Js, P_ASSIGN, P_COND, P_UNARY, Writer};
use crate::module::ModState;
use crate::ty::{ParameterTypes, Ty};
use dartforge_elements::model::{ClassId, Element, FunctionKind, LibraryId, UnitId};
use dartforge_frontend::ast::{self, Ast, ExprId, FunctionBody, StmtId, StmtKind};
use dartforge_intern::SymbolId;
use std::collections::{HashMap, HashSet};

/// JS parameters, prologue and declaration-ordered (kind, Dart name, JS name).
///
/// ```
/// use dartforge_emit_js::body::ParameterDeclaration;
/// let declaration: ParameterDeclaration = (String::new(), String::new(), vec![]);
/// assert!(declaration.2.is_empty());
/// ```
pub type ParameterDeclaration = (String, String, Vec<(ast::ParameterKind, String, String)>);

/// Local variable in scope.
#[derive(Clone, Debug)]
pub struct Local {
    pub js: String,
    pub ty: Ty,
    /// `late x = init`: initializer evaluated on the first read.
    pub lazy_init: Option<String>,
    /// Non-nullable `late x;`: a read checks initialization.
    pub late_check: bool,
    /// `late final x;`: a single assignment only.
    pub late_final: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AsyncKind {
    None,
    Async,
    AsyncStar,
    SyncStar,
}

/// State saved on entering a nested function body.
pub struct SavedFn {
    fresh_lists: HashMap<ExprId, crate::fresh_list_append::FreshList>,
    temps: Vec<String>,
    temp_counter: u32,
    async_kind: AsyncKind,
    ret_ty: Ty,
    returns: Vec<Ty>,
    w: Writer,
    label_counter: u32,
    fn_type_params: Vec<(u32, String)>,
    is_static: bool,
    switch_labels: Vec<Option<String>>,
}

pub struct FnEmitter<'m, 'a> {
    pub ctx: &'m Ctx<'a>,
    pub m: &'m ModState,
    pub unit: UnitId,
    pub lib: LibraryId,
    pub class: Option<ClassId>,
    pub is_static: bool,
    pub scopes: Vec<HashMap<SymbolId, Local>>,
    /// Function type parameters in scope (innermost first): (id, JS name).
    pub fn_type_params: Vec<(u32, String)>,
    pub temps: Vec<String>,
    pub temp_counter: u32,
    pub async_kind: AsyncKind,
    pub ret_ty: Ty,
    /// The expressions inside a deferred access the outer one already checked
    /// (`deferred.rs`).
    pub deferred_checked: std::collections::HashSet<ExprId>,
    /// The expressions whose context only guides inference: the coercions
    /// (implicit `call` tear-off, downcast) apply to the enclosing expression
    /// (`_shouldSkipImplicitCallReferenceDueToForm`).
    pub uncoerced: std::collections::HashSet<ExprId>,
    /// The next [`Self::emit_method_call`] calls a member through an implicit
    /// `this` (`m(arg)`): the identifier.
    pub implicit_this_call: Option<ExprId>,
    /// The out-of-scope type parameters whose bounds [`Self::recipe`] is
    /// expanding (an F-bound must not recurse forever).
    pub expanding_bounds: std::cell::RefCell<Vec<u32>>,
    /// Generic constructor tear-offs made through a type alias, by their
    /// code: an implicit instantiation of one is the alias instantiated
    /// (`Bounded.new` in an `C<int> Function(int)` context is `C<int>.new`).
    pub alias_tearoffs:
        std::collections::HashMap<String, (dartforge_elements::model::TypedefId, String)>,
    pub returns: Vec<Ty>,
    pub w: Writer,
    pub label_counter: u32,
    /// Stack of cascade targets: (js, type).
    pub cascade: Vec<(String, Ty)>,
    /// JS labels of `switch` for an inner `continue` etc.
    pub switch_labels: Vec<Option<String>>,
    /// Dart → JS labels.
    pub labels: Vec<(String, String)>,
    /// In signature mode (`_ti` as the class environment).
    pub sig_mode: bool,
    pub unique: u32,
    pub in_constructor: bool,
    pub factory_ti: bool,
    pub is_closure_body: bool,
    pub rethrow_var: Vec<String>,
    pub pending_promotions: Vec<(SymbolId, Ty)>,
    /// Promotions valid when the condition is false (`x is! T`).
    pub negated_promotions: Vec<(SymbolId, Ty)>,
    pub pending_prefix: Vec<String>,
    pub extension_this: Option<Ty>,
    pub current_extension: Option<dartforge_elements::model::ExtensionId>,
    /// Extension type (erased, not interop) that owns the member being emitted:
    /// its statics are reachable without qualification (`crate::extension_type`).
    pub extension_type: Option<ClassId>,
    /// Explicitly applied extension (`Ext(x).m`), consumed by the next access.
    pub forced_ext: Option<dartforge_elements::model::ExtensionId>,
    /// The function expression assigned to a promoted local: its common type
    /// was inferred with the promoted type as context (the analyzer's rule),
    /// but at run time the context is the declared type (the CFE's).
    pub declared_context_closure: Option<ExprId>,
    /// [`Self::match_type`] binds a parameter matched against `Never` (the
    /// context of an implicit instantiation, where `Never` is a solution).
    pub match_never: std::cell::Cell<bool>,
    /// JS names of the parameters declared in this function
    /// ([`FnEmitter::declare`] does not reuse them).
    pub param_names: Vec<String>,
    /// Null-aware receivers of the compound assignment being emitted
    /// (`FnEmitter::emit_compound`).
    pub compound_guards: Vec<crate::expr::Guard>,
    /// Assignment pattern: the variables already exist.
    pub pattern_assign: bool,
    /// Inside a constant expression (implicit instantiations become `dart.const`).
    pub in_const: bool,
    /// Arguments of a JS interop call: functions go through
    /// `dart.assertInterop` (except a direct `allowInterop(...)`).
    pub interop_args: bool,
    /// `case` labels reachable by `continue`: (Dart name, JS label of the
    /// loop, the assignment of the state variable that selects the case).
    pub case_labels: Vec<(String, String, String)>,
    /// What an unlabeled `continue` leaves, innermost last: a loop (`None`)
    /// or the `while (true)` of a `switch` with case labels (`Some((label,
    /// state variable))`), which the `continue` must leave to reach the
    /// enclosing loop; the flag says such a `continue` was emitted.
    pub continue_scopes: Vec<Option<(String, String, bool)>>,
    /// The body is not a method of a JavaScript class (a constructor, emitted
    /// as `C.new = function() {…}`): `super` is not valid there, and
    /// `super.x` goes through the superclass's prototype with `this` as the
    /// receiver.
    pub explicit_super: bool,
    /// The `_` symbol when the library has wildcards (Dart 3.7): a local,
    /// parameter or type parameter with that name does not bind a name.
    pub wildcard: Option<SymbolId>,
    /// JS name of the wildcard `this._`/`super._` parameters, keyed by the start
    /// of the name in the source: the constructor still uses them to initialize
    /// the field and to forward to the super constructor.
    pub wildcard_params: HashMap<usize, String>,
    /// The library has dot shorthands (Dart 3.10).
    pub shorthands: bool,
    /// Context (*shorthand context*) of each dot shorthand at the root of a
    /// selector chain being emitted: registered by the outermost node of the
    /// chain, and undone when it finishes.
    pub shorthand_contexts: HashMap<ExprId, Option<Ty>>,
    /// Depth of speculative emission (`type_of`): language errors count only
    /// outside it.
    pub speculating: u32,
    /// Type parameters of local generic functions in scope, from the `TypeTable`
    /// (the ones the shared inference uses) to the `Ty::Param` with its own id
    /// that the emitter created ([`Ctx::fresh_param`]): translates the shared
    /// types that mention them ([`FnEmitter::common_ty_in_scope`]).
    pub shared_params: HashMap<u32, Ty>,
    /// Emitting an argument of `JS()` in SDK mode (`crate::own_sdk`): a type
    /// literal comes out as a raw rti and a static *tear-off* without `dart.fn`.
    pub in_foreign_js: bool,
    /// Proven raw selectors only inside the guarded branch of a typed read loop.
    pub(crate) typed_loop_raw: HashMap<ExprId, Ty>,
    /// Certified SDK modulo-result comparison; no class-wide numeric assumptions.
    pub(crate) sdk_number_comparison: Option<ExprId>,
    /// Confined fresh-list append sites certified in this function body.
    pub(crate) fresh_list_append: HashMap<ExprId, crate::fresh_list_append::FreshList>,
    /// Modulo nodes certified in the current unescaped nonnegative counter.
    pub(crate) counter_modulo: HashSet<ExprId>,
}

impl<'m, 'a> FnEmitter<'m, 'a> {
    /// `var (a, b) = e;` / `final [x, y] = e;`: the value in a temporary, the
    /// pattern's variables declared and the pattern test (irrefutable in Dart;
    /// the test only remains where the static type is not enough).
    pub(crate) fn emit_pattern_variables(&mut self, pattern: ast::PatternId, value: ExprId) {
        let (vjs, vty) = self.emit_expr(value, None);
        let t = self.temp();
        crate::emit_line!(self.w, "{t} = {};", vjs.code);
        let mut binds = Vec::new();
        let cond = self.pattern_cond(pattern, &t, &vty, &mut binds, true);
        for (_, _, jsn) in &binds {
            crate::emit_line!(self.w, "let {jsn} = null;");
        }
        if cond != "true" {
            crate::emit_line!(
                self.w,
                "if (!({cond})) dart.throw(new core.StateError.new(\"Pattern matching error\"));"
            );
            self.m.use_sdk("core");
        }
    }

    pub fn new(
        ctx: &'m Ctx<'a>,
        m: &'m ModState,
        unit: UnitId,
        class: Option<ClassId>,
        is_static: bool,
    ) -> Self {
        let lib = ctx.program.unit(unit).library;
        FnEmitter {
            ctx,
            m,
            unit,
            lib,
            class,
            is_static,
            scopes: vec![HashMap::new()],
            fn_type_params: Vec::new(),
            temps: Vec::new(),
            temp_counter: 0,
            async_kind: AsyncKind::None,
            ret_ty: Ty::Dynamic,
            deferred_checked: std::collections::HashSet::new(),
            uncoerced: std::collections::HashSet::new(),
            implicit_this_call: None,
            expanding_bounds: std::cell::RefCell::new(Vec::new()),
            alias_tearoffs: std::collections::HashMap::new(),
            returns: Vec::new(),
            w: Writer::default(),
            label_counter: 0,
            cascade: Vec::new(),
            switch_labels: Vec::new(),
            labels: Vec::new(),
            sig_mode: false,
            unique: 0,
            in_constructor: false,
            factory_ti: false,
            is_closure_body: false,
            rethrow_var: Vec::new(),
            pending_promotions: Vec::new(),
            negated_promotions: Vec::new(),
            pending_prefix: Vec::new(),
            extension_this: None,
            current_extension: None,
            extension_type: None,
            forced_ext: None,
            declared_context_closure: None,
            match_never: std::cell::Cell::new(false),
            param_names: Vec::new(),
            compound_guards: Vec::new(),
            pattern_assign: false,
            in_const: false,
            interop_args: false,
            case_labels: Vec::new(),
            continue_scopes: Vec::new(),
            explicit_super: false,
            wildcard: ctx
                .program
                .library(lib)
                .features
                .has(dartforge_frontend::Feature::WildcardVariables)
                .then(|| ctx.sym("_"))
                .flatten(),
            wildcard_params: HashMap::new(),
            shorthands: ctx
                .program
                .library(lib)
                .features
                .has(dartforge_frontend::Feature::DotShorthands),
            shorthand_contexts: HashMap::new(),
            speculating: 0,
            shared_params: HashMap::new(),
            in_foreign_js: false,
            typed_loop_raw: HashMap::new(),
            sdk_number_comparison: None,
            fresh_list_append: HashMap::new(),
            counter_modulo: HashSet::new(),
        }
    }

    pub fn ast(&self) -> &'a Ast {
        &self.ctx.program.unit(self.unit).ast
    }
    pub fn source(&self) -> &'a str {
        &self.ctx.program.unit(self.unit).source
    }
    pub fn expr(&self, e: ExprId) -> &'a ast::Expr {
        self.ast().expr(e)
    }
    pub fn name(&self, s: SymbolId) -> &'a str {
        self.ctx.interner.resolve(s)
    }
    pub fn text(&self, span: dartforge_diagnostics::Span) -> &'a str {
        &self.source()[span.start..span.end]
    }

    // -----------------------------------------------------------------------
    // Scopes and temporaries
    // -----------------------------------------------------------------------

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }
    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }
    /// Declares a local and returns the JS name (renaming to avoid shadowing
    /// between sibling scopes: JS `let` is block scoped too).
    pub fn declare(&mut self, sym: SymbolId, ty: Ty) -> String {
        if self.wildcard == Some(sym) {
            // Wildcard (Dart 3.7): a new JS name that nobody reads, and nothing
            // in scope — reading `_` still finds whatever is outside.
            return self.wildcard_name();
        }
        let base = js::ident(self.name(sym));
        let in_use = |s: &Self, n: &str| {
            s.param_names.iter().any(|p| p == n)
                || s.scopes.iter().any(|sc| sc.values().any(|l| l.js == n))
        };
        let mut js = base.clone();
        let mut n = 0;
        while in_use(self, &js) {
            n += 1;
            js = format!("{base}${n}");
        }
        self.scopes.last_mut().expect("scope").insert(
            sym,
            Local {
                js: js.clone(),
                ty,
                lazy_init: None,
                late_check: false,
                late_final: false,
            },
        );
        js
    }
    /// JS name of a wildcard: `t$wN`. The `t$` prefix is reserved for
    /// temporaries (`js::ident` appends `$` to user names that start with it) and
    /// temporaries are `t$` followed by digits, so there is no collision; the
    /// counter never goes back within the emitter.
    pub fn wildcard_name(&mut self) -> String {
        let n = format!("t$w{}", self.unique);
        self.unique += 1;
        n
    }

    /// JS name of a type parameter of a generic function: `<_>` in a library
    /// with wildcards does not bind a name, and two of them cannot share the
    /// same JS parameter name.
    pub fn type_param_js_name(&mut self, name: &str) -> String {
        if name == "_" && self.wildcard.is_some() {
            return self.wildcard_name();
        }
        // A method's `T` that shadows the extension's `T`: both are
        // parameters of the same JS function.
        let base = js::ident(name);
        let taken = |n: &str| self.fn_type_params.iter().any(|(_, j)| j == n);
        if !taken(&base) {
            return base;
        }
        (1..)
            .map(|i| format!("{base}${i}"))
            .find(|n| !taken(n))
            .unwrap()
    }

    /// JS name of the already declared parameter `n` (`this.x`, `super.x`): the
    /// one in scope, or the one kept for a wildcard.
    pub fn param_js_name(&self, n: ast::Name) -> String {
        if let Some(j) = self.wildcard_params.get(&n.span.start) {
            return j.clone();
        }
        self.lookup_local(n.sym)
            .map(|l| l.js.clone())
            .unwrap_or_else(|| js::ident(self.name(n.sym)))
    }

    pub fn declare_js(&mut self, sym: SymbolId, js: String, ty: Ty) {
        self.scopes.last_mut().expect("scope").insert(
            sym,
            Local {
                js,
                ty,
                lazy_init: None,
                late_check: false,
                late_final: false,
            },
        );
    }
    pub fn lookup_local(&self, sym: SymbolId) -> Option<&Local> {
        for s in self.scopes.iter().rev() {
            if let Some(l) = s.get(&sym) {
                return Some(l);
            }
        }
        None
    }
    pub fn set_local_ty(&mut self, sym: SymbolId, ty: Ty) {
        for s in self.scopes.iter_mut().rev() {
            if let Some(l) = s.get_mut(&sym) {
                l.ty = ty;
                return;
            }
        }
    }
    pub fn temp(&mut self) -> String {
        let n = format!("t${}", self.temp_counter);
        self.temp_counter += 1;
        self.temps.push(n.clone());
        n
    }
    /// Element type of `t` seen as what the current generator produces:
    /// `Iterable<E>` in a `sync*`, `Stream<E>` in an `async*` (the `E` of
    /// `yield*` and of the `yield` context). `None` outside a generator or when
    /// `t` is not of that type (`dynamic`, `Object`).
    pub fn generator_element(&self, t: &Ty) -> Option<Ty> {
        let target = match self.async_kind {
            AsyncKind::SyncStar => self.ctx.iterable_?,
            AsyncKind::AsyncStar => self.ctx.stream_?,
            _ => return None,
        };
        if t.is_dynamic() {
            return None;
        }
        self.ctx
            .as_super(&t.non_null(), target)
            .and_then(|s| s.args().first().cloned())
    }

    pub fn fresh_label(&mut self) -> String {
        let n = format!("L{}", self.label_counter);
        self.label_counter += 1;
        n
    }

    /// Enters a nested function body; returns the state to restore.
    pub fn enter_fn(&mut self, async_kind: AsyncKind, ret_ty: Ty, is_static: bool) -> SavedFn {
        let saved = SavedFn {
            fresh_lists: std::mem::take(&mut self.fresh_list_append),
            temps: std::mem::take(&mut self.temps),
            temp_counter: self.temp_counter,
            async_kind: self.async_kind,
            ret_ty: std::mem::replace(&mut self.ret_ty, ret_ty),
            returns: std::mem::take(&mut self.returns),
            w: std::mem::take(&mut self.w),
            label_counter: self.label_counter,
            fn_type_params: self.fn_type_params.clone(),
            is_static: self.is_static,
            switch_labels: std::mem::take(&mut self.switch_labels),
        };
        self.async_kind = async_kind;
        self.is_static = is_static;
        self.push_scope();
        saved
    }
    /// Leaves the nested body returning (body text with declared temps, returned types).
    pub fn exit_fn(&mut self, saved: SavedFn) -> (String, Vec<Ty>) {
        self.pop_scope();
        let mut body = String::new();
        if !self.temps.is_empty() {
            body.push_str(&format!("let {};\n", self.temps.join(", ")));
        }
        body.push_str(&self.w.out);
        let returns = std::mem::replace(&mut self.returns, saved.returns);
        self.temps = saved.temps;
        self.temp_counter = saved.temp_counter;
        self.async_kind = saved.async_kind;
        self.ret_ty = saved.ret_ty;
        self.w = saved.w;
        self.label_counter = saved.label_counter;
        self.fn_type_params = saved.fn_type_params;
        self.is_static = saved.is_static;
        self.switch_labels = saved.switch_labels;
        self.fresh_list_append = saved.fresh_lists;
        (body, returns)
    }

    /// Indents a ready-made block of text inside the current writer.
    pub fn write_block_text(&mut self, text: &str) {
        for line in text.lines() {
            self.w.line(line);
        }
    }

    // -----------------------------------------------------------------------
    // References to libraries and elements
    // -----------------------------------------------------------------------

    /// JS variable of a library's namespace, registering the import.
    pub fn lib_var(&self, lib: LibraryId) -> String {
        self.m.lib_var(self.ctx, lib)
    }

    pub fn dartx(&self, name: &str) -> String {
        self.m.dartx(name)
    }

    /// Private symbol `dart.privateName(lib, name)`.
    pub fn private_sym(&self, lib: LibraryId, name: &str) -> String {
        self.m.private_sym(self.ctx, lib, name)
    }

    /// The key of a dynamic access (`dart.dload`, `dart.dput`, `dart.dsend`)
    /// to the Dart member `name`: a private name is the current library's.
    pub fn dynamic_member_key(&self, name: &str) -> String {
        if name.starts_with('_') {
            self.private_sym(self.lib, name)
        } else {
            js::string_literal(&js_member_name(name))
        }
    }

    /// DDC's `_assertInterop`: a function passed to JS without a direct
    /// `allowInterop` becomes `dart.assertInterop(f)` (fails at run time if not wrapped).
    pub fn assert_interop(&self, js: Js, ty: &Ty) -> Js {
        let is_fn = matches!(ty, Ty::Fn { .. })
            || ty.class().is_some_and(|c| Some(c) == self.ctx.function_);
        if is_fn && !js.code.starts_with("js_util.allowInterop(") {
            return Js::prim(format!("dart.assertInterop({})", js.code));
        }
        js
    }

    /// `dart.jsInteropNullCheck(e)` when the interop member is non-nullable.
    pub fn js_null_check(&self, js: Js, mk: &MemberKind) -> Js {
        if self.ctx.js_null_checkable(mk) {
            return Js::prim(format!("dart.jsInteropNullCheck({})", js.code));
        }
        js
    }

    /// JS assignment target for a non-local name (field via `this`, top level).
    pub fn emit_assign_to_name(&mut self, sym: SymbolId) -> (String, Ty) {
        let n = self.name(sym).to_string();
        match self.resolve_ident(sym) {
            crate::expr::IdentTarget::ThisMember(m) => {
                let this_ty = self.ctx.this_ty(m.class);
                let access = self.member_access(&this_ty, &n, true);
                (format!("this{access}"), self.ctx.member_ty(&m))
            }
            crate::expr::IdentTarget::Element(Element::Function(fid))
                if self.ctx.is_js_member(fid) =>
            {
                let f = self.ctx.program.function(fid);
                (
                    self.ctx
                        .js_static_ref(None, f.library, &MemberKind::Setter(fid), &n),
                    Ty::Dynamic,
                )
            }
            crate::expr::IdentTarget::Element(Element::Variable(vid)) => {
                let var = self.ctx.program.variable(vid);
                (
                    format!("{}{}", self.lib_var(var.library), js::prop_access(&n)),
                    self.ctx.var_ty(vid),
                )
            }
            _ => (js::ident(&n), Ty::Dynamic),
        }
    }

    /// JS property name of an instance member (getter/method/field).
    /// Returns `.name`, `[$name]`, `[_private]` or `['+']`.
    pub fn member_access(&self, recv_ty: &Ty, name: &str, setter: bool) -> String {
        // SDK mode: a member with `@JSExportName('Symbol.iterator')` is accessed
        // by the exported name, as it is declared.
        if self.ctx.sdk.is_some() {
            if let Some(m) = self.ctx.lookup_member(recv_ty, name, setter) {
                if let MemberKind::Method(f) | MemberKind::Getter(f) | MemberKind::Setter(f) =
                    m.kind
                {
                    if let Some(x) = crate::own_sdk::export_name(self.ctx, f) {
                        return if x.contains('.') {
                            format!("[{x}]")
                        } else {
                            js::prop_access(x)
                        };
                    }
                }
            }
        }
        if name.starts_with('_') {
            // Private: scope of the library that declares it.
            let lib = self
                .lookup_member_here(recv_ty, name, setter)
                .map(|m| self.ctx.lib_of_class(m.class))
                .unwrap_or(self.lib);
            return format!("[{}]", self.private_sym(lib, name));
        }
        let js_name = js_member_name(name);
        if self.ctx.is_ext_member(recv_ty, name, setter) {
            return format!("[{}]", self.dartx(&js_name));
        }
        js::prop_access(&js_name)
    }

    /// Member access by the JS declaration name (inside the class itself).
    pub fn decl_member_key(&self, class: ClassId, name: &str) -> String {
        if name.starts_with('_') {
            let lib = self.ctx.lib_of_class(class);
            return format!("[{}]", self.private_sym(lib, name));
        }
        // SDK mode: a member of a native class is declared by the `dartx` symbol.
        if crate::own_sdk::is_native_class(self.ctx, class) {
            return crate::own_sdk::native_key(self.m, name);
        }
        js::prop_key(&js_member_name(name))
    }

    /// JS reference to a class.
    pub fn class_ref(&self, c: ClassId) -> String {
        let lib = self.ctx.lib_of_class(c);
        format!("{}.{}", self.lib_var(lib), self.ctx.class_name(c))
    }

    /// Reference to a top-level element.
    pub fn element_ref(&self, e: Element) -> Option<(String, Ty)> {
        match e {
            Element::Class(c) => Some((self.class_ref(c), self.ctx.t_type())),
            Element::Function(fid) => {
                let f = self.ctx.program.function(fid);
                let lib = f.library;
                let n = self.name(f.name);
                let ty = self.ctx.fn_ty(fid);
                if self.ctx.is_js_member(fid) {
                    let mk = match f.kind {
                        FunctionKind::Getter => MemberKind::Getter(fid),
                        FunctionKind::Setter => MemberKind::Setter(fid),
                        _ => MemberKind::Method(fid),
                    };
                    return Some((self.ctx.js_static_ref(None, lib, &mk, n), ty));
                }
                // SDK mode: `@JSExportName('throw')` on `throw_`.
                if let Some(x) = crate::own_sdk::export_name(self.ctx, fid) {
                    return Some((format!("{}{}", self.lib_var(lib), js::prop_access(x)), ty));
                }
                let js = format!(
                    "{}{}",
                    self.lib_var(lib),
                    js::prop_access(&top_level_name(n))
                );
                Some((js, ty))
            }
            Element::Variable(vid) => {
                let v = self.ctx.program.variable(vid);
                let n = self.name(v.name);
                if self.ctx.is_js_var(vid) {
                    return Some((
                        self.ctx
                            .js_static_ref(None, v.library, &MemberKind::Field(vid), n),
                        self.ctx.var_ty(vid),
                    ));
                }
                if let Some(x) = crate::own_sdk::export_name_var(self.ctx, vid) {
                    return Some((
                        format!("{}{}", self.lib_var(v.library), js::prop_access(x)),
                        self.ctx.var_ty(vid),
                    ));
                }
                let js = format!(
                    "{}{}",
                    self.lib_var(v.library),
                    js::prop_access(&top_level_name(n))
                );
                Some((js, self.ctx.var_ty(vid)))
            }
            Element::Typedef(_) | Element::Extension(_) | Element::Prefix(..) => None,
        }
    }

    // -----------------------------------------------------------------------
    // Types: resolution of annotations and the rti environment
    // -----------------------------------------------------------------------

    /// Type parameters in scope by name (class + functions).
    pub fn type_param_by_name(&self, name: &str) -> Option<Ty> {
        for (id, js) in &self.fn_type_params {
            let pname = self
                .ctx
                .param_names
                .borrow()
                .get(id)
                .cloned()
                .unwrap_or_default();
            if pname == name {
                let _ = js;
                return Some(Ty::Param {
                    id: *id,
                    name: name.to_string(),
                    nullable: false,
                });
            }
        }
        if let Some(c) = self.class {
            for p in &self.ctx.class_params[c.0 as usize] {
                if p.name == name {
                    return Some(Ty::Param {
                        id: p.id,
                        name: name.to_string(),
                        nullable: false,
                    });
                }
            }
        }
        None
    }

    /// Resolves a type annotation of the AST to `Ty`.
    pub fn resolve_type(&self, t: ast::TypeId) -> Ty {
        let node = self.ast().ty(t);
        let nullable = node.nullable;
        match &node.kind {
            ast::TypeKind::Void => Ty::Void,
            ast::TypeKind::Named { name, args } => {
                let args: Vec<Ty> = args.iter().map(|a| self.resolve_type(*a)).collect();
                let (lib, sym) = if name.len() == 2 {
                    let prefix = name[0].sym;
                    (Some(prefix), name[1].sym)
                } else {
                    (None, name[0].sym)
                };
                let n = self.name(sym);
                if lib.is_none() {
                    match n {
                        "dynamic" => return Ty::Dynamic,
                        "void" => return Ty::Void,
                        "Never" => return if nullable { Ty::Null } else { Ty::Never },
                        "Null" => return Ty::Null,
                        _ => {}
                    }
                    if let Some(p) = self.type_param_by_name(n) {
                        return p.with_nullable(nullable);
                    }
                }
                let binding = match lib {
                    Some(p) => self.ctx.program.lookup_prefixed_in_unit(self.unit, p, sym),
                    None => self.ctx.program.lookup_in_unit(self.unit, sym),
                };
                match binding.and_then(|b| b.getter) {
                    Some(Element::Class(c)) => {
                        let class = self.ctx.program.class(c);
                        if class.kind == dartforge_elements::model::ClassKind::ExtensionType
                            && !self.ctx.interop_ext_types.contains(&c)
                        {
                            if let Some(rep) = class.representation {
                                let mut map = HashMap::new();
                                for (p, a) in
                                    self.ctx.class_params[c.0 as usize].iter().zip(args.iter())
                                {
                                    map.insert(p.id, a.clone());
                                }
                                let t = self.ctx.var_ty(rep).subst_prop(&map);
                                return if nullable { t.with_nullable(true) } else { t };
                            }
                        }
                        if Some(c) == self.ctx.future_or {
                            let arg = args.into_iter().next().unwrap_or(Ty::Dynamic);
                            return Ty::FutureOr {
                                arg: Box::new(arg),
                                nullable,
                            };
                        }
                        let nparams = self.ctx.class_params[c.0 as usize].len();
                        let mut args = args;
                        while args.len() < nparams {
                            // Raw type: instantiation to bounds. The implicit
                            // bound (`class C<T>`) gives `dynamic`, not the
                            // `Object?` the outline records; a written one gives
                            // itself (`instanciar_para_limites` of the shared
                            // inference).
                            let p = &self.ctx.class_params[c.0 as usize][args.len()];
                            let explicit = self
                                .ctx
                                .table
                                .param(dartforge_types::table::TypeParamId(p.id))
                                .explicit;
                            // A bound that references the class's own parameters
                            // (`class A<E extends A<E>>`): they count as `dynamic`
                            // in the bound (raw `A` is `A<A<dynamic>>`), otherwise
                            // the recipe references an out-of-scope parameter and
                            // the rti recurses forever.
                            let bound = if p.bound.mentions_params() {
                                let map: HashMap<u32, Ty> = self.ctx.class_params[c.0 as usize]
                                    .iter()
                                    .map(|q| (q.id, Ty::Dynamic))
                                    .collect();
                                p.bound.subst(&map)
                            } else {
                                (*p.bound).clone()
                            };
                            args.push(if p.bound.is_dynamic() || !explicit {
                                Ty::Dynamic
                            } else {
                                bound
                            });
                        }
                        Ty::Iface {
                            class: c,
                            args,
                            nullable,
                        }
                    }
                    Some(Element::Typedef(tid)) => {
                        let data = &self.ctx.outline.typedefs[tid.0 as usize];
                        let mut map = HashMap::new();
                        for (p, a) in data.type_params.iter().zip(args.iter()) {
                            map.insert(p.0, a.clone());
                        }
                        let t = self.ctx.ty_of(data.target_type).subst_prop(&map);
                        if nullable { t.with_nullable(true) } else { t }
                    }
                    _ => Ty::Dynamic,
                }
            }
            ast::TypeKind::Function {
                return_type,
                type_params,
                parameters,
            } => {
                // Type parameters of the function type: new ids.
                let mut tps = Vec::new();
                let mut scoped = self.fn_type_params.clone();
                let saved_len = scoped.len();
                let _ = saved_len;
                let mut me_params: Vec<(u32, String)> = Vec::new();
                for tp in type_params.iter() {
                    let p = self.ctx.fresh_param(self.name(tp.name.sym), Ty::Dynamic);
                    me_params.push((p.id, p.name.clone()));
                    tps.push(p);
                }
                // Resolves bounds and signature with the parameters in scope.
                let this_ptr: *const FnEmitter<'m, 'a> = self;
                let _ = this_ptr;
                let sub = self.with_fn_params(&me_params);
                for (tp, p) in type_params.iter().zip(tps.iter_mut()) {
                    if let Some(b) = tp.bound {
                        let bt = sub.resolve_type(b);
                        self.ctx.param_bounds.borrow_mut().insert(p.id, bt.clone());
                        *p.bound = bt;
                    } else {
                        *p.bound = self.ctx.t_object_q();
                        self.ctx
                            .param_bounds
                            .borrow_mut()
                            .insert(p.id, self.ctx.t_object_q());
                    }
                }
                let ret = return_type
                    .map(|r| sub.resolve_type(r))
                    .unwrap_or(Ty::Dynamic);
                let (pos, opt, named) = sub.resolve_params(parameters);
                scoped.truncate(saved_len);
                Ty::Fn {
                    type_params: tps,
                    ret: Box::new(ret),
                    pos,
                    opt,
                    named,
                    nullable,
                }
            }
            ast::TypeKind::Record { positional, named } => {
                let pos = positional.iter().map(|t| self.resolve_type(*t)).collect();
                let mut named: Vec<(String, Ty)> = named
                    .iter()
                    .map(|(n, t)| (self.name(n.sym).to_string(), self.resolve_type(*t)))
                    .collect();
                named.sort_by(|a, b| a.0.cmp(&b.0));
                Ty::Record {
                    pos,
                    named,
                    nullable,
                }
            }
        }
    }

    /// Lightweight copy of the emitter with extra type parameters in scope (only for resolving types).
    fn with_fn_params(&self, extra: &[(u32, String)]) -> FnEmitter<'m, 'a> {
        let mut e = FnEmitter::new(self.ctx, self.m, self.unit, self.class, self.is_static);
        e.fn_type_params = extra.to_vec();
        e.fn_type_params.extend(self.fn_type_params.iter().cloned());
        e.scopes = self.scopes.clone();
        e
    }

    pub fn resolve_params(&self, params: &[ast::Parameter]) -> ParameterTypes {
        let mut pos = Vec::new();
        let mut opt = Vec::new();
        let mut named = Vec::new();
        for p in params {
            let t = self.param_ty(p);
            match p.kind {
                ast::ParameterKind::Required => pos.push(t),
                ast::ParameterKind::Optional => opt.push(t),
                ast::ParameterKind::Named => named.push((
                    p.external_name()
                        .map(|n| self.name(n.sym).to_string())
                        .unwrap_or_default(),
                    t,
                    p.required,
                )),
            }
        }
        named.sort_by(|a, b| a.0.cmp(&b.0));
        (pos, opt, named)
    }

    /// Type of a formal parameter (annotation, old function form, or dynamic).
    pub fn param_ty(&self, p: &ast::Parameter) -> Ty {
        if let Some(fp) = &p.function_parameters {
            let ret = p.ty.map(|t| self.resolve_type(t)).unwrap_or(Ty::Dynamic);
            let (pos, opt, named) = self.resolve_params(fp);
            return Ty::Fn {
                type_params: vec![],
                ret: Box::new(ret),
                pos,
                opt,
                named,
                nullable: false,
            };
        }
        match p.ty {
            Some(t) => self.resolve_type(t),
            None => Ty::Dynamic,
        }
    }

    // -----------------------------------------------------------------------
    // rti recipes
    // -----------------------------------------------------------------------

    /// Recipe of a type; `unbound` are the parameters of nested generic function
    /// types (`0^`), `used_fn` accumulates the function parameters used,
    /// `used_class` flags use of the class's parameters.
    fn recipe(
        &self,
        t: &Ty,
        unbound: &mut Vec<u32>,
        used_fn: &mut Vec<u32>,
        used_class: &mut bool,
    ) -> String {
        match t {
            Ty::Dynamic => "@".into(),
            Ty::Void => "~".into(),
            Ty::Never => "0&".into(),
            Ty::Null => match self.ctx.null_ {
                Some(n) => self.ctx.class_recipe(n),
                None => "core|Null".into(),
            },
            Ty::Iface {
                class,
                args,
                nullable,
            } => {
                // Interop extension type: the recipe is that of the
                // representation type (`JSArray<JSString>` →
                // `_interceptors|JSArray<core|Object?>`, `JSString` →
                // `core|String`), as DDC emits.
                if self.ctx.interop_ext_types.contains(class) {
                    if let Some(t) = self.ctx.erase_ext(*class, args) {
                        let t = if *nullable { t.with_nullable(true) } else { t };
                        return self.recipe(&t, unbound, used_fn, used_class);
                    }
                }
                let mut s = self.ctx.class_recipe(*class);
                // Declared arity, always: `raw|Caixa<@>`, never `raw|Caixa`
                // (`Ctx::args_at_arity`).
                let args = self.ctx.args_at_arity(*class, args);
                if !args.is_empty() {
                    s.push('<');
                    let parts: Vec<String> = args
                        .iter()
                        .map(|a| self.recipe(a, unbound, used_fn, used_class))
                        .collect();
                    s.push_str(&parts.join(","));
                    s.push('>');
                }
                if *nullable {
                    s.push('?');
                }
                s
            }
            Ty::FutureOr { arg, nullable } => {
                let mut s = self.recipe(arg, unbound, used_fn, used_class);
                s.push('/');
                if *nullable {
                    s.push('?');
                }
                s
            }
            Ty::Param { id, nullable, name } => {
                let q = if *nullable { "?" } else { "" };
                if let Some(i) = unbound.iter().position(|x| x == id) {
                    return format!("{i}^{q}");
                }
                if let Some(c) = self.class {
                    if self.ctx.class_params[c.0 as usize]
                        .iter()
                        .any(|p| p.id == *id)
                    {
                        *used_class = true;
                        return format!("{}.{}{}", self.ctx.class_name(c), name, q);
                    }
                }
                if self.fn_type_params.iter().any(|(pid, _)| pid == id) {
                    if !used_fn.contains(id) {
                        used_fn.push(*id);
                    }
                    // Index filled in later: a marker.
                    return format!("\u{1}{id}\u{2}{q}");
                }
                // Out-of-scope parameter: uses the bound; an F-bound
                // (`Y extends A<Y>`) mentioning a parameter being expanded
                // is `dynamic` there.
                let b = self.ctx.bound_of(*id);
                if matches!(b, Ty::Param { .. }) || self.expanding_bounds.borrow().contains(id) {
                    return format!("@{}", "");
                }
                let bt = if *nullable { b.with_nullable(true) } else { b };
                self.expanding_bounds.borrow_mut().push(*id);
                let r = self.recipe(&bt, unbound, used_fn, used_class);
                self.expanding_bounds.borrow_mut().pop();
                r
            }
            Ty::Fn {
                type_params,
                ret,
                pos,
                opt,
                named,
                nullable,
            } => {
                let saved = unbound.clone();
                let mut new_unbound: Vec<u32> = type_params.iter().map(|p| p.id).collect();
                new_unbound.extend(saved.iter().cloned());
                *unbound = new_unbound;
                let mut s = self.recipe(ret, unbound, used_fn, used_class);
                s.push('(');
                let mut parts: Vec<String> = pos
                    .iter()
                    .map(|a| self.recipe(a, unbound, used_fn, used_class))
                    .collect();
                if !opt.is_empty() {
                    let o: Vec<String> = opt
                        .iter()
                        .map(|a| self.recipe(a, unbound, used_fn, used_class))
                        .collect();
                    parts.push(format!("[{}]", o.join(",")));
                }
                if !named.is_empty() {
                    // The rti compares named parameters in name order: the order
                    // is that of the name that goes into the JS (`Ctx::named_key`).
                    let mut ns: Vec<(String, &Ty, bool)> = named
                        .iter()
                        .map(|(n, t, r)| (self.ctx.named_key(n), t, *r))
                        .collect();
                    ns.sort_by(|a, b| a.0.cmp(&b.0));
                    let n: Vec<String> = ns
                        .iter()
                        .map(|(n, t, r)| {
                            format!(
                                "{n}{}{}",
                                if *r { "!" } else { ":" },
                                self.recipe(t, unbound, used_fn, used_class)
                            )
                        })
                        .collect();
                    parts.push(format!("{{{}}}", n.join(",")));
                }
                s.push_str(&parts.join(","));
                s.push(')');
                if !type_params.is_empty() {
                    s.push('<');
                    let b: Vec<String> = type_params
                        .iter()
                        .map(|p| self.recipe(&p.bound, unbound, used_fn, used_class))
                        .collect();
                    s.push_str(&b.join(","));
                    s.push('>');
                }
                *unbound = saved;
                if *nullable {
                    s.push('?');
                }
                s
            }
            Ty::Record {
                pos,
                named,
                nullable,
            } => {
                let mut s = String::from("+");
                let names: Vec<&str> = named.iter().map(|(n, _)| n.as_str()).collect();
                s.push_str(&names.join(","));
                s.push('(');
                let mut parts: Vec<String> = pos
                    .iter()
                    .map(|a| self.recipe(a, unbound, used_fn, used_class))
                    .collect();
                parts.extend(
                    named
                        .iter()
                        .map(|(_, t)| self.recipe(t, unbound, used_fn, used_class)),
                );
                s.push_str(&parts.join(","));
                s.push(')');
                if *nullable {
                    s.push('?');
                }
                s
            }
        }
    }

    /// JS expression that evaluates to the rti of `t` in the current environment.
    pub fn rti(&self, t: &Ty) -> String {
        // A constant cannot depend on type parameters: in a constant context
        // they denote their least closure, `Never` (`const C(f)` in
        // `getC<T>()` is a `C<Never>`).
        if self.in_const && !t.has_no_params() {
            let mut ids = Vec::new();
            t.collect_params(&mut ids);
            if !ids.is_empty() {
                let map: std::collections::HashMap<u32, Ty> =
                    ids.into_iter().map(|id| (id, Ty::Never)).collect();
                return self.rti_unconst(&t.subst(&map));
            }
        }
        self.rti_unconst(t)
    }

    fn rti_unconst(&self, t: &Ty) -> String {
        // Closed type (no `Ty::Param`): the recipe does not mention the
        // environment (`used_fn`/`used_class` are only set in the `Ty::Param`
        // branch), so the expression is the same anywhere in the program and is
        // memoized in the `Ctx`. The classes used are still registered in the module.
        if t.has_no_params() {
            if let Some(s) = self.ctx.rti_memo.borrow().get(t) {
                self.register_recipe_classes(t);
                return s.clone();
            }
            let s = self.rti_calc(t);
            self.ctx.rti_memo.borrow_mut().insert(t.clone(), s.clone());
            return s;
        }
        self.rti_calc(t)
    }

    fn rti_calc(&self, t: &Ty) -> String {
        let mut unbound = Vec::new();
        let mut used_fn = Vec::new();
        let mut used_class = false;
        let raw = self.recipe(t, &mut unbound, &mut used_fn, &mut used_class);
        self.register_recipe_classes(t);
        // Orders the used function parameters in scope order (innermost first).
        let mut ordered: Vec<(u32, String)> = Vec::new();
        for (id, jsn) in &self.fn_type_params {
            if used_fn.contains(id) {
                ordered.push((*id, jsn.clone()));
            }
        }
        let class_env = if used_class {
            Some(if self.sig_mode || self.factory_ti {
                "_ti".to_string()
            } else {
                "dart_rti.instanceType(this)".to_string()
            })
        } else {
            None
        };
        let mut recipe = raw;
        if class_env.is_none() && ordered.len() == 1 {
            let (id, jsn) = &ordered[0];
            recipe = recipe.replace(&format!("\u{1}{id}\u{2}"), "0");
            if recipe == "0" {
                return jsn.clone();
            }
            return format!("{jsn}[_eval]({})", js::string_literal(&recipe));
        }
        for (i, (id, _)) in ordered.iter().enumerate() {
            recipe = recipe.replace(&format!("\u{1}{id}\u{2}"), &(i + 1).to_string());
        }
        if class_env.is_none() && ordered.is_empty() {
            return format!(
                "dart_rti._Universe.eval(dart_rti._theUniverse(), {}, true)",
                js::string_literal(&recipe)
            );
        }
        let mut env = match class_env {
            Some(e) => e,
            None => "dart_rti._Universe.eval(dart_rti._theUniverse(), \"@\", true)".to_string(),
        };
        for (_, jsn) in &ordered {
            env = format!("{env}[_bind]({jsn})");
        }
        format!("{env}[_eval]({})", js::string_literal(&recipe))
    }

    /// "Raw" recipe in class mode (parameters by index 1..n), for the rules.
    pub fn register_recipe_classes(&self, t: &Ty) {
        match t {
            Ty::Iface { class, args, .. } => {
                self.m.note_class(*class);
                for a in args {
                    self.register_recipe_classes(a);
                }
            }
            Ty::Fn {
                ret,
                pos,
                opt,
                named,
                type_params,
                ..
            } => {
                self.register_recipe_classes(ret);
                pos.iter()
                    .chain(opt.iter())
                    .for_each(|a| self.register_recipe_classes(a));
                named
                    .iter()
                    .for_each(|(_, t, _)| self.register_recipe_classes(t));
                type_params
                    .iter()
                    .for_each(|p| self.register_recipe_classes(&p.bound));
            }
            Ty::Record { pos, named, .. } => {
                pos.iter().for_each(|a| self.register_recipe_classes(a));
                named
                    .iter()
                    .for_each(|(_, t)| self.register_recipe_classes(t));
            }
            Ty::FutureOr { arg, .. } => self.register_recipe_classes(arg),
            _ => {}
        }
    }

    /// `T[_is](x)` / `typeof x == 'string'`.
    pub fn is_test(&self, value: &Js, t: &Ty) -> Js {
        if let Ty::Iface {
            class,
            args,
            nullable: false,
        } = t
        {
            if args.is_empty() {
                if Some(*class) == self.ctx.string_ {
                    return Js::new(
                        format!("typeof {} == 'string'", value.at(P_UNARY + 1)),
                        crate::js::P_EQ,
                    );
                }
                if Some(*class) == self.ctx.bool_ {
                    return Js::new(
                        format!("typeof {} == 'boolean'", value.at(P_UNARY + 1)),
                        crate::js::P_EQ,
                    );
                }
            }
        }
        if let Ty::Iface {
            class,
            nullable: true,
            ..
        } = t
        {
            if Some(*class) == self.ctx.object {
                return always_true(value);
            }
        }
        if matches!(t, Ty::Dynamic | Ty::Void) {
            return always_true(value);
        }
        Js::prim(format!("{}[_is]({})", self.rti(t), value.code))
    }

    pub fn as_cast(&self, value: &Js, t: &Ty) -> Js {
        if matches!(t, Ty::Dynamic | Ty::Void) {
            return value.clone();
        }
        if let Ty::Iface {
            class,
            nullable: true,
            ..
        } = t
        {
            if Some(*class) == self.ctx.object {
                return value.clone();
            }
        }
        Js::prim(format!("{}[_as]({})", self.rti(t), value.code))
    }

    // -----------------------------------------------------------------------
    // Function body
    // -----------------------------------------------------------------------

    /// Declares a function's parameters in the current scope and returns the
    /// list of JS parameters (`a, b = 1, opts`) and the prologue (named ones).
    pub fn declare_params(
        &mut self,
        params: &[ast::Parameter],
        tys: Option<&Ty>,
    ) -> (String, String) {
        let (js, prologue, _) = self.declare_params_with_names(params, tys);
        (js, prologue)
    }

    /// Like [`Self::declare_params`], also returning, in declaration order,
    /// `(parameter kind, Dart name, JS name)` of each one — the `noSuchMethod`
    /// forwarder passes each parameter on to the `Invocation`.
    pub fn declare_params_with_names(
        &mut self,
        params: &[ast::Parameter],
        tys: Option<&Ty>,
    ) -> ParameterDeclaration {
        let mut names: Vec<(ast::ParameterKind, String, String)> = Vec::new();
        let mut js_params: Vec<String> = Vec::new();
        let mut prologue = String::new();
        let mut has_named = false;
        let (pos_expected, opt_expected, named_expected): ParameterTypes = match tys {
            Some(Ty::Fn {
                pos, opt, named, ..
            }) => (pos.clone(), opt.clone(), named.clone()),
            _ => (vec![], vec![], vec![]),
        };
        let mut pi = 0;
        let mut oi = 0;
        for p in params {
            let Some(name) = p.name else { continue };
            // External name (the key in `opts`): the public one of a 3.12 private
            // named parameter; the local one stays `name`.
            let external = p.external_name().unwrap_or(name);
            // An old-style generic function parameter (`bar<V>(V v)`) takes
            // the signature's type, which keeps its type parameters.
            let from_signature = p.ty.is_none() && p.function_parameters.is_none()
                || !p.function_type_params.is_empty() && tys.is_some();
            let mut ty = if from_signature {
                match p.kind {
                    ast::ParameterKind::Required => {
                        pos_expected.get(pi).cloned().unwrap_or(Ty::Dynamic)
                    }
                    ast::ParameterKind::Optional => {
                        opt_expected.get(oi).cloned().unwrap_or(Ty::Dynamic)
                    }
                    ast::ParameterKind::Named => named_expected
                        .iter()
                        .find(|(n, _, _)| n == self.name(external.sym))
                        .map(|(_, t, _)| t.clone())
                        .unwrap_or(Ty::Dynamic),
                }
            } else {
                self.param_ty(p)
            };
            if p.this_ {
                // `this.x`: the field's type.
                if let Some(c) = self.class {
                    if let Some(m) =
                        self.ctx
                            .lookup_member(&self.ctx.this_ty(c), self.name(name.sym), false)
                    {
                        if p.ty.is_none() {
                            ty = self.ctx.member_ty(&m);
                        }
                    }
                }
            }
            let jsn = self.declare(name.sym, ty.clone());
            // The JS parameter stays declared in the whole function, even
            // where the Dart name is out of scope (the body of a constructor
            // with `this.y`): a local `y` there gets another JS name.
            self.param_names.push(jsn.clone());
            if self.wildcard == Some(name.sym) && (p.this_ || p.super_) {
                // Wildcard `this._`/`super._`: binds no name, but the constructor
                // still initializes the field/forwards with the value.
                self.wildcard_params.insert(name.span.start, jsn.clone());
            }
            match p.kind {
                ast::ParameterKind::Required => {
                    pi += 1;
                    names.push((p.kind, self.name(name.sym).to_string(), jsn.clone()));
                    js_params.push(jsn);
                }
                ast::ParameterKind::Optional => {
                    oi += 1;
                    let saved_const = self.in_const;
                    self.in_const = true;
                    let def = match p.default_value {
                        Some(d) => self.emit_expr(d, Some(&ty)).0.into_at(P_ASSIGN + 1),
                        None => "null".to_string(),
                    };
                    self.in_const = saved_const;
                    names.push((p.kind, self.name(name.sym).to_string(), jsn.clone()));
                    js_params.push(format!("{jsn} = {def}"));
                }
                ast::ParameterKind::Named => {
                    has_named = true;
                    let saved_const = self.in_const;
                    self.in_const = true;
                    let def = match p.default_value {
                        Some(d) => self.emit_expr(d, Some(&ty)).0.into_at(P_COND + 1),
                        None => "null".to_string(),
                    };
                    self.in_const = saved_const;
                    // The `noSuchMethod` `Invocation` sees the external name.
                    let key = self.name(external.sym);
                    names.push((p.kind, key.to_string(), jsn.clone()));
                    let named_key = self.ctx.named_key(key);
                    prologue.push_str(&format!(
                        "let {jsn} = opts && {} ? opts{} : {def};\n",
                        js::has_named("opts", &named_key),
                        js::prop_access(&named_key)
                    ));
                }
            }
        }
        if has_named {
            js_params.push("opts".to_string());
        }
        (js_params.join(", "), prologue, names)
    }

    /// Emits a function body (block or expression) into the current writer.
    pub fn emit_body(&mut self, body: &FunctionBody) {
        match body {
            FunctionBody::Block(s) => {
                let stmt = self.ast().stmt(*s);
                if let StmtKind::Block(stmts) = &stmt.kind {
                    // The body block is a scope inside the parameters' scope (a
                    // local may shadow a parameter, as in the VM); in JS it is
                    // still the same function body.
                    self.push_scope();
                    self.hoist_locals(stmts);
                    self.fresh_list_append = self.fresh_lists(stmts);
                    for &st in stmts.iter() {
                        self.emit_stmt(st);
                        if self.always_exits(st) {
                            break;
                        }
                    }
                    self.pop_scope();
                } else {
                    self.emit_stmt(*s);
                }
            }
            FunctionBody::Expression(e) => {
                let expected = self.ret_ty.clone();
                // Dot shorthand (3.10) in an `=> e` of an `async` function: the
                // value's context is `FutureOr<T>`, which denotes what `T`
                // denotes (`return` already unwraps the `Future`).
                let future = match (&self.async_kind, &expected) {
                    (AsyncKind::Async, Ty::Iface { args, .. }) if !args.is_empty() => {
                        Some(Ty::FutureOr {
                            arg: Box::new(args[0].clone()),
                            nullable: false,
                        })
                    }
                    (AsyncKind::Async, Ty::FutureOr { .. }) => Some(expected.clone()),
                    _ => None,
                };
                // The value is checked against `FutureOr<T>`, not the
                // function's `Future<T>`.
                let (js, ty) = match future {
                    Some(ctx) => {
                        self.with_shorthand_context(*e, &ctx, |s| s.emit_expr(*e, Some(&ctx)))
                    }
                    None => self.emit_expr(*e, Some(&expected)),
                };
                self.returns.push(ty);
                if matches!(self.async_kind, AsyncKind::None | AsyncKind::Async) {
                    // `void f() => e` still returns the value of `e`.
                    crate::emit_line!(self.w, "return {};", js.code);
                } else {
                    crate::emit_line!(self.w, "{};", js.code);
                }
            }
            FunctionBody::Empty | FunctionBody::Native(_) => {}
        }
    }

    /// Declares the block's local functions ahead of time (for mutual recursion).
    pub fn hoist_locals(&mut self, _stmts: &[StmtId]) {}

    /// Builds the full text of a function: `head` is the header
    /// (`function(a) {`, `(a) => {`, `m(a) {`); applies the async/generator transformation.
    pub fn wrap_async_head(
        &self,
        kind: AsyncKind,
        head: &str,
        prologue: &str,
        body: &str,
        ret_ty: &Ty,
    ) -> String {
        let inner_ty = match ret_ty {
            Ty::Iface { args, .. } if !args.is_empty() => args[0].clone(),
            Ty::FutureOr { arg, .. } => (**arg).clone(),
            _ => Ty::Dynamic,
        };
        match kind {
            AsyncKind::None => format!("{head}\n{prologue}{body}}}"),
            AsyncKind::Async => {
                self.m.use_sdk("async");
                let rti = self.rti(&inner_ty);
                format!(
                    "{head}\n{prologue}let t$completer = async._makeAsyncAwaitCompleter({rti});\n\
let t$gen = (function*() {{\n{body}}}).call(this);\n\
let t$body = async._wrapJsFunctionForAsync((t$code, t$res) => {{\n\
  let t$r;\n\
  try {{ t$r = t$code === 1 ? t$gen.throw(t$res) : t$gen.next(t$res); }}\n\
  catch (t$e) {{ return async._asyncRethrow(t$e, t$completer); }}\n\
  if (t$r.done) return async._asyncReturn(t$r.value, t$completer);\n\
  return async._asyncAwait(t$r.value, t$body, t$completer);\n\
}});\n\
return async._asyncStartSync(t$body, t$completer);\n}}"
                )
            }
            AsyncKind::AsyncStar => {
                self.m.use_sdk("async");
                let rti = self.rti(&inner_ty);
                format!(
                    "{head}\n{prologue}let t$gen = (function*() {{\n{body}}}).call(this);\n\
let t$controller;\n\
let t$body = async._wrapJsFunctionForAsync((t$code, t$res) => {{\n\
  let t$r;\n\
  try {{ t$r = t$code === 2 ? t$gen.return(void 0) : t$code === 1 ? t$gen.throw(t$res) : t$gen.next(t$res); }}\n\
  catch (t$e) {{ return async._asyncStarHelper(t$e, 1, t$controller); }}\n\
  if (t$r.done) return async._asyncStarHelper(null, 0, t$controller);\n\
  return async._asyncStarHelper(t$r.value, t$body, t$controller);\n\
}});\n\
t$controller = async._makeAsyncStarStreamController({rti}, t$body);\n\
return async._streamOfController(t$controller);\n}}"
                )
            }
            AsyncKind::SyncStar => {
                self.m.use_sdk("async");
                let rti = self.rti(&inner_ty);
                format!(
                    "{head}\n{prologue}let t$self = this;\n\
return async._makeSyncStarIterable({rti}, () => {{\n\
  let t$gen = (function*() {{\n{body}}}).call(t$self);\n\
  return (t$it, t$code, t$err) => {{\n\
    let t$r;\n\
    try {{ t$r = t$code === 1 ? t$gen.throw(t$err) : t$gen.next(); }}\n\
    catch (t$e) {{ t$it[_datum] = t$e; return 3; }}\n\
    if (t$r.done) return 0;\n\
    if (t$r.value instanceof async._IterationMarker) return t$it[_yieldStar](t$r.value.value);\n\
    t$it[_current] = t$r.value;\n\
    return 1;\n\
  }};\n\
}});\n}}"
                )
            }
        }
    }

    // -----------------------------------------------------------------------
    // Statements
    // -----------------------------------------------------------------------

    pub fn emit_stmt(&mut self, s: StmtId) {
        let stmt = self.ast().stmt(s);
        match &stmt.kind {
            StmtKind::Block(stmts) => {
                self.w.open("{");
                self.push_scope();
                for &st in stmts.iter() {
                    self.emit_stmt(st);
                    if self.always_exits(st) {
                        break;
                    }
                }
                self.pop_scope();
                self.w.close("}");
            }
            StmtKind::Variables(list) => self.emit_var_list(list),
            StmtKind::PatternVariables { pattern, value, .. } => {
                let schema = self.pattern_schema(*pattern);
                let (vjs, vty) = self.emit_expr(*value, schema.as_ref());
                let (vjs, vty) = match &schema {
                    Some(s) => self.coerce_to(vjs, vty, s),
                    None => (vjs, vty),
                };
                let t = self.temp();
                crate::emit_line!(self.w, "{t} = {};", vjs.code);
                let mut binds = Vec::new();
                let cond = self.pattern_cond(*pattern, &t, &vty, &mut binds, true);
                for (_, _, jsn) in &binds {
                    crate::emit_line!(self.w, "let {jsn} = null;");
                }
                if cond != "true" {
                    crate::emit_line!(
                        self.w,
                        "if (!({cond})) dart.throw(new core.StateError.new(\"Pattern matching error\"));"
                    );
                    self.m.use_sdk("core");
                }
            }
            StmtKind::Function(fid) => self.emit_local_function(*fid),
            StmtKind::Expression(e) => {
                // SDK mode: `JS('', 'throw #', x)` is a statement.
                if self.ctx.sdk.is_some() {
                    if let Some(t) = crate::own_sdk::js_statement(self, *e) {
                        crate::emit_line!(self.w, "{t};");
                        return;
                    }
                }
                let js = self.emit_discarded(*e);
                crate::emit_line!(self.w, "{};", js.code);
            }
            StmtKind::If {
                condition,
                case_pattern,
                guard,
                then,
                else_,
            } => {
                if let Some(pat) = case_pattern {
                    let (vjs, vty) = self.emit_expr(*condition, None);
                    let t = self.temp();
                    crate::emit_line!(self.w, "{t} = {};", vjs.code);
                    self.w.open("{");
                    self.push_scope();
                    let mut binds = Vec::new();
                    let cond = self.pattern_cond(*pat, &t, &vty, &mut binds, false);
                    for (_, _, jsn) in &binds {
                        crate::emit_line!(self.w, "let {jsn} = null;");
                    }
                    let mut full = cond;
                    if let Some(g) = guard {
                        let (gjs, _) = self.emit_expr(*g, Some(&self.ctx.t_bool()));
                        full =
                            format!("{} && {}", paren_if_needed(&full), gjs.at(crate::js::P_AND));
                    }
                    crate::open_block!(self.w, "if ({full}) {{");
                    // The then branch is a scope inside the pattern
                    // variables'; the else branch does not see them.
                    self.push_scope();
                    self.emit_stmt(*then);
                    self.pop_scope();
                    self.w.close("}");
                    let pattern_scope = self.scopes.pop();
                    if let Some(e) = else_ {
                        self.w.open("else {");
                        self.push_scope();
                        self.emit_stmt(*e);
                        self.pop_scope();
                        self.w.close("}");
                    }
                    if let Some(scope) = pattern_scope {
                        self.scopes.push(scope);
                    }
                    self.pop_scope();
                    self.w.close("}");
                    return;
                }
                self.pending_promotions.clear();
                self.negated_promotions.clear();
                let (cjs, _) = self.emit_cond(*condition);
                let promos = std::mem::take(&mut self.pending_promotions);
                let neg_promos = std::mem::take(&mut self.negated_promotions);
                // Constant condition (typical of `const bool.fromEnvironment`):
                // the dead branch is not emitted, as in dartdevc.
                if cjs == "false" || cjs == "true" {
                    let live = if cjs == "true" { Some(*then) } else { *else_ };
                    if let Some(st) = live {
                        self.push_scope();
                        self.emit_stmt(st);
                        self.pop_scope();
                    }
                    return;
                }
                crate::open_block!(self.w, "if ({}) {{", cjs);
                self.push_scope();
                for (sym, t) in &promos {
                    if let Some(loc) = self.lookup_local(*sym).cloned() {
                        self.declare_js(*sym, loc.js, t.clone());
                    }
                }
                self.emit_stmt(*then);
                self.pop_scope();
                self.w.close("}");
                if let Some(e) = else_ {
                    self.w.open("else {");
                    self.push_scope();
                    for (sym, t) in &neg_promos {
                        if let Some(loc) = self.lookup_local(*sym).cloned() {
                            self.declare_js(*sym, loc.js, t.clone());
                        }
                    }
                    self.emit_stmt(*e);
                    self.pop_scope();
                    self.w.close("}");
                } else if self.stmt_ends_with_jump(*then) {
                    // `if (x is! T) return;` promotes `x` in the rest of the block.
                    for (sym, t) in &neg_promos {
                        self.set_local_ty(*sym, t.clone());
                    }
                }
            }
            StmtKind::While { condition, body } => {
                let (cjs, _) = self.emit_cond(*condition);
                crate::open_block!(self.w, "while ({cjs}) {{");
                self.push_scope();
                self.continue_scopes.push(None);
                self.emit_stmt(*body);
                self.continue_scopes.pop();
                self.pop_scope();
                self.w.close("}");
            }
            StmtKind::DoWhile { body, condition } => {
                self.w.open("do {");
                self.push_scope();
                self.continue_scopes.push(None);
                self.emit_stmt(*body);
                self.continue_scopes.pop();
                self.pop_scope();
                let (cjs, _) = self.emit_cond(*condition);
                crate::close_block!(self.w, "}} while ({cjs});");
            }
            StmtKind::For {
                init,
                condition,
                updates,
                body,
                ..
            } => {
                self.push_scope();
                if let Some(ast::ForInit::Variables(list)) = init {
                    if list
                        .variables
                        .iter()
                        .any(|v| v.initializer.is_some_and(|e| self.contains_closure(e)))
                    {
                        self.emit_for_capturing_init(list, *condition, updates, *body);
                        self.pop_scope();
                        return;
                    }
                }
                let init_js = match init {
                    Some(ast::ForInit::Variables(list)) => {
                        let mut parts = Vec::new();
                        let declared = list.ty.map(|t| self.resolve_type(t));
                        for v in list.variables.iter() {
                            let (ijs, ity) = match v.initializer {
                                Some(e) => {
                                    let (j, t) = self.emit_expr(e, declared.as_ref());
                                    (Some(j), t)
                                }
                                None => (None, Ty::Null),
                            };
                            let ty = declared.clone().unwrap_or(ity);
                            let jsn = self.declare(v.name.sym, ty);
                            match ijs {
                                Some(j) => parts.push(format!("{jsn} = {}", j.at(P_ASSIGN + 1))),
                                None => parts.push(jsn),
                            }
                        }
                        format!("let {}", parts.join(", "))
                    }
                    Some(ast::ForInit::Expression(e)) => self.emit_expr(*e, None).0.code,
                    // The pattern declaration comes out before the loop, in its
                    // scope: the `for` has no initializer.
                    Some(ast::ForInit::Pattern { pattern, value, .. }) => {
                        self.emit_pattern_variables(*pattern, *value);
                        String::new()
                    }
                    None => String::new(),
                };
                let counter = self.positive_counter_modulo(init, *condition, updates, *body);
                let previous_counter = std::mem::replace(&mut self.counter_modulo, counter);
                let plan = self.typed_read_loop(init, *condition, updates, *body);
                let upd: Vec<String> = updates
                    .iter()
                    .map(|u| self.emit_discarded(*u).code)
                    .collect();
                let versioned = plan.is_some();
                if let Some(plan) = plan {
                    self.w.open(&format!("if ({}) {{", plan.guard));
                    let saved = std::mem::replace(&mut self.typed_loop_raw, plan.raw);
                    let cond_js = self.emit_cond(condition.unwrap()).0;
                    self.w.open(&format!(
                        "for ({init_js}; {cond_js}; {}) {{",
                        upd.join(", ")
                    ));
                    self.push_scope();
                    self.continue_scopes.push(None);
                    self.emit_stmt(*body);
                    self.continue_scopes.pop();
                    self.pop_scope();
                    self.w.close("}");
                    self.typed_loop_raw = saved;
                    self.w.close("}");
                    self.w.open("else {");
                }
                let cond_js = match condition {
                    Some(c) => self.emit_cond(*c).0,
                    None => String::new(),
                };
                self.w.open(&format!(
                    "for ({init_js}; {cond_js}; {}) {{",
                    upd.join(", ")
                ));
                self.push_scope();
                self.continue_scopes.push(None);
                self.emit_stmt(*body);
                self.continue_scopes.pop();
                self.pop_scope();
                self.w.close("}");
                if versioned {
                    self.w.close("}");
                }
                self.counter_modulo = previous_counter;
                self.pop_scope();
            }
            StmtKind::ForIn {
                await_,
                target,
                iterable,
                body,
            } => {
                self.continue_scopes.push(None);
                self.emit_for_in(*await_, target, *iterable, *body);
                self.continue_scopes.pop();
            }
            StmtKind::Switch { value, cases } => self.emit_switch(*value, cases),
            StmtKind::Break(label) => match label {
                Some(l) => {
                    let js = self.js_label(self.name(l.sym));
                    crate::emit_line!(self.w, "break {js};");
                }
                None => {
                    if let Some(Some(l)) = self.switch_labels.last() {
                        let l = l.clone();
                        crate::emit_line!(self.w, "break {l};");
                    } else if let Some((_, loop_label, _)) = self.case_labels.last().cloned() {
                        crate::emit_line!(self.w, "break {loop_label};");
                    } else {
                        self.w.line("break;");
                    }
                }
            },
            StmtKind::Continue(label) => match label {
                Some(l) => {
                    let n = self.name(l.sym).to_string();
                    if let Some((_, loop_label, state)) = self
                        .case_labels
                        .iter()
                        .rev()
                        .find(|(k, _, _)| *k == n)
                        .cloned()
                    {
                        crate::emit_line!(self.w, "{state};");
                        crate::emit_line!(self.w, "continue {loop_label};");
                    } else {
                        let js = self.js_label(&n);
                        crate::emit_line!(self.w, "continue {js};");
                    }
                }
                None => match self.continue_scopes.last_mut() {
                    // Inside the `while (true)` of a `switch` with case labels:
                    // leave it, and continue the loop after it.
                    Some(Some((label, state, used))) => {
                        *used = true;
                        let (label, state) = (label.clone(), state.clone());
                        crate::emit_line!(self.w, "{state} = -2;");
                        crate::emit_line!(self.w, "break {label};");
                    }
                    _ => self.w.line("continue;"),
                },
            },
            StmtKind::Return(e) => match e {
                Some(e) => {
                    let expected = self.ret_ty.clone();
                    // In an `async` body the value is a `FutureOr<T>` (a
                    // `dynamic` one may be a future).
                    let expected = match (&self.async_kind, &expected) {
                        (AsyncKind::Async, Ty::Iface { args, .. }) if !args.is_empty() => {
                            Ty::FutureOr {
                                arg: Box::new(args[0].clone()),
                                nullable: false,
                            }
                        }
                        (AsyncKind::Async, Ty::FutureOr { .. }) => expected.clone(),
                        _ => expected,
                    };
                    let (js, ty) = self.emit_expr(*e, Some(&expected));
                    self.returns.push(ty);
                    crate::emit_line!(self.w, "return {};", js.code);
                }
                None => self.w.line("return;"),
            },
            StmtKind::Yield { star, value } => {
                // Context: the element type (`yield e`) or the generator's own
                // return type (`yield* e`), when known.
                let element = self.generator_element(&self.ret_ty.clone());
                let context = match (star, &element) {
                    (false, Some(e)) => Some(e.clone()),
                    (true, Some(_)) => Some(self.ret_ty.non_null()),
                    _ => None,
                };
                let (js, ty) = self.emit_expr(*value, context.as_ref());
                // In a generator, `return e;` is forbidden: `returns` keeps the
                // produced element types, which give the inferred return type of
                // a function literal (`emit_function_expr`).
                let produced = if *star {
                    self.generator_element(&ty).unwrap_or(Ty::Dynamic)
                } else {
                    ty
                };
                self.returns.push(produced);
                match (self.async_kind, star) {
                    (AsyncKind::AsyncStar, false) => {
                        self.m.use_sdk("async");
                        crate::emit_line!(
                            self.w,
                            "yield async._IterationMarker.yieldSingle({});",
                            js.code
                        );
                    }
                    (AsyncKind::AsyncStar, true) => {
                        self.m.use_sdk("async");
                        crate::emit_line!(
                            self.w,
                            "yield async._IterationMarker.yieldStar({});",
                            js.code
                        );
                    }
                    (AsyncKind::SyncStar, true) => {
                        self.m.use_sdk("async");
                        crate::emit_line!(
                            self.w,
                            "yield async._IterationMarker.yieldStar({});",
                            js.code
                        );
                    }
                    _ => crate::emit_line!(self.w, "yield {};", js.code),
                }
            }
            StmtKind::Try {
                body,
                catches,
                finally_,
            } => self.emit_try(*body, catches, *finally_),
            StmtKind::Labeled { labels, body } => {
                let mut js_labels = Vec::new();
                for l in labels.iter() {
                    let jl = self.fresh_label();
                    self.labels.push((self.name(l.sym).to_string(), jl.clone()));
                    js_labels.push(jl);
                }
                // A single JS label is enough: the others become aliases for it.
                let first = js_labels[0].clone();
                for l in labels.iter().skip(1) {
                    let n = self.name(l.sym).to_string();
                    if let Some(e) = self.labels.iter_mut().rev().find(|(k, _)| *k == n) {
                        e.1 = first.clone();
                    }
                }
                let body_stmt = self.ast().stmt(*body);
                let is_loop = matches!(
                    body_stmt.kind,
                    StmtKind::For { .. }
                        | StmtKind::ForIn { .. }
                        | StmtKind::While { .. }
                        | StmtKind::DoWhile { .. }
                );
                // A labeled declaration stays in the enclosing scope (no JS
                // block): nothing can break to its label.
                let declaration = matches!(
                    body_stmt.kind,
                    StmtKind::Variables(_)
                        | StmtKind::PatternVariables { .. }
                        | StmtKind::Function(_)
                );
                if declaration {
                    self.emit_stmt(*body);
                } else if is_loop || matches!(body_stmt.kind, StmtKind::Switch { .. }) {
                    crate::emit_line!(self.w, "{first}:");
                    self.emit_stmt(*body);
                } else {
                    crate::open_block!(self.w, "{first}: {{");
                    self.emit_stmt(*body);
                    self.w.close("}");
                }
                for _ in labels.iter() {
                    self.labels.pop();
                }
            }
            StmtKind::Assert { condition, message } => {
                if crate::own_sdk::omit_assert(self.ctx, self.lib) {
                    return;
                }
                let (cjs, _) = self.emit_cond(*condition);
                let msg = match message {
                    Some(m) => self.emit_expr(*m, None).0.code,
                    None => "null".to_string(),
                };
                let text = js::string_literal(self.text(self.expr(*condition).span));
                crate::emit_line!(
                    self.w,
                    "if (!({cjs})) dart.assertFailed({msg}, null, 0, 0, {text});"
                );
            }
            StmtKind::Empty => self.w.line(";"),
        }
    }

    pub fn js_label(&self, name: &str) -> String {
        self.labels
            .iter()
            .rev()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| name.to_string())
    }

    /// Boolean condition (with `dart.test` when the type is not a strict `bool`).
    pub fn emit_cond(&mut self, e: ExprId) -> (String, Ty) {
        let (js, ty) = self.emit_expr(e, Some(&self.ctx.t_bool()));
        if self.ctx.is_bool(&ty) {
            (js.code, ty)
        } else if ty.is_class(self.ctx.bool_) {
            (format!("dart.test({})", js.code), ty)
        } else {
            (format!("dart.dtest({})", js.code), ty)
        }
    }

    fn emit_var_list(&mut self, list: &ast::VariableList) {
        let declared = list.ty.map(|t| self.resolve_type(t));
        for v in list.variables.iter() {
            // Another declaration with the same name in the same scope: the program
            // is invalid (shadowing one from an outer scope is allowed).
            if self.wildcard != Some(v.name.sym)
                && self
                    .scopes
                    .last()
                    .is_some_and(|s| s.contains_key(&v.name.sym))
            {
                let name = self.name(v.name.sym).to_string();
                self.language_error(
                    v.name.span,
                    format!("'{name}' is already declared in this scope."),
                );
            }
            match v.initializer {
                Some(e) => {
                    let saved_const = self.in_const;
                    if list.const_ {
                        self.in_const = true;
                    }
                    let (js, ty) = self.emit_expr(e, declared.as_ref());
                    self.in_const = saved_const;
                    let ty = match &declared {
                        Some(d) => d.clone(),
                        None => ty,
                    };
                    let jsn = self.declare(v.name.sym, ty);
                    if list.late {
                        crate::emit_line!(self.w, "let {jsn} = void 0;");
                        if let Some(l) = self.scopes.last_mut().and_then(|s| s.get_mut(&v.name.sym))
                        {
                            l.lazy_init = Some(js.into_at(crate::js::P_ASSIGN + 1));
                            l.late_final = list.final_;
                        }
                    } else {
                        crate::emit_line!(self.w, "let {jsn} = {};", js.code);
                    }
                }
                None => {
                    let ty = declared.clone().unwrap_or(Ty::Dynamic);
                    let jsn = self.declare(v.name.sym, ty.clone());
                    if list.late {
                        crate::emit_line!(self.w, "let {jsn} = void 0;");
                        if let Some(l) = self.scopes.last_mut().and_then(|s| s.get_mut(&v.name.sym))
                        {
                            l.late_check = !ty.is_nullable();
                            l.late_final = list.final_;
                        }
                    } else {
                        crate::emit_line!(self.w, "let {jsn} = null;");
                    }
                }
            }
        }
    }

    fn emit_local_function(&mut self, fid: ast::FunctionId) {
        let f = self.ast().function(fid);
        let name = f.name.expect("local function with a name");
        let ty = self.local_fn_ty(f);
        let jsn = self.declare(name.sym, ty.clone());
        // The signature is that of `local_fn_ty` (the one in scope, which the
        // calls already read). The shared type gives the return type inferred
        // from every `return`, also those of branches not emitted (`if (false)`).
        let common = self
            .ctx
            .bodies
            .units
            .get(self.unit.0 as usize)
            .and_then(|u| {
                u.function_runtime_type(fid)
                    .or_else(|| u.local_type(name.span.start))
            });
        let (fn_js, inferred) = self.emit_function_expr(fid, Some(&ty), common);
        crate::emit_line!(self.w, "let {jsn} = {};", fn_js.code);
        // Without a written return type, the local function's return type is
        // the one inferred from the body (the same its rti carries): the
        // following reads see it. Inside its own body (recursion) it still
        // returns `dynamic`.
        if f.return_type.is_none() && matches!(inferred, Ty::Fn { .. }) {
            self.set_local_ty(name.sym, inferred);
        }
    }

    /// Declared type of a function (local or expression) from its annotation.
    pub fn local_fn_ty(&mut self, f: &ast::Function) -> Ty {
        let mut tps = Vec::new();
        let mut scope_params = Vec::new();
        for tp in f.type_params.iter() {
            let p = self
                .ctx
                .fresh_param(self.name(tp.name.sym), self.ctx.t_object_q());
            let jsn = self.type_param_js_name(&p.name);
            scope_params.push((p.id, jsn));
            tps.push(p);
        }
        let saved = self.fn_type_params.clone();
        let mut new = scope_params.clone();
        new.extend(saved.iter().cloned());
        self.fn_type_params = new;
        for (tp, p) in f.type_params.iter().zip(tps.iter_mut()) {
            if let Some(b) = tp.bound {
                let bt = self.resolve_type(b);
                self.ctx.param_bounds.borrow_mut().insert(p.id, bt.clone());
                *p.bound = bt;
            }
        }
        let ret = f
            .return_type
            .map(|r| self.resolve_type(r))
            .unwrap_or(Ty::Dynamic);
        let (pos, opt, named) = match &f.parameters {
            Some(ps) => self.resolve_params(ps),
            None => (vec![], vec![], vec![]),
        };
        self.fn_type_params = saved;
        Ty::Fn {
            type_params: tps,
            ret: Box::new(ret),
            pos,
            opt,
            named,
            nullable: false,
        }
    }

    /// Whether a function expression occurs inside `e`. Sub-expressions are
    /// allocated before their parent, so they sit right below `e` in the
    /// arena, with spans inside its span.
    fn contains_closure(&self, e: ExprId) -> bool {
        let ast = self.ast();
        let span = ast.expr(e).span;
        (0..=e.0)
            .rev()
            .map(|i| ast.expr(ExprId(i)))
            .take_while(|x| x.span.start >= span.start && x.span.end <= span.end)
            .any(|x| matches!(x.kind, ast::ExprKind::FunctionExpression(_)))
    }

    /// `for (var i = 0, f = () => i; c; u) b` where a closure in the
    /// initializer captures the loop variables. Dart makes the initializer's
    /// variables those of the first iteration, but a JS `for` copies them
    /// into a fresh environment before it. The variables are declared without
    /// value in the header and initialized in the first iteration, where the
    /// condition also runs; the update still runs in the next iteration's
    /// copy, as in Dart.
    fn emit_for_capturing_init(
        &mut self,
        list: &ast::VariableList,
        condition: Option<ExprId>,
        updates: &[ExprId],
        body: StmtId,
    ) {
        let first = format!("t${}", self.temp_counter);
        self.temp_counter += 1;
        let declared = list.ty.map(|t| self.resolve_type(t));
        let mut names = vec![format!("{first} = true")];
        let mut inits = Vec::new();
        for v in list.variables.iter() {
            let (ijs, ity) = match v.initializer {
                Some(e) => {
                    let (j, t) = self.emit_expr(e, declared.as_ref());
                    (j.at(P_ASSIGN + 1).to_string(), t)
                }
                None => ("null".to_string(), Ty::Null),
            };
            let ty = declared.clone().unwrap_or(ity);
            let jsn = self.declare(v.name.sym, ty);
            inits.push(format!("{jsn} = {ijs};"));
            names.push(jsn);
        }
        let upd: Vec<String> = updates
            .iter()
            .map(|u| self.emit_discarded(*u).code)
            .collect();
        self.w.open(&format!(
            "for (let {}; ; {}) {{",
            names.join(", "),
            upd.join(", ")
        ));
        self.w.open(&format!("if ({first}) {{"));
        crate::emit_line!(self.w, "{first} = false;");
        for i in inits {
            self.w.line(&i);
        }
        self.w.close("}");
        if let Some(c) = condition {
            let (cjs, _) = self.emit_cond(c);
            crate::emit_line!(self.w, "if (!({cjs})) break;");
        }
        self.push_scope();
        self.continue_scopes.push(None);
        self.emit_stmt(body);
        self.continue_scopes.pop();
        self.pop_scope();
        self.w.close("}");
    }

    fn emit_for_in(
        &mut self,
        await_: bool,
        target: &ast::ForInTarget,
        iterable: ExprId,
        body: StmtId,
    ) {
        let (ijs, ity) = self.emit_expr(iterable, None);
        let (ijs, ity) = self.checked_iterable(ijs, ity, await_);
        let elem_ty = if await_ {
            self.ctx
                .stream_
                .and_then(|s| self.ctx.as_super(&ity, s))
                .and_then(|t| t.args().first().cloned())
                .unwrap_or(Ty::Dynamic)
        } else {
            self.ctx
                .iterable_
                .and_then(|s| self.ctx.as_super(&ity, s))
                .and_then(|t| t.args().first().cloned())
                .unwrap_or(Ty::Dynamic)
        };
        self.push_scope();
        let (var_js, decl_ty, pattern) = match target {
            ast::ForInTarget::Declared { ty, name, .. } => {
                let t = ty.map(|t| self.resolve_type(t)).unwrap_or(elem_ty.clone());
                (self.declare(name.sym, t.clone()), t, None)
            }
            ast::ForInTarget::Pattern { pattern, .. } => {
                (self.temp(), elem_ty.clone(), Some(*pattern))
            }
            ast::ForInTarget::Expression(e) => {
                let t = self.temp();
                (
                    t,
                    elem_ty.clone(),
                    Some(*e).map(|_| ast::PatternId(u32::MAX)),
                )
            }
        };
        if await_ {
            self.m.use_sdk("async");
            let it = self.temp();
            let elem_rti = self.rti(&elem_ty);
            let recipe = self
                .ctx
                .recipe_of("dart:async", "StreamIterator", "async|StreamIterator");
            crate::emit_line!(
                self.w,
                "{it} = async.StreamIterator.new({elem_rti}[_eval](\"{recipe}<0>\"), {});",
                ijs.code
            );
            self.w.open("try {");
            crate::open_block!(self.w, "while ((yield {it}.moveNext())) {{");
            match pattern {
                Some(p) if p.0 != u32::MAX => {
                    crate::emit_line!(self.w, "let {var_js} = {it}.current;");
                    self.emit_pattern_bind_stmt(p, &var_js, &elem_ty);
                }
                Some(_) => {
                    if let ast::ForInTarget::Expression(e) = target {
                        let (ljs, _) =
                            self.emit_assign_to(*e, &Js::prim(format!("{it}.current")), &elem_ty);
                        crate::emit_line!(self.w, "{};", ljs.code);
                    }
                }
                None => {
                    let value =
                        self.checked_loop_value(&format!("{it}.current"), &elem_ty, &decl_ty);
                    crate::emit_line!(self.w, "let {var_js} = {value};");
                }
            }
            self.emit_stmt(body);
            self.w.close("}");
            self.w.close("}");
            self.w.open("finally {");
            crate::emit_line!(self.w, "yield {it}.cancel();");
            self.w.close("}");
        } else {
            match pattern {
                Some(p) if p.0 != u32::MAX => {
                    crate::open_block!(self.w, "for (let {var_js} of {}) {{", ijs.code);
                    self.emit_pattern_bind_stmt(p, &var_js, &elem_ty);
                }
                Some(_) => {
                    crate::open_block!(self.w, "for (let {var_js} of {}) {{", ijs.code);
                    if let ast::ForInTarget::Expression(e) = target {
                        let (ljs, _) = self.emit_assign_to(*e, &Js::prim(var_js.clone()), &elem_ty);
                        crate::emit_line!(self.w, "{};", ljs.code);
                    }
                }
                None => {
                    crate::open_block!(self.w, "for (let {var_js} of {}) {{", ijs.code);
                    let value = self.checked_loop_value(&var_js, &elem_ty, &decl_ty);
                    if value != var_js {
                        crate::emit_line!(self.w, "{var_js} = {value};");
                    }
                }
            }
            self.emit_stmt(body);
            self.w.close("}");
        }
        self.pop_scope();
    }

    /// Destructures `value_js` with the pattern, declaring the variables (irrefutable pattern).
    pub fn emit_pattern_bind_stmt(&mut self, p: ast::PatternId, value_js: &str, vty: &Ty) {
        let mut binds = Vec::new();
        let cond = self.pattern_cond(p, value_js, vty, &mut binds, true);
        for (_, _, jsn) in &binds {
            crate::emit_line!(self.w, "let {jsn} = null;");
        }
        if cond != "true" {
            self.m.use_sdk("core");
            crate::emit_line!(
                self.w,
                "if (!({cond})) dart.throw(new core.StateError.new(\"Pattern matching error\"));"
            );
        }
    }

    fn emit_switch(&mut self, value: ExprId, cases: &[ast::SwitchCase]) {
        let (vjs, vty) = self.emit_expr(value, None);
        // Classic JS switch when the value is primitive/enum and all cases are constants.
        let all_const = cases.iter().all(|c| match c.pattern {
            None => true,
            Some(p) => self.is_simple_const_pattern(p),
        });
        let prim = self.ctx.is_js_primitive(&vty)
            || vty.class().is_some_and(|c| self.ctx.is_enum_class(c))
            || matches!(vty, Ty::Iface { class, .. } if Some(class) == self.ctx.type_);
        let no_guards = cases.iter().all(|c| c.guard.is_none());
        if all_const && prim && no_guards {
            let label = self.fresh_label();
            let has_case_labels = cases.iter().any(|c| !c.labels.is_empty());
            self.switch_labels.push(None);
            if has_case_labels {
                // `continue label` to another case: a loop with a state variable.
                let state = self.temp();
                crate::emit_line!(self.w, "{state} = {};", vjs.code);
                crate::emit_line!(self.w, "{label}:");
                self.w.open("while (true) {");
                self.w.open(&format!("switch ({state}) {{"));
                self.continue_scopes
                    .push(Some((label.clone(), state.clone(), false)));
                let mut n_pushed = 0;
                for c in cases {
                    // The value that selects the case: its first constant, or
                    // for `default:` a new object, equal to no constant.
                    let selector = match c.pattern {
                        Some(p) => self.const_pattern_values(p).first().cloned(),
                        None => Some("{}".to_string()),
                    };
                    if let Some(first) = selector {
                        for l in c.labels.iter() {
                            self.case_labels.push((
                                self.name(l.sym).to_string(),
                                label.clone(),
                                format!("{state} = {first}"),
                            ));
                            n_pushed += 1;
                        }
                    }
                }
                for c in cases {
                    match c.pattern {
                        None => self.w.line("default:"),
                        Some(p) => {
                            for cv in self.const_pattern_values(p) {
                                crate::emit_line!(self.w, "case {}:", cv);
                            }
                        }
                    }
                    // An empty case shares the next case's body.
                    if c.body.is_empty() {
                        continue;
                    }
                    self.w.open("{");
                    self.push_scope();
                    for &s in c.body.iter() {
                        self.emit_stmt(s);
                    }
                    self.pop_scope();
                    if !self.ends_with_jump(&c.body) {
                        crate::emit_line!(self.w, "break {label};");
                    }
                    self.w.close("}");
                }
                self.w.close("}");
                self.w.line("break;");
                self.w.close("}");
                for _ in 0..n_pushed {
                    self.case_labels.pop();
                }
                self.switch_labels.pop();
                if let Some(Some((_, _, true))) = self.continue_scopes.pop() {
                    crate::emit_line!(self.w, "if ({state} === -2) continue;");
                }
                return;
            }
            crate::emit_line!(self.w, "{label}:");
            crate::open_block!(self.w, "switch ({}) {{", vjs.code);
            // A case that never matches (`Liveness::dead_case`) loses its
            // label, and its body unless an empty case before it falls into
            // it (the world's rule, `dartforge_world::impact`).
            let mut falls = false;
            for c in cases {
                let has_stmts = !c.body.is_empty();
                let dead = c
                    .pattern
                    .is_some_and(|p| self.ctx.filter.is_some_and(|f| f.dead_case(self.unit, p)));
                if dead && !falls {
                    continue;
                }
                falls = !has_stmts && (!dead || falls);
                match c.pattern {
                    None => self.w.line("default:"),
                    Some(_) if dead => {}
                    Some(p) => {
                        let consts = self.const_pattern_values(p);
                        for cv in consts {
                            crate::emit_line!(self.w, "case {}:", cv);
                        }
                    }
                }
                if has_stmts {
                    self.w.open("{");
                    self.push_scope();
                    for &s in c.body.iter() {
                        self.emit_stmt(s);
                    }
                    self.pop_scope();
                    // Dart 3: no implicit fallthrough.
                    if !self.ends_with_jump(&c.body) {
                        crate::emit_line!(self.w, "break {label};");
                    }
                    self.w.close("}");
                }
            }
            self.w.close("}");
            self.switch_labels.pop();
            return;
        }
        // General form: a labeled block with pattern tests. With case labels
        // (`continue L` to another case), a `while (true)` whose state
        // variable selects the case (-1: match the patterns).
        let label = self.fresh_label();
        let t = self.temp();
        crate::emit_line!(self.w, "{t} = {};", vjs.code);
        self.switch_labels.push(Some(label.clone()));
        let labeled = cases.iter().any(|c| !c.labels.is_empty());
        let state = labeled.then(|| self.temp());
        let mut n_pushed = 0;
        if let Some(state) = &state {
            crate::emit_line!(self.w, "{state} = -1;");
            crate::open_block!(self.w, "{label}: while (true) {{");
            self.continue_scopes
                .push(Some((label.clone(), state.clone(), false)));
            let mut i = 0;
            let mut k = 0;
            while i < cases.len() {
                let mut j = i;
                while j + 1 < cases.len() && cases[j].body.is_empty() {
                    j += 1;
                }
                for c in &cases[i..=j] {
                    for l in c.labels.iter() {
                        self.case_labels.push((
                            self.name(l.sym).to_string(),
                            label.clone(),
                            format!("{state} = {k}"),
                        ));
                        n_pushed += 1;
                    }
                }
                k += 1;
                i = j + 1;
            }
        } else {
            crate::open_block!(self.w, "{label}: {{");
        }
        let mut i = 0;
        let mut k = 0;
        while i < cases.len() {
            // Consecutive cases without a body share the next body.
            let mut j = i;
            while j + 1 < cases.len() && cases[j].body.is_empty() {
                j += 1;
            }
            let group = &cases[i..=j];
            let body_case = &cases[j];
            self.w.open("{");
            self.push_scope();
            let mut alts: Vec<String> = Vec::new();
            let mut binds = Vec::new();
            // Several heads sharing a body: each binds its own variables (a
            // closure in its guard captures that head's), copied to the
            // body's variables when it matches.
            let per_head = group.len() > 1;
            let mut head_scopes = 0;
            let mut heads: Vec<Vec<(SymbolId, Ty, String)>> = Vec::new();
            for c in group {
                let mut hb = Vec::new();
                if per_head {
                    self.push_scope();
                    head_scopes += 1;
                }
                let cond = match c.pattern {
                    None => "true".to_string(),
                    Some(p) => self.pattern_cond(p, &t, &vty, &mut hb, false),
                };
                let mut full = cond;
                if let Some(g) = c.guard {
                    let (gjs, _) = self.emit_expr(g, Some(&self.ctx.t_bool()));
                    full = format!("{} && {}", paren_if_needed(&full), gjs.at(crate::js::P_AND));
                }
                alts.push(full);
                heads.push(hb);
            }
            if per_head && heads.iter().any(|h| !h.is_empty()) {
                // The body's variables: those of the last head, with fresh
                // names while the heads' are still in use.
                let body_vars: Vec<(SymbolId, Ty, String)> = heads
                    .last()
                    .map(|h| {
                        h.iter()
                            .map(|(sym, ty, _)| (*sym, ty.clone(), self.declare(*sym, ty.clone())))
                            .collect()
                    })
                    .unwrap_or_default();
                for (alt, hb) in alts.iter_mut().zip(&heads) {
                    let copies: Vec<String> = body_vars
                        .iter()
                        .filter_map(|(sym, _, b)| {
                            let (_, _, h) = hb.iter().find(|(s, _, _)| s == sym)?;
                            Some(format!("{b} = {h}"))
                        })
                        .collect();
                    if !copies.is_empty() {
                        *alt = format!("{} && ({}, true)", paren_if_needed(alt), copies.join(", "));
                    }
                }
                for hb in heads.drain(..) {
                    binds.extend(hb);
                }
                for _ in 0..head_scopes {
                    self.pop_scope();
                }
                for (sym, ty, js) in &body_vars {
                    self.declare_js(*sym, js.clone(), ty.clone());
                }
                binds.extend(body_vars);
            } else {
                for _ in 0..head_scopes {
                    self.pop_scope();
                }
                // The heads' variables stay visible to the body.
                for hb in heads.drain(..) {
                    for (sym, ty, js) in &hb {
                        if per_head {
                            self.declare_js(*sym, js.clone(), ty.clone());
                        }
                    }
                    binds.extend(hb);
                }
            }
            for (_, _, jsn) in &binds {
                crate::emit_line!(self.w, "let {jsn} = null;");
            }
            let full = if alts.len() == 1 {
                alts.remove(0)
            } else {
                alts.iter()
                    .map(|a| format!("({a})"))
                    .collect::<Vec<_>>()
                    .join(" || ")
            };
            let full = match &state {
                Some(state) => format!("{state} === {k} || {state} === -1 && ({full})"),
                None => full,
            };
            k += 1;
            crate::open_block!(self.w, "if ({full}) {{");
            // The body is a scope inside the pattern variables' one.
            self.push_scope();
            for &s in body_case.body.iter() {
                self.emit_stmt(s);
            }
            self.pop_scope();
            if !self.ends_with_jump(&body_case.body) {
                crate::emit_line!(self.w, "break {label};");
            }
            self.w.close("}");
            self.pop_scope();
            self.w.close("}");
            i = j + 1;
        }
        if let Some(state) = &state {
            // No case matched: leave the loop.
            crate::emit_line!(self.w, "break {label};");
            self.w.close("}");
            for _ in 0..n_pushed {
                self.case_labels.pop();
            }
            if let Some(Some((_, _, true))) = self.continue_scopes.pop() {
                crate::emit_line!(self.w, "if ({state} === -2) continue;");
            }
        } else {
            self.w.close("}");
        }
        self.switch_labels.pop();
    }

    /// Whether `s` always leaves the function, so what follows it in a block
    /// is dead and is not emitted. Only with a production filter (the
    /// world folds the same conditions and skips the same statements:
    /// `dartforge_world::Constants::always_exits`, which this mirrors).
    pub fn always_exits(&self, s: StmtId) -> bool {
        if !self.ctx.filter.is_some_and(|f| f.production()) {
            return false;
        }
        match &self.ast().stmt(s).kind {
            StmtKind::Return(_) => true,
            StmtKind::Expression(e) => matches!(
                self.expr(*e).kind,
                ast::ExprKind::Throw(_) | ast::ExprKind::Rethrow
            ),
            StmtKind::Block(ss) => ss.iter().any(|t| self.always_exits(*t)),
            StmtKind::If {
                condition,
                case_pattern: None,
                guard: None,
                then,
                else_,
            } => match self.constant(*condition) {
                Some(true) => self.always_exits(*then),
                Some(false) => else_.is_some_and(|e| self.always_exits(e)),
                None => false,
            },
            _ => false,
        }
    }

    pub fn stmt_ends_with_jump(&self, s: StmtId) -> bool {
        match &self.ast().stmt(s).kind {
            StmtKind::Block(b) => self.ends_with_jump(b),
            _ => self.ends_with_jump(std::slice::from_ref(&s)),
        }
    }

    fn ends_with_jump(&self, body: &[StmtId]) -> bool {
        let Some(last) = body.last() else {
            return false;
        };
        matches!(
            self.ast().stmt(*last).kind,
            StmtKind::Break(_) | StmtKind::Continue(_) | StmtKind::Return(_)
        ) || matches!(&self.ast().stmt(*last).kind, StmtKind::Expression(e) if matches!(self.expr(*e).kind, ast::ExprKind::Throw(_) | ast::ExprKind::Rethrow))
    }

    pub fn is_simple_const_pattern(&self, p: ast::PatternId) -> bool {
        match &self.ast().pattern(p).kind {
            ast::PatternKind::Constant(e) => self.is_switchable_const(*e),
            ast::PatternKind::Or(a, b) => {
                self.is_simple_const_pattern(*a) && self.is_simple_const_pattern(*b)
            }
            ast::PatternKind::Parenthesized(p) => self.is_simple_const_pattern(*p),
            _ => false,
        }
    }

    fn is_switchable_const(&self, e: ExprId) -> bool {
        match &self.expr(e).kind {
            ast::ExprKind::Int(_) | ast::ExprKind::Bool(_) | ast::ExprKind::Null => true,
            ast::ExprKind::String(lit) => lit.constant_value().is_some(),
            ast::ExprKind::Identifier(_) | ast::ExprKind::Property { .. } => true,
            ast::ExprKind::Unary {
                op: ast::UnaryOp::Neg,
                operand,
            } => self.is_switchable_const(*operand),
            ast::ExprKind::Parenthesized(i) => self.is_switchable_const(*i),
            _ => false,
        }
    }

    fn const_pattern_values(&mut self, p: ast::PatternId) -> Vec<String> {
        match &self.ast().pattern(p).kind {
            ast::PatternKind::Constant(e) => vec![self.emit_expr(*e, None).0.code],
            ast::PatternKind::Or(a, b) => {
                let mut v = self.const_pattern_values(*a);
                v.extend(self.const_pattern_values(*b));
                v
            }
            ast::PatternKind::Parenthesized(p) => self.const_pattern_values(*p),
            _ => vec![],
        }
    }

    fn emit_try(&mut self, body: StmtId, catches: &[ast::CatchClause], finally_: Option<StmtId>) {
        self.w.open("try {");
        self.push_scope();
        self.emit_stmt(body);
        self.pop_scope();
        self.w.close("}");
        if !catches.is_empty() {
            let e = format!("t$e{}", self.unique);
            self.unique += 1;
            crate::open_block!(self.w, "catch ({e}) {{");
            let ex = format!("t$ex{}", self.unique);
            let st = format!("t$st{}", self.unique);
            self.unique += 1;
            crate::emit_line!(self.w, "let {ex} = dart.getThrown({e});");
            if catches.iter().any(|c| c.stack_trace.is_some()) {
                crate::emit_line!(self.w, "let {st} = dart.stackTrace({e});");
            }
            let mut first = true;
            let mut catch_all = false;
            for c in catches {
                let on_ty = c.on_type.map(|t| self.resolve_type(t));
                let is_all = match &on_ty {
                    None => true,
                    Some(Ty::Dynamic) => true,
                    Some(Ty::Iface {
                        class, nullable, ..
                    }) => Some(*class) == self.ctx.object && *nullable,
                    _ => false,
                };
                let cond = match &on_ty {
                    Some(t) if !is_all => self.is_test(&Js::prim(ex.clone()), t).code,
                    _ => "true".to_string(),
                };
                let head = if first {
                    format!("if ({cond}) {{")
                } else {
                    format!("else if ({cond}) {{")
                };
                first = false;
                self.w.open(&head);
                self.push_scope();
                if let Some(n) = c.exception {
                    let ty = match &on_ty {
                        Some(t) if !is_all => t.clone(),
                        _ => self.ctx.t_object(),
                    };
                    let jsn = self.declare(n.sym, ty);
                    crate::emit_line!(self.w, "let {jsn} = {ex};");
                }
                if let Some(n) = c.stack_trace {
                    let ty = self.ctx.stack_trace.map(Ty::iface).unwrap_or(Ty::Dynamic);
                    let jsn = self.declare(n.sym, ty);
                    crate::emit_line!(self.w, "let {jsn} = {st};");
                }
                self.rethrow_var.push(e.clone());
                self.emit_stmt(c.body);
                self.rethrow_var.pop();
                self.pop_scope();
                self.w.close("}");
                if is_all {
                    catch_all = true;
                    break;
                }
            }
            if !catch_all {
                crate::emit_line!(self.w, "else throw {e};");
            }
            self.w.close("}");
        }
        if let Some(f) = finally_ {
            self.w.open("finally {");
            self.push_scope();
            self.emit_stmt(f);
            self.pop_scope();
            self.w.close("}");
        }
    }

    /// True if the body contains `await`/`yield` (to decide on a generator).
    pub fn member_lookup_this(&self, name: &str, setter: bool) -> Option<Member> {
        let c = self.class?;
        self.ctx.lookup_member(&self.ctx.this_ty(c), name, setter)
    }

    pub fn member_of(&self, m: &Member) -> (String, Ty) {
        let _ = m;
        (String::new(), Ty::Dynamic)
    }
}

/// JS member name: `[]` → `_get`, `[]=` → `_set`, `==` → `_equals`,
/// `unary-` → `_negate`, and `constructor`/`prototype` → `_constructor`/
/// `_prototype`.
///
/// This is DDC's `memberNameForDartMember` (`compiler/js_names.dart:411`).
/// The last two are mandatory, not cosmetic: `get constructor()` inside a
/// `class` is a syntax error in JavaScript ("Class constructor may not be an
/// accessor"), and `prototype` would collide with the object's own. A Dart field
/// named `constructor` exists in real code — `package:json_annotation` has one.
pub fn js_member_name(name: &str) -> String {
    js_member_name_ext(name, false)
}

/// Like [`js_member_name`], but `external` keeps the raw name: there the JS
/// member is intentional (DDC uses the same rule, with `isExternal`).
pub fn js_member_name_ext(name: &str, external: bool) -> String {
    match name {
        "[]" => "_get".into(),
        "[]=" => "_set".into(),
        "==" => "_equals".into(),
        "unary-" => "_negate".into(),
        "constructor" | "prototype" if !external => format!("_{name}"),
        _ => name.to_string(),
    }
}

/// Property name of a top-level element (the same as in Dart).
pub fn top_level_name(name: &str) -> String {
    name.to_string()
}

/// Static member name (`as`, `name`, `prototype` get a `_`).
pub fn static_member_name(name: &str) -> String {
    match name {
        "as" | "name" | "prototype" => format!("{name}_"),
        _ if name.ends_with('_') => format!("{name}_"),
        _ => name.to_string(),
    }
}

pub fn paren_if_needed(s: &str) -> String {
    if s == "true"
        || s.chars()
            .all(|c| c.is_alphanumeric() || c == '$' || c == '_')
    {
        s.to_string()
    } else {
        format!("({s})")
    }
}

/// A test against a top type: `true`, after evaluating the operand when it
/// may have effects (`f() is Object?` still calls `f`).
fn always_true(value: &Js) -> Js {
    if crate::expr::is_simple(&value.code) {
        Js::prim("true")
    } else {
        Js::new(
            format!("({}, true)", value.at(P_ASSIGN)),
            crate::js::P_PRIMARY,
        )
    }
}
