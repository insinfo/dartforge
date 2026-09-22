//! Emissão JavaScript dos membros de `dart:core` sobre tipos escalares.
//!
//! Cada membro aqui é uma **decisão semântica** conferida contra o Dart 3.6.2 e o
//! Dart 3.13.4, e não a tradução para o método JavaScript de nome parecido. Os
//! casos em que os dois divergem estão comentados no ponto da emissão e
//! tabelados em `docs/NUCLEO.md`:
//!
//! * `(-2.5).round()` é `-3` no Dart e `-2` em `Math.round`, que arredonda meio
//!   para `+∞`; a emissão espelha em torno de zero.
//! * `(-0.0).isNegative` é `true` no Dart e `-0 < 0` é `false` no JavaScript.
//! * `round`, `floor`, `ceil`, `truncate` e `toInt` **lançam** sobre `NaN` e
//!   infinito, como no oráculo, em vez de propagar `NaN`.
//! * `padLeft` com preenchimento de mais de um caractere repete a cadeia inteira
//!   `delta` vezes no Dart (`'7'.padLeft(5, 'ab')` é `abababab7`), enquanto
//!   `padStart` do JavaScript trunca no comprimento pedido.
//! * `substring`, `codeUnitAt`, `indexOf` e `toRadixString` recusam índice fora
//!   de faixa com `RangeError`; os equivalentes do JavaScript grampeiam ou
//!   devolvem `NaN` em silêncio.
//!
//! # Custo para quem não usa
//!
//! Nenhum auxiliar entra no módulo por existir: [`Output::nucleo`] é o conjunto
//! dos que a emissão realmente pediu, e [`runtime`] emite só esses, em ordem
//! estável. Um programa sem membro de biblioteca deixa o conjunto vazio — um
//! `BTreeSet` vazio não aloca — e o JavaScript sai idêntico ao de antes.
use super::*;

/// Registra um auxiliar e devolve seu nome, para uso no texto emitido.
fn precisa(nome: &'static str, output: &mut Output<'_>) -> &'static str {
    output.nucleo.insert(nome);
    nome
}

/// Emite `receptor` entre parênteses somente quando a precedência exige.
///
/// Um literal, um identificador e uma chamada já são primárias; qualquer outra
/// forma vai entre parênteses para que `.length` não capture parte do operando.
fn receptor(value: &Expr<'_>, output: &mut Output<'_>) {
    let primaria = matches!(
        value.kind,
        ExprKind::Identifier(_)
            | ExprKind::String(_)
            | ExprKind::OwnedString(_)
            | ExprKind::Call { .. }
            | ExprKind::MethodCall { .. }
            | ExprKind::Member { .. }
            | ExprKind::This
            | ExprKind::Index { .. }
    );
    if primaria {
        expression(value, output);
    } else {
        output.push('(');
        expression(value, output);
        output.push(')');
    }
}

/// Emite `auxiliar(receptor, argumentos...)`.
fn auxiliar(
    nome: &'static str,
    alvo: &Expr<'_>,
    argumentos: &[Expr<'_>],
    output: &mut Output<'_>,
) {
    let nome = precisa(nome, output);
    output.push_str(nome);
    output.push('(');
    expression(alvo, output);
    for argumento in argumentos {
        output.push(',');
        expression(argumento, output);
    }
    output.push(')');
}

/// Emite `auxiliar(receptor, argumentos..., padrão)` completando os opcionais.
///
/// O auxiliar recebe sempre a aridade cheia para que o corpo JavaScript não
/// precise distinguir `undefined` de valor escrito: o padrão do Dart aparece
/// aqui, uma vez, ao lado da assinatura que o exige.
fn auxiliar_com_padrao(
    nome: &'static str,
    alvo: &Expr<'_>,
    argumentos: &[Expr<'_>],
    padroes: &[&str],
    output: &mut Output<'_>,
) {
    let nome = precisa(nome, output);
    output.push_str(nome);
    output.push('(');
    expression(alvo, output);
    for argumento in argumentos {
        output.push(',');
        expression(argumento, output);
    }
    for padrao in &padroes[argumentos.len()..] {
        output.push(',');
        output.push_str(padrao);
    }
    output.push(')');
}

/// Emite `receptor.metodo(argumentos...)` do próprio JavaScript.
fn nativo(metodo: &str, alvo: &Expr<'_>, argumentos: &[Expr<'_>], output: &mut Output<'_>) {
    receptor(alvo, output);
    output.push('.');
    output.push_str(metodo);
    output.push('(');
    for (indice, argumento) in argumentos.iter().enumerate() {
        if indice > 0 {
            output.push(',');
        }
        expression(argumento, output);
    }
    output.push(')');
}

/// Emite a leitura de um membro escalar; devolve `false` quando não é um.
///
/// O tipo vem da resolução semântica, que regravou o tipo do receptor com o
/// limite genérico já resolvido. Quem chama só emite a forma antiga quando esta
/// função devolve `false`.
pub(super) fn membro(alvo: &Expr<'_>, nome: &str, ty: Type, output: &mut Output<'_>) -> bool {
    match (ty, nome) {
        (Type::String, "length") => {
            receptor(alvo, output);
            output.push_str(".length");
        }
        (Type::String, "isEmpty") => {
            output.push('(');
            receptor(alvo, output);
            output.push_str(".length===0)");
        }
        (Type::String, "isNotEmpty") => {
            output.push('(');
            receptor(alvo, output);
            output.push_str(".length!==0)");
        }
        (Type::String, "hashCode") => auxiliar("$dartforgeStringHash", alvo, &[], output),
        (Type::String, "codeUnits") => auxiliar("$dartforgeCodeUnits", alvo, &[], output),
        // `int` não tem NaN nem infinito: as duas respostas são constantes, e
        // emitir a constante evita avaliar o receptor sem necessidade — o que
        // seria errado, porque o receptor pode ter efeito colateral. Por isso a
        // sequência com vírgula preserva a avaliação e descarta o valor.
        (Type::Int, "isFinite") => {
            output.push('(');
            expression(alvo, output);
            output.push_str(",true)");
        }
        (Type::Int, "isNaN") => {
            output.push('(');
            expression(alvo, output);
            output.push_str(",false)");
        }
        (Type::Int, "isEven") => {
            output.push('(');
            receptor(alvo, output);
            output.push_str("%2===0)");
        }
        (Type::Int, "isOdd") => {
            output.push('(');
            receptor(alvo, output);
            output.push_str("%2!==0)");
        }
        (Type::Int, "bitLength") => auxiliar("$dartforgeBitLength", alvo, &[], output),
        (Type::Int, "sign") => {
            // `Math.sign(-0)` é `-0`, e `int` não tem zero negativo: `|0` o
            // normaliza sem alterar -1, 0 e 1.
            output.push_str("(Math.sign(");
            expression(alvo, output);
            output.push_str(")|0)");
        }
        (Type::Int, "isNegative") => {
            output.push('(');
            receptor(alvo, output);
            output.push_str("<0)");
        }
        (Type::Int | Type::Double | Type::Num, "hashCode") => {
            auxiliar("$dartforgeNumHash", alvo, &[], output)
        }
        (Type::Double | Type::Num, "isNaN") => nativo_estatico("Number.isNaN", alvo, output),
        (Type::Double | Type::Num, "isFinite") => {
            nativo_estatico("Number.isFinite", alvo, output)
        }
        (Type::Double | Type::Num, "isInfinite") => {
            auxiliar("$dartforgeIsInfinite", alvo, &[], output)
        }
        // `(-0.0).isNegative` é `true` no oráculo e `-0 < 0` é `false` aqui.
        (Type::Double | Type::Num, "isNegative") => {
            auxiliar("$dartforgeIsNegative", alvo, &[], output)
        }
        (Type::Double | Type::Num, "sign") => nativo_estatico("Math.sign", alvo, output),
        (Type::Bool, "hashCode") => {
            // Dart não especifica o valor; o oráculo 3.6.2 usa 1231/1237, os
            // mesmos inteiros do `Boolean.hashCode` do Java, e reproduzi-los
            // custa nada.
            output.push('(');
            receptor(alvo, output);
            output.push_str("?1231:1237)");
        }
        _ => return false,
    }
    true
}

/// Emite `funcao(receptor)` para um intrínseco do JavaScript.
fn nativo_estatico(funcao: &str, alvo: &Expr<'_>, output: &mut Output<'_>) {
    output.push_str(funcao);
    output.push('(');
    expression(alvo, output);
    output.push(')');
}

/// Emite a chamada de um método escalar; devolve `false` quando não é um.
pub(super) fn metodo(
    alvo: &Expr<'_>,
    nome: &str,
    argumentos: &[Expr<'_>],
    ty: Type,
    output: &mut Output<'_>,
) -> bool {
    match (ty, nome) {
        // --- String ---
        (Type::String, "toString") => receptor(alvo, output),
        (Type::String, "toLowerCase") => nativo("toLowerCase", alvo, argumentos, output),
        (Type::String, "toUpperCase") => nativo("toUpperCase", alvo, argumentos, output),
        // `trim` do JavaScript remove o mesmo conjunto que o do Dart, BOM
        // incluído, conferido em `'\u{FEFF}a'.trim()`.
        (Type::String, "trim") => nativo("trim", alvo, argumentos, output),
        (Type::String, "trimLeft") => nativo("trimStart", alvo, argumentos, output),
        (Type::String, "trimRight") => nativo("trimEnd", alvo, argumentos, output),
        (Type::String, "endsWith") => nativo("endsWith", alvo, argumentos, output),
        (Type::String, "lastIndexOf") => nativo("lastIndexOf", alvo, argumentos, output),
        (Type::String, "replaceAll") => nativo("replaceAll", alvo, argumentos, output),
        (Type::String, "substring") => {
            auxiliar_com_padrao("$dartforgeSubstring", alvo, argumentos, &["", "null"], output)
        }
        (Type::String, "codeUnitAt") => auxiliar("$dartforgeCodeUnitAt", alvo, argumentos, output),
        (Type::String, "compareTo") => {
            auxiliar("$dartforgeCompareString", alvo, argumentos, output)
        }
        (Type::String, "indexOf") => {
            auxiliar_com_padrao("$dartforgeIndexOf", alvo, argumentos, &["", "0"], output)
        }
        (Type::String, "contains") => {
            auxiliar_com_padrao("$dartforgeContains", alvo, argumentos, &["", "0"], output)
        }
        (Type::String, "startsWith") => {
            auxiliar_com_padrao("$dartforgeStartsWith", alvo, argumentos, &["", "0"], output)
        }
        (Type::String, "padLeft") => auxiliar_com_padrao(
            "$dartforgePadLeft",
            alvo,
            argumentos,
            &["", "' '"],
            output,
        ),
        (Type::String, "padRight") => auxiliar_com_padrao(
            "$dartforgePadRight",
            alvo,
            argumentos,
            &["", "' '"],
            output,
        ),
        (Type::String, "replaceFirst") => auxiliar_com_padrao(
            "$dartforgeReplaceFirst",
            alvo,
            argumentos,
            &["", "", "0"],
            output,
        ),
        (Type::String, "replaceRange") => {
            auxiliar("$dartforgeReplaceRange", alvo, argumentos, output)
        }
        (Type::String, "split") => auxiliar("$dartforgeSplit", alvo, argumentos, output),
        // --- int ---
        // `round`, `floor`, `ceil`, `truncate`, `toInt` e `toDouble` são
        // identidade em `int`: existem na tabela porque código escrito sobre
        // `num` os chama, e emitir o receptor é a tradução exata.
        (
            Type::Int,
            "round" | "floor" | "ceil" | "truncate" | "toInt" | "toDouble",
        ) => receptor(alvo, output),
        (Type::Int, "abs") => nativo_estatico("Math.abs", alvo, output),
        (Type::Int, "gcd") => auxiliar("$dartforgeGcd", alvo, argumentos, output),
        (Type::Int, "toRadixString") => {
            auxiliar("$dartforgeRadixString", alvo, argumentos, output)
        }
        (Type::Int, "toString") => auxiliar("$dartforgeIntString", alvo, &[], output),
        (Type::Int | Type::Double | Type::Num, "compareTo") => {
            auxiliar("$dartforgeCompareNum", alvo, argumentos, output)
        }
        // --- double e num ---
        (Type::Double, "abs") => nativo_estatico("Math.abs", alvo, output),
        (Type::Num, "abs") => nativo_estatico("Math.abs", alvo, output),
        (Type::Double | Type::Num, "toDouble") => receptor(alvo, output),
        (Type::Double | Type::Num, "toInt" | "truncate") => {
            auxiliar("$dartforgeTruncToInt", alvo, &[], output)
        }
        (Type::Double | Type::Num, "floor") => auxiliar("$dartforgeFloorToInt", alvo, &[], output),
        (Type::Double | Type::Num, "ceil") => auxiliar("$dartforgeCeilToInt", alvo, &[], output),
        (Type::Double | Type::Num, "round") => auxiliar("$dartforgeRoundToInt", alvo, &[], output),
        (Type::Double, "truncateToDouble") => nativo_estatico("Math.trunc", alvo, output),
        (Type::Double, "floorToDouble") => nativo_estatico("Math.floor", alvo, output),
        (Type::Double, "ceilToDouble") => nativo_estatico("Math.ceil", alvo, output),
        (Type::Double, "roundToDouble") => auxiliar("$dartforgeRoundDouble", alvo, &[], output),
        (Type::Double, "toStringAsFixed") => {
            auxiliar("$dartforgeToStringAsFixed", alvo, argumentos, output)
        }
        (Type::Double, "toString") => {
            output.double_used = true;
            output.push_str("$dartforgeDouble(");
            expression(alvo, output);
            output.push(')');
        }
        // --- bool ---
        (Type::Bool, "toString") => {
            output.push('(');
            receptor(alvo, output);
            output.push_str("?'true':'false')");
        }
        _ => return false,
    }
    true
}

/// Fonte JavaScript de cada auxiliar, indexada pelo nome registrado.
///
/// A tabela é um `&[(&str, &str)]` ordenado por nome e consultado por busca
/// binária: nenhum mapa é construído em tempo de execução do compilador.
const AUXILIARES: &[(&str, &str)] = &[
    (
        "$dartforgeBitLength",
        // Faixa de 32 bits com sinal, a mesma que `docs/COLECOES-OPERADORES.md`
        // fixou para os operadores de bits: `~x` traz o negativo ao positivo de
        // mesmo comprimento, como a definição do Dart pede.
        "function $dartforgeBitLength(x) { return 32 - Math.clz32(x < 0 ? ~x : x); }\n",
    ),
    (
        "$dartforgeCeilToInt",
        "function $dartforgeCeilToInt(x) { return Math.ceil($dartforgeFinito(x)); }\n",
    ),
    (
        "$dartforgeCodeUnitAt",
        "function $dartforgeCodeUnitAt(s, i) { if (!Number.isInteger(i) || i < 0 || i >= s.length) throw $dartforgeFaixa('length', 0, s.length - 1, i); return s.charCodeAt(i); }\n",
    ),
    (
        "$dartforgeCodeUnits",
        "function $dartforgeCodeUnits(s) { const v = []; for (let i = 0; i < s.length; i++) v.push(s.charCodeAt(i)); return new $dartforgeList(v, ['int']); }\n",
    ),
    (
        "$dartforgeCompareNum",
        // Regras do `num.compareTo` do oráculo: NaN é maior que tudo e igual a
        // si mesmo, e `0.0` é maior que `-0.0`, que `===` não distingue.
        "function $dartforgeCompareNum(a, b) { if (Number.isNaN(a)) return Number.isNaN(b) ? 0 : 1; if (Number.isNaN(b)) return -1; if (a < b) return -1; if (a > b) return 1; if (a === 0 && b === 0) { const na = Object.is(a, -0), nb = Object.is(b, -0); return na === nb ? 0 : (na ? -1 : 1); } return 0; }\n",
    ),
    (
        "$dartforgeCompareString",
        // `<` do JavaScript compara unidades UTF-16, a mesma ordem do Dart.
        "function $dartforgeCompareString(a, b) { return a < b ? -1 : (a > b ? 1 : 0); }\n",
    ),
    (
        "$dartforgeContains",
        "function $dartforgeContains(s, p, start) { $dartforgeInicio(s, start); return s.includes(p, start); }\n",
    ),
    (
        "$dartforgeFaixa",
        "function $dartforgeFaixa(nome, lo, hi, valor) { return new RangeError('RangeError (' + nome + '): Invalid value: Not in inclusive range ' + lo + '..' + hi + ': ' + valor); }\n",
    ),
    (
        "$dartforgeFinito",
        // O oráculo lança `Unsupported operation: Infinity or NaN toInt` em
        // round/floor/ceil/truncate/toInt; `Math.floor(NaN)` devolveria NaN.
        "function $dartforgeFinito(x) { if (!Number.isFinite(x)) throw new Error('Unsupported operation: Infinity or NaN toInt'); return x; }\n",
    ),
    (
        "$dartforgeFloorToInt",
        "function $dartforgeFloorToInt(x) { return Math.floor($dartforgeFinito(x)); }\n",
    ),
    (
        "$dartforgeGcd",
        // `gcd` do Dart é sempre não negativo e `0.gcd(0)` é 0.
        "function $dartforgeGcd(a, b) { a = Math.abs(a); b = Math.abs(b); while (b !== 0) { const r = a % b; a = b; b = r; } return a; }\n",
    ),
    (
        "$dartforgeIndexOf",
        "function $dartforgeIndexOf(s, p, start) { $dartforgeInicio(s, start); return s.indexOf(p, start); }\n",
    ),
    (
        "$dartforgeInicio",
        "function $dartforgeInicio(s, start) { if (!Number.isInteger(start) || start < 0 || start > s.length) throw $dartforgeFaixa('start', 0, s.length, start); }\n",
    ),
    (
        "$dartforgeIntString",
        // `(0 * -1)` é `-0` no JavaScript e o Dart não tem zero negativo em
        // `int`: `+ 0` normaliza antes de formatar, como já faz `$dartforgeString`.
        "function $dartforgeIntString(x) { return (x + 0).toString(10); }\n",
    ),
    (
        "$dartforgeIsInfinite",
        "function $dartforgeIsInfinite(x) { return x === Infinity || x === -Infinity; }\n",
    ),
    (
        "$dartforgeIsNegative",
        // `(-0.0).isNegative` é `true` no oráculo; `-0 < 0` é `false` aqui.
        "function $dartforgeIsNegative(x) { return x < 0 || Object.is(x, -0); }\n",
    ),
    (
        "$dartforgeNumHash",
        // O Dart não fixa o valor; fixar o inteiro e um embaralhamento estável
        // do double preserva o contrato "iguais têm o mesmo hash".
        "function $dartforgeNumHash(x) { if (Number.isInteger(x)) return x | 0; const buffer = new Float64Array(1); buffer[0] = x; const bits = new Int32Array(buffer.buffer); return (bits[0] ^ bits[1]) | 0; }\n",
    ),
    (
        "$dartforgePadLeft",
        // O Dart repete a cadeia inteira `delta` vezes: `'7'.padLeft(5, 'ab')`
        // é `abababab7`, e `padStart` truncaria em cinco caracteres.
        "function $dartforgePadLeft(s, n, pad) { const delta = n - s.length; if (delta <= 0) return s; return pad.repeat(delta) + s; }\n",
    ),
    (
        "$dartforgePadRight",
        "function $dartforgePadRight(s, n, pad) { const delta = n - s.length; if (delta <= 0) return s; return s + pad.repeat(delta); }\n",
    ),
    (
        "$dartforgeRadixString",
        "function $dartforgeRadixString(x, radix) { if (!Number.isInteger(radix) || radix < 2 || radix > 36) throw $dartforgeFaixa('radix', 2, 36, radix); return (x + 0).toString(radix); }\n",
    ),
    (
        "$dartforgeReplaceFirst",
        "function $dartforgeReplaceFirst(s, from, to, start) { $dartforgeInicio(s, start); const at = s.indexOf(from, start); return at < 0 ? s : s.slice(0, at) + to + s.slice(at + from.length); }\n",
    ),
    (
        "$dartforgeReplaceRange",
        "function $dartforgeReplaceRange(s, start, end, rep) { const fim = end === null ? s.length : end; if (!Number.isInteger(start) || start < 0 || start > fim) throw $dartforgeFaixa('start', 0, fim, start); if (!Number.isInteger(fim) || fim > s.length) throw $dartforgeFaixa('end', start, s.length, fim); return s.slice(0, start) + rep + s.slice(fim); }\n",
    ),
    (
        "$dartforgeRoundDouble",
        // Espelha em torno de zero: o Dart arredonda meio para longe de zero e
        // `Math.round` arredonda meio para `+∞`.
        "function $dartforgeRoundDouble(x) { if (!Number.isFinite(x)) return x; return x < 0 ? -Math.round(-x) : Math.round(x); }\n",
    ),
    (
        "$dartforgeRoundToInt",
        "function $dartforgeRoundToInt(x) { $dartforgeFinito(x); return x < 0 ? -Math.round(-x) : Math.round(x); }\n",
    ),
    (
        "$dartforgeSplit",
        "function $dartforgeSplit(s, p) { return new $dartforgeList(s.split(p), ['string']); }\n",
    ),
    (
        "$dartforgeStartsWith",
        "function $dartforgeStartsWith(s, p, start) { $dartforgeInicio(s, start); return s.startsWith(p, start); }\n",
    ),
    (
        "$dartforgeStringBuffer",
        // Acumula os pedaços num vetor e junta só no `toString`: concatenar a
        // cada `write` é quadrático, e um buffer existe justamente para não ser.
        // `length` é o número de unidades UTF-16 já escritas, como no SDK, e não
        // o número de chamadas — por isso é somado na escrita, não contado depois.
        "class $dartforgeStringBuffer {\n  constructor() { this.pedacos = []; this.total = 0; }\n  get $df_length() { return this.total; }\n  get $df_isEmpty() { return this.total === 0; }\n  get $df_isNotEmpty() { return this.total !== 0; }\n  $df_write(value) { const texto = $dartforgeString(value); this.pedacos.push(texto); this.total += texto.length; }\n  $df_writeln(value = '') { this.$df_write(value); this.$df_write('\\n'); }\n  $df_writeCharCode(code) { this.$df_write(String.fromCharCode(code)); }\n  $df_clear() { this.pedacos.length = 0; this.total = 0; }\n  $df_toString() { const texto = this.pedacos.join(''); this.pedacos = [texto]; return texto; }\n}\n",
    ),
    (
        "$dartforgeStringHash",
        // O valor difere do SDK de propósito: o Dart não o especifica e a VM e o
        // dart2js já discordam entre si. O contrato preservado é o único que
        // existe — cadeias iguais têm o mesmo hash.
        "function $dartforgeStringHash(s) { let h = 0; for (let i = 0; i < s.length; i++) { h = (h * 31 + s.charCodeAt(i)) | 0; } return h; }\n",
    ),
    (
        "$dartforgeSubstring",
        "function $dartforgeSubstring(s, start, end) { const fim = end === null ? s.length : end; if (!Number.isInteger(start) || start < 0 || start > fim) throw $dartforgeFaixa('start', 0, fim, start); if (!Number.isInteger(fim) || fim > s.length) throw $dartforgeFaixa('end', start, s.length, fim); return s.slice(start, fim); }\n",
    ),
    (
        "$dartforgeToStringAsFixed",
        // `(-0.0).toStringAsFixed(1)` é `-0.0` no oráculo e `0.0` em `toFixed`.
        "function $dartforgeToStringAsFixed(x, n) { if (!Number.isInteger(n) || n < 0 || n > 20) throw $dartforgeFaixa('fractionDigits', 0, 20, n); const texto = x.toFixed(n); return Object.is(x, -0) ? '-' + texto : texto; }\n",
    ),
    (
        "$dartforgeTruncToInt",
        "function $dartforgeTruncToInt(x) { return Math.trunc($dartforgeFinito(x)); }\n",
    ),
];

/// Dependências entre auxiliares: fechar o conjunto antes de emitir.
const DEPENDENCIAS: &[(&str, &[&str])] = &[
    ("$dartforgeCeilToInt", &["$dartforgeFinito"]),
    ("$dartforgeCodeUnitAt", &["$dartforgeFaixa"]),
    ("$dartforgeContains", &["$dartforgeInicio"]),
    ("$dartforgeFloorToInt", &["$dartforgeFinito"]),
    ("$dartforgeIndexOf", &["$dartforgeInicio"]),
    ("$dartforgeInicio", &["$dartforgeFaixa"]),
    ("$dartforgeRadixString", &["$dartforgeFaixa"]),
    ("$dartforgeReplaceFirst", &["$dartforgeInicio"]),
    ("$dartforgeReplaceRange", &["$dartforgeFaixa"]),
    ("$dartforgeRoundToInt", &["$dartforgeFinito"]),
    ("$dartforgeStartsWith", &["$dartforgeInicio"]),
    ("$dartforgeSubstring", &["$dartforgeFaixa"]),
    ("$dartforgeToStringAsFixed", &["$dartforgeFaixa"]),
    ("$dartforgeTruncToInt", &["$dartforgeFinito"]),
];

/// Emite os auxiliares pedidos, e só eles, em ordem estável.
///
/// A ordem é a alfabética do conjunto, então o texto de um mesmo programa é
/// byte a byte igual entre execuções — a propriedade que os testes de
/// determinismo afirmam. Devolve a cadeia vazia quando nada foi pedido.
pub(super) fn runtime(output: &Output<'_>) -> String {
    if output.nucleo.is_empty() {
        return String::new();
    }
    let mut pedidos: std::collections::BTreeSet<&'static str> = output.nucleo.clone();
    // Fechamento transitivo: um auxiliar pedido arrasta os que ele chama.
    loop {
        let mut novos = Vec::new();
        for pedido in &pedidos {
            if let Ok(indice) = DEPENDENCIAS.binary_search_by(|(nome, _)| nome.cmp(pedido)) {
                for dependencia in DEPENDENCIAS[indice].1 {
                    if !pedidos.contains(dependencia) {
                        novos.push(*dependencia);
                    }
                }
            }
        }
        if novos.is_empty() {
            break;
        }
        pedidos.extend(novos);
    }
    let mut texto = String::new();
    for pedido in &pedidos {
        let indice = AUXILIARES
            .binary_search_by(|(nome, _)| nome.cmp(pedido))
            .expect("auxiliar do núcleo sem fonte registrada");
        texto.push_str(AUXILIARES[indice].1);
    }
    texto
}

/// Indica se algum auxiliar pedido depende do runtime de coleções.
///
/// `codeUnits` e `split` devolvem `List`, que é a classe de `core.js`: pedir um
/// deles obriga o módulo a trazer aquele runtime mesmo que o programa não
/// escreva literal de lista algum.
pub(super) fn precisa_de_colecoes(output: &Output<'_>) -> bool {
    output.nucleo.contains("$dartforgeCodeUnits") || output.nucleo.contains("$dartforgeSplit")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A busca binária das duas tabelas exige ordem estrita.
    #[test]
    fn tabelas_de_auxiliares_ordenadas() {
        for par in AUXILIARES.windows(2) {
            assert!(par[0].0 < par[1].0, "auxiliares fora de ordem em {}", par[1].0);
        }
        for par in DEPENDENCIAS.windows(2) {
            assert!(
                par[0].0 < par[1].0,
                "dependências fora de ordem em {}",
                par[1].0
            );
        }
    }

    /// Todo auxiliar nomeado numa dependência precisa ter fonte registrada.
    #[test]
    fn dependencias_apontam_para_auxiliares_existentes() {
        for (nome, dependencias) in DEPENDENCIAS {
            assert!(
                AUXILIARES.binary_search_by(|(item, _)| item.cmp(nome)).is_ok(),
                "{nome} não tem fonte"
            );
            for dependencia in *dependencias {
                assert!(
                    AUXILIARES
                        .binary_search_by(|(item, _)| item.cmp(dependencia))
                        .is_ok(),
                    "{dependencia} não tem fonte"
                );
            }
        }
    }

    /// Cada fonte define a função — ou a classe — com o nome registrado.
    #[test]
    fn cada_fonte_define_a_propria_funcao() {
        for (nome, fonte) in AUXILIARES {
            assert!(
                fonte.starts_with(&format!("function {nome}("))
                    || fonte.starts_with(&format!("class {nome} ")),
                "{nome} não abre com a própria declaração"
            );
        }
    }
}
