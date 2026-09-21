//! Alcance conservador de declarações, sem renumerar identidades ou alterar corpos.
use dartforge_syntax::*;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Declarações eliminadas; métodos de classes retidas não são podados separadamente.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TreeShakeStats {
    pub removed_functions: usize,
    pub removed_classes: usize,
}

/// Remove funções e classes inalcançáveis após análise semântica e expansão de macros.
///
/// Main e todas as extensions são raízes. Toda classe alcançada conserva campos,
/// métodos, fábricas e contratos. Tipos reificados também criam dependências.
/// Homônimos locais podem conservar funções adicionais: o passe favorece segurança.
/// Não elimina instruções, efeitos ou formas da tabela de tipos; IDs ficam estáveis.
pub fn tree_shake(program: &mut Program<'_>, resolution: &Resolution) -> TreeShakeStats {
    let mut scan = Scan {
        program,
        resolution,
        names: program
            .functions
            .iter()
            .enumerate()
            .map(|(i, f)| (f.name, i))
            .collect(),
        ids: program
            .classes
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id, i))
            .collect(),
        functions: BTreeSet::new(),
        classes: BTreeSet::new(),
        types: BTreeSet::new(),
        queue: VecDeque::new(),
    };
    scan.body(&program.statements);
    for extension in &program.extensions {
        scan.ty(extension.on_type);
        for function in &extension.methods {
            scan.function(function);
        }
    }
    while let Some(item) = scan.queue.pop_front() {
        match item {
            Item::Function(i) => scan.function(&program.functions[i]),
            Item::Class(i) => {
                let c = &program.classes[i];
                for id in c
                    .superclass
                    .iter()
                    .chain(&c.interfaces)
                    .chain(&c.mixins)
                    .chain(c.mixin_origin.iter())
                {
                    scan.class(*id);
                }
                for field in &c.fields {
                    scan.ty(field.ty);
                    if let Some(e) = &field.initializer {
                        scan.expr(e);
                    }
                }
                if let Some(ctor) = &c.constructor {
                    for p in &ctor.parameters {
                        scan.ty(p.ty);
                        if let Some(default) = &p.default {
                            scan.expr(default);
                        }
                    }
                    scan.body(&ctor.body);
                }
                for args in &c.enum_arguments {
                    for e in args {
                        scan.expr(e);
                    }
                }
                for f in c
                    .methods
                    .iter()
                    .chain(&c.abstract_methods)
                    .chain(&c.factories)
                {
                    scan.function(f);
                }
            }
        }
    }
    let functions = scan.functions;
    let classes = scan.classes;
    let before_functions = program.functions.len();
    let before_classes = program.classes.len();
    let mut index = 0;
    program.functions.retain(|_| {
        let keep = functions.contains(&index);
        index += 1;
        keep
    });
    program.classes.retain(|c| classes.contains(&c.id));
    TreeShakeStats {
        removed_functions: before_functions - program.functions.len(),
        removed_classes: before_classes - program.classes.len(),
    }
}

/// Trabalhos de declaração enfileirados uma única vez.
enum Item {
    Function(usize),
    Class(usize),
}
/// Estado de alcance; ciclos nominais, de tipos e de chamadas são visitados uma vez.
struct Scan<'p, 's> {
    program: &'p Program<'s>,
    resolution: &'p Resolution,
    names: BTreeMap<&'s str, usize>,
    ids: BTreeMap<u32, usize>,
    functions: BTreeSet<usize>,
    classes: BTreeSet<u32>,
    types: BTreeSet<u32>,
    queue: VecDeque<Item>,
}
impl Scan<'_, '_> {
    /// Enfileira a função top-level quando o nome existe.
    fn name(&mut self, name: &str) {
        if let Some(&i) = self.names.get(name)
            && self.functions.insert(i)
        {
            self.queue.push_back(Item::Function(i));
        }
    }
    /// Conserva identidade nominal sem exigir IDs densos.
    fn class(&mut self, id: u32) {
        if self.classes.insert(id)
            && let Some(&i) = self.ids.get(&id)
        {
            self.queue.push_back(Item::Class(i));
        }
    }
    /// Segue descritores estruturais reificados e suas dependências nominais.
    fn ty(&mut self, ty: Type) {
        match ty {
            Type::Class(id) | Type::NullableClass(id) => self.class(id),
            Type::Applied(id) if self.types.insert(id) => {
                if let Some(shape) = self
                    .resolution
                    .types
                    .get(id as usize)
                    .or_else(|| self.program.types.get(id as usize))
                {
                    match shape {
                        TypeShape::Map { key, value } => {
                            self.ty(*key);
                            self.ty(*value);
                        }
                        TypeShape::Record { positional, named } => {
                            for t in positional {
                                self.ty(*t);
                            }
                            for (_, t) in named {
                                self.ty(*t);
                            }
                        }
                        TypeShape::Future(t)
                        | TypeShape::Nullable(t)
                        | TypeShape::List(t)
                        | TypeShape::Set(t)
                        | TypeShape::Iterable(t) => self.ty(*t),
                        TypeShape::Function { result, parameters } => {
                            self.ty(*result);
                            for t in parameters {
                                self.ty(*t);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    /// Assinaturas e limites são observáveis por testes de tipo e tear-offs.
    fn function(&mut self, f: &Function<'_>) {
        self.ty(f.return_type);
        for p in &f.parameters {
            self.ty(p.ty);
            if let Some(default) = &p.default {
                self.expr(default);
            }
        }
        for p in &f.type_parameters {
            self.ty(p.bound);
        }
        self.body(&f.body);
    }
    /// Visita corpos sem inferir execução condicional.
    fn body(&mut self, body: &[Statement<'_>]) {
        for s in body {
            self.statement(s);
        }
    }
    /// Conserva tanto testes de tipo quanto expressões constantes dos padrões.
    fn pattern(&mut self, p: &Pattern<'_>) {
        match p {
            Pattern::Constant(e) => self.expr(e),
            Pattern::Type(t) | Pattern::Binding { ty: t, .. } => self.ty(*t),
            Pattern::Wildcard => {}
        }
    }
    /// Percorre todos os filhos de uma instrução, inclusive atualizações dos laços.
    fn statement(&mut self, s: &Statement<'_>) {
        match &s.kind {
            StatementKind::Variable {
                annotation,
                initializer,
                ..
            } => {
                if let Some(t) = annotation {
                    self.ty(*t);
                }
                self.expr(initializer);
            }
            StatementKind::Assign { value, .. }
            | StatementKind::Print(value)
            | StatementKind::Expression(value)
            | StatementKind::RecordDestructure {
                initializer: value, ..
            } => self.expr(value),
            StatementKind::Return(value) => {
                if let Some(e) = value {
                    self.expr(e);
                }
            }
            StatementKind::FieldAssign {
                receiver, value, ..
            } => {
                self.expr(receiver);
                self.expr(value);
            }
            StatementKind::IndexAssign {
                receiver,
                index,
                value,
            } => {
                self.expr(receiver);
                self.expr(index);
                self.expr(value);
            }
            StatementKind::If {
                condition,
                then_body,
                else_body,
            } => {
                self.expr(condition);
                self.body(then_body);
                if let Some(b) = else_body {
                    self.body(b);
                }
            }
            StatementKind::While { condition, body }
            | StatementKind::DoWhile { condition, body } => {
                self.expr(condition);
                self.body(body);
            }
            StatementKind::For {
                initializer,
                condition,
                update,
                body,
            } => {
                if let Some(s) = initializer {
                    self.statement(s);
                }
                if let Some(e) = condition {
                    self.expr(e);
                }
                if let Some(s) = update {
                    self.statement(s);
                }
                self.body(body);
            }
            StatementKind::Block(b) => self.body(b),
            StatementKind::Switch { scrutinee, cases } => {
                self.expr(scrutinee);
                for c in cases {
                    self.pattern(&c.pattern);
                    if let Some(g) = &c.guard {
                        self.expr(g);
                    }
                    self.body(&c.body);
                }
            }
            StatementKind::Labeled { body, .. } => self.statement(body),
            StatementKind::ForIn {
                annotation,
                iterable,
                body,
                ..
            } => {
                if let Some(t) = annotation {
                    self.ty(*t);
                }
                self.expr(iterable);
                self.body(body);
            }
            StatementKind::Assert { condition, message } => {
                self.expr(condition);
                if let Some(message) = message {
                    self.expr(message);
                }
            }
            StatementKind::Try {
                body,
                catches,
                finally_body,
            } => {
                self.body(body);
                for clause in catches {
                    if let Some(t) = clause.exception_type {
                        self.ty(t);
                    }
                    self.body(&clause.body);
                }
                if let Some(body) = finally_body {
                    self.body(body);
                }
            }
            StatementKind::Break
            | StatementKind::Continue
            | StatementKind::BreakLabel(_)
            | StatementKind::ContinueLabel(_)
            | StatementKind::Rethrow => {}
        }
    }
    /// Preserva dependências de constantes que podem substituir a expressão original.
    fn constant(&mut self, c: &ConstValue) {
        match c {
            ConstValue::Enum { class_id, .. } => self.class(*class_id),
            ConstValue::List {
                element_type,
                values,
            } => {
                self.ty(*element_type);
                for v in values {
                    self.constant(v);
                }
            }
            ConstValue::Instance { class_id, fields } => {
                self.class(*class_id);
                for (_, v) in fields {
                    self.constant(v);
                }
            }
            _ => {}
        }
    }
    /// Visita expressão e tabelas semânticas somente quando ela é alcançada.
    fn expr(&mut self, e: &Expr<'_>) {
        let key = (e.span.start, e.span.end);
        if let Some(t) = self.resolution.expr_types.get(&key) {
            self.ty(*t);
        }
        if let Some(args) = self.resolution.generic_arguments.get(&key) {
            for t in args {
                self.ty(*t);
            }
        }
        if let Some(c) = self.resolution.constant_values.get(&key) {
            self.constant(c);
        }
        match &e.kind {
            // O rótulo não referencia declaração alguma; só o valor é observável.
            ExprKind::NamedArgument { value, .. } => self.expr(value),
            ExprKind::NullAwareElement(value) => self.expr(value),
            ExprKind::DotShorthand { arguments, .. } => {
                // A classe alvo só aparece na resolução; sem isso seria podada.
                if let Some(ty) = self.resolution.expr_types.get(&key) {
                    self.ty(*ty);
                }
                for argument in arguments.iter().flatten() {
                    self.expr(argument);
                }
            }
            ExprKind::Identifier(name) => self.name(name),
            ExprKind::Call { name, arguments }
            | ExprKind::GenericCall {
                name, arguments, ..
            } => {
                self.name(name);
                if let ExprKind::GenericCall { type_arguments, .. } = &e.kind {
                    for t in type_arguments {
                        self.ty(*t);
                    }
                }
                for a in arguments {
                    self.expr(a);
                }
            }
            ExprKind::Construct {
                class_id,
                arguments,
            }
            | ExprKind::NamedConstruct {
                class_id,
                arguments,
                ..
            } => {
                self.class(*class_id);
                for a in arguments {
                    self.expr(a);
                }
            }
            ExprKind::EnumValue { class_id, .. } => self.class(*class_id),
            ExprKind::TypeTest { operand, ty, .. } | ExprKind::Cast { operand, ty } => {
                self.ty(*ty);
                self.expr(operand);
            }
            ExprKind::Unary { operand, .. }
            | ExprKind::Throw(operand)
            | ExprKind::Const(operand) => self.expr(operand),
            ExprKind::Conditional {
                condition,
                then_value,
                else_value,
            } => {
                self.expr(condition);
                self.expr(then_value);
                self.expr(else_value);
            }
            ExprKind::Binary { left, right, .. } => {
                self.expr(left);
                self.expr(right);
            }
            ExprKind::Member { receiver, .. } => self.expr(receiver),
            ExprKind::MethodCall {
                receiver,
                arguments,
                ..
            }
            | ExprKind::Invoke {
                callee: receiver,
                arguments,
            } => {
                self.expr(receiver);
                for a in arguments {
                    self.expr(a);
                }
            }
            ExprKind::Index { receiver, index } => {
                self.expr(receiver);
                self.expr(index);
            }
            ExprKind::Map {
                key_type,
                value_type,
                entries,
            } => {
                for t in key_type.iter().chain(value_type) {
                    self.ty(*t);
                }
                for (k, v) in entries {
                    self.expr(k);
                    // `None` marca elemento de controle: só a chave é real.
                    if let Some(v) = v {
                        self.expr(v);
                    }
                }
            }
            ExprKind::Set {
                element_type,
                elements,
            } => {
                if let Some(t) = element_type {
                    self.ty(*t);
                }
                for element in elements {
                    self.expr(element);
                }
            }
            ExprKind::Spread { operand, .. } => self.expr(operand),
            ExprKind::MapEntry { key, value } => {
                self.expr(key);
                self.expr(value);
            }
            ExprKind::CollectionIf {
                condition,
                then_element,
                else_element,
            } => {
                self.expr(condition);
                self.expr(then_element);
                if let Some(element) = else_element {
                    self.expr(element);
                }
            }
            ExprKind::CollectionFor { header, element } => {
                self.statement(header);
                self.expr(element);
            }
            ExprKind::NullShort {
                receiver, chain, ..
            } => {
                self.expr(receiver);
                self.expr(chain);
            }
            ExprKind::List {
                element_type,
                elements,
            } => {
                if let Some(t) = element_type {
                    self.ty(*t);
                }
                for a in elements {
                    self.expr(a);
                }
            }
            ExprKind::Record { fields } => {
                for (_, e) in fields {
                    self.expr(e);
                }
            }
            ExprKind::Cascade {
                receiver, sections, ..
            } => {
                self.expr(receiver);
                self.body(sections);
            }
            ExprKind::Closure {
                parameters,
                return_type,
                body,
                ..
            } => {
                for p in parameters {
                    self.ty(p.ty);
                }
                self.ty(*return_type);
                self.body(body);
            }
            ExprKind::Switch { scrutinee, arms } => {
                self.expr(scrutinee);
                for a in arms {
                    self.pattern(&a.pattern);
                    if let Some(g) = &a.guard {
                        self.expr(g);
                    }
                    self.expr(&a.value);
                }
            }
            ExprKind::Await(e) => self.expr(e),
            ExprKind::FutureValue { value, value_type } => {
                if let Some(t) = value_type {
                    self.ty(*t);
                }
                if let Some(e) = value {
                    self.expr(e);
                }
            }
            ExprKind::FutureDelayed {
                duration,
                computation,
                value_type,
            } => {
                if let Some(t) = value_type {
                    self.ty(*t);
                }
                self.expr(duration);
                if let Some(e) = computation {
                    self.expr(e);
                }
            }
            ExprKind::Duration { parts } => {
                for (_, e) in parts {
                    self.expr(e);
                }
            }
            // Só as expressões interpoladas alcançam declarações; texto, não.
            ExprKind::Interpolation(parts) => {
                for part in parts {
                    if let dartforge_syntax::StringPart::Expression(value) = part {
                        self.expr(value);
                    }
                }
            }
            ExprKind::CascadeReceiver
            | ExprKind::NullShortTarget
            | ExprKind::This
            | ExprKind::Null
            | ExprKind::Int(_)
            | ExprKind::Double(_)
            | ExprKind::Bool(_)
            | ExprKind::String(_)
            | ExprKind::OwnedString(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Analisa fontes originais antes de calcular alcance, como no pipeline público.
    fn kept(source: &str) -> (Vec<String>, Vec<String>, TreeShakeStats) {
        let tokens = dartforge_lexer::lex(source).unwrap();
        let mut p = dartforge_parser::parse(&tokens, source.len()).unwrap();
        let r = dartforge_semantic::analyze(&p).unwrap();
        let stats = tree_shake(&mut p, &r);
        (
            p.functions.iter().map(|f| f.name.to_owned()).collect(),
            p.classes.iter().map(|c| c.name.to_owned()).collect(),
            stats,
        )
    }
    /// Ciclos sem raiz desaparecem; recursão alcançável e tear-offs permanecem.
    #[test]
    fn cycles_and_tearoffs() {
        let (functions, _, stats) = kept(
            "int a()=>b();int b()=>a();int live(int x){if(x==0){return 1;}return live(x-1);}int called()=>live(1);void main(){var f=called;print(f());}",
        );
        assert_eq!(functions, vec!["live", "called"]);
        assert_eq!(stats.removed_functions, 2);
    }
    /// Inicializadores, construtor e métodos de classe viva mantêm seus efeitos e chamadas.
    #[test]
    fn class_effects_and_factory_map() {
        let (functions, classes, stats) = kept(
            "int effect()=>7;class Dead{} class C{int x=effect();C();factory C.make(Map<String,int> m)=>C();int read()=>effect();}void main(){print(C.make({'x':1}).x);}",
        );
        assert_eq!(functions, vec!["effect"]);
        assert_eq!(classes, vec!["C"]);
        assert_eq!(stats.removed_classes, 1);
    }
    /// Tipos de guardas nominais e de assinaturas continuam disponíveis na emissão.
    #[test]
    fn nominal_guards_keep_contracts() {
        let (_, classes, _) = kept(
            "class A{} class B extends A{} class Dead{} bool check(Object x)=>x is A;void main(){print(check(B()));}",
        );
        assert_eq!(classes, vec!["A", "B"]);
    }
}
