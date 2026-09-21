//! Membros estáticos de classe e variáveis de topo da biblioteca.
//!
//! Estáticos não participam de herança nem de despacho dinâmico: `C.v` é sempre
//! resolvido pelo nome escrito da declaração e uma subclasse não o recebe como
//! membro de instância. Variáveis de topo seguem a mesma regra de resolução e
//! são inicializadas preguiçosamente, como no Dart 3.6.2.
use super::*;

impl<'a> Validator<'a> {
    /// Confere o inicializador de um estático ou de uma variável de topo.
    ///
    /// Um `final` ou `const` exige valor escrito; um mutável não anulável
    /// também, porque o subconjunto não tem `late`. Um mutável anulável sem
    /// valor recebe `null`, como em Dart.
    pub(super) fn validate_static_initializer(
        &mut self,
        member: &dartforge_syntax::StaticField<'a>,
    ) -> Result<(), Diagnostic> {
        let Some(initializer) = &member.initializer else {
            if !member.is_late
                && (member.is_final || member.is_const || !self.may_be_null(member.ty))
            {
                return Err(Diagnostic::new(
                    "Static and top-level variables require an initializer unless the type is nullable",
                    member.span,
                ));
            }
            return Ok(());
        };
        // Um inicializador estático nunca vê `this`, nem membros de instância.
        let class = self.current_class.take();
        let result = self
            .value_expected(initializer, Some(member.ty))
            .and_then(|actual| self.require_type(actual, member.ty, initializer.span));
        self.current_class = class;
        result
    }
    /// Avalia o valor de um `const` já tipado e registra o resultado canônico.
    ///
    /// # Erros
    /// Recusa referências a declarações `const` escritas depois desta, porque a
    /// avaliação segue a ordem do arquivo e não resolve ciclos.
    pub(super) fn constant_initializer(
        &self,
        member: &dartforge_syntax::StaticField<'a>,
    ) -> Result<ConstValue, Diagnostic> {
        let initializer = member.initializer.as_ref().ok_or_else(|| {
            Diagnostic::new("A const declaration requires an initializer", member.span)
        })?;
        self.evaluate_constant(initializer)
    }
    /// Procura uma variável de topo pelo nome escrito.
    pub(super) fn global(&self, name: &str) -> Option<&StaticInfo<'a>> {
        find_by_name(&self.globals, name, |global| global.name)
    }
    /// Procura um campo estático declarado exatamente na classe indicada.
    ///
    /// Não percorre a superclasse: Dart não herda membros estáticos.
    pub(super) fn static_field(&self, class: u32, name: &str) -> Option<&StaticInfo<'a>> {
        find_by_name(&self.classes.get(&class)?.static_fields, name, |field| {
            field.name
        })
    }
    /// Procura um método estático declarado exatamente na classe indicada.
    pub(super) fn static_method(&self, class: u32, name: &str) -> Option<&Signature<'a>> {
        find_by_name(&self.classes.get(&class)?.static_methods, name, |(n, _)| *n)
            .map(|(_, signature)| signature)
    }
    /// Recusa o acesso estático que só existiria por herança, com nome da origem.
    ///
    /// # Erros
    /// Devolve diagnóstico quando o nome existe apenas em uma superclasse.
    pub(super) fn reject_inherited_static(
        &self,
        class: u32,
        name: &str,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let mut current = self.classes.get(&class).and_then(|info| info.superclass);
        while let Some(id) = current {
            if self.static_field(id, name).is_some() || self.static_method(id, name).is_some() {
                return Err(Diagnostic::new(
                    format!(
                        "Static member '{name}' belongs to '{}' and is not inherited",
                        self.classes[&id].name
                    ),
                    span,
                ));
            }
            current = self.classes.get(&id).and_then(|info| info.superclass);
        }
        Ok(())
    }
}
