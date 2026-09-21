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
            // `mixin M on Base` só pode ser aplicado onde `Base` já está na
            // cadeia de superclasses. A verificação acontece aqui porque é
            // exatamente aqui que a cadeia da aplicação fica conhecida: `parent`
            // já é o topo do que foi aplicado antes deste mixin.
            if let Some(constraint) = mixin.mixin_constraint
                && !chain_satisfies(parent, constraint, &rewritten)
            {
                return Err(Diagnostic::new(
                    format!(
                        "Mixin '{}' constrains on '{}', which is not in the superclass chain here",
                        mixin.name,
                        indices
                            .get(&constraint)
                            .map_or("?", |index| original[*index].name)
                    ),
                    class.span,
                ));
            }
            next = next
                .checked_add(1)
                .ok_or_else(|| Diagnostic::new("Synthetic class IDs exhausted", class.span))?;
            base_contract |= has_base_contract(mixin_id, original, &indices);
            rewritten.push(Class {
                mixin_constraint: None,
                factories: vec![],
                constructor: None,
                constructor_extras: None,
                named_constructors: vec![],
                static_fields: vec![],
                static_methods: vec![],
                is_library_globals: false,
                annotations: vec![],
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
                type_parameters: vec![],
            });
            parent = Some(next);
        }
        rewritten[index].superclass = parent;
        rewritten[index].mixins.clear();
    }
    program.classes = rewritten;
    Ok(())
}

/// Indica se a cadeia iniciada em `start` contém a restrição `on` exigida.
///
/// Percorre apenas ligações de superclasse, como manda a regra do Dart: uma
/// classe que apenas `implements Base` não satisfaz `on Base`, porque a
/// restrição existe para garantir que os membros de `Base` estarão lá em
/// execução. Uma aplicação sintética conta quando o mixin que a originou é a
/// própria restrição, o que cobre `class C extends B with A, M` com
/// `mixin M on A`.
fn chain_satisfies(start: Option<u32>, constraint: u32, classes: &[Class<'_>]) -> bool {
    let mut current = start;
    let mut visited = BTreeSet::new();
    while let Some(id) = current {
        if !visited.insert(id) {
            return false;
        }
        if id == constraint {
            return true;
        }
        let Some(class) = classes.iter().find(|class| class.id == id) else {
            return false;
        };
        if class.is_mixin_application && class.mixin_origin == Some(constraint) {
            return true;
        }
        current = class.superclass;
    }
    false
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
    /// A clonagem dos métodos conserva async e expressões await na aplicação sintética.
    #[test]
    fn mixin_applications_preserve_async_methods() {
        let mut program = parse(
            "mixin M{Future<int> value() async=>await Future<int>.value(1);}class C with M{}Future<void> main() async{}",
        );
        expand_mixins(&mut program).unwrap();
        assert!(program.main_is_async);
        let application = program
            .classes
            .iter()
            .find(|class| class.is_mixin_application)
            .unwrap();
        assert!(application.methods[0].is_async);
        assert!(matches!(
            application.methods[0].body[0].kind,
            dartforge_syntax::StatementKind::Return(Some(dartforge_syntax::Expr {
                kind: dartforge_syntax::ExprKind::Await(_),
                ..
            }))
        ));
    }
}
