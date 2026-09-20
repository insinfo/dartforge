//! Expansão ordenada de with em aplicações abstratas, sem herdar via implements.
//! Corpos clonados conservam spans e contexto estático da declaração original;
//! a análise semântica deve validá-los somente no mixin de origem. Aplicações
//! validam contratos/overrides e são transparentes para restrições de biblioteca.
use dartforge_diagnostics::Diagnostic;
use dartforge_syntax::{Class, ClassKind, ClassModifier, Program};
use std::collections::{BTreeMap, BTreeSet};

/// Expande `C extends B with M, N` em `C -> aplicação N -> aplicação M -> B`.
///
/// Deve ocorrer antes da análise semântica, depois da resolução nominal de imports.
/// IDs sintéticos ficam acima de todos os IDs originais; o nome vazio não declara
/// símbolo na fonte. O passe é idempotente e nunca cria construtores invocáveis.
///
/// # Erros
/// Rejeita classes comuns usadas como mixin, bases não resolvidas e declarações
/// de mixin com superclass/with, recursos fora deste subconjunto. Modificadores,
/// conflitos de campos, contratos e privacidade continuam a cargo da semântica.
pub fn expand_mixins(program: &mut Program<'_>) -> Result<(), Diagnostic> {
    if program.classes.iter().all(|class| class.mixins.is_empty()) {
        return Ok(());
    }
    let original = &program.classes;
    let mut indices = BTreeMap::new();
    for (index, class) in original.iter().enumerate() {
        if indices.insert(class.id, index).is_some() {
            return Err(Diagnostic::new(
                "Duplicate class ID before mixin expansion",
                class.span,
            ));
        }
    }
    let mut next = original.iter().map(|class| class.id).max().unwrap_or(0);
    let mut rewritten = program.classes.clone();
    for (index, class) in original.iter().enumerate() {
        if class.mixins.is_empty() {
            continue;
        }
        if class.kind != ClassKind::Class || !class.enum_values.is_empty() {
            return Err(Diagnostic::new(
                "Only class declarations may apply with in this subset",
                class.span,
            ));
        }
        let mut parent = class.superclass;
        let mut base_contract = parent.is_some_and(|id| has_base_contract(id, original, &indices));
        for &mixin_id in &class.mixins {
            let mixin = indices
                .get(&mixin_id)
                .map(|index| &original[*index])
                .ok_or_else(|| Diagnostic::new("Mixin declaration was not resolved", class.span))?;
            if mixin.kind == ClassKind::Class || !mixin.enum_values.is_empty() {
                return Err(Diagnostic::new(
                    "Only mixin or mixin class can be applied with",
                    class.span,
                ));
            }
            if mixin.superclass.is_some() || !mixin.mixins.is_empty() {
                return Err(Diagnostic::new(
                    "Mixin superclass and nested with are unsupported",
                    mixin.span,
                ));
            }
            next = next
                .checked_add(1)
                .ok_or_else(|| Diagnostic::new("Synthetic class IDs exhausted", class.span))?;
            base_contract |= has_base_contract(mixin_id, original, &indices);
            rewritten.push(Class {
                id: next,
                name: "",
                span: class.span,
                is_mixin_application: true,
                mixin_origin: Some(mixin_id),
                modifier: if base_contract {
                    ClassModifier::Base
                } else {
                    ClassModifier::None
                },
                kind: ClassKind::Class,
                mixins: vec![],
                superclass: parent,
                interfaces: vec![mixin_id],
                fields: mixin.fields.clone(),
                methods: mixin.methods.clone(),
                abstract_methods: mixin.abstract_methods.clone(),
                is_abstract: true,
                is_interface: false,
                library_id: class.library_id,
                enum_values: vec![],
                enum_arguments: vec![],
                enum_constructor_fields: vec![],
            });
            parent = Some(next);
        }
        rewritten[index].superclass = parent;
        rewritten[index].mixins.clear();
    }
    program.classes = rewritten;
    Ok(())
}

/// Propaga base/final por superclasses e mixins sem recursão, mesmo antes da expansão.
fn has_base_contract(id: u32, classes: &[Class<'_>], indices: &BTreeMap<u32, usize>) -> bool {
    let mut pending = vec![id];
    let mut visited = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !visited.insert(id) {
            continue;
        }
        if let Some(class) = indices.get(&id).map(|index| &classes[*index]) {
            if matches!(class.modifier, ClassModifier::Base | ClassModifier::Final) {
                return true;
            }
            pending.extend(class.superclass);
            pending.extend(&class.mixins);
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Exercita parser real sem depender da análise semântica em edição paralela.
    fn parse(source: &str) -> Program<'_> {
        let tokens = dartforge_lexer::lex(source).unwrap();
        dartforge_parser::parse(&tokens, source.len()).unwrap()
    }
    /// IDs, contratos e a ordem escrita permanecem estáveis após a expansão.
    #[test]
    fn ordered_applications_are_abstract_and_idempotent() {
        let mut program = parse(
            "class B{} mixin M{int first=1;} mixin N{int last=2;} class C extends B with M,N{} void main(){}",
        );
        expand_mixins(&mut program).unwrap();
        assert_eq!(program.classes.len(), 6);
        assert_eq!(program.classes[3].superclass, Some(5));
        assert_eq!(program.classes[5].superclass, Some(4));
        assert_eq!(program.classes[4].superclass, Some(0));
        assert_eq!(program.classes[4].interfaces, vec![1]);
        assert_eq!(program.classes[5].interfaces, vec![2]);
        assert!(program.classes[4].is_abstract && program.classes[4].is_mixin_application);
        assert_eq!(
            program.classes[4].fields[0].span,
            program.classes[1].fields[0].span
        );
        expand_mixins(&mut program).unwrap();
        assert_eq!(program.classes.len(), 6);
    }
    /// Falhas não deixam o programa parcialmente reescrito.
    #[test]
    fn ordinary_classes_cannot_be_applied() {
        let mut program = parse("class M{} class C with M{} void main(){}");
        assert!(expand_mixins(&mut program).is_err());
        assert_eq!(program.classes.len(), 2);
        assert_eq!(program.classes[1].mixins, vec![0]);
    }
}
