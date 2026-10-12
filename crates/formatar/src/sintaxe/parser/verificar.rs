//! Conferências da árvore produzida pelo parser e despejo textual.
//!
//! * [`tokens_mut`]/[`tokens_do_no`] enumeram os campos de token de cada nó
//!   (o equivalente dos `childEntities` que são tokens no analyzer).
//! * [`conferir`] verifica a cobertura: todo token (menos o fim do arquivo,
//!   as vírgulas e os `.` entre componentes de nomes qualificados e
//!   símbolos) é referenciado por exatamente um nó; `inicio <= fim`; e o
//!   intervalo de cada filho fica dentro do intervalo do pai.
//! * [`despejar`] escreve a árvore no mesmo formato do despejo feito com o
//!   analyzer, para comparar nó a nó: uma linha por nó em pré-ordem com a
//!   profundidade, o tipo, os offsets (em bytes) do primeiro e do último
//!   token e os offsets dos tokens próprios.

use dartforge_frontend::token::Op;

use super::super::arvore::{Arvore, No, NoId, TipoParametro};
use super::super::token::{TipoToken, Tk, Tokens};

/// Chama `f` para cada campo de token de `no` (mutável), na ordem dos
/// campos.
pub fn tokens_mut(no: &mut No, f: &mut dyn FnMut(&mut Tk)) {
    macro_rules! t {
        ($($x:expr),*) => {{ $( f($x); )* }};
    }
    macro_rules! o {
        ($($x:expr),*) => {{ $( if let Some(v) = $x { f(v); } )* }};
    }
    use No::*;
    match no {
        CompilationUnit { script_tag, .. } => o!(script_tag),
        LibraryDirective {
            library_keyword,
            semicolon,
            ..
        } => t!(library_keyword, semicolon),
        ImportDirective {
            import_keyword,
            deferred_keyword,
            as_keyword,
            semicolon,
            ..
        } => {
            t!(import_keyword);
            o!(deferred_keyword, as_keyword);
            t!(semicolon);
        }
        ExportDirective {
            export_keyword,
            semicolon,
            ..
        } => t!(export_keyword, semicolon),
        PartDirective {
            part_keyword, semicolon, ..
        } => t!(part_keyword, semicolon),
        PartOfDirective {
            part_keyword,
            of_keyword,
            semicolon,
            ..
        } => t!(part_keyword, of_keyword, semicolon),
        Configuration {
            if_keyword,
            left_parenthesis,
            equal_token,
            right_parenthesis,
            ..
        } => {
            t!(if_keyword, left_parenthesis);
            o!(equal_token);
            t!(right_parenthesis);
        }
        DottedName { .. } | LibraryIdentifier { .. } => {}
        ShowCombinator { keyword, .. } | HideCombinator { keyword, .. } => t!(keyword),
        ClassDeclaration {
            augment_keyword,
            abstract_keyword,
            macro_keyword,
            sealed_keyword,
            base_keyword,
            interface_keyword,
            final_keyword,
            mixin_keyword,
            class_keyword,
            name,
            left_bracket,
            right_bracket,
            ..
        } => {
            o!(
                augment_keyword,
                abstract_keyword,
                macro_keyword,
                sealed_keyword,
                base_keyword,
                interface_keyword,
                final_keyword,
                mixin_keyword
            );
            t!(class_keyword, name, left_bracket, right_bracket);
        }
        ExtendsClause { extends_keyword, .. } => t!(extends_keyword),
        WithClause { with_keyword, .. } => t!(with_keyword),
        ImplementsClause { implements_keyword, .. } => t!(implements_keyword),
        NativeClause { native_keyword, .. } => t!(native_keyword),
        MixinDeclaration {
            augment_keyword,
            base_keyword,
            mixin_keyword,
            name,
            left_bracket,
            right_bracket,
            ..
        } => {
            o!(augment_keyword, base_keyword);
            t!(mixin_keyword, name, left_bracket, right_bracket);
        }
        MixinOnClause { on_keyword, .. } => t!(on_keyword),
        ExtensionDeclaration {
            augment_keyword,
            extension_keyword,
            name,
            left_bracket,
            right_bracket,
            ..
        } => {
            o!(augment_keyword);
            t!(extension_keyword);
            o!(name);
            t!(left_bracket, right_bracket);
        }
        ExtensionOnClause { on_keyword, .. } => t!(on_keyword),
        ExtensionTypeDeclaration {
            augment_keyword,
            extension_keyword,
            type_keyword,
            const_keyword,
            name,
            left_bracket,
            right_bracket,
            ..
        } => {
            o!(augment_keyword);
            t!(extension_keyword, type_keyword);
            o!(const_keyword);
            t!(name, left_bracket, right_bracket);
        }
        RepresentationDeclaration {
            left_parenthesis,
            field_name,
            right_parenthesis,
            ..
        } => t!(left_parenthesis, field_name, right_parenthesis),
        RepresentationConstructorName { period, name } => t!(period, name),
        EnumDeclaration {
            augment_keyword,
            enum_keyword,
            name,
            left_bracket,
            semicolon,
            right_bracket,
            ..
        } => {
            o!(augment_keyword);
            t!(enum_keyword, name, left_bracket);
            o!(semicolon);
            t!(right_bracket);
        }
        EnumConstantDeclaration { name, .. } => t!(name),
        EnumConstantArguments { .. } => {}
        ConstructorSelector { period, .. } => t!(period),
        ClassTypeAlias {
            augment_keyword,
            abstract_keyword,
            macro_keyword,
            sealed_keyword,
            base_keyword,
            interface_keyword,
            final_keyword,
            mixin_keyword,
            typedef_keyword,
            name,
            equals,
            semicolon,
            ..
        } => {
            o!(
                augment_keyword,
                abstract_keyword,
                macro_keyword,
                sealed_keyword,
                base_keyword,
                interface_keyword,
                final_keyword,
                mixin_keyword
            );
            t!(typedef_keyword, name, equals, semicolon);
        }
        FunctionTypeAlias {
            augment_keyword,
            typedef_keyword,
            name,
            semicolon,
            ..
        } => {
            o!(augment_keyword);
            t!(typedef_keyword, name, semicolon);
        }
        GenericTypeAlias {
            augment_keyword,
            typedef_keyword,
            name,
            equals,
            semicolon,
            ..
        } => {
            o!(augment_keyword);
            t!(typedef_keyword, name, equals, semicolon);
        }
        FunctionDeclaration {
            augment_keyword,
            external_keyword,
            property_keyword,
            name,
            ..
        } => {
            o!(augment_keyword, external_keyword, property_keyword);
            t!(name);
        }
        TopLevelVariableDeclaration {
            augment_keyword,
            external_keyword,
            semicolon,
            ..
        } => {
            o!(augment_keyword, external_keyword);
            t!(semicolon);
        }
        ConstructorDeclaration {
            augment_keyword,
            external_keyword,
            const_keyword,
            factory_keyword,
            period,
            name,
            separator,
            ..
        } => o!(
            augment_keyword,
            external_keyword,
            const_keyword,
            factory_keyword,
            period,
            name,
            separator
        ),
        MethodDeclaration {
            augment_keyword,
            external_keyword,
            modifier_keyword,
            property_keyword,
            operator_keyword,
            name,
            ..
        } => {
            o!(
                augment_keyword,
                external_keyword,
                modifier_keyword,
                property_keyword,
                operator_keyword
            );
            t!(name);
        }
        FieldDeclaration {
            augment_keyword,
            abstract_keyword,
            covariant_keyword,
            external_keyword,
            static_keyword,
            semicolon,
            ..
        } => {
            o!(
                augment_keyword,
                abstract_keyword,
                covariant_keyword,
                external_keyword,
                static_keyword
            );
            t!(semicolon);
        }
        ConstructorFieldInitializer {
            this_keyword,
            period,
            equals,
            ..
        } => {
            o!(this_keyword, period);
            t!(equals);
        }
        SuperConstructorInvocation { super_keyword, period, .. } => {
            t!(super_keyword);
            o!(period);
        }
        RedirectingConstructorInvocation { this_keyword, period, .. } => {
            t!(this_keyword);
            o!(period);
        }
        AssertInitializer {
            assert_keyword,
            left_parenthesis,
            comma,
            right_parenthesis,
            ..
        } => {
            t!(assert_keyword, left_parenthesis);
            o!(comma);
            t!(right_parenthesis);
        }
        BlockFunctionBody { keyword, star, .. } => o!(keyword, star),
        ExpressionFunctionBody {
            keyword,
            star,
            function_definition,
            semicolon,
            ..
        } => {
            o!(keyword, star);
            t!(function_definition);
            o!(semicolon);
        }
        EmptyFunctionBody { semicolon } => t!(semicolon),
        NativeFunctionBody {
            native_keyword,
            semicolon,
            ..
        } => t!(native_keyword, semicolon),
        FormalParameterList {
            left_parenthesis,
            left_delimiter,
            right_delimiter,
            right_parenthesis,
            ..
        } => {
            t!(left_parenthesis);
            o!(left_delimiter, right_delimiter);
            t!(right_parenthesis);
        }
        SimpleFormalParameter {
            covariant_keyword,
            required_keyword,
            keyword,
            name,
            ..
        } => o!(covariant_keyword, required_keyword, keyword, name),
        FieldFormalParameter {
            covariant_keyword,
            required_keyword,
            keyword,
            this_keyword,
            period,
            name,
            question,
            ..
        } => {
            o!(covariant_keyword, required_keyword, keyword);
            t!(this_keyword, period, name);
            o!(question);
        }
        SuperFormalParameter {
            covariant_keyword,
            required_keyword,
            keyword,
            super_keyword,
            period,
            name,
            question,
            ..
        } => {
            o!(covariant_keyword, required_keyword, keyword);
            t!(super_keyword, period, name);
            o!(question);
        }
        FunctionTypedFormalParameter {
            covariant_keyword,
            required_keyword,
            name,
            question,
            ..
        } => {
            o!(covariant_keyword, required_keyword);
            t!(name);
            o!(question);
        }
        DefaultFormalParameter { separator, .. } => o!(separator),
        NamedType {
            import_prefix,
            name2,
            question,
            ..
        } => {
            if let Some((a, b)) = import_prefix {
                t!(a, b);
            }
            t!(name2);
            o!(question);
        }
        GenericFunctionType {
            function_keyword,
            question,
            ..
        } => {
            t!(function_keyword);
            o!(question);
        }
        RecordTypeAnnotation {
            left_parenthesis,
            right_parenthesis,
            question,
            ..
        } => {
            t!(left_parenthesis, right_parenthesis);
            o!(question);
        }
        RecordTypeAnnotationNamedFields {
            left_bracket,
            right_bracket,
            ..
        }
        | TypeArgumentList {
            left_bracket,
            right_bracket,
            ..
        }
        | TypeParameterList {
            left_bracket,
            right_bracket,
            ..
        }
        | Block {
            left_bracket,
            right_bracket,
            ..
        }
        | ListLiteral {
            const_keyword: None,
            left_bracket,
            right_bracket,
            ..
        }
        | SetOrMapLiteral {
            const_keyword: None,
            left_bracket,
            right_bracket,
            ..
        }
        | ListPattern {
            left_bracket,
            right_bracket,
            ..
        }
        | MapPattern {
            left_bracket,
            right_bracket,
            ..
        } => t!(left_bracket, right_bracket),
        ListLiteral {
            const_keyword: Some(c),
            left_bracket,
            right_bracket,
            ..
        }
        | SetOrMapLiteral {
            const_keyword: Some(c),
            left_bracket,
            right_bracket,
            ..
        } => t!(c, left_bracket, right_bracket),
        RecordTypeAnnotationPositionalField { name, .. } => o!(name),
        RecordTypeAnnotationNamedField { name, .. } => t!(name),
        TypeParameter {
            name, extends_keyword, ..
        } => {
            t!(name);
            o!(extends_keyword);
        }
        Annotation { at_sign, period, .. } => {
            t!(at_sign);
            o!(period);
        }
        ExpressionStatement { semicolon, .. } => o!(semicolon),
        VariableDeclarationStatement { semicolon, .. }
        | PatternVariableDeclarationStatement { semicolon, .. }
        | EmptyStatement { semicolon } => t!(semicolon),
        VariableDeclarationList {
            late_keyword, keyword, ..
        } => o!(late_keyword, keyword),
        VariableDeclaration { name, equals, .. } => {
            t!(name);
            o!(equals);
        }
        FunctionDeclarationStatement { .. } => {}
        IfStatement {
            if_keyword,
            left_parenthesis,
            right_parenthesis,
            else_keyword,
            ..
        }
        | IfElement {
            if_keyword,
            left_parenthesis,
            right_parenthesis,
            else_keyword,
            ..
        } => {
            t!(if_keyword, left_parenthesis, right_parenthesis);
            o!(else_keyword);
        }
        CaseClause { case_keyword, .. } => t!(case_keyword),
        GuardedPattern { .. } => {}
        WhenClause { when_keyword, .. } => t!(when_keyword),
        ForStatement {
            await_keyword,
            for_keyword,
            left_parenthesis,
            right_parenthesis,
            ..
        }
        | ForElement {
            await_keyword,
            for_keyword,
            left_parenthesis,
            right_parenthesis,
            ..
        } => {
            o!(await_keyword);
            t!(for_keyword, left_parenthesis, right_parenthesis);
        }
        ForPartsWithDeclarations {
            left_separator,
            right_separator,
            ..
        }
        | ForPartsWithExpression {
            left_separator,
            right_separator,
            ..
        }
        | ForPartsWithPattern {
            left_separator,
            right_separator,
            ..
        } => t!(left_separator, right_separator),
        ForEachPartsWithDeclaration { in_keyword, .. } | ForEachPartsWithIdentifier { in_keyword, .. } => {
            t!(in_keyword)
        }
        ForEachPartsWithPattern { keyword, in_keyword, .. } => t!(keyword, in_keyword),
        DeclaredIdentifier { keyword, name, .. } => {
            o!(keyword);
            t!(name);
        }
        WhileStatement {
            while_keyword,
            left_parenthesis,
            right_parenthesis,
            ..
        } => t!(while_keyword, left_parenthesis, right_parenthesis),
        DoStatement {
            do_keyword,
            while_keyword,
            left_parenthesis,
            right_parenthesis,
            semicolon,
            ..
        } => t!(do_keyword, while_keyword, left_parenthesis, right_parenthesis, semicolon),
        SwitchStatement {
            switch_keyword,
            left_parenthesis,
            right_parenthesis,
            left_bracket,
            right_bracket,
            ..
        }
        | SwitchExpression {
            switch_keyword,
            left_parenthesis,
            right_parenthesis,
            left_bracket,
            right_bracket,
            ..
        } => t!(switch_keyword, left_parenthesis, right_parenthesis, left_bracket, right_bracket),
        SwitchCase { keyword, colon, .. }
        | SwitchPatternCase { keyword, colon, .. }
        | SwitchDefault { keyword, colon, .. } => t!(keyword, colon),
        TryStatement {
            try_keyword,
            finally_keyword,
            ..
        } => {
            t!(try_keyword);
            o!(finally_keyword);
        }
        CatchClause {
            on_keyword,
            catch_keyword,
            left_parenthesis,
            comma,
            right_parenthesis,
            ..
        } => o!(on_keyword, catch_keyword, left_parenthesis, comma, right_parenthesis),
        CatchClauseParameter { name } => t!(name),
        ReturnStatement {
            return_keyword,
            semicolon,
            ..
        } => t!(return_keyword, semicolon),
        BreakStatement {
            break_keyword,
            semicolon,
            ..
        } => t!(break_keyword, semicolon),
        ContinueStatement {
            continue_keyword,
            semicolon,
            ..
        } => t!(continue_keyword, semicolon),
        YieldStatement {
            yield_keyword,
            star,
            semicolon,
            ..
        } => {
            t!(yield_keyword);
            o!(star);
            t!(semicolon);
        }
        LabeledStatement { .. } => {}
        Label { colon, .. } => t!(colon),
        AssertStatement {
            assert_keyword,
            left_parenthesis,
            comma,
            right_parenthesis,
            semicolon,
            ..
        } => {
            t!(assert_keyword, left_parenthesis);
            o!(comma);
            t!(right_parenthesis, semicolon);
        }
        PatternVariableDeclaration { keyword, equals, .. } => t!(keyword, equals),
        AdjacentStrings { .. } => {}
        AsExpression { as_operator, .. } => t!(as_operator),
        AssignmentExpression { operator, .. }
        | BinaryExpression { operator, .. }
        | PostfixExpression { operator, .. }
        | PrefixExpression { operator, .. }
        | LogicalAndPattern { operator, .. }
        | LogicalOrPattern { operator, .. }
        | NullAssertPattern { operator, .. }
        | NullCheckPattern { operator, .. }
        | RelationalPattern { operator, .. }
        | RestPatternElement { operator, .. }
        | PropertyAccess { operator, .. } => t!(operator),
        AwaitExpression { await_keyword, .. } => t!(await_keyword),
        BooleanLiteral { literal }
        | DoubleLiteral { literal }
        | IntegerLiteral { literal }
        | NullLiteral { literal }
        | SimpleStringLiteral { literal } => t!(literal),
        CascadeExpression { .. } => {}
        ConditionalExpression { question, colon, .. } => t!(question, colon),
        FunctionExpression { .. } | FunctionExpressionInvocation { .. } | FunctionReference { .. } => {}
        IndexExpression {
            period,
            question,
            left_bracket,
            right_bracket,
            ..
        } => {
            o!(period, question);
            t!(left_bracket, right_bracket);
        }
        InstanceCreationExpression { keyword, .. } => o!(keyword),
        ConstructorName { period, .. } => o!(period),
        IsExpression {
            is_operator,
            not_operator,
            ..
        } => {
            t!(is_operator);
            o!(not_operator);
        }
        MapLiteralEntry { separator, .. } | MapPatternEntry { separator, .. } => t!(separator),
        MethodInvocation { operator, .. } => o!(operator),
        NamedExpression { .. } => {}
        ParenthesizedExpression {
            left_parenthesis,
            right_parenthesis,
            ..
        }
        | ObjectPattern {
            left_parenthesis,
            right_parenthesis,
            ..
        }
        | ParenthesizedPattern {
            left_parenthesis,
            right_parenthesis,
            ..
        }
        | RecordPattern {
            left_parenthesis,
            right_parenthesis,
            ..
        }
        | ArgumentList {
            left_parenthesis,
            right_parenthesis,
            ..
        } => t!(left_parenthesis, right_parenthesis),
        PrefixedIdentifier { period, .. } => t!(period),
        RecordLiteral {
            const_keyword,
            left_parenthesis,
            right_parenthesis,
            ..
        } => {
            o!(const_keyword);
            t!(left_parenthesis, right_parenthesis);
        }
        RethrowExpression { rethrow_keyword } => t!(rethrow_keyword),
        SimpleIdentifier { token } => t!(token),
        StringInterpolation { .. } => {}
        InterpolationExpression {
            left_bracket,
            right_bracket,
            ..
        } => {
            t!(left_bracket);
            o!(right_bracket);
        }
        InterpolationString { contents } => t!(contents),
        SuperExpression { super_keyword } => t!(super_keyword),
        ThisExpression { this_keyword } => t!(this_keyword),
        SymbolLiteral { pound_sign, components } => {
            t!(pound_sign);
            for c in components {
                f(c);
            }
        }
        ThrowExpression { throw_keyword, .. } => t!(throw_keyword),
        SwitchExpressionCase { arrow, .. } => t!(arrow),
        SpreadElement { spread_operator, .. } => t!(spread_operator),
        AssignedVariablePattern { name } => t!(name),
        CastPattern { as_token, .. } => t!(as_token),
        ConstantPattern { const_keyword, .. } => o!(const_keyword),
        DeclaredVariablePattern { keyword, name, .. } | WildcardPattern { keyword, name, .. } => {
            o!(keyword);
            t!(name);
        }
        PatternField { .. } => {}
        PatternFieldName { name, colon } => {
            o!(name);
            t!(colon);
        }
        PatternAssignment { equals, .. } => t!(equals),
    }
}

/// Os tokens próprios de `no`, na ordem dos campos.
pub fn tokens_do_no(no: &No) -> Vec<Tk> {
    let mut c = no.clone();
    let mut v = Vec::new();
    tokens_mut(&mut c, &mut |t| v.push(*t));
    v
}

/// Nome da variante (o nome da classe do analyzer).
pub fn nome_do_no(no: &No) -> String {
    let s = format!("{no:?}");
    s.split([' ', '{', '(']).next().unwrap_or("").to_string()
}

/// Confere a árvore: cobertura de tokens e limites (ver o módulo).
///
/// # Erros
/// A primeira inconsistência encontrada, descrita em texto.
pub fn conferir(tokens: &Tokens, arv: &Arvore, fonte: &str) -> Result<(), String> {
    let n = tokens.lista.len();
    let mut uso = vec![0u32; n];
    let mut visto = vec![false; arv.nos.len()];
    let mut pontos_ok = vec![false; n];
    let Some(raiz) = arv.raiz else {
        return Err("árvore sem raiz".into());
    };
    let mut pilha = vec![raiz];
    let texto = |i: usize| -> String {
        let t = &tokens.lista[i];
        format!("'{}'@{}", &fonte[t.inicio..t.fim], t.inicio)
    };
    while let Some(id) = pilha.pop() {
        let i = id.0 as usize;
        if visto[i] {
            return Err(format!("nó {} visitado duas vezes", nome_do_no(arv.no(id))));
        }
        visto[i] = true;
        let d = arv.dados(id);
        if d.inicio > d.fim {
            return Err(format!("{}: início {} depois do fim {}", nome_do_no(&d.no), texto(d.inicio as usize), texto(d.fim as usize)));
        }
        for t in tokens_do_no(&d.no) {
            let t = t as usize;
            if t >= n {
                return Err(format!("{}: token fora da lista", nome_do_no(&d.no)));
            }
            let curinga = matches!(d.no, No::WildcardPattern { keyword: Some(k), .. } if k as usize == t);
            if !curinga && (t < d.inicio as usize || t > d.fim as usize) {
                return Err(format!(
                    "{} [{}..{}]: token {} fora do intervalo",
                    nome_do_no(&d.no),
                    texto(d.inicio as usize),
                    texto(d.fim as usize),
                    texto(t)
                ));
            }
            uso[t] += 1;
        }
        if matches!(d.no, No::DottedName { .. } | No::LibraryIdentifier { .. } | No::SymbolLiteral { .. }) {
            for p in pontos_ok.iter_mut().take(d.fim as usize + 1).skip(d.inicio as usize) {
                *p = true;
            }
        }
        let mut filhos = Vec::new();
        d.no.filhos(&mut |f| filhos.push(f));
        for f in filhos {
            let fd = arv.dados(f);
            if fd.inicio < d.inicio || fd.fim > d.fim {
                return Err(format!(
                    "{} [{}..{}] fora de {} [{}..{}]",
                    nome_do_no(&fd.no),
                    texto(fd.inicio as usize),
                    texto(fd.fim as usize),
                    nome_do_no(&d.no),
                    texto(d.inicio as usize),
                    texto(d.fim as usize)
                ));
            }
            if fd.pai != Some(id) {
                return Err(format!("{}: pai incorreto", nome_do_no(&fd.no)));
            }
            pilha.push(f);
        }
    }
    for (i, &u) in uso.iter().enumerate() {
        let t = &tokens.lista[i];
        if t.tipo == TipoToken::Eof {
            continue;
        }
        if u > 1 {
            return Err(format!("token {} referenciado {} vezes", texto(i), u));
        }
        if u == 0 {
            let livre = t.tipo == TipoToken::Op(Op::Comma) || (t.tipo == TipoToken::Op(Op::Dot) && pontos_ok[i]);
            if !livre {
                return Err(format!("token {} não referenciado", texto(i)));
            }
        }
    }
    Ok(())
}

/// Despeja a árvore no formato do oráculo (ver o módulo).
pub fn despejar(tokens: &Tokens, arv: &Arvore) -> String {
    let mut s = String::new();
    if let Some(r) = arv.raiz {
        despejar_no(tokens, arv, r, 0, &mut s);
    }
    s
}

fn despejar_no(tokens: &Tokens, arv: &Arvore, id: NoId, prof: usize, s: &mut String) {
    use std::fmt::Write;
    let d = arv.dados(id);
    let off = |t: Tk| tokens.lista[t as usize].inicio;
    let mut toks: Vec<usize> = tokens_do_no(&d.no).into_iter().map(off).collect();
    toks.sort_unstable();
    let lista: Vec<String> = toks.iter().map(|t| t.to_string()).collect();
    let tipo = match &d.no {
        No::SimpleFormalParameter { kind, .. }
        | No::FieldFormalParameter { kind, .. }
        | No::SuperFormalParameter { kind, .. }
        | No::FunctionTypedFormalParameter { kind, .. }
        | No::DefaultFormalParameter { kind, .. } => match kind {
            TipoParametro::Obrigatorio => " k=R",
            TipoParametro::OpcionalPosicional => " k=P",
            TipoParametro::Nomeado => " k=N",
        },
        _ => "",
    };
    let _ = writeln!(
        s,
        "{} {} {} {} {}{}",
        prof,
        nome_do_no(&d.no),
        off(d.inicio),
        off(d.fim),
        lista.join(","),
        tipo
    );
    let mut filhos = Vec::new();
    d.no.filhos(&mut |f| filhos.push(f));
    filhos.sort_by_key(|&f| off(arv.inicio(f)));
    for f in filhos {
        despejar_no(tokens, arv, f, prof + 1, s);
    }
}
