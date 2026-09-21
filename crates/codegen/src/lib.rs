//! Gera módulos JavaScript para o subconjunto Dart validado semanticamente.
//!
//! Literais inteiros são i32, mas operações usam Number do JavaScript; precisão
//! e transbordamento ainda não equivalem à semântica completa de inteiros Dart.
//! Parênteses preservam a árvore e a ordem de avaliação. Negação e multiplicação
//! normalizam zero inteiro para zero positivo. Não reproduzimos a variação de
//! impressão de zero negativo observada no dart2js 3.6.2 conforme outros tipos
//! impressos no mesmo programa. Laços nativos preservam break e continue.
//! A asserção de não nulo avalia seu operando uma vez e lança TypeError do
//! JavaScript para null; isso não implementa integralmente o TypeError Dart.
//! Classes usam métodos nativos e construtor sem argumentos que preserva a ordem Dart, com
//! herança única previamente validada. Não incluem reflexão ou runtimeType Dart.
//! Cascades usam uma função arrow lexical por receptor, preservando this e o valor
//! original. O guard de ?.. antecede todas as seções; cascades aninhados mantêm
//! contextos separados. Referência: SDK 3.6.2 tests/language/cascade/nested_test.dart.
use dartforge_hir::Module;
use dartforge_syntax::{
    BinaryOp, Class, Expr, ExprKind, Function, Statement, StatementKind, Type, UnaryOp,
};
use std::fmt::Write;
/// Rejeita bindings nativos antes da emissão JavaScript, inclusive em código morto.
/// # Erros
/// FFI exige o backend Native AOT e não pode virar um corpo JavaScript vazio.
pub fn validate_javascript(module: &Module<'_>) -> Result<(), dartforge_diagnostics::Diagnostic> {
    if let Some(binding) = module
        .functions
        .iter()
        .find_map(|function| function.native_binding.as_ref())
    {
        return Err(dartforge_diagnostics::Diagnostic::new(
            "@Native não é suportado pelo backend JavaScript",
            binding.span,
        ));
    }
    Ok(())
}
mod asynchronous;
mod colecoes;
mod constructors;
mod features;
mod fluxo;
mod strings;
mod types;

/// Estado local de emissão com a resolução estática fornecida pela análise.
struct Output<'a> {
    text: String,
    resolution: &'a dartforge_syntax::Resolution,
    collections: bool,
    /// Alguma interpolação precisou converter um valor que não é String.
    strings_used: bool,
    runtime_types_used: bool,
    records: bool,
    next_record: usize,
    next_cascade: usize,
    cascade_receivers: Vec<String>,
    modulo_used: bool,
    async_used: bool,
    enum_ids: std::collections::HashSet<u32>,
    constructor_factories: std::collections::HashSet<u32>,
    nominal_members: std::collections::HashMap<u32, Vec<u32>>,
    next_switch: usize,
    next_wildcard: usize,
    /// Numera os temporários de `for-in` para não colidirem no mesmo escopo.
    next_for_in: usize,
    break_targets: Vec<Option<String>>,
    /// Numera as variáveis de `catch` para não colidirem em try aninhados.
    next_catch: usize,
    /// Variáveis de captura ativas; `rethrow` usa a do topo.
    caught: Vec<String>,
    /// Alguma expressão `throw` precisou do auxiliar de lançamento.
    throw_used: bool,
    /// Alguma asserção foi emitida e exige o construtor do erro.
    assert_used: bool,
    /// Alguma cláusula ligou o rastro de pilha de `catch (e, s)`.
    stack_used: bool,
    /// Alguma constante é instância `const` e exige o auxiliar de canonicalização.
    const_instance_used: bool,
    /// Alguma célula `late` exigiu o erro de leitura-antes-escrita.
    late_used: bool,
    /// Algum double precisou do formatador `$dartforgeDouble` (toString Dart).
    double_used: bool,
    /// Alguma divisão `~/` precisou do auxiliar de truncamento.
    truncdiv_used: bool,
    /// Algum `%` com double precisou do módulo euclidiano sem lançamento.
    doublemod_used: bool,
    /// Algum deslocamento precisou dos auxiliares de bits do alvo web.
    shift_used: bool,
    /// Algum `...?` precisou do auxiliar que omite o operando null.
    spread_used: bool,
    /// Numera os acumuladores dos literais com `if`/`for`.
    next_build: usize,
    /// Numera os temporários das cadeias null-aware.
    next_short: usize,
    /// Receptores sintéticos ativos; `NullShortTarget` usa o do topo.
    null_short_targets: Vec<String>,
    /// Alguma comparação precisou despachar `operator ==` em tempo de execução.
    equals_used: bool,
    /// O módulo declara uma função de topo chamada `identical`.
    ///
    /// Quando declara, `identical(...)` é chamada comum; quando não, é o
    /// intrínseco de identidade de `dart:core`. O linker renomeia nomes de
    /// topo, então a checagem por nome é exata nas duas montagens.
    declares_identical: bool,
    /// Classes do módulo para o despacho estático de `C.x`, `C.m()` e nomeados.
    classes: &'a [Class<'a>],
}
impl std::ops::Deref for Output<'_> {
    type Target = String;
    /// Permite consultar o texto sem copiar o buffer.
    fn deref(&self) -> &String {
        &self.text
    }
}
impl std::ops::DerefMut for Output<'_> {
    /// Encaminha escrita ao buffer único do módulo.
    fn deref_mut(&mut self) -> &mut String {
        &mut self.text
    }
}

/// Gera um módulo ES com funções, exportação de main e chamada inicial de main.
///
/// O módulo deve ter passado pela análise semântica: nomes válidos, tipos
/// compatíveis, break/continue dentro de laços e cabeçalhos de for válidos.
/// A resolução deve incluir cada chamada de extension selecionada semanticamente;
/// chamadas sem entrada são tratadas como métodos de instância. O emissor não
/// infere tipos nem escolhe extensions. Não inclui runtime Dart completo, mapas
/// de origem ou otimizações globais.
///
/// # Pânicos
///
/// Falha se um cabeçalho de for contém uma instrução incompatível. A inicialização
/// aceita variável, atribuição ou expressão; a atualização aceita somente as
/// duas últimas. Também falha em hierarquias cíclicas ou bases ausentes.
/// Essas falhas indicam uma violação do contrato interno da AST.
///
/// ```
/// use dartforge_hir::lower;
/// use dartforge_syntax::Program;
/// let module = lower(Program { main_is_arrow: false, main_is_async: false, types: vec![], extensions: vec![], classes: vec![], functions: vec![], statements: vec![] });
/// let javascript = dartforge_codegen::emit(&module);
/// assert!(javascript.contains("export function main()"));
/// assert!(javascript.ends_with("main();\n"));
/// ```
pub fn emit(module: &Module<'_>) -> String {
    let mut output = Output {
        text: String::from("// Saída do subconjunto DartForge\n"),
        resolution: &module.resolution,
        modulo_used: false,
        strings_used: false,
        async_used: module.main_is_async
            || module
                .resolution
                .types
                .iter()
                .any(|t| matches!(t, dartforge_syntax::TypeShape::Future(_)))
            || module
                .resolution
                .expr_types
                .values()
                .any(|t| matches!(t, Type::Duration | Type::Timer)),
        runtime_types_used: false,
        records: module
            .resolution
            .types
            .iter()
            .any(|ty| matches!(ty, dartforge_syntax::TypeShape::Record { .. })),
        next_record: 0,
        next_cascade: 0,
        cascade_receivers: vec![],
        constructor_factories: constructors::factory_ids(&module.classes),
        enum_ids: module
            .classes
            .iter()
            .filter(|c| !c.enum_values.is_empty())
            .map(|c| c.id)
            .collect(),
        nominal_members: features::nominal_members(&module.classes),
        next_switch: 0,
        next_wildcard: 0,
        next_for_in: 0,
        break_targets: vec![],
        next_catch: 0,
        caught: vec![],
        throw_used: false,
        assert_used: false,
        stack_used: false,
        const_instance_used: false,
        late_used: false,
        double_used: false,
        truncdiv_used: false,
        doublemod_used: false,
        shift_used: false,
        spread_used: false,
        next_build: 0,
        next_short: 0,
        null_short_targets: vec![],
        equals_used: false,
        declares_identical: module.functions.iter().any(|f| f.name == "identical"),
        classes: &module.classes,
        collections: module.resolution.types.iter().any(|t| {
            matches!(
                t,
                dartforge_syntax::TypeShape::List(_)
                    | dartforge_syntax::TypeShape::Set(_)
                    | dartforge_syntax::TypeShape::Iterable(_)
                    | dartforge_syntax::TypeShape::Record { .. }
                    | dartforge_syntax::TypeShape::Map { .. }
            )
        }),
    };

    if output.collections {
        output.runtime_types_used = true;
        output.push_str(include_str!("core.js"));
    }

    if module.extensions.iter().any(|extension| {
        extension
            .methods
            .iter()
            .any(|method| statements_need_null_assert(&method.body))
    }) || statements_need_null_assert(&module.statements)
        || module
            .functions
            .iter()
            .any(|function| statements_need_null_assert(&function.body))
        || module.classes.iter().any(|class| {
            class.fields.iter().any(|field| {
                field
                    .initializer
                    .as_ref()
                    .is_some_and(expression_needs_null_assert)
            }) || class
                .constructor
                .as_ref()
                .is_some_and(|ctor| statements_need_null_assert(&ctor.body))
                || class
                    .methods
                    .iter()
                    .chain(&class.factories)
                    .any(|method| statements_need_null_assert(&method.body))
        })
    {
        // Nome fora do prefixo $df_ reservado aos identificadores do usuário.
        output.push_str("function $dartforgeNullAssert(value) {\n  if (value === null) { throw new TypeError(\"Null check operator used on a null value\"); }\n  return value;\n}\n");
    }
    for extension in &module.extensions {
        for (index, method) in extension.methods.iter().enumerate() {
            write!(
                output,
                "function $dartforgeExtension{}Method{}(",
                extension.id, index
            )
            .expect("escrever em String não falha");
            parameter_header(&method.parameters, &mut output);
            output.push_str(") ");
            function_body(method, 0, &mut output);
            output.push('\n');
        }
    }
    emit_classes(&module.classes, &mut output);
    constructors::emit(&module.classes, &mut output);
    for class in &module.classes {
        for factory in &class.factories {
            write!(output, "function $dartforgeFactory{}", class.id).unwrap();
            identifier(factory.name, &mut output);
            output.push('(');
            parameter_header(&factory.parameters, &mut output);
            output.push_str(") ");
            function_body(factory, 0, &mut output);
            output.push('\n');
        }
    }
    for function in &module.functions {
        output.push_str("function ");
        identifier(function.name, &mut output);
        output.push('(');
        let wrote = parameter_header(&function.parameters, &mut output);
        if !function.type_parameters.is_empty() {
            output.runtime_types_used = true;
            if wrote {
                output.push(',');
            }
            output.push_str("$dartforgeTypes");
        }
        output.push_str(") ");
        function_body(function, 0, &mut output);
        output.push('\n');
    }
    emit_globals(&module.classes, &mut output);
    output.push_str("export function main() {\n");
    if module.main_is_async {
        asynchronous::begin(&mut output);
    }
    statements(&module.statements, 1, &mut output);
    if module.main_is_async {
        asynchronous::end(Type::Void, &mut output);
    }
    output.push_str("}\n");
    let flow = fluxo::runtime(&output);
    output.push_str(&flow);
    if output.const_instance_used {
        output.push_str("const $dartforgeConstInstances = new Map();
function $dartforgeConstInstance(key, proto, fields) { let found = $dartforgeConstInstances.get(key); if (found === undefined) { found = Object.freeze(Object.assign(Object.create(proto), fields)); $dartforgeConstInstances.set(key, found); } return found; }
");
    }
    if output.shift_used {
        output.push_str(colecoes::BITWISE_RUNTIME);
    }
    if output.spread_used {
        output.push_str(colecoes::SPREAD_RUNTIME);
    }
    if output.modulo_used {
        output.push_str("function $dartforgeModulo(a,b) { if (b === 0) throw new RangeError('Integer division by zero'); const d = Math.abs(b), r = a % d; return r < 0 ? r + d : r + 0; }\n");
    }
    if output.double_used || output.truncdiv_used || output.doublemod_used {
        output.push_str(DOUBLE_RUNTIME);
    }
    // Com o runtime de records presente, `$dartforgeEqual` já compara records
    // e Duration e já consulta o operador; o apelido evita duas cópias da
    // mesma regra. Sem ele, basta a versão enxuta.
    if output.equals_used {
        let shared = output.records || output.async_used;
        output.push_str(if shared {
            "const $dartforgeEquals = $dartforgeEqual;\n"
        } else {
            EQUALS_RUNTIME
        });
    }
    // Um programa que só interpola escalares recebe apenas a conversão, não o
    // runtime de coleções inteiro; com coleções, core.js já a trouxe.
    if output.strings_used && !output.collections {
        output.text.insert_str(0, strings::runtime());
    }
    if output.records || output.async_used {
        output.text.insert_str(0, include_str!("records.js"));
    }
    if output.async_used {
        output.runtime_types_used = true;
        output.text.insert_str(0, include_str!("async.js"));
    }
    if output.runtime_types_used {
        types::metadata(module, &mut output);
        output.text.insert_str(0, include_str!("types.js"));
    }
    // Célula `late` sem inicializador: um sentinela exclusivo distingue "ainda
    // não inicializado" de qualquer valor do usuário, `null` inclusive — por
    // isso é um Symbol e não `null` ou `undefined`, que o programa pode
    // atribuir. As mensagens são as do SDK Dart 3.6.2, conferidas com
    // `dart run`: locais dizem `Local`, campos e variáveis de topo dizem
    // `Field`. `$dartforgeLateSet` existe separado porque a escrita em
    // `receptor.campo` não pode reavaliar o receptor para conferir o sentinela.
    // `dynamic` reutiliza $dartforgeCast/$dartforgeIs; `typedef` e extension
    // types resolvem para o tipo subjacente no parse (erasure, sem emissão própria).
    if output.late_used {
        output.text.insert_str(
            0,
            "const $dartforgeLate = Symbol(\"late\");
function $dartforgeLateError(kind, name, part) { return new Error(\"LateInitializationError: \" + kind + \" '\" + name + \"' has \" + part + \" been initialized.\"); }
function $dartforgeLateRead(value, kind, name) { if (value === $dartforgeLate) { throw $dartforgeLateError(kind, name, \"not\"); } return value; }
function $dartforgeLateWrite(current, value, kind, name) { if (current !== $dartforgeLate) { throw $dartforgeLateError(kind, name, \"already\"); } return value; }
function $dartforgeLateSet(target, key, value, kind, name) { if (target[key] !== $dartforgeLate) { throw $dartforgeLateError(kind, name, \"already\"); } target[key] = value; return value; }
",
        );
    }
    output.push_str("const $df_main = main;\nmain();\n");
    output.text
}

/// Runtime numérico de doubles (semântica WEB/JS Number com toString Dart).
///
/// `$dartforgeDouble` reproduz o `toString` do Dart 3.6.2 para os casos da
/// bateria validada: inteiros ganham `.0`, exponenciais seguem o `String(x)`
/// do JavaScript (mesmo limiar `1e21` e mesmo `e+`/`e-` do oráculo) e `-0.0`,
/// `NaN` e `±Infinity` têm texto próprio. `$dartforgeTruncDiv` trunca em
/// direção a zero e lança com divisor nulo; `$dartforgeDoubleModulo` é o
/// módulo euclidiano sem lançamento (`7.5 % 0` vale NaN, como no oráculo).
/// Despacho de `==` quando alguma classe declara `operator ==`.
///
/// Reproduz literalmente as duas regras do Dart 3.6.2: `e1 == e2` com `e1`
/// nulo é verdadeiro somente contra `null` e **não** chama o operador; com
/// `e1` não nulo, quem decide é a implementação do lado esquerdo, mesmo que
/// `e2` seja `null`. Sem operador declarado no receptor, a comparação volta a
/// ser identidade, que é o `==` herdado de `Object`.
const EQUALS_RUNTIME: &str = "function $dartforgeEquals(left, right) { if (left === null) { return right === null; } const operator = typeof left === 'object' ? left.$df$eq : undefined; return operator === undefined ? left === right : operator.call(left, right); }\n";

const DOUBLE_RUNTIME: &str = "function $dartforgeDouble(value) { if (Number.isNaN(value)) return 'NaN'; if (value === Infinity) return 'Infinity'; if (value === -Infinity) return '-Infinity'; if (Object.is(value, -0)) return '-0.0'; const text = String(value); return (/^[+-]?\\d+$/.test(text) ? text + '.0' : text); }\nfunction $dartforgeTruncDiv(a,b) { if (b === 0) throw new RangeError('Integer division by zero'); return Math.trunc(a / b); }\nfunction $dartforgeDoubleModulo(a,b) { const d = Math.abs(b), r = a % d; return r < 0 ? r + d : r + 0; }\n";

/// Emite um literal double preservando o valor IEEE-754 no JavaScript.
///
/// Inteiros finitos abaixo de 1e21 saem com `.0` (`1.0`); o restante usa o
/// decimal de Rust, que o JavaScript lê para o mesmo binário; não finitos
/// viram os identificadores `Infinity`/`-Infinity`/`NaN` do JavaScript.
pub(crate) fn double_literal(value: f64, output: &mut Output<'_>) {
    if value.is_nan() {
        output.push_str("NaN");
    } else if value == f64::INFINITY {
        output.push_str("Infinity");
    } else if value == f64::NEG_INFINITY {
        output.push_str("-Infinity");
    } else if value.fract() == 0.0 && value.abs() < 1e21 {
        write!(output, "{value:.1}").expect("escrever em String não falha");
    } else {
        write!(output, "{value}").expect("escrever em String não falha");
    }
}

/// Tipo estático registrado para a expressão, quando a resolução o conhece.
/// Indica se o tipo estático resolvido é uma instância nominal de classe.
fn is_instance_type(ty: Option<Type>) -> bool {
    matches!(ty, Some(Type::Class(_) | Type::NullableClass(_)))
}

fn static_type(value: &Expr<'_>, output: &Output<'_>) -> Option<Type> {
    output
        .resolution
        .expr_types
        .get(&(value.span.start, value.span.end))
        .copied()
}

/// Ordena a herança em O(V+E) esperado, com IDs esparsos e sem recursão.
///
/// A fila e as listas de filhos seguem a ordem da fonte; não iteramos o HashMap.
/// Inicializadores continuam dentro dos construtores e não são reordenados.
fn class_order(classes: &[Class<'_>]) -> Vec<usize> {
    use std::collections::{HashMap, VecDeque};
    let mut indexes = HashMap::with_capacity(classes.len());
    for (index, class) in classes.iter().enumerate() {
        assert!(
            indexes.insert(class.id, index).is_none(),
            "AST inválida: ID de classe duplicado"
        );
    }
    let mut children = vec![Vec::new(); classes.len()];
    let mut ready = VecDeque::new();
    for (index, class) in classes.iter().enumerate() {
        if let Some(base) = class.superclass {
            let parent = *indexes
                .get(&base)
                .expect("AST inválida: classe base ausente");
            children[parent].push(index);
        } else {
            ready.push_back(index);
        }
    }
    let mut order = Vec::with_capacity(classes.len());
    while let Some(index) = ready.pop_front() {
        order.push(index);
        ready.extend(children[index].iter().copied());
    }
    assert_eq!(order.len(), classes.len(), "AST inválida: ciclo na herança");
    order
}

/// Emite classes em ordem de herança, inclusive quando a base aparece depois.
fn emit_classes(classes: &[Class<'_>], output: &mut Output<'_>) {
    for index in class_order(classes) {
        let class = &classes[index];
        // A pseudo-classe de variáveis de topo não vira classe JS: os campos
        // saem como `let`/`const` do módulo em `emit_globals`.
        if class.is_library_globals {
            continue;
        }
        if !class.enum_values.is_empty() && (!class.fields.is_empty() || !class.methods.is_empty())
        {
            features::enhanced_enum(class, output);
            continue;
        }
        if !class.enum_values.is_empty() {
            write!(
                output,
                "const $dartforgeClass{} = Object.freeze({{",
                class.id
            )
            .unwrap();
            for (index, name) in class.enum_values.iter().enumerate() {
                if index != 0 {
                    output.push(',');
                }
                identifier(name, output);
                write!(
                    output,
                    ":Object.freeze({{$dartforgeEnumTag:{},$df_name:",
                    class.id
                )
                .unwrap();
                string_literal(name, output);
                write!(output, ",$df_index:{index}}})").unwrap();
            }
            output.push_str("});\n");
            continue;
        }
        write!(output, "class $dartforgeClass{}", class.id).expect("escrever em String não falha");
        if let Some(base) = class.superclass {
            write!(output, " extends $dartforgeClass{base}").expect("escrever em String não falha");
        }
        output.push_str(" {\n");
        if !output.constructor_factories.contains(&class.id) {
            indent(1, output);
            output.push_str("constructor() {\n");
            // Dart avalia os campos da classe derivada antes dos campos da base.
            // Temporários não usam this antes de super, como exige o JavaScript.
            for (index, field) in class.fields.iter().enumerate() {
                indent(2, output);
                write!(output, "const $dartforgeField{index} = ")
                    .expect("escrever em String não falha");
                if let Some(initializer) = &field.initializer {
                    expression(initializer, output);
                } else if field.is_late {
                    output.late_used = true;
                    output.push_str("$dartforgeLate");
                } else {
                    output.push_str("null");
                }
                output.push_str(";\n");
            }
            if class.superclass.is_some() {
                indent(2, output);
                output.push_str("super();\n");
            }
            for (index, field) in class.fields.iter().enumerate() {
                indent(2, output);
                output.push_str("this.");
                identifier(field.name, output);
                writeln!(output, " = $dartforgeField{index};")
                    .expect("escrever em String não falha");
            }
            indent(1, output);
            output.push_str("}\n");
        }
        for method in &class.methods {
            indent(1, output);
            // O acessor com um parâmetro é o setter; os dois viram acessores
            // nativos do JavaScript, então `c.x` e `c.x = v` continuam sendo
            // leitura e escrita de propriedade, sem forma de chamada nova.
            if method.is_setter() {
                output.push_str("set ");
            } else if method.is_getter {
                output.push_str("get ");
            }
            identifier(method.name, output);
            output.push('(');
            parameter_header(&method.parameters, output);
            output.push_str(") ");
            function_body(method, 1, output);
            output.push('\n');
        }
        // Estáticos pertencem à declaração e nunca são herdados: saem como
        // membros `static` da própria classe; a resolução semântica já vetou
        // o acesso por subclasses com o nome da origem no diagnóstico.
        for field in &class.static_fields {
            indent(1, output);
            output.push_str("static ");
            identifier(field.name, output);
            output.push_str(" = ");
            if let Some(initializer) = &field.initializer {
                expression(initializer, output);
            } else if field.is_late {
                output.late_used = true;
                output.push_str("$dartforgeLate");
            } else {
                output.push_str("null");
            }
            output.push_str(";\n");
        }
        for method in &class.static_methods {
            indent(1, output);
            output.push_str("static ");
            if method.is_getter {
                output.push_str("get ");
            }
            identifier(method.name, output);
            output.push('(');
            parameter_header(&method.parameters, output);
            output.push_str(") ");
            function_body(method, 1, output);
            output.push('\n');
        }
        output.push_str("}\n");
    }
}

/// Emite variáveis de topo como `let`/`const` do módulo, na ordem escrita.
///
/// `final`/`const` viram `const`, mutáveis viram `let`. Sem inicializador
/// escrito, um anulável recebe `null` (a análise semântica já exigiu um dos
/// dois). Os nomes usam o mesmo prefixo das declarações locais, então leituras
/// e escritas existentes continuam válidas sem consultar a resolução.
fn emit_globals(classes: &[Class<'_>], output: &mut Output<'_>) {
    for class in classes {
        if !class.is_library_globals {
            continue;
        }
        for field in &class.static_fields {
            output.push_str(if (field.is_final || field.is_const) && !field.is_late {
                "const "
            } else {
                "let "
            });
            identifier(field.name, output);
            output.push_str(" = ");
            if let Some(initializer) = &field.initializer {
                expression(initializer, output);
            } else if field.is_late {
                output.late_used = true;
                output.push_str("$dartforgeLate");
            } else {
                output.push_str("null");
            }
            output.push_str(";\n");
        }
    }
}

/// Localiza a classe pelo ID para o despacho estático de `C.x` e `C.m(...)`.
fn class_by_id<'x, 'y>(classes: &'x [Class<'y>], id: u32) -> &'x Class<'y> {
    classes
        .iter()
        .find(|class| class.id == id)
        .expect("AST inválida: classe estática ausente")
}

/// Separa parâmetros dos locais e produz null no retorno nullable implícito.
fn function_body(function: &Function<'_>, depth: usize, output: &mut Output<'_>) {
    output.push_str("{\n");
    if function.is_async {
        asynchronous::begin(output);
    }
    // O bloco interno permite que um local Dart sombreie um parâmetro JavaScript.
    indent(depth + 1, output);
    block(&function.body, depth + 1, output);
    output.push('\n');
    if matches!(
        function.return_type,
        Type::Null
            | Type::NullableInt
            | Type::NullableDouble
            | Type::NullableNum
            | Type::NullableString
            | Type::NullableBool
            | Type::NullableObject
            | Type::NullableParameter(_)
            | Type::NullableClass(_)
    ) || matches!(function.return_type, Type::Applied(id) if matches!(output.resolution.types[id as usize], dartforge_syntax::TypeShape::Nullable(_)))
    {
        indent(depth + 1, output);
        output.push_str("return null;\n");
    }
    if function.is_async {
        asynchronous::end(
            asynchronous::result_type(function.return_type, output),
            output,
        );
    }
    indent(depth, output);
    output.push('}');
}

/// Detecta asserções em qualquer expressão, inclusive argumentos e operandos.
fn expression_needs_null_assert(value: &Expr<'_>) -> bool {
    match &value.kind {
        ExprKind::NamedArgument { value, .. } => expression_needs_null_assert(value),
        ExprKind::Cascade {
            receiver, sections, ..
        } => expression_needs_null_assert(receiver) || statements_need_null_assert(sections),
        ExprKind::Map { entries, .. } => entries.iter().any(|(key, value)| {
            expression_needs_null_assert(key)
                || value.as_ref().is_some_and(expression_needs_null_assert)
        }),
        ExprKind::Set { elements, .. } => elements.iter().any(expression_needs_null_assert),
        ExprKind::Spread { operand, .. } => expression_needs_null_assert(operand),
        ExprKind::MapEntry { key, value } => {
            expression_needs_null_assert(key) || expression_needs_null_assert(value)
        }
        ExprKind::CollectionIf {
            condition,
            then_element,
            else_element,
        } => {
            expression_needs_null_assert(condition)
                || expression_needs_null_assert(then_element)
                || else_element
                    .as_deref()
                    .is_some_and(expression_needs_null_assert)
        }
        ExprKind::CollectionFor { header, element } => {
            statement_needs_null_assert(header) || expression_needs_null_assert(element)
        }
        ExprKind::NullShort {
            receiver, chain, ..
        } => expression_needs_null_assert(receiver) || expression_needs_null_assert(chain),
        ExprKind::NullShortTarget => false,
        ExprKind::Record { fields } => fields
            .iter()
            .any(|(_, field)| expression_needs_null_assert(field)),
        ExprKind::Const(e) | ExprKind::Await(e) => expression_needs_null_assert(e),
        ExprKind::FutureValue { value, .. } => value
            .as_ref()
            .is_some_and(|e| expression_needs_null_assert(e)),
        ExprKind::FutureDelayed {
            duration,
            computation,
            ..
        } => {
            expression_needs_null_assert(duration)
                || computation
                    .as_ref()
                    .is_some_and(|e| expression_needs_null_assert(e))
        }
        ExprKind::Duration { parts } => parts.iter().any(|(_, e)| expression_needs_null_assert(e)),
        ExprKind::TypeTest { operand, .. } | ExprKind::Cast { operand, .. } => {
            expression_needs_null_assert(operand)
        }
        ExprKind::Switch { scrutinee, arms } => {
            expression_needs_null_assert(scrutinee)
                || arms.iter().any(|a| {
                    a.guard.as_ref().is_some_and(expression_needs_null_assert)
                        || expression_needs_null_assert(&a.value)
                })
        }
        ExprKind::Closure { body, .. } => statements_need_null_assert(body),
        ExprKind::NullAwareElement(inner) => expression_needs_null_assert(inner),
        ExprKind::DotShorthand { arguments, .. } => arguments
            .as_ref()
            .is_some_and(|a| a.iter().any(expression_needs_null_assert)),
        ExprKind::List { elements, .. } => elements.iter().any(expression_needs_null_assert),
        ExprKind::Index { receiver, index } => {
            expression_needs_null_assert(receiver) || expression_needs_null_assert(index)
        }
        ExprKind::Invoke { callee, arguments } => {
            expression_needs_null_assert(callee)
                || arguments.iter().any(expression_needs_null_assert)
        }
        ExprKind::Unary { op, operand } => {
            *op == UnaryOp::NullAssert || expression_needs_null_assert(operand)
        }
        ExprKind::Binary { left, right, .. } => {
            expression_needs_null_assert(left) || expression_needs_null_assert(right)
        }
        ExprKind::Call { arguments, .. }
        | ExprKind::GenericCall { arguments, .. }
        | ExprKind::Construct { arguments, .. }
        | ExprKind::NamedConstruct { arguments, .. } => {
            arguments.iter().any(expression_needs_null_assert)
        }
        ExprKind::Conditional {
            condition,
            then_value,
            else_value,
        } => {
            expression_needs_null_assert(condition)
                || expression_needs_null_assert(then_value)
                || expression_needs_null_assert(else_value)
        }
        ExprKind::Throw(value) => expression_needs_null_assert(value),
        ExprKind::Member { receiver, .. } => expression_needs_null_assert(receiver),
        ExprKind::MethodCall {
            receiver,
            arguments,
            ..
        } => {
            expression_needs_null_assert(receiver)
                || arguments.iter().any(expression_needs_null_assert)
        }
        _ => false,
    }
}

/// Percorre todos os corpos e cabeçalhos para incluir o auxiliar somente se usado.
fn statements_need_null_assert(body: &[Statement<'_>]) -> bool {
    body.iter().any(statement_needs_null_assert)
}

/// Verifica uma instrução sem alterar ordem de avaliação ou alocar cópias da AST.
fn statement_needs_null_assert(statement: &Statement<'_>) -> bool {
    match &statement.kind {
        StatementKind::Switch { scrutinee, cases } => {
            expression_needs_null_assert(scrutinee)
                || cases.iter().any(|c| {
                    c.guard.as_ref().is_some_and(expression_needs_null_assert)
                        || statements_need_null_assert(&c.body)
                })
        }
        StatementKind::IndexAssign {
            receiver,
            index,
            value,
        } => {
            expression_needs_null_assert(receiver)
                || expression_needs_null_assert(index)
                || expression_needs_null_assert(value)
        }
        StatementKind::RecordDestructure { initializer, .. }
        | StatementKind::Variable { initializer, .. } => expression_needs_null_assert(initializer),
        StatementKind::FieldAssign {
            receiver, value, ..
        } => expression_needs_null_assert(receiver) || expression_needs_null_assert(value),
        StatementKind::Assign { value, .. }
        | StatementKind::Print(value)
        | StatementKind::Expression(value) => expression_needs_null_assert(value),
        StatementKind::Return(value) => value.as_ref().is_some_and(expression_needs_null_assert),
        StatementKind::If {
            condition,
            then_body,
            else_body,
        } => {
            expression_needs_null_assert(condition)
                || statements_need_null_assert(then_body)
                || else_body
                    .as_ref()
                    .is_some_and(|body| statements_need_null_assert(body))
        }
        StatementKind::While { condition, body } | StatementKind::DoWhile { condition, body } => {
            expression_needs_null_assert(condition) || statements_need_null_assert(body)
        }
        StatementKind::For {
            initializer,
            condition,
            update,
            body,
        } => {
            initializer
                .as_deref()
                .is_some_and(statement_needs_null_assert)
                || condition.as_ref().is_some_and(expression_needs_null_assert)
                || update.as_deref().is_some_and(statement_needs_null_assert)
                || statements_need_null_assert(body)
        }
        StatementKind::Block(body) => statements_need_null_assert(body),
        StatementKind::ForIn { iterable, body, .. } => {
            expression_needs_null_assert(iterable) || statements_need_null_assert(body)
        }
        StatementKind::Labeled { body, .. } => statement_needs_null_assert(body),
        StatementKind::Assert { condition, message } => {
            expression_needs_null_assert(condition)
                || message.as_ref().is_some_and(expression_needs_null_assert)
        }
        StatementKind::Try {
            body,
            catches,
            finally_body,
        } => {
            statements_need_null_assert(body)
                || catches
                    .iter()
                    .any(|clause| statements_need_null_assert(&clause.body))
                || finally_body
                    .as_ref()
                    .is_some_and(|body| statements_need_null_assert(body))
        }
        StatementKind::Break
        | StatementKind::Continue
        | StatementKind::BreakLabel(_)
        | StatementKind::ContinueLabel(_)
        | StatementKind::Rethrow => false,
    }
}

/// Emite o nome de uma ligação declarada, isolando curingas `_` do Dart 3.7.
///
/// Cada `_` recebe identidade única no JavaScript porque a linguagem alvo proíbe
/// redeclarar o mesmo nome no escopo. A análise semântica garante que nenhuma
/// referência alcança esses nomes.
fn declaration(name: &str, output: &mut Output<'_>) {
    if name == "_" {
        let id = output.next_wildcard;
        output.next_wildcard += 1;
        write!(output, "$df_$wild{id}").expect("escrever em String não falha");
        return;
    }
    identifier(name, output);
}

/// Prefixo reservado das chaves do objeto de parâmetros nomeados.
///
/// Fica fora do espaço `$df_` dos identificadores do usuário, de modo que uma
/// chave nunca colide com um nome Dart nem com um auxiliar do runtime.
const NAMED_KEY: &str = "$dfn$";

/// Emite o cabeçalho de parâmetros de uma função, método ou fábrica.
///
/// Os posicionais saem na ordem declarada; um opcional sem padrão explícito
/// recebe `null`, porque Dart não possui `undefined`. Os nomeados viram um
/// único objeto desestruturado no fim da lista, com chave `$dfn$rótulo` e
/// ligação no nome interno. O objeto tem padrão `{}` para que uma chamada sem
/// nomeado alguma continue válida.
///
/// Devolve `true` se algo foi escrito, para que o chamador saiba se precisa de
/// separador antes de acrescentar parâmetros sintéticos.
fn parameter_header(
    parameters: &[dartforge_syntax::Parameter<'_>],
    output: &mut Output<'_>,
) -> bool {
    let mut wrote = false;
    for parameter in parameters.iter().filter(|p| !p.kind.is_named()) {
        if wrote {
            output.push_str(", ");
        }
        wrote = true;
        declaration(parameter.name, output);
        default_value(parameter.kind, parameter.default.as_deref(), output);
    }
    if !parameters.iter().any(|p| p.kind.is_named()) {
        return wrote;
    }
    if wrote {
        output.push_str(", ");
    }
    output.push('{');
    for (index, parameter) in parameters.iter().filter(|p| p.kind.is_named()).enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        output.push_str(NAMED_KEY);
        output.push_str(parameter.label());
        output.push_str(": ");
        declaration(parameter.name, output);
        default_value(parameter.kind, parameter.default.as_deref(), output);
    }
    output.push_str("} = {}");
    true
}

/// Escreve `= padrão` quando o parâmetro é opcional.
///
/// Sem padrão escrito, um opcional recebe `null`: a ausência do argumento em
/// Dart produz null, nunca `undefined`.
fn default_value(
    kind: dartforge_syntax::ParameterKind,
    default: Option<&Expr<'_>>,
    output: &mut Output<'_>,
) {
    if let Some(default) = default {
        output.push_str(" = ");
        literal_default(default, output);
    } else if !kind.is_required() {
        output.push_str(" = null");
    }
}

/// Escreve o literal escalar de um valor padrão sem consultar tabela por span.
///
/// A análise semântica já restringiu o padrão a um literal escalar. Emitir a
/// árvore diretamente mantém o padrão independente da renumeração de spans que
/// o linker aplica ao combinar bibliotecas. Zero inteiro sai sempre positivo,
/// como no restante da emissão.
fn literal_default(default: &Expr<'_>, output: &mut Output<'_>) {
    match &default.kind {
        ExprKind::Int(value) => write!(output, "{value}").expect("escrever em String não falha"),
        ExprKind::Double(value) => double_literal(*value, output),
        ExprKind::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        ExprKind::String(value) => string_literal(value, output),
        ExprKind::OwnedString(value) => string_literal(value, output),
        ExprKind::Null => output.push_str("null"),
        ExprKind::Unary {
            op: UnaryOp::Negate,
            operand,
        } => match operand.kind {
            ExprKind::Int(value) => {
                write!(output, "{}", -i64::from(value)).expect("escrever em String não falha");
            }
            // `-0.0 == 0.0`: negar zera o sinal, então o padrão é sempre `0.0`.
            ExprKind::Double(0.0) => output.push_str("0.0"),
            ExprKind::Double(value) => {
                output.push('-');
                double_literal(value, output);
            }
            _ => expression(default, output),
        },
        // Árvores montadas fora do pipeline validado caem no caminho geral.
        _ => expression(default, output),
    }
}

/// Emite argumentos posicionais e o objeto de nomeados na ordem escrita.
///
/// O objeto entra no fim da lista; suas propriedades seguem a ordem escrita,
/// que o JavaScript avalia de cima para baixo, preservando a ordem Dart.
fn argument_list(values: &[Expr<'_>], separator: &str, output: &mut Output<'_>) {
    let mut wrote = false;
    let mut object = false;
    for value in values {
        if let ExprKind::NamedArgument { label, value } = &value.kind {
            if object {
                output.push_str(separator);
            } else {
                if wrote {
                    output.push_str(separator);
                }
                output.push('{');
                object = true;
            }
            output.push_str(NAMED_KEY);
            output.push_str(label);
            output.push_str(": ");
            expression(value, output);
            wrote = true;
            continue;
        }
        if wrote {
            output.push_str(separator);
        }
        expression(value, output);
        wrote = true;
    }
    if object {
        output.push('}');
    }
}

/// Nome JavaScript do método gerado por `operator ==`.
///
/// Fica fora do espaço `$df_` reservado aos identificadores do usuário: o
/// separador é `$`, e nenhum nome Dart produz `$df$eq`.
const EQUALS_MEMBER: &str = "$df$eq";

/// Fecha uma checagem de `late` com o tipo de declaração e o nome Dart.
///
/// `kind` é o rótulo que o SDK usa na mensagem: `Local` para um local
/// declarado, `Field` para campo de instância e para variável de topo. O nome
/// vai como literal JSON para que um identificador com aspas ou barra invertida
/// não escape do texto — o lexer não aceita esses nomes hoje, mas o auxiliar
/// não depende disso.
fn late_tail(kind: &str, name: &str, output: &mut Output<'_>) {
    output.push_str(", \"");
    output.push_str(kind);
    output.push_str("\", ");
    string_literal(name, output);
    output.push(')');
}

/// Aplica o prefixo estável usado em declarações e referências.
fn identifier(name: &str, output: &mut Output<'_>) {
    // `operator ==` não tem nome Dart: o membro gerado usa o espaço `$df$`,
    // separado do `$df_` dos identificadores escritos pelo usuário.
    if name == dartforge_syntax::EQUALS_OPERATOR {
        output.push_str(EQUALS_MEMBER);
        return;
    }
    // O prefixo injetivo evita palavras reservadas e globais do JavaScript.
    // Blocos léxicos preservam sombreamento; referências recebem o mesmo prefixo.
    output.push_str("$df_");
    output.push_str(name);
}

/// Acrescenta dois espaços por nível léxico.
fn indent(depth: usize, output: &mut Output<'_>) {
    for _ in 0..depth {
        output.push_str("  ");
    }
}

/// Emite instruções na ordem original e preserva os respectivos escopos.
fn statements(body: &[Statement<'_>], depth: usize, output: &mut Output<'_>) {
    for statement in body {
        indent(depth, output);
        statement_at(statement, depth, output);
    }
}

/// Emite uma instrução já recuada; um rótulo apenas prefixa a instrução seguinte.
fn statement_at(statement: &Statement<'_>, depth: usize, output: &mut Output<'_>) {
    {
        match &statement.kind {
            StatementKind::RecordDestructure {
                is_final,
                positional,
                named,
                initializer,
            } => {
                let id = output.next_record;
                output.next_record += 1;
                write!(output, "const $dartforgeRecordTemp{id} = ").unwrap();
                expression(initializer, output);
                output.push_str(";\n");
                for (index, (name, _)) in positional.iter().enumerate() {
                    if *name == "_" {
                        continue;
                    }
                    indent(depth, output);
                    output.push_str(if *is_final { "const " } else { "let " });
                    identifier(name, output);
                    writeln!(output, " = $dartforgeRecordTemp{id}.$df_${};", index + 1).unwrap();
                }
                for (field, name, _) in named {
                    if *name == "_" {
                        continue;
                    }
                    indent(depth, output);
                    output.push_str(if *is_final { "const " } else { "let " });
                    identifier(name, output);
                    write!(output, " = $dartforgeRecordTemp{id}.").unwrap();
                    identifier(field, output);
                    output.push_str(";\n");
                }
            }
            StatementKind::Switch { scrutinee, cases } => {
                features::switch_statement(scrutinee, cases, depth, output)
            }
            StatementKind::Variable {
                name,
                is_final,
                is_late,
                initializer,
                ..
            } => {
                output.push_str(if *is_final && !*is_late {
                    "const "
                } else {
                    "let "
                });
                declaration(name, output);
                output.push_str(" = ");
                // `late` sem inicializador nasce no sentinela: o parser recusa
                // a forma com inicializador, então não há valor a avaliar aqui.
                if *is_late {
                    output.late_used = true;
                    output.push_str("$dartforgeLate");
                } else {
                    expression(initializer, output);
                }
                output.push_str(";\n");
            }
            StatementKind::Assign { name, value } => {
                let chave = (statement.span.start, statement.span.end);
                let implicito = output.resolution.implicit_members.contains(&chave);
                // `late final` admite uma atribuição só; a checagem lê o valor
                // corrente antes de avaliar o novo, e o novo é avaliado sempre,
                // como no oráculo: o efeito do lado direito precede o
                // lançamento. Reler `x` ou `this.x` não tem efeito colateral.
                let unica = output.resolution.late_final_writes.contains(&chave);
                if implicito {
                    output.push_str("this.");
                }
                identifier(name, output);
                output.push_str(" = ");
                if unica {
                    output.late_used = true;
                    output.push_str("$dartforgeLateWrite(");
                    if implicito {
                        output.push_str("this.");
                    }
                    identifier(name, output);
                    output.push_str(", ");
                    expression(value, output);
                    let global = output.resolution.global_accesses.contains(&chave);
                    late_tail(
                        if implicito || global {
                            "Field"
                        } else {
                            "Local"
                        },
                        name,
                        output,
                    );
                } else {
                    expression(value, output);
                }
                output.push_str(";\n");
            }
            StatementKind::FieldAssign {
                receiver,
                name,
                value,
            } => {
                if output
                    .resolution
                    .late_final_writes
                    .contains(&(statement.span.start, statement.span.end))
                {
                    // O receptor pode ter efeito colateral e é avaliado uma
                    // única vez: o auxiliar recebe o objeto e a chave, nunca
                    // uma segunda cópia da expressão do receptor.
                    output.late_used = true;
                    output.push_str("$dartforgeLateSet(");
                    expression(receiver, output);
                    output.push_str(", \"");
                    output.push_str("$df_");
                    output.push_str(name);
                    output.push_str("\", ");
                    expression(value, output);
                    late_tail("Field", name, output);
                    output.push_str(";\n");
                    return;
                }
                expression(receiver, output);
                output.push('.');
                identifier(name, output);
                output.push_str(" = ");
                expression(value, output);
                output.push_str(";\n");
            }
            StatementKind::IndexAssign {
                receiver,
                index,
                value,
            } => {
                output.push_str("$dartforgeIndexSet(");
                expression(receiver, output);
                output.push(',');
                expression(index, output);
                output.push(',');
                expression(value, output);
                output.push_str(");\n");
            }
            StatementKind::Print(value) => {
                // Doubles imprimem via `$dartforgeDouble` (toString Dart);
                // `num` segue o caminho escalar com o limite documentado para
                // doubles de valor inteiro (apagamento Number).
                if static_type(value, output) == Some(Type::Double) {
                    output.double_used = true;
                    output.push_str("console.log($dartforgeDouble(");
                    expression(value, output);
                    output.push_str("));\n");
                    return;
                }
                // Instância de classe imprime pelo `toString` declarado, que a
                // análise já exigiu; o despacho é do objeto, não do tipo
                // estático, então uma derivada com `toString` próprio aparece
                // mesmo por uma referência da base.
                if is_instance_type(static_type(value, output)) {
                    output.strings_used = true;
                    output.push_str("console.log($dartforgeString(");
                    expression(value, output);
                    output.push_str("));\n");
                    return;
                }
                let collections = output.collections;
                output.push_str(if collections {
                    "console.log($dartforgeFormat("
                } else {
                    "console.log("
                });
                expression(value, output);
                let collections = output.collections;
                output.push_str(if collections { "));\n" } else { ");\n" });
            }
            StatementKind::Return(value) => {
                output.push_str("return");
                if let Some(value) = value {
                    output.push(' ');
                    expression(value, output);
                }
                output.push_str(";\n");
            }
            StatementKind::Expression(value) => {
                expression(value, output);
                output.push_str(";\n");
            }
            StatementKind::If {
                condition,
                then_body,
                else_body,
            } => {
                output.push_str("if (");
                expression(condition, output);
                output.push_str(") {\n");
                statements(then_body, depth + 1, output);
                indent(depth, output);
                output.push('}');
                if let Some(else_body) = else_body {
                    output.push_str(" else {\n");
                    statements(else_body, depth + 1, output);
                    indent(depth, output);
                    output.push('}');
                }
                output.push('\n');
            }
            StatementKind::While { condition, body } => {
                output.break_targets.push(None);
                output.push_str("while (");
                expression(condition, output);
                output.push_str(") ");
                block(body, depth, output);
                output.break_targets.pop();
                output.push('\n');
            }
            StatementKind::DoWhile { body, condition } => {
                output.break_targets.push(None);
                output.push_str("do ");
                block(body, depth, output);
                output.push_str(" while (");
                expression(condition, output);
                output.push_str(");\n");
                output.break_targets.pop();
            }
            StatementKind::For {
                initializer,
                condition,
                update,
                body,
            } => {
                output.break_targets.push(None);
                output.push_str("for (");
                if let Some(initializer) = initializer {
                    for_clause(initializer, true, output);
                }
                output.push_str("; ");
                if let Some(condition) = condition {
                    expression(condition, output);
                }
                output.push_str("; ");
                if let Some(update) = update {
                    for_clause(update, false, output);
                }
                output.push_str(") ");
                block(body, depth, output);
                output.break_targets.pop();
                output.push('\n');
            }
            StatementKind::Break => {
                if let Some(Some(label)) = output.break_targets.last().cloned() {
                    writeln!(output, "break {label};").unwrap();
                } else {
                    output.push_str("break;\n");
                }
            }
            StatementKind::Continue => output.push_str("continue;\n"),
            StatementKind::BreakLabel(name) => {
                output.push_str("break ");
                fluxo::label(name, output);
                output.push_str(";\n");
            }
            StatementKind::ContinueLabel(name) => {
                output.push_str("continue ");
                fluxo::label(name, output);
                output.push_str(";\n");
            }
            StatementKind::Labeled { label, body } => {
                fluxo::label(label, output);
                output.push_str(": ");
                statement_at(body, depth, output);
            }
            StatementKind::Try {
                body,
                catches,
                finally_body,
            } => fluxo::try_statement(body, catches, finally_body.as_ref(), depth, output),
            StatementKind::Rethrow => fluxo::rethrow(output),
            StatementKind::Assert { condition, message } => {
                fluxo::assert_statement(condition, message.as_ref(), output);
            }
            StatementKind::ForIn {
                is_final,
                name,
                iterable,
                body,
                ..
            } => fluxo::for_in(*is_final, name, iterable, body, depth, output),
            StatementKind::Block(body) => {
                output.push_str("{\n");
                statements(body, depth + 1, output);
                indent(depth, output);
                output.push_str("}\n");
            }
        }
    }
}

/// Emite um bloco sem recuo inicial ou quebra de linha final.
fn block(body: &[Statement<'_>], depth: usize, output: &mut Output<'_>) {
    output.push_str("{\n");
    statements(body, depth + 1, output);
    indent(depth, output);
    output.push('}');
}

/// Emite uma cláusula de for validada, sem separadores ou quebras de linha.
fn for_clause(statement: &Statement<'_>, allow_variable: bool, output: &mut Output<'_>) {
    match &statement.kind {
        StatementKind::Variable {
            name,
            is_final,
            is_late,
            initializer,
            ..
        } if allow_variable => {
            output.push_str(if *is_final && !*is_late {
                "const "
            } else {
                "let "
            });
            declaration(name, output);
            output.push_str(" = ");
            if *is_late {
                output.late_used = true;
                output.push_str("$dartforgeLate");
            } else {
                expression(initializer, output);
            }
        }
        StatementKind::Assign { name, value } => {
            identifier(name, output);
            output.push_str(" = ");
            expression(value, output);
        }
        StatementKind::Expression(value) => expression(value, output),
        _ => panic!("AST inválida: instrução incompatível com cabeçalho de for"),
    }
}

/// Serializa texto já decodificado, preservando controles, Unicode e aspas.
///
/// O escape JSON impede que conteúdo da string seja interpretado como código
/// JavaScript. Strings emprestadas e alocadas seguem exatamente o mesmo caminho.
fn string_literal(value: &str, output: &mut Output<'_>) {
    output.push_str(&serde_json::to_string(value).expect("serializar uma string não falha"));
}

/// Emite uma expressão sem duplicar a avaliação de operandos.
fn expression(value: &Expr<'_>, output: &mut Output<'_>) {
    if let Some(constant) = output
        .resolution
        .constant_values
        .get(&(value.span.start, value.span.end))
        .cloned()
    {
        features::constant(&constant, output);
        return;
    }
    match &value.kind {
        // `argument_list` intercepta o rótulo; aqui só resta o valor interno,
        // o que mantém a emissão correta se a forma escapar de uma chamada.
        ExprKind::NamedArgument { value, .. } => expression(value, output),
        // `x++` e `++x` do JavaScript têm exatamente a semântica do Dart: o
        // alvo é lido e escrito uma única vez, a forma pós-fixa produz o valor
        // anterior e a prefixa o já atualizado. A análise semântica já restringiu
        // o alvo a um nome simples com tipo numérico, que é lvalue no JavaScript
        // emitido — `this.$df_x` inclusive. Os parênteses preservam a precedência
        // em qualquer posição de expressão.
        ExprKind::Increment {
            target,
            increase,
            prefix,
        } => {
            output.push('(');
            if *prefix {
                output.push_str(if *increase { "++" } else { "--" });
            }
            expression(target, output);
            if !*prefix {
                output.push_str(if *increase { "++" } else { "--" });
            }
            output.push(')');
        }
        ExprKind::Await(_)
        | ExprKind::FutureValue { .. }
        | ExprKind::FutureDelayed { .. }
        | ExprKind::Duration { .. } => asynchronous::expression(value, output),
        ExprKind::Cascade {
            receiver,
            null_aware,
            sections,
        } => {
            let id = output.next_cascade;
            output.next_cascade += 1;
            let temporary = format!("$dartforgeCascade{id}");
            writeln!(output, "(({temporary}) => {{").unwrap();
            if *null_aware {
                writeln!(output, "if ({temporary} === null) return {temporary};").unwrap();
            }
            output.cascade_receivers.push(temporary.clone());
            statements(sections, 1, output);
            output.cascade_receivers.pop();
            write!(output, "return {temporary};\n}})(").unwrap();
            expression(receiver, output);
            output.push(')');
        }
        ExprKind::CascadeReceiver => {
            let receiver = output
                .cascade_receivers
                .last()
                .expect("receiver sintético fora de cascade")
                .clone();
            output.push_str(&receiver);
        }
        ExprKind::NullAwareElement(_) => {
            panic!("AST inválida: elemento null-aware fora de literal de coleção")
        }
        // `{}` com contexto de conjunto resolve para `Set` na análise; o
        // literal continua sendo `Map` na AST e só a emissão muda de forma.
        ExprKind::Map { entries, .. }
            if entries.is_empty()
                && matches!(
                    output.resolution.expr_types.get(&(value.span.start, value.span.end)),
                    Some(Type::Applied(id)) if matches!(
                        output.resolution.types[*id as usize],
                        dartforge_syntax::TypeShape::Set(_)
                    )
                ) =>
        {
            output.push_str("new $dartforgeSet([],");
            types::descriptor(types::element_type(value, output), output);
            output.push(')');
        }
        ExprKind::Map { entries, .. } => {
            output.push_str("new $dartforgeMap(");
            if colecoes::entries_need_builder(entries) {
                // `if`/`for` produzem uma quantidade variável de entradas; o
                // caminho comum continua saindo como literal de array.
                colecoes::builder_entries(entries, 0, output);
            } else {
                colecoes::entry_values(entries, output);
            }
            output.push(',');
            let ty = output
                .resolution
                .expr_types
                .get(&(value.span.start, value.span.end))
                .expect("Map sem tipo resolvido");
            let Type::Applied(id) = ty else {
                panic!("Map sem tipo estrutural")
            };
            let dartforge_syntax::TypeShape::Map { key, value } =
                output.resolution.types[*id as usize]
            else {
                panic!("Map com tipo incompatível")
            };
            types::descriptor(key, output);
            output.push(',');
            types::descriptor(value, output);
            output.push(')');
        }
        ExprKind::NamedConstruct {
            class_id,
            name,
            arguments,
        } => {
            // `C.nome(...)` designa, nesta ordem, fábrica, construtor nomeado
            // ou método estático, como na resolução semântica.
            let class = class_by_id(output.classes, *class_id);
            if class.factories.iter().any(|factory| factory.name == *name) {
                write!(output, "$dartforgeFactory{class_id}").unwrap();
                identifier(name, output);
                output.push('(');
                argument_list(arguments, ",", output);
                output.push(')');
            } else if class
                .named_constructors
                .iter()
                .any(|declared| declared.name == *name)
            {
                write!(output, "$dartforgeNew{class_id}").unwrap();
                identifier(name, output);
                output.push('(');
                argument_list(arguments, ",", output);
                output.push(')');
            } else {
                write!(output, "$dartforgeClass{class_id}.").unwrap();
                identifier(name, output);
                output.push('(');
                argument_list(arguments, ", ", output);
                output.push(')');
            }
        }
        ExprKind::Record { fields } => {
            output.push_str("$dartforgeRecord([");
            for (index, (name, field)) in fields.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output.push('[');
                if let Some(name) = name {
                    string_literal(name, output);
                } else {
                    output.push_str("null");
                }
                output.push(',');
                expression(field, output);
                output.push(']');
            }
            output.push_str("])");
        }
        ExprKind::TypeTest {
            operand,
            ty,
            negated,
        } => {
            // `is double`/`is num` não passam pelo descritor reificado
            // (apagamento Number): checagem `typeof` embutida, com o limite
            // documentado para doubles de valor inteiro.
            if matches!(
                ty,
                Type::Double | Type::Num | Type::NullableDouble | Type::NullableNum
            ) {
                if *negated {
                    output.push('!');
                }
                if matches!(ty, Type::NullableDouble | Type::NullableNum) {
                    output.push_str("(($dartforgeValue)=>$dartforgeValue===null||typeof $dartforgeValue==='number')(");
                } else {
                    output.push_str("(($dartforgeValue)=>typeof $dartforgeValue==='number')(");
                }
                expression(operand, output);
                output.push(')');
                return;
            }
            if *negated {
                output.push('!');
            }
            output.push_str("$dartforgeIs(");
            expression(operand, output);
            output.push(',');
            types::descriptor(*ty, output);
            output.push(')');
        }
        ExprKind::Cast { operand, ty } => {
            // Casts para double/num são verificados estaticamente na semântica
            // (apagamento Number): sem checagem de runtime a emitir.
            if matches!(
                ty,
                Type::Double | Type::Num | Type::NullableDouble | Type::NullableNum
            ) {
                expression(operand, output);
                return;
            }
            output.push_str("$dartforgeCast(");
            expression(operand, output);
            output.push(',');
            types::descriptor(*ty, output);
            output.push(')');
        }
        ExprKind::Const(e) => expression(e, output),
        ExprKind::Switch { scrutinee, arms } => {
            features::switch_expression(scrutinee, arms, output)
        }
        ExprKind::Closure {
            parameters,
            body,
            is_async,
            ..
        } => {
            output.push_str("$dartforgeTyped(((");
            for (i, p) in parameters.iter().enumerate() {
                if i > 0 {
                    output.push(',');
                }
                declaration(p.name, output);
            }
            output.push_str(") => {\n");
            if *is_async {
                asynchronous::begin(output);
            }
            block(body, 1, output);
            if *is_async {
                let ty = output.resolution.expr_types[&(value.span.start, value.span.end)];
                let Type::Applied(id) = ty else {
                    panic!("closure sem assinatura");
                };
                let dartforge_syntax::TypeShape::Function { result, .. } =
                    output.resolution.types[id as usize]
                else {
                    panic!("closure sem assinatura");
                };
                asynchronous::end(asynchronous::result_type(result, output), output);
            }
            output.push_str("\nreturn null;\n}),");
            let ty = *output
                .resolution
                .expr_types
                .get(&(value.span.start, value.span.end))
                .expect("closure sem tipo resolvido");
            types::descriptor(ty, output);
            output.push(')');
        }
        ExprKind::List { elements, .. } => {
            output.push_str("new $dartforgeList(");
            if colecoes::needs_builder(elements) {
                colecoes::builder(elements, false, 0, output);
            } else {
                colecoes::sequence_values(elements, output);
            }
            output.push(',');
            types::descriptor(types::element_type(value, output), output);
            output.push(')');
        }
        ExprKind::Set { elements, .. } => {
            output.push_str("new $dartforgeSet(");
            if colecoes::needs_builder(elements) {
                colecoes::builder(elements, false, 0, output);
            } else {
                colecoes::sequence_values(elements, output);
            }
            output.push(',');
            types::descriptor(types::element_type(value, output), output);
            output.push(')');
        }
        ExprKind::NullShort {
            receiver, chain, ..
        } => colecoes::null_short(receiver, chain, output),
        ExprKind::NullShortTarget => {
            let target = output
                .null_short_targets
                .last()
                .expect("receiver sintético fora de cadeia null-aware")
                .clone();
            output.push_str(&target);
        }
        ExprKind::Spread { .. }
        | ExprKind::MapEntry { .. }
        | ExprKind::CollectionIf { .. }
        | ExprKind::CollectionFor { .. } => {
            panic!("AST inválida: elemento de coleção fora de literal de coleção")
        }
        ExprKind::Index { receiver, index } => {
            output.push_str("$dartforgeIndex(");
            expression(receiver, output);
            output.push(',');
            expression(index, output);
            output.push(')');
        }
        ExprKind::Invoke { callee, arguments } => {
            output.push('(');
            expression(callee, output);
            output.push_str(")(");
            argument_list(arguments, ",", output);
            output.push(')');
        }
        ExprKind::EnumValue { class_id, name } => {
            // `C.v` alcança um campo estático da própria classe ou um valor de
            // enum; ambos vivem como propriedades da construção emitida.
            write!(output, "$dartforgeClass{class_id}.").unwrap();
            identifier(name, output);
        }
        ExprKind::DotShorthand { name, arguments } => {
            // O alvo nominal vem da resolução: o parser não conhece a classe.
            let ty = output
                .resolution
                .expr_types
                .get(&(value.span.start, value.span.end))
                .expect("atalho de ponto sem tipo resolvido");
            let Type::Class(class_id) = ty else {
                panic!("AST inválida: atalho de ponto sem classe resolvida")
            };
            match arguments {
                None => {
                    write!(output, "$dartforgeClass{class_id}.").unwrap();
                    identifier(name, output);
                }
                Some(arguments) if *name == "new" => {
                    if output.constructor_factories.contains(class_id) {
                        write!(output, "$dartforgeNew{class_id}(").unwrap();
                    } else {
                        write!(output, "new $dartforgeClass{class_id}(").unwrap();
                    }
                    argument_list(arguments, ",", output);
                    output.push(')');
                }
                Some(arguments) => {
                    let class = class_by_id(output.classes, *class_id);
                    if class.factories.iter().any(|factory| factory.name == *name) {
                        write!(output, "$dartforgeFactory{class_id}").unwrap();
                        identifier(name, output);
                        output.push('(');
                        argument_list(arguments, ",", output);
                        output.push(')');
                    } else if class
                        .named_constructors
                        .iter()
                        .any(|declared| declared.name == *name)
                    {
                        write!(output, "$dartforgeNew{class_id}").unwrap();
                        identifier(name, output);
                        output.push('(');
                        argument_list(arguments, ",", output);
                        output.push(')');
                    } else {
                        write!(output, "$dartforgeClass{class_id}.").unwrap();
                        identifier(name, output);
                        output.push('(');
                        argument_list(arguments, ", ", output);
                        output.push(')');
                    }
                }
            }
        }
        ExprKind::Null => output.push_str("null"),
        ExprKind::This => output.push_str("this"),
        // `C<int>(...)`: a análise resolveu para construção de classe genérica e
        // registrou o id; os argumentos de tipo foram apagados (erasure).
        ExprKind::GenericCall { arguments, .. }
            if output
                .resolution
                .generic_constructions
                .contains_key(&(value.span.start, value.span.end)) =>
        {
            let class_id =
                output.resolution.generic_constructions[&(value.span.start, value.span.end)];
            if output.constructor_factories.contains(&class_id) {
                write!(output, "$dartforgeNew{class_id}(").unwrap();
            } else {
                write!(output, "new $dartforgeClass{class_id}(").unwrap();
            }
            argument_list(arguments, ",", output);
            output.push(')');
        }
        ExprKind::Construct {
            class_id,
            arguments,
        } => {
            if output.constructor_factories.contains(class_id) {
                write!(output, "$dartforgeNew{class_id}(").unwrap();
            } else {
                write!(output, "new $dartforgeClass{class_id}(").unwrap();
            }
            argument_list(arguments, ",", output);
            output.push(')');
        }
        ExprKind::Member { receiver, name } => {
            let tardia = output
                .resolution
                .late_reads
                .contains(&(value.span.start, value.span.end));
            if tardia {
                output.late_used = true;
                output.push_str("$dartforgeLateRead(");
            }
            expression(receiver, output);
            output.push('.');
            identifier(name, output);
            if tardia {
                late_tail("Field", name, output);
            }
        }
        ExprKind::MethodCall {
            receiver,
            name,
            arguments,
        } => {
            if let Some((extension, method)) = output
                .resolution
                .extension_calls
                .get(&(value.span.start, value.span.end))
                .map(|target| (target.extension_id, target.method_index))
            {
                write!(output, "$dartforgeExtension{extension}Method{method}.call(")
                    .expect("escrever em String não falha");
                expression(receiver, output);
                for argument in arguments {
                    output.push_str(", ");
                    expression(argument, output);
                }
                output.push(')');
                return;
            }
            expression(receiver, output);
            output.push('.');
            identifier(name, output);
            output.push('(');
            argument_list(arguments, ", ", output);
            if *name == "map" && types::collection_element(value, output).is_some() {
                if !arguments.is_empty() {
                    output.push(',');
                }
                types::descriptor(types::element_type(value, output), output);
            }
            output.push(')');
        }
        ExprKind::Int(value) => write!(output, "{value}").expect("escrever em String não falha"),
        ExprKind::Double(value) => double_literal(*value, output),
        ExprKind::String(value) => string_literal(value, output),
        ExprKind::OwnedString(value) => string_literal(value, output),
        ExprKind::Interpolation(parts) => strings::interpolation(parts, output),
        ExprKind::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        ExprKind::Identifier(name) => {
            // Só as leituras que a análise registrou pagam a checagem; um
            // identificador comum continua sendo leitura direta.
            let tardia = output
                .resolution
                .late_reads
                .contains(&(value.span.start, value.span.end));
            let implicito = output
                .resolution
                .implicit_members
                .contains(&(value.span.start, value.span.end));
            if tardia {
                output.late_used = true;
                output.push_str("$dartforgeLateRead(");
            }
            if implicito {
                output.push_str("this.");
            }
            identifier(name, output);
            if tardia {
                // Uma variável de topo diz `Field` na mensagem do SDK, como um
                // campo de instância; só o local declarado diz `Local`.
                let global = output
                    .resolution
                    .global_accesses
                    .contains(&(value.span.start, value.span.end));
                late_tail(
                    if implicito || global {
                        "Field"
                    } else {
                        "Local"
                    },
                    name,
                    output,
                );
            }
        }
        // `print(obj)` converte pelo `toString` declarado; a análise já
        // rejeitou instâncias sem ele e já recusou sombrear `print`.
        ExprKind::Call {
            name: "print",
            arguments,
        } if arguments.len() == 1 && is_instance_type(static_type(&arguments[0], output)) => {
            output.strings_used = true;
            output.push_str("console.log($dartforgeString(");
            expression(&arguments[0], output);
            output.push_str("))");
        }
        // `identical` é identidade de referência: nunca consulta o operador
        // declarado, e os operandos já foram restritos a instâncias e null.
        ExprKind::Call {
            name: "identical",
            arguments,
        } if arguments.len() == 2 && !output.declares_identical => {
            output.push('(');
            expression(&arguments[0], output);
            output.push_str(" === ");
            expression(&arguments[1], output);
            output.push(')');
        }
        ExprKind::Call { name, arguments }
        | ExprKind::GenericCall {
            name, arguments, ..
        } => {
            if output
                .resolution
                .implicit_members
                .contains(&(value.span.start, value.span.end))
            {
                output.push_str("this.");
                identifier(name, output);
            } else {
                match *name {
                    "Timer"
                        if output
                            .resolution
                            .async_builtins
                            .contains(&(value.span.start, value.span.end)) =>
                    {
                        output.async_used = true;
                        output.runtime_types_used = true;
                        output.push_str("new $dartforgeTimer");
                    }
                    "scheduleMicrotask"
                        if output
                            .resolution
                            .async_builtins
                            .contains(&(value.span.start, value.span.end)) =>
                    {
                        output.async_used = true;
                        output.runtime_types_used = true;
                        output.push_str("$dartforgeScheduleMicrotask");
                    }
                    "main" => output.push_str("$df_main"),
                    "print" => {
                        let collections = output.collections;
                        output.push_str(if collections {
                            "$dartforgePrint"
                        } else {
                            "console.log"
                        });
                    }
                    _ => identifier(name, output),
                }
            }
            output.push('(');
            argument_list(arguments, ", ", output);
            if let Some(types) = output
                .resolution
                .generic_arguments
                .get(&(value.span.start, value.span.end))
                .cloned()
            {
                if !arguments.is_empty() {
                    output.push(',');
                }
                output.push('[');
                for (index, ty) in types.iter().enumerate() {
                    if index > 0 {
                        output.push(',');
                    }
                    types::descriptor(*ty, output);
                }
                output.push(']');
            }
            output.push(')');
        }
        ExprKind::Unary {
            op: UnaryOp::NullAssert,
            operand,
        } => {
            output.push_str("$dartforgeNullAssert(");
            expression(operand, output);
            output.push(')');
        }
        ExprKind::Unary {
            op: UnaryOp::BitNot,
            operand,
        } => {
            // Semântica de inteiro do alvo web: `~` devolve o inteiro sem
            // sinal de 32 bits, como `dart compile js` (`~0` vale 4294967295).
            output.push_str("((~ ");
            expression(operand, output);
            output.push_str(") >>> 0)");
        }
        ExprKind::Unary { op, operand } => {
            output.push('(');
            output.push_str(if *op == UnaryOp::Negate { "-" } else { "!" });
            output.push(' ');
            expression(operand, output);
            if *op == UnaryOp::Negate && !matches!(static_type(operand, output), Some(Type::Double))
            {
                // O operando int normaliza o zero negativo do JavaScript.
                // Somar zero preserva precisão e transbordamento de Number.
                // Doubles preservam `-0.0`, como no oráculo Dart.
                output.push_str(" + 0");
            }
            output.push(')');
        }
        ExprKind::Binary {
            op: BinaryOp::Remainder,
            left,
            right,
        } => {
            // Com doubles o módulo é euclidiano sem lançamento (`7.5 % 0`
            // vale NaN no oráculo); int/int (ou tipo desconhecido em ASTs de
            // teste sem resolução) usa `$dartforgeModulo`.
            if matches!(
                static_type(value, output),
                None | Some(Type::Int) | Some(Type::NullableInt)
            ) {
                output.modulo_used = true;
                output.push_str("$dartforgeModulo(");
            } else {
                output.doublemod_used = true;
                output.push_str("$dartforgeDoubleModulo(");
            }
            expression(left, output);
            output.push(',');
            expression(right, output);
            output.push(')');
        }
        ExprKind::Binary {
            op: op @ (BinaryOp::ShiftLeft | BinaryOp::ShiftRight | BinaryOp::ShiftRightUnsigned),
            left,
            right,
        } => {
            // Contagem negativa lança e contagem acima de 31 zera (ou satura,
            // em `>>`), como no alvo web; ver `colecoes::BITWISE_RUNTIME`.
            output.shift_used = true;
            output.push_str(match op {
                BinaryOp::ShiftLeft => "$dartforgeShl(",
                BinaryOp::ShiftRight => "$dartforgeShr(",
                _ => "$dartforgeUshr(",
            });
            expression(left, output);
            output.push(',');
            expression(right, output);
            output.push(')');
        }
        ExprKind::Binary {
            op: op @ (BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor),
            left,
            right,
        } => {
            // O `>>> 0` reproduz o inteiro sem sinal de 32 bits do alvo web.
            output.push_str("((");
            expression(left, output);
            output.push_str(match op {
                BinaryOp::BitAnd => " & ",
                BinaryOp::BitOr => " | ",
                _ => " ^ ",
            });
            expression(right, output);
            output.push_str(") >>> 0)");
        }
        ExprKind::Binary {
            op: BinaryOp::TruncDivide,
            left,
            right,
        } => {
            // `~/` trunca em direção a zero e lança com divisor nulo.
            output.truncdiv_used = true;
            output.push_str("$dartforgeTruncDiv(");
            expression(left, output);
            output.push(',');
            expression(right, output);
            output.push(')');
        }
        // Comparação marcada pela análise: o operando esquerdo pode ser uma
        // instância cuja classe declara `operator ==`. O auxiliar preserva as
        // duas regras do Dart — receptor null nunca chama o operador, e é o
        // lado esquerdo que escolhe a implementação.
        ExprKind::Binary {
            op: op @ (BinaryOp::Equal | BinaryOp::NotEqual),
            left,
            right,
        } if output
            .resolution
            .equality_operators
            .contains(&(value.span.start, value.span.end)) =>
        {
            output.equals_used = true;
            if *op == BinaryOp::NotEqual {
                output.push('!');
            }
            output.push_str("$dartforgeEquals(");
            expression(left, output);
            output.push(',');
            expression(right, output);
            output.push(')');
        }
        ExprKind::Binary {
            op: op @ (BinaryOp::Equal | BinaryOp::NotEqual),
            left,
            right,
        } if output.records || output.async_used => {
            if *op == BinaryOp::NotEqual {
                output.push('!');
            }
            output.push_str("$dartforgeEqual(");
            expression(left, output);
            output.push(',');
            expression(right, output);
            output.push(')');
        }
        ExprKind::Conditional {
            condition,
            then_value,
            else_value,
        } => fluxo::conditional(condition, then_value, else_value, output),
        ExprKind::Throw(value) => fluxo::throw_expression(value, output),
        ExprKind::Binary { op, left, right } => {
            output.push('(');
            expression(left, output);
            output.push_str(match op {
                BinaryOp::Add => " + ",
                BinaryOp::Subtract => " - ",
                BinaryOp::Multiply => " * ",
                BinaryOp::Remainder => unreachable!("emissão dedicada"),
                // `/` do JavaScript já é divisão double, como no Dart.
                BinaryOp::Divide => " / ",
                BinaryOp::TruncDivide => unreachable!("emissão dedicada"),
                BinaryOp::Equal => " === ",
                BinaryOp::NotEqual => " !== ",
                BinaryOp::Less => " < ",
                BinaryOp::LessEqual => " <= ",
                BinaryOp::Greater => " > ",
                BinaryOp::GreaterEqual => " >= ",
                BinaryOp::And => " && ",
                BinaryOp::Or => " || ",
                BinaryOp::IfNull => " ?? ",
                BinaryOp::BitAnd
                | BinaryOp::BitOr
                | BinaryOp::BitXor
                | BinaryOp::ShiftLeft
                | BinaryOp::ShiftRight
                | BinaryOp::ShiftRightUnsigned => unreachable!("emissão dedicada"),
            });
            expression(right, output);
            if *op == BinaryOp::Multiply
                && !matches!(static_type(value, output), Some(Type::Double | Type::Num))
            {
                // Um inteiro negativo multiplicado por zero continua sendo zero inteiro.
                // Doubles preservam `-0.0`, como no oráculo Dart.
                output.push_str(" + 0");
            }
            output.push(')');
        }
    }
}

#[cfg(test)]
mod tests {
    /// Enums usam singletons congelados e nomes originais mesmo quando não são identificadores JS.
    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn enum_singletons_are_frozen_and_have_metadata() {
        let mut enumeration = empty_class(12, None);
        enumeration.enum_values = vec!["name", "other"];
        let program = Program {
            main_is_arrow: false,
            main_is_async: false,
            types: vec![],
            classes: vec![enumeration],
            extensions: vec![],
            functions: vec![],
            statements: vec![],
        };
        let module = dartforge_hir::lower(program);
        let mut js = emit(&module);
        js.push_str("console.log($dartforgeClass12.$df_name === $dartforgeClass12.$df_name); console.log($dartforgeClass12.$df_name === $dartforgeClass12.$df_other); console.log($dartforgeClass12.$df_name.$df_name); console.log($dartforgeClass12.$df_other.$df_index); console.log(Object.isFrozen($dartforgeClass12.$df_name));");
        let run = std::process::Command::new("node")
            .args(["--eval", &js])
            .output()
            .unwrap();
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&run.stdout).replace("\r\n", "\n"),
            "true\nfalse\nname\n1\ntrue\n"
        );
    }
    use super::*;
    use dartforge_diagnostics::Span;
    use dartforge_syntax::{Function, Parameter, Program, Type};

    fn expr(kind: ExprKind<'_>) -> Expr<'_> {
        Expr {
            kind,
            span: Span { start: 0, end: 0 },
        }
    }
    fn statement(kind: StatementKind<'_>) -> Statement<'_> {
        Statement {
            kind,
            span: Span { start: 0, end: 0 },
        }
    }
    fn print(value: Expr<'_>) -> Statement<'_> {
        statement(StatementKind::Print(value))
    }
    fn binary<'a>(op: BinaryOp, left: Expr<'a>, right: Expr<'a>) -> Expr<'a> {
        expr(ExprKind::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        })
    }
    fn variable<'a>(name: &'a str, is_final: bool, initializer: Expr<'a>) -> Statement<'a> {
        statement(StatementKind::Variable {
            is_const: false,
            name,
            annotation: None,
            is_final,
            is_late: false,
            initializer,
        })
    }
    fn compile(statements: Vec<Statement<'_>>) -> String {
        emit(&dartforge_hir::lower(Program {
            main_is_arrow: false,
            main_is_async: false,
            types: vec![],
            extensions: vec![],
            classes: vec![],
            functions: vec![],
            statements,
        }))
    }

    #[test]
    fn preserves_nested_expression_tree_and_unary_tokens() {
        let value = binary(
            BinaryOp::Multiply,
            binary(
                BinaryOp::Add,
                expr(ExprKind::Int(2)),
                expr(ExprKind::Int(3)),
            ),
            expr(ExprKind::Unary {
                op: UnaryOp::Negate,
                operand: Box::new(expr(ExprKind::Int(-4))),
            }),
        );
        assert!(compile(vec![print(value)]).contains("console.log(((2 + 3) * (- -4 + 0) + 0));"));
    }

    #[test]
    fn protects_globals_keywords_and_nested_shadowing() {
        let output = compile(vec![
            variable("console", false, expr(ExprKind::Int(1))),
            variable("main", true, expr(ExprKind::Int(2))),
            variable("function", false, expr(ExprKind::Bool(true))),
            variable("$df_console", true, expr(ExprKind::Int(3))),
            statement(StatementKind::Block(vec![
                variable("console", false, expr(ExprKind::Int(4))),
                statement(StatementKind::Assign {
                    name: "console",
                    value: expr(ExprKind::Int(5)),
                }),
                print(expr(ExprKind::Identifier("console"))),
            ])),
            print(expr(ExprKind::Identifier("console"))),
        ]);
        assert!(output.contains("let $df_console = 1;"));
        assert!(output.contains("const $df_main = 2;"));
        assert!(output.contains("let $df_function = true;"));
        assert!(output.contains("const $df_$df_console = 3;"));
        assert!(output.contains("  {\n    let $df_console = 4;\n    $df_console = 5;\n    console.log($df_console);\n  }\n  console.log($df_console);"));
        assert!(output.ends_with("}\nconst $df_main = main;\nmain();\n"));
    }

    #[test]
    fn strings_cannot_break_out_of_javascript_literal() {
        let value = "\"\\\n\r\t'); throw Error('bad'); // 🦀";
        let output = compile(vec![print(expr(ExprKind::String(value)))]);
        let serialized = output
            .split("console.log(")
            .nth(1)
            .unwrap()
            .split(");\n")
            .next()
            .unwrap();
        assert_eq!(serde_json::from_str::<String>(serialized).unwrap(), value);
    }

    #[test]
    fn maps_equality_and_short_circuit_operators() {
        for (op, text) in [
            (BinaryOp::Equal, " === "),
            (BinaryOp::NotEqual, " !== "),
            (BinaryOp::And, " && "),
            (BinaryOp::Or, " || "),
            (BinaryOp::LessEqual, " <= "),
            (BinaryOp::GreaterEqual, " >= "),
        ] {
            let output = compile(vec![print(binary(
                op,
                expr(ExprKind::Bool(true)),
                expr(ExprKind::Bool(false)),
            ))]);
            assert!(output.contains(&format!("(true{text}false)")));
        }
    }
    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn numeric_int_zero_is_canonical_and_large_values_match_js() {
        let output = compile(vec![
            variable("zero", false, expr(ExprKind::Int(0))),
            variable("large", false, expr(ExprKind::Int(i32::MAX))),
            print(expr(ExprKind::Unary {
                op: UnaryOp::Negate,
                operand: Box::new(expr(ExprKind::Identifier("zero"))),
            })),
            print(expr(ExprKind::Unary {
                op: UnaryOp::Negate,
                operand: Box::new(expr(ExprKind::Int(0))),
            })),
            print(binary(
                BinaryOp::Multiply,
                expr(ExprKind::Identifier("large")),
                expr(ExprKind::Identifier("large")),
            )),
            print(binary(
                BinaryOp::Multiply,
                binary(
                    BinaryOp::Multiply,
                    expr(ExprKind::Identifier("large")),
                    expr(ExprKind::Identifier("large")),
                ),
                expr(ExprKind::Identifier("large")),
            )),
            variable(
                "negativeZero",
                false,
                expr(ExprKind::Unary {
                    op: UnaryOp::Negate,
                    operand: Box::new(expr(ExprKind::Int(0))),
                }),
            ),
            print(binary(
                BinaryOp::Multiply,
                expr(ExprKind::Int(-1)),
                expr(ExprKind::Identifier("negativeZero")),
            )),
            print(expr(ExprKind::Bool(true))),
            print(expr(ExprKind::String("plain"))),
        ]);
        let run = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &output])
            .output()
            .expect("Node.js necessário");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        // Mantém zero inteiro positivo também nas operações seguintes.
        // dart2js 3.6.2 imprime 0 em programas que imprimem apenas int, mas pode
        // imprimir -0.0 se outros tipos também forem impressos. Preservamos zero
        // inteiro estável sem reproduzir esse efeito da otimização global.
        // Os valores grandes seguem dart2js, não a aritmética de 64 bits da VM.
        assert_eq!(
            String::from_utf8(run.stdout).unwrap().replace("\r\n", "\n"),
            "0\n0\n4611686014132420600\n9.903520300447984e+27\n0\ntrue\nplain\n"
        );
    }
    fn call(name: &'static str, arguments: Vec<Expr<'static>>) -> Expr<'static> {
        expr(ExprKind::Call { name, arguments })
    }

    fn function(
        name: &'static str,
        return_type: Type,
        parameters: &[(&'static str, Type)],
        body: Vec<Statement<'static>>,
    ) -> Function<'static> {
        Function {
            is_async: false,
            is_arrow: false,
            annotations: vec![],
            native_binding: None,
            is_getter: false,
            type_parameters: vec![],
            name,
            return_type,
            parameters: parameters
                .iter()
                .map(|&(name, ty)| Parameter::required(name, ty, Span { start: 0, end: 0 }))
                .collect(),
            body,
            span: Span { start: 0, end: 0 },
        }
    }

    fn functions_fixture() -> String {
        let functions = vec![
            function(
                "returnedPrint",
                Type::Void,
                &[],
                vec![statement(StatementKind::Return(Some(call(
                    "print",
                    vec![expr(ExprKind::String("returned print"))],
                ))))],
            ),
            function(
                "factorial",
                Type::Int,
                &[("n", Type::Int)],
                vec![statement(StatementKind::If {
                    condition: binary(
                        BinaryOp::LessEqual,
                        expr(ExprKind::Identifier("n")),
                        expr(ExprKind::Int(1)),
                    ),
                    then_body: vec![statement(StatementKind::Return(Some(expr(ExprKind::Int(
                        1,
                    )))))],
                    else_body: Some(vec![statement(StatementKind::Return(Some(binary(
                        BinaryOp::Multiply,
                        expr(ExprKind::Identifier("n")),
                        call(
                            "factorial",
                            vec![binary(
                                BinaryOp::Subtract,
                                expr(ExprKind::Identifier("n")),
                                expr(ExprKind::Int(1)),
                            )],
                        ),
                    ))))]),
                })],
            ),
            function(
                "mark",
                Type::Int,
                &[("console", Type::Int)],
                vec![
                    print(expr(ExprKind::Identifier("console"))),
                    statement(StatementKind::Return(Some(expr(ExprKind::Identifier(
                        "console",
                    ))))),
                ],
            ),
            function(
                "function",
                Type::Int,
                &[("main", Type::Int), ("second", Type::Int)],
                vec![statement(StatementKind::Return(Some(binary(
                    BinaryOp::Add,
                    binary(
                        BinaryOp::Multiply,
                        expr(ExprKind::Identifier("main")),
                        expr(ExprKind::Int(10)),
                    ),
                    expr(ExprKind::Identifier("second")),
                ))))],
            ),
            function(
                "stop",
                Type::Void,
                &[("flag", Type::Bool)],
                vec![
                    statement(StatementKind::If {
                        condition: expr(ExprKind::Identifier("flag")),
                        then_body: vec![
                            print(expr(ExprKind::String("stop"))),
                            statement(StatementKind::Return(None)),
                        ],
                        else_body: None,
                    }),
                    print(expr(ExprKind::String("go"))),
                ],
            ),
        ];
        let statements = vec![
            statement(StatementKind::Expression(call("returnedPrint", vec![]))),
            print(call("factorial", vec![expr(ExprKind::Int(5))])),
            print(call(
                "function",
                vec![
                    call("mark", vec![expr(ExprKind::Int(1))]),
                    call("mark", vec![expr(ExprKind::Int(2))]),
                ],
            )),
            statement(StatementKind::Expression(call(
                "stop",
                vec![expr(ExprKind::Bool(true))],
            ))),
            statement(StatementKind::Expression(call(
                "stop",
                vec![expr(ExprKind::Bool(false))],
            ))),
        ];
        emit(&dartforge_hir::lower(Program {
            main_is_arrow: false,
            main_is_async: false,
            types: vec![],
            extensions: vec![],
            classes: vec![],
            functions,
            statements,
        }))
    }

    #[test]
    fn emits_named_functions_parameters_returns_and_branches() {
        let output = functions_fixture();
        assert!(output.contains("function $df_function($df_main, $df_second) {"));
        assert!(output.contains("function $df_mark($df_console) {"));
        assert!(output.contains("if (($df_n <= 1)) {\n      return 1;\n    } else {"));
        assert!(output.contains("    return;\n"));
        assert!(output.contains("$df_stop(true);"));
        assert!(output.contains("return console.log(\"returned print\");"));
        assert!(!output.contains("$df_print("));
        let main_call = compile(vec![statement(StatementKind::Expression(call(
            "main",
            vec![],
        )))]);
        assert!(main_call.contains("  $df_main();\n"));
        assert!(main_call.contains("const $df_main = main;"));
    }

    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn functions_execute_recursion_ordered_arguments_and_early_return() {
        let output = functions_fixture();
        let run = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &output])
            .output()
            .expect("Node.js necessário");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(
            String::from_utf8(run.stdout).unwrap().replace("\r\n", "\n"),
            "returned print\n120\n1\n2\n12\nstop\ngo\n"
        );
    }
    fn assign(name: &'static str, value: Expr<'static>) -> Statement<'static> {
        statement(StatementKind::Assign { name, value })
    }
    fn name(value: &'static str) -> Expr<'static> {
        expr(ExprKind::Identifier(value))
    }
    fn int(value: i32) -> Expr<'static> {
        expr(ExprKind::Int(value))
    }
    fn when(condition: Expr<'static>, body: Vec<Statement<'static>>) -> Statement<'static> {
        statement(StatementKind::If {
            condition,
            then_body: body,
            else_body: None,
        })
    }
    fn loops_fixture() -> String {
        let functions = vec![
            function(
                "tick",
                Type::Int,
                &[("n", Type::Int)],
                vec![
                    print(expr(ExprKind::String("update"))),
                    print(name("n")),
                    statement(StatementKind::Return(Some(binary(
                        BinaryOp::Add,
                        name("n"),
                        int(1),
                    )))),
                ],
            ),
            function(
                "check",
                Type::Bool,
                &[("n", Type::Int)],
                vec![
                    print(expr(ExprKind::String("condition"))),
                    print(name("n")),
                    statement(StatementKind::Return(Some(binary(
                        BinaryOp::Less,
                        name("n"),
                        int(2),
                    )))),
                ],
            ),
        ];
        let statements = vec![
            variable("i", false, int(77)),
            statement(StatementKind::For {
                initializer: Some(Box::new(variable("i", false, int(0)))),
                condition: Some(binary(BinaryOp::Less, name("i"), int(3))),
                update: Some(Box::new(assign("i", call("tick", vec![name("i")])))),
                body: vec![
                    variable("j", false, int(0)),
                    statement(StatementKind::While {
                        condition: binary(BinaryOp::Less, name("j"), int(3)),
                        body: vec![
                            assign("j", binary(BinaryOp::Add, name("j"), int(1))),
                            when(
                                binary(BinaryOp::Equal, name("j"), int(1)),
                                vec![statement(StatementKind::Continue)],
                            ),
                            when(
                                binary(BinaryOp::Equal, name("j"), int(3)),
                                vec![statement(StatementKind::Break)],
                            ),
                            print(binary(
                                BinaryOp::Add,
                                binary(BinaryOp::Multiply, name("i"), int(10)),
                                name("j"),
                            )),
                        ],
                    }),
                    when(
                        binary(BinaryOp::Equal, name("i"), int(1)),
                        vec![statement(StatementKind::Continue)],
                    ),
                    print(name("i")),
                ],
            }),
            print(name("i")),
            variable("d", false, int(0)),
            statement(StatementKind::DoWhile {
                body: vec![
                    assign("d", binary(BinaryOp::Add, name("d"), int(1))),
                    statement(StatementKind::Continue),
                ],
                condition: call("check", vec![name("d")]),
            }),
            print(name("d")),
            statement(StatementKind::For {
                initializer: None,
                condition: None,
                update: None,
                body: vec![statement(StatementKind::Break)],
            }),
        ];
        emit(&dartforge_hir::lower(Program {
            main_is_arrow: false,
            main_is_async: false,
            types: vec![],
            extensions: vec![],
            classes: vec![],
            functions,
            statements,
        }))
    }

    #[test]
    fn loops_keep_native_control_flow_and_header_scope() {
        let output = loops_fixture();
        assert!(output.contains("for (let $df_i = 0; ($df_i < 3); $df_i = $df_tick($df_i)) {"));
        assert!(output.contains("while (($df_j < 3)) {"));
        assert!(output.contains("} while ($df_check($df_d));"));
        assert!(output.contains("for (; ; ) {"));
    }

    #[test]
    #[should_panic(expected = "AST inválida")]
    fn rejects_invalid_for_header_instead_of_emitting_broken_javascript() {
        compile(vec![statement(StatementKind::For {
            initializer: None,
            condition: None,
            update: Some(Box::new(variable("invalid", false, int(0)))),
            body: vec![],
        })]);
    }

    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn loops_execute_nested_break_continue_updates_and_conditions() {
        let output = loops_fixture();
        let run = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &output])
            .output()
            .expect("Node.js necessário");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(
            String::from_utf8(run.stdout).unwrap().replace("\r\n", "\n"),
            "2\n0\nupdate\n0\n12\nupdate\n1\n22\n2\nupdate\n2\n77\ncondition\n1\ncondition\n2\n2\n"
        );
    }
    #[test]
    fn borrowed_and_decoded_strings_emit_identical_literals() {
        let value = "aspas: \"' barra: \\ controles: \n\r\t\0 Unicode: 😀\u{2028}\u{2029}\"); process.exit(17); //";
        let borrowed = compile(vec![print(expr(ExprKind::String(value)))]);
        let owned = compile(vec![print(expr(ExprKind::OwnedString(value.to_owned())))]);
        assert_eq!(borrowed, owned);
        let literal = owned
            .split("console.log(")
            .nth(1)
            .unwrap()
            .split(");\n")
            .next()
            .unwrap();
        assert_eq!(serde_json::from_str::<String>(literal).unwrap(), value);
    }

    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn decoded_strings_execute_without_injection_or_unicode_loss() {
        let value = "aspas: \"' barra: \\ controles: \n\r\t\0 Unicode: 😀\u{2028}\u{2029}\"); process.exit(17); //";
        let output = compile(vec![
            print(expr(ExprKind::OwnedString(value.to_owned()))),
            print(expr(ExprKind::String(value))),
        ]);
        let run = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &output])
            .output()
            .expect("Node.js necessário");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        // Compara bytes: normalizar novas linhas esconderia corrupção de \r.
        assert_eq!(run.stdout, format!("{value}\n{value}\n").as_bytes());
    }
    /// Confere sombreamento de parâmetro sem declaração JavaScript duplicada.
    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn function_body_locals_can_shadow_parameters() {
        let module = dartforge_hir::lower(Program {
            main_is_arrow: false,
            main_is_async: false,
            types: vec![],
            extensions: vec![],
            classes: vec![],
            functions: vec![function(
                "f",
                Type::Int,
                &[("x", Type::Int)],
                vec![
                    variable("x", false, int(1)),
                    statement(StatementKind::Return(Some(name("x")))),
                ],
            )],
            statements: vec![print(call("f", vec![int(99)]))],
        });
        let output = emit(&module);
        let run = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &output])
            .output()
            .expect("Node.js necessário");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(run.stdout, b"1\n");
    }
    /// Monta uma asserção pós-fixa preservando o operando e seu efeito.
    fn asserted(value: Expr<'static>) -> Expr<'static> {
        expr(ExprKind::Unary {
            op: UnaryOp::NullAssert,
            operand: Box::new(value),
        })
    }

    /// Verifica inclusão sob demanda do auxiliar e agrupamento do operador ?? .
    #[test]
    fn null_helper_is_emitted_only_when_needed() {
        let plain = compile(vec![print(binary(
            BinaryOp::IfNull,
            expr(ExprKind::Null),
            int(4),
        ))]);
        assert!(plain.contains("console.log((null ?? 4));"));
        assert!(!plain.contains("$dartforgeNullAssert"));
        let module = dartforge_hir::lower(Program {
            main_is_arrow: false,
            main_is_async: false,
            types: vec![],
            extensions: vec![],
            classes: vec![],
            functions: vec![function(
                "checked",
                Type::Int,
                &[("n", Type::NullableInt)],
                vec![statement(StatementKind::Return(Some(asserted(name("n")))))],
            )],
            statements: vec![],
        });
        let checked = emit(&module);
        assert_eq!(checked.matches("function $dartforgeNullAssert(").count(), 1);
        assert!(checked.contains("return $dartforgeNullAssert($df_n);"));
    }

    /// Confere curto-circuito, valores falsy não nulos e avaliação única com efeitos.
    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn null_operators_preserve_lazy_evaluation_and_single_effects() {
        let module = dartforge_hir::lower(Program {
            main_is_arrow: false,
            main_is_async: false,
            types: vec![],
            extensions: vec![],
            classes: vec![],
            functions: vec![function(
                "mark",
                Type::NullableInt,
                &[("n", Type::NullableInt)],
                vec![
                    print(name("n")),
                    statement(StatementKind::Return(Some(name("n")))),
                ],
            )],
            statements: vec![
                print(binary(
                    BinaryOp::IfNull,
                    call("mark", vec![int(0)]),
                    call("mark", vec![int(9)]),
                )),
                print(binary(
                    BinaryOp::IfNull,
                    call("mark", vec![expr(ExprKind::Null)]),
                    call("mark", vec![int(7)]),
                )),
                print(asserted(call("mark", vec![int(8)]))),
                print(binary(
                    BinaryOp::IfNull,
                    expr(ExprKind::Bool(false)),
                    expr(ExprKind::Bool(true)),
                )),
                print(binary(
                    BinaryOp::IfNull,
                    expr(ExprKind::String("")),
                    expr(ExprKind::String("fallback")),
                )),
                print(expr(ExprKind::Null)),
            ],
        });
        let output = emit(&module);
        let run = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &output])
            .output()
            .expect("Node.js necessário");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(run.stdout, b"0\n0\nnull\n7\n7\n8\n8\nfalse\n\nnull\n");
    }

    /// A falha usa TypeError JavaScript do subconjunto, após um único efeito.
    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn null_assert_throws_after_evaluating_operand_once() {
        let module = dartforge_hir::lower(Program {
            main_is_arrow: false,
            main_is_async: false,
            types: vec![],
            extensions: vec![],
            classes: vec![],
            functions: vec![function(
                "missing",
                Type::NullableInt,
                &[],
                vec![
                    print(expr(ExprKind::String("evaluated"))),
                    statement(StatementKind::Return(Some(expr(ExprKind::Null)))),
                ],
            )],
            statements: vec![
                print(asserted(call("missing", vec![]))),
                print(expr(ExprKind::String("unreachable"))),
            ],
        });
        let output = emit(&module);
        let run = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &output])
            .output()
            .expect("Node.js necessário");
        assert!(!run.status.success());
        assert_eq!(run.stdout, b"evaluated\n");
        assert!(
            String::from_utf8_lossy(&run.stderr)
                .contains("TypeError: Null check operator used on a null value")
        );
    }
    /// Retornos nullable implícitos não podem expor undefined do JavaScript.
    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn nullable_function_fallthrough_returns_null() {
        let module = dartforge_hir::lower(Program {
            main_is_arrow: false,
            main_is_async: false,
            types: vec![],
            extensions: vec![],
            classes: vec![],
            functions: vec![
                function("number", Type::NullableInt, &[], vec![]),
                function("text", Type::NullableString, &[], vec![]),
                function("flag", Type::NullableBool, &[], vec![]),
                function("nothing", Type::Null, &[], vec![]),
            ],
            statements: vec![
                print(call("number", vec![])),
                print(call("text", vec![])),
                print(call("flag", vec![])),
                print(call("nothing", vec![])),
            ],
        });
        let output = emit(&module);
        let run = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &output])
            .output()
            .expect("Node.js necessário");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(run.stdout, b"null\nnull\nnull\nnull\n");
    }
    /// Cria um acesso explícito de membro, sem duplicar avaliação do receptor.
    fn member(receiver: Expr<'static>, field: &'static str) -> Expr<'static> {
        expr(ExprKind::Member {
            receiver: Box::new(receiver),
            name: field,
        })
    }
    /// Cria chamada de método com ligação dinâmica pelo receptor.
    fn method(receiver: Expr<'static>, method: &'static str) -> Expr<'static> {
        expr(ExprKind::MethodCall {
            receiver: Box::new(receiver),
            name: method,
            arguments: vec![],
        })
    }

    /// Verifica herança antecipada, despacho, inicializadores e atribuição com efeito.
    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn classes_preserve_inheritance_dispatch_and_receiver_effects() {
        use dartforge_syntax::Field;
        let span = Span { start: 0, end: 0 };
        let base = Class {
            constructor_extras: None,
            named_constructors: vec![],
            static_fields: vec![],
            static_methods: vec![],
            is_library_globals: false,
            type_parameters: vec![],
            factories: vec![],
            constructor: None,
            annotations: vec![],
            modifier: dartforge_syntax::ClassModifier::None,
            kind: dartforge_syntax::ClassKind::Class,
            mixins: vec![],
            is_mixin_application: false,
            mixin_origin: None,
            enum_arguments: vec![],
            enum_constructor_fields: vec![],
            is_interface: false,
            library_id: 0,
            is_abstract: false,
            interfaces: vec![],
            abstract_methods: vec![],
            enum_values: vec![],
            id: 0,
            name: "Base",
            superclass: None,
            span,
            fields: vec![Field {
                name: "value",
                ty: Type::Int,
                is_final: false,
                is_late: false,
                initializer: Some(asserted(call("initialize", vec![]))),
                span,
            }],
            methods: vec![function(
                "get",
                Type::Int,
                &[],
                vec![statement(StatementKind::Return(Some(member(
                    expr(ExprKind::This),
                    "value",
                ))))],
            )],
        };
        let child = Class {
            constructor_extras: None,
            named_constructors: vec![],
            static_fields: vec![],
            static_methods: vec![],
            is_library_globals: false,
            type_parameters: vec![],
            factories: vec![],
            constructor: None,
            annotations: vec![],
            modifier: dartforge_syntax::ClassModifier::None,
            kind: dartforge_syntax::ClassKind::Class,
            mixins: vec![],
            is_mixin_application: false,
            mixin_origin: None,
            enum_arguments: vec![],
            enum_constructor_fields: vec![],
            is_interface: false,
            library_id: 0,
            is_abstract: false,
            interfaces: vec![],
            abstract_methods: vec![],
            enum_values: vec![],
            id: 1,
            name: "Child",
            superclass: Some(0),
            fields: vec![],
            span,
            methods: vec![function(
                "get",
                Type::Int,
                &[],
                vec![statement(StatementKind::Return(Some(binary(
                    BinaryOp::Add,
                    member(expr(ExprKind::This), "value"),
                    int(10),
                ))))],
            )],
        };
        let module = dartforge_hir::lower(Program {
            main_is_arrow: false,
            main_is_async: false,
            types: vec![],
            extensions: vec![],
            classes: vec![child, base],
            functions: vec![
                function(
                    "initialize",
                    Type::NullableInt,
                    &[],
                    vec![
                        print(expr(ExprKind::String("initialize"))),
                        statement(StatementKind::Return(Some(int(2)))),
                    ],
                ),
                function(
                    "receiver",
                    Type::Class(0),
                    &[("item", Type::Class(0))],
                    vec![
                        print(expr(ExprKind::String("receiver"))),
                        statement(StatementKind::Return(Some(name("item")))),
                    ],
                ),
            ],
            statements: vec![
                variable(
                    "class_0",
                    false,
                    expr(ExprKind::Construct {
                        class_id: 1,
                        arguments: vec![],
                    }),
                ),
                print(method(name("class_0"), "get")),
                statement(StatementKind::FieldAssign {
                    receiver: call("receiver", vec![name("class_0")]),
                    name: "value",
                    value: int(5),
                }),
                print(method(name("class_0"), "get")),
            ],
        });
        let output = emit(&module);
        assert!(
            output.find("class $dartforgeClass0").unwrap()
                < output.find("class $dartforgeClass1").unwrap()
        );
        assert!(output.contains("function $dartforgeNullAssert("));
        let run = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &output])
            .output()
            .expect("Node.js necessário");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(run.stdout, b"initialize\n12\nreceiver\n15\n");
    }
    /// Gera classes vazias com IDs arbitrários para validar o contrato interno.
    fn empty_class(id: u32, superclass: Option<u32>) -> Class<'static> {
        Class {
            constructor_extras: None,
            named_constructors: vec![],
            static_fields: vec![],
            static_methods: vec![],
            is_library_globals: false,
            type_parameters: vec![],
            factories: vec![],
            constructor: None,
            annotations: vec![],
            modifier: dartforge_syntax::ClassModifier::None,
            kind: dartforge_syntax::ClassKind::Class,
            mixins: vec![],
            is_mixin_application: false,
            mixin_origin: None,
            enum_arguments: vec![],
            enum_constructor_fields: vec![],
            is_interface: false,
            library_id: 0,
            is_abstract: false,
            interfaces: vec![],
            abstract_methods: vec![],
            enum_values: vec![],
            id,
            name: "Synthetic",
            superclass,
            fields: vec![],
            methods: vec![],
            span: Span { start: 0, end: 0 },
        }
    }

    /// IDs esparsos e bases posteriores mantêm ordem determinística por fonte.
    #[test]
    fn class_order_handles_sparse_forward_ids_deterministically() {
        let classes = vec![
            empty_class(9000, Some(42)),
            empty_class(7, None),
            empty_class(42, None),
            empty_class(18, Some(9000)),
        ];
        assert_eq!(class_order(&classes), [1, 2, 0, 3]);
        assert_eq!(class_order(&classes), class_order(&classes));
        let chain: Vec<_> = (0..10000)
            .rev()
            .map(|id| empty_class(id, id.checked_sub(1)))
            .collect();
        assert_eq!(class_order(&chain), (0..10000).rev().collect::<Vec<_>>());
    }

    /// Uma base ausente deve falhar antes da emissão de JavaScript inválido.
    #[test]
    #[should_panic(expected = "classe base ausente")]
    fn class_order_rejects_unknown_base() {
        class_order(&[empty_class(7, Some(99))]);
    }

    /// Ciclos não podem bloquear a fila nem provocar recursão sem limite.
    #[test]
    #[should_panic(expected = "ciclo na herança")]
    fn class_order_rejects_cycle() {
        class_order(&[empty_class(7, Some(99)), empty_class(99, Some(7))]);
    }

    /// IDs repetidos violam a identidade nominal, mesmo em classes sem base.
    #[test]
    #[should_panic(expected = "ID de classe duplicado")]
    fn class_order_rejects_duplicate_ids() {
        class_order(&[empty_class(7, None), empty_class(7, None)]);
    }
    /// Despacho resolvido mantém this primitivo, efeitos ordenados e auxiliar !.
    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn extensions_use_static_targets_and_preserve_primitive_this() {
        use dartforge_syntax::{Extension, ExtensionTarget, Resolution};
        let span = Span { start: 10, end: 20 };
        let mut selected = expr(ExprKind::MethodCall {
            receiver: Box::new(call("mark", vec![int(7)])),
            name: "add",
            arguments: vec![call("mark", vec![int(2)])],
        });
        selected.span = span;
        let program = Program {
            main_is_arrow: false,
            main_is_async: false,
            types: vec![],
            classes: vec![],
            extensions: vec![Extension {
                id: 42,
                name: "Numbers",
                on_type: Type::Int,
                span,
                methods: vec![function(
                    "add",
                    Type::Int,
                    &[("n", Type::Int)],
                    vec![
                        print(binary(BinaryOp::Equal, expr(ExprKind::This), int(7))),
                        statement(StatementKind::Return(Some(asserted(binary(
                            BinaryOp::Add,
                            expr(ExprKind::This),
                            name("n"),
                        ))))),
                    ],
                )],
            }],
            functions: vec![function(
                "mark",
                Type::Int,
                &[("n", Type::Int)],
                vec![
                    print(name("n")),
                    statement(StatementKind::Return(Some(name("n")))),
                ],
            )],
            statements: vec![print(selected)],
        };
        let resolution = Resolution {
            async_builtins: Default::default(),
            generic_constructions: Default::default(),
            late_final_writes: Default::default(),
            late_reads: Default::default(),
            generic_arguments: Default::default(),
            constant_values: Default::default(),
            implicit_members: Default::default(),
            getter_accesses: Default::default(),
            global_accesses: Default::default(),
            equality_operators: Default::default(),
            types: vec![],
            expr_types: Default::default(),
            extension_calls: std::collections::BTreeMap::from([(
                (10, 20),
                ExtensionTarget {
                    extension_id: 42,
                    method_index: 0,
                },
            )]),
        };
        let output = emit(&dartforge_hir::lower_resolved(program, resolution));
        assert!(output.contains("$dartforgeExtension42Method0.call($df_mark(7), $df_mark(2))"));
        assert!(output.contains("function $dartforgeNullAssert("));
        let run = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &output])
            .output()
            .expect("Node.js necessário");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(run.stdout, b"7\n2\ntrue\n9\n");
    }
}
