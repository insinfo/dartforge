//! Layout nominal, despacho e handles rastreados. Referências temporárias ficam
//! em slots por ponto estático de expressão e por local. Laços sobrescrevem os
//! slots de expressões; raízes separadas de locais preservam valores de iterações
//! anteriores. Cada ativação recursiva possui seu próprio frame. A quantidade de
//! slots por ativação depende da IR, não da quantidade de iterações. Slots não
//! são limpos na saída lexical: ainda pode haver retenção até sobrescrita/retorno.
//! O SDK 3.6.2 runtime/vm/compiler/backend/flow_graph_compiler.cc usa stack maps;
//! esta implementação conservadora não reproduz sua análise de vivacidade.
//! Handles não expõem endereços, zero representa null e campos de referência são
//! marcados precisamente. Int?/bool? ocupam presença e payload em slots separados.
//! Strings usam UTF-8 interno somente no subconjunto de escalares Unicode aceito
//! pelo frontend; isso não implementa indexação UTF-16 de Dart. Referência consultada:
//! SDK 3.6.2 sdk/lib/core/string.dart. Nenhum código do SDK foi copiado.
//! Interfaces participam da relação nominal e do despacho, sem herdar corpos.
//! Declarações abstratas conservam separadamente assinatura e implementação herdada.
//! Enums usam singletons do runtime; slots 0/1 guardam index/name. Consulte SDK
//! 3.6.2 sdk/lib/core/enum.dart para identidade nominal e ordinal de declaração.
//! Construtores sem nome recebem argumentos avaliados antes da alocação. Campos
//! são inicializados da derivada para a base; corpos executam da base para a derivada.
//! Helpers void preservam return sem perder o handle retornado pelo factory.
//! Initializing formals não são locais no corpo, conforme SDK 3.6.2
//! tests/language/initializing_formal/scope_test.dart; parâmetros comuns podem sombrear campos.
//! Não há super(...) neste subconjunto: bases com parâmetros obrigatórios são diagnosticadas.
use super::*;
use std::cell::RefCell;
use std::collections::BTreeMap;

/// Contrato C do runtime: handles e índices i64, flags i8 e bytes UTF-8.
pub(super) const DECLARATIONS: &str = "
declare i64 @dartforge_gc_push_frame(i64)
declare void @dartforge_gc_set_root(i64, i64, i64)
declare void @dartforge_gc_pop_frame(i64)
declare i64 @dartforge_object_new(i64, i64)
declare i64 @dartforge_enum_get(i64, i64, ptr, i64)
declare i64 @dartforge_object_get(i64, i64)
declare void @dartforge_object_set(i64, i64, i64, i8)
declare i64 @dartforge_object_class(i64)
declare i64 @dartforge_string_new(ptr, i64)
declare i64 @dartforge_string_concat(i64, i64)
declare i8 @dartforge_string_equal(i64, i64)
declare void @dartforge_print_string(i64)
";

#[derive(Clone)]
/// Tipo e posição física de um campo, incluindo campos herdados.
///
/// A mesma estrutura descreve um slot da área de estáticos de
/// [`crate::statics`], que usa exatamente o mesmo protocolo de leitura e escrita
/// por índice; daí a visibilidade dos campos alcançar o módulo irmão.
pub(super) struct Field {
    pub(super) ty: Ty,
    pub(super) offset: usize,
}
#[derive(Clone)]
/// Implementação concreta e assinatura escolhidas para um método nominal.
struct Method {
    signature: Signature,
    owner: u32,
    index: usize,
    implementation: Option<Signature>,
}
/// Layout completo de uma classe após incorporar a cadeia de bases.
struct Layout {
    parent: Option<u32>,
    interfaces: Vec<u32>,
    is_abstract: bool,
    enum_values: Vec<String>,
    fields: BTreeMap<String, Field>,
    methods: BTreeMap<String, Method>,
    slots: usize,
    constructor_parameters: Vec<Ty>,
    /// Construtores nomeados na ordem escrita; o índice compõe o símbolo emitido.
    ///
    /// Não é herdado: um construtor pertence à declaração que o escreve, e
    /// `C.nome(...)` resolve sempre nessa declaração.
    named_constructors: Vec<(String, Vec<Ty>)>,
    /// Métodos estáticos da própria declaração, com o índice escrito.
    ///
    /// Estáticos não participam de herança nem de despacho dinâmico, então esta
    /// tabela nunca recebe entradas da base, ao contrário de `methods`.
    static_methods: BTreeMap<String, (usize, Signature)>,
}
/// Metadados nominais compartilhados e constantes de strings do módulo.
pub(super) struct Objects {
    layouts: BTreeMap<u32, Layout>,
    pub(super) implicit_members: std::collections::BTreeSet<(usize, usize)>,
    /// Tipo estático de cada expressão, indexado pelo intervalo original da AST.
    pub(super) expr_types: BTreeMap<(usize, usize), Type>,
    pub(super) globals: RefCell<Vec<String>>,
    /// Área única de estáticos: variáveis de topo e campos estáticos de classes.
    pub(super) statics: Statics,
    /// Leituras de variável de topo resolvidas pela análise, por intervalo.
    pub(super) global_accesses: std::collections::BTreeSet<(usize, usize)>,
}
impl Objects {
    /// Monta bases antes das derivadas; diagnostica ciclos em HIR externa.
    pub(super) fn new(module: &Module<'_>) -> Result<Self, Diagnostic> {
        let mut layouts: BTreeMap<u32, Layout> = BTreeMap::new();
        while layouts.len() < module.classes.len() {
            let before = layouts.len();
            for class in &module.classes {
                if layouts.contains_key(&class.id)
                    || class
                        .superclass
                        .is_some_and(|id| !layouts.contains_key(&id))
                    || class.interfaces.iter().any(|id| !layouts.contains_key(id))
                {
                    continue;
                }
                let parent = class.superclass.and_then(|id| layouts.get(&id));
                let mut fields = parent.map(|p| p.fields.clone()).unwrap_or_default();
                let mut methods = parent.map(|p| p.methods.clone()).unwrap_or_default();
                let mut slots = parent.map_or(0, |p| p.slots);
                for interface in &class.interfaces {
                    for (name, method) in &layouts[interface].methods {
                        methods.entry(name.clone()).or_insert_with(|| {
                            let mut method = method.clone();
                            method.implementation = None;
                            method
                        });
                    }
                }
                if !class.enum_values.is_empty() {
                    fields.insert(
                        "index".into(),
                        Field {
                            ty: Ty::Int,
                            offset: 0,
                        },
                    );
                    fields.insert(
                        "name".into(),
                        Field {
                            ty: Ty::String,
                            offset: 1,
                        },
                    );
                    slots = 2;
                }
                for field in &class.fields {
                    if let Some(initializer) = &field.initializer {
                        validate_expression(initializer)?;
                    }
                    let ty = value_ty(field.ty, field.span)?;
                    fields.insert(field.name.into(), Field { ty, offset: slots });
                    slots += if matches!(ty, Ty::NullableInt | Ty::NullableBool) {
                        2
                    } else {
                        1
                    };
                }
                for (index, method) in class
                    .methods
                    .iter()
                    .chain(&class.abstract_methods)
                    .enumerate()
                {
                    validate_statements(&method.body)?;
                    // O backend AOT ainda não emite prólogo de opcionais ou nomeados.
                    if let Some(parameter) = method
                        .parameters
                        .iter()
                        .find(|parameter| parameter.kind != ParameterKind::RequiredPositional)
                    {
                        return Err(error(
                            parameter.span,
                            "parâmetros opcionais ou nomeados no backend nativo",
                        ));
                    }
                    let signature = Signature {
                        symbol: format!("df_method_{}_{}", class.id, index),
                        result: ty(method.return_type, method.span)?,
                        parameters: method
                            .parameters
                            .iter()
                            .map(|p| value_ty(p.ty, p.span))
                            .collect::<Result<_, _>>()?,
                    };
                    let implementation = if index < class.methods.len() {
                        Some(signature.clone())
                    } else {
                        methods
                            .get(method.name)
                            .and_then(|m| m.implementation.clone())
                    };
                    methods.insert(
                        method.name.into(),
                        Method {
                            owner: class.id,
                            index,
                            signature,
                            implementation,
                        },
                    );
                }

                let constructor_parameters = match &class.constructor {
                    Some(constructor) => constructor_types(class, constructor)?,
                    None => vec![],
                };
                let mut named_constructors = Vec::with_capacity(class.named_constructors.len());
                for declared in &class.named_constructors {
                    named_constructors.push((
                        declared.name.to_owned(),
                        constructor_types(class, &declared.constructor)?,
                    ));
                }
                // A base só precisa de argumentos escritos quando a derivada não
                // fornece `super(...)`; com a lista de inicialização suportada, a
                // recusa passou a depender de cada variante e vive em `emit`.
                let mut static_methods = BTreeMap::new();
                for (index, method) in class.static_methods.iter().enumerate() {
                    validate_statements(&method.body)?;
                    if let Some(parameter) = method
                        .parameters
                        .iter()
                        .find(|parameter| parameter.kind != ParameterKind::RequiredPositional)
                    {
                        return Err(error(
                            parameter.span,
                            "parâmetros opcionais ou nomeados no backend nativo",
                        ));
                    }
                    let signature = Signature {
                        symbol: format!("df_static_{}_{index}", class.id),
                        result: ty(method.return_type, method.span)?,
                        parameters: method
                            .parameters
                            .iter()
                            .map(|p| value_ty(p.ty, p.span))
                            .collect::<Result<_, _>>()?,
                    };
                    if static_methods
                        .insert(method.name.to_owned(), (index, signature))
                        .is_some()
                    {
                        return Err(Diagnostic::new(
                            "método estático duplicado na HIR LLVM",
                            method.span,
                        ));
                    }
                }
                for member in &class.static_fields {
                    if let Some(initializer) = &member.initializer {
                        validate_expression(initializer)?;
                    }
                }
                layouts.insert(
                    class.id,
                    Layout {
                        parent: class.superclass,
                        interfaces: class.interfaces.clone(),
                        is_abstract: class.is_abstract,
                        enum_values: class.enum_values.iter().map(|s| (*s).to_owned()).collect(),
                        fields,
                        methods,
                        slots,
                        constructor_parameters,
                        named_constructors,
                        static_methods,
                    },
                );
            }
            if before == layouts.len() {
                return Err(error(
                    module.classes[0].span,
                    "hierarquia cíclica ou base ausente",
                ));
            }
        }
        Ok(Self {
            layouts,
            implicit_members: module.resolution.implicit_members.clone(),
            expr_types: module.resolution.expr_types.clone(),
            globals: RefCell::new(vec![]),
            statics: Statics::new(module)?,
            global_accesses: module.resolution.global_accesses.clone(),
        })
    }
    /// Assinatura de um método estático da própria declaração, sem herança.
    pub(super) fn static_method(&self, class: u32, name: &str) -> Option<&Signature> {
        self.layouts
            .get(&class)
            .and_then(|layout| layout.static_methods.get(name))
            .map(|(_, signature)| signature)
    }
    /// Índice escrito e tipos de um construtor nomeado da própria declaração.
    pub(super) fn named_constructor(&self, class: u32, name: &str) -> Option<(usize, &[Ty])> {
        self.layouts
            .get(&class)?
            .named_constructors
            .iter()
            .enumerate()
            .find(|(_, (declared, _))| declared == name)
            .map(|(index, (_, types))| (index, types.as_slice()))
    }
    /// Lista as classes concretas cuja identidade satisfaz um teste nominal.
    ///
    /// `dartforge_object_class` devolve sempre a classe concreta do handle; um teste
    /// contra tipo abstrato ou interface precisa comparar com todos os implementadores.
    pub(super) fn concrete_descendants(&self, id: u32) -> Vec<u32> {
        self.layouts
            .iter()
            .filter(|(child, layout)| {
                !layout.is_abstract && self.assignable(Ty::Class(**child), Ty::Class(id))
            })
            .map(|(child, _)| *child)
            .collect()
    }
    /// Compatibilidade nominal preserva a identidade concreta do objeto.
    pub(super) fn assignable(&self, actual: Ty, expected: Ty) -> bool {
        if actual == expected {
            return true;
        }
        if let (Ty::Class(actual), Ty::Class(expected)) = (actual, expected) {
            let mut pending = vec![actual];
            let mut seen = std::collections::BTreeSet::new();
            while let Some(id) = pending.pop() {
                if !seen.insert(id) {
                    continue;
                }
                if id == expected {
                    return true;
                }
                if let Some(layout) = self.layouts.get(&id) {
                    pending.extend(layout.parent);
                    pending.extend(&layout.interfaces);
                }
            }
        }
        false
    }
    /// Resolve o slot pelo tipo estático do receiver, preservando diagnóstico de origem.
    pub(super) fn field(&self, receiver: Ty, name: &str, span: Span) -> Result<Field, Diagnostic> {
        let Ty::Class(id) = receiver.base() else {
            return Err(error(span, "campo neste tipo"));
        };
        self.layouts
            .get(&id)
            .and_then(|l| l.fields.get(name))
            .cloned()
            .ok_or_else(|| error(span, "campo ausente"))
    }
    /// Emite construção em três camadas, na ordem de inicialização do Dart.
    ///
    /// `df_new_{classe}[_{índice}]` aloca o objeto e devolve o handle.
    /// `df_init_{classe}[_{índice}]` executa, nesta ordem, os inicializadores de
    /// declaração da própria classe (com os formais `this.campo` no lugar),
    /// a lista de inicialização, a construção da base — cujos argumentos são
    /// avaliados nesse ponto — e por fim `df_ctorbody_{classe}[_{índice}]`.
    ///
    /// Como a base inicializa no meio da rotina derivada, o corpo da base termina
    /// antes de o corpo da derivada começar, que é o que o Dart 3.6.2 faz e o que
    /// o backend JavaScript emite. O `_{índice}` é a posição escrita do construtor
    /// nomeado: o símbolo é determinístico e nenhum identificador do usuário entra
    /// na IR, invariante que este crate mantém em todos os símbolos.
    pub(super) fn emit(
        &self,
        module: &Module<'_>,
        signatures: &HashMap<String, Signature>,
    ) -> Result<String, Diagnostic> {
        let mut output = String::new();
        let plain = ConstructorExtras::default();
        for class in &module.classes {
            if !class.is_library_globals
                && class.enum_values.is_empty()
                && class.kind != ClassKind::Mixin
            {
                let mut variants: Vec<(
                    Option<usize>,
                    Option<&Constructor<'_>>,
                    &ConstructorExtras<'_>,
                )> = Vec::with_capacity(class.named_constructors.len() + 1);
                // Uma declaração que só tem construtores nomeados não ganha o sem
                // nome: `C()` não existe e nenhuma derivada pode chamá-lo
                // implicitamente, porque o Dart exige `super.nome(...)` nesse caso.
                if class.constructor.is_some() || class.named_constructors.is_empty() {
                    variants.push((
                        None,
                        class.constructor.as_ref(),
                        class.constructor_extras.as_deref().unwrap_or(&plain),
                    ));
                }
                for (index, declared) in class.named_constructors.iter().enumerate() {
                    variants.push((Some(index), Some(&declared.constructor), &declared.extras));
                }
                for (variant, constructor, extras) in variants {
                    output.push_str(&self.emit_construction(
                        signatures,
                        class,
                        variant,
                        constructor,
                        extras,
                    )?);
                }
            }
            for (index, method) in class.static_methods.iter().enumerate() {
                let signature = self
                    .static_method(class.id, method.name)
                    .expect("método estático registrado no layout")
                    .clone();
                let mut emitter = FunctionEmitter::new(signatures, self, signature.result);
                let mut parameters = vec![];
                for (position, parameter) in method.parameters.iter().enumerate() {
                    let ty = signature.parameters[position];
                    let value = Value {
                        ty,
                        text: format!("%a{position}"),
                    };
                    parameters.push(format!("{} %a{position}", ty.ir()));
                    emitter.root(&value);
                    let pointer = emitter.local(parameter.name, ty);
                    emitter.root_local(&pointer, &value);
                    emitter.line(format!("store {} {}, ptr {pointer}", ty.ir(), value.text));
                }
                // Um estático não recebe receptor: `this` dentro dele é erro da
                // análise semântica, e o símbolo não tem parâmetro para ele.
                emitter.block(&method.body)?;
                output.push_str(&emitter.finish(
                    &format!("df_static_{}_{index}", class.id),
                    &parameters.join(", "),
                ));
            }
            for (index, method) in class.methods.iter().enumerate() {
                let info = &self.layouts[&class.id].methods[method.name];
                let mut emitter = FunctionEmitter::new(signatures, self, info.signature.result);
                emitter.this_class = Some(class.id);
                emitter.root(&Value {
                    ty: Ty::Class(class.id),
                    text: "%this".into(),
                });
                let mut params = vec!["i64 %this".into()];
                for (i, p) in method.parameters.iter().enumerate() {
                    let t = info.signature.parameters[i];
                    params.push(format!("{} %a{i}", t.ir()));
                    emitter.root(&Value {
                        ty: t,
                        text: format!("%a{i}"),
                    });
                    let ptr = emitter.local(p.name, t);
                    emitter.root_local(
                        &ptr,
                        &Value {
                            ty: t,
                            text: format!("%a{i}"),
                        },
                    );
                    emitter.line(format!("store {} %a{i}, ptr {ptr}", t.ir()));
                }
                emitter.block(&method.body)?;
                output.push_str(&emitter.finish(
                    &format!("df_method_{}_{index}", class.id),
                    &params.join(", "),
                ));
            }
        }
        // Adaptadores preservam contravariancia dos parametros e retorno covariante.
        for (&id, layout) in &self.layouts {
            for (name, method) in &layout.methods {
                let sig = &method.signature;
                let mut emitter = FunctionEmitter::new(signatures, self, sig.result);
                let mut params = vec!["i64 %this".into()];
                emitter.root(&Value {
                    ty: Ty::Class(id),
                    text: "%this".into(),
                });
                for (i, t) in sig.parameters.iter().enumerate() {
                    params.push(format!("{} %a{i}", t.ir()));
                    emitter.root(&Value {
                        ty: *t,
                        text: format!("%a{i}"),
                    });
                }
                emitter.line("%class = call i64 @dartforge_object_class(i64 %this)".into());
                emitter.line("switch i64 %class, label %invalid [".into());
                let descendants = self
                    .layouts
                    .iter()
                    .filter(|(child, layout)| {
                        !layout.is_abstract && self.assignable(Ty::Class(**child), Ty::Class(id))
                    })
                    .collect::<Vec<_>>();
                for (child, _) in &descendants {
                    emitter.line(format!("  i64 {child}, label %case{child}"));
                }
                emitter.line("]".into());
                emitter.terminated = true;
                emitter.start("invalid");
                emitter.line("call void @dartforge_null_assert_fail()".into());
                emitter.line("unreachable".into());
                emitter.terminated = true;
                for (child, descendant) in descendants {
                    emitter.start(&format!("case{child}"));
                    let concrete = &descendant.methods[name];
                    let target = concrete.implementation.as_ref().ok_or_else(|| {
                        error(
                            Span { start: 0, end: 0 },
                            "metodo abstrato sem implementacao concreta",
                        )
                    })?;
                    let mut args = vec!["i64 %this".into()];
                    for (i, (actual, expected)) in
                        sig.parameters.iter().zip(&target.parameters).enumerate()
                    {
                        let value = emitter.coerce(
                            Value {
                                ty: *actual,
                                text: format!("%a{i}"),
                            },
                            *expected,
                            Span { start: 0, end: 0 },
                        )?;
                        args.push(format!("{} {}", value.ty.ir(), value.text));
                    }
                    let value = if target.result == Ty::Void {
                        emitter.line(format!("call void @{}({})", target.symbol, args.join(", ")));
                        Value {
                            ty: Ty::Void,
                            text: String::new(),
                        }
                    } else {
                        let r = emitter.register();
                        emitter.line(format!(
                            "{r} = call {} @{}({})",
                            target.result.ir(),
                            target.symbol,
                            args.join(", ")
                        ));
                        Value {
                            ty: target.result,
                            text: r,
                        }
                    };
                    // Um contrato void permite implementação que retorna valor;
                    // a chamada mantém seus efeitos e o adaptador descarta o resultado.
                    let value = if sig.result == Ty::Void {
                        Value {
                            ty: Ty::Void,
                            text: String::new(),
                        }
                    } else {
                        emitter.coerce(value, sig.result, Span { start: 0, end: 0 })?
                    };
                    emitter.end_frame();
                    emitter.line(if sig.result == Ty::Void {
                        "ret void".into()
                    } else {
                        format!("ret {} {}", value.ty.ir(), value.text)
                    });
                    emitter.terminated = true;
                }
                output.push_str(&emitter.finish(
                    &format!("df_dispatch_{id}_{}_{}", method.owner, method.index),
                    &params.join(", "),
                ));
            }
        }
        Ok(output)
    }

    /// Emite as três funções de uma variante de construção da classe.
    ///
    /// `variant` é `None` para o construtor sem nome e `Some(índice)` para o
    /// nomeado na posição escrita. A entrada `df_new_` só existe para classes
    /// concretas; `df_init_` existe também nas abstratas, porque uma derivada
    /// precisa chamá-la para inicializar o prefixo herdado.
    ///
    /// # Erros
    /// Recusa campo não anulável sem valor, lista de inicialização sobre campo
    /// herdado, `super.nome` inexistente na base e aridade de `super` incorreta.
    fn emit_construction(
        &self,
        signatures: &HashMap<String, Signature>,
        class: &Class<'_>,
        variant: Option<usize>,
        constructor: Option<&Constructor<'_>>,
        extras: &ConstructorExtras<'_>,
    ) -> Result<String, Diagnostic> {
        let layout = &self.layouts[&class.id];
        let suffix = variant.map_or_else(String::new, |index| format!("_{index}"));
        let types: &[Ty] = match variant {
            None => &layout.constructor_parameters,
            Some(index) => &layout.named_constructors[index].1,
        };
        let mut output = String::new();
        let header = |emitter: &mut FunctionEmitter<'_>| {
            let mut parameters = Vec::with_capacity(types.len());
            for (index, ty) in types.iter().enumerate() {
                parameters.push(format!("{} %a{index}", ty.ir()));
                emitter.root(&Value {
                    ty: *ty,
                    text: format!("%a{index}"),
                });
            }
            parameters
        };
        let arguments = |prefix: &str| {
            let mut values = vec![prefix.to_owned()];
            for (index, ty) in types.iter().enumerate() {
                values.push(format!("{} %a{index}", ty.ir()));
            }
            values.join(", ")
        };
        if !class.is_abstract {
            let mut emitter = FunctionEmitter::new(signatures, self, Ty::Class(class.id));
            let parameters = header(&mut emitter);
            emitter.line(format!(
                "%this = call i64 @dartforge_object_new(i64 {}, i64 {})",
                class.id, layout.slots
            ));
            emitter.root(&Value {
                ty: Ty::Class(class.id),
                text: "%this".into(),
            });
            emitter.line(format!(
                "call void @df_init_{}{suffix}({})",
                class.id,
                arguments("i64 %this")
            ));
            emitter.end_frame();
            emitter.line("ret i64 %this".into());
            emitter.terminated = true;
            output.push_str(&emitter.finish(
                &format!("df_new_{}{suffix}", class.id),
                &parameters.join(", "),
            ));
        }
        let mut emitter = FunctionEmitter::new(signatures, self, Ty::Void);
        emitter.this_class = Some(class.id);
        emitter.root(&Value {
            ty: Ty::Class(class.id),
            text: "%this".into(),
        });
        let mut parameters = vec!["i64 %this".to_owned()];
        parameters.extend(header(&mut emitter));
        // A lista de inicialização e os argumentos de `super` enxergam os
        // parâmetros comuns do construtor pelo nome escrito. Um formal
        // `this.campo` não entra em escopo, conforme SDK 3.6.2
        // tests/language/initializing_formal/scope_test.dart.
        if let Some(constructor) = constructor {
            for (index, parameter) in constructor.parameters.iter().enumerate() {
                if parameter.field.is_some() {
                    continue;
                }
                let value = Value {
                    ty: types[index],
                    text: format!("%a{index}"),
                };
                let pointer = emitter.local(parameter.name, value.ty);
                emitter.root_local(&pointer, &value);
                emitter.line(format!(
                    "store {} {}, ptr {pointer}",
                    value.ty.ir(),
                    value.text
                ));
            }
        }
        for field in &class.fields {
            let formal = constructor.and_then(|declared| {
                declared
                    .parameters
                    .iter()
                    .position(|parameter| parameter.field == Some(field.name))
            });
            // Mesmo um formal que sobrescreve o campo preserva os efeitos do initializer.
            let initialized = field
                .initializer
                .as_ref()
                .map(|initializer| emitter.expression(initializer))
                .transpose()?;
            let slot = layout.fields[field.name].clone();
            let value = if let Some(index) = formal {
                Value {
                    ty: types[index],
                    text: format!("%a{index}"),
                }
            } else if let Some(initialized) = initialized {
                initialized
            } else {
                // A lista de inicialização grava o campo logo depois; escrever
                // null antes seria trabalho perdido e não é observável.
                if extras
                    .initializers
                    .iter()
                    .any(|entry| entry.field == field.name)
                {
                    continue;
                }
                if !slot.ty.nullable() {
                    return Err(error(
                        field.span,
                        "campo não nullable sem inicializador ou initializing formal",
                    ));
                }
                Value {
                    ty: Ty::Null,
                    text: "zeroinitializer".into(),
                }
            };
            emitter.store_field("%this", &slot, value, field.span)?;
        }
        for entry in &extras.initializers {
            // Dart só admite campo da própria declaração numa lista de
            // inicialização; o layout também tem os herdados e não serve de filtro.
            if !class.fields.iter().any(|field| field.name == entry.field) {
                return Err(error(
                    entry.span,
                    "lista de inicialização sobre campo que não é da declaração",
                ));
            }
            let slot = layout.fields[entry.field].clone();
            let value = emitter.expression(&entry.value)?;
            emitter.store_field("%this", &slot, value, entry.span)?;
        }
        if let Some(base) = class.superclass {
            let span = extras
                .super_call
                .as_ref()
                .map_or(class.span, |call| call.span);
            let target = extras.super_call.as_ref().and_then(|call| call.name);
            let (base_suffix, base_types) = match target {
                None => (
                    String::new(),
                    self.layouts[&base].constructor_parameters.clone(),
                ),
                Some(name) => {
                    let (index, base_types) = self
                        .named_constructor(base, name)
                        .ok_or_else(|| error(span, "construtor nomeado ausente na base"))?;
                    (format!("_{index}"), base_types.to_vec())
                }
            };
            let written = extras
                .super_call
                .as_ref()
                .map_or(&[][..], |call| call.arguments.as_slice());
            if written.len() != base_types.len() {
                return Err(error(
                    span,
                    "construtor da base com aridade diferente da chamada de super",
                ));
            }
            let mut values = vec!["i64 %this".to_owned()];
            for (argument, expected) in written.iter().zip(&base_types) {
                let value = emitter.expression(argument)?;
                let value = emitter.coerce(value, *expected, argument.span)?;
                values.push(format!("{} {}", expected.ir(), value.text));
            }
            emitter.line(format!(
                "call void @df_init_{base}{base_suffix}({})",
                values.join(", ")
            ));
        }
        let body = constructor.filter(|declared| !declared.body.is_empty());
        if body.is_some() {
            emitter.line(format!(
                "call void @df_ctorbody_{}{suffix}({})",
                class.id,
                arguments("i64 %this")
            ));
        }
        output.push_str(&emitter.finish(
            &format!("df_init_{}{suffix}", class.id),
            &parameters.join(", "),
        ));
        if let Some(constructor) = body {
            let mut emitter = FunctionEmitter::new(signatures, self, Ty::Void);
            emitter.this_class = Some(class.id);
            emitter.root(&Value {
                ty: Ty::Class(class.id),
                text: "%this".into(),
            });
            let mut parameters = vec!["i64 %this".to_owned()];
            for (index, parameter) in constructor.parameters.iter().enumerate() {
                let ty = types[index];
                let value = Value {
                    ty,
                    text: format!("%a{index}"),
                };
                parameters.push(format!("{} %a{index}", ty.ir()));
                emitter.root(&value);
                if parameter.field.is_none() {
                    let pointer = emitter.local(parameter.name, ty);
                    emitter.root_local(&pointer, &value);
                    emitter.line(format!("store {} {}, ptr {pointer}", ty.ir(), value.text));
                }
            }
            emitter.block(&constructor.body)?;
            output.push_str(&emitter.finish(
                &format!("df_ctorbody_{}{suffix}", class.id),
                &parameters.join(", "),
            ));
        }
        Ok(output)
    }

    /// Cria e preenche a área de estáticos no prólogo de `dartforge_entry`.
    ///
    /// A ordem é a de [`crate::statics`]: campos estáticos por classe em ordem de
    /// herança, depois as variáveis de topo na ordem escrita. O handle fica em
    /// `@df_statics` e é enraizado no frame da entrada, que vive até o fim do
    /// programa; os campos passam a ser rastreados pelo GC como qualquer objeto.
    ///
    /// # Erros
    /// Propaga os diagnósticos das expressões de inicialização.
    pub(super) fn emit_statics(
        &self,
        emitter: &mut FunctionEmitter<'_>,
        module: &Module<'_>,
    ) -> Result<(), Diagnostic> {
        if self.statics.slots == 0 {
            return Ok(());
        }
        self.globals.borrow_mut().push(format!(
            "{} = internal global i64 0",
            crate::statics::STATICS_HANDLE
        ));
        let handle = emitter.register();
        emitter.line(format!(
            "{handle} = call i64 @dartforge_object_new(i64 {}, i64 {})",
            crate::statics::STATICS_CLASS,
            self.statics.slots
        ));
        let slot = emitter.reserve_root();
        emitter.line(format!(
            "call void @dartforge_gc_set_root(i64 %gcframe, i64 {slot}, i64 {handle})"
        ));
        emitter.line(format!(
            "store i64 {handle}, ptr {}",
            crate::statics::STATICS_HANDLE
        ));
        for (class_id, written) in &self.statics.ordem {
            let member = self.statics.declaracao(module, *class_id, *written)?;
            let slot = match class_id {
                Some(id) => self.statics.class_field(*id, member.name),
                None => self.statics.global(member.name),
            }
            .expect("estático registrado na área")
            .clone();
            let value = match &member.initializer {
                Some(initializer) => emitter.expression(initializer)?,
                // A análise semântica só dispensa o valor escrito quando o tipo
                // aceita null; o slot já está zerado, mas a escrita explícita
                // mantém a forma da IR igual nos dois caminhos.
                None => Value {
                    ty: Ty::Null,
                    text: "zeroinitializer".into(),
                },
            };
            emitter.store_static(&slot, value, member.span)?;
        }
        Ok(())
    }
}

impl FunctionEmitter<'_> {
    /// Carrega o handle da área de estáticos gravado pelo prólogo da entrada.
    pub(super) fn statics_handle(&mut self) -> String {
        let register = self.register();
        self.line(format!(
            "{register} = load i64, ptr {}",
            crate::statics::STATICS_HANDLE
        ));
        register
    }
    /// Lê um estático da área única, com o mesmo protocolo de campo de instância.
    pub(super) fn load_static(&mut self, slot: &Field) -> Value {
        let handle = self.statics_handle();
        let value = self.load_field(&handle, slot);
        self.root(&value);
        value
    }
    /// Grava um estático na área única, marcando referências para o GC.
    ///
    /// # Erros
    /// Recusa valor incompatível com o tipo declarado do estático.
    pub(super) fn store_static(
        &mut self,
        slot: &Field,
        value: Value,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let handle = self.statics_handle();
        self.store_field(&handle, slot, value, span)
    }
    /// Avalia argumentos em ordem e protege referências antes da alocação no construtor.
    pub(super) fn construct(
        &mut self,
        class_id: u32,
        arguments: &[Expr<'_>],
        span: Span,
    ) -> Result<Value, Diagnostic> {
        let layout = self
            .objects
            .layouts
            .get(&class_id)
            .ok_or_else(|| error(span, "classe ausente"))?;
        if layout.is_abstract || !layout.enum_values.is_empty() {
            return Err(error(span, "instanciação de classe abstrata ou enum"));
        }
        let types = layout.constructor_parameters.clone();
        if types.len() != arguments.len() {
            return Err(error(span, "quantidade de argumentos do construtor"));
        }
        let mut values = vec![];
        for (argument, ty) in arguments.iter().zip(types) {
            let value = self.expression(argument)?;
            let value = self.coerce(value, ty, argument.span)?;
            values.push(format!("{} {}", ty.ir(), value.text));
        }
        let result = self.register();
        self.line(format!(
            "{result} = call i64 @df_new_{class_id}({})",
            values.join(", ")
        ));
        Ok(Value {
            ty: Ty::Class(class_id),
            text: result,
        })
    }
    /// Baixa `C.nome(...)`: construtor nomeado ou método estático da declaração.
    ///
    /// A ordem de resolução é a da análise semântica — fábrica, construtor
    /// nomeado, método estático — e as fábricas já foram recusadas antes da
    /// emissão. Nada aqui consulta a superclasse: estáticos e construtores
    /// pertencem à declaração escrita e não são herdados.
    ///
    /// # Erros
    /// Recusa nome ausente na declaração, classe abstrata ou enum na construção e
    /// aridade diferente da declarada.
    pub(super) fn named_construct(
        &mut self,
        class_id: u32,
        name: &str,
        arguments: &[Expr<'_>],
        span: Span,
    ) -> Result<Value, Diagnostic> {
        if let Some((index, types)) = self.objects.named_constructor(class_id, name) {
            let types = types.to_vec();
            let layout = &self.objects.layouts[&class_id];
            if layout.is_abstract || !layout.enum_values.is_empty() {
                return Err(error(span, "instanciação de classe abstrata ou enum"));
            }
            if types.len() != arguments.len() {
                return Err(error(span, "quantidade de argumentos do construtor"));
            }
            let mut values = vec![];
            for (argument, ty) in arguments.iter().zip(types) {
                let value = self.expression(argument)?;
                let value = self.coerce(value, ty, argument.span)?;
                values.push(format!("{} {}", ty.ir(), value.text));
            }
            let result = self.register();
            self.line(format!(
                "{result} = call i64 @df_new_{class_id}_{index}({})",
                values.join(", ")
            ));
            return Ok(Value {
                ty: Ty::Class(class_id),
                text: result,
            });
        }
        let signature = self
            .objects
            .static_method(class_id, name)
            .ok_or_else(|| error(span, "construtor nomeado ou método estático ausente"))?
            .clone();
        if signature.parameters.len() != arguments.len() {
            return Err(error(span, "aridade de método estático"));
        }
        let mut values = vec![];
        for (argument, ty) in arguments.iter().zip(&signature.parameters) {
            let value = self.expression(argument)?;
            let value = self.coerce(value, *ty, argument.span)?;
            values.push(format!("{} {}", ty.ir(), value.text));
        }
        if signature.result == Ty::Void {
            self.line(format!(
                "call void @{}({})",
                signature.symbol,
                values.join(", ")
            ));
            return Ok(Value {
                ty: Ty::Void,
                text: String::new(),
            });
        }
        let result = self.register();
        self.line(format!(
            "{result} = call {} @{}({})",
            signature.result.ir(),
            signature.symbol,
            values.join(", ")
        ));
        Ok(Value {
            ty: signature.result,
            text: result,
        })
    }
    /// Usa o dono físico do método clonado para localizar campos após o prefixo da base.
    pub(super) fn this_value(&self, span: Span) -> Result<Value, Diagnostic> {
        Ok(Value {
            ty: Ty::Class(
                self.this_class
                    .ok_or_else(|| error(span, "membro implicito fora de metodo"))?,
            ),
            text: "%this".into(),
        })
    }
    /// A análise distingue acesso a campo de getter; ambos recebem o mesmo receiver.
    pub(super) fn read_member(
        &mut self,
        receiver: Value,
        name: &str,
        span: Span,
    ) -> Result<Value, Diagnostic> {
        if let Ok(field) = self.objects.field(receiver.ty, name, span) {
            Ok(self.load_field(&receiver.text, &field))
        } else {
            self.method_call(receiver, name, &[], span)
        }
    }
    /// Materializa singleton canônico; o runtime mantém a raiz persistente do enum.
    pub(super) fn enum_value(
        &mut self,
        class_id: u32,
        name: &str,
        span: Span,
    ) -> Result<Value, Diagnostic> {
        let index = self
            .objects
            .layouts
            .get(&class_id)
            .and_then(|layout| layout.enum_values.iter().position(|value| value == name))
            .ok_or_else(|| error(span, "valor de enum inexistente"))?;
        let id = self.objects.globals.borrow().len();
        let bytes = name
            .as_bytes()
            .iter()
            .map(|b| format!("\\{b:02X}"))
            .collect::<String>();
        self.objects.globals.borrow_mut().push(format!(
            "@df_string_{id} = private constant [{} x i8] c\"{bytes}\"",
            name.len()
        ));
        let register = self.register();
        self.line(format!("{register} = call i64 @dartforge_enum_get(i64 {class_id}, i64 {index}, ptr @df_string_{id}, i64 {})",name.len()));
        Ok(Value {
            ty: Ty::Class(class_id),
            text: register,
        })
    }
    /// Reserva um slot estático, compartilhado apenas por reexecuções deste ponto.
    pub(super) fn reserve_root(&mut self) -> usize {
        self.has_roots = true;
        let slot = self.root_slots;
        self.root_slots += 1;
        slot
    }
    /// Atualiza o slot de um local independentemente do temporário que o originou.
    pub(super) fn root_local(&mut self, pointer: &str, value: &Value) {
        if let Some(slot) = self.local_roots.get(pointer).copied() {
            self.line(format!(
                "call void @dartforge_gc_set_root(i64 %gcframe, i64 {slot}, i64 {})",
                value.text
            ));
        }
    }
    /// Mantém temporários vivos; cada ponto de emissão possui slot próprio reutilizável.
    pub(super) fn root(&mut self, value: &Value) {
        if value.ty.reference() {
            let slot = self.reserve_root();
            self.line(format!(
                "call void @dartforge_gc_set_root(i64 %gcframe, i64 {slot}, i64 {})",
                value.text
            ));
        }
    }
    /// Literais usam bytes UTF-8 escapados: nenhum texto Dart vira sintaxe LLVM.
    pub(super) fn string(&mut self, text: &str) -> Value {
        let id = self.objects.globals.borrow().len();
        let len = text.len();
        let bytes = text
            .as_bytes()
            .iter()
            .map(|b| format!("\\{b:02X}"))
            .collect::<String>();
        self.objects.globals.borrow_mut().push(format!(
            "@df_string_{id} = private constant [{len} x i8] c\"{bytes}\""
        ));
        let r = self.register();
        self.line(format!(
            "{r} = call i64 @dartforge_string_new(ptr @df_string_{id}, i64 {len})"
        ));
        Value {
            ty: Ty::String,
            text: r,
        }
    }
    /// Grava payload e presença; informa ao GC quais slots contêm referências.
    pub(super) fn store_field(
        &mut self,
        receiver: &str,
        field: &Field,
        value: Value,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let value = self.coerce(value, field.ty, span)?;
        let mut offset = field.offset;
        let value = if matches!(value.ty, Ty::NullableInt | Ty::NullableBool) {
            let tag = self.present(&value);
            let r = self.register();
            self.line(format!("{r} = zext i1 {tag} to i64"));
            self.line(format!(
                "call void @dartforge_object_set(i64 {receiver}, i64 {offset}, i64 {r}, i8 0)"
            ));
            offset += 1;
            self.payload(&value)
        } else {
            value
        };
        let bits = if value.ty == Ty::Bool {
            let r = self.register();
            self.line(format!("{r} = zext i1 {} to i64", value.text));
            r
        } else {
            value.text
        };
        self.line(format!(
            "call void @dartforge_object_set(i64 {receiver}, i64 {offset}, i64 {bits}, i8 {})",
            u8::from(field.ty.reference())
        ));
        Ok(())
    }
    /// Reconstrói o valor tipado dos slots; a expressão chamadora enraíza referências.
    pub(super) fn load_field(&mut self, receiver: &str, field: &Field) -> Value {
        let r = self.register();
        self.line(format!(
            "{r} = call i64 @dartforge_object_get(i64 {receiver}, i64 {})",
            field.offset
        ));
        if matches!(field.ty, Ty::NullableInt | Ty::NullableBool) {
            let tag = self.register();
            self.line(format!("{tag} = trunc i64 {r} to i1"));
            let payload = self.register();
            self.line(format!(
                "{payload} = call i64 @dartforge_object_get(i64 {receiver}, i64 {})",
                field.offset + 1
            ));
            let payload = if field.ty == Ty::NullableBool {
                let b = self.register();
                self.line(format!("{b} = trunc i64 {payload} to i1"));
                b
            } else {
                payload
            };
            let a = self.register();
            let b = self.register();
            self.line(format!(
                "{a} = insertvalue {} zeroinitializer, i1 {tag}, 0",
                field.ty.ir()
            ));
            self.line(format!(
                "{b} = insertvalue {} {a}, {} {payload}, 1",
                field.ty.ir(),
                field.ty.base().ir()
            ));
            Value {
                ty: field.ty,
                text: b,
            }
        } else if field.ty == Ty::Bool {
            let b = self.register();
            self.line(format!("{b} = trunc i64 {r} to i1"));
            Value {
                ty: Ty::Bool,
                text: b,
            }
        } else {
            Value {
                ty: field.ty,
                text: r,
            }
        }
    }
    /// Avalia receiver e argumentos em ordem antes do despacho pela classe concreta.
    pub(super) fn method_call(
        &mut self,
        receiver: Value,
        name: &str,
        args: &[Expr<'_>],
        span: Span,
    ) -> Result<Value, Diagnostic> {
        let Ty::Class(id) = receiver.ty.base() else {
            return Err(error(span, "metodo neste tipo"));
        };
        let receiver = self.coerce(receiver, Ty::Class(id), span)?;
        let method = self
            .objects
            .layouts
            .get(&id)
            .and_then(|l| l.methods.get(name))
            .cloned()
            .ok_or_else(|| error(span, "metodo ausente"))?;
        if args.len() != method.signature.parameters.len() {
            return Err(error(span, "aridade de metodo"));
        }
        let mut values = vec![format!("i64 {}", receiver.text)];
        for (arg, t) in args.iter().zip(&method.signature.parameters) {
            let v = self.expression(arg)?;
            let v = self.coerce(v, *t, arg.span)?;
            values.push(format!("{} {}", t.ir(), v.text));
        }
        let symbol = format!("df_dispatch_{id}_{}_{}", method.owner, method.index);
        let result = method.signature.result;
        let text = if result == Ty::Void {
            self.line(format!("call void @{symbol}({})", values.join(", ")));
            String::new()
        } else {
            let r = self.register();
            self.line(format!(
                "{r} = call {} @{symbol}({})",
                result.ir(),
                values.join(", ")
            ));
            r
        };
        Ok(Value { ty: result, text })
    }
}

/// Tipos dos parâmetros de um construtor, nomeado ou não.
///
/// Um formal `this.campo` recebe o tipo do campo, não o escrito no parâmetro:
/// é o campo que determina a largura do slot e a conversão aplicada.
///
/// # Erros
/// Recusa parâmetros opcionais ou nomeados, tipos fora do subconjunto nativo e
/// formais que apontam para campo inexistente na declaração.
fn constructor_types(
    class: &Class<'_>,
    constructor: &Constructor<'_>,
) -> Result<Vec<Ty>, Diagnostic> {
    validate_statements(&constructor.body)?;
    if let Some(parameter) = constructor
        .parameters
        .iter()
        .find(|parameter| parameter.kind != ParameterKind::RequiredPositional)
    {
        return Err(error(
            parameter.span,
            "parâmetros opcionais ou nomeados no backend nativo",
        ));
    }
    constructor
        .parameters
        .iter()
        .map(|parameter| {
            let parameter_type = if let Some(field) = parameter.field {
                class
                    .fields
                    .iter()
                    .find(|candidate| candidate.name == field)
                    .ok_or_else(|| error(parameter.span, "campo de initializing formal ausente"))?
                    .ty
            } else {
                parameter.ty
            };
            value_ty(parameter_type, parameter.span)
        })
        .collect()
}
