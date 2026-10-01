use std::io::Write;

use codemap::{CodeMap, Span};

use crate::{
    ast::{CssStmt, MediaQuery, Style},
    color::{Color, ColorFormat, ColorSpace, NAMED_COLORS},
    common::{BinaryOp, Brackets, ListSeparator, QuoteKind},
    error::SassResult,
    selector::{
        Combinator, ComplexSelector, ComplexSelectorComponent, CompoundSelector, Namespace, Pseudo,
        SelectorList, SimpleSelector,
    },
    utils::hex_char_for,
    value::{
        fuzzy_equals, ArgList, CalculationArg, SassCalculation, SassFunction, SassMap, SassNumber,
        Value,
    },
    Options,
};

pub(crate) fn serialize_selector_list(
    list: &SelectorList,
    options: &Options,
    span: Span,
) -> String {
    let map = CodeMap::new();
    let mut serializer = Serializer::new(options, &map, false, span);

    serializer.write_selector_list(list);

    serializer.finish_for_expr()
}

pub(crate) fn serialize_calculation_arg(
    arg: &CalculationArg,
    options: &Options,
    span: Span,
) -> SassResult<String> {
    let map = CodeMap::new();
    let mut serializer = Serializer::new(options, &map, false, span);

    serializer.write_calculation_arg(arg)?;

    Ok(serializer.finish_for_expr())
}

pub(crate) fn serialize_value(val: &Value, options: &Options, span: Span) -> SassResult<String> {
    let map = CodeMap::new();
    let mut serializer = Serializer::new(options, &map, false, span);

    serializer.visit_value(val, span)?;

    Ok(serializer.finish_for_expr())
}

pub(crate) fn inspect_value(val: &Value, options: &Options, span: Span) -> SassResult<String> {
    let map = CodeMap::new();
    let mut serializer = Serializer::new(options, &map, true, span);

    serializer.visit_value(val, span)?;

    Ok(serializer.finish_for_expr())
}

pub(crate) fn inspect_float(number: f64, options: &Options, span: Span) -> String {
    let map = CodeMap::new();
    let mut serializer = Serializer::new(options, &map, true, span);

    serializer.write_number(number);

    serializer.finish_for_expr()
}

pub(crate) fn inspect_map(map: &SassMap, options: &Options, span: Span) -> SassResult<String> {
    let code_map = CodeMap::new();
    let mut serializer = Serializer::new(options, &code_map, true, span);

    serializer.visit_map(map, span)?;

    Ok(serializer.finish_for_expr())
}

pub(crate) fn inspect_function_ref(
    func: &SassFunction,
    options: &Options,
    span: Span,
) -> SassResult<String> {
    let code_map = CodeMap::new();
    let mut serializer = Serializer::new(options, &code_map, true, span);

    serializer.visit_function_ref(func, span)?;

    Ok(serializer.finish_for_expr())
}

pub(crate) fn inspect_number(
    number: &SassNumber,
    options: &Options,
    span: Span,
) -> SassResult<String> {
    let map = CodeMap::new();
    let mut serializer = Serializer::new(options, &map, true, span);

    serializer.visit_number(number)?;

    Ok(serializer.finish_for_expr())
}

pub(crate) struct Serializer<'a> {
    indentation: usize,
    options: &'a Options<'a>,
    inspect: bool,
    indent_width: usize,
    // todo: use this field
    _quote: bool,
    buffer: Vec<u8>,
    map: &'a CodeMap,
    /// O mapa de fontes em construção, quando pedido.
    mapa: Option<crate::mapa::BufferMapa>,
    /// O `span` dos erros sem nó próprio (o do valor ou da folha).
    span: Span,
}

impl<'a> Serializer<'a> {
    pub fn new(options: &'a Options<'a>, map: &'a CodeMap, inspect: bool, span: Span) -> Self {
        Self {
            inspect,
            _quote: true,
            indentation: 0,
            indent_width: 2,
            options,
            buffer: Vec::new(),
            map,
            mapa: None,
            span,
        }
    }

    fn omit_spaces_around_complex_component(&self, component: &ComplexSelectorComponent) -> bool {
        self.options.is_compressed()
            && matches!(component, ComplexSelectorComponent::Combinator(..))
    }

    fn write_pseudo_selector(&mut self, pseudo: &Pseudo) {
        if let Some(sel) = &pseudo.selector {
            if pseudo.name == "not" && sel.is_invisible() {
                return;
            }
        }

        self.buffer.push(b':');

        if !pseudo.is_syntactic_class {
            self.buffer.push(b':');
        }

        self.buffer.extend_from_slice(pseudo.name.as_bytes());

        if pseudo.argument.is_none() && pseudo.selector.is_none() {
            return;
        }

        self.buffer.push(b'(');
        if let Some(arg) = &pseudo.argument {
            self.buffer.extend_from_slice(arg.as_bytes());
            if pseudo.selector.is_some() {
                self.buffer.push(b' ');
            }
        }

        if let Some(sel) = &pseudo.selector {
            self.write_selector_list(sel);
        }

        self.buffer.push(b')');
    }

    fn write_namespace(&mut self, namespace: &Namespace) {
        match namespace {
            Namespace::Empty => self.buffer.push(b'|'),
            Namespace::Asterisk => self.buffer.extend_from_slice(b"*|"),
            Namespace::Other(namespace) => {
                self.buffer.extend_from_slice(namespace.as_bytes());
                self.buffer.push(b'|');
            }
            Namespace::None => {}
        }
    }

    fn write_simple_selector(&mut self, simple: &SimpleSelector) {
        match simple {
            SimpleSelector::Id(name) => {
                self.buffer.push(b'#');
                self.buffer.extend_from_slice(name.as_bytes());
            }
            SimpleSelector::Class(name) => {
                self.buffer.push(b'.');
                self.buffer.extend_from_slice(name.as_bytes());
            }
            SimpleSelector::Placeholder(name) => {
                self.buffer.push(b'%');
                self.buffer.extend_from_slice(name.as_bytes());
            }
            SimpleSelector::Universal(namespace) => {
                self.write_namespace(namespace);
                self.buffer.push(b'*');
            }
            SimpleSelector::Pseudo(pseudo) => self.write_pseudo_selector(pseudo),
            SimpleSelector::Type(name) => {
                self.write_namespace(&name.namespace);
                self.buffer.extend_from_slice(name.ident.as_bytes());
            }
            SimpleSelector::Attribute(attr) => write!(&mut self.buffer, "{}", attr).unwrap(),
            SimpleSelector::Parent(..) => unreachable!("It should not be possible to format `&`."),
        }
    }

    fn write_compound_selector(&mut self, compound: &CompoundSelector) {
        let mut did_write = false;
        for simple in &compound.components {
            if did_write {
                self.write_simple_selector(simple);
            } else {
                let len = self.buffer.len();
                self.write_simple_selector(simple);
                if self.buffer.len() != len {
                    did_write = true;
                }
            }
        }

        // If we emit an empty compound, it's because all of the components got
        // optimized out because they match all selectors, so we just emit the
        // universal selector.
        if !did_write {
            self.buffer.push(b'*');
        }
    }

    fn write_complex_selector_component(&mut self, component: &ComplexSelectorComponent) {
        match component {
            ComplexSelectorComponent::Combinator(Combinator::NextSibling) => self.buffer.push(b'+'),
            ComplexSelectorComponent::Combinator(Combinator::Child) => self.buffer.push(b'>'),
            ComplexSelectorComponent::Combinator(Combinator::FollowingSibling) => {
                self.buffer.push(b'~')
            }
            ComplexSelectorComponent::Compound(compound) => self.write_compound_selector(compound),
        }
    }

    fn write_complex_selector(&mut self, complex: &ComplexSelector) {
        let mut last_component = None;

        for component in &complex.components {
            if let Some(c) = last_component {
                if !self.omit_spaces_around_complex_component(c)
                    && !self.omit_spaces_around_complex_component(component)
                {
                    self.buffer.push(b' ');
                }
            }
            self.write_complex_selector_component(component);
            last_component = Some(component);
        }
    }

    fn write_selector_list(&mut self, list: &SelectorList) {
        let complexes = list.components.iter().filter(|c| !c.is_invisible());

        let mut first = true;

        for complex in complexes {
            if first {
                first = false;
            } else {
                self.buffer.push(b',');
                if complex.line_break {
                    // dart-sass `visitSelectorList`: `_writeLineFeed();
                    // _writeIndentation();` — o grass esquecia a indentação.
                    self.write_newline();
                    self.write_indentation();
                } else {
                    self.write_optional_space();
                }
            }
            self.write_complex_selector(complex);
        }
    }

    fn write_newline(&mut self) {
        if !self.options.is_compressed() {
            self.buffer.push(b'\n');
        }
    }

    /// `visitCalculation`.
    fn visit_calculation(&mut self, calculation: &SassCalculation) -> SassResult<()> {
        self.buffer.extend_from_slice(calculation.name.as_bytes());
        self.buffer.push(b'(');
        for (i, arg) in calculation.args.iter().enumerate() {
            if i > 0 {
                let sep = self.comma_separator();
                self.buffer.extend_from_slice(sep);
            }
            self.write_calculation_arg(arg)?;
        }
        self.buffer.push(b')');
        Ok(())
    }

    /// `_writeCalculationValue`.
    fn write_calculation_arg(&mut self, arg: &CalculationArg) -> SassResult<()> {
        match arg {
            CalculationArg::Number(n) if !n.num.0.is_finite() => {
                let (numer, denom) = unit_lists(&n.unit);
                self.write_calculation_infinite(n.num.0, &numer, &denom);
            }
            CalculationArg::Number(n) if n.unit.is_complex() => {
                let (numer, denom) = unit_lists(&n.unit);
                self.write_number(n.num.0);
                if let Some((first, rest)) = numer.split_first() {
                    self.buffer.extend_from_slice(first.as_bytes());
                    self.write_calculation_units(rest, &denom);
                } else {
                    self.write_calculation_units(&[], &denom);
                }
            }
            CalculationArg::Number(num) => self.visit_number(num)?,
            CalculationArg::Calculation(calc) => self.visit_calculation(calc)?,
            CalculationArg::String(s) => self.visit_unquoted_string(s),
            CalculationArg::Operation { lhs, op, rhs } => {
                // `CalculationOperator.precedence`: `+`/`-` 1, `*`/`/` 2.
                let prec = |o: &BinaryOp| {
                    if matches!(o, BinaryOp::Plus | BinaryOp::Minus) {
                        1
                    } else {
                        2
                    }
                };
                let paren_left = matches!(&**lhs, CalculationArg::Operation { op: op2, .. } if prec(op2) < prec(op));
                if paren_left {
                    self.buffer.push(b'(');
                }
                self.write_calculation_arg(lhs)?;
                if paren_left {
                    self.buffer.push(b')');
                }
                let operator_whitespace = !self.options.is_compressed() || prec(op) == 1;
                if operator_whitespace {
                    self.buffer.push(b' ');
                }
                self.buffer.extend_from_slice(op.to_string().as_bytes());
                if operator_whitespace {
                    self.buffer.push(b' ');
                }
                let paren_right = match &**rhs {
                    CalculationArg::Operation { op: op2, .. } => {
                        CalculationArg::parenthesize_calculation_rhs(*op, *op2)
                    }
                    CalculationArg::Number(n) if *op == BinaryOp::Div => {
                        if n.num.0.is_finite() {
                            n.unit.is_complex()
                        } else {
                            n.unit != crate::unit::Unit::None
                        }
                    }
                    _ => false,
                };
                if paren_right {
                    self.buffer.push(b'(');
                }
                self.write_calculation_arg(rhs)?;
                if paren_right {
                    self.buffer.push(b')');
                }
            }
        }
        Ok(())
    }

    fn write_media_query(&mut self, query: &MediaQuery) {
        if let Some(modifier) = &query.modifier {
            self.buffer.extend_from_slice(modifier.as_bytes());
            self.buffer.push(b' ');
        }

        if let Some(media_type) = &query.media_type {
            self.buffer.extend_from_slice(media_type.as_bytes());

            if !query.conditions.is_empty() {
                self.buffer.extend_from_slice(b" and ");
            }
        }

        if query.conditions.len() == 1 && query.conditions.first().unwrap().starts_with("(not ") {
            self.buffer.extend_from_slice(b"not ");
            let condition = query.conditions.first().unwrap();
            self.buffer
                .extend_from_slice(condition["(not ".len()..condition.len() - 1].as_bytes());
        } else {
            let operator = if query.conjunction { " and " } else { " or " };
            self.buffer
                .extend_from_slice(query.conditions.join(operator).as_bytes());
        }
    }

    fn finish_for_expr(self) -> String {
        String::from_utf8(self.buffer).expect("o serializador só escreve UTF-8")
    }

    fn write_indentation(&mut self) {
        if self.options.is_compressed() {
            return;
        }

        self.buffer.reserve(self.indentation);
        for _ in 0..self.indentation {
            self.buffer.push(b' ');
        }
    }

    fn write_list_separator(&mut self, sep: ListSeparator) {
        match (sep, self.options.is_compressed()) {
            (ListSeparator::Space | ListSeparator::Undecided, _) => self.buffer.push(b' '),
            (ListSeparator::Comma, true) => self.buffer.push(b','),
            (ListSeparator::Comma, false) => self.buffer.extend_from_slice(b", "),
            (ListSeparator::Slash, true) => self.buffer.push(b'/'),
            (ListSeparator::Slash, false) => self.buffer.extend_from_slice(b" / "),
        }
    }

    fn elem_needs_parens(sep: ListSeparator, elem: &Value) -> bool {
        match elem {
            Value::List(elems, sep2, brackets) => {
                if elems.len() < 2 {
                    return false;
                }

                if *brackets == Brackets::Bracketed {
                    return false;
                }

                match sep {
                    ListSeparator::Comma => *sep2 == ListSeparator::Comma,
                    ListSeparator::Slash => {
                        *sep2 == ListSeparator::Comma || *sep2 == ListSeparator::Slash
                    }
                    _ => *sep2 != ListSeparator::Undecided,
                }
            }
            _ => false,
        }
    }

    fn visit_list(
        &mut self,
        list_elems: &[Value],
        sep: ListSeparator,
        brackets: Brackets,
        span: Span,
    ) -> SassResult<()> {
        if brackets == Brackets::Bracketed {
            self.buffer.push(b'[');
        } else if list_elems.is_empty() {
            if !self.inspect {
                return Err(("() isn't a valid CSS value.", span).into());
            }

            self.buffer.extend_from_slice(b"()");
            return Ok(());
        }

        let is_singleton = self.inspect
            && list_elems.len() == 1
            && (sep == ListSeparator::Comma || sep == ListSeparator::Slash);

        if is_singleton && brackets != Brackets::Bracketed {
            self.buffer.push(b'(');
        }

        let (mut x, mut y);
        let elems: &mut dyn Iterator<Item = &Value> = if self.inspect {
            x = list_elems.iter();
            &mut x
        } else {
            y = list_elems.iter().filter(|elem| !elem.is_blank());
            &mut y
        };

        let mut elems = elems.peekable();

        while let Some(elem) = elems.next() {
            if self.inspect {
                let needs_parens = Self::elem_needs_parens(sep, elem);
                if needs_parens {
                    self.buffer.push(b'(');
                }

                self.visit_value(elem, span)?;

                if needs_parens {
                    self.buffer.push(b')');
                }
            } else {
                self.visit_value(elem, span)?;
            }

            if elems.peek().is_some() {
                self.write_list_separator(sep);
            }
        }

        if is_singleton {
            match sep {
                ListSeparator::Comma => self.buffer.push(b','),
                ListSeparator::Slash => self.buffer.push(b'/'),
                _ => unreachable!(),
            }

            if brackets != Brackets::Bracketed {
                self.buffer.push(b')');
            }
        }

        if brackets == Brackets::Bracketed {
            self.buffer.push(b']');
        }

        Ok(())
    }

    fn write_map_element(&mut self, value: &Value, span: Span) -> SassResult<()> {
        let needs_parens = matches!(value, Value::List(_, ListSeparator::Comma, Brackets::None));

        if needs_parens {
            self.buffer.push(b'(');
        }

        self.visit_value(value, span)?;

        if needs_parens {
            self.buffer.push(b')');
        }

        Ok(())
    }

    fn visit_map(&mut self, map: &SassMap, span: Span) -> SassResult<()> {
        if !self.inspect {
            return Err((
                format!(
                    "{} isn't a valid CSS value.",
                    inspect_map(map, self.options, span)?
                ),
                span,
            )
                .into());
        }

        self.buffer.push(b'(');

        let mut elems = map.iter().peekable();

        while let Some((k, v)) = elems.next() {
            self.write_map_element(&k.node, k.span)?;
            self.buffer.extend_from_slice(b": ");
            self.write_map_element(v, k.span)?;
            if elems.peek().is_some() {
                self.buffer.extend_from_slice(b", ");
            }
        }

        self.buffer.push(b')');

        Ok(())
    }

    fn visit_function_ref(&mut self, func: &SassFunction, span: Span) -> SassResult<()> {
        if !self.inspect {
            return Err((
                format!(
                    "{} isn't a valid CSS value.",
                    inspect_function_ref(func, self.options, span)?
                ),
                span,
            )
                .into());
        }

        self.buffer.extend_from_slice(b"get-function(");
        self.visit_quoted_string(false, func.name().as_str());
        self.buffer.push(b')');

        Ok(())
    }

    fn visit_arglist(&mut self, arglist: &ArgList, span: Span) -> SassResult<()> {
        self.visit_list(&arglist.elems, ListSeparator::Comma, Brackets::None, span)
    }

    fn visit_value(&mut self, value: &Value, span: Span) -> SassResult<()> {
        match value {
            Value::Dimension(num) => self.visit_number(num)?,
            Value::Color(color) if self.options.v166() => self.visit_color_166(color, span)?,
            Value::Color(color) => self.visit_color(color),
            Value::Calculation(calc) => self.visit_calculation(calc)?,
            Value::List(elems, sep, brackets) => self.visit_list(elems, *sep, *brackets, span)?,
            Value::True => self.buffer.extend_from_slice(b"true"),
            Value::False => self.buffer.extend_from_slice(b"false"),
            Value::Null => {
                if self.inspect {
                    self.buffer.extend_from_slice(b"null")
                }
            }
            Value::Map(map) => self.visit_map(map, span)?,
            Value::FunctionRef(func) => self.visit_function_ref(func, span)?,
            Value::String(s, QuoteKind::Quoted) => self.visit_quoted_string(false, s),
            Value::String(s, QuoteKind::None) => self.visit_unquoted_string(s),
            Value::ArgList(arglist) => self.visit_arglist(arglist, span)?,
        }

        Ok(())
    }

    fn write_optional_space(&mut self) {
        if !self.options.is_compressed() {
            self.buffer.push(b' ');
        }
    }

    // ## Porte do `_SerializeVisitor` do dart-sass 1.102.0
    //
    // Tudo daqui para baixo segue `lib/src/visitor/serialize.dart`: a
    // disposição das regras (`visitCssStylesheet`, `_visitChildren`,
    // comentários à direita, `isGroupEnd`), a escrita de números, cores e
    // strings, e os pontos do mapa de fontes (`_for`). O grass tinha a sua
    // própria disposição, que divergia em indentação, espaços e números.

    /// Liga o mapa de fontes.
    pub fn ligar_mapa(&mut self) {
        self.mapa = Some(crate::mapa::BufferMapa::default());
    }

    /// `_for`/`_buffer.forSpan`: associa o que `f` escreve a `span`.
    fn for_span<T>(&mut self, span: Span, f: impl FnOnce(&mut Self) -> T) -> T {
        let anterior = match self.mapa.as_mut() {
            Some(m) => Some(m.inicio(&self.buffer, self.map, span)),
            None => None,
        };
        let r = f(self);
        if let (Some(m), Some(a)) = (self.mapa.as_mut(), anterior) {
            m.fim(&self.buffer, a);
        }
        r
    }

    fn write_line_feed(&mut self) {
        if !self.options.is_compressed() {
            self.buffer.push(b'\n');
        }
    }

    fn comma_separator(&self) -> &'static [u8] {
        if self.options.is_compressed() {
            b","
        } else {
            b", "
        }
    }

    /// `_isInvisible` (fora do `inspect`): no `compressed`, também os
    /// comentários não preservados (`isInvisibleHidingComments`).
    fn is_invisible_node(&self, node: &CssStmt) -> bool {
        let comprimido = self.options.is_compressed();
        match node {
            CssStmt::RuleSet { selector, body, .. } => {
                selector.is_invisible() || body.iter().all(|c| self.is_invisible_node(c))
            }
            CssStmt::Style(_) | CssStmt::Import(..) | CssStmt::UnknownAtRule(..) => false,
            CssStmt::Comment(text, _) => comprimido && !text.starts_with("/*!"),
            CssStmt::Media(m, _) => m.body.iter().all(|c| self.is_invisible_node(c)),
            CssStmt::Supports(s, _) => s.body.iter().all(|c| self.is_invisible_node(c)),
            CssStmt::KeyframesRuleSet(k) => k.body.iter().all(|c| self.is_invisible_node(c)),
        }
    }

    /// `_requiresSemicolon`: nó pai só quando não tem bloco; comentário
    /// nunca; o resto sempre.
    pub fn requires_semicolon(stmt: &CssStmt) -> bool {
        match stmt {
            CssStmt::Style(_) | CssStmt::Import(..) => true,
            CssStmt::UnknownAtRule(rule, _) => !rule.has_body,
            CssStmt::Comment(..) => false,
            _ => false,
        }
    }

    /// A linha (0-based) de uma posição.
    fn line_of(&self, pos: codemap::Pos) -> usize {
        self.map.find_file(pos).find_line(pos)
    }

    /// `_isTrailingComment(node, previous)`: um comentário na mesma linha
    /// em que o nó anterior termina (ou, se o anterior é o pai, na linha da
    /// `{` que abre o bloco) fica na mesma linha.
    fn is_trailing_comment(&self, node: &CssStmt, previous: Span) -> bool {
        if self.options.is_compressed() {
            return false;
        }
        let CssStmt::Comment(_, span) = node else {
            return false;
        };
        let span = *span;
        let arquivo = self.map.find_file(span.low());
        if !arquivo.span.contains(previous) {
            return false;
        }
        if !previous.contains(span) {
            return self.line_of(span.low()) == self.line_of(previous.high());
        }
        let search_from = span.low() - previous.low();
        if search_from == 0 {
            return false;
        }
        let texto = arquivo.source_slice(previous).as_bytes();
        let limite = (search_from as usize - 1).min(texto.len().saturating_sub(1));
        let end_offset = texto[..=limite]
            .iter()
            .rposition(|&b| b == b'{')
            .unwrap_or(0);
        let fim = previous.low() + end_offset as u64;
        self.line_of(span.low()) == self.line_of(fim)
    }

    /// `visitCssStylesheet`.
    pub fn visit_stylesheet(&mut self, stmts: Vec<CssStmt>) -> SassResult<()> {
        let mut previous: Option<(bool, bool, Span)> = None;
        for child in stmts {
            if self.is_invisible_node(&child) {
                continue;
            }
            if let Some((semicolon, group_end, prev_span)) = previous {
                if semicolon {
                    self.buffer.push(b';');
                }
                if self.is_trailing_comment(&child, prev_span) {
                    self.write_optional_space();
                } else {
                    self.write_line_feed();
                    if group_end {
                        self.write_line_feed();
                    }
                }
            }
            previous = Some((
                Self::requires_semicolon(&child),
                child.is_group_end(),
                child.span(),
            ));
            self.visit_stmt(child)?;
        }
        if let Some((true, _, _)) = previous {
            if !self.options.is_compressed() {
                self.buffer.push(b';');
            }
        }
        Ok(())
    }

    /// `_visitChildren`.
    fn visit_children(&mut self, parent_span: Span, children: Vec<CssStmt>) -> SassResult<()> {
        self.buffer.push(b'{');
        // (precisa de `;`, `span`, é comentário)
        let mut pre_previous: Option<(bool, Span, bool)> = None;
        let mut previous: Option<(bool, Span, bool)> = None;
        let mut ultimo: Option<CssStmt> = None;
        for child in children {
            if self.is_invisible_node(&child) {
                continue;
            }
            if let Some((true, _, _)) = previous {
                self.buffer.push(b';');
            }
            let anterior = previous.map_or(parent_span, |p| p.1);
            let info = (
                Self::requires_semicolon(&child),
                child.span(),
                matches!(child, CssStmt::Comment(..)),
            );
            let copia = if info.2 { Some(child.clone()) } else { None };
            if self.is_trailing_comment(&child, anterior) {
                self.write_optional_space();
                let salvo = self.indentation;
                self.indentation = 0;
                self.visit_stmt(child)?;
                self.indentation = salvo;
            } else {
                self.write_line_feed();
                self.indentation += self.indent_width;
                self.visit_stmt(child)?;
                self.indentation -= self.indent_width;
            }
            pre_previous = previous;
            previous = Some(info);
            ultimo = copia;
        }
        if let Some((semicolon, _, _)) = previous {
            if semicolon && !self.options.is_compressed() {
                self.buffer.push(b';');
            }
            let trailing = pre_previous.is_none()
                && ultimo
                    .as_ref()
                    .is_some_and(|u| self.is_trailing_comment(u, parent_span));
            if trailing {
                self.write_optional_space();
            } else {
                self.write_line_feed();
                self.write_indentation();
            }
        }
        self.buffer.push(b'}');
        Ok(())
    }

    fn visit_stmt(&mut self, stmt: CssStmt) -> SassResult<()> {
        match stmt {
            CssStmt::RuleSet {
                selector,
                body,
                selector_span,
                span,
                ..
            } => {
                // `visitCssStyleRule`
                self.write_indentation();
                self.for_span(selector_span, |s| {
                    s.write_selector_list(&selector.as_selector_list())
                });
                self.write_optional_space();
                self.visit_children(span, body)?;
            }
            CssStmt::Media(media_rule, ..) => {
                // `visitCssMediaRule`
                self.write_indentation();
                let span = media_rule.span;
                self.for_span(span, |s| {
                    s.buffer.extend_from_slice(b"@media");
                    if let Some(first) = media_rule.query.first() {
                        if !s.options.is_compressed()
                            || first.modifier.is_some()
                            || first.media_type.is_some()
                            || (first.conditions.len() == 1
                                && first.conditions[0].starts_with("(not "))
                        {
                            s.buffer.push(b' ');
                        }
                    }
                    let mut primeiro = true;
                    for q in &media_rule.query {
                        if !primeiro {
                            let sep = s.comma_separator();
                            s.buffer.extend_from_slice(sep);
                        }
                        primeiro = false;
                        s.write_media_query(q);
                    }
                });
                self.write_optional_space();
                self.visit_children(span, media_rule.body)?;
            }
            CssStmt::UnknownAtRule(rule, ..) => {
                // `visitCssAtRule`
                self.write_indentation();
                let span = rule.span;
                self.for_span(span, |s| {
                    s.buffer.push(b'@');
                    s.buffer.extend_from_slice(rule.name.as_bytes());
                    if !rule.params.is_empty() {
                        s.buffer.push(b' ');
                        s.buffer.extend_from_slice(rule.params.as_bytes());
                    }
                });
                if rule.has_body {
                    self.write_optional_space();
                    self.visit_children(span, rule.body)?;
                }
            }
            CssStmt::Style(style) => self.write_declaration(style)?,
            CssStmt::Comment(comment, span) => self.write_comment(&comment, span),
            CssStmt::KeyframesRuleSet(kf) => {
                // `visitCssKeyframeBlock`
                self.write_indentation();
                self.for_span(kf.selector_span, |s| {
                    let mut primeiro = true;
                    for sel in &kf.selector {
                        if !primeiro {
                            let sep = s.comma_separator();
                            s.buffer.extend_from_slice(sep);
                        }
                        primeiro = false;
                        s.buffer.extend_from_slice(sel.to_string().as_bytes());
                    }
                });
                self.write_optional_space();
                self.visit_children(kf.span, kf.body)?;
            }
            CssStmt::Import(url, modifiers, span) => {
                // `visitCssImport`
                self.write_indentation();
                self.for_span(span, |s| {
                    s.buffer.extend_from_slice(b"@import");
                    s.write_optional_space();
                    s.for_span(span, |s| s.write_import_url(&url));
                    if let Some(m) = &modifiers {
                        s.write_optional_space();
                        s.buffer.extend_from_slice(m.as_bytes());
                    }
                });
            }
            CssStmt::Supports(rule, _) => {
                // `visitCssSupportsRule`
                self.write_indentation();
                let span = rule.span;
                self.for_span(span, |s| {
                    s.buffer.extend_from_slice(b"@supports");
                    if !(s.options.is_compressed() && rule.params.starts_with('(')) {
                        s.buffer.push(b' ');
                    }
                    s.buffer.extend_from_slice(rule.params.as_bytes());
                });
                self.write_optional_space();
                self.visit_children(span, rule.body)?;
            }
        }
        Ok(())
    }

    /// `_writeImportUrl`.
    fn write_import_url(&mut self, url: &str) {
        if !self.options.is_compressed() || !url.starts_with('u') {
            self.buffer.extend_from_slice(url.as_bytes());
            return;
        }
        let contents = &url[4..url.len() - 1];
        if contents.starts_with('\'') || contents.starts_with('"') {
            self.buffer.extend_from_slice(contents.as_bytes());
        } else {
            self.visit_quoted_string(false, contents);
        }
    }

    /// `visitCssComment`.
    fn write_comment(&mut self, text: &str, span: Span) {
        self.for_span(span, |s| {
            if s.options.is_compressed() && !text.starts_with("/*!") {
                return;
            }
            if text.starts_with("/*# sourceMappingURL=") || text.starts_with("/*# sourceURL=") {
                return;
            }
            match minimum_indentation(text) {
                Some(min) => {
                    let coluna = s.column_of(span.low()) as i64;
                    s.write_indentation();
                    s.write_with_indent(text, min.min(coluna));
                }
                None => {
                    s.write_indentation();
                    s.buffer.extend_from_slice(text.as_bytes());
                }
            }
        });
    }

    /// A coluna (em unidades UTF-16, como no Dart) de uma posição.
    fn column_of(&self, pos: codemap::Pos) -> usize {
        let arquivo = self.map.find_file(pos);
        let linha = arquivo.find_line(pos);
        let inicio = arquivo.line_span(linha).low();
        arquivo
            .source_slice(
                arquivo
                    .span
                    .subspan(inicio - arquivo.span.low(), pos - arquivo.span.low()),
            )
            .encode_utf16()
            .count()
    }

    /// `_writeWithIndent`.
    fn write_with_indent(&mut self, text: &str, minimum_indentation: i64) {
        let b = text.as_bytes();
        let mut i = 0;
        while i < b.len() {
            let c = b[i];
            i += 1;
            if c == b'\n' {
                break;
            }
            self.buffer.push(c);
        }
        if i >= b.len() && !text.ends_with('\n') {
            return;
        }
        loop {
            let mut line_start = i;
            let mut newlines = 1;
            loop {
                if i >= b.len() {
                    self.buffer.push(b' ');
                    return;
                }
                let c = b[i];
                i += 1;
                match c {
                    b' ' | b'\t' => continue,
                    b'\n' => {
                        line_start = i;
                        newlines += 1;
                    }
                    _ => break,
                }
            }
            for _ in 0..newlines {
                self.buffer.push(b'\n');
            }
            self.write_indentation();
            let desde = (line_start as i64 + minimum_indentation).max(0) as usize;
            // `scanner.substring(lineStart + minimumIndentation)`: até a
            // posição atual (o primeiro caractere não branco já lido).
            self.buffer.extend_from_slice(&b[desde.min(i)..i]);
            loop {
                if i >= b.len() {
                    return;
                }
                let c = b[i];
                i += 1;
                if c == b'\n' {
                    break;
                }
                self.buffer.push(c);
            }
        }
    }

    /// `visitCssDeclaration`.
    fn write_declaration(&mut self, style: Style) -> SassResult<()> {
        self.write_indentation();
        let nome = style.property.resolve_ref();
        self.for_span(style.name_span, |s| {
            s.buffer.extend_from_slice(nome.as_bytes())
        });
        self.buffer.push(b':');
        if style.declared_as_custom_property {
            // Valor cru de propriedade customizada (`parsedAsSassScript`
            // falso): o texto como veio, dobrado no `compressed` e
            // reindentado no `expanded`.
            let texto = match &style.value.node {
                Value::String(t, _) => t.clone(),
                v => serialize_value(v, self.options, style.value.span)?,
            };
            let coluna_nome = self.column_of(style.name_span.low()) as i64;
            self.for_span(style.value.span, |s| {
                if s.options.is_compressed() {
                    s.write_folded_value(&texto);
                } else {
                    s.write_reindented_value(&texto, coluna_nome);
                }
            });
            return Ok(());
        }
        self.write_optional_space();
        let valor = style.value.node;
        let span = style.value.span;
        self.for_span(style.value_span_for_map, |s| s.visit_value(&valor, span))
    }

    /// `_writeFoldedValue`.
    fn write_folded_value(&mut self, texto: &str) {
        let b = texto.as_bytes();
        let mut i = 0;
        while i < b.len() {
            let c = b[i];
            i += 1;
            if c != b'\n' {
                self.buffer.push(c);
                continue;
            }
            self.buffer.push(b' ');
            while i < b.len() && matches!(b[i], b' ' | b'\t' | b'\n' | b'\r' | b'\x0c') {
                i += 1;
            }
        }
    }

    /// `_writeReindentedValue`.
    fn write_reindented_value(&mut self, texto: &str, coluna_nome: i64) {
        match minimum_indentation(texto) {
            None => self.buffer.extend_from_slice(texto.as_bytes()),
            Some(-1) => {
                self.buffer
                    .extend_from_slice(trim_ascii_right_exclude_escape(texto).as_bytes());
                self.buffer.push(b' ');
            }
            Some(min) => self.write_with_indent(texto, min.min(coluna_nome)),
        }
    }

    /// O CSS pronto e o mapa: `serialize` acrescenta o `@charset` (ou o
    /// BOM, no `compressed`) quando há caractere fora do ASCII.
    pub fn finish_com_mapa(mut self) -> (String, Option<crate::mapa::Mapa>) {
        let prefixo = if self.options.allows_charset && self.buffer.iter().any(|&c| c > 0x7F) {
            if self.options.is_compressed() {
                "\u{FEFF}"
            } else {
                "@charset \"UTF-8\";\n"
            }
        } else {
            ""
        };
        let buffer = std::mem::take(&mut self.buffer);
        let mapa = self.mapa.take().map(|m| m.construir(&buffer, prefixo));
        let css = String::from_utf8(buffer).expect("o serializador só escreve UTF-8");
        (format!("{prefixo}{css}"), mapa)
    }

    // ### Números

    /// `_asInt`.
    ///
    /// No modo 1.66, sempre o `fuzzyAsInt` (o `_writeNumber` do 1.66 não
    /// distingue o `inspect`).
    fn as_int(&self, n: f64) -> Option<f64> {
        // No 1.101 (antes do 1.101.4) o contrário do 1.102: o aproximado fora
        // do `inspect`, e no `inspect` só o inteiro exato.
        if self.options.v1101() {
            let i = crate::value::fuzzy_as_int(n).map(|i| i as f64)?;
            return (!self.inspect || i == n).then_some(i);
        }
        if self.inspect || self.options.v166() {
            return crate::value::fuzzy_as_int(n).map(|i| i as f64);
        }
        let r = n.round();
        (r == n).then_some(r)
    }

    /// `_writeNumber`: sem notação exponencial, com no máximo
    /// `SassNumber.precision` (10) casas, e sem o `0` inicial no
    /// `compressed` (só quando o texto começa por `0`: `-0.5` fica).
    fn write_number(&mut self, number: f64) {
        if !number.is_finite() {
            self.write_calculation_infinite(number, &[], &[]);
            return;
        }
        if let Some(i) = self.as_int(number) {
            // `integer.toString()`; `-0.0` arredonda para o inteiro 0.
            let t = dart_int_text(i);
            self.buffer.extend_from_slice(t.as_bytes());
            return;
        }
        // O `{}` do Rust é o `_removeExponent(number.toString())` do Dart:
        // os mesmos dígitos mínimos, em notação posicional.
        let mut text = format!("{number}");
        // Precisão total no `inspect` só a partir do dart-sass 1.91.0.
        if self.inspect && !self.options.v166() {
            self.buffer.extend_from_slice(text.as_bytes());
            return;
        }
        if text.len() < 12 {
            if self.options.is_compressed() && text.starts_with('0') {
                text.remove(0);
            }
            self.buffer.extend_from_slice(text.as_bytes());
            return;
        }
        self.write_rounded(&text);
    }

    /// `_writeRounded`.
    fn write_rounded(&mut self, text: &str) {
        let t = text.as_bytes();
        if text.ends_with(".0") {
            self.buffer.extend_from_slice(&t[..t.len() - 2]);
            return;
        }
        let mut digits = vec![0u8; t.len() + 1];
        let mut di = 1;
        let mut ti = 0;
        let negative = t[0] == b'-';
        if negative {
            ti += 1;
        }
        loop {
            if ti == t.len() {
                self.buffer.extend_from_slice(t);
                return;
            }
            let c = t[ti];
            ti += 1;
            if c == b'.' {
                break;
            }
            digits[di] = c - b'0';
            di += 1;
        }
        let first_fractional = di;
        let after_precision = ti + 10;
        if after_precision >= t.len() {
            self.buffer.extend_from_slice(t);
            return;
        }
        while ti < after_precision {
            digits[di] = t[ti] - b'0';
            di += 1;
            ti += 1;
        }
        if t[ti] - b'0' >= 5 {
            loop {
                digits[di - 1] += 1;
                if digits[di - 1] != 10 {
                    break;
                }
                di -= 1;
            }
        }
        while di < first_fractional {
            digits[di] = 0;
            di += 1;
        }
        while di > first_fractional && digits[di - 1] == 0 {
            di -= 1;
        }
        if di == 2 && digits[0] == 0 && digits[1] == 0 {
            self.buffer.push(b'0');
            return;
        }
        if negative {
            self.buffer.push(b'-');
        }
        let mut wi = 0;
        if digits[0] == 0 {
            wi += 1;
            if self.options.is_compressed() && digits[1] == 0 {
                wi += 1;
            }
        }
        while wi < first_fractional {
            self.buffer.push(b'0' + digits[wi]);
            wi += 1;
        }
        if di > first_fractional {
            self.buffer.push(b'.');
            while wi < di {
                self.buffer.push(b'0' + digits[wi]);
                wi += 1;
            }
        }
    }

    /// `visitNumber`.
    pub fn visit_number(&mut self, number: &SassNumber) -> SassResult<()> {
        if let Some(as_slash) = &number.as_slash {
            self.visit_number(&as_slash.0)?;
            self.buffer.push(b'/');
            self.visit_number(&as_slash.1)?;
            return Ok(());
        }
        let (numer, denom) = unit_lists(&number.unit);
        if !number.num.0.is_finite() && self.options.v166() {
            // `_writeCalculationValue` do 1.66: só a primeira unidade do
            // numerador; unidades complexas são erro fora do `inspect`.
            if (numer.len() > 1 || !denom.is_empty()) && !self.inspect {
                return Err((
                    format!(
                        "{} isn't a valid CSS value.",
                        inspect_number(number, self.options, self.span)?
                    ),
                    self.span,
                )
                    .into());
            }
            self.buffer.extend_from_slice(b"calc(");
            self.write_calculation_infinite(number.num.0, &numer[..numer.len().min(1)], &[]);
            self.buffer.push(b')');
            return Ok(());
        }
        if !number.num.0.is_finite() {
            self.buffer.extend_from_slice(b"calc(");
            self.write_calculation_infinite(number.num.0, &numer, &denom);
            self.buffer.push(b')');
            return Ok(());
        }
        if (numer.len() > 1 || !denom.is_empty()) && self.options.v166() {
            // `visitNumber` do 1.66: unidades complexas não são CSS; o
            // `inspect` escreve o `unitString`.
            if !self.inspect {
                return Err((
                    format!(
                        "{} isn't a valid CSS value.",
                        inspect_number(number, self.options, self.span)?
                    ),
                    self.span,
                )
                    .into());
            }
            self.write_number(number.num.0);
            let texto = match (numer.as_slice(), denom.as_slice()) {
                ([], [d]) => format!("{d}^-1"),
                ([], _) => format!("({})^-1", denom.join("*")),
                (_, []) => numer.join("*"),
                _ => format!("{}/{}", numer.join("*"), denom.join("*")),
            };
            self.buffer.extend_from_slice(texto.as_bytes());
            return Ok(());
        }
        if numer.len() > 1 || !denom.is_empty() {
            // `hasComplexUnits`: `calc()` sem simplificar.
            self.buffer.extend_from_slice(b"calc(");
            self.write_number(number.num.0);
            if let Some((first, rest)) = numer.split_first() {
                self.buffer.extend_from_slice(first.as_bytes());
                self.write_calculation_units(rest, &denom);
            } else {
                self.write_calculation_units(&[], &denom);
            }
            self.buffer.push(b')');
            return Ok(());
        }
        self.write_number(number.num.0);
        if let Some(u) = numer.first() {
            self.buffer.extend_from_slice(u.as_bytes());
        }
        Ok(())
    }

    fn write_calculation_infinite(&mut self, n: f64, numer: &[String], denom: &[String]) {
        if n.is_nan() {
            self.buffer.extend_from_slice(b"NaN");
        } else if n > 0.0 {
            self.buffer.extend_from_slice(b"infinity");
        } else {
            self.buffer.extend_from_slice(b"-infinity");
        }
        self.write_calculation_units(numer, denom);
    }

    /// `_writeCalculationUnits`.
    fn write_calculation_units(&mut self, numer: &[String], denom: &[String]) {
        for u in numer {
            self.write_optional_space();
            self.buffer.push(b'*');
            self.write_optional_space();
            self.buffer.push(b'1');
            self.buffer.extend_from_slice(u.as_bytes());
        }
        for u in denom {
            self.write_optional_space();
            self.buffer.push(b'/');
            self.write_optional_space();
            self.buffer.push(b'1');
            self.buffer.extend_from_slice(u.as_bytes());
        }
    }

    // ### Cores

    /// `visitColor`.
    pub fn visit_color(&mut self, color: &Color) {
        let space = color.space();
        let legado_completo = matches!(space, ColorSpace::Rgb | ColorSpace::Hsl | ColorSpace::Hwb)
            && !color.has_missing_channel();
        if legado_completo {
            self.write_legacy_color(color);
            return;
        }
        match space {
            ColorSpace::Rgb => {
                self.buffer.extend_from_slice(b"rgb(");
                self.write_channel_opt(color.channel0_or_null(), None);
                self.buffer.push(b' ');
                self.write_channel_opt(color.channel1_or_null(), None);
                self.buffer.push(b' ');
                self.write_channel_opt(color.channel2_or_null(), None);
                self.maybe_write_slash_alpha(color);
                self.buffer.push(b')');
            }
            ColorSpace::Hsl | ColorSpace::Hwb => {
                self.buffer.extend_from_slice(space.name().as_bytes());
                self.buffer.push(b'(');
                let deg = if self.options.is_compressed() {
                    None
                } else {
                    Some("deg")
                };
                self.write_channel_opt(color.channel0_or_null(), deg);
                self.buffer.push(b' ');
                self.write_channel_opt(color.channel1_or_null(), Some("%"));
                self.buffer.push(b' ');
                self.write_channel_opt(color.channel2_or_null(), Some("%"));
                self.maybe_write_slash_alpha(color);
                self.buffer.push(b')');
            }
            ColorSpace::Lab | ColorSpace::Lch | ColorSpace::Oklab | ColorSpace::Oklch => {
                let max0 = if matches!(space, ColorSpace::Lab | ColorSpace::Lch) {
                    100.0
                } else {
                    1.0
                };
                let fora = !self.inspect
                    && ((!crate::color::fuzzy_in_range(color.channel0(), 0.0, max0)
                        && !color.is_channel1_missing()
                        && !color.is_channel2_missing())
                        || (matches!(space, ColorSpace::Lch | ColorSpace::Oklch)
                            && crate::value::fuzzy_less_than(color.channel1(), 0.0)
                            && !color.is_channel0_missing()
                            && !color.is_channel1_missing()));
                if fora {
                    // `color-mix()` com o XYZ para representar a cor fora da
                    // gama sem cortar os canais.
                    self.buffer.extend_from_slice(b"color-mix(in ");
                    self.buffer.extend_from_slice(space.name().as_bytes());
                    let sep = self.comma_separator();
                    self.buffer.extend_from_slice(sep);
                    let xyz = color.to_space(ColorSpace::XyzD65, true);
                    self.write_color_function(&xyz);
                    self.write_optional_space();
                    self.buffer.extend_from_slice(b"100%");
                    self.buffer.extend_from_slice(sep);
                    self.buffer
                        .extend_from_slice(if self.options.is_compressed() {
                            b"red"
                        } else {
                            b"black"
                        });
                    self.buffer.push(b')');
                    return;
                }
                self.buffer.extend_from_slice(space.name().as_bytes());
                self.buffer.push(b'(');
                let polar = space.channels()[2].is_polar_angle;
                if !self.inspect
                    && (!crate::color::fuzzy_in_range(color.channel0(), 0.0, 100.0)
                        || (polar && crate::value::fuzzy_less_than(color.channel1(), 0.0)))
                {
                    self.buffer.extend_from_slice(b"from ");
                    self.buffer
                        .extend_from_slice(if self.options.is_compressed() {
                            b"red"
                        } else {
                            b"black"
                        });
                    self.buffer.push(b' ');
                }
                if !self.options.is_compressed() && !color.is_channel0_missing() {
                    let max = space.channels()[0].max;
                    self.write_number(color.channel0() * 100.0 / max);
                    self.buffer.push(b'%');
                } else {
                    self.write_channel_opt(color.channel0_or_null(), None);
                }
                self.buffer.push(b' ');
                self.write_channel_opt(color.channel1_or_null(), None);
                self.buffer.push(b' ');
                let deg = if polar && !self.options.is_compressed() {
                    Some("deg")
                } else {
                    None
                };
                self.write_channel_opt(color.channel2_or_null(), deg);
                self.maybe_write_slash_alpha(color);
                self.buffer.push(b')');
            }
            _ => self.write_color_function(color),
        }
    }

    /// `visitColor` do dart-sass 1.66.0 (modo de compatibilidade): o RGB
    /// inteiro e os formatos do 1.66 (`rgbFunction`, `hslFunction` e o texto
    /// original); sem formato, o nome, o hexadecimal ou `rgba()`. Cor que o
    /// modelo do 1.66 não teria é recusada.
    fn visit_color_166(&mut self, color: &Color, span: Span) -> SassResult<()> {
        use crate::color::v166;
        let Some([r, g, b]) = v166::rgb(color) else {
            return Err((v166::nao_garantida(color), span).into());
        };
        let alpha = v166::alpha(color);
        let (ri, gi, bi) = (r as u32, g as u32, b as u32);
        // `namesByColor[value]`: a igualdade do `SassColor` compara o alfa
        // exato; os nomes (fora `transparent`) são opacos.
        let nome = if alpha == 1.0 {
            NAMED_COLORS
                .get_by_rgba([ri as u8, gi as u8, bi as u8])
                .copied()
        } else {
            None
        };
        if self.options.is_compressed() {
            if !fuzzy_equals(alpha, 1.0) {
                self.write_rgb_166(color, [r, g, b], alpha);
                return Ok(());
            }
            let curto = (ri & 0xF == ri >> 4) && (gi & 0xF == gi >> 4) && (bi & 0xF == bi >> 4);
            match nome {
                Some(n) if n.len() <= if curto { 4 } else { 7 } => {
                    self.buffer.extend_from_slice(n.as_bytes());
                }
                _ if curto => {
                    self.buffer.push(b'#');
                    self.buffer.push(hex_char_for(ri & 0xF) as u8);
                    self.buffer.push(hex_char_for(gi & 0xF) as u8);
                    self.buffer.push(hex_char_for(bi & 0xF) as u8);
                }
                _ => {
                    self.buffer.push(b'#');
                    self.write_hex_component(ri);
                    self.write_hex_component(gi);
                    self.write_hex_component(bi);
                }
            }
            return Ok(());
        }
        match &color.format {
            ColorFormat::RgbFunction => self.write_rgb_166(color, [r, g, b], alpha),
            ColorFormat::HslFunction => {
                let Some([h, s, l]) = v166::hsl(color) else {
                    return Err((v166::nao_garantida(color), span).into());
                };
                let opaque = fuzzy_equals(alpha, 1.0);
                self.buffer
                    .extend_from_slice(if opaque { b"hsl(" } else { b"hsla(" });
                let sep = self.comma_separator();
                self.write_number(h);
                self.buffer.extend_from_slice(sep);
                self.write_number(s);
                self.buffer.push(b'%');
                self.buffer.extend_from_slice(sep);
                self.write_number(l);
                self.buffer.push(b'%');
                if !opaque {
                    self.buffer.extend_from_slice(sep);
                    self.write_number(alpha);
                }
                self.buffer.push(b')');
            }
            ColorFormat::Literal(text) => self.buffer.extend_from_slice(text.as_bytes()),
            ColorFormat::Infer => {
                if let Some(n) = nome {
                    self.buffer.extend_from_slice(n.as_bytes());
                } else if fuzzy_equals(alpha, 1.0) {
                    self.buffer.push(b'#');
                    self.write_hex_component(ri);
                    self.write_hex_component(gi);
                    self.write_hex_component(bi);
                } else {
                    self.write_rgb_166(color, [r, g, b], alpha);
                }
            }
        }
        Ok(())
    }

    /// `_writeRgb` do 1.66: os inteiros, com vírgulas.
    fn write_rgb_166(&mut self, _color: &Color, [r, g, b]: [f64; 3], alpha: f64) {
        let opaque = fuzzy_equals(alpha, 1.0);
        self.buffer
            .extend_from_slice(if opaque { b"rgb(" } else { b"rgba(" });
        let sep = self.comma_separator();
        for (i, c) in [r, g, b].into_iter().enumerate() {
            if i > 0 {
                self.buffer.extend_from_slice(sep);
            }
            self.buffer.extend_from_slice(dart_int_text(c).as_bytes());
        }
        if !opaque {
            self.buffer.extend_from_slice(sep);
            self.write_number(alpha);
        }
        self.buffer.push(b')');
    }

    /// `_writeColorFunction`.
    fn write_color_function(&mut self, color: &Color) {
        self.buffer.extend_from_slice(b"color(");
        self.buffer
            .extend_from_slice(color.space().name().as_bytes());
        self.buffer.push(b' ');
        let canais = color.channels_or_null();
        for (i, c) in canais.iter().enumerate() {
            if i > 0 {
                self.buffer.push(b' ');
            }
            self.write_channel_opt(*c, None);
        }
        self.maybe_write_slash_alpha(color);
        self.buffer.push(b')');
    }

    /// `_maybeWriteSlashAlpha`.
    fn maybe_write_slash_alpha(&mut self, color: &Color) {
        if fuzzy_equals(color.alpha_f64(), 1.0) {
            return;
        }
        self.write_optional_space();
        self.buffer.push(b'/');
        self.write_optional_space();
        self.write_channel_opt(color.alpha_or_null(), None);
    }

    /// `_writeChannel` com canal que pode faltar (`none`).
    fn write_channel_opt(&mut self, channel: Option<f64>, unit: Option<&str>) {
        match channel {
            None => self.buffer.extend_from_slice(b"none"),
            Some(c) => self.write_channel(c, unit),
        }
    }

    /// `_writeLegacyColor`.
    fn write_legacy_color(&mut self, color: &Color) {
        let opaque = fuzzy_equals(color.alpha_f64(), 1.0);
        if !color.is_in_gamut() && !self.inspect {
            self.write_hsl(color);
            return;
        }
        if self.options.is_compressed() {
            let rgb = color.to_space(ColorSpace::Rgb, true);
            if opaque && self.try_hex_or_named_rgb(&rgb) {
                return;
            }
            if self.options.v1101() {
                self.write_legacy_color_1101(color, &rgb, opaque);
                return;
            }
            let inicio = self.buffer.len();
            self.write_rgb(&rgb);
            let rgb_texto = self.buffer.split_off(inicio);
            self.write_hsl(&rgb.to_space(ColorSpace::Hsl, true));
            let hsl_texto = self.buffer.split_off(inicio);
            // Mais dois caracteres para o HSL, pelos `%`.
            if rgb_texto.len() <= hsl_texto.len() + 2 {
                self.buffer.extend_from_slice(&rgb_texto);
            } else {
                self.buffer.extend_from_slice(&hsl_texto);
            }
            return;
        }
        if color.space() == ColorSpace::Hsl {
            self.write_hsl(color);
            return;
        } else if self.inspect && color.space() == ColorSpace::Hwb {
            self.write_hwb(color);
            return;
        }
        match &color.format {
            ColorFormat::RgbFunction => {
                self.write_rgb(color);
                return;
            }
            ColorFormat::Literal(text) => {
                self.buffer.extend_from_slice(text.as_bytes());
                return;
            }
            ColorFormat::Infer | ColorFormat::HslFunction => {}
        }
        // Cor transparente gerada sai sempre como `rgba` (sass/sass#1782).
        if opaque {
            let rgb = color.to_space(ColorSpace::Rgb, true);
            if let Some(name) = Self::color_name(&rgb) {
                self.buffer.extend_from_slice(name.as_bytes());
                return;
            }
            if let Some([r, g, b]) = Self::hex_channels(&rgb) {
                self.buffer.push(b'#');
                self.write_hex_component(r);
                self.write_hex_component(g);
                self.write_hex_component(b);
                return;
            }
        }
        if color.space() == ColorSpace::Hwb {
            self.write_hsl(color);
        } else {
            self.write_rgb(color);
        }
    }

    /// A cor legada no `compressed` do dart-sass 1.101.0–1.101.3
    /// (`_writeLegacyColor`, antes do 1.101.4): `rgb` ou `hsl` pelo tamanho só
    /// dos textos dos canais (o HSL ganha dois pelos `%`), e o alfa depois.
    fn write_legacy_color_1101(&mut self, color: &Color, rgb: &Color, opaque: bool) {
        let texto = |s: &mut Self, n: f64| {
            let inicio = s.buffer.len();
            s.write_number(n);
            String::from_utf8(s.buffer.split_off(inicio)).unwrap_or_default()
        };
        let (r, g, b) = (
            texto(self, rgb.channel0()),
            texto(self, rgb.channel1()),
            texto(self, rgb.channel2()),
        );
        let hsl = color.to_space(ColorSpace::Hsl, true);
        let (h, s, l) = (
            texto(self, hsl.channel0()),
            texto(self, hsl.channel1()),
            texto(self, hsl.channel2()),
        );
        if r.len() + g.len() + b.len() <= h.len() + s.len() + l.len() + 2 {
            let cabeca: &[u8] = if opaque { b"rgb(" } else { b"rgba(" };
            self.buffer.extend_from_slice(cabeca);
            self.buffer
                .extend_from_slice(format!("{r},{g},{b}").as_bytes());
        } else {
            let cabeca: &[u8] = if opaque { b"hsl(" } else { b"hsla(" };
            self.buffer.extend_from_slice(cabeca);
            self.buffer
                .extend_from_slice(format!("{h},{s}%,{l}%").as_bytes());
        }
        if !opaque {
            self.buffer.push(b',');
            self.write_number(color.alpha_f64());
        }
        self.buffer.push(b')');
    }

    /// `_canUseHex`: os canais (de uma cor `rgb`), se todos cabem em hex.
    fn hex_channels(rgb: &Color) -> Option<[u32; 3]> {
        let mut out = [0u32; 3];
        for (i, c) in [rgb.channel0(), rgb.channel1(), rgb.channel2()]
            .into_iter()
            .enumerate()
        {
            let inteiro = crate::value::fuzzy_as_int(c)?;
            if !(crate::color::fuzzy_greater_than_or_equals(c, 0.0)
                && crate::value::fuzzy_less_than(c, 256.0))
            {
                return None;
            }
            out[i] = inteiro.clamp(0, 255) as u32;
        }
        Some(out)
    }

    /// `namesByColor[rgb]` (cor opaca).
    fn color_name(rgb: &Color) -> Option<&'static str> {
        let [r, g, b] = Self::hex_channels(rgb)?;
        NAMED_COLORS
            .get_by_rgba([r as u8, g as u8, b as u8])
            .copied()
    }

    /// `_tryHexOrNamedRgb`.
    fn try_hex_or_named_rgb(&mut self, rgb: &Color) -> bool {
        let Some([r, g, b]) = Self::hex_channels(rgb) else {
            return false;
        };
        let short = (r & 0xF == r >> 4) && (g & 0xF == g >> 4) && (b & 0xF == b >> 4);
        match Self::color_name(rgb) {
            Some(name) if name.len() <= if short { 4 } else { 7 } => {
                self.buffer.extend_from_slice(name.as_bytes());
            }
            _ if short => {
                self.buffer.push(b'#');
                self.buffer.push(hex_char_for(r & 0xF) as u8);
                self.buffer.push(hex_char_for(g & 0xF) as u8);
                self.buffer.push(hex_char_for(b & 0xF) as u8);
            }
            _ => {
                self.buffer.push(b'#');
                self.write_hex_component(r);
                self.write_hex_component(g);
                self.write_hex_component(b);
            }
        }
        true
    }

    fn write_hex_component(&mut self, channel: u32) {
        self.buffer.push(hex_char_for(channel >> 4) as u8);
        self.buffer.push(hex_char_for(channel & 0xF) as u8);
    }

    /// `_writeChannel`.
    fn write_channel(&mut self, channel: f64, unit: Option<&str>) {
        if channel.is_finite() {
            self.write_number(channel);
            if let Some(u) = unit {
                self.buffer.extend_from_slice(u.as_bytes());
            }
        } else {
            let numer: Vec<String> = unit.map(|u| vec![u.to_owned()]).unwrap_or_default();
            self.buffer.extend_from_slice(b"calc(");
            self.write_calculation_infinite(channel, &numer, &[]);
            self.buffer.push(b')');
        }
    }

    /// `_writeRgb`.
    fn write_rgb(&mut self, color: &Color) {
        let opaque = fuzzy_equals(color.alpha_f64(), 1.0);
        let rgb = color.to_space(ColorSpace::Rgb, true);
        self.buffer
            .extend_from_slice(if opaque { b"rgb(" } else { b"rgba(" });
        let sep = self.comma_separator();
        let canais = [rgb.channel0(), rgb.channel1(), rgb.channel2()];
        // No 1.101 cada canal é um número (`_writeNumber`), sem o `%`.
        if self.options.v1101() {
            for (i, c) in canais.iter().enumerate() {
                if i > 0 {
                    self.buffer.extend_from_slice(sep);
                }
                self.write_number(*c);
            }
            if !opaque {
                self.buffer.extend_from_slice(sep);
                self.write_number(color.alpha_f64());
            }
            self.buffer.push(b')');
            return;
        }
        let inteiros: Option<Vec<f64>> = canais.iter().map(|&c| self.as_int(c)).collect();
        match inteiros {
            Some(v) => {
                for (i, c) in v.iter().enumerate() {
                    if i > 0 {
                        self.buffer.extend_from_slice(sep);
                    }
                    let t = dart_int_text(*c);
                    self.buffer.extend_from_slice(t.as_bytes());
                }
            }
            None => {
                // Os canais **da cor original** (`color.channel0`), como no
                // dart-sass, divididos por 255.
                let orig = [color.channel0(), color.channel1(), color.channel2()];
                for (i, c) in orig.iter().enumerate() {
                    if i > 0 {
                        self.buffer.extend_from_slice(sep);
                    }
                    self.write_channel(c * 100.0 / 255.0, Some("%"));
                }
            }
        }
        if !opaque {
            self.buffer.extend_from_slice(sep);
            self.write_number(color.alpha_f64());
        }
        self.buffer.push(b')');
    }

    /// `_writeHsl`.
    fn write_hsl(&mut self, color: &Color) {
        let opaque = fuzzy_equals(color.alpha_f64(), 1.0);
        let hsl = color.to_space(ColorSpace::Hsl, true);
        self.buffer
            .extend_from_slice(if opaque { b"hsl(" } else { b"hsla(" });
        let sep = self.comma_separator();
        self.write_channel(hsl.channel("hue").unwrap_or(0.0), None);
        self.buffer.extend_from_slice(sep);
        self.write_channel(hsl.channel("saturation").unwrap_or(0.0), Some("%"));
        self.buffer.extend_from_slice(sep);
        self.write_channel(hsl.channel("lightness").unwrap_or(0.0), Some("%"));
        if !opaque {
            self.buffer.extend_from_slice(sep);
            self.write_number(color.alpha_f64());
        }
        self.buffer.push(b')');
    }

    /// `_writeHwb` (só no `inspect`).
    fn write_hwb(&mut self, color: &Color) {
        self.buffer.extend_from_slice(b"hwb(");
        let hwb = color.to_space(ColorSpace::Hwb, true);
        self.write_number(hwb.channel("hue").unwrap_or(0.0));
        self.buffer.push(b' ');
        self.write_number(hwb.channel("whiteness").unwrap_or(0.0));
        self.buffer.push(b'%');
        self.buffer.push(b' ');
        self.write_number(hwb.channel("blackness").unwrap_or(0.0));
        self.buffer.push(b'%');
        if !fuzzy_equals(color.alpha_f64(), 1.0) {
            self.buffer.extend_from_slice(b" / ");
            self.write_number(color.alpha_f64());
        }
        self.buffer.push(b')');
    }

    // ### Strings

    /// `_visitUnquotedString`.
    fn visit_unquoted_string(&mut self, string: &str) {
        let mut after_newline = false;
        let mut chars = string.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\n' => {
                    self.buffer.push(b' ');
                    after_newline = true;
                }
                ' ' => {
                    if !after_newline {
                        self.buffer.push(b' ');
                    }
                }
                c => {
                    after_newline = false;
                    if !self.try_private_use(c, chars.peek().copied()) {
                        let mut tmp = [0u8; 4];
                        self.buffer
                            .extend_from_slice(c.encode_utf8(&mut tmp).as_bytes());
                    }
                }
            }
        }
    }

    /// `_tryPrivateUseCharacter`: fora do `compressed`, caractere de uso
    /// privado sai como escape.
    fn try_private_use(&mut self, c: char, next: Option<char>) -> bool {
        if self.options.is_compressed() {
            return false;
        }
        let cp = c as u32;
        if (0xE000..=0xF8FF).contains(&cp) || cp >= 0xF0000 {
            Self::write_escape(&mut self.buffer, cp, next);
            return true;
        }
        false
    }

    /// `_writeEscape`.
    fn write_escape(buffer: &mut Vec<u8>, cp: u32, next: Option<char>) {
        buffer.push(b'\\');
        buffer.extend_from_slice(format!("{cp:x}").as_bytes());
        if let Some(n) = next {
            if n.is_ascii_hexdigit() || n == ' ' || n == '\t' {
                buffer.push(b' ');
            }
        }
    }

    /// `_visitQuotedString`.
    fn visit_quoted_string(&mut self, force_double_quote: bool, string: &str) {
        let mut includes_single = false;
        let mut includes_double = false;
        let mut buffer = Vec::new();
        if force_double_quote {
            buffer.push(b'"');
        }
        let mut chars = string.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\'' if force_double_quote => buffer.push(b'\''),
                '\'' if includes_double => {
                    self.visit_quoted_string(true, string);
                    return;
                }
                '\'' => {
                    includes_single = true;
                    buffer.push(b'\'');
                }
                '"' if force_double_quote => buffer.extend_from_slice(b"\\\""),
                '"' if includes_single => {
                    self.visit_quoted_string(true, string);
                    return;
                }
                '"' => {
                    includes_double = true;
                    buffer.push(b'"');
                }
                // Quebras de linha e ASCII não imprimível viram escape (a tabulação não).
                // O U+007F só a partir do dart-sass 1.69.6.
                '\u{0}'..='\u{8}' | '\u{A}'..='\u{1F}' => {
                    Self::write_escape(&mut buffer, c as u32, chars.peek().copied());
                }
                '\u{7F}' if !self.options.v166() => {
                    Self::write_escape(&mut buffer, c as u32, chars.peek().copied());
                }
                '\\' => buffer.extend_from_slice(b"\\\\"),
                c => {
                    let cp = c as u32;
                    if !self.options.is_compressed()
                        && ((0xE000..=0xF8FF).contains(&cp) || cp >= 0xF0000)
                    {
                        Self::write_escape(&mut buffer, cp, chars.peek().copied());
                    } else {
                        let mut tmp = [0u8; 4];
                        buffer.extend_from_slice(c.encode_utf8(&mut tmp).as_bytes());
                    }
                }
            }
        }
        if force_double_quote {
            buffer.push(b'"');
            self.buffer.extend_from_slice(&buffer);
        } else {
            let quote = if includes_double { b'\'' } else { b'"' };
            self.buffer.push(quote);
            self.buffer.extend_from_slice(&buffer);
            self.buffer.push(quote);
        }
    }
}

/// `_minimumIndentation`: a menor indentação das linhas não vazias depois
/// da primeira; `None` sem quebra de linha, `-1` com quebras mas sem linha
/// indentada.
fn minimum_indentation(text: &str) -> Option<i64> {
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        i += 1;
        if c == b'\n' {
            break;
        }
    }
    if i >= b.len() {
        return if b.last() == Some(&b'\n') {
            Some(-1)
        } else {
            None
        };
    }
    let mut min: Option<i64> = None;
    while i < b.len() {
        let inicio = i;
        while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
            i += 1;
        }
        if i >= b.len() {
            continue;
        }
        if b[i] == b'\n' {
            i += 1;
            continue;
        }
        let coluna = (i - inicio) as i64;
        min = Some(min.map_or(coluna, |m| m.min(coluna)));
        while i < b.len() {
            let c = b[i];
            i += 1;
            if c == b'\n' {
                break;
            }
        }
    }
    Some(min.unwrap_or(-1))
}

/// `trimAsciiRight(value, excludeEscape: true)` do dart-sass: se o último
/// caractere não branco é uma `\\` (nem o primeiro nem o último), o
/// branco que a segue fica (é parte de um escape).
fn trim_ascii_right_exclude_escape(s: &str) -> &str {
    let b = s.as_bytes();
    let branco = |c: u8| matches!(c, b' ' | b'\t' | b'\n' | b'\r' | b'\x0c');
    let Some(i) = (0..b.len()).rev().find(|&i| !branco(b[i])) else {
        return "";
    };
    if b[i] == b'\\' && i != 0 && i != b.len() - 1 {
        &s[..i + 2]
    } else {
        &s[..i + 1]
    }
}

/// Os numeradores e denominadores de uma unidade, como textos.
fn unit_lists(unit: &crate::unit::Unit) -> (Vec<String>, Vec<String>) {
    match unit {
        crate::unit::Unit::None => (Vec::new(), Vec::new()),
        crate::unit::Unit::Complex(c) => (
            c.numer.iter().map(ToString::to_string).collect(),
            c.denom.iter().map(ToString::to_string).collect(),
        ),
        u => (vec![u.to_string()], Vec::new()),
    }
}

/// `integer.toString()` de um `double` inteiro (`_asInt`): na faixa do
/// `int` de 64 bits da VM, os dígitos exatos; fora dela o `round()` não
/// devolve o mesmo valor e o dart-sass escreve `number.toString()` sem
/// expoente — os dígitos mínimos com zeros, como o `{}` do Rust.
fn dart_int_text(i: f64) -> String {
    if i == 0.0 {
        "0".to_owned()
    } else if i.abs() < 9.223_372_036_854_775_807e18 {
        format!("{}", i as i64)
    } else {
        format!("{i}")
    }
}
