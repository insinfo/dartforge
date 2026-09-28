use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    sync::Arc,
};

use codemap::{Span, Spanned};

use crate::{
    ast::Mixin,
    builtin::GLOBAL_FUNCTIONS,
    common::Identifier,
    error::SassResult,
    value::{SassFunction, Value},
};

#[allow(clippy::type_complexity)]
#[derive(Debug, Default, Clone)]
pub(crate) struct Scopes {
    pub(crate) variables: Arc<RefCell<Vec<Arc<RefCell<BTreeMap<Identifier, Value>>>>>>,
    pub(crate) mixins: Arc<RefCell<Vec<Arc<RefCell<BTreeMap<Identifier, Mixin>>>>>>,
    pub(crate) functions: Arc<RefCell<Vec<Arc<RefCell<BTreeMap<Identifier, SassFunction>>>>>>,
    len: Arc<Cell<usize>>,
    pub last_variable_index: Option<(Identifier, usize)>,
    /// O nó de cada variável (`_variableNodes` do `Environment` do
    /// dart-sass): o `span` da expressão que lhe deu o valor, para o
    /// `valueSpanForMap` das declarações. Paralelo a `variables`.
    pub(crate) nodes: Arc<RefCell<Vec<Arc<RefCell<BTreeMap<Identifier, Span>>>>>>,
}

impl Scopes {
    pub fn new() -> Self {
        Self {
            variables: Arc::new(RefCell::new(vec![Arc::new(RefCell::new(BTreeMap::new()))])),
            mixins: Arc::new(RefCell::new(vec![Arc::new(RefCell::new(BTreeMap::new()))])),
            functions: Arc::new(RefCell::new(vec![Arc::new(RefCell::new(BTreeMap::new()))])),
            len: Arc::new(Cell::new(1)),
            last_variable_index: None,
            nodes: Arc::new(RefCell::new(vec![Arc::new(RefCell::new(BTreeMap::new()))])),
        }
    }

    pub fn new_closure(&self) -> Self {
        debug_assert_eq!(self.len(), (*self.variables).borrow().len());
        Self {
            variables: Arc::new(RefCell::new(
                (*self.variables).borrow().iter().map(Arc::clone).collect(),
            )),
            mixins: Arc::new(RefCell::new(
                (*self.mixins).borrow().iter().map(Arc::clone).collect(),
            )),
            functions: Arc::new(RefCell::new(
                (*self.functions).borrow().iter().map(Arc::clone).collect(),
            )),
            len: Arc::new(Cell::new(self.len())),
            last_variable_index: self.last_variable_index,
            nodes: Arc::new(RefCell::new(
                (*self.nodes).borrow().iter().map(Arc::clone).collect(),
            )),
        }
    }

    pub fn global_variables(&self) -> Arc<RefCell<BTreeMap<Identifier, Value>>> {
        debug_assert_eq!(self.len(), (*self.variables).borrow().len());
        Arc::clone(&(*self.variables).borrow()[0])
    }

    pub fn global_functions(&self) -> Arc<RefCell<BTreeMap<Identifier, SassFunction>>> {
        Arc::clone(&(*self.functions).borrow()[0])
    }

    pub fn global_mixins(&self) -> Arc<RefCell<BTreeMap<Identifier, Mixin>>> {
        Arc::clone(&(*self.mixins).borrow()[0])
    }

    pub fn find_var(&mut self, name: Identifier) -> Option<usize> {
        debug_assert_eq!(self.len(), (*self.variables).borrow().len());

        match self.last_variable_index {
            Some((prev_name, idx)) if prev_name == name => return Some(idx),
            _ => {}
        };

        for (idx, scope) in (*self.variables).borrow().iter().enumerate().rev() {
            if (**scope).borrow().contains_key(&name) {
                self.last_variable_index = Some((name, idx));
                return Some(idx);
            }
        }

        None
    }

    pub fn len(&self) -> usize {
        (*self.len).get()
    }

    pub fn enter_new_scope(&mut self) {
        let len = self.len();
        debug_assert_eq!(self.len(), (*self.variables).borrow().len());
        (*self.len).set(len + 1);
        (*self.variables)
            .borrow_mut()
            .push(Arc::new(RefCell::new(BTreeMap::new())));
        (*self.nodes)
            .borrow_mut()
            .push(Arc::new(RefCell::new(BTreeMap::new())));
        (*self.mixins)
            .borrow_mut()
            .push(Arc::new(RefCell::new(BTreeMap::new())));
        (*self.functions)
            .borrow_mut()
            .push(Arc::new(RefCell::new(BTreeMap::new())));
    }

    pub fn exit_scope(&mut self) {
        debug_assert_eq!(self.len(), (*self.variables).borrow().len());
        let len = self.len();
        (*self.len).set(len - 1);
        (*self.variables).borrow_mut().pop();
        (*self.nodes).borrow_mut().pop();
        (*self.mixins).borrow_mut().pop();
        (*self.functions).borrow_mut().pop();
        self.last_variable_index = None;
    }
}

/// Variables
impl Scopes {
    /// Guarda (ou apaga) o nó da variável `name` no escopo `idx`.
    pub fn set_node(&mut self, idx: usize, name: Identifier, node: Option<Span>) {
        let nodes = (*self.nodes).borrow();
        let Some(scope) = nodes.get(idx) else { return };
        let mut scope = (**scope).borrow_mut();
        match node {
            Some(span) => {
                scope.insert(name, span);
            }
            None => {
                scope.remove(&name);
            }
        }
    }

    /// O nó da variável visível `name` (`getVariableNode`).
    pub fn get_node(&self, name: Identifier) -> Option<Span> {
        let vars = (*self.variables).borrow();
        let nodes = (*self.nodes).borrow();
        for (idx, scope) in vars.iter().enumerate().rev() {
            if (**scope).borrow().contains_key(&name) {
                return nodes
                    .get(idx)
                    .and_then(|n| (**n).borrow().get(&name).copied());
            }
        }
        None
    }

    pub fn insert_var(&mut self, idx: usize, name: Identifier, v: Value) -> Option<Value> {
        debug_assert_eq!(self.len(), (*self.variables).borrow().len());
        (*(*self.variables).borrow_mut()[idx])
            .borrow_mut()
            .insert(name, v)
    }

    /// Always insert this variable into the innermost scope
    ///
    /// Used, for example, for variables from `@each` and `@for`
    pub fn insert_var_last(
        &mut self,
        name: Identifier,
        v: Value,
        node: Option<Span>,
    ) -> Option<Value> {
        debug_assert_eq!(self.len(), (*self.variables).borrow().len());
        let last_idx = self.len() - 1;
        self.set_node(last_idx, name, node);
        self.last_variable_index = Some((name, last_idx));
        (*(*self.variables).borrow_mut()[last_idx])
            .borrow_mut()
            .insert(name, v)
    }

    pub fn get_var(&mut self, name: Spanned<Identifier>) -> SassResult<Value> {
        debug_assert_eq!(self.len(), (*self.variables).borrow().len());

        match self.last_variable_index {
            Some((prev_name, idx)) if prev_name == name.node => {
                return Ok((*(*self.variables).borrow()[idx]).borrow()[&name.node].clone());
            }
            _ => {}
        };

        for (idx, scope) in (*self.variables).borrow().iter().enumerate().rev() {
            match (**scope).borrow().get(&name.node) {
                Some(var) => {
                    self.last_variable_index = Some((name.node, idx));
                    return Ok(var.clone());
                }
                None => continue,
            }
        }

        Err(("Undefined variable.", name.span).into())
    }

    pub fn var_exists(&self, name: Identifier) -> bool {
        debug_assert_eq!(self.len(), (*self.variables).borrow().len());
        for scope in (*self.variables).borrow().iter() {
            if (**scope).borrow().contains_key(&name) {
                return true;
            }
        }

        false
    }

    pub fn global_var_exists(&self, name: Identifier) -> bool {
        self.global_variables().borrow().contains_key(&name)
    }
}

/// Mixins
impl Scopes {
    pub fn insert_mixin(&mut self, name: Identifier, mixin: Mixin) {
        debug_assert_eq!(self.len(), (*self.variables).borrow().len());
        (*(*self.mixins).borrow_mut().last_mut().unwrap())
            .borrow_mut()
            .insert(name, mixin);
    }

    pub fn get_mixin(&self, name: Spanned<Identifier>) -> SassResult<Mixin> {
        debug_assert_eq!(self.len(), (*self.variables).borrow().len());
        for scope in (*self.mixins).borrow().iter().rev() {
            match (**scope).borrow().get(&name.node) {
                Some(mixin) => return Ok(mixin.clone()),
                None => continue,
            }
        }

        Err(("Undefined mixin.", name.span).into())
    }

    pub fn mixin_exists(&self, name: Identifier) -> bool {
        debug_assert_eq!(self.len(), (*self.variables).borrow().len());
        for scope in (*self.mixins).borrow().iter() {
            if (**scope).borrow().contains_key(&name) {
                return true;
            }
        }

        false
    }
}

/// Functions
impl Scopes {
    pub fn insert_fn(&mut self, func: SassFunction) {
        debug_assert_eq!(self.len(), (*self.variables).borrow().len());
        (*(*self.functions).borrow_mut().last_mut().unwrap())
            .borrow_mut()
            .insert(func.name(), func);
    }

    pub fn get_fn(&self, name: Identifier) -> Option<SassFunction> {
        debug_assert_eq!(self.len(), (*self.variables).borrow().len());
        for scope in (*self.functions).borrow().iter().rev() {
            let func = (**scope).borrow().get(&name).cloned();

            if func.is_some() {
                return func;
            }
        }

        None
    }

    pub fn fn_exists(&self, name: Identifier) -> bool {
        debug_assert_eq!(self.len(), (*self.variables).borrow().len());
        for scope in (*self.functions).borrow().iter() {
            if (**scope).borrow().contains_key(&name) {
                return true;
            }
        }

        GLOBAL_FUNCTIONS.contains_key(name.as_str())
    }
}
