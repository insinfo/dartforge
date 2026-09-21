//! Emissão de interpolação de strings com a ordem de avaliação do Dart.
//!
//! A interpolação vira uma única concatenação JavaScript, avaliada da esquerda
//! para a direita: cada expressão aparece uma vez e na posição em que foi
//! escrita. A conversão para texto não usa `String(x)` do JavaScript, porque a
//! semântica numérica do Dart web tem regras próprias; quem converte é
//! `$dartforgeString`, definida no runtime `core.js`.
use super::*;
use dartforge_syntax::StringPart;

/// Recorta de `core.js` somente a conversão `toString`, entre seus marcadores.
///
/// A definição continua única, no runtime. O recorte existe para que um programa
/// que apenas interpola escalares não carregue o runtime de coleções inteiro; a
/// delegação ao formatador fica sem destino nesse recorte, mas é inalcançável
/// porque a análise semântica já garantiu que só escalares chegam ali.
pub(super) fn runtime() -> &'static str {
    // O runtime pode estar gravado com LF ou CRLF; o marcador ignora os dois.
    const RUNTIME: &str = include_str!("core.js");
    const ABERTURA: &str = "// >>> $dartforgeString";
    const FECHAMENTO: &str = "// <<< $dartforgeString";
    let start = RUNTIME
        .find(ABERTURA)
        .expect("marcador de abertura presente")
        + ABERTURA.len();
    let end = RUNTIME
        .find(FECHAMENTO)
        .expect("marcador de fechamento presente");
    RUNTIME[start..end].trim_start_matches(['\r', '\n'])
}

/// Emite a concatenação de uma interpolação preservando a ordem de avaliação.
pub(super) fn interpolation(parts: &[StringPart<'_>], output: &mut Output<'_>) {
    // Os parênteses isolam a concatenação de qualquer operador ao redor.
    output.push('(');
    if parts.is_empty() {
        output.push_str("\"\"");
    }
    for (index, part) in parts.iter().enumerate() {
        if index != 0 {
            output.push_str(" + ");
        }
        match part {
            StringPart::Borrowed(text) => string_literal(text, output),
            StringPart::Owned(text) => string_literal(text, output),
            StringPart::Expression(value) => convert(value, output),
        }
    }
    output.push(')');
}

/// Converte uma expressão interpolada com a semântica de `toString` do Dart.
///
/// Uma `String` já é o próprio texto e dispensa a chamada de runtime, o que
/// deixa `'$nome'` com o mesmo custo de uma variável. Os demais tipos passam por
/// `$dartforgeString`, que imprime `int` sem depender de `String(x)`, converte
/// `null` em `"null"` e delega coleções e records ao formatador do runtime.
fn convert(value: &Expr<'_>, output: &mut Output<'_>) {
    if output
        .resolution
        .expr_types
        .get(&(value.span.start, value.span.end))
        == Some(&Type::String)
    {
        output.push('(');
        expression(value, output);
        output.push(')');
        return;
    }
    output.strings_used = true;
    output.push_str("$dartforgeString(");
    expression(value, output);
    output.push(')');
}
