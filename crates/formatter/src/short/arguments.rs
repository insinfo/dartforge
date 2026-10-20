//! Port of `short/argument_list_visitor.dart`: `ArgumentListVisitor` and
//! `ArgumentSublist`, with the special handling of block arguments
//! (functions with a `{}` body, collections and multiline strings).

use super::arena::{ChunkId, RuleId, cost};
use super::visitor::SourceVisitor;
use crate::syntax::token::Tk;
use crate::syntax::tree::{Node, NodeId};

/// `ArgumentListVisitor`.
pub struct ArgumentListVisitor {
    left_side: Tk,
    right_: Tk,
    all: Vec<NodeId>,
    arguments_: ArgumentSublist,
    functions: Option<Vec<NodeId>>,
    after_functions: Option<ArgumentSublist>,
}

/// `ArgumentSublist`.
pub struct ArgumentSublist {
    all: Vec<NodeId>,
    positionals: Vec<NodeId>,
    named_args: Vec<NodeId>,
    blocks: Vec<(NodeId, Tk)>,
    leading_blocks: usize,
    trailing_blocks: usize,
    block_rule: Option<RuleId>,
    previous_split: Option<ChunkId>,
}

impl ArgumentListVisitor {
    /// `ArgumentListVisitor(visitor, node)`.
    pub fn new(v: &SourceVisitor, list: NodeId) -> ArgumentListVisitor {
        let Node::ArgumentList {
            left_parenthesis,
            arguments,
            right_parenthesis,
        } = v.node(list)
        else {
            panic!("expected ArgumentList");
        };
        Self::for_arguments(v, *left_parenthesis, *right_parenthesis, arguments)
    }

    /// `ArgumentListVisitor.forArguments`.
    pub fn for_arguments(
        v: &SourceVisitor,
        left_side: Tk,
        right_: Tk,
        arguments_: &[NodeId],
    ) -> ArgumentListVisitor {
        match contiguous_functions(v, arguments_) {
            None => ArgumentListVisitor {
                left_side,
                right_,
                all: arguments_.to_vec(),
                arguments_: ArgumentSublist::new(v, arguments_, arguments_),
                functions: None,
                after_functions: None,
            },
            Some((start_, end)) => ArgumentListVisitor {
                left_side,
                right_,
                all: arguments_.to_vec(),
                arguments_: ArgumentSublist::new(v, arguments_, &arguments_[..start_]),
                functions: Some(arguments_[start_..end].to_vec()),
                after_functions: Some(ArgumentSublist::new(v, arguments_, &arguments_[end..])),
            },
        }
    }

    fn is_single(&self, v: &SourceVisitor) -> bool {
        self.all.len() == 1 && !matches!(v.node(self.all[0]), Node::NamedExpression { .. })
    }

    /// `hasBlockArguments`.
    pub fn has_block_arguments(&self) -> bool {
        !self.arguments_.blocks.is_empty() || self.functions.is_some()
    }

    /// `visit()`.
    pub fn visit(mut self, v: &mut SourceVisitor) {
        let single = self.is_single(v);
        if single {
            v.start_span(cost::NORMAL);
        }
        v.start_span(cost::NORMAL);
        v.token(self.left_side);
        self.arguments_.visit(v);
        v.end_span();
        if let Some(functions) = &self.functions {
            if functions[0] == self.all[0] {
                v.solo_zero_split();
            } else {
                v.solo_split(cost::NORMAL);
            }
            for (i, &arg) in functions.iter().enumerate() {
                if i > 0 {
                    v.space();
                }
                v.visit(Some(arg));
                if v.has_comma_after(arg) {
                    let comma = v.tree_.end(arg) + 1;
                    v.token(comma);
                }
            }
            v.start_span(cost::NORMAL);
            self.after_functions.as_mut().expect("sublist").visit(v);
            v.end_span();
        }
        v.token(self.right_);
        if single {
            v.end_span();
        }
    }
}

/// `_contiguousFunctions(arguments)`.
fn contiguous_functions(v: &SourceVisitor, arguments_: &[NodeId]) -> Option<(usize, usize)> {
    let mut start: Option<usize> = None;
    let mut end: i64 = -1;
    for (i, &arg) in arguments_.iter().enumerate() {
        if is_block_function(v, arg) {
            start.get_or_insert(i);
            if end != -1 && end != i as i64 {
                return None;
            }
            end = i as i64 + 1;
        }
    }
    let start = start?;
    let end = end as usize;
    let all_named = |xs: &[NodeId]| {
        xs.iter()
            .all(|&a| matches!(v.node(a), Node::NamedExpression { .. }))
    };
    if all_named(arguments_) && (start > 0 || end < arguments_.len()) {
        return None;
    }
    if all_named(&arguments_[start..end]) {
        let named_arrow = |e: NodeId| -> bool {
            let Node::NamedExpression { expression, .. } = v.node(e) else {
                return false;
            };
            match v.node(*expression) {
                Node::FunctionExpression { body, .. } => {
                    matches!(v.node(*body), Node::ExpressionFunctionBody { .. })
                }
                _ => false,
            }
        };
        for &a in &arguments_[..start] {
            if named_arrow(a) {
                return None;
            }
        }
        for &a in &arguments_[end..] {
            if named_arrow(a) {
                return None;
            }
        }
    }
    Some((start, end))
}

/// `_isBlockFunction(expression)`.
fn is_block_function(v: &SourceVisitor, expr: NodeId) -> bool {
    let mut e = expr;
    if let Node::NamedExpression { expression, .. } = v.node(e) {
        e = *expression;
    }
    match v.node(e) {
        Node::MethodInvocation {
            target,
            argument_list,
            ..
        } => {
            if !is_valid_wrapping_target(v, *target) {
                return false;
            }
            let Node::ArgumentList { arguments, .. } = v.node(*argument_list) else {
                return false;
            };
            if arguments.len() != 1 {
                return false;
            }
            return is_block_function(v, arguments[0]);
        }
        Node::InstanceCreationExpression { argument_list, .. } => {
            let Node::ArgumentList { arguments, .. } = v.node(*argument_list) else {
                return false;
            };
            if arguments.len() != 1 {
                return false;
            }
            return is_block_function(v, arguments[0]);
        }
        Node::FunctionExpressionInvocation {
            function,
            argument_list,
            ..
        } => {
            let Node::ArgumentList { arguments, .. } = v.node(*argument_list) else {
                return false;
            };
            if !arguments.is_empty() {
                return false;
            }
            e = *function;
        }
        _ => {}
    }
    while let Node::ParenthesizedExpression { expression, .. } = v.node(e) {
        e = *expression;
    }
    let Node::FunctionExpression { body, .. } = v.node(e) else {
        return false;
    };
    let Node::BlockFunctionBody { block, .. } = v.node(*body) else {
        return false;
    };
    let Node::Block {
        statements,
        right_bracket,
        ..
    } = v.node(*block)
    else {
        return false;
    };
    !statements.is_empty() || !v.tk.comments_of(*right_bracket).is_empty()
}

/// `_isValidWrappingTarget(expression)`.
fn is_valid_wrapping_target(v: &SourceVisitor, target_: Option<NodeId>) -> bool {
    let Some(mut e) = target_ else {
        return true;
    };
    while let Node::PropertyAccess {
        target: Some(t), ..
    } = v.node(e)
    {
        e = *t;
    }
    matches!(
        v.node(e),
        Node::PrefixedIdentifier { .. } | Node::SimpleIdentifier { .. }
    )
}

impl ArgumentSublist {
    /// `ArgumentSublist(allArguments, arguments)`.
    fn new(v: &SourceVisitor, all: &[NodeId], arguments_: &[NodeId]) -> ArgumentSublist {
        let (positionals, named_args) = split_argument_lists(v, arguments_);
        let mut blocks: Vec<(NodeId, Tk)> = Vec::new();
        for &a in arguments_ {
            if let Some(t) = block_token(v, a) {
                blocks.push((a, t));
            }
        }
        let is_block = |blocks: &Vec<(NodeId, Tk)>, a: NodeId| blocks.iter().any(|(b, _)| *b == a);
        let mut leading = 0;
        for &a in arguments_ {
            if !is_block(&blocks, a) {
                break;
            }
            leading += 1;
        }
        let mut trailing = 0;
        if leading != arguments_.len() {
            for &a in arguments_.iter().rev() {
                if !is_block(&blocks, a) {
                    break;
                }
                trailing += 1;
            }
        }
        if leading != blocks.len() {
            leading = 0;
        }
        if trailing != blocks.len() {
            trailing = 0;
        }
        if leading == 0 && trailing == 0 {
            blocks.clear();
        }
        ArgumentSublist {
            all: all.to_vec(),
            positionals,
            named_args,
            blocks,
            leading_blocks: leading,
            trailing_blocks: trailing,
            block_rule: None,
            previous_split: None,
        }
    }

    fn visit(&mut self, v: &mut SourceVisitor) {
        if !self.blocks.is_empty() {
            self.block_rule = Some(v.a.rule(cost::SPLIT_BLOCKS));
        }
        let rule = self.visit_positional(v);
        self.visit_named(v, rule);
    }

    fn visit_positional(&mut self, v: &mut SourceVisitor) -> Option<RuleId> {
        if self.positionals.is_empty() {
            return None;
        }
        let leading = self.leading_blocks.min(self.positionals.len());
        let trailing = self.trailing_blocks.saturating_sub(self.named_args.len());
        let rule =
            v.a.positional_rule(self.block_rule, self.positionals.len(), leading, trailing);
        let positionals = self.positionals.clone();
        self.visit_arguments(v, &positionals, rule, true);
        Some(rule)
    }

    fn visit_named(&mut self, v: &mut SourceVisitor, positional: Option<RuleId>) {
        if self.named_args.is_empty() {
            return;
        }
        let leading = self.leading_blocks.saturating_sub(self.positionals.len());
        let trailing = self.trailing_blocks.min(self.named_args.len());
        let named_ = v.a.named_rule(self.block_rule, leading, trailing);
        if let Some(p) = positional {
            v.a.named_constraints(p, named_);
        }
        let named_args = self.named_args.clone();
        self.visit_arguments(v, &named_args, named_, false);
    }

    fn visit_arguments(
        &mut self,
        v: &mut SourceVisitor,
        arguments_: &[NodeId],
        rule: RuleId,
        are_positional: bool,
    ) {
        v.start_rule(Some(rule));
        let c = v.builder_split(true, arguments_[0] != self.all[0]);
        self.previous_split = Some(c);
        v.a.before_argument(rule, Some(c));
        if are_positional {
            v.start_span(cost::POSITIONAL_ARGUMENTS);
        }
        for (i, &arg) in arguments_.iter().enumerate() {
            self.visit_argument(v, rule, arg);
            if i + 1 != arguments_.len() {
                let c = v.split();
                self.previous_split = Some(c);
                v.a.before_argument(rule, Some(c));
            }
        }
        if are_positional {
            v.end_span();
        }
        v.end_rule();
    }

    fn visit_argument(&mut self, v: &mut SourceVisitor, rule: RuleId, arg: NodeId) {
        let block = self.blocks.iter().find(|(b, _)| *b == arg).map(|(_, t)| *t);
        let named = matches!(v.node(arg), Node::NamedExpression { .. });
        let several =
            self.all.len() > 1 || matches!(v.node(self.all[0]), Node::RecordLiteral { .. });
        if let Some(t) = block {
            v.a.disable_inner(rule);
            let rb = self.block_rule.expect("block rule");
            v.before_block(t, rb, self.previous_split);
        } else if several {
            v.start_block_argument_nesting();
        } else if !named {
            v.a.disable_inner(rule);
        }

        if let Node::NamedExpression { name, expression } = v.node(arg) {
            let Node::Label { label, colon } = v.node(*name) else {
                panic!("expected Label");
            };
            let Node::SimpleIdentifier { token } = v.node(*label) else {
                panic!("expected SimpleIdentifier");
            };
            v.visit_named_node(*token, *colon, *expression, Some(rule));
        } else {
            v.visit(Some(arg));
        }

        if block.is_some() {
            v.a.enable_inner(rule);
        } else if several {
            v.end_block_argument_nesting();
        } else if !named {
            v.a.enable_inner(rule);
        }

        if v.has_comma_after(arg) {
            let comma = v.tree_.end(arg) + 1;
            v.token(comma);
        }
    }
}

/// `_splitArgumentLists(arguments)`.
fn split_argument_lists(v: &SourceVisitor, arguments_: &[NodeId]) -> (Vec<NodeId>, Vec<NodeId>) {
    let mut positionals = Vec::new();
    let mut named_args = Vec::new();
    let mut in_named = false;
    for &a in arguments_ {
        if matches!(v.node(a), Node::NamedExpression { .. }) {
            in_named = true;
        } else if in_named {
            return (Vec::new(), arguments_.to_vec());
        }
        if in_named {
            named_args.push(a);
        } else {
            positionals.push(a);
        }
    }
    (positionals, named_args)
}

/// `_blockToken(expression)`.
fn block_token(v: &SourceVisitor, expr: NodeId) -> Option<Tk> {
    let mut e = expr;
    if let Node::NamedExpression { expression, .. } = v.node(e) {
        e = *expression;
    }
    match v.node(e) {
        Node::ListLiteral { left_bracket, .. } | Node::SetOrMapLiteral { left_bracket, .. } => {
            Some(*left_bracket)
        }
        Node::RecordLiteral {
            left_parenthesis, ..
        } => Some(*left_parenthesis),
        Node::SimpleStringLiteral { .. } | Node::StringInterpolation { .. }
            if v.is_multiline_string(e) =>
        {
            Some(v.tree_.start(e))
        }
        _ => None,
    }
}
