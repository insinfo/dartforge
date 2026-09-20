//! Records estruturais e declaração desestruturante com escopo léxico definido.
use super::*;
impl<'a> Validator<'a> {
    /// Confere nomes canônicos sem confundir campos nomeados $01 com o posicional $1.
    pub(super) fn record_shape(
        &self,
        positional: usize,
        named: &[(String, Type)],
        span: Span,
    ) -> Result<(), Diagnostic> {
        let mut previous = None;
        for (name, _) in named {
            if name.starts_with('_')
                || matches!(
                    name.as_str(),
                    "hashCode" | "runtimeType" | "toString" | "noSuchMethod"
                )
                || (1..=positional).any(|i| name == &format!("${i}"))
            {
                return Err(Diagnostic::new("Invalid record field name", span));
            }
            if previous.is_some_and(|p: &str| p >= name.as_str()) {
                return Err(Diagnostic::new(
                    "Record field names must be unique and canonically sorted",
                    span,
                ));
            }
            previous = Some(name.as_str());
        }
        Ok(())
    }
    /// Analisa valores na ordem fonte, mas ordena os nomes apenas na identidade estrutural.
    pub(super) fn record_expression(
        &self,
        fields: &[(Option<&'a str>, Expr<'a>)],
        expected: Option<Type>,
        span: Span,
    ) -> Result<Type, Diagnostic> {
        let context = expected.and_then(|t| self.shape(self.without_null(t)));
        let mut positional = Vec::new();
        let mut named = Vec::new();
        for (name, value) in fields {
            let expected = match &context {
                Some(TypeShape::Record {
                    positional: p,
                    named: n,
                }) => match name {
                    Some(name) => n.iter().find(|(key, _)| key == name).map(|(_, ty)| *ty),
                    None => p.get(positional.len()).copied(),
                },
                _ => None,
            };
            let ty = self.value_expected(value, expected)?;
            if let Some(name) = name {
                named.push(((*name).to_owned(), ty));
            } else {
                positional.push(ty);
            }
        }
        named.sort_by(|a, b| a.0.cmp(&b.0));
        self.record_shape(positional.len(), &named, span)?;
        Ok(self.intern(TypeShape::Record { positional, named }))
    }
    /// Obtém campo somente leitura pela posição ou pelo nome estrutural exato.
    pub(super) fn record_field(
        &self,
        ty: Type,
        name: &str,
        span: Span,
    ) -> Result<Type, Diagnostic> {
        let Some(TypeShape::Record { positional, named }) = self.shape(ty) else {
            return Err(Diagnostic::new("Expected a record receiver", span));
        };
        if let Some((_, ty)) = named.iter().find(|(key, _)| key == name) {
            return Ok(*ty);
        }
        for (index, ty) in positional.iter().enumerate() {
            if name == format!("${}", index + 1) {
                return Ok(*ty);
            }
        }
        Err(Diagnostic::new("Unknown record field", span))
    }
    /// Inicializa bindings somente após analisar uma única expressão de record completa.
    pub(super) fn record_destructure(
        &mut self,
        positional: &[(&'a str, Span)],
        named: &[(&'a str, &'a str, Span)],
        initializer: &Expr<'a>,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let ty = self.upper_bound(self.value(initializer)?);
        let Some(TypeShape::Record {
            positional: types,
            named: fields,
        }) = self.shape(ty)
        else {
            return Err(Diagnostic::new(
                "Record destructuring requires a non-null record type",
                span,
            ));
        };
        if types.len() != positional.len() || fields.len() != named.len() {
            return Err(Diagnostic::new(
                "Record destructuring shape does not match",
                span,
            ));
        }
        let mut initialized = positional
            .iter()
            .zip(types)
            .map(|((name, _), ty)| (*name, ty))
            .collect::<Vec<_>>();
        let mut seen = HashSet::new();
        for (field, name, span) in named {
            if !seen.insert(*field) {
                return Err(Diagnostic::new("Duplicate record pattern field", *span));
            }
            let ty = fields
                .iter()
                .find(|(key, _)| key == field)
                .map(|(_, ty)| *ty)
                .ok_or_else(|| Diagnostic::new("Record pattern field is missing", *span))?;
            initialized.push((*name, ty));
        }
        let scope = self.scopes.last_mut().expect("escopo do padrão");
        for (name, ty) in initialized {
            if name != "_" {
                scope.get_mut(name).expect("binding pré-declarado").ty = Some(ty);
            }
        }
        Ok(())
    }
}
