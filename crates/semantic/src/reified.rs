//! Bounds e formas anuláveis usadas pela checagem estática e descritores reificados.
use super::*;
impl<'a> Validator<'a> {
    /// Normaliza a união com null sem perder parâmetros simbólicos nem formas estruturais.
    pub(super) fn nullable(&self, ty: Type) -> Type {
        match ty {
            Type::Int => Type::NullableInt,
            Type::String => Type::NullableString,
            Type::Bool => Type::NullableBool,
            Type::Class(id) => Type::NullableClass(id),
            Type::Object => Type::NullableObject,
            Type::Parameter(id) => Type::NullableParameter(id),
            Type::Applied(_) if !matches!(self.shape(ty), Some(TypeShape::Nullable(_))) => {
                self.intern(TypeShape::Nullable(ty))
            }
            _ => ty,
        }
    }
    /// Retira a camada explícita nullable; não inventa a interseção T & Object.
    pub(super) fn without_null(&self, ty: Type) -> Type {
        if let Some(TypeShape::Nullable(inner)) = self.shape(ty) {
            inner
        } else {
            non_null(ty)
        }
    }
    /// Resolve o bound superior, conservando anulabilidade; bounds cíclicos são recusados antes.
    pub(super) fn upper_bound(&self, ty: Type) -> Type {
        match ty {
            Type::Parameter(id) => self
                .type_parameters
                .get(id as usize)
                .map_or(Type::NullableObject, |p| self.upper_bound(p.bound)),
            Type::NullableParameter(id) => self.nullable(self.upper_bound(Type::Parameter(id))),
            _ => ty,
        }
    }
    /// Determina se o tipo pode aceitar null, incluindo parâmetros e estruturas anuláveis.
    pub(super) fn may_be_null(&self, ty: Type) -> bool {
        let upper = self.upper_bound(ty);
        upper == Type::Null
            || is_nullable(upper)
            || matches!(self.shape(upper), Some(TypeShape::Nullable(_)))
    }
    /// Rejeita bounds recursivos neste recorte e verifica os nomes de todos os bounds declarados.
    pub(super) fn validate_bounds(&self) -> Result<(), Diagnostic> {
        for (id, parameter) in self.type_parameters.iter().enumerate() {
            self.check_type_name(parameter.bound, parameter.span)?;
            if matches!(parameter.bound, Type::Void | Type::Inferred | Type::Null) {
                return Err(Diagnostic::new("Unsupported generic bound", parameter.span));
            }
            let mut visited = HashSet::new();
            let mut stack = vec![parameter.bound];
            while let Some(ty) = stack.pop() {
                match ty {
                    Type::Parameter(next) | Type::NullableParameter(next) => {
                        if next as usize == id {
                            return Err(Diagnostic::new(
                                "Recursive generic bounds are unsupported",
                                parameter.span,
                            ));
                        }
                        if visited.insert(next) {
                            stack.push(
                                self.type_parameters
                                    .get(next as usize)
                                    .ok_or_else(|| {
                                        Diagnostic::new(
                                            "Unknown type parameter in bound",
                                            parameter.span,
                                        )
                                    })?
                                    .bound,
                            );
                        }
                    }
                    Type::Applied(_) => match self.shape(ty) {
                        Some(TypeShape::Record { positional, named }) => {
                            stack.extend(positional);
                            stack.extend(named.into_iter().map(|(_, t)| t));
                        }
                        Some(
                            TypeShape::List(t) | TypeShape::Iterable(t) | TypeShape::Nullable(t),
                        ) => stack.push(t),
                        Some(TypeShape::Function { result, parameters }) => {
                            stack.push(result);
                            stack.extend(parameters);
                        }
                        None => {}
                    },
                    _ => {}
                }
            }
        }
        Ok(())
    }
    /// Valida o alvo de is/as antes de exigir sua verificação efetiva pelo backend.
    pub(super) fn runtime_type(&self, ty: Type, span: Span) -> Result<(), Diagnostic> {
        self.check_type_name(ty, span)?;
        if matches!(ty, Type::Void | Type::Inferred) {
            Err(Diagnostic::new("Unsupported runtime type test", span))
        } else {
            Ok(())
        }
    }
}
