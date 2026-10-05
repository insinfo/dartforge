//! O `FfiVerifier` do analyzer 3.6.2
//! (`analyzer/lib/src/generated/ffi_verifier.dart`), inteiro: os 48 códigos
//! de `FfiCode` sobre o uso das APIs de `dart:ffi`.
//!
//! O percurso é o do `RecursiveAstVisitor` original:
//! * a classe (`visitClassDeclaration`, só a `ClassDeclaration`: o
//!   `class A = B with C;` não passa): a superclasse de `dart:ffi` (`Struct`,
//!   `Union`, `AbiSpecificInteger`) e os subtipos de composto em `extends`,
//!   `implements` e `with`; dentro de um composto, os campos de instância
//!   (`_validateFieldsInCompound`);
//! * o `@Native` de campos, variáveis de topo, funções (de topo e locais) e
//!   métodos (`_checkFfiNative`); o `@DefaultAsset` da diretiva `library`;
//! * as invocações: `Pointer.fromFunction`, `Pointer.elementAt`,
//!   `Struct.create`/`Union.create`, `Native.addressOf`, `asFunction`,
//!   `lookupFunction`, `sizeOf`, o `call` de `AllocatorAlloc`, `ref`, `[]`,
//!   `address` e a criação de `Struct`/`Union`/`NativeCallable`.
//!
//! `visitIndexExpression` não chama o `super`: nada dentro de uma expressão
//! de índice (o alvo, o índice, closures neles) é visitado, e o porte segue
//! essa regra.
//!
//! O valor de uma anotação de `dart:ffi` (`computeConstantValue`) é o objeto
//! que o construtor do SDK 3.6.2 monta: `Native({assetId, isLeaf = false,
//! symbol})`, `Packed(memberAlignment)`, os quatro de `Array` (que
//! redirecionam a `_ArraySize`), `AbiSpecificIntegerMapping(mapping)` e
//! `DefaultAsset(id)`. O motor de constantes não executa construtores de
//! bibliotecas que não foram inferidas (as do SDK); os campos são montados
//! aqui a partir dos argumentos, que o motor avalia, pelo texto daqueles
//! construtores. O tipo do valor (`Native<T>`) é o da instanciação que a
//! inferência registrou para a lista de argumentos da anotação.
//!
//! `strict-casts` não existe no motor: o `isAssignableTo` do
//! `Native.addressOf` é o sem `strict-casts`.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::constantes::avaliador::{Constante, Ctx, Motor};
use crate::constantes::valor::{Estado, Valor};
use crate::exibicao::{Arg, Exibidor};
use crate::resolve::OutlineTypes;
use crate::resolved::{BodyTypes, MemberRef, Resolved, UnitBodyTypes};
use crate::subtyping::{SubtypeEnv, is_subtype};
use crate::table::{CoreTypes, Type, TypeId, TypeTable};
use dartforge_diagnostics::codigos::ffi as cf;
use dartforge_diagnostics::{Codigo, Diagnostic, Span};
use dartforge_elements::model::{
    ClassId, ClassKind, Element, ExtensionId, FunctionElementId, FunctionKind, FunctionRef, Library, LibraryId, Program, UnitId, VariableId,
    VariableRef,
};
use dartforge_frontend::ast::{self, CollectionElement, DeclKind, DirectiveKind, ExprId, ExprKind, MemberKind, ParameterKind, StmtKind, TypeKind};
use dartforge_frontend::pais::{Pai, Pais};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};

/// `_primitiveIntegerNativeTypesFixedSize`.
const INTEIROS_FIXOS: [&str; 8] = ["Int8", "Int16", "Int32", "Int64", "Uint8", "Uint16", "Uint32", "Uint64"];
/// `_addressOfCompoundExtensionNames`.
const ENDERECO_DE_COMPOSTO: [&str; 3] = ["ArrayAddress", "StructAddress", "UnionAddress"];
/// `_addressOfPrimitiveExtensionNames`.
const ENDERECO_DE_PRIMITIVO: [&str; 3] = ["BoolAddress", "DoubleAddress", "IntAddress"];
/// `_addressOfTypedDataExtensionNames`.
const ENDERECO_DE_TYPED_DATA: [&str; 10] = [
    "Float32ListAddress",
    "Float64ListAddress",
    "Int16ListAddress",
    "Int32ListAddress",
    "Int64ListAddress",
    "Int8ListAddress",
    "Uint16ListAddress",
    "Uint32ListAddress",
    "Uint64ListAddress",
    "Uint8ListAddress",
];

/// `_PrimitiveDartType`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Primitivo {
    Double,
    Int,
    Bool,
    Void,
    Handle,
    Nenhum,
}

/// `_FfiTypeCheckDirection`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Direcao {
    NativoParaDart,
    DartParaNativo,
}

impl Direcao {
    fn inversa(self) -> Direcao {
        match self {
            Direcao::NativoParaDart => Direcao::DartParaNativo,
            Direcao::DartParaNativo => Direcao::NativoParaDart,
        }
    }
}

/// Os `allow*` de `_isValidFfiNativeType`.
#[derive(Clone, Copy, Default)]
struct Permite {
    void_: bool,
    struct_vazio: bool,
    array: bool,
    handle: bool,
    opaco: bool,
}

/// O valor de uma anotação (`DartObject`), com os campos que o verificador
/// consulta (`getField`).
struct Instancia {
    /// A classe do tipo do valor.
    classe: ClassId,
    /// O tipo do valor (`Native<T>`).
    tipo: TypeId,
    /// `isLeaf`.
    is_leaf: Option<bool>,
    /// `memberAlignment`.
    alinhamento: Option<i64>,
    /// `dimensions` (`toListValue` dos `toIntValue`).
    lista: Option<Vec<i64>>,
    /// `dimension1` a `dimension5`.
    dimensoes: [Option<i64>; 5],
    /// `variableLength`.
    variavel: Option<bool>,
    /// Os tipos dos valores de `mapping`.
    mapa: Option<Vec<TypeId>>,
}

/// O elemento a que uma anotação se refere (`Annotation.element`).
#[derive(Clone, Copy)]
enum AlvoDaAnotacao {
    Construtor(ClassId, FunctionElementId),
    Classe(ClassId),
    Variavel(VariableId),
    Outro,
}

/// O nó de uma declaração que o `_checkFfiNative` examina.
#[derive(Clone, Copy)]
enum No {
    Funcao(ast::FunctionId),
    VariavelDeTopo(ast::DeclId, usize),
    Campo(ast::MemberId, usize),
    /// Função local: o tipo dela.
    Local(TypeId),
}

/// O elemento declarado (`declarationElement`).
#[derive(Clone, Copy)]
enum Declaracao {
    Variavel(VariableId),
    Funcao(FunctionElementId),
    Local(TypeId),
}

/// Uma referência constante (`ConstVariableElement`).
enum RefConstante {
    Variavel(VariableId),
    Local,
    Parametro(Option<ExprId>),
}

/// A tabela de tipos, solta até o motor ser preciso.
enum Tabela<'a> {
    Livre(&'a mut TypeTable),
    Motor(Box<Motor<'a>>),
    Vazia,
}

#[derive(Default)]
struct Indices {
    funcoes: HashMap<u32, FunctionElementId>,
    topo: HashMap<(u32, usize), VariableId>,
    campos: HashMap<(u32, usize), VariableId>,
    classes: HashMap<u32, ClassId>,
}

struct V<'a> {
    program: &'a Program,
    interner: &'a Interner,
    core: &'a CoreTypes,
    outline: &'a OutlineTypes,
    corpos: &'a BodyTypes,
    inferidas: &'a HashSet<LibraryId>,
    corpo: &'a UnitBodyTypes,
    u: UnitId,
    a: &'a ast::Ast,
    fonte: &'a str,
    ffi: LibraryId,
    typed_data: Option<LibraryId>,
    t: Tabela<'a>,
    out: Vec<Diagnostic>,
    cascatas: HashMap<ExprId, ExprId>,
    pais: Pais,
    indices: Option<Indices>,
    /// Os trechos das expressões de índice (`visitIndexExpression` não
    /// visita os filhos).
    indices_de_colchete: Vec<Span>,
}

fn nome_da_biblioteca(interner: &Interner, l: &Library) -> String {
    l.name.as_ref().map(|n| n.iter().map(|s| interner.resolve(*s)).collect::<Vec<_>>().join(".")).unwrap_or_default()
}

/// Os relatos do `FfiVerifier` na unidade `u`.
#[allow(clippy::too_many_arguments)]
pub fn verificar<'a>(
    program: &'a Program,
    interner: &'a Interner,
    table: &'a mut TypeTable,
    core: &'a CoreTypes,
    outline: &'a OutlineTypes,
    corpos: &'a BodyTypes,
    inferidas: &'a HashSet<LibraryId>,
    u: UnitId,
) -> Vec<Diagnostic> {
    let Some(ffi) = program.libraries.iter().position(|l| nome_da_biblioteca(interner, l) == "dart.ffi").map(|i| LibraryId(i as u32)) else {
        return Vec::new();
    };
    let typed_data = program.libraries.iter().position(|l| nome_da_biblioteca(interner, l) == "dart.typed_data").map(|i| LibraryId(i as u32));
    let Some(corpo) = corpos.units.get(u.0 as usize) else { return Vec::new() };
    let unidade = program.unit(u);
    let a = &unidade.ast;
    let indices_de_colchete = a.exprs.iter().filter(|e| matches!(e.kind, ExprKind::Index { .. })).map(|e| e.span).collect();
    let mut v = V {
        program,
        interner,
        core,
        outline,
        corpos,
        inferidas,
        corpo,
        u,
        a,
        fonte: &unidade.source,
        ffi,
        typed_data,
        t: Tabela::Livre(table),
        out: Vec::new(),
        cascatas: crate::fase_catch_error::alvos_de_cascata(a),
        pais: crate::lints_tipados::pais_da_unidade(program, u),
        indices: None,
        indices_de_colchete,
    };
    v.declaracoes();
    v.expressoes();
    v.out
}

impl<'a> V<'a> {
    // -- Infraestrutura --------------------------------------------------------

    fn tabela(&mut self) -> &mut TypeTable {
        match &mut self.t {
            Tabela::Livre(t) => &mut **t,
            Tabela::Motor(m) => &mut *m.table,
            Tabela::Vazia => unreachable!("tabela emprestada"),
        }
    }

    fn tabela_ref(&self) -> &TypeTable {
        match &self.t {
            Tabela::Livre(t) => &**t,
            Tabela::Motor(m) => &*m.table,
            Tabela::Vazia => unreachable!("tabela emprestada"),
        }
    }

    fn motor(&mut self) -> &mut Motor<'a> {
        if let Tabela::Livre(_) = self.t
            && let Tabela::Livre(t) = std::mem::replace(&mut self.t, Tabela::Vazia)
        {
            self.t = Tabela::Motor(Box::new(Motor::novo(self.program, self.interner, t, self.core, self.outline, self.corpos, self.inferidas)));
        }
        match &mut self.t {
            Tabela::Motor(m) => &mut **m,
            _ => unreachable!("motor criado acima"),
        }
    }

    fn tipo(&self, t: TypeId) -> Type {
        self.tabela_ref().get(t).clone()
    }

    fn sub(&mut self, a: TypeId, b: TypeId) -> bool {
        let outline = self.outline;
        let core = self.core;
        let mut env = SubtypeEnv::new(self.tabela(), &outline.hierarchy, core);
        is_subtype(a, b, &mut env)
    }

    fn relatar(&mut self, codigo: Codigo, span: Span, args: &[Arg<'_>]) {
        let (textos, contexto) = Exibidor { table: self.tabela_ref(), interner: self.interner, program: self.program }.argumentos_e_contexto(args);
        let mut d = Diagnostic::com_codigo(codigo, span, textos);
        d.contexto.extend(contexto);
        self.out.push(d);
    }

    fn nome(&self, s: SymbolId) -> &'a str {
        self.interner.resolve(s)
    }

    /// Dentro de uma expressão de índice (sem ser ela).
    fn dentro_de_indice(&self, s: Span) -> bool {
        self.indices_de_colchete.iter().any(|r| r.start <= s.start && s.end <= r.end && (r.start, r.end) != (s.start, s.end))
    }

    fn span(&self, e: ExprId) -> Span {
        self.a.expr(e).span
    }

    /// O nó do argumento (`NamedExpression` inteira, se nomeado).
    fn span_do_argumento(&self, x: &ast::Argument) -> Span {
        let s = self.span(x.value);
        match x.name {
            Some(n) => Span { start: n.span.start, end: s.end },
            None => s,
        }
    }

    /// `Annotation.name` (o identificador, prefixado ou não).
    fn nome_da_anotacao(&self, m: &ast::Annotation) -> Span {
        match (m.name.first(), m.name.get(1).or(m.name.first())) {
            (Some(a), Some(b)) => Span { start: a.span.start, end: b.span.end },
            _ => m.span,
        }
    }

    fn alvo_real(&self, e: ExprId) -> ExprId {
        self.cascatas.get(&e).copied().unwrap_or(e)
    }

    fn pai(&self, e: ExprId) -> Option<ExprId> {
        match self.pais.pai(e) {
            Pai::Expr(p) => Some(p),
            _ => None,
        }
    }

    fn tipo_escrito(&self, t: ast::TypeId) -> Option<TypeId> {
        self.corpo.tipos_de_anotacoes.get(&t).or_else(|| self.outline.tipos_escritos.get(&(self.u, t))).copied()
    }

    /// `typeArgumentTypes` da invocação cuja lista de argumentos é `args`.
    fn instanciacao(&self, args: &ast::Arguments) -> Vec<TypeId> {
        self.corpo.instanciacao(args.span.start).map(|v| v.to_vec()).unwrap_or_default()
    }

    fn indices(&mut self) -> &Indices {
        if self.indices.is_none() {
            let u = self.u;
            let mut ix = Indices::default();
            for (i, f) in self.program.functions.iter().enumerate() {
                if let FunctionRef::Function { unit, function } = f.node
                    && unit == u
                {
                    ix.funcoes.insert(function.0, FunctionElementId(i as u32));
                }
            }
            for (i, v) in self.program.variables.iter().enumerate() {
                match v.node {
                    VariableRef::TopLevel { unit, decl, index } if unit == u => {
                        ix.topo.insert((decl.0, index), VariableId(i as u32));
                    }
                    VariableRef::Field { unit, member, index } if unit == u => {
                        ix.campos.insert((member.0, index), VariableId(i as u32));
                    }
                    _ => {}
                }
            }
            for (i, c) in self.program.classes.iter().enumerate() {
                if let Some(d) = c.decl
                    && d.unit == u
                {
                    ix.classes.insert(d.decl.0, ClassId(i as u32));
                }
            }
            self.indices = Some(ix);
        }
        self.indices.as_ref().expect("índices montados")
    }

    fn declaracao(&mut self, no: No) -> Option<Declaracao> {
        Some(match no {
            No::Funcao(f) => Declaracao::Funcao(*self.indices().funcoes.get(&f.0)?),
            No::VariavelDeTopo(d, i) => Declaracao::Variavel(*self.indices().topo.get(&(d.0, i))?),
            No::Campo(m, i) => Declaracao::Variavel(*self.indices().campos.get(&(m.0, i))?),
            No::Local(t) => Declaracao::Local(t),
        })
    }

    // -- Elementos e tipos de `dart:ffi` --------------------------------------

    fn de_ffi(&self, c: ClassId) -> bool {
        self.program.class(c).library == self.ffi
    }

    fn classe_ffi(&self, c: ClassId, nome: &str) -> bool {
        let k = self.program.class(c);
        k.library == self.ffi && self.nome(k.name) == nome
    }

    fn classe_ffi_por_nome(&self, nome: &str) -> Option<ClassId> {
        let s = self.interner.lookup(nome)?;
        match self.program.library(self.ffi).declared.get(&s)?.getter? {
            Element::Class(c) => Some(c),
            _ => None,
        }
    }

    fn extensao_ffi(&self, x: ExtensionId, nomes: &[&str]) -> bool {
        let e = self.program.extension(x);
        e.library == self.ffi && e.name.is_some_and(|n| nomes.contains(&self.nome(n)))
    }

    fn composto(&self, c: ClassId) -> bool {
        self.classe_ffi(c, "Struct") || self.classe_ffi(c, "Union")
    }

    /// O elemento de um `InterfaceType` (classe, enum, mixin, tipo de
    /// extensão, `Null`).
    fn classe_do_tipo(&self, t: TypeId) -> Option<ClassId> {
        match self.tabela_ref().get(t) {
            Type::Interface { class, .. } => Some(*class),
            Type::ExtensionType { decl, .. } => Some(*decl),
            Type::Null => self.core.null_class,
            _ => None,
        }
    }

    /// `dartType is InterfaceType` (no analyzer, `Null` e `FutureOr` também).
    fn e_interface(&self, t: TypeId) -> bool {
        matches!(
            self.tabela_ref().get(t),
            Type::Interface { .. } | Type::ExtensionType { .. } | Type::Null | Type::FutureOr { .. }
        )
    }

    fn argumentos_do_tipo(&self, t: TypeId) -> Vec<TypeId> {
        match self.tabela_ref().get(t) {
            Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.to_vec(),
            Type::FutureOr { arg, .. } => vec![*arg],
            _ => Vec::new(),
        }
    }

    fn e_ffi_t(&self, t: TypeId, nome: &str) -> bool {
        matches!(self.tabela_ref().get(t), Type::Interface { .. }) && self.classe_do_tipo(t).is_some_and(|c| self.classe_ffi(c, nome))
    }

    /// `element.supertype` do tipo.
    fn superclasse(&self, t: TypeId) -> Option<ClassId> {
        self.classe_do_tipo(t).and_then(|c| self.program.class(c).supertype_class)
    }

    fn subtipo_de_composto(&self, t: TypeId) -> bool {
        self.superclasse(t).is_some_and(|s| self.composto(s))
    }

    fn subtipo_de_abi(&self, t: TypeId) -> bool {
        self.superclasse(t).is_some_and(|s| self.classe_ffi(s, "AbiSpecificInteger"))
    }

    fn subtipo_de_opaco(&self, t: TypeId) -> bool {
        self.superclasse(t).is_some_and(|s| self.classe_ffi(s, "Opaque"))
    }

    fn e_core(&self, t: TypeId, c: Option<ClassId>) -> bool {
        matches!(self.tabela_ref().get(t), Type::Interface { class, .. } if Some(*class) == c)
    }

    fn e_int(&self, t: TypeId) -> bool {
        self.e_core(t, self.core.int_class)
    }

    fn e_double(&self, t: TypeId) -> bool {
        self.e_core(t, self.core.double_class)
    }

    fn e_bool(&self, t: TypeId) -> bool {
        self.e_core(t, self.core.bool_class)
    }

    fn e_void(&self, t: TypeId) -> bool {
        matches!(self.tabela_ref().get(t), Type::Void)
    }

    fn e_parametro(&self, t: TypeId) -> bool {
        matches!(self.tabela_ref().get(t), Type::TypeParameter { .. })
    }

    /// `isTypedDataClass`.
    fn classe_de_typed_data(&self, c: ClassId) -> bool {
        Some(self.program.class(c).library) == self.typed_data
    }

    /// `isTypedData`: só as listas que correspondem a um `Pointer`.
    fn e_typed_data(&self, t: TypeId) -> bool {
        let Some(c) = self.classe_do_tipo(t) else { return false };
        if !self.classe_de_typed_data(c) {
            return false;
        }
        let nome = self.nome(self.program.class(c).name);
        if !nome.ends_with("List") {
            return false;
        }
        if nome == "Float32List" || nome == "Float64List" {
            return true;
        }
        INTEIROS_FIXOS.contains(&nome.replace("List", "").as_str())
    }

    /// `_primitiveNativeType`.
    fn primitivo(&self, t: TypeId) -> Primitivo {
        let Some(c) = self.classe_do_tipo(t) else { return Primitivo::Nenhum };
        if !self.de_ffi(c) {
            return Primitivo::Nenhum;
        }
        match self.nome(self.program.class(c).name) {
            "Int8" | "Int16" | "Int32" | "Int64" | "Uint8" | "Uint16" | "Uint32" | "Uint64" | "IntPtr" => Primitivo::Int,
            "Float" | "Double" => Primitivo::Double,
            "Bool" => Primitivo::Bool,
            "Void" => Primitivo::Void,
            "Handle" => Primitivo::Handle,
            _ => Primitivo::Nenhum,
        }
    }

    /// `arrayDimensions`.
    fn dimensoes_de_array(&self, t: TypeId) -> usize {
        let mut atual = t;
        let mut n = 0;
        while self.e_ffi_t(atual, "Array") {
            n += 1;
            match self.argumentos_do_tipo(atual).as_slice() {
                [x] => atual = *x,
                _ => break,
            }
        }
        n
    }

    fn tipo_da_variavel(&self, v: VariableId) -> TypeId {
        self.outline.variables.get(v.0 as usize).and_then(|d| d.declared_type.or(d.inferred)).unwrap_or(self.core.dynamic_)
    }

    /// Os tipos dos campos da classe (`InterfaceElement.fields`: os
    /// declarados e os sintéticos dos acessores explícitos).
    fn tipos_dos_campos(&self, c: ClassId) -> Vec<TypeId> {
        let k = self.program.class(c);
        let mut v = Vec::new();
        let mut nomes: HashSet<SymbolId> = HashSet::new();
        for &f in &k.fields {
            nomes.insert(self.program.variable(f).name);
            v.push(self.tipo_da_variavel(f));
        }
        let membros = || k.instance_members.values().chain(k.static_members.values()).copied();
        for f in membros() {
            let g = self.program.function(f);
            if g.variable.is_none() && g.kind == FunctionKind::Getter && nomes.insert(g.name) {
                v.push(self.outline.functions.get(f.0 as usize).map_or(self.core.dynamic_, |d| d.return_type));
            }
        }
        for f in membros() {
            let g = self.program.function(f);
            if g.variable.is_none() && g.kind == FunctionKind::Setter && nomes.insert(g.name) {
                let t = self.outline.functions.get(f.0 as usize).and_then(|d| d.parameters.first()).map_or(self.core.dynamic_, |p| p.ty);
                v.push(t);
            }
        }
        v
    }

    /// `isEmptyStruct`.
    fn struct_vazio(&self, c: ClassId) -> bool {
        for t in self.tipos_dos_campos(c) {
            if self.e_int(t)
                || self.e_double(t)
                || self.e_bool(t)
                || self.e_ffi_t(t, "Pointer")
                || self.subtipo_de_composto(t)
                || self.e_ffi_t(t, "Array")
            {
                return false;
            }
        }
        true
    }

    /// `_isSized`.
    fn dimensionado(&self, t: TypeId) -> bool {
        match self.primitivo(t) {
            Primitivo::Double | Primitivo::Int | Primitivo::Bool => true,
            Primitivo::Void | Primitivo::Handle => false,
            Primitivo::Nenhum => {
                self.subtipo_de_composto(t) || self.e_ffi_t(t, "Pointer") || self.e_ffi_t(t, "Array") || self.subtipo_de_abi(t)
            }
        }
    }

    /// `flattenVarArgs`.
    fn achatar(&self, v: &[TypeId]) -> Vec<TypeId> {
        let Some(&ultimo) = v.last() else { return Vec::new() };
        if !self.e_ffi_t(ultimo, "VarArgs") {
            return v.to_vec();
        }
        let args = self.argumentos_do_tipo(ultimo);
        let [r] = args.as_slice() else { return v.to_vec() };
        match self.tabela_ref().get(*r) {
            Type::Record { positional, named, .. } if named.is_empty() => {
                let mut s = v[..v.len() - 1].to_vec();
                s.extend(positional.iter().copied());
                s
            }
            _ => v.to_vec(),
        }
    }

    /// `_isValidFfiNativeFunctionType`.
    fn funcao_nativa_valida(&self, t: TypeId) -> bool {
        let Type::Function { positional, optional, named, ret, .. } = self.tipo(t) else { return false };
        if !named.is_empty() || !optional.is_empty() {
            return false;
        }
        if !self.valido_nativo(ret, Permite { void_: true, handle: true, ..Permite::default() }) {
            return false;
        }
        self.achatar(&positional).into_iter().all(|x| self.valido_nativo(x, Permite { handle: true, ..Permite::default() }))
    }

    /// `_isValidFfiNativeType`.
    fn valido_nativo(&self, t: TypeId, p: Permite) -> bool {
        if matches!(self.tabela_ref().get(t), Type::Function { .. }) {
            return self.funcao_nativa_valida(t);
        }
        if !self.e_interface(t) {
            return false;
        }
        match self.primitivo(t) {
            Primitivo::Void => return p.void_,
            Primitivo::Handle => return p.handle,
            Primitivo::Double | Primitivo::Int | Primitivo::Bool => return true,
            Primitivo::Nenhum => {}
        }
        let args = self.argumentos_do_tipo(t);
        if self.e_ffi_t(t, "NativeFunction") {
            return matches!(args.as_slice(), [x] if self.funcao_nativa_valida(*x));
        }
        if self.e_ffi_t(t, "Pointer") {
            let [x] = args.as_slice() else { return false };
            let tudo = Permite { void_: true, struct_vazio: true, handle: true, opaco: true, array: false };
            return self.valido_nativo(*x, tudo) || self.subtipo_de_composto(*x) || self.e_ffi_t(*x, "NativeType");
        }
        if self.subtipo_de_composto(t) {
            if !p.struct_vazio
                && let Some(c) = self.classe_do_tipo(t)
                && self.struct_vazio(c)
            {
                return false;
            }
            return true;
        }
        if self.e_ffi_t(t, "Opaque") {
            return p.opaco;
        }
        if self.subtipo_de_opaco(t) || self.subtipo_de_abi(t) {
            return true;
        }
        if p.array && self.e_ffi_t(t, "Array") {
            return matches!(args.as_slice(), [x] if self.valido_nativo(*x, Permite::default()));
        }
        false
    }

    /// `_extendsNativeFieldWrapperClass1`: o texto do tipo e dos supertipos.
    fn estende_nfwc1(&self, t: TypeId) -> bool {
        if self.tabela_ref().format_sem_alias(t, self.interner, self.program) == "NativeFieldWrapperClass1" {
            return true;
        }
        let inicio = self.classe_do_tipo(t).and_then(|c| self.program.class(c).supertype_class);
        self.cadeia_tem_nfwc1(inicio)
    }

    fn cadeia_tem_nfwc1(&self, mut atual: Option<ClassId>) -> bool {
        let mut passos = 0;
        while let Some(c) = atual {
            passos += 1;
            if passos > 256 {
                break;
            }
            let k = self.program.class(c);
            if self.nome(k.name) == "NativeFieldWrapperClass1" && k.type_params.is_empty() {
                return true;
            }
            atual = k.supertype_class;
        }
        false
    }

    /// `_extendsNativeFieldWrapperClass1(cls.thisType)`.
    fn classe_estende_nfwc1(&self, c: ClassId) -> bool {
        let k = self.program.class(c);
        (self.nome(k.name) == "NativeFieldWrapperClass1" && k.type_params.is_empty()) || self.cadeia_tem_nfwc1(k.supertype_class)
    }

    /// `_isValidTypedData`.
    fn typed_data_valido(&self, nativo: TypeId, dart: TypeId) -> bool {
        if !self.e_ffi_t(nativo, "Pointer") {
            return false;
        }
        let args = self.argumentos_do_tipo(nativo);
        let [elemento] = args.as_slice() else { return false };
        let nome_do_elemento = match self.tabela_ref().get(*elemento) {
            Type::TypeParameter { param, .. } => Some(self.nome(self.tabela_ref().param(*param).name)),
            _ => self.classe_do_tipo(*elemento).map(|c| self.nome(self.program.class(c).name)),
        };
        let Some(dc) = self.classe_do_tipo(dart) else { return false };
        if !self.classe_de_typed_data(dc) {
            return false;
        }
        let nome_dart = self.nome(self.program.class(dc).name);
        match nome_do_elemento {
            Some("Float") => nome_dart == "Float32List",
            Some("Double") => nome_dart == "Float64List",
            Some(n) => INTEIROS_FIXOS.contains(&n) && nome_dart == format!("{n}List"),
            None => false,
        }
    }

    /// `_validateCompatibleNativeType`.
    fn compat_nativo(&mut self, dir: Direcao, dart: TypeId, nativo: TypeId, nfw: bool, funcoes: bool) -> bool {
        let prim = self.primitivo(nativo);
        let super_abi = matches!(self.tabela_ref().get(nativo), Type::Interface { .. })
            && self.superclasse(nativo).is_some_and(|s| self.nome(self.program.class(s).name) == "AbiSpecificInteger");
        if prim == Primitivo::Int || super_abi {
            return self.e_int(dart);
        }
        if prim == Primitivo::Double {
            return self.e_double(dart);
        }
        if prim == Primitivo::Bool {
            return self.e_bool(dart);
        }
        if prim == Primitivo::Void {
            return dir == Direcao::DartParaNativo || self.e_void(dart);
        }
        if self.e_void(dart) {
            return false;
        }
        if prim == Primitivo::Handle {
            return match dir {
                Direcao::DartParaNativo => true,
                Direcao::NativoParaDart => {
                    let objeto = self.core.object;
                    self.sub(objeto, dart)
                }
            };
        }
        if self.e_interface(dart) && self.e_interface(nativo) {
            if nfw && self.estende_nfwc1(dart) {
                return self.e_ffi_t(nativo, "Pointer") && matches!(self.argumentos_do_tipo(nativo).as_slice(), [x] if self.primitivo(*x) == Primitivo::Void);
            }
            if self.typed_data_valido(nativo, dart) {
                return true;
            }
            return match dir {
                Direcao::DartParaNativo => self.sub(dart, nativo),
                Direcao::NativoParaDart => self.sub(nativo, dart),
            };
        }
        if funcoes && matches!(self.tabela_ref().get(dart), Type::Function { .. }) && self.e_ffi_t(nativo, "NativeFunction") {
            let Some(&f) = self.argumentos_do_tipo(nativo).first() else { return false };
            return self.compat_funcoes(dir, dart, f, nfw, false);
        }
        false
    }

    /// `_validateCompatibleFunctionTypes`.
    fn compat_funcoes(&mut self, dir: Direcao, dart: TypeId, nativo: TypeId, nfw: bool, permissivo: bool) -> bool {
        let (
            Type::Function { type_params: tpd, ret: rd, positional: pd, optional: od, named: nd, .. },
            Type::Function { type_params: tpn, ret: rn, positional: pn, optional: on, named: nn, .. },
        ) = (self.tipo(dart), self.tipo(nativo))
        else {
            return false;
        };
        let pn = self.achatar(&pn);
        if pd.len() != pn.len() {
            return false;
        }
        if !tpd.is_empty() || !tpn.is_empty() {
            return false;
        }
        if !nd.is_empty() || !od.is_empty() || !nn.is_empty() || !on.is_empty() {
            return false;
        }
        if permissivo {
            if !(self.compat_nativo(Direcao::NativoParaDart, rd, rn, false, false) || self.compat_nativo(Direcao::DartParaNativo, rd, rn, false, false)) {
                return false;
            }
        } else if !self.compat_nativo(dir, rd, rn, false, false) {
            return false;
        }
        for i in 0..pd.len() {
            if !self.compat_nativo(dir.inversa(), pd[i], pn[i], nfw, false) {
                return false;
            }
        }
        true
    }

    /// `isAssignableTo` sem `strict-casts`.
    fn atribuivel(&mut self, de: TypeId, para: TypeId) -> bool {
        if self.sub(de, para) || self.tabela_ref().e_invalido(de) {
            return true;
        }
        // O tear-off do `call`.
        if let Type::Interface { nullable: false, .. } = self.tabela_ref().get(de)
            && self.aceita_funcao(para)
            && let Some(chamada) = self.tipo_do_call(de)
            && self.atribuivel(chamada, para)
        {
            return true;
        }
        matches!(self.tabela_ref().get(de), Type::Dynamic)
    }

    /// `acceptsFunctionType`.
    fn aceita_funcao(&self, t: TypeId) -> bool {
        match self.tabela_ref().get(t) {
            Type::FutureOr { arg, .. } => self.aceita_funcao(*arg),
            Type::Function { .. } => true,
            Type::Interface { class, .. } => Some(*class) == self.core.function_class,
            _ => false,
        }
    }

    /// `getCallMethodType`: o método `call` da interface, instanciado.
    fn tipo_do_call(&mut self, t: TypeId) -> Option<TypeId> {
        let Type::Interface { class, args, .. } = self.tipo(t) else { return None };
        let call = self.interner.lookup("call")?;
        // A classe e as superclasses, na ordem da busca.
        let mut dono = Some(class);
        let mut achado = None;
        while let Some(k) = dono {
            if let Some(&f) = self.program.class(k).instance_members.get(&call)
                && self.program.function(f).kind == FunctionKind::Function
            {
                achado = Some((k, f));
                break;
            }
            dono = self.program.class(k).supertype_class;
        }
        let (k, f) = achado?;
        let assinatura = self.outline.functions.get(f.0 as usize)?.signature;
        // Os parâmetros de `k` vistos de `class`, depois os de `class` pelos argumentos.
        let formais_de_classe = self.outline.classes.get(class.0 as usize).map(|d| d.type_params.to_vec()).unwrap_or_default();
        let mut mapa_classe: HashMap<crate::table::TypeParamId, TypeId> = HashMap::new();
        for (p, a) in formais_de_classe.iter().zip(args.iter()) {
            mapa_classe.insert(*p, *a);
        }
        let mut sig = assinatura;
        if k != class {
            let visto = self.outline.hierarchy.get(class).and_then(|h| h.supertypes.get(&k).copied())?;
            let argumentos_k = self.argumentos_do_tipo(visto);
            let formais_k = self.outline.classes.get(k.0 as usize).map(|d| d.type_params.to_vec()).unwrap_or_default();
            let mapa_k: HashMap<crate::table::TypeParamId, TypeId> = formais_k.iter().copied().zip(argumentos_k).collect();
            sig = crate::ops::substitute(sig, &mapa_k, self.tabela());
        }
        Some(crate::ops::substitute(sig, &mapa_classe, self.tabela()))
    }

    // -- Anotações -----------------------------------------------------------

    /// `Annotation.element` da anotação `m`, escrita na unidade `u`.
    fn alvo_da_anotacao(&self, u: UnitId, m: &ast::Annotation) -> AlvoDaAnotacao {
        let a = &self.program.unit(u).ast;
        let Some(primeiro) = m.name.first() else { return AlvoDaAnotacao::Outro };
        if crate::anotacoes::sombreado(a, self.interner, m.span.start, primeiro.sym) {
            return AlvoDaAnotacao::Outro;
        }
        let p = self.program;
        let (alvo, membro) = match &m.name[..] {
            [c] => (p.lookup_na_unidade(u, c.sym).and_then(|b| b.getter), None),
            [x, y] => match p.lookup_na_unidade(u, x.sym).and_then(|b| b.getter) {
                Some(el @ Element::Class(_)) => (Some(el), Some(y.sym)),
                _ => (p.lookup_prefixed_na_unidade(u, x.sym, y.sym).and_then(|b| b.getter), None),
            },
            [x, c, n] => (p.lookup_prefixed_na_unidade(u, x.sym, c.sym).and_then(|b| b.getter), Some(n.sym)),
            _ => (None, None),
        };
        match (alvo, m.arguments.is_some(), membro) {
            (Some(Element::Class(c)), true, _) => {
                let chave = membro.or_else(|| self.interner.lookup(""));
                match chave.and_then(|k| p.class(c).constructors.get(&k)) {
                    Some(&f) => AlvoDaAnotacao::Construtor(c, f),
                    None => AlvoDaAnotacao::Outro,
                }
            }
            (Some(Element::Class(c)), false, None) => AlvoDaAnotacao::Classe(c),
            (Some(Element::Class(c)), false, Some(n)) => {
                let k = p.class(c);
                match k.enum_constants.iter().chain(k.fields.iter()).copied().find(|v| p.variable(*v).name == n && p.variable(*v).static_) {
                    Some(v) => AlvoDaAnotacao::Variavel(v),
                    None => AlvoDaAnotacao::Outro,
                }
            }
            (Some(Element::Variable(v)), false, None) => AlvoDaAnotacao::Variavel(v),
            (Some(Element::Function(g)), false, None) => match p.function(g).variable {
                Some(v) => AlvoDaAnotacao::Variavel(v),
                None => AlvoDaAnotacao::Outro,
            },
            _ => AlvoDaAnotacao::Outro,
        }
    }

    /// `annotation.element` é um construtor da classe `nome` de `dart:ffi`
    /// (`isArray`, `isPacked`, `isAbiSpecificIntegerMapping`).
    fn construtor_ffi(&self, u: UnitId, m: &ast::Annotation, nome: &str) -> bool {
        matches!(self.alvo_da_anotacao(u, m), AlvoDaAnotacao::Construtor(c, _) if self.classe_ffi(c, nome))
    }

    /// `annotation.element.ffiClass`.
    fn classe_ffi_da_anotacao(&self, u: UnitId, m: &ast::Annotation) -> Option<ClassId> {
        match self.alvo_da_anotacao(u, m) {
            AlvoDaAnotacao::Construtor(c, _) | AlvoDaAnotacao::Classe(c) if self.de_ffi(c) => Some(c),
            _ => None,
        }
    }

    /// `computeConstantValue` da anotação.
    fn instancia_da_anotacao(&mut self, u: UnitId, m: &'a ast::Annotation) -> Option<Instancia> {
        match self.alvo_da_anotacao(u, m) {
            AlvoDaAnotacao::Construtor(c, f) => {
                let args = m.arguments.as_ref()?;
                self.montar(u, c, f, args)
            }
            AlvoDaAnotacao::Variavel(v) => self.instancia_da_variavel(v),
            _ => None,
        }
    }

    /// O valor de uma variável constante usada como anotação: a criação do
    /// inicializador, ou o valor que o motor calcula.
    fn instancia_da_variavel(&mut self, v: VariableId) -> Option<Instancia> {
        let x = self.program.variable(v);
        if !x.const_ {
            return None;
        }
        let (unidade, inicial) = match x.node {
            VariableRef::TopLevel { unit, decl, index } => {
                let DeclKind::Variables(l) = &self.program.unit(unit).ast.decls[decl.0 as usize].kind else { return None };
                (unit, l.variables.get(index)?.initializer?)
            }
            VariableRef::Field { unit, member, index } => {
                let MemberKind::Field(l) = &self.program.unit(unit).ast.member(member).kind else { return None };
                (unit, l.variables.get(index)?.initializer?)
            }
            _ => return None,
        };
        let a = &self.program.unit(unidade).ast;
        let args = match &a.expr(inicial).kind {
            ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } => Some(&**arguments),
            _ => None,
        };
        let resolvido = self.corpos.units.get(unidade.0 as usize).and_then(|b| b.get_resolved(inicial));
        if let (Some(args), Some(Resolved::Constructor(f))) = (args, resolvido)
            && let Some(c) = self.program.function(*f).class
        {
            return self.montar(unidade, c, *f, args);
        }
        let valor = match self.motor().valor_de_variavel(v)? {
            Constante::Valor(x) => x,
            Constante::Invalida(_) => return None,
        };
        let classe = self.classe_do_tipo(valor.tipo)?;
        Some(Instancia {
            classe,
            tipo: valor.tipo,
            is_leaf: None,
            alinhamento: None,
            lista: None,
            dimensoes: [None; 5],
            variavel: None,
            mapa: None,
        })
    }

    /// O objeto que o construtor `f` da classe `c` monta com os argumentos
    /// `args` (escritos na unidade `u`).
    fn montar(&mut self, u: UnitId, c: ClassId, f: FunctionElementId, args: &'a ast::Arguments) -> Option<Instancia> {
        let n = self.program.class(c).type_params.len();
        let argumentos_de_tipo = match self.corpos.units.get(u.0 as usize).and_then(|b| b.instanciacao(args.span.start)) {
            Some(v) if v.len() == n => v.to_vec(),
            _ => vec![self.core.dynamic_; n],
        };
        let tipo = self.tabela().intern(Type::Interface { class: c, args: argumentos_de_tipo.into_boxed_slice(), nullable: false });
        let mut i = Instancia {
            classe: c,
            tipo,
            is_leaf: None,
            alinhamento: None,
            lista: None,
            dimensoes: [None; 5],
            variavel: None,
            mapa: None,
        };
        if !self.de_ffi(c) {
            return Some(i);
        }
        // Os argumentos, avaliados (um que não é constante deixa a anotação
        // sem valor).
        let lib = self.program.unit(u).library;
        let cx = Ctx::simples(u, lib);
        let mut posicionais: Vec<Valor> = Vec::new();
        let mut nomeados: Vec<(SymbolId, Valor)> = Vec::new();
        for x in args.args.iter() {
            let Constante::Valor(v) = self.motor().avaliar(&cx, x.value, true) else { return None };
            match x.name {
                Some(n) => nomeados.push((n.sym, v)),
                None => posicionais.push(v),
            }
        }
        let inteiro = |v: &Valor| match &v.estado {
            Estado::Int(x) => *x,
            _ => None,
        };
        let lista = |v: &Valor| match &v.estado {
            Estado::Lista { elementos, desconhecida: false, .. } => Some(elementos.iter().filter_map(inteiro).collect::<Vec<i64>>()),
            _ => None,
        };
        let classe = self.nome(self.program.class(c).name);
        let construtor = self.nome(self.program.function(f).name);
        match (classe, construtor) {
            ("Native", "") => {
                i.is_leaf = match nomeados.iter().find(|(n, _)| self.nome(*n) == "isLeaf") {
                    Some((_, v)) => match &v.estado {
                        Estado::Bool(b) => *b,
                        _ => None,
                    },
                    None => Some(false),
                };
            }
            ("Packed", "") => i.alinhamento = posicionais.first().and_then(inteiro),
            ("Array", "") => {
                for k in 0..5 {
                    i.dimensoes[k] = posicionais.get(k).and_then(inteiro);
                }
                i.variavel = Some(false);
            }
            ("Array", "multi") => {
                i.lista = posicionais.first().and_then(lista);
                i.variavel = Some(false);
            }
            ("Array", "variable") => {
                i.dimensoes[0] = Some(0);
                for k in 0..4 {
                    i.dimensoes[k + 1] = posicionais.get(k).and_then(inteiro);
                }
                i.variavel = Some(true);
            }
            ("Array", "variableMulti") => {
                i.lista = posicionais.first().and_then(lista);
                i.variavel = Some(true);
            }
            ("AbiSpecificIntegerMapping", "") => {
                i.mapa = posicionais.first().and_then(|v| match &v.estado {
                    Estado::Mapa { entradas, desconhecido: false } => Some(entradas.iter().map(|(_, x)| x.tipo).collect()),
                    _ => None,
                });
            }
            _ => {}
        }
        Some(i)
    }

    /// As anotações da declaração de uma função ou variável, com a unidade.
    fn anotacoes_do_elemento(&self, d: Declaracao) -> (UnitId, &'a [ast::Annotation]) {
        let p = self.program;
        match d {
            Declaracao::Funcao(f) => {
                let unidade = match p.function(f).node {
                    FunctionRef::Function { unit, .. } | FunctionRef::Constructor { unit, .. } => unit,
                    FunctionRef::None => return (self.u, &[]),
                };
                (unidade, crate::fase_resultado::anotacoes_da_funcao(p, f))
            }
            Declaracao::Variavel(v) => match p.variable(v).node {
                VariableRef::TopLevel { unit, decl, .. } => (unit, &p.unit(unit).ast.decls[decl.0 as usize].metadata[..]),
                VariableRef::Field { unit, member, .. } => (unit, &p.unit(unit).ast.member(member).metadata[..]),
                _ => (self.u, &[]),
            },
            Declaracao::Local(_) => (self.u, &[]),
        }
    }

    // -- Constantes ------------------------------------------------------------

    /// A referência constante de um identificador (`staticElement is
    /// ConstVariableElement`, direto ou pela variável do acessor).
    fn referencia_constante(&mut self, e: ExprId) -> Option<RefConstante> {
        let a = self.a;
        match &a.expr(e).kind {
            ExprKind::Identifier(_) => {}
            ExprKind::Property { target, .. } if matches!(a.expr(*target).kind, ExprKind::Identifier(_)) => {}
            _ => return None,
        }
        let r = self.corpo.get_resolved(e)?.clone();
        let p = self.program;
        let do_campo = |v: VariableId| {
            let x = p.variable(v);
            if x.const_ {
                return true;
            }
            // `constFieldsForFinalInstance`: campo `final` de instância com
            // inicializador, de classe ou enum.
            let de_classe = x.class.is_some_and(|c| matches!(p.class(c).kind, ClassKind::Class | ClassKind::Enum));
            let tem_inicializador = match x.node {
                VariableRef::Field { unit, member, index } => match &p.unit(unit).ast.member(member).kind {
                    MemberKind::Field(l) => l.variables.get(index).is_some_and(|y| y.initializer.is_some()),
                    _ => false,
                },
                _ => false,
            };
            x.final_ && !x.static_ && de_classe && tem_inicializador
        };
        match r {
            Resolved::Local(_) => {
                let u = self.u;
                self.motor().local_constante(u, e).then_some(RefConstante::Local)
            }
            Resolved::Parameter { name, .. } => {
                let par = self.parametro_declarado(a.expr(e).span.start, name)?;
                (par.kind != ParameterKind::Required).then_some(RefConstante::Parametro(par.default_value))
            }
            Resolved::Element(Element::Variable(v)) => p.variable(v).const_.then_some(RefConstante::Variavel(v)),
            Resolved::Element(Element::Function(g)) => {
                let v = p.function(g).variable?;
                p.variable(v).const_.then_some(RefConstante::Variavel(v))
            }
            Resolved::Member { member: MemberRef::Variable(v), .. } => do_campo(v).then_some(RefConstante::Variavel(v)),
            Resolved::Member { member: MemberRef::Function(g), .. } | Resolved::ExtensionMember { member: g, .. } => {
                let v = p.function(g).variable?;
                do_campo(v).then_some(RefConstante::Variavel(v))
            }
            _ => None,
        }
    }

    /// O parâmetro `nome` da função mais interna que contém `pos` e o
    /// declara.
    fn parametro_declarado(&self, pos: usize, nome: SymbolId) -> Option<&'a ast::Parameter> {
        let a = self.a;
        let mut melhor: Option<(usize, &'a ast::Parameter)> = None;
        let mut considerar = |span: Span, ps: &'a [ast::Parameter]| {
            if span.start <= pos
                && pos < span.end
                && let Some(p) = ps.iter().find(|p| p.name.is_some_and(|n| n.sym == nome))
                && melhor.is_none_or(|(t, _)| span.end - span.start < t)
            {
                melhor = Some((span.end - span.start, p));
            }
        };
        for f in a.functions.iter() {
            if let Some(ps) = &f.parameters {
                considerar(f.span, ps);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                considerar(m.span, &k.parameters);
            }
        }
        melhor.map(|(_, p)| p)
    }

    /// `_isConst`.
    fn e_const(&mut self, e: ExprId) -> bool {
        match &self.a.expr(e).kind {
            ExprKind::Int(_)
            | ExprKind::Double(_)
            | ExprKind::Bool(_)
            | ExprKind::Null
            | ExprKind::String(_)
            | ExprKind::Symbol(_)
            | ExprKind::List { .. }
            | ExprKind::SetOrMap { .. }
            | ExprKind::Record { .. } => true,
            _ => self.referencia_constante(e).is_some(),
        }
    }

    /// `_maybeGetBoolConstValue`.
    fn bool_constante(&mut self, e: ExprId) -> Option<bool> {
        if let ExprKind::Bool(b) = self.a.expr(e).kind {
            return Some(b);
        }
        let u = self.u;
        let lib = self.program.unit(u).library;
        let r = match self.referencia_constante(e)? {
            RefConstante::Variavel(v) => self.motor().valor_de_variavel(v)?,
            RefConstante::Local => self.motor().avaliar(&Ctx::simples(u, lib), e, true),
            RefConstante::Parametro(Some(d)) => self.motor().resultado_padrao(u, lib, d),
            RefConstante::Parametro(None) => return None,
        };
        match r {
            Constante::Valor(v) => match v.estado {
                Estado::Bool(b) => b,
                _ => None,
            },
            Constante::Invalida(_) => None,
        }
    }

    /// `_isLeaf`.
    fn e_folha(&mut self, args: &'a ast::Arguments) -> bool {
        for x in args.args.iter() {
            if x.name.is_some_and(|n| self.nome(n.sym) == "isLeaf") {
                return self.bool_constante(x.value).unwrap_or(false);
            }
        }
        false
    }

    /// `_validateIsLeafIsConst`.
    fn validar_folha_constante(&mut self, args: &'a ast::Arguments) {
        for x in args.args.iter() {
            if x.name.is_some_and(|n| self.nome(n.sym) == "isLeaf") && !self.e_const(x.value) {
                let s = self.span(x.value);
                self.relatar(cf::ARGUMENT_MUST_BE_A_CONSTANT, s, &[Arg::from("isLeaf")]);
            }
        }
    }

    // -- Declarações -----------------------------------------------------------

    fn declaracoes(&mut self) {
        let a = self.a;
        let u = self.u;
        let program = self.program;
        for d in program.unit(u).unit.directives.iter() {
            if let DirectiveKind::Library { .. } = d.kind {
                self.ativos_padrao(&d.metadata);
            }
        }
        for (di, d) in a.decls.iter().enumerate() {
            match &d.kind {
                DeclKind::Class(x) => {
                    if !x.mixin_application {
                        self.classe(di, d, x);
                    }
                }
                DeclKind::Mixin(x) => self.membros(&x.members, None),
                DeclKind::Enum(x) => self.membros(&x.members, None),
                DeclKind::ExtensionType(x) => self.membros(&x.members, None),
                DeclKind::Extension(x) => self.membros(&x.members, None),
                DeclKind::Function(fid) => {
                    let f = a.function(*fid);
                    if let Some(n) = f.name {
                        self.checar_nativo(n.span, No::Funcao(*fid), &d.metadata, f.parameters.as_deref().unwrap_or(&[]), f.external);
                    }
                }
                DeclKind::Variables(l) => {
                    for (i, v) in l.variables.iter().enumerate() {
                        self.checar_nativo(v.name.span, No::VariavelDeTopo(ast::DeclId(di as u32), i), &d.metadata, &[], l.external);
                    }
                }
                DeclKind::Typedef(_) => {}
            }
        }
        // As funções locais (`FunctionDeclarationStatement`).
        for (si, s) in a.stmts.iter().enumerate() {
            let StmtKind::Function(fid) = &s.kind else { continue };
            let Some((_, metadata)) = a.metadados_locais.iter().find(|(id, _)| id.0 as usize == si) else { continue };
            let f = a.function(*fid);
            let Some(n) = f.name else { continue };
            if self.dentro_de_indice(f.span) {
                continue;
            }
            let Some(t) = self.corpo.tipo_local(n.span.start) else { continue };
            self.checar_nativo(n.span, No::Local(t), metadata, f.parameters.as_deref().unwrap_or(&[]), f.external);
        }
    }

    /// `visitLibraryDirective`: no máximo um `@DefaultAsset`.
    fn ativos_padrao(&mut self, metadata: &'a [ast::Annotation]) {
        let mut houve = false;
        for m in metadata {
            let Some(inst) = self.instancia_da_anotacao(self.u, m) else { continue };
            if !self.classe_ffi(inst.classe, "DefaultAsset") {
                continue;
            }
            if houve {
                let s = self.nome_da_anotacao(m);
                self.relatar(cf::FFI_NATIVE_INVALID_DUPLICATE_DEFAULT_ASSET, s, &[]);
            }
            houve = true;
        }
    }

    /// O elemento do `NamedType` `t` escrito no cabeçalho de uma classe com
    /// os parâmetros de tipo `tps`.
    fn elemento_do_tipo_nomeado(&self, t: ast::TypeId, tps: &[ast::TypeParameter]) -> Option<Element> {
        let TypeKind::Named { name, .. } = &self.a.ty(t).kind else { return None };
        match &name[..] {
            [n] => {
                if tps.iter().any(|p| p.name.sym == n.sym) {
                    return None;
                }
                self.program.lookup_na_unidade(self.u, n.sym).and_then(|b| b.getter)
            }
            [p, n] => self.program.lookup_prefixed_na_unidade(self.u, p.sym, n.sym).and_then(|b| b.getter),
            _ => None,
        }
    }

    /// `name2.lexeme` do `NamedType`.
    fn lexema(&self, t: ast::TypeId) -> &'a str {
        match &self.a.ty(t).kind {
            TypeKind::Named { name, .. } => name.last().map_or("", |n| self.nome(n.sym)),
            _ => "",
        }
    }

    /// `ClassElement` (classe ou alias de classe; não mixin, enum ou tipo de
    /// extensão).
    fn e_class_element(&self, c: ClassId) -> bool {
        matches!(self.program.class(c).kind, ClassKind::Class | ClassKind::MixinApplication)
    }

    /// `allSupertypes` (sem a própria classe).
    fn supertipos(&self, c: ClassId) -> Vec<ClassId> {
        self.outline.hierarchy.get(c).map(|h| h.supertypes.keys().copied().filter(|k| *k != c).collect()).unwrap_or_default()
    }

    /// `NamedType.isCompoundSubtype`.
    fn nomeado_composto(&self, c: ClassId) -> bool {
        self.e_class_element(c) && self.supertipos(c).into_iter().any(|k| self.composto(k))
    }

    /// `NamedType.isAbiSpecificIntegerSubtype`.
    fn nomeado_abi(&self, c: ClassId) -> bool {
        self.e_class_element(c) && self.supertipos(c).into_iter().any(|k| self.classe_ffi(k, "AbiSpecificInteger"))
    }

    /// `visitClassDeclaration`.
    fn classe(&mut self, di: usize, d: &'a ast::Decl, x: &'a ast::ClassDecl) {
        let a = self.a;
        let mut composto: Option<(ClassId, &'a str)> = None;
        let c = if x.extends.is_some() || !x.implements.is_empty() || !x.with.is_empty() {
            self.indices().classes.get(&(di as u32)).copied()
        } else {
            None
        };
        if let (Some(c), Some(sup)) = (c, x.extends) {
            let lexema = self.lexema(sup);
            let nome_da_classe = self.nome(x.name.sym);
            match self.elemento_do_tipo_nomeado(sup, &x.type_params) {
                Some(Element::Class(sc)) if self.e_class_element(sc) && self.de_ffi(sc) => {
                    let nome = self.nome(self.program.class(sc).name);
                    if nome == "Struct" || nome == "Union" {
                        composto = Some((c, lexema));
                        if self.struct_vazio(c) {
                            self.relatar(cf::EMPTY_STRUCT, x.name.span, &[Arg::from(nome_da_classe), Arg::from(nome)]);
                        }
                        if nome == "Struct" {
                            self.validar_packed(&d.metadata);
                        }
                    } else if nome == "AbiSpecificInteger" {
                        self.validar_abi(x);
                        self.validar_mapeamento(x.name, &d.metadata);
                    }
                }
                Some(Element::Class(sc)) if self.nomeado_composto(sc) || self.nomeado_abi(sc) => {
                    let s = a.ty(sup).span;
                    self.relatar(cf::SUBTYPE_OF_STRUCT_CLASS_IN_EXTENDS, s, &[Arg::from(nome_da_classe), Arg::from(lexema)]);
                }
                _ => {}
            }
        }
        for &t in x.implements.iter() {
            self.checar_supertipo(x, t, cf::SUBTYPE_OF_STRUCT_CLASS_IN_IMPLEMENTS);
        }
        for &t in x.with.iter() {
            self.checar_supertipo(x, t, cf::SUBTYPE_OF_STRUCT_CLASS_IN_WITH);
        }
        if let Some((c, _)) = composto {
            let nome_da_classe = self.nome(x.name.sym);
            if !x.type_params.is_empty() {
                self.relatar(cf::GENERIC_STRUCT_SUBCLASS, x.name.span, &[Arg::from(nome_da_classe)]);
            }
            if !x.implements.is_empty()
                && let Some(fin) = self.classe_ffi_por_nome("Finalizable")
                && self.supertipos(c).contains(&fin)
            {
                self.relatar(cf::COMPOUND_IMPLEMENTS_FINALIZABLE, x.name.span, &[Arg::from(nome_da_classe)]);
            }
        }
        self.membros_da_classe(&x.members, composto);
    }

    /// `checkSupertype` de `implements` e `with`.
    fn checar_supertipo(&mut self, x: &'a ast::ClassDecl, t: ast::TypeId, codigo: Codigo) {
        let el = self.elemento_do_tipo_nomeado(t, &x.type_params);
        let nome = match el {
            Some(Element::Class(c)) => Some(self.nome(self.program.class(c).name)),
            Some(Element::Typedef(td)) => Some(self.nome(self.program.typedef(td).name)),
            _ => None,
        };
        if matches!(nome, Some("Allocator" | "Finalizable")) {
            return;
        }
        if let Some(Element::Class(c)) = el
            && (self.nomeado_composto(c) || self.nomeado_abi(c))
        {
            let s = self.a.ty(t).span;
            let nome_da_classe = self.nome(x.name.sym);
            let lexema = self.lexema(t);
            self.relatar(codigo, s, &[Arg::from(nome_da_classe), Arg::from(lexema)]);
        }
    }

    fn membros(&mut self, membros: &'a [ast::MemberId], composto: Option<(ClassId, &'a str)>) {
        self.membros_da_classe(membros, composto);
    }

    fn membros_da_classe(&mut self, membros: &'a [ast::MemberId], composto: Option<(ClassId, &'a str)>) {
        let a = self.a;
        for &mid in membros {
            let m = a.member(mid);
            match &m.kind {
                MemberKind::Field(l) => {
                    if let Some((c, sup)) = composto {
                        self.campos_no_composto(mid, m, l, c, sup, membros);
                    }
                    for (i, v) in l.variables.iter().enumerate() {
                        self.checar_nativo(v.name.span, No::Campo(mid, i), &m.metadata, &[], l.external);
                    }
                }
                MemberKind::Method(fid) => {
                    let f = a.function(*fid);
                    if let Some(n) = f.name {
                        self.checar_nativo(n.span, No::Funcao(*fid), &m.metadata, f.parameters.as_deref().unwrap_or(&[]), f.external);
                    }
                }
                MemberKind::Constructor(_) => {}
            }
        }
    }

    /// `_validatePackedAnnotation`.
    fn validar_packed(&mut self, metadata: &'a [ast::Annotation]) {
        let u = self.u;
        let packed: Vec<&'a ast::Annotation> = metadata.iter().filter(|m| self.construtor_ffi(u, m, "Packed")).collect();
        let Some(&primeira) = packed.first() else { return };
        for m in &packed[1..] {
            self.relatar(cf::PACKED_ANNOTATION, m.span, &[]);
        }
        let valor = self.instancia_da_anotacao(u, primeira).and_then(|i| i.alinhamento);
        if !matches!(valor, Some(1 | 2 | 4 | 8 | 16)) {
            let s = primeira.arguments.as_ref().and_then(|x| x.args.first()).map_or(primeira.span, |x| self.span_do_argumento(x));
            self.relatar(cf::PACKED_ANNOTATION_ALIGNMENT, s, &[]);
        }
    }

    /// `_validateAbiSpecificIntegerAnnotation`.
    fn validar_abi(&mut self, x: &'a ast::ClassDecl) {
        let valida = x.type_params.is_empty()
            && x.members.len() == 1
            && matches!(&self.a.member(x.members[0]).kind, MemberKind::Constructor(k) if k.const_);
        if !valida {
            self.relatar(cf::ABI_SPECIFIC_INTEGER_INVALID, x.name.span, &[]);
        }
    }

    /// `_validateAbiSpecificIntegerMappingAnnotation`.
    fn validar_mapeamento(&mut self, nome: ast::Name, metadata: &'a [ast::Annotation]) {
        let u = self.u;
        let mapas: Vec<&'a ast::Annotation> = metadata.iter().filter(|m| self.construtor_ffi(u, m, "AbiSpecificIntegerMapping")).collect();
        let Some(&primeira) = mapas.first() else {
            self.relatar(cf::ABI_SPECIFIC_INTEGER_MAPPING_MISSING, nome.span, &[]);
            return;
        };
        for m in &mapas[1..] {
            let s = self.nome_da_anotacao(m);
            self.relatar(cf::ABI_SPECIFIC_INTEGER_MAPPING_EXTRA, s, &[]);
        }
        let Some(args) = &primeira.arguments else { return };
        for x in args.args.iter() {
            if x.name.is_some() {
                continue;
            }
            let ExprKind::SetOrMap { elements, .. } = &self.a.expr(x.value).kind else { continue };
            for el in elements.iter() {
                let CollectionElement::MapEntry { value, .. } = el else { continue };
                let Some(t) = self.corpo.get_type(*value) else { continue };
                if !self.e_interface(t) {
                    continue;
                }
                let Some(c) = self.classe_do_tipo(t) else { continue };
                let n = self.nome(self.program.class(c).name);
                if !INTEIROS_FIXOS.contains(&n) {
                    let s = self.span(*value);
                    self.relatar(cf::ABI_SPECIFIC_INTEGER_MAPPING_UNSUPPORTED, s, &[Arg::from(n)]);
                }
            }
            return;
        }
        let Some(valores) = self.instancia_da_anotacao(u, primeira).and_then(|i| i.mapa) else { return };
        let Some(primeiro) = args.args.first() else { return };
        let s = self.span_do_argumento(primeiro);
        for t in valores {
            let Some(c) = self.classe_do_tipo(t) else { continue };
            let n = self.nome(self.program.class(c).name);
            if !INTEIROS_FIXOS.contains(&n) {
                self.relatar(cf::ABI_SPECIFIC_INTEGER_MAPPING_UNSUPPORTED, s, &[Arg::from(n)]);
            }
        }
    }

    /// `_validateFieldsInCompound`.
    fn campos_no_composto(&mut self, mid: ast::MemberId, m: &'a ast::Member, l: &'a ast::VariableList, c: ClassId, sup: &'a str, membros: &'a [ast::MemberId]) {
        if l.static_ {
            return;
        }
        let Some(primeira) = l.variables.first() else { return };
        if !l.external {
            self.relatar(cf::FIELD_MUST_BE_EXTERNAL_IN_STRUCT, primeira.name.span, &[]);
        }
        let Some(ty) = l.ty else {
            self.relatar(cf::MISSING_FIELD_TYPE_IN_STRUCT, primeira.name.span, &[]);
            return;
        };
        let span_tipo = self.a.ty(ty).span;
        let Some(declarado) = self.tipo_escrito(ty) else { return };
        let (arvore, fonte, interner) = (self.a, self.fonte, self.interner);
        let fonte_do_tipo = move || dartforge_frontend::fonte::de_tipo(arvore, fonte, interner, ty);
        if self.tabela_ref().get(declarado).is_declared_nullable() {
            let texto = fonte_do_tipo();
            self.relatar(cf::INVALID_FIELD_TYPE_IN_STRUCT, span_tipo, &[Arg::from(texto)]);
        } else if self.e_int(declarado) {
            self.validar_anotacoes(span_tipo, declarado, &m.metadata, Primitivo::Int, sup);
        } else if self.e_double(declarado) {
            self.validar_anotacoes(span_tipo, declarado, &m.metadata, Primitivo::Double, sup);
        } else if self.e_bool(declarado) {
            self.validar_anotacoes(span_tipo, declarado, &m.metadata, Primitivo::Bool, sup);
        } else if self.e_ffi_t(declarado, "Pointer") {
            self.sem_anotacoes(&m.metadata);
        } else if self.e_ffi_t(declarado, "Array") {
            if let [arg] = self.argumentos_do_tipo(declarado).as_slice()
                && !self.dimensionado(*arg)
            {
                let s = match &self.a.ty(ty).kind {
                    TypeKind::Named { args, .. } if !args.is_empty() => self.a.ty(args[0]).span,
                    _ => span_tipo,
                };
                self.relatar(cf::NON_SIZED_TYPE_ARGUMENT, s, &[Arg::from("Array"), Arg::Tipo(*arg)]);
            }
            let dimensoes = self.dimensoes_de_array(declarado);
            let ultimo = self.e_ultimo_campo(c, mid, membros);
            self.validar_tamanho(span_tipo, &m.metadata, dimensoes, ultimo);
        } else if self.subtipo_de_composto(declarado) {
            if let Some(k) = self.classe_do_tipo(declarado)
                && self.struct_vazio(k)
            {
                let nome = self.nome(self.program.class(k).name);
                let supertipo = self
                    .outline
                    .classes
                    .get(k.0 as usize)
                    .and_then(|d| d.supertype)
                    .map(|t| self.tabela_ref().format_sem_alias(t, self.interner, self.program))
                    .unwrap_or_default();
                self.relatar(cf::EMPTY_STRUCT, m.span, &[Arg::from(nome), Arg::from(supertipo)]);
            }
        } else {
            let texto = fonte_do_tipo();
            self.relatar(cf::INVALID_FIELD_TYPE_IN_STRUCT, span_tipo, &[Arg::from(texto)]);
        }
    }

    /// O primeiro campo da declaração `mid` é o último campo de instância
    /// externo da classe (`fields.reversed`, com os sintéticos dos acessores
    /// depois dos declarados).
    fn e_ultimo_campo(&mut self, c: ClassId, mid: ast::MemberId, membros: &'a [ast::MemberId]) -> bool {
        let Some(&primeiro) = self.indices().campos.get(&(mid.0, 0)) else { return false };
        let k = self.program.class(c);
        let mut ultimo: Option<Option<VariableId>> = None;
        let mut declarados: HashSet<SymbolId> = HashSet::new();
        for &v in &k.fields {
            let x = self.program.variable(v);
            declarados.insert(x.name);
            if !x.static_ && x.external {
                ultimo = Some(Some(v));
            }
        }
        // Os campos sintéticos dos acessores explícitos, na ordem.
        let mut sinteticos: Vec<(SymbolId, bool, bool)> = Vec::new();
        for &m in membros {
            let MemberKind::Method(fid) = &self.a.member(m).kind else { continue };
            let f = self.a.function(*fid);
            if !matches!(f.kind, ast::FunctionKind::Getter | ast::FunctionKind::Setter) {
                continue;
            }
            let Some(n) = f.name else { continue };
            if declarados.contains(&n.sym) {
                continue;
            }
            match sinteticos.iter_mut().find(|(s, _, _)| *s == n.sym) {
                Some(x) => x.2 |= f.external,
                None => sinteticos.push((n.sym, f.static_, f.external)),
            }
        }
        if sinteticos.iter().any(|(_, estatico, externo)| !estatico && *externo) {
            ultimo = Some(None);
        }
        ultimo == Some(Some(primeiro))
    }

    /// `_validateAnnotations`.
    fn validar_anotacoes(&mut self, span_tipo: Span, declarado: TypeId, metadata: &'a [ast::Annotation], requerido: Primitivo, sup: &'a str) {
        let u = self.u;
        let mut achou = false;
        let mut extras: Vec<&'a ast::Annotation> = Vec::new();
        for m in metadata {
            let alvo = self.alvo_da_anotacao(u, m);
            let (de_ffi, de_abi) = match alvo {
                AlvoDaAnotacao::Construtor(c, _) => {
                    let k = self.program.class(c);
                    let abi = k.kind == ClassKind::Class && k.supertype_class.is_some_and(|s| self.classe_ffi(s, "AbiSpecificInteger"));
                    (self.de_ffi(c), abi)
                }
                AlvoDaAnotacao::Classe(c) => (self.de_ffi(c), false),
                _ => (false, false),
            };
            if !(de_ffi || de_abi) {
                continue;
            }
            if achou {
                extras.push(m);
            } else if self.tipo_da_anotacao(alvo) == requerido {
                achou = true;
            } else {
                extras.push(m);
            }
        }
        if !extras.is_empty() {
            let mut resto = &extras[..];
            if !achou {
                self.relatar(cf::MISMATCHED_ANNOTATION_ON_STRUCT_FIELD, extras[0].span, &[]);
                resto = &extras[1..];
            }
            for m in resto {
                self.relatar(cf::EXTRA_ANNOTATION_ON_STRUCT_FIELD, m.span, &[]);
            }
        } else if !achou {
            self.relatar(cf::MISSING_ANNOTATION_ON_STRUCT_FIELD, span_tipo, &[Arg::Tipo(declarado), Arg::from(sup)]);
        }
    }

    /// `_typeForAnnotation`.
    fn tipo_da_anotacao(&self, alvo: AlvoDaAnotacao) -> Primitivo {
        let AlvoDaAnotacao::Construtor(c, _) = alvo else { return Primitivo::Nenhum };
        let k = self.program.class(c);
        match self.nome(k.name) {
            "Int8" | "Int16" | "Int32" | "Int64" | "Uint8" | "Uint16" | "Uint32" | "Uint64" | "IntPtr" => Primitivo::Int,
            "Float" | "Double" => Primitivo::Double,
            "Bool" => Primitivo::Bool,
            _ => {
                if k.supertype_class.is_some_and(|s| self.classe_ffi(s, "AbiSpecificInteger")) {
                    Primitivo::Int
                } else {
                    Primitivo::Nenhum
                }
            }
        }
    }

    /// `_validateNoAnnotations`.
    fn sem_anotacoes(&mut self, metadata: &'a [ast::Annotation]) {
        let u = self.u;
        for m in metadata {
            if self.classe_ffi_da_anotacao(u, m).is_some() {
                self.relatar(cf::ANNOTATION_ON_POINTER_FIELD, m.span, &[]);
            }
        }
    }

    /// `_validateSizeOfAnnotation`.
    fn validar_tamanho(&mut self, erro: Span, metadata: &'a [ast::Annotation], dimensoes_esperadas: usize, permite_variavel: bool) {
        let u = self.u;
        let arrays: Vec<&'a ast::Annotation> = metadata.iter().filter(|m| self.construtor_ffi(u, m, "Array")).collect();
        let Some(&primeira) = arrays.first() else {
            self.relatar(cf::MISSING_SIZE_ANNOTATION_CARRAY, erro, &[]);
            return;
        };
        for m in &arrays[1..] {
            self.relatar(cf::EXTRA_SIZE_ANNOTATION_CARRAY, m.span, &[]);
        }
        // `arraySizeDimensions`.
        let (dimensoes, variavel) = match self.instancia_da_anotacao(u, primeira) {
            Some(i) => {
                let variavel = i.variavel.unwrap_or(false);
                match i.lista {
                    Some(l) => {
                        let mut d = Vec::new();
                        if variavel {
                            d.push(0);
                        }
                        d.extend(l);
                        (d, variavel)
                    }
                    None => (i.dimensoes.iter().flatten().copied().collect::<Vec<i64>>(), variavel),
                }
            }
            None => (Vec::new(), false),
        };
        if dimensoes.len() != dimensoes_esperadas {
            self.relatar(cf::SIZE_ANNOTATION_DIMENSIONS, primeira.span, &[]);
        }
        if variavel && !permite_variavel {
            self.relatar(cf::VARIABLE_LENGTH_ARRAY_NOT_LAST, primeira.span, &[]);
        }
        // Os nós dos argumentos: os elementos da lista única, ou os
        // argumentos.
        let mut nos: Vec<Span> = Vec::new();
        if let Some(args) = &primeira.arguments {
            match &args.args[..] {
                [x] if x.name.is_none() && matches!(self.a.expr(x.value).kind, ExprKind::List { .. }) => {
                    if let ExprKind::List { elements, .. } = &self.a.expr(x.value).kind {
                        for el in elements.iter() {
                            let s = self.motor().span_de_elemento(u, el);
                            nos.push(s);
                        }
                    }
                }
                xs => {
                    for x in xs {
                        nos.push(self.span_do_argumento(x));
                    }
                }
            }
        }
        for (i, d) in dimensoes.iter().enumerate() {
            if i == 0 && variavel {
                continue;
            }
            if *d <= 0 {
                let s = if nos.is_empty() { primeira.span } else { nos.get(i).copied().unwrap_or(primeira.span) };
                self.relatar(cf::NON_POSITIVE_ARRAY_DIMENSION, s, &[]);
            }
        }
    }

    /// `_checkFfiNative`.
    fn checar_nativo(&mut self, nome: Span, no: No, metadata: &'a [ast::Annotation], parametros: &'a [ast::Parameter], externo: bool) {
        if metadata.is_empty() {
            return;
        }
        let u = self.u;
        let mut houve = false;
        for m in metadata {
            // Só o valor de `Native` interessa; o resto nem é avaliado.
            let candidato = match self.alvo_da_anotacao(u, m) {
                AlvoDaAnotacao::Construtor(c, _) => self.classe_ffi(c, "Native"),
                AlvoDaAnotacao::Variavel(v) => self.program.variable(v).const_,
                _ => false,
            };
            if !candidato {
                continue;
            }
            let Some(inst) = self.instancia_da_anotacao(u, m) else { continue };
            if !self.classe_ffi(inst.classe, "Native") {
                continue;
            }
            if houve {
                let s = self.nome_da_anotacao(m);
                self.relatar(cf::FFI_NATIVE_INVALID_MULTIPLE_ANNOTATIONS, s, &[]);
                break;
            }
            houve = true;
            if !externo {
                self.relatar(cf::FFI_NATIVE_MUST_BE_EXTERNAL, nome, &[]);
            }
            let Some(decl) = self.declaracao(no) else { continue };
            let Some(&assinatura) = self.argumentos_do_tipo(inst.tipo).first() else { continue };
            if matches!(self.tabela_ref().get(assinatura), Type::Function { .. }) {
                match decl {
                    Declaracao::Funcao(_) | Declaracao::Local(_) => self.checar_funcao_nativa(nome, decl, assinatura, inst.is_leaf, parametros),
                    Declaracao::Variavel(_) => self.relatar(cf::NATIVE_FIELD_INVALID_TYPE, nome, &[Arg::Tipo(assinatura)]),
                }
            } else {
                let funcao = match decl {
                    Declaracao::Funcao(f) => !matches!(self.program.function(f).kind, FunctionKind::Getter | FunctionKind::Setter),
                    Declaracao::Local(_) => true,
                    Declaracao::Variavel(_) => false,
                };
                if funcao {
                    self.relatar(cf::MUST_BE_A_NATIVE_FUNCTION_TYPE, nome, &[Arg::from("T"), Arg::from("Native")]);
                } else {
                    self.checar_campo_nativo(nome, decl, metadata, assinatura, false);
                }
            }
        }
    }

    /// O tipo do i-ésimo parâmetro formal da declaração.
    fn tipo_do_parametro(&self, d: Declaracao, i: usize, parametros: &'a [ast::Parameter]) -> Option<TypeId> {
        match d {
            Declaracao::Funcao(f) => self.outline.functions.get(f.0 as usize).and_then(|x| x.parameters.get(i)).map(|p| p.ty),
            Declaracao::Local(t) => {
                let Type::Function { positional, optional, named, .. } = self.tipo(t) else { return None };
                let p = parametros.get(i)?;
                match p.kind {
                    ParameterKind::Required => positional.get(parametros[..i].iter().filter(|x| x.kind == ParameterKind::Required).count()).copied(),
                    ParameterKind::Optional => optional.get(parametros[..i].iter().filter(|x| x.kind == ParameterKind::Optional).count()).copied(),
                    ParameterKind::Named => {
                        let n = p.name?.sym;
                        named.iter().find(|(s, _, _)| *s == n).map(|x| x.1)
                    }
                }
            }
            Declaracao::Variavel(_) => None,
        }
    }

    /// `_checkFfiNativeFunction`.
    fn checar_funcao_nativa(&mut self, nome: Span, d: Declaracao, assinatura: TypeId, is_leaf: Option<bool>, parametros: &'a [ast::Parameter]) {
        if is_leaf == Some(true) {
            self.folha_sem_handles(assinatura, nome);
        }
        let Type::Function { type_params, ret, positional, optional, named, nullable } = self.tipo(assinatura) else { return };
        let mut tipos_ffi = self.achatar(&positional);
        let mut posicionais_ffi = positional.to_vec();
        let de_instancia = match d {
            Declaracao::Funcao(f) => {
                let g = self.program.function(f);
                (g.class.is_some() || g.extension.is_some()) && !g.static_
            }
            _ => false,
        };
        let formais = parametros.len();
        if de_instancia {
            if formais + 1 != tipos_ffi.len() {
                let (a, b) = ((formais + 1).to_string(), tipos_ffi.len().to_string());
                self.relatar(cf::FFI_NATIVE_UNEXPECTED_NUMBER_OF_PARAMETERS_WITH_RECEIVER, nome, &[Arg::from(a), Arg::from(b)]);
                return;
            }
            if let Some(&receptor) = positional.first()
                && self.e_ffi_t(receptor, "Pointer")
            {
                let estende = match d {
                    Declaracao::Funcao(f) => self.program.function(f).class.is_some_and(|c| self.classe_estende_nfwc1(c)),
                    _ => false,
                };
                if !estende {
                    self.relatar(cf::FFI_NATIVE_ONLY_CLASSES_EXTENDING_NATIVEFIELDWRAPPERCLASS1_CAN_BE_POINTER, nome, &[]);
                }
            }
            tipos_ffi.remove(0);
            if !posicionais_ffi.is_empty() {
                posicionais_ffi.remove(0);
            }
        } else if formais != tipos_ffi.len() {
            let (a, b) = (tipos_ffi.len().to_string(), formais.to_string());
            self.relatar(cf::FFI_NATIVE_UNEXPECTED_NUMBER_OF_PARAMETERS, nome, &[Arg::from(a), Arg::from(b)]);
            return;
        }
        for i in 0..formais {
            if !self.e_ffi_t(tipos_ffi[i], "Pointer") {
                continue;
            }
            let t = self.tipo_do_parametro(d, i, parametros);
            let ok = t.is_some_and(|t| {
                matches!(self.tabela_ref().get(t), Type::Interface { .. } | Type::ExtensionType { .. } | Type::Null | Type::FutureOr { .. })
                    && (self.e_ffi_t(t, "Pointer") || self.estende_nfwc1(t) || self.e_typed_data(t))
            });
            if !ok {
                self.relatar(cf::FFI_NATIVE_ONLY_CLASSES_EXTENDING_NATIVEFIELDWRAPPERCLASS1_CAN_BE_POINTER, nome, &[]);
            }
        }
        let tipo_dart = match d {
            Declaracao::Funcao(f) => self.outline.functions.get(f.0 as usize).map(|x| x.signature),
            Declaracao::Local(t) => Some(t),
            Declaracao::Variavel(_) => None,
        };
        let Some(tipo_dart) = tipo_dart else { return };
        let nativo = self.tabela().intern(Type::Function {
            type_params,
            ret,
            positional: posicionais_ffi.into_boxed_slice(),
            optional,
            named,
            nullable,
        });
        if !self.funcao_nativa_valida(nativo) {
            self.relatar(cf::MUST_BE_A_NATIVE_FUNCTION_TYPE, nome, &[Arg::Tipo(nativo), Arg::from("Native")]);
            return;
        }
        if !self.compat_funcoes(Direcao::NativoParaDart, tipo_dart, nativo, true, true) {
            self.relatar(cf::MUST_BE_A_SUBTYPE, nome, &[Arg::Tipo(nativo), Arg::Tipo(tipo_dart), Arg::from("Native")]);
        }
    }

    /// `_checkFfiNativeField`.
    fn checar_campo_nativo(&mut self, nome: Span, d: Declaracao, metadata: &'a [ast::Annotation], assinatura: TypeId, permite_variavel: bool) {
        let tipo = match d {
            Declaracao::Variavel(v) => {
                let x = self.program.variable(v);
                if (x.class.is_some() || x.extension.is_some()) && !x.static_ {
                    self.relatar(cf::NATIVE_FIELD_NOT_STATIC, nome, &[]);
                }
                self.tipo_da_variavel(v)
            }
            Declaracao::Funcao(f) => {
                // O acessor: o tipo da variável sintética (o retorno do
                // getter de mesmo nome, ou o parâmetro do setter).
                let g = self.program.function(f);
                if let Some(v) = g.variable {
                    self.tipo_da_variavel(v)
                } else {
                    let getter = if g.kind == FunctionKind::Getter {
                        Some(f)
                    } else {
                        let mapa = match (g.class, g.extension) {
                            (Some(c), _) => {
                                let k = self.program.class(c);
                                if g.static_ { &k.static_members } else { &k.instance_members }
                            }
                            (None, Some(x)) => {
                                let k = self.program.extension(x);
                                if g.static_ { &k.static_members } else { &k.instance_members }
                            }
                            (None, None) => {
                                let alvo = self.program.library(g.library).declared.get(&g.name).and_then(|b| b.getter);
                                match alvo {
                                    Some(Element::Function(h)) if self.program.function(h).kind == FunctionKind::Getter => {
                                        return self.checar_campo_com_tipo(nome, metadata, assinatura, permite_variavel, self.retorno(h));
                                    }
                                    _ => {
                                        let t = self.parametro_do_setter(f);
                                        return self.checar_campo_com_tipo(nome, metadata, assinatura, permite_variavel, t);
                                    }
                                }
                            }
                        };
                        mapa.get(&g.name).copied().filter(|h| self.program.function(*h).kind == FunctionKind::Getter)
                    };
                    match getter {
                        Some(h) => self.retorno(h),
                        None => self.parametro_do_setter(f),
                    }
                }
            }
            Declaracao::Local(_) => {
                self.relatar(cf::NATIVE_FIELD_NOT_STATIC, nome, &[]);
                return;
            }
        };
        self.checar_campo_com_tipo(nome, metadata, assinatura, permite_variavel, tipo);
    }

    fn retorno(&self, f: FunctionElementId) -> TypeId {
        self.outline.functions.get(f.0 as usize).map_or(self.core.dynamic_, |d| d.return_type)
    }

    fn parametro_do_setter(&self, f: FunctionElementId) -> TypeId {
        self.outline.functions.get(f.0 as usize).and_then(|d| d.parameters.first()).map_or(self.core.dynamic_, |p| p.ty)
    }

    fn checar_campo_com_tipo(&mut self, nome: Span, metadata: &'a [ast::Annotation], assinatura: TypeId, permite_variavel: bool, tipo: TypeId) {
        let mut assinatura = assinatura;
        if matches!(self.tabela_ref().get(assinatura), Type::Dynamic) && !self.tabela_ref().e_invalido(assinatura) {
            // `_canonicalFfiTypeForDartType`.
            if self.e_ffi_t(tipo, "Pointer") || self.subtipo_de_composto(tipo) || self.e_ffi_t(tipo, "Array") {
                assinatura = tipo;
            } else {
                self.relatar(cf::NATIVE_FIELD_MISSING_TYPE, nome, &[]);
                return;
            }
        }
        if !self.compat_nativo(Direcao::NativoParaDart, tipo, assinatura, false, true) {
            self.relatar(cf::MUST_BE_A_SUBTYPE, nome, &[Arg::Tipo(tipo), Arg::Tipo(assinatura), Arg::from("Native")]);
        } else if self.e_ffi_t(assinatura, "Array") {
            let d = self.dimensoes_de_array(assinatura);
            self.validar_tamanho(nome, metadata, d, permite_variavel);
        } else if self.e_ffi_t(assinatura, "Handle") || self.e_ffi_t(assinatura, "NativeFunction") {
            self.relatar(cf::NATIVE_FIELD_INVALID_TYPE, nome, &[Arg::Tipo(assinatura)]);
        }
    }

    /// `_validateFfiLeafCallUsesNoHandles`.
    fn folha_sem_handles(&mut self, nativo: TypeId, span: Span) {
        let Type::Function { ret, positional, .. } = self.tipo(nativo) else { return };
        if self.primitivo(ret) == Primitivo::Handle {
            self.relatar(cf::LEAF_CALL_MUST_NOT_RETURN_HANDLE, span, &[]);
        }
        for p in positional.iter() {
            if self.primitivo(*p) == Primitivo::Handle {
                self.relatar(cf::LEAF_CALL_MUST_NOT_TAKE_HANDLE, span, &[]);
            }
        }
    }

    // -- Expressões ------------------------------------------------------------

    fn expressoes(&mut self) {
        let a = self.a;
        for (i, e) in a.exprs.iter().enumerate() {
            let id = ExprId(i as u32);
            match &e.kind {
                ExprKind::Call { target, arguments } => {
                    if !self.dentro_de_indice(e.span) {
                        self.chamada(id, *target, arguments);
                    }
                }
                ExprKind::InstanceCreation { ty, constructor, arguments, .. } => {
                    if self.dentro_de_indice(e.span) {
                        continue;
                    }
                    if let Some(Resolved::Constructor(f)) = self.corpo.get_resolved(id) {
                        let inicio = a.ty(*ty).span.start;
                        let fim = constructor.map_or(a.ty(*ty).span.end, |n| n.span.end);
                        self.criacao(id, *f, Span { start: inicio, end: fim }, arguments);
                    }
                }
                ExprKind::Index { target, .. } => {
                    if !self.dentro_de_indice(e.span) {
                        self.indice(id, *target);
                    }
                }
                ExprKind::Property { target, name, .. } => {
                    if !self.dentro_de_indice(e.span) {
                        self.propriedade(id, *target, *name);
                    }
                }
                _ => {}
            }
        }
    }

    /// `MethodInvocation` e `FunctionExpressionInvocation` (e a criação
    /// implícita, que o analyzer reescreve em `InstanceCreationExpression`).
    fn chamada(&mut self, id: ExprId, target: ExprId, args: &'a ast::Arguments) {
        let a = self.a;
        match self.corpo.get_resolved(id) {
            Some(Resolved::ExtensionMember { extension, member }) => {
                // `valor<T>(n)`: o `call` de `AllocatorAlloc`. O `.call(…)`
                // escrito é uma `MethodInvocation`, que não é verificada.
                let escrito = matches!(&a.expr(target).kind, ExprKind::Property { name, .. } if self.nome(name.sym) == "call");
                if !escrito && self.extensao_ffi(*extension, &["AllocatorAlloc"]) && self.nome(self.program.function(*member).name) == "call" {
                    self.validar_alocacao(id, args);
                }
                if !escrito {
                    return;
                }
            }
            Some(Resolved::Constructor(f)) => {
                let s = self.span(target);
                self.criacao(id, *f, s, args);
                return;
            }
            _ => {}
        }
        let (nome_do_metodo, r) = match &a.expr(target).kind {
            ExprKind::Property { name, .. } | ExprKind::Identifier(name) => (*name, self.corpo.get_resolved(target)),
            _ => return,
        };
        let f = match r {
            Some(Resolved::Member { member: MemberRef::Function(f), .. }) | Some(Resolved::ExtensionMember { member: f, .. }) => *f,
            Some(Resolved::Element(Element::Function(f))) => *f,
            _ => return,
        };
        let g = self.program.function(f);
        if g.kind != FunctionKind::Function {
            return;
        }
        let nome = self.nome(g.name);
        if let Some(c) = g.class {
            if self.classe_ffi(c, "Pointer") {
                match nome {
                    "fromFunction" => self.validar_from_function(nome_do_metodo, args),
                    "elementAt" => self.validar_element_at(id, target),
                    _ => {}
                }
            } else if self.composto(c) {
                if nome == "create" {
                    let classe = self.nome(self.program.class(c).name);
                    self.validar_create(id, args, classe);
                }
            } else if self.classe_ffi(c, "Native") && nome == "addressOf" {
                self.validar_address_of(id, args);
            }
        } else if let Some(x) = g.extension {
            if self.extensao_ffi(x, &["NativeFunctionPointer"]) {
                if nome == "asFunction" {
                    self.validar_as_function(id, target, nome_do_metodo, args);
                }
            } else if self.extensao_ffi(x, &["DynamicLibraryExtension"]) && nome == "lookupFunction" {
                self.validar_lookup_function(args);
            }
        } else if g.library == self.ffi && nome == "sizeOf" {
            self.validar_size_of(id, args);
        }
    }

    /// O receptor (`realTarget`) da invocação cujo nome de método é `target`.
    fn receptor(&self, target: ExprId) -> Option<ExprId> {
        match &self.a.expr(target).kind {
            ExprKind::Property { target: r, .. } => Some(self.alvo_real(*r)),
            _ => None,
        }
    }

    /// `visitInstanceCreationExpression`.
    fn criacao(&mut self, id: ExprId, f: FunctionElementId, nome_do_construtor: Span, args: &'a ast::Arguments) {
        let g = self.program.function(f);
        let Some(c) = g.class else { return };
        let sup = self.program.class(c).supertype_class;
        if sup.is_some_and(|s| self.composto(s)) && self.e_class_element(c) {
            if !self.program.function(self.program.publico(f)).factory {
                self.relatar(cf::CREATION_OF_STRUCT_OR_UNION, nome_do_construtor, &[]);
            }
        } else if self.classe_ffi(c, "NativeCallable") {
            self.validar_native_callable(id, f, nome_do_construtor, args);
        }
    }

    /// `_validateNativeCallable`.
    fn validar_native_callable(&mut self, id: ExprId, f: FunctionElementId, nome_do_construtor: Span, args: &'a ast::Arguments) {
        let nome = self.nome(self.program.function(f).name);
        let isolado = nome == "isolateLocal";
        let n = args.args.len();
        if !(n == 1 || (isolado && n == 2)) {
            return;
        }
        let Some(st) = self.corpo.get_type(id) else { return };
        let Some(&argumento) = self.argumentos_do_tipo(st).first() else { return };
        if !self.funcao_nativa_valida(argumento) {
            self.relatar(cf::MUST_BE_A_NATIVE_FUNCTION_TYPE, nome_do_construtor, &[Arg::Tipo(argumento), Arg::from("NativeCallable")]);
            return;
        }
        let f0 = &args.args[0];
        let ft = self.corpo.get_type(f0.value).unwrap_or(self.core.dynamic_);
        if !self.compat_funcoes(Direcao::DartParaNativo, ft, argumento, false, false) {
            let s = self.span_do_argumento(f0);
            self.relatar(cf::MUST_BE_A_SUBTYPE, s, &[Arg::Tipo(ft), Arg::Tipo(argumento), Arg::from("NativeCallable")]);
            return;
        }
        let Type::Function { ret, .. } = self.tipo(argumento) else { return };
        if isolado {
            if self.primitivo(ret) == Primitivo::Void || self.e_ffi_t(ret, "Pointer") || self.e_ffi_t(ret, "Handle") || self.subtipo_de_composto(ret) {
                if n != 1 {
                    let s = self.span_do_argumento(&args.args[1]);
                    self.relatar(cf::INVALID_EXCEPTION_VALUE, s, &[Arg::from(nome)]);
                }
            } else if n != 2 {
                let s = self.span(id);
                self.relatar(cf::MISSING_EXCEPTION_VALUE, s, &[Arg::from(nome)]);
            } else {
                let e = args.args[1].value;
                let et = self.corpo.get_type(e).unwrap_or(self.core.dynamic_);
                let s = self.span(e);
                if !self.compat_nativo(Direcao::DartParaNativo, et, ret, false, false) {
                    self.relatar(cf::MUST_BE_A_SUBTYPE, s, &[Arg::Tipo(et), Arg::Tipo(ret), Arg::from(nome)]);
                }
                if !self.e_const(e) {
                    self.relatar(cf::ARGUMENT_MUST_BE_A_CONSTANT, s, &[Arg::from("exceptionalReturn")]);
                }
            }
        } else if self.primitivo(ret) != Primitivo::Void {
            let s = self.span_do_argumento(f0);
            self.relatar(cf::MUST_RETURN_VOID, s, &[Arg::Tipo(ret)]);
        }
    }

    /// `_validateAllocate`.
    fn validar_alocacao(&mut self, id: ExprId, args: &'a ast::Arguments) {
        let tipos = self.instanciacao(args);
        let [t] = tipos.as_slice() else { return };
        if !self.valido_nativo(*t, Permite { void_: true, struct_vazio: true, ..Permite::default() }) {
            let s = self.span(id);
            self.relatar(cf::NON_CONSTANT_TYPE_ARGUMENT, s, &[Arg::from("AllocatorAlloc.call")]);
        }
    }

    /// `_validateFromFunction`.
    fn validar_from_function(&mut self, nome_do_metodo: ast::Name, args: &'a ast::Arguments) {
        let n = args.args.len();
        if !(1..=2).contains(&n) {
            return;
        }
        let Some(&t) = self.instanciacao(args).first() else { return };
        if !self.funcao_nativa_valida(t) {
            let s = args.type_args.first().map_or(nome_do_metodo.span, |&x| self.a.ty(x).span);
            self.relatar(cf::MUST_BE_A_NATIVE_FUNCTION_TYPE, s, &[Arg::Tipo(t), Arg::from("fromFunction")]);
            return;
        }
        let f0 = &args.args[0];
        let ft = self.corpo.get_type(f0.value).unwrap_or(self.core.dynamic_);
        if !self.compat_funcoes(Direcao::DartParaNativo, ft, t, false, false) {
            let s = self.span_do_argumento(f0);
            self.relatar(cf::MUST_BE_A_SUBTYPE, s, &[Arg::Tipo(ft), Arg::Tipo(t), Arg::from("fromFunction")]);
            return;
        }
        let Type::Function { ret: r, .. } = self.tipo(t) else { return };
        if self.primitivo(r) == Primitivo::Void || self.e_ffi_t(r, "Pointer") || self.e_ffi_t(r, "Handle") || self.subtipo_de_composto(r) {
            if n != 1 {
                let s = self.span_do_argumento(&args.args[1]);
                self.relatar(cf::INVALID_EXCEPTION_VALUE, s, &[Arg::from("fromFunction")]);
            }
        } else if n != 2 {
            self.relatar(cf::MISSING_EXCEPTION_VALUE, nome_do_metodo.span, &[Arg::from("fromFunction")]);
        } else {
            let x = &args.args[1];
            let et = self.corpo.get_type(x.value).unwrap_or(self.core.dynamic_);
            let s = self.span_do_argumento(x);
            if !self.compat_nativo(Direcao::DartParaNativo, et, r, false, false) {
                self.relatar(cf::MUST_BE_A_SUBTYPE, s, &[Arg::Tipo(et), Arg::Tipo(r), Arg::from("fromFunction")]);
            }
            let constante = x.name.is_none() && self.e_const(x.value);
            if !constante {
                self.relatar(cf::ARGUMENT_MUST_BE_A_CONSTANT, s, &[Arg::from("exceptionalReturn")]);
            }
        }
    }

    /// `_validateElementAt`.
    fn validar_element_at(&mut self, id: ExprId, target: ExprId) {
        let Some(r) = self.receptor(target) else { return };
        let Some(tt) = self.corpo.get_type(r) else { return };
        if !self.e_ffi_t(tt, "Pointer") {
            return;
        }
        let Some(&t) = self.argumentos_do_tipo(tt).first() else { return };
        if !self.valido_nativo(t, Permite { void_: true, struct_vazio: true, ..Permite::default() }) {
            let s = self.span(id);
            self.relatar(cf::NON_CONSTANT_TYPE_ARGUMENT, s, &[Arg::from("elementAt")]);
        }
    }

    /// `_validateCreate`.
    fn validar_create(&mut self, id: ExprId, args: &'a ast::Arguments, classe: &str) {
        let tipos = self.instanciacao(args);
        let [t] = tipos.as_slice() else { return };
        if !self.valido_nativo(*t, Permite::default()) {
            let s = self.span(id);
            self.relatar(cf::NON_CONSTANT_TYPE_ARGUMENT, s, &[Arg::from(format!("{classe}.create"))]);
        }
    }

    /// `_validateSizeOf`.
    fn validar_size_of(&mut self, id: ExprId, args: &'a ast::Arguments) {
        let tipos = self.instanciacao(args);
        let [t] = tipos.as_slice() else { return };
        if !self.valido_nativo(*t, Permite { void_: true, struct_vazio: true, ..Permite::default() }) {
            let s = self.span(id);
            self.relatar(cf::NON_CONSTANT_TYPE_ARGUMENT, s, &[Arg::from("sizeOf")]);
        }
    }

    /// `_validateAsFunction`.
    fn validar_as_function(&mut self, id: ExprId, target: ExprId, nome_do_metodo: ast::Name, args: &'a ast::Arguments) {
        let escritos = &args.type_args;
        let span_erro = escritos.first().map_or(self.span(id), |&x| self.a.ty(x).span);
        if let [x] = &escritos[..]
            && let Some(t) = self.tipo_escrito(*x)
            && self.e_parametro(t)
        {
            let s = self.a.ty(*x).span;
            self.relatar(cf::NON_CONSTANT_TYPE_ARGUMENT, s, &[Arg::from("asFunction")]);
            return;
        }
        if let Some(alvo) = self.receptor(target)
            && let Some(tt) = self.corpo.get_type(alvo)
            && self.e_ffi_t(tt, "Pointer")
        {
            let Some(&t) = self.argumentos_do_tipo(tt).first() else { return };
            if !self.e_ffi_t(t, "NativeFunction") {
                return;
            }
            let Some(&argumento) = self.argumentos_do_tipo(t).first() else { return };
            if self.e_parametro(argumento) {
                let s = self.span(alvo);
                self.relatar(cf::NON_CONSTANT_TYPE_ARGUMENT, s, &[Arg::from("asFunction")]);
                return;
            }
            if !self.funcao_nativa_valida(argumento) {
                self.relatar(cf::NON_NATIVE_FUNCTION_TYPE_ARGUMENT_TO_POINTER, span_erro, &[Arg::Tipo(t)]);
                return;
            }
            let Some(&f) = self.instanciacao(args).first() else { return };
            let folha = self.e_folha(args);
            if !self.compat_funcoes(Direcao::NativoParaDart, f, argumento, false, false) {
                let s = self.span(id);
                self.relatar(cf::MUST_BE_A_SUBTYPE, s, &[Arg::Tipo(argumento), Arg::Tipo(f), Arg::from("asFunction")]);
            }
            if folha {
                self.folha_sem_handles(argumento, nome_do_metodo.span);
            }
        }
        self.validar_folha_constante(args);
    }

    /// `_validateLookupFunction`.
    fn validar_lookup_function(&mut self, args: &'a ast::Arguments) {
        let escritos = &args.type_args;
        if escritos.len() != 2 {
            return;
        }
        let tipos = self.instanciacao(args);
        let (Some(&s), Some(&f)) = (tipos.first(), tipos.get(1)) else { return };
        let span0 = self.a.ty(escritos[0]).span;
        let span1 = self.a.ty(escritos[1]).span;
        if !self.funcao_nativa_valida(s) {
            self.relatar(cf::MUST_BE_A_NATIVE_FUNCTION_TYPE, span0, &[Arg::Tipo(s), Arg::from("lookupFunction")]);
            return;
        }
        let folha = self.e_folha(args);
        if !self.compat_funcoes(Direcao::NativoParaDart, f, s, false, false) {
            self.relatar(cf::MUST_BE_A_SUBTYPE, span1, &[Arg::Tipo(s), Arg::Tipo(f), Arg::from("lookupFunction")]);
        }
        self.validar_folha_constante(args);
        if folha {
            self.folha_sem_handles(s, span0);
        }
    }

    /// O elemento (não sintético) que um identificador referencia.
    fn elemento_referido(&self, e: ExprId) -> Option<Declaracao> {
        match &self.a.expr(e).kind {
            ExprKind::Identifier(_) => {}
            ExprKind::Property { target, .. } if matches!(self.a.expr(*target).kind, ExprKind::Identifier(_)) => {}
            _ => return None,
        }
        let p = self.program;
        let pela_funcao = |f: FunctionElementId| match p.function(f).variable {
            Some(v) => Declaracao::Variavel(v),
            None => Declaracao::Funcao(f),
        };
        match self.corpo.get_resolved(e)? {
            Resolved::Element(Element::Function(f)) => Some(pela_funcao(*f)),
            Resolved::Element(Element::Variable(v)) => Some(Declaracao::Variavel(*v)),
            Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } => Some(pela_funcao(*f)),
            Resolved::Member { member: MemberRef::Variable(v), .. } => Some(Declaracao::Variavel(*v)),
            _ => None,
        }
    }

    /// `_validateNativeAddressOf`.
    fn validar_address_of(&mut self, id: ExprId, args: &'a ast::Arguments) {
        let tipos = self.instanciacao(args);
        if tipos.len() != 1 || args.args.len() != 1 {
            return;
        }
        let argumento = &args.args[0];
        let alvo = tipos[0];
        let mut valido = false;
        let referido = if argumento.name.is_none() { self.elemento_referido(argumento.value) } else { None };
        if let Some(r) = referido {
            let (unidade, metadata) = self.anotacoes_do_elemento(r);
            for m in metadata {
                let Some(inst) = self.instancia_da_anotacao(unidade, m) else { continue };
                if !self.classe_ffi(inst.classe, "Native") {
                    continue;
                }
                let Some(&nativo) = self.argumentos_do_tipo(inst.tipo).first() else { continue };
                let s = self.span(id);
                if matches!(self.tabela_ref().get(nativo), Type::Function { .. }) {
                    if !self.e_ffi_t(alvo, "NativeFunction") {
                        self.relatar(cf::MUST_BE_A_NATIVE_FUNCTION_TYPE, s, &[Arg::Tipo(alvo), Arg::from("Native.addressOf")]);
                    } else if let Some(&funcao_alvo) = self.argumentos_do_tipo(alvo).first()
                        && !self.atribuivel(nativo, funcao_alvo)
                    {
                        self.relatar(cf::MUST_BE_A_SUBTYPE, s, &[Arg::Tipo(nativo), Arg::Tipo(funcao_alvo), Arg::from("Native.addressOf")]);
                    }
                } else {
                    let mut nativo = nativo;
                    if matches!(self.tabela_ref().get(nativo), Type::Dynamic)
                        && !self.tabela_ref().e_invalido(nativo)
                        && let Some(st) = self.corpo.get_type(argumento.value)
                        && (self.e_ffi_t(st, "Pointer") || self.subtipo_de_composto(st) || self.e_ffi_t(st, "Array"))
                    {
                        nativo = st;
                    }
                    if !self.atribuivel(nativo, alvo) {
                        self.relatar(cf::MUST_BE_A_SUBTYPE, s, &[Arg::Tipo(nativo), Arg::Tipo(alvo), Arg::from("Native.addressOf")]);
                    }
                }
                valido = true;
                break;
            }
        }
        if !valido {
            let s = self.span_do_argumento(argumento);
            self.relatar(cf::ARGUMENT_MUST_BE_NATIVE, s, &[]);
        }
    }

    /// `visitIndexExpression`: o `[]` de `StructPointer`, `StructArray`,
    /// `UnionPointer` e `UnionArray`.
    fn indice(&mut self, id: ExprId, target: ExprId) {
        let Some(Resolved::ExtensionMember { extension, member }) = self.corpo.get_resolved(id) else { return };
        if !self.extensao_ffi(*extension, &["StructPointer", "StructArray", "UnionPointer", "UnionArray"]) || self.nome(self.program.function(*member).name) != "[]" {
            return;
        }
        let alvo = self.alvo_real(target);
        let Some(t) = self.corpo.get_type(alvo) else { return };
        if !self.valido_nativo(t, Permite { struct_vazio: true, array: true, ..Permite::default() }) {
            let s = self.span(id);
            self.relatar(cf::NON_CONSTANT_TYPE_ARGUMENT, s, &[Arg::from("[]")]);
        }
    }

    /// `visitPrefixedIdentifier` e `visitPropertyAccess`: `ref` e `address`.
    fn propriedade(&mut self, id: ExprId, target: ExprId, nome: ast::Name) {
        let Some(Resolved::ExtensionMember { extension, member }) = self.corpo.get_resolved(id) else { return };
        let (x, f) = (*extension, *member);
        if self.program.function(f).kind == FunctionKind::Setter {
            return;
        }
        let nome_do_membro = self.nome(self.program.function(f).name);
        if self.extensao_ffi(x, &["StructPointer", "UnionPointer"]) {
            if nome_do_membro == "ref" {
                let alvo = self.alvo_real(target);
                let Some(t) = self.corpo.get_type(alvo) else { return };
                if !self.valido_nativo(t, Permite { struct_vazio: true, ..Permite::default() }) {
                    let s = self.span(id);
                    self.relatar(cf::NON_CONSTANT_TYPE_ARGUMENT, s, &[Arg::from("ref")]);
                }
            }
        } else if self.extensao_ffi(x, &ENDERECO_DE_COMPOSTO) || self.extensao_ffi(x, &ENDERECO_DE_PRIMITIVO) || self.extensao_ffi(x, &ENDERECO_DE_TYPED_DATA) {
            if nome_do_membro == "address" {
                self.validar_posicao_de_endereco(id, nome.span);
                let extensao = self.program.extension(x).name.map(|n| self.nome(n)).unwrap_or("");
                self.validar_receptor_de_endereco(target, extensao, nome.span);
            }
        }
    }

    /// `_validateAddressPosition`: o `.address` só como argumento de uma
    /// chamada a uma função `@Native(isLeaf: true)` (ou pelo `.cast()`).
    fn validar_posicao_de_endereco(&mut self, no: ExprId, erro: Span) {
        let a = self.a;
        let mut atual = no;
        if let Some(p) = self.pai(no)
            && let ExprKind::Property { target, name, .. } = &a.expr(p).kind
            && *target == no
            && self.nome(name.sym) == "cast"
            && let Some(Resolved::Member { member: MemberRef::Function(f), .. }) = self.corpo.get_resolved(p)
            && self.program.function(*f).kind == FunctionKind::Function
            && self.program.function(*f).class.is_some_and(|c| self.classe_ffi(c, "Pointer"))
            && let Some(c) = self.pai(p)
            && matches!(&a.expr(c).kind, ExprKind::Call { target, .. } if *target == p)
        {
            atual = c;
        }
        let ok = match self.pai(atual) {
            Some(g) => match &a.expr(g).kind {
                ExprKind::Call { target, arguments } => {
                    *target != atual && arguments.args.iter().any(|x| x.name.is_none() && x.value == atual) && self.chamada_de_folha(*target)
                }
                _ => false,
            },
            None => false,
        };
        if !ok {
            self.relatar(cf::ADDRESS_POSITION, erro, &[]);
        }
    }

    /// `isNativeLeafInvocation`: a invocação de método cujo elemento é uma
    /// função ou método com `@Native(isLeaf: true)`.
    fn chamada_de_folha(&mut self, alvo: ExprId) -> bool {
        if !matches!(self.a.expr(alvo).kind, ExprKind::Identifier(_) | ExprKind::Property { .. }) {
            return false;
        }
        let f = match self.corpo.get_resolved(alvo) {
            Some(Resolved::Element(Element::Function(f))) => *f,
            Some(Resolved::Member { member: MemberRef::Function(f), .. }) | Some(Resolved::ExtensionMember { member: f, .. }) => *f,
            _ => return false,
        };
        if self.program.function(f).kind != FunctionKind::Function {
            return false;
        }
        let (unidade, metadata) = self.anotacoes_do_elemento(Declaracao::Funcao(f));
        for m in metadata {
            let Some(inst) = self.instancia_da_anotacao(unidade, m) else { continue };
            if self.classe_ffi(inst.classe, "Native") && inst.is_leaf == Some(true) {
                return true;
            }
        }
        false
    }

    /// `_validateAddressReceiver`.
    fn validar_receptor_de_endereco(&mut self, receptor: ExprId, extensao: &str, erro: Span) {
        if ENDERECO_DE_COMPOSTO.contains(&extensao) || ENDERECO_DE_TYPED_DATA.contains(&extensao) {
            return;
        }
        let receptor = self.alvo_real(receptor);
        let ok = match &self.a.expr(receptor).kind {
            // Elemento de `Array` ou de lista tipada.
            ExprKind::Index { target, .. } => {
                let t = self.corpo.get_type(self.alvo_real(*target));
                t.is_some_and(|t| self.e_ffi_t(t, "Array") || self.e_typed_data(t))
            }
            // Campo de `Struct` ou `Union`.
            ExprKind::Property { target, .. } => {
                let t = self.corpo.get_type(self.alvo_real(*target));
                t.is_some_and(|t| self.subtipo_de_composto(t))
            }
            _ => false,
        };
        if !ok {
            self.relatar(cf::ADDRESS_RECEIVER, erro, &[]);
        }
    }
}
