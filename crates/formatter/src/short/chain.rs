//! Port of `short/call_chain_visitor.dart`: flattens a selector chain
//! (`a.b(1)[2].c()!`) into a left-to-right list and writes the splits before
//! each `.`, handling the leading properties, the calls with block arguments
//! and the final hanging call.

use super::arena::{RuleId, cost};
use super::arguments::ArgumentListVisitor;
use super::visitor::SourceVisitor;
use crate::syntax::tree::{Node, NodeId};

/// Selector kind (`_MethodSelector`, `_PrefixedSelector`,
/// `_PropertySelector`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectorKind {
    Method,
    Prefixed,
    Property,
}

/// `_Selector`: the selector node and the postfix operators (`[i]`, `!`, `()`)
/// attached to it.
#[derive(Debug, Clone)]
struct Selector {
    kind_: SelectorKind,
    node: NodeId,
    postfixes: Vec<NodeId>,
}

impl Selector {
    fn is_property(&self) -> bool {
        self.kind_ != SelectorKind::Method
    }
}

/// `CallChainVisitor`.
pub struct CallChainVisitor {
    target_: NodeId,
    properties: Vec<Selector>,
    calls: Vec<Selector>,
    block_calls: Option<Vec<Selector>>,
    dangling: Option<Selector>,
    active_rule: bool,
    finished_span: bool,
    properties_rule: Option<RuleId>,
}

impl CallChainVisitor {
    /// `CallChainVisitor(visitor, node)`.
    pub fn new(v: &SourceVisitor, node: NodeId) -> CallChainVisitor {
        let mut calls: Vec<Selector> = Vec::new();
        let target_ = unwrap_target(v, node, &mut calls);
        let mut properties = Vec::new();
        if matches!(
            v.node(unwrap_null_assertion(v, target_)),
            Node::SimpleIdentifier { .. }
        ) {
            properties = calls
                .iter()
                .take_while(|c| c.is_property())
                .cloned()
                .collect();
        }
        calls.drain(..properties.len());

        let mut block_calls: Option<Vec<Selector>> = None;
        let mut dangling: Option<Selector> = None;
        let mut in_block = false;
        let n = calls.len();
        for (i, c) in calls.iter().enumerate() {
            if is_block_call(v, c) {
                in_block = true;
                block_calls.get_or_insert_with(Vec::new).push(c.clone());
            } else if in_block {
                if i == n - 1 {
                    dangling = Some(c.clone());
                    break;
                }
                block_calls = None;
                break;
            }
        }
        if let Some(bs) = &block_calls {
            for b in bs {
                if let Some(p) = calls.iter().position(|c| c.node == b.node) {
                    calls.remove(p);
                }
            }
        }
        if let Some(p) = &dangling
            && let Some(i) = calls.iter().position(|c| c.node == p.node)
        {
            calls.remove(i);
        }
        CallChainVisitor {
            target_,
            properties,
            calls,
            block_calls,
            dangling,
            active_rule: false,
            finished_span: false,
            properties_rule: None,
        }
    }

    /// `visit()`.
    pub fn visit(mut self, v: &mut SourceVisitor) {
        v.nest_expression(None, false);
        v.start_span(cost::NORMAL);
        let split_at_target = forces_split(v, self.target_);
        if split_at_target {
            if self.properties.len() > 1 {
                let r = v.a.positional_rule(None, self.properties.len(), 0, 0);
                self.properties_rule = Some(r);
                v.start_lazy_rule(Some(r));
            } else {
                self.enable_rule(v, true);
            }
        }
        v.visit(Some(self.target_));

        if self.properties.len() == 1 {
            v.solo_zero_split();
            let p = self.properties[0].clone();
            self.write(v, &p);
        } else if self.properties.len() > 1 {
            if !split_at_target {
                let r = v.a.positional_rule(None, self.properties.len(), 0, 0);
                self.properties_rule = Some(r);
                v.start_rule(Some(r));
            }
            for p in self.properties.clone() {
                let c = v.zero_split();
                v.a.before_argument(self.properties_rule.expect("rule"), Some(c));
                self.write(v, &p);
            }
            v.end_rule();
        }

        if self.calls.len() > 1 {
            v.start_block_argument_nesting();
        }
        for c in self.calls.clone() {
            self.enable_rule(v, false);
            v.zero_split();
            self.write(v, &c);
        }
        if self.calls.len() > 1 {
            v.end_block_argument_nesting();
        }

        if let Some(bs) = self.block_calls.clone() {
            self.enable_rule(v, false);
            v.zero_split();
            self.disable_rule(v);
            for b in &bs {
                self.write(v, b);
            }
            if let Some(p) = self.dangling.clone() {
                self.write(v, &p);
            }
        }

        self.disable_rule(v);
        self.end_span(v);
        v.unnest();
    }

    /// `_Selector.write(visitor)`.
    fn write(&mut self, v: &mut SourceVisitor, s: &Selector) {
        self.write_selector(v, s);
        v.nest_expression(None, false);
        for &p in &s.postfixes {
            match v.node(p) {
                Node::FunctionExpressionInvocation {
                    type_arguments,
                    argument_list,
                    ..
                } => {
                    v.solo_zero_split();
                    v.visit(*type_arguments);
                    v.visit_argument_list(*argument_list, true);
                }
                Node::IndexExpression { .. } => v.finish_index_expression(p),
                Node::PostfixExpression { operator, .. } => v.token(*operator),
                _ => {}
            }
        }
        v.unnest();
    }

    fn write_selector(&mut self, v: &mut SourceVisitor, s: &Selector) {
        match v.node(s.node) {
            Node::MethodInvocation {
                operator,
                method_name,
                type_arguments,
                argument_list,
                ..
            } => {
                v.token_opt(*operator);
                let name_ = match v.node(*method_name) {
                    Node::SimpleIdentifier { token } => *token,
                    _ => unreachable!(),
                };
                v.token(name_);
                self.before_method_arguments(v, s);
                v.nest_expression(None, false);
                v.visit(*type_arguments);
                v.visit_argument_list(*argument_list, false);
                v.unnest();
            }
            Node::PrefixedIdentifier {
                period, identifier, ..
            } => {
                v.token(*period);
                v.visit(Some(*identifier));
            }
            Node::PropertyAccess {
                operator,
                property_name,
                ..
            } => {
                v.token(*operator);
                v.visit(Some(*property_name));
            }
            _ => unreachable!(),
        }
    }

    /// `_beforeMethodArguments(selector)`.
    fn before_method_arguments(&mut self, v: &mut SourceVisitor, s: &Selector) {
        if self.block_calls.is_none() && self.calls.last().is_some_and(|c| c.node == s.node) {
            self.disable_rule(v);
        }
        if self.properties.is_empty()
            && self.calls.len() == 1
            && self.block_calls.is_none()
            && matches!(v.node(self.target_), Node::SimpleIdentifier { .. })
        {
            self.end_span(v);
        }
    }

    fn disable_rule(&mut self, v: &mut SourceVisitor) {
        if !self.active_rule {
            return;
        }
        v.end_rule();
        self.active_rule = false;
    }

    fn enable_rule(&mut self, v: &mut SourceVisitor, lazy: bool) {
        if self.active_rule {
            return;
        }
        let r = v.a.rule(cost::NORMAL);
        if let Some(p) = self.properties_rule {
            v.a.named_constraints(p, r);
        }
        if lazy {
            v.start_lazy_rule(Some(r));
        } else {
            v.start_rule(Some(r));
        }
        self.active_rule = true;
    }

    fn end_span(&mut self, v: &mut SourceVisitor) {
        if self.finished_span {
            return;
        }
        v.end_span();
        self.finished_span = true;
    }
}

/// `isBlockCall(visitor)`.
fn is_block_call(v: &SourceVisitor, s: &Selector) -> bool {
    if s.kind_ != SelectorKind::Method {
        return false;
    }
    let Node::MethodInvocation { argument_list, .. } = v.node(s.node) else {
        return false;
    };
    ArgumentListVisitor::new(v, *argument_list).has_block_arguments()
}

/// `_forcesSplit(expression)`.
fn forces_split(v: &SourceVisitor, expr: NodeId) -> bool {
    let mut e = expr;
    while let Node::ParenthesizedExpression { expression, .. } = v.node(e) {
        e = *expression;
    }
    if v.is_collection_literal(e) {
        return false;
    }
    if let Node::FunctionExpression { body, .. } = v.node(e) {
        let Node::BlockFunctionBody { block, .. } = v.node(*body) else {
            return false;
        };
        return match v.node(*block) {
            Node::Block { statements, .. } => statements.is_empty(),
            _ => false,
        };
    }
    let list = match v.node(e) {
        Node::MethodInvocation { argument_list, .. }
        | Node::InstanceCreationExpression { argument_list, .. }
        | Node::FunctionExpressionInvocation { argument_list, .. } => *argument_list,
        _ => return true,
    };
    let Node::ArgumentList { arguments, .. } = v.node(list) else {
        return true;
    };
    let Some(&last) = arguments.last() else {
        return true;
    };
    if v.has_comma_after(last) {
        return false;
    }
    let mut arg = last;
    if let Node::NamedExpression { expression, .. } = v.node(arg) {
        arg = *expression;
    }
    forces_split(v, arg)
}

/// `_unwrapNullAssertion(expression)`.
fn unwrap_null_assertion(v: &SourceVisitor, e: NodeId) -> NodeId {
    if let Node::PostfixExpression { operand, operator } = v.node(e)
        && v.tk.text(v.source, *operator) == "!"
    {
        return *operand;
    }
    e
}

/// `_unwrapTarget(node, calls)`.
fn unwrap_target(v: &SourceVisitor, node: NodeId, calls: &mut Vec<Selector>) -> NodeId {
    if v.looks_like_static_call(node) {
        return node;
    }
    match v.node(node) {
        Node::MethodInvocation {
            target: Some(t), ..
        } => unwrap_selector(v, *t, SelectorKind::Method, node, calls),
        Node::PropertyAccess {
            target: Some(t), ..
        } => unwrap_selector(v, *t, SelectorKind::Property, node, calls),
        Node::PrefixedIdentifier { prefix, .. } => {
            unwrap_selector(v, *prefix, SelectorKind::Prefixed, node, calls)
        }
        Node::IndexExpression {
            target: Some(t), ..
        } => unwrap_postfix(v, node, *t, calls),
        Node::FunctionExpressionInvocation { function, .. } => {
            unwrap_postfix(v, node, *function, calls)
        }
        Node::PostfixExpression { operand, operator } if v.tk.text(v.source, *operator) == "!" => {
            unwrap_postfix(v, node, *operand, calls)
        }
        _ => node,
    }
}

fn unwrap_postfix(
    v: &SourceVisitor,
    node: NodeId,
    target_: NodeId,
    calls: &mut Vec<Selector>,
) -> NodeId {
    let target_ = unwrap_target(v, target_, calls);
    if calls.is_empty() {
        return node;
    }
    calls.last_mut().expect("selector").postfixes.push(node);
    target_
}

fn unwrap_selector(
    v: &SourceVisitor,
    target_: NodeId,
    kind_: SelectorKind,
    node: NodeId,
    calls: &mut Vec<Selector>,
) -> NodeId {
    let target_ = unwrap_target(v, target_, calls);
    calls.push(Selector {
        kind_,
        node,
        postfixes: Vec::new(),
    });
    target_
}
