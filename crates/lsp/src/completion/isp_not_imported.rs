//! The operations of the analysis server 3.6.2 `NotImportedCompletionPass`
//! (`AS:src/services/completion/dart/not_imported_completion_pass.dart`):
//! `StaticMembersOperation` (`addNotImportedTopLevelDeclarations`),
//! `ConstructorsOperation` (`addNotImportedConstructors`) and
//! `InstanceExtensionMembersOperation` (`addNotImportedExtensionMethods`),
//! through the pass's own `DeclarationHelper` (its options and its
//! `VisibilityTracker`), over a library not imported yet that the query has
//! already loaded. Each new item carries the `import` that makes it visible.
//! Written without compiling or running (2026-10-05).

use super::isp::{Isp, Operation};
use super::isp_declarations::Import;
use super::*;
use dartforge_elements::model::ExtensionId;

impl<'a, 'c> Isp<'a, 'c> {
    /// Runs the operations registered for library `lib`, imported through the
    /// URI `uri`.
    pub(super) fn operations_not_imported(&mut self, lib: LibraryId, uri: &str) {
        use dartforge_elements::model::ClassKind as K;
        let imp = Import { prefix: None, not_imported: true };
        let before = self.collector.items.len();
        let operations = self.operations.clone();
        for op in operations {
            match op {
                Operation::StaticMembers => {
                    let p = &self.query.program;
                    let mut space: Vec<(SymbolId, dartforge_elements::model::Binding)> = p.library(lib).exported.iter().map(|(&s, &b)| (s, b)).collect();
                    space.sort_by(|a, b| self.query.name(a.0).cmp(self.query.name(b.0)));
                    self.external_declarations(&space, &imp);
                }
                Operation::Constructors => {
                    let units = self.query.program.library(lib).units.clone();
                    for u in units {
                        let e = self.unit_elements(u);
                        for c in e.classes {
                            if self.query.program.class(c).kind == K::MixinApplication && self.query.program.class(c).decl.is_none() {
                                continue;
                            }
                            let abstract_ = self.query.program.class(c).modifiers.abstract_;
                            self.suggest_constructors(c, Some(&imp), !abstract_);
                        }
                        for c in e.enums {
                            self.suggest_constructors(c, Some(&imp), true);
                        }
                        for c in e.extension_types {
                            self.suggest_constructors(c, Some(&imp), true);
                        }
                        for t in e.typedefs {
                            self.alias_constructors(t, Some(&imp));
                        }
                    }
                }
                Operation::ExtensionMembers { ty, methods, setters, .. } => {
                    // `applicableTo(targetType: Null ? ty : promoteToNonNull(ty))`.
                    let target = if matches!(self.query.table.get(ty), Type::Null) { ty } else { dartforge_types::non_nullable(ty, &mut self.query.table) };
                    let p = &self.query.program;
                    let mut exported: Vec<(String, ExtensionId)> = p
                        .library(lib)
                        .exported
                        .iter()
                        .filter_map(|(s, b)| match b.getter {
                            Some(Element::Extension(x)) => Some((self.query.name(*s).to_string(), x)),
                            _ => None,
                        })
                        .collect();
                    exported.sort();
                    let candidates: Vec<ExtensionId> = exported.into_iter().map(|(_, x)| x).collect();
                    let applicable = {
                        let Query { program, names, table, core, outline, .. } = &mut *self.query;
                        dartforge_types::extensoes_aplicaveis_dentre(program, names, table, core, outline, &candidates, target)
                    };
                    let library = self.library();
                    for (x, _) in applicable {
                        let ext = self.query.program.extension(x);
                        let visible = ext.library == library || ext.name.is_some_and(|n| !self.query.name(n).starts_with('_'));
                        if visible {
                            self.not_imported_extension_members(x, &imp, methods, setters);
                        }
                    }
                }
            }
        }
        // The `import` that makes the item visible (`requiredImports`).
        let (span, text) = crate::actions::insert_import(self.source, self.cu, uri);
        for i in &mut self.collector.items[before..] {
            i.import = Some(AutoImport { uri: uri.to_string(), span, text: text.clone() });
            // The list `detail` is the element's; the `Auto import from` comes in
            // `completionItem/resolve` (`resolveDartCompletion`).
            i.rel.not_imported = true;
        }
    }

    /// `addMembersFromExtensionElement`: the instance methods and accessors,
    /// with the interface of the extended type as the reference for the
    /// inheritance distance.
    fn not_imported_extension_members(&mut self, x: ExtensionId, imp: &Import, methods: bool, setters: bool) {
        let on = self.query.outline.extensions[x.0 as usize].on;
        let reference = match self.query.table.get(on) {
            Type::Interface { class, .. } => Some(*class),
            _ => None,
        };
        let (accessors, _, ms) = self.declared_members(None, Some(x));
        let _ = (methods, setters);
        for m in ms {
            if !self.query.program.function(m).static_ {
                self.suggest_method(m, None, false, Some(imp), reference);
            }
        }
        for f in accessors {
            if !self.query.program.function(f).static_ {
                self.suggest_property(f, None, false, Some(imp), reference);
            }
        }
    }
}
