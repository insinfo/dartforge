//! Port of `shadow_css.dart` from ngcompiler 3.0.0-dev.3
//! (`lib/v1/src/compiler/stylesheet_compiler/shadow_css.dart`): the shim for
//! emulated style encapsulation.
//!
//! `shimShadowCss` reads the stylesheet with the csslib parser
//! ([`crate::csslib`]), rewrites each selector group (`_ShadowTransformer`) and
//! prints the compact tree (`CssPrinter`). A parse error is only a warning in
//! the official code (`logWarning`) and the output is produced anyway; an
//! exception thrown along the way brings the builder down, and here becomes
//! [`Exception`].
//!
//! The `_LegacyShadowTransformer` (option `use_legacy_style_encapsulation`) is
//! not ported: the option is false by default and the generator does not read
//! it.
use std::rc::Rc;

use crate::csslib::printer::CssPrinter;
use crate::csslib::token_kind as tk;
use crate::csslib::tree::*;
use crate::csslib::visitor::{R, Visitor};
use crate::csslib::{self, Exception};

/// The stack of the thread that runs the shim: the parser, the visitor and the
/// printer are recursive like the official ones, and the nesting goes up to the
/// parser's limit (`csslib::parser`), well beyond what a thread's default stack
/// holds. The memory is only used as deep as the nesting goes.
const STACK_SIZE: usize = 64 << 20;

/// `shimShadowCss(css, contentClass, hostClass)`, without the legacy
/// encapsulation.
pub fn shim_shadow_css(
    css: &str,
    content_class: &str,
    host_class: &str,
) -> Result<String, Exception> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("shim-css".into())
            .stack_size(STACK_SIZE)
            .spawn_scoped(scope, || shim_in_thread(css, content_class, host_class))
            .map_err(|_| Exception("no thread for the shim"))?
            .join()
            .map_err(|_| Exception("panic in the shim"))?
    })
}

/// The body of `shimShadowCss`.
fn shim_in_thread(css: &str, content_class: &str, host_class: &str) -> Result<String, Exception> {
    let mut sheet = csslib::parse(css)?;
    let mut transformer = ShadowTransformer {
        content_class,
        host_class,
    };
    transformer.visit_tree(&mut sheet)?;
    let mut printer = CssPrinter::default();
    printer.visit_tree(&mut sheet)?;
    Ok(printer.text())
}

/// `_isHost`: `:host`.
fn is_host(s: &SimpleSelector) -> bool {
    s.is_pseudo_class() && s.name() == "host"
}

/// `_isHostFunction`: `:host()`.
fn is_host_function(s: &SimpleSelector) -> bool {
    s.is_pseudo_class_function() && s.name() == "host"
}

/// `_isHostContextFunction`: `:host-context()`.
fn is_host_context_function(s: &SimpleSelector) -> bool {
    s.is_pseudo_class_function() && s.name() == "host-context"
}

/// `_isGlobalContextFunction`.
fn is_global_context_function(s: &SimpleSelector) -> bool {
    s.is_pseudo_class_function() && s.name() == "global-context"
}

/// `PseudoClassFunctionSelector.selector` (`argument as Selector`).
fn selector_argument(s: &SimpleSelector) -> R<Rc<Selector>> {
    match &s.kind {
        SimpleKind::PseudoClassFunction(PseudoArgument::Selector(sel)) => Ok(Rc::clone(sel)),
        _ => Err(Exception("TypeError")),
    }
}

/// `_clone`: a copy of the sequences (the simple selectors are the same).
fn clone_all(it: &[Rc<Sequence>]) -> Vec<Rc<Sequence>> {
    it.iter().map(|s| s.duplicate()).collect()
}

/// `_createElementSelectorSequence`.
fn create_element_selector_sequence(name: &str) -> Rc<Sequence> {
    Sequence::new(
        SimpleSelector::created(SimpleKind::Element, name),
        tk::COMBINATOR_NONE,
    )
}

/// `_createClassSelectorSequence`.
fn create_class_selector_sequence(name: &str) -> Rc<Sequence> {
    Sequence::new(
        SimpleSelector::created(SimpleKind::Class, name),
        tk::COMBINATOR_NONE,
    )
}

/// `_createPseudoClassSelectorSequence`.
fn create_pseudo_class_selector_sequence(name: &str) -> Rc<Sequence> {
    Sequence::new(
        SimpleSelector::created(SimpleKind::PseudoClass, name),
        tk::COMBINATOR_NONE,
    )
}

/// `_ComplexSelector`: seletores compostos separados por combinadores.
struct ComplexSelector {
    compound_selectors: Vec<CompoundSelector>,
}

impl ComplexSelector {
    /// `_ComplexSelector.from`.
    fn from(selector: &Selector) -> R<Self> {
        let mut compound_selectors = Vec::new();
        let seqs = &selector.sequences;
        if !seqs.is_empty() {
            let mut start = 0;
            for i in 1..=seqs.len() {
                if i == seqs.len() || !seqs[i].is_combinator_none() {
                    compound_selectors.push(CompoundSelector::from(&seqs[start..i])?);
                    start = i;
                }
            }
        }
        Ok(ComplexSelector { compound_selectors })
    }

    /// `containsHostContext`.
    fn contains_host_context(&self) -> bool {
        self.compound_selectors
            .iter()
            .any(|c| c.contains_host_context())
    }

    /// `toSelector`.
    fn build_selector(&mut self) -> Selector {
        let mut sequences = Vec::new();
        for c in &mut self.compound_selectors {
            sequences.extend(c.fixed_sequences().iter().cloned());
        }
        Selector { sequences }
    }
}

/// `_CompoundSelector`: a sequence of simple selectors, in the order of a valid
/// compound (type first, pseudo-element last).
struct CompoundSelector {
    combinator: i32,
    sequences: Vec<Rc<Sequence>>,
}

impl CompoundSelector {
    /// `_CompoundSelector()`.
    fn new(combinator: i32) -> Self {
        CompoundSelector {
            combinator,
            sequences: Vec::new(),
        }
    }

    /// `_CompoundSelector.from`.
    fn from(sequences: &[Rc<Sequence>]) -> R<Self> {
        let combinator = sequences
            .first()
            .map(|s| s.combinator.get())
            .unwrap_or(tk::COMBINATOR_NONE);
        let mut c = CompoundSelector::new(combinator);
        c.add_all(sequences.to_vec())?;
        Ok(c)
    }

    /// `containsHost`.
    fn contains_host(&self) -> bool {
        self.sequences.iter().any(|s| is_host(&s.selector))
    }

    /// `containsHostContext`.
    fn contains_host_context(&self) -> bool {
        self.sequences
            .iter()
            .any(|s| is_host_context_function(&s.selector))
    }

    /// `containsGlobalContext`.
    fn contains_global_context(&self) -> bool {
        self.sequences
            .iter()
            .any(|s| is_global_context_function(&s.selector))
    }

    /// `removeIfNgDeep`: `::ng-deep` becomes an empty type selector, which
    /// holds the combinator.
    fn remove_if_ng_deep(&mut self) -> bool {
        let Some(first) = self.sequences.first() else {
            return false;
        };
        let s = &first.selector;
        if s.is_pseudo_element() && s.name() == "ng-deep" {
            let seq = create_element_selector_sequence("");
            seq.combinator.set(self.combinator);
            self.sequences = vec![seq];
            return true;
        }
        false
    }

    /// `_compare`: negative if `a` comes before `b`, positive if after. The
    /// warning about two type selectors reads `span!` — null for the ones
    /// created by the shim.
    fn compare(a: &Sequence, b: &Sequence) -> R<i32> {
        let x = &a.selector;
        let y = &b.selector;
        let aviso = || {
            if x.has_span && y.has_span {
                Ok(0)
            } else {
                Err(Exception("TypeError"))
            }
        };
        if (x.is_element() && y.is_element()) || (x.is_namespace() && y.is_namespace()) {
            // "Compound selector contains multiple type selectors"
            aviso()
        } else if x.is_pseudo_element() && y.is_pseudo_element() {
            // "Compound selector contains multiple pseudo element selectors"
            aviso()
        } else if x.is_pseudo_element() || y.is_element() || y.is_namespace() {
            Ok(1)
        } else if y.is_pseudo_element() || x.is_element() || x.is_namespace() {
            Ok(-1)
        } else {
            Ok(0)
        }
    }

    /// `append`.
    fn append(&mut self, seq: Rc<Sequence>) {
        self.sequences.push(seq);
    }

    /// `add`: before the first one that must come after it.
    fn add(&mut self, seq: Rc<Sequence>) -> R<()> {
        let mut i = 0;
        while i < self.sequences.len() {
            if Self::compare(&seq, &self.sequences[i])? < 0 {
                break;
            }
            i += 1;
        }
        self.sequences.insert(i, seq);
        Ok(())
    }

    /// `addAll`: interleaves, keeping the order of each side.
    fn add_all(&mut self, additions: Vec<Rc<Sequence>>) -> R<()> {
        let mut new_declarations = Vec::with_capacity(self.sequences.len() + additions.len());
        let mut current_ones = std::mem::take(&mut self.sequences).into_iter().peekable();
        let mut additions = additions.into_iter().peekable();
        while let (Some(current), Some(addition)) = (current_ones.peek(), additions.peek()) {
            if Self::compare(addition, current)? < 0 {
                new_declarations.extend(additions.next());
            } else {
                new_declarations.extend(current_ones.next());
            }
        }
        new_declarations.extend(current_ones);
        new_declarations.extend(additions);
        self.sequences = new_declarations;
        Ok(())
    }

    /// `clone`.
    fn duplicate(&self) -> Self {
        CompoundSelector {
            combinator: self.combinator,
            sequences: clone_all(&self.sequences),
        }
    }

    /// `toSequences`: fixes the combinators (the compound's on the first, none
    /// on the others) and returns the list.
    fn fixed_sequences(&mut self) -> &[Rc<Sequence>] {
        if !self.sequences.is_empty() {
            for s in &self.sequences {
                s.combinator.set(tk::COMBINATOR_NONE);
            }
            self.sequences[0].combinator.set(self.combinator);
        }
        &self.sequences
    }
}

/// `_Indices`.
struct Indices {
    /// First compound after a combinator that crosses the shadow boundary.
    deep_index: usize,
    /// Last compound with a host selector (`-1`: none).
    host_index: isize,
}

/// `_ShadowTransformer`.
struct ShadowTransformer<'a> {
    content_class: &'a str,
    host_class: &'a str,
}

impl ShadowTransformer<'_> {
    /// `_createDescendantHostSelectorFor`: for `:host-context()`, the selector
    /// in which the host is a descendant of the argument.
    ///
    ///   `:host-context(.x) > .y`  =>  `.x :host > .y`
    fn create_descendant_host_selector_for(
        &self,
        selector: &mut ComplexSelector,
    ) -> R<ComplexSelector> {
        let mut new = ComplexSelector {
            compound_selectors: Vec::new(),
        };
        for compound in &mut selector.compound_selectors {
            if compound.contains_host_context() {
                let mut ancestral = CompoundSelector::new(compound.combinator);
                let mut descendant = CompoundSelector::new(tk::COMBINATOR_DESCENDANT);
                let sequences = clone_all(compound.fixed_sequences());
                for seq in sequences {
                    if is_host_context_function(&seq.selector) {
                        // No copy: the argument's sequences go in as they are.
                        let arg = selector_argument(&seq.selector)?;
                        ancestral.add_all(arg.sequences.clone())?;
                    } else {
                        descendant.append(seq);
                    }
                }
                descendant.add(create_pseudo_class_selector_sequence("host"))?;
                new.compound_selectors.push(ancestral);
                new.compound_selectors.push(descendant);
            } else {
                new.compound_selectors.push(compound.duplicate());
            }
        }
        Ok(new)
    }

    /// `shimDeepCombinators`: replaces `::ng-deep` with a descendant combinator
    /// and records the index.
    fn shim_deep_combinators(&self, selector: &mut ComplexSelector, indices: &mut Indices) {
        for i in (0..selector.compound_selectors.len()).rev() {
            if selector.compound_selectors[i].remove_if_ng_deep() {
                indices.deep_index = i;
            }
        }
    }

    /// `shimSelectors`.
    ///
    ///   `:host(.x) > .y ::ng-deep .z`  =>  `.x.host > .y.content .z`
    fn shim_selectors(&self, selector: &mut ComplexSelector) -> R<()> {
        let mut indices = Indices {
            deep_index: selector.compound_selectors.len(),
            host_index: -1,
        };
        self.shim_deep_combinators(selector, &mut indices);

        // Scope everything between the last host selector and the first
        // `::ng-deep`.
        let mut i = indices.deep_index as isize - 1;
        while i > indices.host_index {
            let compound = &mut selector.compound_selectors[i as usize];
            if compound.contains_host()
                || compound.contains_host_context()
                || compound.contains_global_context()
            {
                indices.host_index = i;
            } else {
                compound.add(create_class_selector_sequence(self.content_class))?;
            }
            i -= 1;
        }

        // The host selectors.
        let mut i = indices.host_index;
        while i >= 0 {
            let compound = &mut selector.compound_selectors[i as usize];
            let mut j = 0;
            while j < compound.sequences.len() {
                let s = Rc::clone(&compound.sequences[j].selector);
                if is_host_function(&s) || is_host_context_function(&s) {
                    // :host() or :host-context() => the host class, and the
                    // argument goes into the compound.
                    compound.sequences[j] = create_class_selector_sequence(self.host_class);
                    let arg = selector_argument(&s)?;
                    compound.add_all(clone_all(&arg.sequences))?;
                } else if is_host(&s) {
                    compound.sequences[j] = create_class_selector_sequence(self.host_class);
                } else if is_global_context_function(&s) {
                    // The pseudo disappears and the context selector remains.
                    compound.sequences.remove(j);
                    let arg = selector_argument(&s)?;
                    compound.add_all(clone_all(&arg.sequences))?;
                }
                j += 1;
            }
            i -= 1;
        }
        Ok(())
    }
}

impl Visitor for ShadowTransformer<'_> {
    /// `visitSelectorGroup`: each selector becomes its shim; with
    /// `:host-context()`, also the one of the descendant host.
    fn visit_selector_group(&mut self, g: &mut SelectorGroup) -> R<()> {
        let mut complexes = Vec::new();
        for s in &g.selectors {
            let mut complex = ComplexSelector::from(s)?;
            let has_context = complex.contains_host_context();
            let descendant = if has_context {
                Some(self.create_descendant_host_selector_for(&mut complex)?)
            } else {
                None
            };
            complexes.push(complex);
            complexes.extend(descendant);
        }
        g.selectors.clear();
        for mut c in complexes {
            self.shim_selectors(&mut c)?;
            g.selectors.push(c.build_selector());
        }
        Ok(())
    }
}
