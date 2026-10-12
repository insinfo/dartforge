//! As estruturas do back end do estilo curto, em arenas: regras de quebra
//! (`rule/*.dart`), *chunks* (`chunk.dart`), *spans* e níveis de
//! aninhamento de expressão (`nesting_level.dart`).
//!
//! O original usa objetos com identidade (comparações `==` entre chunks e
//! regras, conjuntos e mapas com ordem de inserção). Aqui cada objeto é um
//! índice na sua arena e a identidade é o índice; as coleções ordenadas são
//! vetores, preservando a ordem de iteração do Dart.

/// Índice de uma regra em [`Arenas::rules`].
pub type RuleId = u32;
/// Índice de um chunk em [`Arenas::chunks`].
pub type ChunkId = u32;
/// Índice de um span em [`Arenas::spans`].
pub type SpanId = u32;
/// Índice de um nível de aninhamento em [`Arenas::nestings`].
pub type NestingId = u32;

/// `Rule.unsplit`.
pub const UNSPLIT: i32 = 0;
/// `Rule.mustSplit`: qualquer valor, desde que quebre.
pub const MUST_SPLIT: i32 = -1;
/// `Rule._fullSplitConstraint`.
const FULL_SPLIT: i32 = -2;

/// Custos (`Cost` de `constants.dart`).
pub mod cost {
    pub const ARROW: i32 = 0;
    pub const NORMAL: i32 = 1;
    pub const ASSIGN: i32 = 1;
    pub const ASSIGN_BLOCK: i32 = 2;
    pub const POSITIONAL_ARGUMENTS: i32 = 2;
    pub const SINGLE_ELEMENT_LIST: i32 = 2;
    pub const SPLIT_BLOCKS: i32 = 2;
    pub const CONSTRUCTOR_NAME: i32 = 4;
    pub const INDEX: i32 = 4;
    pub const TYPE_ARGUMENT: i32 = 4;
    pub const PARAMETER_TYPE: i32 = 4;
}

/// Recuos (`Indent` de `constants.dart`).
pub mod recuo {
    pub const BLOCK: i32 = 2;
    pub const CASCADE: i32 = 2;
    pub const EXPRESSION: i32 = 4;
    pub const CONSTRUCTOR_INITIALIZER: i32 = 4;
}

/// Restrição de uma regra sobre outra (`_Constraint`).
#[derive(Debug, Clone, Copy)]
struct Constraint {
    min: i32,
    max: i32,
    other_value: i32,
}

/// As subclasses de `Rule` e seus dados.
#[derive(Debug, Clone)]
pub enum RuleKind {
    /// `Rule`.
    Simple,
    /// `SplitContainingRule` puro (não usado diretamente pelo visitante, mas
    /// a base de [`RuleKind::Positional`] e [`RuleKind::NamedRule`]).
    Positional {
        arguments_: Vec<Option<ChunkId>>,
        leading_collections: usize,
        trailing_collections: usize,
    },
    /// `NamedRule`.
    NamedRule { arguments_: Vec<Option<ChunkId>> },
    /// `CombinatorRule`.
    Combinator {
        combinators_: Vec<ChunkId>,
        names: Vec<Vec<ChunkId>>,
    },
    /// `TypeArgumentRule`.
    TypeArgument { arguments_: Vec<ChunkId> },
}

/// Uma regra de quebra (`Rule`).
#[derive(Debug, Clone)]
pub struct Rule {
    pub tipo: RuleKind,
    cost: i32,
    /// `index`: posição na lista de regras do último divisor que a usou.
    /// Como no original, nunca volta a `None`.
    pub index: Option<usize>,
    is_hardened: bool,
    /// `SplitContainingRule._trackInnerRules`.
    track_inner: bool,
    constraints: Vec<(RuleId, Vec<Constraint>)>,
    all_constrained: Option<Vec<RuleId>>,
}

/// Dados de um chunk de bloco (`BlockChunk`).
#[derive(Debug, Clone)]
pub struct Block {
    /// `argument`: o chunk antes do argumento que contém este bloco.
    pub argument: Option<ChunkId>,
    pub children: Vec<ChunkId>,
    /// Memória de `unsplitBlockLength`, calculada depois que os filhos ficam
    /// prontos.
    length: Option<usize>,
}

/// Um chunk (`Chunk`/`BlockChunk`).
#[derive(Debug, Clone)]
pub struct Chunk {
    pub texto: String,
    /// Comprimento de `texto` em unidades UTF-16 (o `length` do Dart).
    pub text_len: usize,
    pub recuo: i32,
    pub nesting_: NestingId,
    pub rule: RuleId,
    pub double: bool,
    pub flush_left: bool,
    pub space: bool,
    pub splittable: bool,
    pub spans: Vec<SpanId>,
    pub block: Option<Block>,
}

impl Chunk {
    /// `length`.
    pub fn length(&self) -> usize {
        usize::from(self.space) + self.text_len
    }
}

/// Um nível de aninhamento de expressão (`NestingLevel`).
#[derive(Debug, Clone)]
pub struct Nesting {
    pub parent: Option<NestingId>,
    pub recuo: i32,
    marked: bool,
    total: Option<i32>,
}

/// Um span (`Span`): custo pago uma vez se alguma quebra cair dentro dele.
#[derive(Debug, Clone)]
pub struct SpanC {
    pub cost: i32,
    marked: bool,
}

/// As arenas de uma formatação e os parâmetros do formatador.
#[derive(Debug)]
pub struct Arenas {
    pub rules: Vec<Rule>,
    pub chunks: Vec<Chunk>,
    pub spans: Vec<SpanC>,
    pub nestings: Vec<Nesting>,
    /// `Rule.dummy`.
    pub dummy_rule: RuleId,
    pub largura: usize,
    pub line_ending: String,
}

/// Comprimento de um texto em unidades UTF-16.
pub fn len16(s: &str) -> usize {
    if s.is_ascii() {
        s.len()
    } else {
        s.encode_utf16().count()
    }
}

impl Arenas {
    /// Arenas vazias com a regra falsa já criada.
    pub fn new_arenas(largura: usize, line_ending: String) -> Arenas {
        let mut a = Arenas {
            rules: Vec::new(),
            chunks: Vec::new(),
            spans: Vec::new(),
            nestings: Vec::new(),
            dummy_rule: 0,
            largura,
            line_ending,
        };
        a.dummy_rule = a.hard_rule();
        a
    }

    // ------------------------------------------------------------- regras

    fn new_rule(&mut self, tipo: RuleKind, cost: i32) -> RuleId {
        self.rules.push(Rule {
            tipo,
            cost,
            index: None,
            is_hardened: false,
            track_inner: true,
            constraints: Vec::new(),
            all_constrained: None,
        });
        self.rules.len() as RuleId - 1
    }

    /// `Rule([cost])`.
    pub fn rule(&mut self, cost: i32) -> RuleId {
        self.new_rule(RuleKind::Simple, cost)
    }

    /// `Rule.hard()`.
    pub fn hard_rule(&mut self) -> RuleId {
        let r = self.new_rule(RuleKind::Simple, 0);
        self.rules[r as usize].is_hardened = true;
        r
    }

    /// `TypeArgumentRule()`.
    pub fn type_argument_rule(&mut self) -> RuleId {
        self.new_rule(RuleKind::TypeArgument { arguments_: Vec::new() }, cost::TYPE_ARGUMENT)
    }

    /// `CombinatorRule()`.
    pub fn combinator_rule(&mut self) -> RuleId {
        self.new_rule(
            RuleKind::Combinator {
                combinators_: Vec::new(),
                names: Vec::new(),
            },
            cost::NORMAL,
        )
    }

    /// `PositionalRule(collectionRule, argumentCount:, leadingCollections:,
    /// trailingCollections:)`.
    pub fn positional_rule(
        &mut self,
        collection_rule: Option<RuleId>,
        amount: usize,
        leading: usize,
        trailing: usize,
    ) -> RuleId {
        let r = self.new_rule(
            RuleKind::Positional {
                arguments_: Vec::new(),
                leading_collections: leading,
                trailing_collections: trailing,
            },
            cost::NORMAL,
        );
        let n = amount as i32;
        if leading > 0 {
            self.add_constraint(r, 1, collection_rule.expect("regra de coleção"), UNSPLIT);
        }
        if leading > 0 || trailing > 0 {
            self.add_constraint(r, n + 1, collection_rule.expect("regra de coleção"), 1);
        }
        for arg in 0..leading as i32 {
            let value = n - arg + 1;
            self.add_constraint(r, value, collection_rule.expect("regra de coleção"), UNSPLIT);
        }
        let mut arg = n - trailing as i32;
        while arg < n {
            let value = n - arg + 1;
            self.add_constraint(r, value, collection_rule.expect("regra de coleção"), UNSPLIT);
            arg += 1;
        }
        r
    }

    /// `NamedRule(collectionRule, leadingCollections, trailingCollections)`.
    pub fn named_rule(&mut self, collection_rule: Option<RuleId>, leading: usize, trailing: usize) -> RuleId {
        let r = self.new_rule(RuleKind::NamedRule { arguments_: Vec::new() }, cost::NORMAL);
        if leading > 0 || trailing > 0 {
            self.add_constraint(r, 1, collection_rule.expect("regra de coleção"), UNSPLIT);
        }
        r
    }

    /// `ArgumentRule.beforeArgument` e `TypeArgumentRule.beforeArgument`.
    pub fn before_argument(&mut self, r: RuleId, chunk: Option<ChunkId>) {
        match &mut self.rules[r as usize].tipo {
            RuleKind::Positional { arguments_, .. } | RuleKind::NamedRule { arguments_ } => arguments_.push(chunk),
            RuleKind::TypeArgument { arguments_ } => arguments_.push(chunk.expect("chunk do argumento de tipo")),
            _ => panic!("beforeArgument numa regra que não é de argumentos"),
        }
    }

    /// `CombinatorRule.addCombinator`.
    pub fn add_combinator(&mut self, r: RuleId, chunk: ChunkId) {
        if let RuleKind::Combinator { combinators_, names } = &mut self.rules[r as usize].tipo {
            if !combinators_.contains(&chunk) {
                combinators_.push(chunk);
            }
            names.push(Vec::new());
        } else {
            panic!("addCombinator fora de CombinatorRule");
        }
    }

    /// `CombinatorRule.addName`.
    pub fn add_name(&mut self, r: RuleId, chunk: ChunkId) {
        if let RuleKind::Combinator { names, .. } = &mut self.rules[r as usize].tipo {
            let ultimo = names.last_mut().expect("combinador antes dos nomes");
            if !ultimo.contains(&chunk) {
                ultimo.push(chunk);
            }
        } else {
            panic!("addName fora de CombinatorRule");
        }
    }

    /// `rule is CombinatorRule`.
    pub fn is_combinator(&self, r: RuleId) -> bool {
        matches!(self.rules[r as usize].tipo, RuleKind::Combinator { .. })
    }

    /// `numValues`.
    pub fn num_values(&self, r: RuleId) -> i32 {
        match &self.rules[r as usize].tipo {
            RuleKind::Simple => 2,
            RuleKind::Positional {
                arguments_,
                leading_collections,
                trailing_collections,
            } => {
                let mut n = arguments_.len() as i32 + 1;
                if arguments_.len() > 1 {
                    n += 1;
                }
                if *leading_collections > 0 || *trailing_collections > 0 {
                    n += 1;
                }
                n
            }
            RuleKind::NamedRule { .. } => 3,
            RuleKind::Combinator { names, .. } => {
                if names.len() == 2 {
                    5
                } else {
                    3
                }
            }
            RuleKind::TypeArgument { arguments_ } => {
                if arguments_.len() == 1 {
                    2
                } else {
                    arguments_.len() as i32 + 2
                }
            }
        }
    }

    /// `fullySplitValue`.
    pub fn full_value(&self, r: RuleId) -> i32 {
        self.num_values(r) - 1
    }

    /// `cost`.
    pub fn cost(&self, r: RuleId) -> i32 {
        self.rules[r as usize].cost
    }

    /// `isHardened`.
    pub fn is_hardened(&self, r: RuleId) -> bool {
        self.rules[r as usize].is_hardened
    }

    /// `harden()`.
    pub fn harden(&mut self, r: RuleId) {
        self.rules[r as usize].is_hardened = true;
    }

    /// `splitsOnInnerRules`.
    pub fn splits_with_inner(&self, r: RuleId) -> bool {
        match self.rules[r as usize].tipo {
            RuleKind::Positional { .. } | RuleKind::NamedRule { .. } => self.rules[r as usize].track_inner,
            _ => true,
        }
    }

    /// `disableSplitOnInnerRules()`.
    pub fn disable_inner(&mut self, r: RuleId) {
        debug_assert!(self.rules[r as usize].track_inner);
        self.rules[r as usize].track_inner = false;
    }

    /// `enableSplitOnInnerRules()`.
    pub fn enable_inner(&mut self, r: RuleId) {
        debug_assert!(!self.rules[r as usize].track_inner);
        self.rules[r as usize].track_inner = true;
    }

    /// `isSplit(value, chunk)`.
    pub fn split(&self, r: RuleId, value: i32, chunk: ChunkId) -> bool {
        let rule = &self.rules[r as usize];
        if let RuleKind::TypeArgument { arguments_ } = &rule.tipo {
            // `TypeArgumentRule` sobrescreve o `isSplit` inteiro.
            if value == UNSPLIT {
                return false;
            }
            if value == self.num_values(r) - 1 {
                return true;
            }
            let i = arguments_.len() as i32 - value;
            return arguments_.get(i as usize).is_some_and(|c| *c == chunk) && i >= 0;
        }
        if rule.is_hardened {
            return true;
        }
        if value == UNSPLIT {
            return false;
        }
        match &rule.tipo {
            RuleKind::Simple => true,
            RuleKind::Positional {
                arguments_,
                leading_collections,
                trailing_collections,
            } => {
                let n = arguments_.len() as i32;
                if value == 1 {
                    return arguments_.first().copied().flatten() == Some(chunk);
                }
                if value <= n {
                    let arg = (n - value + 1) as usize;
                    return arguments_[arg] == Some(chunk);
                }
                if value == n + 1 {
                    for a in arguments_.iter().take(*leading_collections) {
                        if *a == Some(chunk) {
                            return false;
                        }
                    }
                    for a in arguments_.iter().skip(arguments_.len() - trailing_collections) {
                        if *a == Some(chunk) {
                            return false;
                        }
                    }
                    return true;
                }
                true
            }
            RuleKind::NamedRule { arguments_ } => {
                if value == 1 {
                    return arguments_.first().copied().flatten() == Some(chunk);
                }
                true
            }
            RuleKind::Combinator { combinators_, names } => {
                let combinator_node = |i: usize| combinators_.contains(&chunk) || names[i].contains(&chunk);
                match value {
                    1 => combinators_.contains(&chunk),
                    2 => combinator_node(0),
                    3 => {
                        if names.len() == 2 {
                            combinator_node(1)
                        } else {
                            true
                        }
                    }
                    _ => true,
                }
            }
            RuleKind::TypeArgument { .. } => unreachable!(),
        }
    }

    /// `constrain(value, other)`.
    pub fn constrains(&self, r: RuleId, value: i32, other: RuleId) -> Option<i32> {
        if value == UNSPLIT {
            return None;
        }
        let rule = &self.rules[r as usize];
        let (_, list) = rule.constraints.iter().find(|(o, _)| *o == other)?;
        for c in list {
            if value >= c.min && value <= c.max {
                if c.other_value == FULL_SPLIT {
                    return Some(self.full_value(other));
                }
                return Some(c.other_value);
            }
        }
        None
    }

    /// `addConstraint(value, other, otherValue)`.
    pub fn add_constraint(&mut self, r: RuleId, value: i32, other: RuleId, other_value: i32) {
        self.add_range_constraint(r, value, value, other, other_value);
    }

    /// `addRangeConstraint(min, max, other, otherValue)`.
    pub fn add_range_constraint(&mut self, r: RuleId, min: i32, max: i32, other: RuleId, other_value: i32) {
        let rule = &mut self.rules[r as usize];
        let c = Constraint { min, max, other_value };
        if let Some((_, list)) = rule.constraints.iter_mut().find(|(o, _)| *o == other) {
            list.push(c);
        } else {
            rule.constraints.push((other, vec![c]));
        }
    }

    /// `constrainWhenSplit(other)`.
    pub fn constrain_when_split(&mut self, r: RuleId, other: RuleId) {
        self.add_range_constraint(r, 1, 100000, other, FULL_SPLIT);
    }

    /// `constrainWhenFullySplit(other)`.
    pub fn constrain_when_fully_split(&mut self, r: RuleId, other: RuleId) {
        let v = self.full_value(r);
        self.add_constraint(r, v, other, FULL_SPLIT);
    }

    /// `PositionalRule.addNamedArgsConstraints(rule)`.
    pub fn named_constraints(&mut self, positional: RuleId, other: RuleId) {
        self.constrain_when_fully_split(positional, other);
        let total = self.full_value(positional);
        self.add_range_constraint(positional, 1, total, other, MUST_SPLIT);
    }

    /// `forgetUnusedRules()`.
    pub fn forget_unused(&mut self, r: RuleId) {
        let indices: Vec<bool> = self.rules[r as usize]
            .constraints
            .iter()
            .map(|(o, _)| self.rules[*o as usize].index.is_some())
            .collect();
        let rule = &mut self.rules[r as usize];
        let mut i = 0;
        rule.constraints.retain(|_| {
            let manter = indices[i];
            i += 1;
            manter
        });
        rule.all_constrained = None;
    }

    /// `constrainedRules`.
    pub fn constrained_rules(&self, r: RuleId) -> Vec<RuleId> {
        self.rules[r as usize].constraints.iter().map(|(o, _)| *o).collect()
    }

    /// Quantidade de regras restringidas (`constrainedRules.length`).
    pub fn has_constrained(&self, r: RuleId) -> bool {
        !self.rules[r as usize].constraints.is_empty()
    }

    /// A regra restringida de posição `i`.
    pub fn constrained(&self, r: RuleId, i: usize) -> Option<RuleId> {
        self.rules[r as usize].constraints.get(i).map(|(o, _)| *o)
    }

    /// `allConstrainedRules`: o fecho transitivo, incluindo a própria regra.
    pub fn all_constrained(&mut self, r: RuleId) -> Vec<RuleId> {
        if let Some(v) = &self.rules[r as usize].all_constrained {
            return v.clone();
        }
        let mut v: Vec<RuleId> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        self.walk(&mut v, &mut seen, r);
        self.rules[r as usize].all_constrained = Some(v.clone());
        v
    }

    fn walk(&self, v: &mut Vec<RuleId>, seen: &mut std::collections::HashSet<RuleId>, r: RuleId) {
        if !seen.insert(r) {
            return;
        }
        v.push(r);
        for (o, _) in &self.rules[r as usize].constraints {
            self.walk(v, seen, *o);
        }
    }

    // ------------------------------------------------------------- chunks

    /// `Chunk(rule, indent, nesting, space:, flushLeft:, isDouble:)`.
    pub fn new_chunk(&mut self, rule: RuleId, recuo: i32, nesting_: NestingId, space: bool, flush_left: bool, double: bool) -> ChunkId {
        self.chunks.push(Chunk {
            texto: String::new(),
            text_len: 0,
            recuo,
            nesting_,
            rule,
            double,
            flush_left,
            space,
            splittable: true,
            spans: Vec::new(),
            block: None,
        });
        self.chunks.len() as ChunkId - 1
    }

    /// `BlockChunk(argument, rule, indent, nesting, space:, flushLeft:)`.
    pub fn new_block(&mut self, argument: Option<ChunkId>, rule: RuleId, recuo: i32, nesting_: NestingId, space: bool, flush_left: bool) -> ChunkId {
        let c = self.new_chunk(rule, recuo, nesting_, space, flush_left, false);
        self.chunks[c as usize].block = Some(Block {
            argument,
            children: Vec::new(),
            length: None,
        });
        c
    }

    /// `Chunk.dummy()`.
    pub fn chunk_falso(&mut self) -> ChunkId {
        let n = self.new_root_nesting();
        let r = self.dummy_rule;
        let c = self.new_chunk(r, 0, n, false, false, false);
        self.chunks[c as usize].texto = "(dummy)".into();
        self.chunks[c as usize].text_len = 7;
        c
    }

    /// `appendText`.
    pub fn append(&mut self, c: ChunkId, texto: &str) {
        let ch = &mut self.chunks[c as usize];
        ch.texto.push_str(texto);
        ch.text_len += len16(texto);
    }

    /// `updateSplit(flushLeft:, isDouble:, space:)`.
    pub fn update_split(&mut self, c: ChunkId, flush_left: Option<bool>, double: bool, space: Option<bool>) {
        let ch = &mut self.chunks[c as usize];
        if let Some(r) = flush_left {
            ch.flush_left = r;
        }
        if double {
            ch.double = true;
        }
        if let Some(e) = space {
            ch.space = e;
        }
    }

    /// `unsplitBlockLength`.
    pub fn block_length(&mut self, c: ChunkId) -> usize {
        let Some(b) = &self.chunks[c as usize].block else {
            return 0;
        };
        if let Some(n) = b.length {
            return n;
        }
        let children = b.children.clone();
        let mut n = 0;
        for f in children {
            n += self.chunks[f as usize].length() + self.block_length(f);
        }
        if let Some(b) = &mut self.chunks[c as usize].block {
            b.length = Some(n);
        }
        n
    }

    /// `indentBlock(getValue)`, com o valor da regra do argumento já dado.
    pub fn indents_block(&self, c: ChunkId, value_of: &dyn Fn(RuleId) -> i32) -> bool {
        let Some(b) = &self.chunks[c as usize].block else {
            return false;
        };
        let Some(arg) = b.argument else {
            return false;
        };
        let rule = self.chunks[arg as usize].rule;
        if rule == self.dummy_rule {
            return true;
        }
        self.split(rule, value_of(rule), arg)
    }

    // ------------------------------------------------------------ aninhamento

    /// `NestingLevel()`.
    pub fn new_root_nesting(&mut self) -> NestingId {
        self.nestings.push(Nesting {
            parent: None,
            recuo: 0,
            marked: false,
            total: None,
        });
        self.nestings.len() as NestingId - 1
    }

    /// `nest(spaces)`.
    pub fn nest(&mut self, parent: NestingId, recuo: i32) -> NestingId {
        self.nestings.push(Nesting {
            parent: Some(parent),
            recuo,
            marked: false,
            total: None,
        });
        self.nestings.len() as NestingId - 1
    }

    /// `isNested`.
    pub fn is_nested(&self, n: NestingId) -> bool {
        self.nestings[n as usize].parent.is_some()
    }

    /// `mark()`.
    pub fn mark_nesting(&mut self, n: NestingId) -> bool {
        let x = &mut self.nestings[n as usize];
        if x.marked {
            return false;
        }
        x.marked = true;
        true
    }

    /// `unmark()`.
    pub fn unmark_nesting(&mut self, n: NestingId) {
        self.nestings[n as usize].marked = false;
    }

    /// `clearTotalUsedIndent()`.
    pub fn clear_total(&mut self, n: NestingId) {
        let mut current = Some(n);
        while let Some(i) = current {
            self.nestings[i as usize].total = None;
            current = self.nestings[i as usize].parent;
        }
    }

    /// `refreshTotalUsedIndent()`.
    pub fn update_total(&mut self, n: NestingId) {
        if self.nestings[n as usize].total.is_some() {
            return;
        }
        let mut total = 0;
        if let Some(p) = self.nestings[n as usize].parent {
            self.update_total(p);
            total += self.nestings[p as usize].total.expect("total do pai");
        }
        if self.nestings[n as usize].marked {
            total += self.nestings[n as usize].recuo;
        }
        self.nestings[n as usize].total = Some(total);
    }

    /// `totalUsedIndent`.
    pub fn nesting_total(&self, n: NestingId) -> i32 {
        self.nestings[n as usize].total.expect("totalUsedIndent calculado")
    }

    // ------------------------------------------------------------- spans

    /// `Span(cost)`.
    pub fn new_span(&mut self, cost: i32) -> SpanId {
        self.spans.push(SpanC { cost, marked: false });
        self.spans.len() as SpanId - 1
    }

    /// `mark()` de span.
    pub fn mark_span(&mut self, s: SpanId) -> bool {
        let x = &mut self.spans[s as usize];
        if x.marked {
            return false;
        }
        x.marked = true;
        true
    }

    /// `unmark()` de span.
    pub fn unmark_span(&mut self, s: SpanId) {
        self.spans[s as usize].marked = false;
    }
}
