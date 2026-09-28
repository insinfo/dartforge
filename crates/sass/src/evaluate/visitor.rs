use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet, HashSet},
    ffi::OsStr,
    fmt,
    iter::FromIterator,
    mem,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

use codemap::{CodeMap, Span, Spanned};
use indexmap::IndexSet;

use crate::{
    ast::*,
    builtin::{
        meta::if_arguments,
        modules::{
            declare_module_color, declare_module_list, declare_module_map, declare_module_math,
            declare_module_meta, declare_module_selector, declare_module_string, Module,
        },
        GLOBAL_FUNCTIONS,
    },
    common::{unvendor, BinaryOp, Brackets, Identifier, ListSeparator, QuoteKind, UnaryOp},
    error::{SassError, SassResult},
    interner::InternedString,
    lexer::Lexer,
    parse::{
        AtRootQueryParser, CssParser, KeyframesSelectorParser, SassParser, ScssParser,
        StylesheetParser,
    },
    selector::{
        ComplexSelectorComponent, ExtendRule, ExtendedSelector, ExtensionStore, SelectorList,
        SelectorParser,
    },
    utils::{to_sentence, trim_ascii},
    value::{
        ArgList, CalculationArg, Number, SassCalculation, SassFunction, SassMap, SassNumber,
        UserDefinedFunction, Value,
    },
    ContextFlags, InputSyntax, Options,
};

use super::{
    bin_op::{add, cmp, div, mul, rem, single_eq, sub},
    css_tree::{CssTree, CssTreeIdx},
    env::Environment,
};

trait UserDefinedCallable {
    fn name(&self) -> Identifier;
    fn arguments(&self) -> &ArgumentDeclaration;
}

impl UserDefinedCallable for AstFunctionDecl {
    fn name(&self) -> Identifier {
        self.name.node
    }

    fn arguments(&self) -> &ArgumentDeclaration {
        &self.arguments
    }
}

impl UserDefinedCallable for Arc<AstFunctionDecl> {
    fn name(&self) -> Identifier {
        self.name.node
    }

    fn arguments(&self) -> &ArgumentDeclaration {
        &self.arguments
    }
}

impl UserDefinedCallable for AstMixin {
    fn name(&self) -> Identifier {
        self.name
    }

    fn arguments(&self) -> &ArgumentDeclaration {
        &self.args
    }
}

impl UserDefinedCallable for Arc<CallableContentBlock> {
    fn name(&self) -> Identifier {
        Identifier::from("@content")
    }

    fn arguments(&self) -> &ArgumentDeclaration {
        &self.content.args
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CallableContentBlock {
    content: AstContentBlock,
    env: Environment,
}

/// O resultado de uma condição do `if()` do CSS: decidida ou texto do CSS.
pub(crate) enum CondIf {
    Bool(bool),
    Str(String),
}

/// Evaluation context of the current execution
#[derive(Debug)]
pub struct Visitor<'a> {
    pub(crate) declaration_name: Option<String>,
    pub(crate) flags: ContextFlags,
    pub(crate) env: Environment,
    pub(crate) style_rule_ignoring_at_root: Option<ExtendedSelector>,
    /// Os `span`s (seletor, regra) da regra de estilo corrente: a cópia que
    /// o dart-sass faz dela (`copyWithoutChildren`) dentro de `@media` e
    /// afins mantém os dela.
    pub(crate) style_rule_spans: (Span, Span),
    // avoid emitting duplicate warnings for the same span
    pub(crate) warnings_emitted: HashSet<Span>,
    pub(crate) media_queries: Option<Vec<MediaQuery>>,
    pub(crate) media_query_sources: Option<IndexSet<MediaQuery>>,
    pub(crate) extender: ExtensionStore,

    /// The complete file path of the current file being visited. Imports are
    /// resolved relative to this path
    pub current_import_path: PathBuf,
    pub(crate) is_plain_css: bool,
    pub(crate) modules: BTreeMap<PathBuf, Arc<RefCell<Module>>>,
    pub(crate) active_modules: BTreeSet<PathBuf>,
    css_tree: CssTree,
    parent: Option<CssTreeIdx>,
    configuration: Rc<RefCell<Configuration>>,
    import_nodes: Vec<CssStmt>,
    pub options: &'a Options<'a>,
    pub(crate) map: &'a mut CodeMap,
    // todo: remove
    empty_span: Span,
    import_cache: BTreeMap<PathBuf, StyleSheet>,
    /// As a simple heuristic, we don't cache the results of an import unless it
    /// has been seen in the past. In the majority of cases, files are imported
    /// at most once.
    files_seen: BTreeSet<PathBuf>,
}

impl<'a> Visitor<'a> {
    pub fn new(
        path: &Path,
        options: &'a Options<'a>,
        map: &'a mut CodeMap,
        empty_span: Span,
    ) -> Self {
        let mut flags = ContextFlags::empty();
        flags.set(ContextFlags::IN_SEMI_GLOBAL_SCOPE, true);

        let extender = ExtensionStore::new(empty_span);

        let current_import_path = path.to_path_buf();

        Self {
            declaration_name: None,
            style_rule_ignoring_at_root: None,
            style_rule_spans: (empty_span, empty_span),
            flags,
            warnings_emitted: HashSet::new(),
            media_queries: None,
            media_query_sources: None,
            env: Environment::new(),
            extender,
            css_tree: CssTree::new(),
            parent: None,
            current_import_path,
            configuration: Rc::new(RefCell::new(Configuration::empty())),
            is_plain_css: false,
            import_nodes: Vec::new(),
            modules: BTreeMap::new(),
            active_modules: BTreeSet::new(),
            options,
            empty_span,
            map,
            import_cache: BTreeMap::new(),
            files_seen: BTreeSet::new(),
        }
    }

    pub(crate) fn visit_stylesheet(&mut self, mut style_sheet: StyleSheet) -> SassResult<()> {
        self.active_modules.insert(style_sheet.url.clone());
        let was_in_plain_css = self.is_plain_css;
        self.is_plain_css = style_sheet.is_plain_css;
        mem::swap(&mut self.current_import_path, &mut style_sheet.url);

        for stmt in style_sheet.body {
            let result = self.visit_stmt(stmt)?;
            debug_assert!(result.is_none());
        }

        mem::swap(&mut self.current_import_path, &mut style_sheet.url);
        self.is_plain_css = was_in_plain_css;

        self.active_modules.remove(&style_sheet.url);

        Ok(())
    }

    pub(crate) fn finish(mut self) -> Vec<CssStmt> {
        let mut finished_tree = self.css_tree.finish();
        if self.import_nodes.is_empty() {
            finished_tree
        } else {
            self.import_nodes.append(&mut finished_tree);
            self.import_nodes
        }
    }

    fn visit_return_rule(&mut self, ret: AstReturn) -> SassResult<Option<Value>> {
        let val = self.visit_expr(ret.val)?;

        Ok(Some(self.without_slash(val)))
    }

    // todo: we really don't have to return Option<Value> from all of these children
    pub(crate) fn visit_stmt(&mut self, stmt: AstStmt) -> SassResult<Option<Value>> {
        match stmt {
            AstStmt::RuleSet(ruleset) => self.visit_ruleset(ruleset),
            AstStmt::Style(style) => self.visit_style(style),
            AstStmt::SilentComment(..) => Ok(None),
            AstStmt::If(if_stmt) => self.visit_if_stmt(if_stmt),
            AstStmt::For(for_stmt) => self.visit_for_stmt(for_stmt),
            AstStmt::Return(ret) => self.visit_return_rule(ret),
            AstStmt::Each(each_stmt) => self.visit_each_stmt(each_stmt),
            AstStmt::Media(media_rule) => self.visit_media_rule(media_rule),
            AstStmt::Include(include_stmt) => self.visit_include_stmt(include_stmt),
            AstStmt::While(while_stmt) => self.visit_while_stmt(&while_stmt),
            AstStmt::VariableDecl(decl) => self.visit_variable_decl(decl),
            AstStmt::LoudComment(comment) => self.visit_loud_comment(comment),
            AstStmt::ImportRule(import_rule) => self.visit_import_rule(import_rule),
            AstStmt::FunctionDecl(func) => {
                self.visit_function_decl(func);
                Ok(None)
            }
            AstStmt::Mixin(mixin) => {
                self.visit_mixin_decl(mixin);
                Ok(None)
            }
            AstStmt::ContentRule(content_rule) => self.visit_content_rule(content_rule),
            AstStmt::Warn(warn_rule) => {
                self.visit_warn_rule(warn_rule)?;
                Ok(None)
            }
            AstStmt::UnknownAtRule(unknown_at_rule) => self.visit_unknown_at_rule(unknown_at_rule),
            AstStmt::ErrorRule(error_rule) => Err(self.visit_error_rule(error_rule)?),
            AstStmt::Extend(extend_rule) => self.visit_extend_rule(extend_rule),
            AstStmt::AtRootRule(at_root_rule) => self.visit_at_root_rule(at_root_rule),
            AstStmt::Debug(debug_rule) => self.visit_debug_rule(debug_rule),
            AstStmt::Use(use_rule) => {
                self.visit_use_rule(use_rule)?;
                Ok(None)
            }
            AstStmt::Forward(forward_rule) => {
                self.visit_forward_rule(forward_rule)?;
                Ok(None)
            }
            AstStmt::Supports(supports_rule) => {
                self.visit_supports_rule(supports_rule)?;
                Ok(None)
            }
        }
    }

    fn visit_forward_rule(&mut self, forward_rule: AstForwardRule) -> SassResult<()> {
        let old_config = Rc::clone(&self.configuration);
        let adjusted_config = Configuration::through_forward(Rc::clone(&old_config), &forward_rule);

        if !forward_rule.configuration.is_empty() {
            let new_configuration =
                self.add_forward_configuration(Rc::clone(&adjusted_config), &forward_rule)?;

            self.load_module(
                forward_rule.url.as_path(),
                Some(Rc::clone(&new_configuration)),
                false,
                forward_rule.span,
                |visitor, module, _| {
                    visitor.env.forward_module(module, forward_rule.clone());

                    Ok(())
                },
            )?;

            Self::remove_used_configuration(
                &adjusted_config,
                &new_configuration,
                &forward_rule
                    .configuration
                    .iter()
                    .filter(|var| !var.is_guarded)
                    .map(|var| var.name.node)
                    .collect(),
            );

            // Remove all the variables that weren't configured by this particular
            // `@forward` before checking that the configuration is empty. Errors for
            // outer `with` clauses will be thrown once those clauses finish
            // executing.
            let configured_variables: HashSet<Identifier> = forward_rule
                .configuration
                .iter()
                .map(|var| var.name.node)
                .collect();

            let mut to_remove = Vec::new();

            for name in (*new_configuration).borrow().values.keys() {
                if !configured_variables.contains(&name) {
                    to_remove.push(name);
                }
            }

            for name in to_remove {
                (*new_configuration).borrow_mut().remove(name);
            }

            Self::assert_configuration_is_empty(&new_configuration, false)?;
        } else {
            self.configuration = adjusted_config;
            let url = forward_rule.url.clone();
            self.load_module(
                url.as_path(),
                None,
                false,
                forward_rule.span,
                move |visitor, module, _| {
                    visitor.env.forward_module(module, forward_rule.clone());

                    Ok(())
                },
            )?;
            self.configuration = old_config;
        }

        Ok(())
    }

    #[allow(clippy::unnecessary_unwrap)]
    fn add_forward_configuration(
        &mut self,
        config: Rc<RefCell<Configuration>>,
        forward_rule: &AstForwardRule,
    ) -> SassResult<Rc<RefCell<Configuration>>> {
        let mut new_values = BTreeMap::from_iter((*config).borrow().values.iter());

        for variable in &forward_rule.configuration {
            if variable.is_guarded {
                let old_value = (*config).borrow_mut().remove(variable.name.node);

                if old_value.is_some()
                    && !matches!(
                        old_value,
                        Some(ConfiguredValue {
                            value: Value::Null,
                            ..
                        })
                    )
                {
                    new_values.insert(variable.name.node, old_value.unwrap());
                    continue;
                }
            }

            // todo: superfluous clone?
            let node = self.expression_node(&variable.expr.node, variable.expr.span);
            let value = self.visit_expr(variable.expr.node.clone())?;
            let value = self.without_slash(value);

            new_values.insert(
                variable.name.node,
                ConfiguredValue::explicit(value, variable.expr.span).with_node(node),
            );
        }

        Ok(Rc::new(RefCell::new(
            if !(*config).borrow().is_implicit() || (*config).borrow().is_empty() {
                Configuration::explicit(new_values, forward_rule.span)
            } else {
                Configuration::implicit(new_values)
            },
        )))
    }

    /// Remove configured values from `upstream` that have been removed from
    /// `downstream`, unless they match a name in `except`.
    fn remove_used_configuration(
        upstream: &Rc<RefCell<Configuration>>,
        downstream: &Rc<RefCell<Configuration>>,
        except: &HashSet<Identifier>,
    ) {
        let mut names_to_remove = Vec::new();
        let downstream_keys = (*downstream).borrow().values.keys();
        for name in (*upstream).borrow().values.keys() {
            if except.contains(&name) {
                continue;
            }

            if !downstream_keys.contains(&name) {
                names_to_remove.push(name);
            }
        }

        for name in names_to_remove {
            (*upstream).borrow_mut().remove(name);
        }
    }

    fn parenthesize_supports_condition(
        &mut self,
        condition: AstSupportsCondition,
        operator: Option<&str>,
    ) -> SassResult<String> {
        match &condition {
            AstSupportsCondition::Negation(..) => {
                Ok(format!("({})", self.visit_supports_condition(condition)?))
            }
            AstSupportsCondition::Operation {
                operator: operator2,
                ..
            } if operator2.is_none() || operator2.as_deref() != operator => {
                Ok(format!("({})", self.visit_supports_condition(condition)?))
            }
            _ => self.visit_supports_condition(condition),
        }
    }

    fn visit_supports_condition(&mut self, condition: AstSupportsCondition) -> SassResult<String> {
        match condition {
            AstSupportsCondition::Operation {
                left,
                operator,
                right,
            } => Ok(format!(
                "{} {} {}",
                self.parenthesize_supports_condition(*left, operator.as_deref())?,
                operator.as_ref().unwrap(),
                self.parenthesize_supports_condition(*right, operator.as_deref())?
            )),
            AstSupportsCondition::Negation(condition) => Ok(format!(
                "not {}",
                self.parenthesize_supports_condition(*condition, None)?
            )),
            AstSupportsCondition::Interpolation(expr) => {
                self.evaluate_to_css(expr, QuoteKind::None, self.empty_span)
            }
            AstSupportsCondition::Declaration { name, value } => {
                let old_in_supports_decl = self.flags.in_supports_declaration();
                self.flags.set(ContextFlags::IN_SUPPORTS_DECLARATION, true);

                let is_custom_property = match &name {
                    AstExpr::String(StringExpr(text, QuoteKind::None), ..) => {
                        text.initial_plain().starts_with("--")
                    }
                    _ => false,
                };

                let result = format!(
                    "({}:{}{})",
                    self.evaluate_to_css(name, QuoteKind::Quoted, self.empty_span)?,
                    if is_custom_property { "" } else { " " },
                    self.evaluate_to_css(value, QuoteKind::Quoted, self.empty_span)?,
                );

                self.flags
                    .set(ContextFlags::IN_SUPPORTS_DECLARATION, old_in_supports_decl);

                Ok(result)
            }
            AstSupportsCondition::Function { name, args } => Ok(format!(
                "{}({})",
                self.perform_interpolation(name, false)?,
                self.perform_interpolation(args, false)?
            )),
            AstSupportsCondition::Anything { contents } => Ok(format!(
                "({})",
                self.perform_interpolation(contents, false)?,
            )),
        }
    }

    fn visit_supports_rule(&mut self, supports_rule: AstSupportsRule) -> SassResult<()> {
        if self.declaration_name.is_some() {
            return Err((
                "Supports rules may not be used within nested declarations.",
                supports_rule.span,
            )
                .into());
        }

        let condition = self.visit_supports_condition(supports_rule.condition)?;

        let css_supports_rule = CssStmt::Supports(
            SupportsRule {
                params: condition,
                body: Vec::new(),
                span: supports_rule.span,
            },
            false,
        );

        let children = supports_rule.body;

        self.with_parent(
            css_supports_rule,
            true,
            |visitor| {
                if !visitor.style_rule_exists() {
                    for stmt in children {
                        let result = visitor.visit_stmt(stmt)?;
                        debug_assert!(result.is_none());
                    }
                } else {
                    // If we're in a style rule, copy it into the supports rule so that
                    // declarations immediately inside @supports have somewhere to go.
                    //
                    // For example, "a {@supports (a: b) {b: c}}" should produce "@supports
                    // (a: b) {a {b: c}}".
                    let selector = visitor.style_rule_ignoring_at_root.clone().unwrap();
                    let (selector_span, span) = visitor.style_rule_spans;
                    let ruleset = CssStmt::RuleSet {
                        selector,
                        body: Vec::new(),
                        is_group_end: false,
                        selector_span,
                        span,
                    };

                    visitor.with_parent(
                        ruleset,
                        false,
                        |visitor| {
                            for stmt in children {
                                let result = visitor.visit_stmt(stmt)?;
                                debug_assert!(result.is_none());
                            }

                            Ok(())
                        },
                        |_| false,
                    )?;
                }

                Ok(())
            },
            CssStmt::is_style_rule,
        )?;

        Ok(())
    }

    fn execute(
        &mut self,
        stylesheet: StyleSheet,
        configuration: Option<Rc<RefCell<Configuration>>>,
        // todo: different errors based on this
        _names_in_errors: bool,
    ) -> SassResult<Arc<RefCell<Module>>> {
        let url = stylesheet.url.clone();

        // todo: use canonical url for modules
        if let Some(already_loaded) = self.modules.get(&stylesheet.url) {
            let current_configuration =
                configuration.unwrap_or_else(|| Rc::clone(&self.configuration));

            if !current_configuration.borrow().is_implicit() {
                //   if (!_moduleConfigurations[url]!.sameOriginal(currentConfiguration) &&
                //       currentConfiguration is ExplicitConfiguration) {
                //     var message = namesInErrors
                //         ? "${p.prettyUri(url)} was already loaded, so it can't be "
                //             "configured using \"with\"."
                //         : "This module was already loaded, so it can't be configured using "
                //             "\"with\".";

                //     var existingSpan = _moduleNodes[url]?.span;
                //     var configurationSpan = configuration == null
                //         ? currentConfiguration.nodeWithSpan.span
                //         : null;
                //     var secondarySpans = {
                //       if (existingSpan != null) existingSpan: "original load",
                //       if (configurationSpan != null) configurationSpan: "configuration"
                //     };

                //     throw secondarySpans.isEmpty
                //         ? _exception(message)
                //         : _multiSpanException(message, "new load", secondarySpans);
                //   }
            }

            return Ok(Arc::clone(already_loaded));
        }

        let env = Environment::new();
        let mut extension_store = ExtensionStore::new(self.empty_span);

        self.with_environment::<SassResult<()>, _>(env.new_closure(), |visitor| {
            let old_parent = visitor.parent;
            mem::swap(&mut visitor.extender, &mut extension_store);
            let old_style_rule = visitor.style_rule_ignoring_at_root.take();
            let old_style_rule_spans = visitor.style_rule_spans;
            let old_media_queries = visitor.media_queries.take();
            let old_declaration_name = visitor.declaration_name.take();
            let old_in_unknown_at_rule = visitor.flags.in_unknown_at_rule();
            let old_at_root_excluding_style_rule = visitor.flags.at_root_excluding_style_rule();
            let old_in_keyframes = visitor.flags.in_keyframes();
            let old_configuration = if let Some(new_config) = configuration {
                Some(mem::replace(&mut visitor.configuration, new_config))
            } else {
                None
            };
            visitor.parent = None;
            visitor.flags.set(ContextFlags::IN_UNKNOWN_AT_RULE, false);
            visitor
                .flags
                .set(ContextFlags::AT_ROOT_EXCLUDING_STYLE_RULE, false);
            visitor.flags.set(ContextFlags::IN_KEYFRAMES, false);

            visitor.visit_stylesheet(stylesheet)?;

            // visitor.importer = old_importer;
            // visitor.stylesheet = old_stylesheet;
            // visitor.root = old_root;
            visitor.parent = old_parent;
            // visitor.end_of_imports = old_end_of_imports;
            // visitor.out_of_order_imports = old_out_of_order_imports;
            mem::swap(&mut visitor.extender, &mut extension_store);
            visitor.style_rule_ignoring_at_root = old_style_rule;
            visitor.style_rule_spans = old_style_rule_spans;
            visitor.media_queries = old_media_queries;
            visitor.declaration_name = old_declaration_name;
            visitor
                .flags
                .set(ContextFlags::IN_UNKNOWN_AT_RULE, old_in_unknown_at_rule);
            visitor.flags.set(
                ContextFlags::AT_ROOT_EXCLUDING_STYLE_RULE,
                old_at_root_excluding_style_rule,
            );
            visitor
                .flags
                .set(ContextFlags::IN_KEYFRAMES, old_in_keyframes);
            if let Some(old_config) = old_configuration {
                visitor.configuration = old_config;
            }

            Ok(())
        })?;

        let module = env.to_module(extension_store);

        self.modules.insert(url, Arc::clone(&module));

        Ok(module)
    }

    pub(crate) fn load_module(
        &mut self,
        url: &Path,
        configuration: Option<Rc<RefCell<Configuration>>>,
        names_in_errors: bool,
        span: Span,
        callback: impl Fn(&mut Self, Arc<RefCell<Module>>, StyleSheet) -> SassResult<()>,
    ) -> SassResult<()> {
        let builtin = match url.to_string_lossy().as_ref() {
            "sass:color" => Some(declare_module_color()),
            "sass:list" => Some(declare_module_list()),
            "sass:map" => Some(declare_module_map()),
            "sass:math" => Some(declare_module_math()),
            "sass:meta" => Some(declare_module_meta()),
            "sass:selector" => Some(declare_module_selector()),
            "sass:string" => Some(declare_module_string()),
            _ => None,
        };

        if let Some(builtin) = builtin {
            // todo: lots of ugly unwraps here
            if configuration.is_some()
                && !(**configuration.as_ref().unwrap()).borrow().is_implicit()
            {
                let msg = if names_in_errors {
                    format!(
                        "Built-in module {} can't be configured.",
                        url.to_string_lossy()
                    )
                } else {
                    "Built-in modules can't be configured.".to_owned()
                };

                return Err((
                    msg,
                    (**configuration.as_ref().unwrap()).borrow().span.unwrap(),
                )
                    .into());
            }

            callback(
                self,
                Arc::new(RefCell::new(builtin)),
                StyleSheet::new(false, url.to_path_buf()),
            )?;
            return Ok(());
        }

        // todo: decide on naming convention for style_sheet vs stylesheet
        let stylesheet = self.load_style_sheet(url.to_string_lossy().as_ref(), false, span)?;

        let canonical_url = self
            .options
            .fs
            .canonicalize(&stylesheet.url)
            .unwrap_or_else(|_| stylesheet.url.clone());

        if self.active_modules.contains(&canonical_url) {
            return Err(("Module loop: this module is already being loaded.", span).into());
        }

        self.active_modules.insert(canonical_url.clone());

        let module = self.execute(stylesheet.clone(), configuration, names_in_errors)?;

        self.active_modules.remove(&canonical_url);

        callback(self, module, stylesheet)?;

        Ok(())
    }

    fn visit_use_rule(&mut self, use_rule: AstUseRule) -> SassResult<()> {
        let configuration = if use_rule.configuration.is_empty() {
            Rc::new(RefCell::new(Configuration::empty()))
        } else {
            let mut values = BTreeMap::new();

            for var in use_rule.configuration {
                let node = self.expression_node(&var.expr.node, var.expr.span);
                let value = self.visit_expr(var.expr.node)?;
                let value = self.without_slash(value);
                values.insert(
                    var.name.node,
                    ConfiguredValue::explicit(value, var.name.span.merge(var.expr.span))
                        .with_node(node),
                );
            }

            Rc::new(RefCell::new(Configuration::explicit(values, use_rule.span)))
        };

        let span = use_rule.span;

        let namespace = use_rule
            .namespace
            .as_ref()
            .map(|s| Identifier::from(s.trim_start_matches("sass:")));

        self.load_module(
            &use_rule.url,
            Some(Rc::clone(&configuration)),
            false,
            span,
            |visitor, module, _| {
                visitor.env.add_module(namespace, module, span)?;

                Ok(())
            },
        )?;

        Self::assert_configuration_is_empty(&configuration, false)?;

        Ok(())
    }

    pub(crate) fn assert_configuration_is_empty(
        config: &Rc<RefCell<Configuration>>,
        name_in_error: bool,
    ) -> SassResult<()> {
        let config = (**config).borrow();
        // By definition, implicit configurations are allowed to only use a subset
        // of their values.
        if config.is_empty() || config.is_implicit() {
            return Ok(());
        }

        let Spanned { node: name, span } = config.first().unwrap();

        let msg = if name_in_error {
            format!(
                "${name} was not declared with !default in the @used module.",
                name = name
            )
        } else {
            "This variable was not declared with !default in the @used module.".to_owned()
        };

        Err((msg, span).into())
    }

    fn visit_import_rule(&mut self, import_rule: AstImportRule) -> SassResult<Option<Value>> {
        for import in import_rule.imports {
            match import {
                AstImport::Sass(dynamic_import) => {
                    self.visit_dynamic_import_rule(&dynamic_import)?;
                }
                AstImport::Plain(static_import) => self.visit_static_import_rule(static_import)?,
            }
        }

        Ok(None)
    }

    /// Searches the current directory of the file then searches in `load_paths` directories
    /// if the import has not yet been found.
    ///
    /// <https://sass-lang.com/documentation/at-rules/import#finding-the-file>
    /// <https://sass-lang.com/documentation/at-rules/import#load-paths>
    #[allow(clippy::cognitive_complexity, clippy::redundant_clone)]
    pub fn find_import(&self, path: &Path) -> Option<PathBuf> {
        let path_buf = if path.is_absolute() {
            path.into()
        } else {
            self.current_import_path
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .join(path)
        };

        macro_rules! try_path {
            ($path:expr) => {
                let path = $path;
                let dirname = path.parent().unwrap_or_else(|| Path::new(""));
                let basename = path.file_name().unwrap_or_else(|| OsStr::new(".."));

                let partial = dirname.join(format!("_{}", basename.to_str().unwrap()));

                if self.options.fs.is_file(&path) {
                    return Some(path.to_path_buf());
                }

                if self.options.fs.is_file(&partial) {
                    return Some(partial);
                }
            };
        }

        if path_buf.extension() == Some(OsStr::new("scss"))
            || path_buf.extension() == Some(OsStr::new("sass"))
            || path_buf.extension() == Some(OsStr::new("css"))
        {
            let extension = path_buf.extension().unwrap();
            try_path!(path_buf.with_extension(format!(".import{}", extension.to_str().unwrap())));
            try_path!(path_buf);
            // todo: consider load paths
            return None;
        }

        macro_rules! try_path_with_extensions {
            ($path:expr) => {
                let path = $path;
                try_path!(path.with_extension("import.sass"));
                try_path!(path.with_extension("import.scss"));
                try_path!(path.with_extension("import.css"));
                try_path!(path.with_extension("sass"));
                try_path!(path.with_extension("scss"));
                try_path!(path.with_extension("css"));
            };
        }

        try_path_with_extensions!(path_buf.clone());

        if self.options.fs.is_dir(&path_buf) {
            try_path_with_extensions!(path_buf.join("index"));
        }

        for load_path in &self.options.load_paths {
            let path_buf = load_path.join(path);

            try_path_with_extensions!(&path_buf);

            if self.options.fs.is_dir(&path_buf) {
                try_path_with_extensions!(path_buf.join("index"));
            }
        }

        None
    }

    fn parse_file(
        &mut self,
        lexer: Lexer,
        path: &Path,
        empty_span: Span,
    ) -> SassResult<StyleSheet> {
        match InputSyntax::for_path(path) {
            InputSyntax::Scss => ScssParser::new(lexer, self.options, empty_span, path).__parse(),
            InputSyntax::Sass => SassParser::new(lexer, self.options, empty_span, path).__parse(),
            InputSyntax::Css => CssParser::new(lexer, self.options, empty_span, path).__parse(),
        }
    }

    fn import_like_node(
        &mut self,
        url: &str,
        for_import: bool,
        span: Span,
    ) -> SassResult<StyleSheet> {
        if let Some(importer) = self.options.importer {
            let base = self.current_import_path.to_string_lossy().into_owned();
            let canonical = match importer.canonicalize(url, &base, for_import) {
                Ok(Some(c)) => c,
                Ok(None) => return Err(("Can't find stylesheet to import.", span).into()),
                Err(e) => return Err((e, span).into()),
            };
            let name = PathBuf::from(&canonical);
            if let Some(style_sheet) = self.import_cache.get(&name) {
                return Ok(style_sheet.clone());
            }
            let loaded = importer.load(&canonical).map_err(|e| (e, span))?;
            let file = self.map.add_file(canonical.clone(), loaded.contents);

            let old_is_use_allowed = self.flags.is_use_allowed();
            self.flags.set(ContextFlags::IS_USE_ALLOWED, true);
            let lexer = Lexer::new_from_file(&file);
            let empty = file.span.subspan(0, 0);
            let style_sheet = match loaded.syntax {
                InputSyntax::Scss => ScssParser::new(lexer, self.options, empty, &name).__parse(),
                InputSyntax::Sass => SassParser::new(lexer, self.options, empty, &name).__parse(),
                InputSyntax::Css => CssParser::new(lexer, self.options, empty, &name).__parse(),
            }?;
            self.flags
                .set(ContextFlags::IS_USE_ALLOWED, old_is_use_allowed);
            // O `ImportCache` do dart-sass guarda toda folha carregada pela
            // URL canônica: a mesma folha não é relida nem reanalisada.
            self.import_cache.insert(name, style_sheet.clone());
            return Ok(style_sheet);
        }
        if let Some(name) = self.find_import(url.as_ref()) {
            let name = self.options.fs.canonicalize(&name).unwrap_or(name);
            if let Some(style_sheet) = self.import_cache.get(&name) {
                return Ok(style_sheet.clone());
            }

            let file = self.map.add_file(
                name.to_string_lossy().into(),
                String::from_utf8(self.options.fs.read(&name)?)?,
            );

            let old_is_use_allowed = self.flags.is_use_allowed();
            self.flags.set(ContextFlags::IS_USE_ALLOWED, true);

            let style_sheet =
                self.parse_file(Lexer::new_from_file(&file), &name, file.span.subspan(0, 0))?;

            self.flags
                .set(ContextFlags::IS_USE_ALLOWED, old_is_use_allowed);

            if self.files_seen.contains(&name) {
                self.import_cache.insert(name, style_sheet.clone());
            } else {
                self.files_seen.insert(name);
            }

            return Ok(style_sheet);
        }

        Err(("Can't find stylesheet to import.", span).into())
    }

    pub(crate) fn load_style_sheet(
        &mut self,
        url: &str,
        // default=false
        for_import: bool,
        span: Span,
    ) -> SassResult<StyleSheet> {
        // todo: import cache
        self.import_like_node(url, for_import, span)
    }

    fn visit_dynamic_import_rule(&mut self, dynamic_import: &AstSassImport) -> SassResult<()> {
        let stylesheet = self.load_style_sheet(&dynamic_import.url, true, dynamic_import.span)?;

        let url = stylesheet.url.clone();

        if self.active_modules.contains(&url) {
            return Err(("This file is already being loaded.", dynamic_import.span).into());
        }

        self.active_modules.insert(url.clone());

        // If the imported stylesheet doesn't use any modules, we can inject its
        // CSS directly into the current stylesheet. If it does use modules, we
        // need to put its CSS into an intermediate [ModifiableCssStylesheet] so
        // that we can hermetically resolve `@extend`s before injecting it.
        if stylesheet.uses.is_empty() && stylesheet.forwards.is_empty() {
            self.visit_stylesheet(stylesheet)?;
            return Ok(());
        }

        // todo:
        let loads_user_defined_modules = true;

        // this todo should be unreachable, as we currently do not push
        // to stylesheet.uses or stylesheet.forwards
        // let mut children = Vec::new();
        let env = self.env.for_import();

        self.with_environment::<SassResult<()>, _>(env.clone(), |visitor| {
            let old_parent = visitor.parent;
            let old_configuration = Rc::clone(&visitor.configuration);

            if loads_user_defined_modules {
                visitor.parent = Some(CssTree::ROOT);
            }

            // This configuration is only used if it passes through a `@forward`
            // rule, so we avoid creating unnecessary ones for performance reasons.
            if !stylesheet.forwards.is_empty() {
                visitor.configuration = Rc::new(RefCell::new(env.to_implicit_configuration()));
            }

            visitor.visit_stylesheet(stylesheet)?;

            if loads_user_defined_modules {
                visitor.parent = old_parent;
            }
            visitor.configuration = old_configuration;

            Ok(())
        })?;

        // Create a dummy module with empty CSS and no extensions to make forwarded
        // members available in the current import context and to combine all the
        // CSS from modules used by [stylesheet].
        let module = env.to_dummy_module(self.empty_span);
        self.env.import_forwards(module);

        if loads_user_defined_modules {
            // todo:
            //     if (module.transitivelyContainsCss) {
            //       // If any transitively used module contains extensions, we need to
            //       // clone all modules' CSS. Otherwise, it's possible that they'll be
            //       // used or imported from another location that shouldn't have the same
            //       // extensions applied.
            //       await _combineCss(module,
            //               clone: module.transitivelyContainsExtensions)
            //           .accept(this);
            //     }

            //     var visitor = _ImportedCssVisitor(this);
            //     for (var child in children) {
            //       child.accept(visitor);
            //     }
        }

        self.active_modules.remove(&url);

        Ok(())
    }

    fn visit_static_import_rule(&mut self, static_import: AstPlainCssImport) -> SassResult<()> {
        let import = self.interpolation_to_value(static_import.url, false, false)?;

        let modifiers = static_import
            .modifiers
            .map(|modifiers| self.interpolation_to_value(modifiers, false, false))
            .transpose()?;

        let node = CssStmt::Import(import, modifiers, static_import.span);

        if self.parent.is_some() && self.parent != Some(CssTree::ROOT) {
            self.copy_parent_after_sibling();
            self.css_tree.add_stmt(node, self.parent);
        } else {
            self.import_nodes.push(node);
        }

        Ok(())
    }

    fn visit_debug_rule(&mut self, debug_rule: AstDebugRule) -> SassResult<Option<Value>> {
        if self.options.quiet {
            return Ok(None);
        }

        let message = self.visit_expr(debug_rule.value)?;
        let message = message.inspect(debug_rule.span)?;

        let loc = self.map.look_up_span(debug_rule.span);
        self.options.logger.debug(loc, message.as_str());

        Ok(None)
    }

    fn visit_content_rule(&mut self, content_rule: AstContentRule) -> SassResult<Option<Value>> {
        let span = content_rule.args.span;
        if let Some(content) = &self.env.content {
            #[allow(mutable_borrow_reservation_conflict)]
            self.run_user_defined_callable(
                MaybeEvaledArguments::Invocation(content_rule.args),
                Arc::clone(content),
                &content.env.clone(),
                span,
                |content, visitor| {
                    for stmt in content.content.body.clone() {
                        let result = visitor.visit_stmt(stmt)?;
                        debug_assert!(result.is_none());
                    }

                    Ok(())
                },
            )?;
        }

        Ok(None)
    }

    fn trim_included(&self, nodes: &[CssTreeIdx]) -> CssTreeIdx {
        if nodes.is_empty() {
            return CssTree::ROOT;
        }

        let mut parent = self.parent;

        let mut innermost_contiguous: Option<usize> = None;

        for i in 0..nodes.len() {
            while parent != nodes.get(i).copied() {
                innermost_contiguous = None;

                let grandparent = self.css_tree.child_to_parent.get(&parent.unwrap()).copied();
                if grandparent.is_none() {
                    unreachable!(
                        "Expected {:?} to be an ancestor of {:?}.",
                        nodes[i], grandparent
                    )
                }
                parent = grandparent;
            }
            innermost_contiguous = innermost_contiguous.or(Some(i));

            let grandparent = self.css_tree.child_to_parent.get(&parent.unwrap()).copied();
            if grandparent.is_none() {
                unreachable!(
                    "Expected {:?} to be an ancestor of {:?}.",
                    nodes[i], grandparent
                )
            }
            parent = grandparent;
        }

        if parent != Some(CssTree::ROOT) {
            return CssTree::ROOT;
        }

        nodes[innermost_contiguous.unwrap()]
    }

    fn visit_at_root_rule(&mut self, mut at_root_rule: AstAtRootRule) -> SassResult<Option<Value>> {
        let query = match at_root_rule.query.clone() {
            Some(query) => {
                let resolved = self.perform_interpolation(query.node, true)?;

                let span = query.span;

                let query_toks = Lexer::new_from_string(&resolved, span);

                AtRootQueryParser::new(query_toks).parse()?
            }
            None => AtRootQuery::default(),
        };

        let mut current_parent_idx = self.parent;

        let mut included = Vec::new();

        while let Some(parent_idx) = current_parent_idx {
            let parent = self.css_tree.get(parent_idx);
            let grandparent_idx = match &*parent {
                Some(parent) => {
                    if !query.excludes(parent) {
                        included.push(parent_idx);
                    }
                    self.css_tree.child_to_parent.get(&parent_idx).copied()
                }
                None => break,
            };

            current_parent_idx = grandparent_idx;
        }

        let root = self.trim_included(&included);

        // If we didn't exclude any rules, we don't need to use the copies we might
        // have created.
        if Some(root) == self.parent {
            self.with_scope::<SassResult<()>, _>(false, true, |visitor| {
                for stmt in at_root_rule.body {
                    let result = visitor.visit_stmt(stmt)?;
                    debug_assert!(result.is_none());
                }

                Ok(())
            })?;
            return Ok(None);
        }

        let inner_copy = if !included.is_empty() {
            let inner_copy = self
                .css_tree
                .get(*included.first().unwrap())
                .as_ref()
                .map(CssStmt::copy_without_children);
            let mut outer_copy = self.css_tree.add_stmt(inner_copy.unwrap(), None);

            for node in &included[1..] {
                let copy = self
                    .css_tree
                    .get(*node)
                    .as_ref()
                    .map(CssStmt::copy_without_children)
                    .unwrap();

                let copy_idx = self.css_tree.add_stmt(copy, None);
                self.css_tree.link_child_to_parent(outer_copy, copy_idx);

                outer_copy = copy_idx;
            }

            Some(outer_copy)
        } else {
            let inner_copy = self
                .css_tree
                .get(root)
                .as_ref()
                .map(CssStmt::copy_without_children);
            inner_copy.map(|p| self.css_tree.add_stmt(p, None))
        };

        let body = mem::take(&mut at_root_rule.body);

        self.with_scope_for_at_root::<SassResult<()>, _>(inner_copy, &query, |visitor| {
            for stmt in body {
                let result = visitor.visit_stmt(stmt)?;
                debug_assert!(result.is_none());
            }

            Ok(())
        })?;

        Ok(None)
    }

    fn with_scope_for_at_root<T, F: FnOnce(&mut Self) -> T>(
        &mut self,
        new_parent_idx: Option<CssTreeIdx>,
        query: &AtRootQuery,
        callback: F,
    ) -> T {
        let old_parent = self.parent;
        self.parent = new_parent_idx;

        let old_at_root_excluding_style_rule = self.flags.at_root_excluding_style_rule();

        if query.excludes_style_rules() {
            self.flags
                .set(ContextFlags::AT_ROOT_EXCLUDING_STYLE_RULE, true);
        }

        let old_media_query_info = if self.media_queries.is_some() && query.excludes_name("media") {
            Some((self.media_queries.take(), self.media_query_sources.take()))
        } else {
            None
        };

        let was_in_keyframes = if self.flags.in_keyframes() && query.excludes_name("keyframes") {
            let was = self.flags.in_keyframes();
            self.flags.set(ContextFlags::IN_KEYFRAMES, false);
            was
        } else {
            self.flags.in_keyframes()
        };

        // todo:
        // if self.flags.in_unknown_at_rule() && !included.iter().any(|parent| parent is CssAtRule)

        let res = self.with_scope(false, true, callback);

        self.parent = old_parent;

        self.flags.set(
            ContextFlags::AT_ROOT_EXCLUDING_STYLE_RULE,
            old_at_root_excluding_style_rule,
        );

        if let Some((old_media_queries, old_media_query_sources)) = old_media_query_info {
            self.media_queries = old_media_queries;
            self.media_query_sources = old_media_query_sources;
        }

        self.flags.set(ContextFlags::IN_KEYFRAMES, was_in_keyframes);

        res
    }

    fn visit_function_decl(&mut self, fn_decl: AstFunctionDecl) {
        let name = fn_decl.name.node;
        // todo: independency

        let func = SassFunction::UserDefined(UserDefinedFunction {
            function: Arc::new(fn_decl),
            name,
            env: self.env.new_closure(),
        });

        self.env.insert_fn(func);
    }

    pub(crate) fn parse_selector_from_string(
        &mut self,
        selector_text: &str,
        allows_parent: bool,
        allows_placeholder: bool,
        span: Span,
    ) -> SassResult<SelectorList> {
        let sel_toks = Lexer::new_from_string(selector_text, span);

        SelectorParser::new(sel_toks, allows_parent, allows_placeholder, span).parse()
    }

    fn visit_extend_rule(&mut self, extend_rule: AstExtendRule) -> SassResult<Option<Value>> {
        if !self.style_rule_exists() || self.declaration_name.is_some() {
            return Err((
                "@extend may only be used within style rules.",
                extend_rule.span,
            )
                .into());
        }

        let super_selector = self.style_rule_ignoring_at_root.clone().unwrap();

        let target_text = self.interpolation_to_value(extend_rule.value, false, true)?;

        let list = self.parse_selector_from_string(&target_text, false, true, extend_rule.span)?;

        for complex in list.components {
            if complex.components.len() != 1 || !complex.components.first().unwrap().is_compound() {
                // If the selector was a compound selector but not a simple
                // selector, emit a more explicit error.
                return Err(("complex selectors may not be extended.", extend_rule.span).into());
            }

            let compound = match complex.components.first() {
                Some(ComplexSelectorComponent::Compound(c)) => c,
                Some(..) | None => unreachable!("checked by above condition"),
            };
            if compound.components.len() != 1 {
                return Err((
                    format!(
                        "compound selectors may no longer be extended.\nConsider `@extend {}` instead.\nSee http://bit.ly/ExtendCompound for details.\n",
                        compound.components.iter().map(ToString::to_string).collect::<Vec<String>>().join(", ")
                    )
                , extend_rule.span).into());
            }

            self.extender.add_extension(
                super_selector.clone().into_selector().0,
                compound.components.first().unwrap(),
                &ExtendRule {
                    is_optional: extend_rule.is_optional,
                },
                &self.media_queries,
                extend_rule.span,
            );
        }

        Ok(None)
    }

    fn visit_error_rule(&mut self, error_rule: AstErrorRule) -> SassResult<Box<SassError>> {
        let value = self
            .visit_expr(error_rule.value)?
            .inspect(error_rule.span)?;

        Ok((value, error_rule.span).into())
    }

    fn merge_media_queries(
        queries1: &[MediaQuery],
        queries2: &[MediaQuery],
    ) -> Option<Vec<MediaQuery>> {
        let mut queries = Vec::new();

        for query1 in queries1 {
            for query2 in queries2 {
                match query1.merge(query2) {
                    MediaQueryMergeResult::Empty => continue,
                    MediaQueryMergeResult::Unrepresentable => return None,
                    MediaQueryMergeResult::Success(result) => queries.push(result),
                }
            }
        }

        Some(queries)
    }

    fn visit_media_queries(
        &mut self,
        queries: Interpolation,
        span: Span,
    ) -> SassResult<Vec<CssMediaQuery>> {
        let resolved = self.perform_interpolation(queries, true)?;

        CssMediaQuery::parse_list(&resolved, span)
    }

    fn visit_media_rule(&mut self, media_rule: AstMedia) -> SassResult<Option<Value>> {
        if self.declaration_name.is_some() {
            return Err((
                "Media rules may not be used within nested declarations.",
                media_rule.span,
            )
                .into());
        }

        let queries1 = self.visit_media_queries(media_rule.query, media_rule.query_span)?;
        // todo: superfluous clone?
        let queries2 = self.media_queries.clone();
        let merged_queries = queries2
            .as_ref()
            .and_then(|queries2| Self::merge_media_queries(queries2, &queries1));

        let merged_sources = match &merged_queries {
            Some(merged_queries) if merged_queries.is_empty() => return Ok(None),
            Some(..) => {
                let mut set = IndexSet::new();
                set.extend(self.media_query_sources.clone().unwrap());
                set.extend(self.media_queries.clone().unwrap());
                set.extend(queries1.clone());
                set
            }
            None => IndexSet::new(),
        };

        let children = media_rule.body;

        let query = merged_queries.clone().unwrap_or_else(|| queries1.clone());

        let media_rule = CssStmt::Media(
            MediaRule {
                query,
                body: Vec::new(),
                span: media_rule.span,
            },
            false,
        );

        self.with_parent(
            media_rule,
            true,
            |visitor| {
                visitor.with_media_queries(
                    Some(merged_queries.unwrap_or(queries1)),
                    Some(merged_sources.clone()),
                    |visitor| {
                        if !visitor.style_rule_exists() {
                            for stmt in children {
                                let result = visitor.visit_stmt(stmt)?;
                                debug_assert!(result.is_none());
                            }
                        } else {
                            // If we're in a style rule, copy it into the media query so that
                            // declarations immediately inside @media have somewhere to go.
                            //
                            // For example, "a {@media screen {b: c}}" should produce
                            // "@media screen {a {b: c}}".
                            let selector = visitor.style_rule_ignoring_at_root.clone().unwrap();
                            let (selector_span, span) = visitor.style_rule_spans;
                            let ruleset = CssStmt::RuleSet {
                                selector,
                                body: Vec::new(),
                                is_group_end: false,
                                selector_span,
                                span,
                            };

                            visitor.with_parent(
                                ruleset,
                                false,
                                |visitor| {
                                    for stmt in children {
                                        let result = visitor.visit_stmt(stmt)?;
                                        debug_assert!(result.is_none());
                                    }

                                    Ok(())
                                },
                                |_| false,
                            )?;
                        }

                        Ok(())
                    },
                )
            },
            |stmt| match stmt {
                CssStmt::RuleSet { .. } => true,
                // todo: node.queries.every(mergedSources.contains))
                CssStmt::Media(media_rule, ..) => {
                    !merged_sources.is_empty()
                        && media_rule
                            .query
                            .iter()
                            .all(|query| merged_sources.contains(query))
                }
                _ => false,
            },
        )?;

        Ok(None)
    }

    fn visit_unknown_at_rule(
        &mut self,
        unknown_at_rule: AstUnknownAtRule,
    ) -> SassResult<Option<Value>> {
        if self.declaration_name.is_some() {
            return Err((
                "At-rules may not be used within nested declarations.",
                unknown_at_rule.span,
            )
                .into());
        }

        let name = self.interpolation_to_value(unknown_at_rule.name, false, false)?;

        let value = unknown_at_rule
            .value
            .map(|v| self.interpolation_to_value(v, true, true))
            .transpose()?;

        if unknown_at_rule.body.is_none() {
            let stmt = CssStmt::UnknownAtRule(
                UnknownAtRule {
                    name,
                    params: value.unwrap_or_default(),
                    body: Vec::new(),
                    has_body: false,
                    span: unknown_at_rule.span,
                },
                false,
            );

            self.copy_parent_after_sibling();
            self.css_tree.add_stmt(stmt, self.parent);

            return Ok(None);
        }

        let was_in_keyframes = self.flags.in_keyframes();
        let was_in_unknown_at_rule = self.flags.in_unknown_at_rule();

        if unvendor(&name) == "keyframes" {
            self.flags.set(ContextFlags::IN_KEYFRAMES, true);
        } else {
            self.flags.set(ContextFlags::IN_UNKNOWN_AT_RULE, true);
        }

        let at_rule_span = unknown_at_rule.span;
        let children = unknown_at_rule.body.unwrap();

        let stmt = CssStmt::UnknownAtRule(
            UnknownAtRule {
                name,
                params: value.unwrap_or_default(),
                body: Vec::new(),
                has_body: true,
                span: at_rule_span,
            },
            false,
        );

        self.with_parent(
            stmt,
            true,
            |visitor| {
                if !visitor.style_rule_exists() || visitor.flags.in_keyframes() {
                    for stmt in children {
                        let result = visitor.visit_stmt(stmt)?;
                        debug_assert!(result.is_none());
                    }
                } else {
                    // If we're in a style rule, copy it into the at-rule so that
                    // declarations immediately inside it have somewhere to go.
                    //
                    // For example, "a {@foo {b: c}}" should produce "@foo {a {b: c}}".
                    let selector = visitor.style_rule_ignoring_at_root.clone().unwrap();
                    let (selector_span, span) = visitor.style_rule_spans;

                    let style_rule = CssStmt::RuleSet {
                        selector,
                        body: Vec::new(),
                        is_group_end: false,
                        selector_span,
                        span,
                    };

                    visitor.with_parent(
                        style_rule,
                        false,
                        |visitor| {
                            for stmt in children {
                                let result = visitor.visit_stmt(stmt)?;
                                debug_assert!(result.is_none());
                            }

                            Ok(())
                        },
                        |_| false,
                    )?;
                }

                Ok(())
            },
            CssStmt::is_style_rule,
        )?;

        self.flags.set(ContextFlags::IN_KEYFRAMES, was_in_keyframes);
        self.flags
            .set(ContextFlags::IN_UNKNOWN_AT_RULE, was_in_unknown_at_rule);

        Ok(None)
    }

    pub(crate) fn emit_warning(&mut self, message: &str, span: Span) {
        if self.options.quiet {
            return;
        }
        let loc = self.map.look_up_span(span);
        self.options.logger.warn(loc, message);
    }

    fn visit_warn_rule(&mut self, warn_rule: AstWarn) -> SassResult<()> {
        if self.warnings_emitted.insert(warn_rule.span) {
            let value = self.visit_expr(warn_rule.value)?;
            let message = value.to_css_string(warn_rule.span, self.options.is_compressed())?;
            self.emit_warning(&message, warn_rule.span);
        }

        Ok(())
    }

    fn with_media_queries<T>(
        &mut self,
        queries: Option<Vec<MediaQuery>>,
        sources: Option<IndexSet<MediaQuery>>,
        callback: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let old_media_queries = self.media_queries.take();
        let old_media_query_sources = self.media_query_sources.take();
        self.media_queries = queries;
        self.media_query_sources = sources;
        let result = callback(self);
        self.media_queries = old_media_queries;
        self.media_query_sources = old_media_query_sources;
        result
    }

    fn with_environment<T, F: FnOnce(&mut Self) -> T>(
        &mut self,
        env: Environment,
        callback: F,
    ) -> T {
        let mut old_env = env;
        mem::swap(&mut self.env, &mut old_env);
        let val = callback(self);
        mem::swap(&mut self.env, &mut old_env);
        val
    }

    fn add_child<F: Fn(&CssStmt) -> bool>(
        &mut self,
        node: CssStmt,
        through: Option<F>,
    ) -> CssTreeIdx {
        if self.parent.is_none() || self.parent == Some(CssTree::ROOT) {
            return self.css_tree.add_stmt(node, self.parent);
        }

        let mut parent = self.parent.unwrap();

        if let Some(through) = through {
            while parent != CssTree::ROOT && through(self.css_tree.get(parent).as_ref().unwrap()) {
                let grandparent = self.css_tree.child_to_parent.get(&parent).copied();
                debug_assert!(
                    grandparent.is_some(),
                    "through() must return false for at least one parent of $node."
                );
                parent = grandparent.unwrap();
            }

            // If the parent has a (visible) following sibling, we shouldn't add to
            // the parent. Instead, we should create a copy and add it after the
            // interstitial sibling.
            if self.css_tree.has_following_sibling(parent) {
                let grandparent = self.css_tree.child_to_parent.get(&parent).copied().unwrap();
                // Se já há uma cópia do pai como último filho do avô (e nada
                // depois dela), o dart-sass a reaproveita
                // (`equalsIgnoringChildren`).
                let ultimo = self.css_tree.children_of(grandparent).last().copied();
                let reaproveita = ultimo.filter(|&u| {
                    let a = self.css_tree.get(parent);
                    let b = self.css_tree.get(u);
                    matches!((a.as_ref(), b.as_ref()), (Some(a), Some(b)) if a.equals_ignoring_children(b))
                });
                parent = match reaproveita {
                    Some(u) => u,
                    None => {
                        let parent_node = self
                            .css_tree
                            .get(parent)
                            .as_ref()
                            .map(CssStmt::copy_without_children)
                            .unwrap();
                        self.css_tree.add_child(parent_node, grandparent)
                    }
                };
            }
        }

        self.css_tree.add_child(node, parent)
    }

    /// `_copyParentAfterSibling`: se o pai corrente não é o último filho do
    /// avô (uma regra aninhada já saiu depois dele), o que vier agora vai
    /// para uma cópia vazia do pai, posta depois — a ordem do dart-sass.
    fn copy_parent_after_sibling(&mut self) {
        let Some(parent) = self.parent else { return };
        if parent == CssTree::ROOT {
            return;
        }
        let Some(&grandparent) = self.css_tree.child_to_parent.get(&parent) else {
            return;
        };
        if self.css_tree.children_of(grandparent).last() == Some(&parent) {
            return;
        }
        let copia = self
            .css_tree
            .get(parent)
            .as_ref()
            .map(CssStmt::copy_without_children);
        if let Some(copia) = copia {
            self.parent = Some(self.css_tree.add_child(copia, grandparent));
        }
    }

    fn with_parent<F: FnOnce(&mut Self) -> SassResult<()>, FT: Fn(&CssStmt) -> bool>(
        &mut self,
        parent: CssStmt,
        // default=true
        scope_when: bool,
        callback: F,
        // todo: optional
        through: FT,
    ) -> SassResult<()> {
        let parent_idx = self.add_child(parent, Some(through));
        let old_parent = self.parent;
        self.parent = Some(parent_idx);
        let result = self.with_scope(false, scope_when, callback);
        self.parent = old_parent;
        result
    }

    fn with_scope<T, F: FnOnce(&mut Self) -> T>(
        &mut self,
        // default=false
        semi_global: bool,
        // default=true
        when: bool,
        callback: F,
    ) -> T {
        let semi_global = semi_global && self.flags.in_semi_global_scope();
        let was_in_semi_global_scope = self.flags.in_semi_global_scope();
        self.flags
            .set(ContextFlags::IN_SEMI_GLOBAL_SCOPE, semi_global);

        if !when {
            let v = callback(self);
            self.flags
                .set(ContextFlags::IN_SEMI_GLOBAL_SCOPE, was_in_semi_global_scope);

            return v;
        }

        self.env.scopes_mut().enter_new_scope();

        let v = callback(self);

        self.flags
            .set(ContextFlags::IN_SEMI_GLOBAL_SCOPE, was_in_semi_global_scope);
        self.env.scopes_mut().exit_scope();

        v
    }

    fn with_content<T>(
        &mut self,
        content: Option<Arc<CallableContentBlock>>,
        callback: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let old_content = self.env.content.take();
        self.env.content = content;
        let v = callback(self);
        self.env.content = old_content;
        v
    }

    fn visit_include_stmt(&mut self, include_stmt: AstInclude) -> SassResult<Option<Value>> {
        let mixin = self
            .env
            .get_mixin(include_stmt.name, include_stmt.namespace)?;

        match mixin {
            Mixin::Builtin(mixin) => {
                if include_stmt.content.is_some() {
                    return Err(("Mixin doesn't accept a content block.", include_stmt.span).into());
                }

                let args = self.eval_args(include_stmt.args, include_stmt.name.span)?;
                mixin(args, self)?;

                Ok(None)
            }
            Mixin::UserDefined(mixin, env) => {
                if include_stmt.content.is_some() && !mixin.has_content {
                    return Err(("Mixin doesn't accept a content block.", include_stmt.span).into());
                }

                let AstInclude { args, content, .. } = include_stmt;

                let old_in_mixin = self.flags.in_mixin();
                self.flags.set(ContextFlags::IN_MIXIN, true);

                let callable_content = content.map(|c| {
                    Arc::new(CallableContentBlock {
                        content: c,
                        env: self.env.new_closure(),
                    })
                });

                self.run_user_defined_callable::<_, (), _>(
                    MaybeEvaledArguments::Invocation(args),
                    mixin,
                    &env,
                    include_stmt.name.span,
                    |mixin, visitor| {
                        visitor.with_content(callable_content, |visitor| {
                            for stmt in mixin.body {
                                let result = visitor.visit_stmt(stmt)?;
                                debug_assert!(result.is_none());
                            }
                            Ok(())
                        })
                    },
                )?;

                self.flags.set(ContextFlags::IN_MIXIN, old_in_mixin);

                Ok(None)
            }
        }
    }

    fn visit_mixin_decl(&mut self, mixin: AstMixin) {
        self.env.insert_mixin(
            mixin.name,
            Mixin::UserDefined(mixin, self.env.new_closure()),
        );
    }

    fn visit_each_stmt(&mut self, each_stmt: AstEach) -> SassResult<Option<Value>> {
        let node = Some(self.expression_node(&each_stmt.list, each_stmt.list_span));
        let list = self.visit_expr(each_stmt.list)?.as_list();

        // todo: not setting semi_global: true maybe means we can't assign to global scope when declared as global
        self.env.scopes_mut().enter_new_scope();

        let mut result = None;

        'outer: for val in list {
            if each_stmt.variables.len() == 1 {
                let val = self.without_slash(val);
                self.env
                    .scopes_mut()
                    .insert_var_last(each_stmt.variables[0], val, node);
            } else {
                for (&var, val) in each_stmt.variables.iter().zip(
                    val.as_list()
                        .into_iter()
                        .chain(std::iter::once(Value::Null).cycle()),
                ) {
                    let val = self.without_slash(val);
                    self.env.scopes_mut().insert_var_last(var, val, node);
                }
            }

            for stmt in each_stmt.body.clone() {
                let val = self.visit_stmt(stmt)?;
                if val.is_some() {
                    result = val;
                    break 'outer;
                }
            }
        }

        self.env.scopes_mut().exit_scope();

        Ok(result)
    }

    fn visit_for_stmt(&mut self, for_stmt: AstFor) -> SassResult<Option<Value>> {
        let from_span = for_stmt.from.span;
        let for_node = Some(self.expression_node(&for_stmt.from.node, from_span));
        let to_span = for_stmt.to.span;
        let from_number = self
            .visit_expr(for_stmt.from.node)?
            .assert_number(from_span)?;
        let to_number = self.visit_expr(for_stmt.to.node)?.assert_number(to_span)?;

        if !to_number.unit().comparable(from_number.unit()) {
            // todo: better error message here
            return Err((
                "to and from values have incompatible units",
                from_span.merge(to_span),
            )
                .into());
        }

        let from = from_number.num.assert_int(from_span)?;
        let mut to = to_number
            .num
            .convert(to_number.unit(), from_number.unit())
            .assert_int(to_span)?;

        let direction = if from > to { -1 } else { 1 };

        if to == i64::MAX || to == i64::MIN {
            return Err((
                "@for loop upper bound exceeds valid integer representation (i64::MAX)",
                to_span,
            )
                .into());
        }

        if !for_stmt.is_exclusive {
            to += direction;
        }

        if from == to {
            return Ok(None);
        }

        // todo: self.with_scopes
        self.env.scopes_mut().enter_new_scope();

        let mut result = None;

        let mut i = from;
        'outer: while i != to {
            self.env.scopes_mut().insert_var_last(
                for_stmt.variable.node,
                Value::Dimension(SassNumber {
                    num: Number::from(i),
                    unit: from_number.unit().clone(),
                    as_slash: None,
                }),
                for_node,
            );

            for stmt in for_stmt.body.clone() {
                let val = self.visit_stmt(stmt)?;
                if val.is_some() {
                    result = val;
                    break 'outer;
                }
            }

            i += direction;
        }

        self.env.scopes_mut().exit_scope();

        Ok(result)
    }

    fn visit_while_stmt(&mut self, while_stmt: &AstWhile) -> SassResult<Option<Value>> {
        self.with_scope(true, true, |visitor| {
            let mut result = None;

            'outer: while visitor
                .visit_expr(while_stmt.condition.clone())?
                .is_truthy()
            {
                for stmt in while_stmt.body.clone() {
                    let val = visitor.visit_stmt(stmt)?;
                    if val.is_some() {
                        result = val;
                        break 'outer;
                    }
                }
            }

            Ok(result)
        })
    }

    fn visit_if_stmt(&mut self, if_stmt: AstIf) -> SassResult<Option<Value>> {
        let mut clause: Option<Vec<AstStmt>> = if_stmt.else_clause;
        for clause_to_check in if_stmt.if_clauses {
            if self.visit_expr(clause_to_check.condition)?.is_truthy() {
                clause = Some(clause_to_check.body);
                break;
            }
        }

        // todo: self.with_scope
        self.env.scopes_mut().enter_new_scope();

        let mut result = None;

        if let Some(stmts) = clause {
            for stmt in stmts {
                let val = self.visit_stmt(stmt)?;
                if val.is_some() {
                    result = val;
                    break;
                }
            }
        }

        self.env.scopes_mut().exit_scope();

        Ok(result)
    }

    fn visit_loud_comment(&mut self, comment: AstLoudComment) -> SassResult<Option<Value>> {
        if self.flags.in_function() {
            return Ok(None);
        }

        // todo: Comments are allowed to appear between CSS imports
        // if (_parent == _root && _endOfImports == _root.children.length) {
        //   _endOfImports++;
        // }

        let comment = CssStmt::Comment(
            self.perform_interpolation(comment.text, false)?,
            comment.span,
        );
        self.copy_parent_after_sibling();
        self.css_tree.add_stmt(comment, self.parent);

        Ok(None)
    }

    /// `_expressionNode`: o `span` que representa `expr` no mapa de fontes —
    /// o nó da variável, se `expr` for uma variável com nó conhecido.
    fn expression_node(&self, expr: &AstExpr, fallback: Span) -> Span {
        match expr {
            AstExpr::Variable { name, namespace } => self
                .env
                .get_var_node(name.node, namespace.map(|n| n.node))
                .unwrap_or(fallback),
            _ => fallback,
        }
    }

    fn visit_variable_decl(&mut self, decl: AstVariableDecl) -> SassResult<Option<Value>> {
        let name = Spanned {
            node: decl.name,
            span: decl.span,
        };

        if decl.is_guarded {
            if decl.namespace.is_none() && self.env.at_root() {
                let var_override = (*self.configuration).borrow_mut().remove(decl.name);
                if !matches!(
                    var_override,
                    Some(ConfiguredValue {
                        value: Value::Null,
                        ..
                    }) | None
                ) {
                    let var_override = var_override.unwrap();
                    self.env.insert_var(
                        name,
                        None,
                        var_override.value,
                        true,
                        self.flags.in_semi_global_scope(),
                        var_override.node,
                    )?;
                    return Ok(None);
                }
            }

            if self.env.var_exists(decl.name, decl.namespace)? {
                let value = self.env.get_var(name, decl.namespace).unwrap();

                if value != Value::Null {
                    return Ok(None);
                }
            }
        }

        // `_expressionNode(node.expression)`: com o valor vindo de outra
        // variável, o nó é o dela.
        let node = self.expression_node(&decl.value, decl.value_span);
        let value = self.visit_expr(decl.value)?;
        let value = self.without_slash(value);

        self.env.insert_var(
            name,
            decl.namespace,
            value,
            decl.is_global,
            self.flags.in_semi_global_scope(),
            Some(node),
        )?;

        Ok(None)
    }

    fn interpolation_to_value(
        &mut self,
        interpolation: Interpolation,
        // default=false
        trim: bool,
        // default=false
        warn_for_color: bool,
    ) -> SassResult<String> {
        let result = self.perform_interpolation(interpolation, warn_for_color)?;

        Ok(if trim {
            trim_ascii(&result, true).to_owned()
        } else {
            result
        })
    }

    fn perform_interpolation(
        &mut self,
        mut interpolation: Interpolation,
        // todo check to emit warning if this is true
        _warn_for_color: bool,
    ) -> SassResult<String> {
        let result = match interpolation.contents.len() {
            0 => String::new(),
            1 => match interpolation.contents.pop() {
                Some(InterpolationPart::String(s)) => s,
                Some(InterpolationPart::Expr(e)) => {
                    let span = e.span;
                    let result = self.visit_expr(e.node)?;
                    // todo: span for specific expr
                    self.serialize(result, QuoteKind::None, span)?
                }
                None => unreachable!(),
            },
            _ => interpolation
                .contents
                .into_iter()
                .map(|part| match part {
                    InterpolationPart::String(s) => Ok(s),
                    InterpolationPart::Expr(e) => {
                        let span = e.span;
                        let result = self.visit_expr(e.node)?;
                        // todo: span for specific expr
                        self.serialize(result, QuoteKind::None, span)
                    }
                })
                .collect::<SassResult<String>>()?,
        };

        Ok(result)
    }

    fn evaluate_to_css(
        &mut self,
        expr: AstExpr,
        quote: QuoteKind,
        span: Span,
    ) -> SassResult<String> {
        let result = self.visit_expr(expr)?;
        self.serialize(result, quote, span)
    }

    #[allow(clippy::unused_self)]
    fn without_slash(&mut self, v: Value) -> Value {
        match v {
            Value::Dimension(SassNumber { .. }) if v.as_slash().is_some() => {
                // todo: emit warning. we don't currently because it can be quite loud
                // self.emit_warning(
                //     Cow::Borrowed("Using / for division is deprecated and will be removed at some point in the future"),
                //     self.empty_span,
                // );
            }
            _ => {}
        }

        v.without_slash()
    }

    fn eval_maybe_args(
        &mut self,
        args: MaybeEvaledArguments,
        span: Span,
    ) -> SassResult<ArgumentResult> {
        match args {
            MaybeEvaledArguments::Invocation(args) => self.eval_args(args, span),
            MaybeEvaledArguments::Evaled(args) => Ok(args),
        }
    }

    fn eval_args(
        &mut self,
        arguments: ArgumentInvocation,
        span: Span,
    ) -> SassResult<ArgumentResult> {
        let mut positional = Vec::with_capacity(arguments.positional.len());
        let mut nodes = crate::ast::ArgNodes::default();

        for (i, expr) in arguments.positional.into_iter().enumerate() {
            nodes.positional.push(
                arguments
                    .positional_spans
                    .get(i)
                    .map(|&s| self.expression_node(&expr, s)),
            );
            let val = self.visit_expr(expr)?;
            positional.push(self.without_slash(val));
        }

        let mut named = BTreeMap::new();

        for (key, expr) in arguments.named {
            if let Some(&s) = arguments.named_spans.get(&key) {
                nodes.named.insert(key, self.expression_node(&expr, s));
            }
            let val = self.visit_expr(expr)?;
            named.insert(key, self.without_slash(val));
        }

        if arguments.rest.is_none() {
            return Ok(ArgumentResult {
                positional,
                named,
                separator: ListSeparator::Undecided,
                span,
                touched: BTreeSet::new(),
                nodes: nodes.clone(),
            });
        }

        let rest = self.visit_expr(arguments.rest.unwrap())?;

        let mut separator = ListSeparator::Undecided;

        match rest {
            Value::Map(rest) => self.add_rest_map(&mut named, rest)?,
            Value::List(elems, list_separator, _) => {
                let mut list = elems
                    .into_iter()
                    .map(|e| self.without_slash(e))
                    .collect::<Vec<_>>();
                positional.append(&mut list);
                separator = list_separator;
            }
            Value::ArgList(arglist) => {
                // todo: superfluous clone
                for (&key, value) in arglist.keywords() {
                    named.insert(key, self.without_slash(value.clone()));
                }

                let mut list = arglist
                    .elems
                    .into_iter()
                    .map(|e| self.without_slash(e))
                    .collect::<Vec<_>>();
                positional.append(&mut list);
                separator = arglist.separator;
            }
            _ => {
                positional.push(self.without_slash(rest));
            }
        }

        if arguments.keyword_rest.is_none() {
            return Ok(ArgumentResult {
                positional,
                named,
                separator: ListSeparator::Undecided,
                span: arguments.span,
                touched: BTreeSet::new(),
                nodes: nodes.clone(),
            });
        }

        match self.visit_expr(arguments.keyword_rest.unwrap())? {
            Value::Map(keyword_rest) => {
                self.add_rest_map(&mut named, keyword_rest)?;

                Ok(ArgumentResult {
                    positional,
                    named,
                    separator,
                    span: arguments.span,
                    touched: BTreeSet::new(),
                    nodes,
                })
            }
            v => Err((
                format!(
                    "Variable keyword arguments must be a map (was {}).",
                    v.inspect(arguments.span)?
                ),
                arguments.span,
            )
                .into()),
        }
    }

    fn add_rest_map(
        &mut self,
        named: &mut BTreeMap<Identifier, Value>,
        rest: SassMap,
    ) -> SassResult<()> {
        for (key, val) in rest {
            match key.node {
                Value::String(text, ..) => {
                    let val = self.without_slash(val);
                    named.insert(Identifier::from(text), val);
                }
                _ => {
                    return Err((
                        // todo: we have to render the map for this error message
                        "Variable keyword argument map must have string keys.",
                        key.span,
                    )
                        .into());
                }
            }
        }

        Ok(())
    }

    fn run_user_defined_callable<
        F: UserDefinedCallable,
        V: fmt::Debug,
        R: FnOnce(F, &mut Self) -> SassResult<V>,
    >(
        &mut self,
        arguments: MaybeEvaledArguments,
        func: F,
        env: &Environment,
        span: Span,
        run: R,
    ) -> SassResult<V> {
        let mut evaluated = self.eval_maybe_args(arguments, span)?;

        let mut name = func.name().to_string();

        if name != "@content" {
            name.push_str("()");
        }

        self.with_environment(env.new_closure(), |visitor| {
            visitor.with_scope(false, true, move |visitor| {
                func.arguments().verify(
                    evaluated.positional.len(),
                    &evaluated.named,
                    evaluated.span,
                )?;

                let declared_arguments = &func.arguments().args;
                let min_len = evaluated.positional.len().min(declared_arguments.len());

                let positional_len = evaluated.positional.len();

                #[allow(clippy::needless_range_loop)]
                for i in (0..min_len).rev() {
                    let node = evaluated.nodes.positional.get(i).copied().flatten();
                    visitor.env.scopes_mut().insert_var_last(
                        declared_arguments[i].name,
                        evaluated.positional.remove(i),
                        node,
                    );
                }

                // todo: better name for var
                let additional_declared_args = if declared_arguments.len() > positional_len {
                    &declared_arguments[positional_len..declared_arguments.len()]
                } else {
                    &[]
                };

                for argument in additional_declared_args {
                    let name = argument.name;
                    let node = evaluated.nodes.named.get(&argument.name).copied();
                    let value = evaluated.named.remove(&argument.name).map_or_else(
                        || {
                            // todo: superfluous clone
                            let v = visitor.visit_expr(argument.default.clone().unwrap())?;
                            Ok(visitor.without_slash(v))
                        },
                        SassResult::Ok,
                    )?;
                    visitor.env.scopes_mut().insert_var_last(name, value, node);
                }

                let were_keywords_accessed = Rc::new(Cell::new(false));

                let num_named_args = evaluated.named.len();

                let has_arg_list = if let Some(rest_arg) = func.arguments().rest {
                    let rest = if !evaluated.positional.is_empty() {
                        evaluated.positional
                    } else {
                        Vec::new()
                    };

                    let arg_list = Value::ArgList(ArgList::new(
                        rest,
                        Rc::clone(&were_keywords_accessed),
                        // todo: superfluous clone
                        evaluated.named.clone(),
                        if evaluated.separator == ListSeparator::Undecided {
                            ListSeparator::Comma
                        } else {
                            ListSeparator::Space
                        },
                    ));

                    visitor
                        .env
                        .scopes_mut()
                        .insert_var_last(rest_arg, arg_list, None);

                    true
                } else {
                    false
                };

                let val = run(func, visitor)?;

                if !has_arg_list || num_named_args == 0 {
                    return Ok(val);
                }

                if (*were_keywords_accessed).get() {
                    return Ok(val);
                }

                let argument_word = if num_named_args == 1 {
                    "argument"
                } else {
                    "arguments"
                };

                let argument_names = to_sentence(
                    evaluated
                        .named
                        .keys()
                        .map(|key| format!("${key}", key = key))
                        .collect(),
                    "or",
                );

                Err((
                    format!(
                        "No {argument_word} named {argument_names}.",
                        argument_word = argument_word,
                        argument_names = argument_names
                    ),
                    span,
                )
                    .into())
            })
        })
    }

    pub(crate) fn run_function_callable(
        &mut self,
        func: SassFunction,
        arguments: ArgumentInvocation,
        span: Span,
    ) -> SassResult<Value> {
        self.run_function_callable_with_maybe_evaled(
            func,
            MaybeEvaledArguments::Invocation(arguments),
            span,
        )
    }

    pub(crate) fn run_function_callable_with_maybe_evaled(
        &mut self,
        func: SassFunction,
        arguments: MaybeEvaledArguments,
        span: Span,
    ) -> SassResult<Value> {
        match func {
            SassFunction::Builtin(func, _name) => {
                let evaluated = self.eval_maybe_args(arguments, span)?;
                let val = func.0(evaluated, self)?;
                Ok(self.without_slash(val))
            }
            SassFunction::UserDefined(UserDefinedFunction { function, env, .. }) => self
                .run_user_defined_callable(arguments, function, &env, span, |function, visitor| {
                    for stmt in function.body.clone() {
                        let result = visitor.visit_stmt(stmt)?;

                        if let Some(val) = result {
                            return Ok(val);
                        }
                    }

                    Err(("Function finished without @return.", span).into())
                }),
            SassFunction::Plain { name } => {
                let has_named;
                let mut rest = None;

                // todo: somewhat hacky solution to support plain css fns passed
                // as strings to `call(..)`
                let arguments = match arguments {
                    MaybeEvaledArguments::Invocation(args) => {
                        has_named = !args.named.is_empty() || args.keyword_rest.is_some();
                        rest = args.rest;
                        args.positional
                            .into_iter()
                            .map(|arg| self.evaluate_to_css(arg, QuoteKind::Quoted, span))
                            .collect::<SassResult<Vec<_>>>()?
                    }
                    MaybeEvaledArguments::Evaled(args) => {
                        has_named = !args.named.is_empty();

                        args.positional
                            .into_iter()
                            .map(|arg| arg.to_css_string(span, self.options.is_compressed()))
                            .collect::<SassResult<Vec<_>>>()?
                    }
                };

                if has_named {
                    return Err(
                        ("Plain CSS functions don't support keyword arguments.", span).into(),
                    );
                }

                let mut buffer = format!("{}(", name.as_str());
                let mut first = true;

                for argument in arguments {
                    if first {
                        first = false;
                    } else {
                        buffer.push_str(", ");
                    }

                    buffer.push_str(&argument);
                }

                if let Some(rest_arg) = rest {
                    let rest = self.visit_expr(rest_arg)?;
                    if !first {
                        buffer.push_str(", ");
                    }
                    buffer.push_str(&self.serialize(rest, QuoteKind::Quoted, span)?);
                }
                buffer.push(')');

                Ok(Value::String(buffer, QuoteKind::None))
            }
        }
    }

    fn visit_list_expr(&mut self, list: ListExpr) -> SassResult<Value> {
        let elems = list
            .elems
            .into_iter()
            .map(|e| {
                let value = self.visit_expr(e.node)?;
                Ok(value)
            })
            .collect::<SassResult<Vec<_>>>()?;

        Ok(Value::List(elems, list.separator, list.brackets))
    }

    fn visit_function_call_expr(&mut self, func_call: FunctionCallExpr) -> SassResult<Value> {
        let name = func_call.name;

        let func = match self.env.get_fn(name, func_call.namespace)? {
            Some(func) => func,
            None => {
                // dart-sass `visitFunctionExpression`: sem função com o nome, as
                // funções de cálculo do CSS viram cálculos (`min`, `max`,
                // `round` e `abs` só com argumentos seguros).
                if func_call.namespace.is_none() && !self.is_plain_css {
                    let lower = name.as_str().to_ascii_lowercase();
                    let args = &func_call.arguments;
                    match lower.as_str() {
                        "min" | "max" | "round" | "abs"
                            if args.named.is_empty()
                                && args.rest.is_none()
                                && args.positional.iter().all(Self::is_calculation_safe) =>
                        {
                            return self.visit_calculation_expr(
                                &lower,
                                (*func_call.arguments).clone(),
                                true,
                                func_call.span,
                            );
                        }
                        "calc" | "clamp" | "hypot" | "sin" | "cos" | "tan" | "asin" | "acos"
                        | "atan" | "sqrt" | "exp" | "sign" | "mod" | "rem" | "atan2" | "pow"
                        | "log" | "calc-size" => {
                            return self.visit_calculation_expr(
                                &lower,
                                (*func_call.arguments).clone(),
                                false,
                                func_call.span,
                            );
                        }
                        _ => {}
                    }
                }
                if let Some(f) = self.options.custom_fns.get(name.as_str()) {
                    SassFunction::Builtin(f.clone(), name)
                } else if let Some(f) = GLOBAL_FUNCTIONS.get(name.as_str()) {
                    SassFunction::Builtin(f.clone(), name)
                } else {
                    if func_call.namespace.is_some() {
                        return Err(("Undefined function.", func_call.span).into());
                    }

                    SassFunction::Plain { name }
                }
            }
        };

        let old_in_function = self.flags.in_function();
        self.flags.set(ContextFlags::IN_FUNCTION, true);
        let value =
            self.run_function_callable(func, (*func_call.arguments).clone(), func_call.span)?;
        self.flags.set(ContextFlags::IN_FUNCTION, old_in_function);

        Ok(value)
    }

    fn visit_interpolated_func_expr(&mut self, func: InterpolatedFunction) -> SassResult<Value> {
        let InterpolatedFunction {
            name,
            arguments: args,
            span,
        } = func;
        let fn_name = self.perform_interpolation(name, false)?;

        if !args.named.is_empty() || args.keyword_rest.is_some() {
            return Err(("Plain CSS functions don't support keyword arguments.", span).into());
        }

        let mut buffer = format!("{}(", fn_name);

        let mut first = true;
        for arg in args.positional.clone() {
            if first {
                first = false;
            } else {
                buffer.push_str(", ");
            }
            let evaluated = self.evaluate_to_css(arg, QuoteKind::Quoted, span)?;
            buffer.push_str(&evaluated);
        }

        if let Some(rest_arg) = args.rest {
            let rest = self.visit_expr(rest_arg)?;
            if !first {
                buffer.push_str(", ");
            }
            buffer.push_str(&self.serialize(rest, QuoteKind::None, span)?);
        }

        buffer.push(')');

        Ok(Value::String(buffer, QuoteKind::None))
    }

    fn visit_parent_selector(&self) -> Value {
        match &self.style_rule_ignoring_at_root {
            Some(selector) => selector.as_selector_list().clone().to_sass_list(),
            None => Value::Null,
        }
    }

    fn visit_expr(&mut self, expr: AstExpr) -> SassResult<Value> {
        Ok(match expr {
            AstExpr::Color(color) => Value::Color(color),
            AstExpr::Number { n, unit } => Value::Dimension(SassNumber {
                num: n,
                unit,
                as_slash: None,
            }),
            AstExpr::List(list) => self.visit_list_expr(list)?,
            AstExpr::String(StringExpr(text, quote), ..) => self.visit_string(text, quote)?,
            AstExpr::BinaryOp(binop) => self.visit_bin_op(
                binop.lhs.clone(),
                binop.op,
                binop.rhs.clone(),
                binop.allows_slash,
                binop.span,
            )?,
            AstExpr::True => Value::True,
            AstExpr::False => Value::False,
            AstExpr::FunctionCall(func_call) => self.visit_function_call_expr(func_call)?,
            AstExpr::If(if_expr) => self.visit_ternary((*if_expr).clone())?,
            AstExpr::CssIf(e) => self.visit_css_if(&e)?,
            AstExpr::InterpolatedFunction(func) => {
                self.visit_interpolated_func_expr((*func).clone())?
            }
            AstExpr::Map(map) => self.visit_map(map)?,
            AstExpr::Null => Value::Null,
            AstExpr::Paren(expr) => self.visit_expr((*expr).clone())?,
            AstExpr::ParentSelector => self.visit_parent_selector(),
            AstExpr::UnaryOp(op, expr, span) => self.visit_unary_op(op, (*expr).clone(), span)?,
            AstExpr::Variable { name, namespace } => self.env.get_var(name, namespace)?,
            AstExpr::Supports(condition) => Value::String(
                self.visit_supports_condition((*condition).clone())?,
                QuoteKind::None,
            ),
        })
    }

    /// `isCalculationSafe` (`IsCalculationSafeVisitor`).
    fn is_calculation_safe(expr: &AstExpr) -> bool {
        match expr {
            AstExpr::BinaryOp(b) => {
                matches!(
                    b.op,
                    BinaryOp::Mul | BinaryOp::Div | BinaryOp::Plus | BinaryOp::Minus
                ) && Self::is_calculation_safe(&b.lhs)
                    && Self::is_calculation_safe(&b.rhs)
            }
            AstExpr::FunctionCall(..)
            | AstExpr::InterpolatedFunction(..)
            | AstExpr::If(..)
            | AstExpr::CssIf(..) => true,
            AstExpr::List(l) => {
                l.separator == ListSeparator::Space
                    && l.brackets == Brackets::None
                    && l.elems.len() > 1
                    && l.elems.iter().all(|e| Self::is_calculation_safe(&e.node))
            }
            AstExpr::Number { .. } | AstExpr::Variable { .. } => true,
            AstExpr::Paren(e) => Self::is_calculation_safe(e),
            AstExpr::String(StringExpr(text, QuoteKind::None), _) => {
                let t = text.initial_plain().as_bytes();
                !t.starts_with(b"!")
                    && !t.starts_with(b"#")
                    && t.get(1) != Some(&b'+')
                    && t.get(3) != Some(&b'(')
            }
            _ => false,
        }
    }

    /// `_visitCalculationExpression`.
    fn visit_calculation_value(
        &mut self,
        expr: AstExpr,
        in_legacy: bool,
        span: Span,
    ) -> SassResult<CalculationArg> {
        Ok(match expr {
            AstExpr::Paren(inner) => {
                let result = self.visit_calculation_value((*inner).clone(), in_legacy, span)?;
                match result {
                    CalculationArg::String(text) => CalculationArg::String(format!("({text})")),
                    r => r,
                }
            }
            AstExpr::String(StringExpr(text, QuoteKind::None), _)
                if Self::is_calculation_safe(&AstExpr::String(
                    StringExpr(text.clone(), QuoteKind::None),
                    span,
                )) =>
            {
                match text.as_plain().map(str::to_ascii_lowercase).as_deref() {
                    Some("pi") => {
                        CalculationArg::Number(SassNumber::new_unitless(std::f64::consts::PI))
                    }
                    Some("e") => {
                        CalculationArg::Number(SassNumber::new_unitless(std::f64::consts::E))
                    }
                    Some("infinity") => {
                        CalculationArg::Number(SassNumber::new_unitless(f64::INFINITY))
                    }
                    Some("-infinity") => {
                        CalculationArg::Number(SassNumber::new_unitless(f64::NEG_INFINITY))
                    }
                    Some("nan") => CalculationArg::Number(SassNumber::new_unitless(f64::NAN)),
                    _ => CalculationArg::String(self.perform_interpolation(text, false)?),
                }
            }
            AstExpr::BinaryOp(binop) => {
                self.check_whitespace_around_calculation_operator(&binop)?;
                let op = match binop.op {
                    BinaryOp::Plus | BinaryOp::Minus | BinaryOp::Mul | BinaryOp::Div => binop.op,
                    _ => {
                        return Err(
                            ("This operation can't be used in a calculation.", binop.span).into(),
                        )
                    }
                };
                let lhs = self.visit_calculation_value(binop.lhs.clone(), in_legacy, span)?;
                let rhs = self.visit_calculation_value(binop.rhs.clone(), in_legacy, span)?;
                SassCalculation::operate_internal(
                    op,
                    lhs,
                    rhs,
                    in_legacy,
                    !self.flags.in_supports_declaration(),
                    binop.span,
                )?
            }
            AstExpr::Number { .. }
            | AstExpr::Variable { .. }
            | AstExpr::FunctionCall { .. }
            | AstExpr::InterpolatedFunction(..)
            | AstExpr::CssIf(..)
            | AstExpr::If(..) => {
                let result = self.visit_expr(expr)?;
                match result {
                    Value::Dimension(n) => CalculationArg::Number(n),
                    Value::Calculation(calc) => CalculationArg::Calculation(calc),
                    Value::String(s, QuoteKind::None) => CalculationArg::String(s),
                    value => {
                        return Err((
                            format!(
                                "Value {} can't be used in a calculation.",
                                value.inspect(span)?
                            ),
                            span,
                        )
                            .into())
                    }
                }
            }
            AstExpr::List(list)
                if list.separator == ListSeparator::Space
                    && list.brackets == Brackets::None
                    && list.elems.len() > 1 =>
            {
                let mut elements = Vec::with_capacity(list.elems.len());
                for e in &list.elems {
                    elements.push(self.visit_calculation_value(e.node.clone(), in_legacy, span)?);
                }
                // `_checkAdjacentCalculationValues`.
                for i in 1..elements.len() {
                    if matches!(elements[i - 1], CalculationArg::String(..))
                        || matches!(elements[i], CalculationArg::String(..))
                    {
                        continue;
                    }
                    let atual = &list.elems[i].node;
                    let menos = matches!(atual, AstExpr::UnaryOp(UnaryOp::Neg | UnaryOp::Plus, ..))
                        || matches!(atual, AstExpr::Number { n, .. } if n.0 < 0.0);
                    return Err(if menos {
                        (
                            "\"+\" and \"-\" must be surrounded by whitespace in calculations.",
                            list.elems[i].span,
                        )
                            .into()
                    } else {
                        (
                            "Missing math operator.",
                            list.elems[i - 1].span.merge(list.elems[i].span),
                        )
                            .into()
                    });
                }
                let mut partes = Vec::with_capacity(elements.len());
                for (i, e) in elements.into_iter().enumerate() {
                    let entre_parenteses = matches!(list.elems[i].node, AstExpr::Paren(..));
                    let texto = self.calculation_arg_text(&e, span)?;
                    partes.push(
                        if entre_parenteses && matches!(e, CalculationArg::Operation { .. }) {
                            format!("({texto})")
                        } else {
                            texto
                        },
                    );
                }
                CalculationArg::String(partes.join(" "))
            }
            _ => return Err(("This expression can't be used in a calculation.", span).into()),
        })
    }

    /// O texto de um argumento de cálculo como o `toString` do dart-sass
    /// (a serialização `inspect`).
    fn calculation_arg_text(&self, arg: &CalculationArg, span: Span) -> SassResult<String> {
        Ok(match arg {
            CalculationArg::String(s) => s.clone(),
            CalculationArg::Number(n) => Value::Dimension(n.clone()).inspect(span)?,
            CalculationArg::Calculation(c) => Value::Calculation(c.clone()).inspect(span)?,
            op => {
                let t = Value::Calculation(SassCalculation::unsimplified("", vec![op.clone()]))
                    .inspect(span)?;
                t[1..t.len() - 1].to_owned()
            }
        })
    }

    /// `_checkWhitespaceAroundCalculationOperator`.
    fn check_whitespace_around_calculation_operator(&self, b: &BinaryOpExpr) -> SassResult<()> {
        if b.op != BinaryOp::Plus && b.op != BinaryOp::Minus {
            return Ok(());
        }
        let (l, r) = (b.lhs_span, b.rhs_span);
        if l.high() >= r.low() {
            return Ok(());
        }
        let arquivo = self.map.find_file(l.high());
        if !arquivo.span.contains(r) {
            return Ok(());
        }
        let base = arquivo.span.low();
        let texto = arquivo.source_slice(arquivo.span.subspan(l.high() - base, r.low() - base));
        let ok = |c: u8| matches!(c, b' ' | b'\t' | b'\n' | b'\r' | b'\x0c' | b'/');
        let bytes = texto.as_bytes();
        if bytes.is_empty() || !ok(bytes[0]) || !ok(bytes[bytes.len() - 1]) {
            return Err((
                "\"+\" and \"-\" must be surrounded by whitespace in calculations.",
                b.span,
            )
                .into());
        }
        Ok(())
    }

    /// `_visitCalculation`.
    fn visit_calculation_expr(
        &mut self,
        name: &str,
        arguments: ArgumentInvocation,
        in_legacy: bool,
        span: Span,
    ) -> SassResult<Value> {
        if !arguments.named.is_empty() || arguments.keyword_rest.is_some() {
            return Err(("Keyword arguments can't be used with calculations.", span).into());
        }
        if arguments.rest.is_some() {
            return Err(("Rest arguments can't be used with calculations.", span).into());
        }
        let n = arguments.positional.len();
        let max = match name {
            "calc" | "sqrt" | "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "abs" | "exp"
            | "sign" => Some(1),
            "min" | "max" | "hypot" => None,
            "pow" | "atan2" | "log" | "mod" | "rem" | "calc-size" => Some(2),
            _ => Some(3),
        };
        if n == 0 {
            return Err(("Missing argument.", span).into());
        }
        if let Some(max) = max {
            if n > max {
                return Err((
                    format!(
                        "Only {max} argument{} allowed, but {n} {} passed.",
                        if max == 1 { "" } else { "s" },
                        if n == 1 { "was" } else { "were" }
                    ),
                    span,
                )
                    .into());
            }
        }
        let mut args = Vec::with_capacity(n);
        for a in arguments.positional {
            args.push(self.visit_calculation_value(a, in_legacy, span)?);
        }
        if self.flags.in_supports_declaration() {
            return Ok(Value::Calculation(SassCalculation::unsimplified(
                name, args,
            )));
        }
        let mut it = args.clone().into_iter();
        let a0 = it.next().unwrap();
        let a1 = it.next();
        let a2 = it.next();
        match name {
            "calc" => Ok(SassCalculation::calc(a0)),
            "sqrt" | "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "abs" | "exp" | "sign" => {
                SassCalculation::single_function(name, a0, span)
            }
            "min" => SassCalculation::min(args, span),
            "max" => SassCalculation::max(args, span),
            "hypot" => SassCalculation::hypot(args, span),
            "pow" => SassCalculation::pow(a0, a1, span),
            "atan2" => SassCalculation::atan2(a0, a1, span),
            "log" => SassCalculation::log(a0, a1, span),
            "mod" => SassCalculation::modulo(a0, a1, span),
            "rem" => SassCalculation::rem(a0, a1, span),
            "round" => SassCalculation::round(a0, a1, a2, in_legacy, span),
            "clamp" => SassCalculation::clamp(a0, a1, a2, span),
            "calc-size" => SassCalculation::calc_size(a0, a1, span),
            _ => unreachable!("cálculo desconhecido {name}"),
        }
    }

    /// `visitIfExpression`: o `if()` do CSS. Ramo com condição `sass()`
    /// resolvida escolhe na compilação; condições do CSS viram o `if()` na
    /// saída.
    fn visit_css_if(&mut self, e: &CssIfExpr) -> SassResult<Value> {
        let mut results: Option<Vec<(String, Value)>> = None;
        for (condition, expr) in &e.branches {
            let r = match condition {
                None => CondIf::Bool(true),
                Some(c) => self.visit_if_condition(c)?,
            };
            match r {
                CondIf::Str(c) => {
                    let v = self.visit_expr(expr.clone())?;
                    results.get_or_insert_with(Vec::new).push((c, v));
                }
                CondIf::Bool(true) => {
                    let v = self.visit_expr(expr.clone())?;
                    match &mut results {
                        Some(r) => r.push(("else".to_owned(), v)),
                        None => return Ok(v),
                    }
                }
                CondIf::Bool(false) => {}
            }
        }
        let Some(results) = results else {
            return Ok(Value::Null);
        };
        let mut partes = Vec::with_capacity(results.len());
        for (c, v) in results {
            partes.push(format!("{c}: {}", v.to_css_string(e.span, false)?));
        }
        Ok(Value::String(
            format!("if({})", partes.join("; ")),
            QuoteKind::None,
        ))
    }

    fn visit_if_condition(&mut self, c: &IfCondition) -> SassResult<CondIf> {
        Ok(match c {
            IfCondition::Paren(e) => match self.visit_if_condition(e)? {
                CondIf::Str(s) => CondIf::Str(format!("({s})")),
                b => b,
            },
            IfCondition::Not(e) => match self.visit_if_condition(e)? {
                CondIf::Str(s) => CondIf::Str(format!("not {s}")),
                CondIf::Bool(b) => CondIf::Bool(!b),
            },
            IfCondition::Op(es, and) => {
                let mut values: Option<Vec<(bool, String)>> = None;
                for e in es {
                    match self.visit_if_condition(e)? {
                        CondIf::Str(s) => values
                            .get_or_insert_with(Vec::new)
                            .push((matches!(e, IfCondition::Paren(..)), s)),
                        CondIf::Bool(false) if *and => return Ok(CondIf::Bool(false)),
                        CondIf::Bool(true) if !*and => return Ok(CondIf::Bool(true)),
                        CondIf::Bool(_) => {}
                    }
                }
                match values {
                    None => CondIf::Bool(*and),
                    Some(v) if v.len() == 1 && v[0].0 => {
                        CondIf::Str(v[0].1[1..v[0].1.len() - 1].to_owned())
                    }
                    Some(v) => CondIf::Str(
                        v.into_iter()
                            .map(|(_, s)| s)
                            .collect::<Vec<_>>()
                            .join(if *and { " and " } else { " or " }),
                    ),
                }
            }
            IfCondition::Function { name, args } => {
                let n = self.perform_interpolation(name.clone(), false)?;
                let a = self.perform_interpolation(args.clone(), false)?;
                CondIf::Str(format!("{n}({a})"))
            }
            IfCondition::Sass(e, _) => CondIf::Bool(self.visit_expr(e.clone())?.is_truthy()),
            IfCondition::Raw(t) => CondIf::Str(self.perform_interpolation(t.clone(), false)?),
        })
    }

    fn visit_unary_op(&mut self, op: UnaryOp, expr: AstExpr, span: Span) -> SassResult<Value> {
        let operand = self.visit_expr(expr)?;

        match op {
            UnaryOp::Plus => operand.unary_plus(self, span),
            UnaryOp::Neg => operand.unary_neg(self, span),
            UnaryOp::Div => operand.unary_div(self, span),
            UnaryOp::Not => Ok(operand.unary_not()),
        }
    }

    fn visit_ternary(&mut self, if_expr: Ternary) -> SassResult<Value> {
        if_arguments().verify(if_expr.0.positional.len(), &if_expr.0.named, if_expr.0.span)?;

        let mut positional = if_expr.0.positional;
        let mut named = if_expr.0.named;

        let condition = if positional.is_empty() {
            named.remove(&Identifier::from("condition")).unwrap()
        } else {
            positional.remove(0)
        };

        let if_true = if positional.is_empty() {
            named.remove(&Identifier::from("if_true")).unwrap()
        } else {
            positional.remove(0)
        };

        let if_false = if positional.is_empty() {
            named.remove(&Identifier::from("if_false")).unwrap()
        } else {
            positional.remove(0)
        };

        let value = if self.visit_expr(condition)?.is_truthy() {
            self.visit_expr(if_true)?
        } else {
            self.visit_expr(if_false)?
        };

        Ok(self.without_slash(value))
    }

    fn visit_string(&mut self, mut text: Interpolation, quote: QuoteKind) -> SassResult<Value> {
        // Don't use [performInterpolation] here because we need to get the raw text
        // from strings, rather than the semantic value.
        let old_in_supports_declaration = self.flags.in_supports_declaration();
        self.flags.set(ContextFlags::IN_SUPPORTS_DECLARATION, false);

        let result = match text.contents.len() {
            0 => String::new(),
            1 => match text.contents.pop() {
                Some(InterpolationPart::String(s)) => s,
                Some(InterpolationPart::Expr(Spanned { node, span })) => {
                    match self.visit_expr(node)? {
                        Value::String(s, ..) => s,
                        e => self.serialize(e, QuoteKind::None, span)?,
                    }
                }
                None => unreachable!(),
            },
            _ => text
                .contents
                .into_iter()
                .map(|part| match part {
                    InterpolationPart::String(s) => Ok(s),
                    InterpolationPart::Expr(Spanned { node, span }) => {
                        match self.visit_expr(node)? {
                            Value::String(s, ..) => Ok(s),
                            e => self.serialize(e, QuoteKind::None, span),
                        }
                    }
                })
                .collect::<SassResult<String>>()?,
        };

        self.flags.set(
            ContextFlags::IN_SUPPORTS_DECLARATION,
            old_in_supports_declaration,
        );

        Ok(Value::String(result, quote))
    }

    fn visit_map(&mut self, map: AstSassMap) -> SassResult<Value> {
        let mut sass_map = SassMap::new();

        for pair in map.0 {
            let key_span = pair.0.span;
            let key = self.visit_expr(pair.0.node)?;
            let value = self.visit_expr(pair.1)?;

            if sass_map.get_ref(&key).is_some() {
                return Err(("Duplicate key.", key_span).into());
            }

            sass_map.insert(
                Spanned {
                    node: key,
                    span: key_span,
                },
                value,
            );
        }

        Ok(Value::Map(sass_map))
    }

    fn visit_bin_op(
        &mut self,
        lhs: AstExpr,
        op: BinaryOp,
        rhs: AstExpr,
        allows_slash: bool,
        span: Span,
    ) -> SassResult<Value> {
        let left = self.visit_expr(lhs)?;

        Ok(match op {
            BinaryOp::SingleEq => {
                let right = self.visit_expr(rhs)?;
                single_eq(&left, &right, self.options, span)?
            }
            BinaryOp::Or => {
                if left.is_truthy() {
                    left
                } else {
                    self.visit_expr(rhs)?
                }
            }
            BinaryOp::And => {
                if left.is_truthy() {
                    self.visit_expr(rhs)?
                } else {
                    left
                }
            }
            BinaryOp::Equal => {
                let right = self.visit_expr(rhs)?;
                Value::bool(left == right)
            }
            BinaryOp::NotEqual => {
                let right = self.visit_expr(rhs)?;
                Value::bool(left != right)
            }
            BinaryOp::GreaterThan
            | BinaryOp::GreaterThanEqual
            | BinaryOp::LessThan
            | BinaryOp::LessThanEqual => {
                let right = self.visit_expr(rhs)?;
                cmp(&left, &right, self.options, span, op)?
            }
            BinaryOp::Plus => {
                let right = self.visit_expr(rhs)?;
                add(left, right, self.options, span)?
            }
            BinaryOp::Minus => {
                let right = self.visit_expr(rhs)?;
                sub(left, right, self.options, span)?
            }
            BinaryOp::Mul => {
                let right = self.visit_expr(rhs)?;
                mul(left, right, self.options, span)?
            }
            BinaryOp::Div => {
                let right = self.visit_expr(rhs)?;

                let left_is_number = matches!(left, Value::Dimension { .. });
                let right_is_number = matches!(right, Value::Dimension { .. });

                if left_is_number && right_is_number && allows_slash {
                    let result = div(left.clone(), right.clone(), self.options, span)?;
                    return result.with_slash(
                        left.assert_number(span)?,
                        right.assert_number(span)?,
                        span,
                    );
                } else if left_is_number && right_is_number {
                    // todo: emit warning here. it prints too frequently, so we do not currently
                    // self.emit_warning(
                    //     Cow::Borrowed(format!(
                    //         "Using / for division outside of calc() is deprecated"
                    //     )),
                    //     span,
                    // );
                }

                div(left, right, self.options, span)?
            }
            BinaryOp::Rem => {
                let right = self.visit_expr(rhs)?;
                rem(left, right, self.options, span)?
            }
        })
    }

    // todo: superfluous taking `expr` by value
    fn serialize(&mut self, mut expr: Value, quote: QuoteKind, span: Span) -> SassResult<String> {
        if quote == QuoteKind::None {
            expr = expr.unquote();
        }

        expr.to_css_string(span, self.options.is_compressed())
    }

    pub(crate) fn visit_ruleset(&mut self, ruleset: AstRuleSet) -> SassResult<Option<Value>> {
        if self.declaration_name.is_some() {
            return Err((
                "Style rules may not be used within nested declarations.",
                ruleset.span,
            )
                .into());
        }

        let AstRuleSet {
            selector: ruleset_selector,
            body: ruleset_body,
            ..
        } = ruleset;

        let selector_text = self.interpolation_to_value(ruleset_selector, true, true)?;

        if self.flags.in_keyframes() {
            let span = ruleset.selector_span;
            let sel_toks = Lexer::new_from_string(&selector_text, span);
            let parsed_selector =
                KeyframesSelectorParser::new(sel_toks).parse_keyframes_selector()?;

            let keyframes_ruleset = CssStmt::KeyframesRuleSet(KeyframesRuleSet {
                selector: parsed_selector,
                body: Vec::new(),
                selector_span: ruleset.selector_span,
                span: ruleset.span,
            });

            self.with_parent(
                keyframes_ruleset,
                true,
                |visitor| {
                    for stmt in ruleset_body {
                        let result = visitor.visit_stmt(stmt)?;
                        debug_assert!(result.is_none());
                    }

                    Ok(())
                },
                CssStmt::is_style_rule,
            )?;

            return Ok(None);
        }

        let mut parsed_selector = self.parse_selector_from_string(
            &selector_text,
            !self.is_plain_css,
            !self.is_plain_css,
            ruleset.selector_span,
        )?;

        parsed_selector = parsed_selector.resolve_parent_selectors(
            self.style_rule_ignoring_at_root
                .as_ref()
                // todo: this clone should be superfluous(?)
                .map(|x| x.as_selector_list().clone()),
            !self.flags.at_root_excluding_style_rule(),
        )?;

        // todo: _mediaQueries
        let selector = self
            .extender
            .add_selector(parsed_selector, &self.media_queries);

        let rule = CssStmt::RuleSet {
            selector: selector.clone(),
            body: Vec::new(),
            is_group_end: false,
            selector_span: ruleset.selector_span,
            span: ruleset.span,
        };

        let old_at_root_excluding_style_rule = self.flags.at_root_excluding_style_rule();

        self.flags
            .set(ContextFlags::AT_ROOT_EXCLUDING_STYLE_RULE, false);

        let old_style_rule_ignoring_at_root = self.style_rule_ignoring_at_root.take();
        self.style_rule_ignoring_at_root = Some(selector);
        let old_style_rule_spans = self.style_rule_spans;
        self.style_rule_spans = (ruleset.selector_span, ruleset.span);

        self.with_parent(
            rule,
            true,
            |visitor| {
                for stmt in ruleset_body {
                    let result = visitor.visit_stmt(stmt)?;
                    debug_assert!(result.is_none());
                }

                Ok(())
            },
            CssStmt::is_style_rule,
        )?;

        self.style_rule_ignoring_at_root = old_style_rule_ignoring_at_root;
        self.style_rule_spans = old_style_rule_spans;
        self.flags.set(
            ContextFlags::AT_ROOT_EXCLUDING_STYLE_RULE,
            old_at_root_excluding_style_rule,
        );

        self.set_group_end();

        Ok(None)
    }

    fn set_group_end(&mut self) -> Option<()> {
        if !self.style_rule_exists() {
            let children = self
                .css_tree
                .parent_to_child
                .get(&self.parent.unwrap_or(CssTree::ROOT))?;
            let child = *children.last()?;
            self.css_tree
                .get_mut(child)
                .as_mut()
                .map(CssStmt::set_group_end)?;
        }

        Some(())
    }

    fn style_rule_exists(&self) -> bool {
        !self.flags.at_root_excluding_style_rule() && self.style_rule_ignoring_at_root.is_some()
    }

    pub(crate) fn visit_style(&mut self, style: AstStyle) -> SassResult<Option<Value>> {
        if !self.style_rule_exists()
            && !self.flags.in_unknown_at_rule()
            && !self.flags.in_keyframes()
        {
            return Err((
                "Declarations may only be used within style rules.",
                style.span,
            )
                .into());
        }

        let is_custom_property = style.is_custom_property();

        let mut name = self.interpolation_to_value(style.name, false, true)?;

        if let Some(declaration_name) = &self.declaration_name {
            name = format!("{}-{}", declaration_name, name);
        }

        let value_node = style
            .value
            .as_ref()
            .map(|v| self.expression_node(&v.node, v.span));
        if let Some(value) = style
            .value
            .map(|s| {
                SassResult::Ok(Spanned {
                    node: self.visit_expr(s.node)?,
                    span: s.span,
                })
            })
            .transpose()?
        {
            // If the value is an empty list, preserve it, because converting it to CSS
            // will throw an error that we want the user to see.
            // dart-sass `visitDeclaration`: propriedade customizada pode ter
            // valor vazio ("per spec").
            if !value.is_blank() || value.is_empty_list() || name.starts_with("--") {
                self.copy_parent_after_sibling();
                // todo: superfluous clones?
                self.css_tree.add_stmt(
                    CssStmt::Style(Style {
                        property: InternedString::get_or_intern(&name),
                        value_span_for_map: value_node.unwrap_or(value.span),
                        value: Box::new(value),
                        declared_as_custom_property: is_custom_property,
                        name_span: style.span,
                        span: style.span,
                    }),
                    self.parent,
                );
            } else if name.starts_with("--") {
                return Err(("Custom property values may not be empty.", style.span).into());
            }
        }

        let children = style.body;

        if !children.is_empty() {
            let old_declaration_name = self.declaration_name.take();
            self.declaration_name = Some(name);
            self.with_scope::<SassResult<()>, _>(false, true, |visitor| {
                for stmt in children {
                    let result = visitor.visit_stmt(stmt)?;
                    debug_assert!(result.is_none());
                }

                Ok(())
            })?;
            self.declaration_name = old_declaration_name;
        }

        Ok(None)
    }
}
