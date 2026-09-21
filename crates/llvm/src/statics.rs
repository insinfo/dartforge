//! Estáticos de classe e variáveis de topo no alvo nativo AOT.
//!
//! # Decisão de inicialização
//!
//! O alvo nativo inicializa **na carga**, não preguiçosamente. Todos os
//! estáticos são gravados no começo de `dartforge_entry()`, antes da primeira
//! instrução de `main`, nesta ordem observável: campos estáticos de cada classe,
//! classes em ordem de herança (bases antes das derivadas, empate resolvido pela
//! ordem escrita), e depois as variáveis de topo na ordem escrita.
//!
//! Essa é exatamente a ordem do backend JavaScript, que emite as classes — com
//! seus `static` — antes dos `let`/`const` de módulo e executa `main()` por
//! último. A regra do projeto é que os dois alvos concordem no comportamento
//! observável do mesmo programa, e é por isso que a ordem foi copiada de lá em
//! vez de ser inventada aqui.
//!
//! O Dart 3.6.2 e o 3.13.4 inicializam variáveis de topo e estáticos
//! **preguiçosamente**, no primeiro acesso. Um programa cujo efeito colateral de
//! inicializador seja observável imprime numa ordem diferente nos dois SDKs e
//! nos dois backends do DartForge. A divergência é do projeto, não deste
//! arquivo, e está registrada em `docs/NATIVO-PRODUCAO.md`.
//!
//! # Consequência: leitura prematura
//!
//! Com inicialização na carga, ler um estático cujo inicializador ainda não
//! executou não tem resposta correta. O JavaScript emitido falha nesse caso
//! (`let`/`const` de módulo e classes ficam na zona morta temporal e lançam
//! `ReferenceError`), então o nativo também não pode responder: recusa em tempo
//! de compilação, com o span da leitura. Nenhum programa que o JavaScript
//! executa com sucesso é recusado por esta regra.
//!
//! # Representação
//!
//! Estáticos vivem numa **área única** alocada no heap gerenciado, com um slot
//! por declaração — dois para `int?` e `bool?`, como nos campos de instância. O
//! handle da área fica em `@df_statics` e é enraizado no frame de
//! `dartforge_entry`, que vive por toda a execução; os campos são rastreados
//! precisamente pelo GC, que já percorre objetos. Não existe outra forma de
//! manter um handle vivo sem alterar o runtime: raízes só existem em frames.
//!
//! `const` não recebe tratamento próprio. Um escalar `const` é gravado no mesmo
//! slot que um `final`, pela mesma expressão já validada pela análise semântica;
//! o resultado observável é idêntico e o alvo não tem canonicalização de objetos
//! const para preservar.
//!
//! Estáticos **não participam de herança**: `C.v` resolve na declaração escrita
//! e uma subclasse não o recebe. Por isso a tabela é indexada por `(classe,
//! nome)` e nunca herda do pai, ao contrário de [`crate::objects`].
use super::*;
use crate::objects::Field;
use std::collections::{BTreeMap, BTreeSet};

/// Classe sintética da área de estáticos; nenhum despacho consulta esta identidade.
///
/// O valor é negativo de propósito: IDs nominais do frontend são `u32`, então
/// nenhuma classe do usuário pode colidir e nenhum `switch` de despacho virtual
/// pode selecionar a área por engano.
pub(super) const STATICS_CLASS: i64 = -1;

/// Nome do global LLVM que guarda o handle da área de estáticos.
pub(super) const STATICS_HANDLE: &str = "@df_statics";

/// Posição de um estático na área única e sua ordem de inicialização.
struct Entrada {
    field: Field,
    ordem: usize,
}

/// Layout da área de estáticos e a ordem observável de inicialização.
pub(super) struct Statics {
    /// Slots físicos da área; zero significa que o programa não tem estáticos.
    pub(super) slots: usize,
    globais: BTreeMap<String, Entrada>,
    campos: BTreeMap<(u32, String), Entrada>,
    /// Ordem de inicialização: `None` é o grupo de variáveis de topo.
    ///
    /// O par identifica a declaração pela classe e pelo índice escrito, o que
    /// permite reconstruir a expressão a partir do módulo na emissão sem
    /// guardar empréstimos nesta estrutura.
    pub(super) ordem: Vec<(Option<u32>, usize)>,
}

impl Statics {
    /// Reserva um slot por estático, seguindo a ordem observável de inicialização.
    ///
    /// # Erros
    /// Recusa tipos fora do subconjunto nativo e hierarquias inconsistentes.
    pub(super) fn new(module: &Module<'_>) -> Result<Self, Diagnostic> {
        let mut statics = Self {
            slots: 0,
            globais: BTreeMap::new(),
            campos: BTreeMap::new(),
            ordem: Vec::new(),
        };
        for index in ordem_de_classes(&module.classes)? {
            let class = &module.classes[index];
            if class.is_library_globals {
                continue;
            }
            for (written, member) in class.static_fields.iter().enumerate() {
                let field = statics.reservar(member)?;
                statics
                    .campos
                    .insert((class.id, member.name.to_owned()), field);
                statics.ordem.push((Some(class.id), written));
            }
        }
        for class in module.classes.iter().filter(|c| c.is_library_globals) {
            for (written, member) in class.static_fields.iter().enumerate() {
                let field = statics.reservar(member)?;
                statics.globais.insert(member.name.to_owned(), field);
                statics.ordem.push((None, written));
            }
        }
        Ok(statics)
    }

    /// Reserva os slots de uma declaração; anuláveis escalares ocupam dois.
    fn reservar(&mut self, member: &StaticField<'_>) -> Result<Entrada, Diagnostic> {
        let ty = value_ty(member.ty, member.span)?;
        let field = Field {
            ty,
            offset: self.slots,
        };
        self.slots += if matches!(ty, Ty::NullableInt | Ty::NullableBool) {
            2
        } else {
            1
        };
        Ok(Entrada {
            field,
            ordem: self.ordem.len(),
        })
    }

    /// Posição de uma variável de topo já resolvida pela análise semântica.
    pub(super) fn global(&self, name: &str) -> Option<&Field> {
        self.globais.get(name).map(|entrada| &entrada.field)
    }

    /// Posição de um campo estático da própria declaração, sem consultar a base.
    pub(super) fn class_field(&self, class: u32, name: &str) -> Option<&Field> {
        self.campos
            .get(&(class, name.to_owned()))
            .map(|entrada| &entrada.field)
    }

    /// Recusa qualquer leitura de estático que a ordem de carga não pode atender.
    ///
    /// Confere duas coisas: um inicializador não lê estático de posição igual ou
    /// posterior à sua (o que inclui ler a si mesmo), e nenhuma rotina alcançável
    /// a partir de um inicializador toca estático algum. A segunda regra é
    /// conservadora de propósito: o fecho transitivo não sabe em que ponto da
    /// sequência a rotina executa, e recusar é a única resposta honesta.
    ///
    /// # Erros
    /// Devolve o span exato da leitura recusada, não o do inicializador.
    pub(super) fn validate_order(&self, module: &Module<'_>) -> Result<(), Diagnostic> {
        if self.ordem.is_empty() {
            return Ok(());
        }
        let grafo = Grafo::new(module);
        let mut chamados = Vec::new();
        let mut leituras = Vec::new();
        let mut pendentes = Vec::new();
        for (posicao, (class_id, written)) in self.ordem.iter().enumerate() {
            let Some(initializer) = self
                .declaracao(module, *class_id, *written)?
                .initializer
                .as_ref()
            else {
                continue;
            };
            chamados.clear();
            leituras.clear();
            grafo.expressao(self, module, initializer, &mut chamados, &mut leituras);
            if let Some((_, span)) = leituras
                .iter()
                .find(|(ordem, _)| *ordem >= posicao)
                .copied()
            {
                return Err(error(
                    span,
                    "a leitura de um estático ainda não inicializado (a ordem nativa é a da carga)",
                ));
            }
            pendentes.extend(chamados.iter().copied());
        }
        let mut visitadas = BTreeSet::new();
        while let Some(rotina) = pendentes.pop() {
            if !visitadas.insert(rotina) {
                continue;
            }
            chamados.clear();
            leituras.clear();
            grafo.rotina(self, module, rotina, &mut chamados, &mut leituras);
            if let Some((_, span)) = leituras.first().copied() {
                return Err(error(
                    span,
                    "acesso a estático dentro de rotina chamada por inicializador de estático",
                ));
            }
            pendentes.extend(chamados.iter().copied());
            pendentes.extend(grafo.rotinas[rotina].base);
        }
        Ok(())
    }

    /// Localiza a declaração escrita que ocupa uma posição da ordem.
    ///
    /// # Erros
    /// Recusa HIR construída manualmente cuja ordem não corresponde às classes.
    pub(super) fn declaracao<'m>(
        &self,
        module: &'m Module<'m>,
        class_id: Option<u32>,
        written: usize,
    ) -> Result<&'m StaticField<'m>, Diagnostic> {
        let class = module
            .classes
            .iter()
            .find(|class| match class_id {
                Some(id) => class.id == id,
                None => class.is_library_globals,
            })
            .ok_or_else(|| {
                Diagnostic::new(
                    "classe de estáticos ausente na HIR LLVM",
                    Span { start: 0, end: 0 },
                )
            })?;
        class
            .static_fields
            .get(written)
            .ok_or_else(|| Diagnostic::new("estático ausente na HIR LLVM", class.span))
    }
}

/// Ordem de emissão de classes: bases antes das derivadas, empate pela fonte.
///
/// Reproduz `class_order` do backend JavaScript porque a ordem dos
/// inicializadores estáticos é observável e os dois alvos precisam concordar.
///
/// # Erros
/// Recusa IDs duplicados, base ausente e ciclo de herança na HIR recebida.
fn ordem_de_classes(classes: &[Class<'_>]) -> Result<Vec<usize>, Diagnostic> {
    let mut indices = BTreeMap::new();
    for (index, class) in classes.iter().enumerate() {
        if indices.insert(class.id, index).is_some() {
            return Err(Diagnostic::new(
                "ID de classe duplicado na HIR LLVM",
                class.span,
            ));
        }
    }
    let mut filhos = vec![Vec::new(); classes.len()];
    let mut prontas = std::collections::VecDeque::new();
    for (index, class) in classes.iter().enumerate() {
        if let Some(base) = class.superclass {
            let pai = *indices
                .get(&base)
                .ok_or_else(|| Diagnostic::new("classe base ausente na HIR LLVM", class.span))?;
            filhos[pai].push(index);
        } else {
            prontas.push_back(index);
        }
    }
    let mut ordem = Vec::with_capacity(classes.len());
    while let Some(index) = prontas.pop_front() {
        ordem.push(index);
        prontas.extend(filhos[index].iter().copied());
    }
    if ordem.len() != classes.len() {
        return Err(Diagnostic::new(
            "ciclo de herança na HIR LLVM",
            classes[0].span,
        ));
    }
    Ok(ordem)
}

/// Corpo alcançável a partir de um inicializador de estático.
struct Rotina<'a> {
    /// Instruções executadas; vazio para grupos que só têm expressões.
    corpo: &'a [Statement<'a>],
    /// Expressões avaliadas fora do corpo: inicializadores de campo, lista de
    /// inicialização e argumentos de `super`.
    avulsas: Vec<&'a Expr<'a>>,
    /// Elo seguinte da cadeia de construção, quando a classe tem base.
    base: Option<usize>,
}

/// Grafo de chamadas aproximado, suficiente para recusar leitura prematura.
///
/// O despacho dinâmico é aproximado pelo nome: um `receptor.m()` alcança todo
/// método `m` de qualquer classe. Aproximar para mais alcança mais rotinas e
/// portanto recusa mais programas — nunca aceita um que deveria ser recusado.
struct Grafo<'a> {
    rotinas: Vec<Rotina<'a>>,
    funcoes: BTreeMap<&'a str, usize>,
    metodos: BTreeMap<&'a str, Vec<usize>>,
    estaticos: BTreeMap<(u32, &'a str), usize>,
    construtores: BTreeMap<(u32, Option<&'a str>), usize>,
}

impl<'a> Grafo<'a> {
    /// Indexa funções, métodos, estáticos e cadeias de construção do módulo.
    fn new(module: &'a Module<'a>) -> Self {
        let mut grafo = Self {
            rotinas: Vec::new(),
            funcoes: BTreeMap::new(),
            metodos: BTreeMap::new(),
            estaticos: BTreeMap::new(),
            construtores: BTreeMap::new(),
        };
        for function in &module.functions {
            let id = grafo.adicionar(&function.body, Vec::new());
            grafo.funcoes.insert(function.name, id);
        }
        // Cada construção precisa saber qual elo da base continua a cadeia; os
        // índices só existem depois de todas as rotinas, então a ligação é feita
        // num segundo passe.
        let mut elos = Vec::new();
        for class in &module.classes {
            for method in class.methods.iter().chain(&class.abstract_methods) {
                let id = grafo.adicionar(&method.body, Vec::new());
                grafo.metodos.entry(method.name).or_default().push(id);
            }
            for method in &class.static_methods {
                let id = grafo.adicionar(&method.body, Vec::new());
                grafo.estaticos.insert((class.id, method.name), id);
            }
            if class.is_library_globals {
                continue;
            }
            let extras = class.constructor_extras.as_deref();
            let id = grafo.construcao(class, class.constructor.as_ref(), extras);
            grafo.construtores.insert((class.id, None), id);
            elos.push((
                id,
                class.superclass,
                extras
                    .and_then(|e| e.super_call.as_ref())
                    .and_then(|c| c.name),
            ));
            for declared in &class.named_constructors {
                let id =
                    grafo.construcao(class, Some(&declared.constructor), Some(&declared.extras));
                grafo
                    .construtores
                    .insert((class.id, Some(declared.name)), id);
                elos.push((
                    id,
                    class.superclass,
                    declared.extras.super_call.as_ref().and_then(|c| c.name),
                ));
            }
        }
        for (id, base, alvo) in elos {
            grafo.rotinas[id].base =
                base.and_then(|base| grafo.construtores.get(&(base, alvo)).copied());
        }
        grafo
    }

    /// Registra uma rotina e devolve seu índice estável.
    fn adicionar(&mut self, corpo: &'a [Statement<'a>], avulsas: Vec<&'a Expr<'a>>) -> usize {
        self.rotinas.push(Rotina {
            corpo,
            avulsas,
            base: None,
        });
        self.rotinas.len() - 1
    }

    /// Reúne as expressões que uma construção avalia antes e além do corpo.
    fn construcao(
        &mut self,
        class: &'a Class<'a>,
        constructor: Option<&'a Constructor<'a>>,
        extras: Option<&'a ConstructorExtras<'a>>,
    ) -> usize {
        let mut avulsas: Vec<&'a Expr<'a>> = class
            .fields
            .iter()
            .filter_map(|field| field.initializer.as_ref())
            .collect();
        if let Some(extras) = extras {
            avulsas.extend(extras.initializers.iter().map(|entry| &entry.value));
            if let Some(call) = &extras.super_call {
                avulsas.extend(call.arguments.iter());
            }
        }
        let corpo = constructor.map_or(&[][..], |declared| declared.body.as_slice());
        self.adicionar(corpo, avulsas)
    }

    /// Percorre uma rotina inteira, acumulando chamadas e acessos a estáticos.
    fn rotina(
        &self,
        statics: &Statics,
        module: &Module<'_>,
        rotina: usize,
        chamados: &mut Vec<usize>,
        leituras: &mut Vec<(usize, Span)>,
    ) {
        for expression in &self.rotinas[rotina].avulsas {
            self.expressao(statics, module, expression, chamados, leituras);
        }
        self.instrucoes(
            statics,
            module,
            self.rotinas[rotina].corpo,
            chamados,
            leituras,
        );
    }

    /// Percorre instruções sem recursar em formas já recusadas pela validação.
    fn instrucoes(
        &self,
        statics: &Statics,
        module: &Module<'_>,
        body: &[Statement<'_>],
        chamados: &mut Vec<usize>,
        leituras: &mut Vec<(usize, Span)>,
    ) {
        for statement in body {
            match &statement.kind {
                StatementKind::Assign { name, value } => {
                    // Uma escrita antes do inicializador é o mesmo perigo de uma
                    // leitura: o inicializador executaria depois e a sobreporia.
                    if module
                        .resolution
                        .global_accesses
                        .contains(&(statement.span.start, statement.span.end))
                        && let Some(entrada) = statics.globais.get(*name)
                    {
                        leituras.push((entrada.ordem, statement.span));
                    }
                    self.expressao(statics, module, value, chamados, leituras);
                }
                StatementKind::Variable { initializer, .. }
                | StatementKind::Print(initializer)
                | StatementKind::Expression(initializer) => {
                    self.expressao(statics, module, initializer, chamados, leituras);
                }
                StatementKind::Return(Some(value)) => {
                    self.expressao(statics, module, value, chamados, leituras);
                }
                StatementKind::Return(None) => {}
                StatementKind::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    self.expressao(statics, module, condition, chamados, leituras);
                    self.instrucoes(statics, module, then_body, chamados, leituras);
                    if let Some(body) = else_body {
                        self.instrucoes(statics, module, body, chamados, leituras);
                    }
                }
                StatementKind::While { condition, body }
                | StatementKind::DoWhile { condition, body } => {
                    self.expressao(statics, module, condition, chamados, leituras);
                    self.instrucoes(statics, module, body, chamados, leituras);
                }
                StatementKind::For {
                    initializer,
                    condition,
                    update,
                    body,
                } => {
                    if let Some(value) = initializer {
                        self.instrucoes(
                            statics,
                            module,
                            std::slice::from_ref(value.as_ref()),
                            chamados,
                            leituras,
                        );
                    }
                    if let Some(value) = condition {
                        self.expressao(statics, module, value, chamados, leituras);
                    }
                    if let Some(value) = update {
                        self.instrucoes(
                            statics,
                            module,
                            std::slice::from_ref(value.as_ref()),
                            chamados,
                            leituras,
                        );
                    }
                    self.instrucoes(statics, module, body, chamados, leituras);
                }
                StatementKind::Switch { scrutinee, cases } => {
                    self.expressao(statics, module, scrutinee, chamados, leituras);
                    for case in cases {
                        if let Pattern::Constant(value) = &case.pattern {
                            self.expressao(statics, module, value, chamados, leituras);
                        }
                        if let Some(guard) = &case.guard {
                            self.expressao(statics, module, guard, chamados, leituras);
                        }
                        self.instrucoes(statics, module, &case.body, chamados, leituras);
                    }
                }
                StatementKind::FieldAssign {
                    receiver, value, ..
                } => {
                    self.expressao(statics, module, receiver, chamados, leituras);
                    self.expressao(statics, module, value, chamados, leituras);
                }
                StatementKind::Block(body) => {
                    self.instrucoes(statics, module, body, chamados, leituras);
                }
                // As demais formas já foram recusadas pela validação do módulo;
                // percorrê-las aqui não acrescentaria alcance algum.
                _ => {}
            }
        }
    }

    /// Percorre uma expressão, anotando alvos de chamada e acessos a estáticos.
    fn expressao(
        &self,
        statics: &Statics,
        module: &Module<'_>,
        expression: &Expr<'_>,
        chamados: &mut Vec<usize>,
        leituras: &mut Vec<(usize, Span)>,
    ) {
        let chave = (expression.span.start, expression.span.end);
        match &expression.kind {
            ExprKind::Identifier(name) => {
                if module.resolution.implicit_members.contains(&chave) {
                    chamados.extend(self.metodos.get(*name).into_iter().flatten().copied());
                } else if module.resolution.global_accesses.contains(&chave)
                    && let Some(entrada) = statics.globais.get(*name)
                {
                    leituras.push((entrada.ordem, expression.span));
                }
            }
            ExprKind::EnumValue { class_id, name } => {
                if let Some(entrada) = statics.campos.get(&(*class_id, (*name).to_owned())) {
                    leituras.push((entrada.ordem, expression.span));
                }
            }
            ExprKind::Call { name, arguments } => {
                if let Some(id) = self.funcoes.get(*name) {
                    chamados.push(*id);
                } else {
                    chamados.extend(self.metodos.get(*name).into_iter().flatten().copied());
                }
                for argument in arguments {
                    self.expressao(statics, module, argument, chamados, leituras);
                }
            }
            ExprKind::MethodCall {
                receiver,
                name,
                arguments,
            } => {
                chamados.extend(self.metodos.get(*name).into_iter().flatten().copied());
                self.expressao(statics, module, receiver, chamados, leituras);
                for argument in arguments {
                    self.expressao(statics, module, argument, chamados, leituras);
                }
            }
            ExprKind::Member { receiver, name } => {
                chamados.extend(self.metodos.get(*name).into_iter().flatten().copied());
                self.expressao(statics, module, receiver, chamados, leituras);
            }
            ExprKind::Construct {
                class_id,
                arguments,
            } => {
                if let Some(id) = self.construtores.get(&(*class_id, None)) {
                    chamados.push(*id);
                }
                for argument in arguments {
                    self.expressao(statics, module, argument, chamados, leituras);
                }
            }
            ExprKind::NamedConstruct {
                class_id,
                name,
                arguments,
            } => {
                if let Some(id) = self.construtores.get(&(*class_id, Some(*name))) {
                    chamados.push(*id);
                }
                if let Some(id) = self.estaticos.get(&(*class_id, *name)) {
                    chamados.push(*id);
                }
                for argument in arguments {
                    self.expressao(statics, module, argument, chamados, leituras);
                }
            }
            ExprKind::Conditional {
                condition,
                then_value,
                else_value,
            } => {
                self.expressao(statics, module, condition, chamados, leituras);
                self.expressao(statics, module, then_value, chamados, leituras);
                self.expressao(statics, module, else_value, chamados, leituras);
            }
            ExprKind::Binary { left, right, .. } => {
                self.expressao(statics, module, left, chamados, leituras);
                self.expressao(statics, module, right, chamados, leituras);
            }
            ExprKind::Unary { operand, .. } | ExprKind::Const(operand) => {
                self.expressao(statics, module, operand, chamados, leituras);
            }
            ExprKind::Switch { scrutinee, arms } => {
                self.expressao(statics, module, scrutinee, chamados, leituras);
                for arm in arms {
                    if let Pattern::Constant(value) = &arm.pattern {
                        self.expressao(statics, module, value, chamados, leituras);
                    }
                    if let Some(guard) = &arm.guard {
                        self.expressao(statics, module, guard, chamados, leituras);
                    }
                    self.expressao(statics, module, &arm.value, chamados, leituras);
                }
            }
            // As demais formas já foram recusadas pela validação do módulo.
            _ => {}
        }
    }
}
