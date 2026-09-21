//! Tabela dos membros de `dart:core` reconhecidos sobre os tipos escalares.
//!
//! Antes deste módulo o compilador reconhecia onze nomes de membro no total, e
//! nenhum deles em `String`, `int` ou `double`: `'abc'.substring(1)` parava em
//! `Unknown instance or extension method`. Código Dart de produção usa centenas
//! de membros, e a contagem no corpus de `references/pub/` diz **quais**:
//! `length` 360 vezes, `add` 212, `isNotEmpty` 87, `substring` 59, `write` 49,
//! `toString` 61, `compareTo` 17. A tabela abaixo cobre os medidos, não um
//! alfabeto completo.
//!
//! # Por que uma tabela estática e não um `HashMap`
//!
//! A resolução de membro fica no caminho quente da análise semântica, que
//! `docs/DESEMPENHO.md` mede em **alocações por compilação**: a estrutura do
//! validador é copiada a cada ramificação de fluxo, então nada que aloque pode
//! entrar nela. Cada tabela aqui é um `&'static [Membro]` ordenado por nome,
//! consultado por busca binária: zero alocação, zero construção por chamada e
//! nada novo para copiar por ramificação. Um `HashMap` por chamada — ou mesmo um
//! único construído na entrada da análise — pagaria alocação em todo programa,
//! inclusive nos que não usam membro algum.
//!
//! A ordenação é invariante do módulo, não convenção: o teste
//! [`tabelas_ordenadas_e_sem_duplicatas`] falha se alguma entrada sair de ordem,
//! porque uma busca binária sobre lista desordenada erraria em silêncio.
//!
//! # Cada membro é uma decisão semântica
//!
//! Nenhuma entrada é tradução direta para JavaScript. O oráculo é o Dart 3.6.2
//! (`dart`) e o Dart 3.13.4, e as divergências que o apagamento para `Number` do
//! JavaScript produz estão listadas em `docs/NUCLEO.md` com o texto exato de cada
//! diagnóstico. Os casos que o alvo web não consegue reproduzir são **recusados
//! em compilação**, nunca emitidos com resultado plausível e errado.
use super::*;

/// Tipo do resultado de um membro reconhecido.
///
/// A maior parte dos resultados é escalar e cabe numa constante. `List<int>` e
/// `List<String>` não cabem: [`Type::Applied`] é um índice na arena de formas da
/// unidade, que só existe em tempo de análise. Essas duas formas viajam como
/// marcas e são internadas na hora do uso.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Resultado {
    /// Tipo escalar, já conhecido em tempo de compilação.
    Escalar(Type),
    /// `List<int>`, de `String.codeUnits`.
    ListaDeInt,
    /// `List<String>`, de `String.split`.
    ListaDeString,
}

/// Membro de um tipo de biblioteca: getter ou método de assinatura fixa.
///
/// Os posicionais obrigatórios precedem os opcionais, como numa assinatura Dart,
/// e `obrigatorios` diz onde termina a parte obrigatória. Nenhum membro desta
/// tabela tem parâmetro nomeado: os que têm em Dart (`List.from` com
/// `growable:`) ficam fora do subconjunto e são recusados com diagnóstico
/// próprio, não aceitos pela metade.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct Membro {
    /// Nome escrito em Dart.
    pub(super) nome: &'static str,
    /// Getter (`s.length`) em vez de método (`s.trim()`).
    pub(super) getter: bool,
    /// Posicionais na ordem escrita; os obrigatórios vêm primeiro.
    pub(super) parametros: &'static [Type],
    /// Quantos dos primeiros `parametros` a chamada precisa fornecer.
    pub(super) obrigatorios: usize,
    /// Tipo produzido pela leitura do getter ou pela chamada do método.
    pub(super) resultado: Resultado,
}

/// Atalho de construção de um getter sem parâmetros.
const fn getter(nome: &'static str, resultado: Type) -> Membro {
    Membro {
        nome,
        getter: true,
        parametros: &[],
        obrigatorios: 0,
        resultado: Resultado::Escalar(resultado),
    }
}

/// Atalho de construção de um método com todos os posicionais obrigatórios.
const fn metodo(nome: &'static str, parametros: &'static [Type], resultado: Type) -> Membro {
    Membro {
        nome,
        getter: false,
        parametros,
        obrigatorios: parametros.len(),
        resultado: Resultado::Escalar(resultado),
    }
}

/// Atalho de construção de um método com posicionais opcionais no fim.
const fn opcional(
    nome: &'static str,
    parametros: &'static [Type],
    obrigatorios: usize,
    resultado: Type,
) -> Membro {
    Membro {
        nome,
        getter: false,
        parametros,
        obrigatorios,
        resultado: Resultado::Escalar(resultado),
    }
}

/// Membros de `String`, ordenados por nome.
///
/// `Pattern` não existe neste subconjunto — não há `RegExp` — então todo
/// parâmetro que o Dart declara como `Pattern` aparece aqui como `String`. Isso
/// não aceita nada que o Dart recuse: recusa `RegExp`, que já não existe.
///
/// Os `int?` do SDK (`substring` e `replaceFirst` têm fim opcional anulável)
/// ficam como [`Type::NullableInt`], de modo que `s.substring(0, 4)` e
/// `s.substring(0, null)` sejam ambos aceitos, como no oráculo.
pub(super) const STRING: &[Membro] = &[
    metodo("codeUnitAt", &[Type::Int], Type::Int),
    Membro {
        nome: "codeUnits",
        getter: true,
        parametros: &[],
        obrigatorios: 0,
        resultado: Resultado::ListaDeInt,
    },
    metodo("compareTo", &[Type::String], Type::Int),
    opcional("contains", &[Type::String, Type::Int], 1, Type::Bool),
    metodo("endsWith", &[Type::String], Type::Bool),
    getter("hashCode", Type::Int),
    opcional("indexOf", &[Type::String, Type::Int], 1, Type::Int),
    getter("isEmpty", Type::Bool),
    getter("isNotEmpty", Type::Bool),
    metodo("lastIndexOf", &[Type::String], Type::Int),
    getter("length", Type::Int),
    opcional("padLeft", &[Type::Int, Type::String], 1, Type::String),
    opcional("padRight", &[Type::Int, Type::String], 1, Type::String),
    metodo("replaceAll", &[Type::String, Type::String], Type::String),
    opcional(
        "replaceFirst",
        &[Type::String, Type::String, Type::Int],
        2,
        Type::String,
    ),
    metodo(
        "replaceRange",
        &[Type::Int, Type::NullableInt, Type::String],
        Type::String,
    ),
    Membro {
        nome: "split",
        getter: false,
        parametros: &[Type::String],
        obrigatorios: 1,
        resultado: Resultado::ListaDeString,
    },
    opcional("startsWith", &[Type::String, Type::Int], 1, Type::Bool),
    opcional("substring", &[Type::Int, Type::NullableInt], 1, Type::String),
    metodo("toLowerCase", &[], Type::String),
    metodo("toString", &[], Type::String),
    metodo("toUpperCase", &[], Type::String),
    metodo("trim", &[], Type::String),
    metodo("trimLeft", &[], Type::String),
    metodo("trimRight", &[], Type::String),
];

/// Membros de `int`, ordenados por nome.
///
/// `round`, `floor`, `ceil` e `truncate` devolvem o próprio valor: em `int` são
/// identidade, e existem na tabela porque código genérico sobre `num` os chama.
/// `bitLength` e `toRadixString` seguem a **representação de 32 bits com sinal**
/// que `docs/COLECOES-OPERADORES.md` já fixou para os operadores de bits; a VM
/// do Dart usa 64 bits e diverge fora dessa faixa, como já divergia lá.
pub(super) const INT: &[Membro] = &[
    metodo("abs", &[], Type::Int),
    getter("bitLength", Type::Int),
    metodo("ceil", &[], Type::Int),
    metodo("compareTo", &[Type::Num], Type::Int),
    metodo("floor", &[], Type::Int),
    metodo("gcd", &[Type::Int], Type::Int),
    getter("hashCode", Type::Int),
    getter("isEven", Type::Bool),
    getter("isFinite", Type::Bool),
    getter("isNaN", Type::Bool),
    getter("isNegative", Type::Bool),
    getter("isOdd", Type::Bool),
    metodo("round", &[], Type::Int),
    getter("sign", Type::Int),
    metodo("toDouble", &[], Type::Double),
    metodo("toInt", &[], Type::Int),
    metodo("toRadixString", &[Type::Int], Type::String),
    metodo("toString", &[], Type::String),
    metodo("truncate", &[], Type::Int),
];

/// Membros de `double`, ordenados por nome.
///
/// `round`/`floor`/`ceil`/`truncate` devolvem `int` e **lançam** sobre `NaN` e
/// infinito, como no oráculo; as variantes `...ToDouble` devolvem `double` e não
/// lançam. `sign` de `double` é `double` (`-1.0`), não `int`.
pub(super) const DOUBLE: &[Membro] = &[
    metodo("abs", &[], Type::Double),
    metodo("ceil", &[], Type::Int),
    metodo("ceilToDouble", &[], Type::Double),
    metodo("compareTo", &[Type::Num], Type::Int),
    metodo("floor", &[], Type::Int),
    metodo("floorToDouble", &[], Type::Double),
    getter("hashCode", Type::Int),
    getter("isFinite", Type::Bool),
    getter("isInfinite", Type::Bool),
    getter("isNaN", Type::Bool),
    getter("isNegative", Type::Bool),
    metodo("round", &[], Type::Int),
    metodo("roundToDouble", &[], Type::Double),
    getter("sign", Type::Double),
    metodo("toDouble", &[], Type::Double),
    metodo("toInt", &[], Type::Int),
    metodo("toString", &[], Type::String),
    metodo("toStringAsFixed", &[Type::Int], Type::String),
    metodo("truncate", &[], Type::Int),
    metodo("truncateToDouble", &[], Type::Double),
];

/// Membros de `num`, ordenados por nome: a interseção de `int` e `double`.
///
/// `abs` e `sign` devolvem `num` porque o valor concreto decide, e `num` é
/// exatamente o tipo que não sabe qual dos dois é. `toString` **não** está aqui:
/// ver [`super::Validator::nucleo_membro`] e `docs/NUCLEO.md`.
pub(super) const NUM: &[Membro] = &[
    metodo("abs", &[], Type::Num),
    metodo("ceil", &[], Type::Int),
    metodo("compareTo", &[Type::Num], Type::Int),
    metodo("floor", &[], Type::Int),
    getter("hashCode", Type::Int),
    getter("isFinite", Type::Bool),
    getter("isInfinite", Type::Bool),
    getter("isNaN", Type::Bool),
    getter("isNegative", Type::Bool),
    metodo("round", &[], Type::Int),
    getter("sign", Type::Num),
    metodo("toDouble", &[], Type::Double),
    metodo("toInt", &[], Type::Int),
    metodo("truncate", &[], Type::Int),
];

/// Membros de `bool`, ordenados por nome.
pub(super) const BOOL: &[Membro] = &[getter("hashCode", Type::Int), metodo("toString", &[], Type::String)];

/// Membros de `StringBuffer`, ordenados por nome.
///
/// `write` aceita `Object?` porque é o que o SDK aceita, e a conversão passa
/// pelo mesmo `toString` de `print` e da interpolação: uma instância sem
/// `String toString()` declarado continua recusada, com o diagnóstico de
/// `docs/OBJETO.md`, em vez de virar `Instance of 'Nome'`.
///
/// `length` é o número de unidades UTF-16 já escritas, não o número de
/// chamadas a `write`.
pub(super) const STRING_BUFFER: &[Membro] = &[
    metodo("clear", &[], Type::Void),
    getter("isEmpty", Type::Bool),
    getter("isNotEmpty", Type::Bool),
    getter("length", Type::Int),
    metodo("toString", &[], Type::String),
    metodo("write", &[Type::NullableObject], Type::Void),
    metodo("writeCharCode", &[Type::Int], Type::Void),
    opcional("writeln", &[Type::NullableObject], 0, Type::Void),
];

/// Procura um membro pelo nome numa tabela ordenada.
///
/// A busca é binária sobre um `&'static [Membro]`: nenhuma alocação e nenhuma
/// tabela construída por chamada. Devolve `None` quando o nome não pertence ao
/// tipo, e quem chama transforma isso no diagnóstico do caso.
pub(super) fn membro(tabela: &'static [Membro], nome: &str) -> Option<&'static Membro> {
    let indice = tabela
        .binary_search_by(|candidato| candidato.nome.cmp(nome))
        .ok()?;
    Some(&tabela[indice])
}

/// Tabela de um tipo escalar, quando ele tem membros reconhecidos.
///
/// `num` aparece separado de `int` e `double` de propósito: as três tabelas
/// diferem no resultado de `abs` e `sign`, e tratar `num` como `double` daria
/// `double` onde o oráculo dá `num`.
pub(super) fn tabela(ty: Type) -> Option<&'static [Membro]> {
    match ty {
        Type::String => Some(STRING),
        Type::Int => Some(INT),
        Type::Double => Some(DOUBLE),
        Type::Num => Some(NUM),
        Type::Bool => Some(BOOL),
        _ => None,
    }
}

/// Nomes de `Object` que este subconjunto recusa, com a razão de cada recusa.
///
/// A recusa é a política, não uma pendência: o texto diz por que o membro não
/// existe, o que aceitá-lo desativaria e o que usar no lugar — o mesmo padrão da
/// recusa de `dynamic`.
pub(super) fn recusa_de_objeto(nome: &str) -> Option<&'static str> {
    match nome {
        "runtimeType" => Some(
            "runtimeType is unsupported: reading it would force every class of the program to carry its Dart name into the emitted JavaScript, because the text is chosen by the object and not by the static type — the same cost that made this subset refuse \"Instance of 'Name'\"; use 'is' or 'as' to recover the concrete type",
        ),
        "noSuchMethod" => Some(
            "noSuchMethod is unsupported: it only has an effect under dynamic dispatch, which this subset refuses on purpose; declare the member, or use 'is' to branch on the concrete type",
        ),
        _ => None,
    }
}

impl<'a> Validator<'a> {
    /// Resolve `receptor.nome` sobre um escalar e fixa o tipo que o emissor lê.
    ///
    /// Devolve `None` quando o receptor não é escalar, ou quando o nome não é
    /// membro dele **e** a forma escrita é uma chamada: aí quem chama continua a
    /// busca por extension, porque uma extension sobre `String` é legítima e a
    /// ausência na tabela não pode escondê-la. Uma **leitura** de nome inexistente
    /// volta como erro, porque getters de extension não estão no subconjunto e
    /// não há mais nada que possa resolvê-la.
    ///
    /// O tipo do receptor é regravado em `Resolution::expr_types` com o limite
    /// superior já resolvido. É o emissor que precisa disso: ele escolhe a forma
    /// JavaScript pelo tipo estático do receptor, e um parâmetro genérico
    /// `T extends String` chegaria lá como `Parameter(n)`, que não diz nada sobre
    /// qual membro emitir.
    pub(super) fn nucleo_escalar(
        &self,
        receptor: &Expr<'a>,
        tipo: Type,
        nome: &str,
        argumentos: Option<&[Expr<'a>]>,
        span: Span,
    ) -> Option<Result<Type, Diagnostic>> {
        if tabela(tipo).is_none() {
            return None;
        }
        self.resolution
            .borrow_mut()
            .expr_types
            .insert((receptor.span.start, receptor.span.end), tipo);
        if let Some(razao) = recusa_de_objeto(nome) {
            return Some(Err(Diagnostic::new(razao, span)));
        }
        if let Some(resultado) = self.nucleo_membro(tipo, nome, argumentos, span) {
            return Some(resultado);
        }
        if argumentos.is_some() {
            return None;
        }
        Some(Err(Diagnostic::new(
            format!(
                "'{}' declares no member '{nome}' in this subset; the recognized members are listed in docs/NUCLEO.md",
                self.nome_do_tipo(tipo)
            ),
            span,
        )))
    }

    /// Diagnóstico de método ausente num escalar, quando nem extension resolveu.
    ///
    /// Quem chama tenta a tabela primeiro e as extensions depois; esta mensagem é
    /// o desfecho quando as duas falham, e diz o tipo em vez de repetir o genérico
    /// `Unknown instance or extension method`.
    pub(super) fn nucleo_metodo_ausente(&self, tipo: Type, nome: &str, span: Span) -> Diagnostic {
        Diagnostic::new(
            format!(
                "'{}' declares no method '{nome}' in this subset, and no extension on it declares one either; the recognized members are listed in docs/NUCLEO.md",
                self.nome_do_tipo(tipo)
            ),
            span,
        )
    }

    /// Resolve um membro de tipo escalar de `dart:core`.
    ///
    /// Devolve `None` quando o receptor **não** tem tabela ou quando o nome não
    /// pertence a ela: nos dois casos quem chama segue com a resolução que já
    /// existia, inclusive a busca por extensions, porque em Dart um membro de
    /// instância tem precedência sobre extension e a ausência aqui não pode
    /// esconder uma extension declarada.
    ///
    /// `argumentos` distingue as duas formas: `None` é a leitura `x.nome` e
    /// `Some(args)` é a chamada `x.nome(args)`. Um método lido sem chamar é
    /// recusado — tear-off de membro de biblioteca não está no subconjunto — e um
    /// getter chamado como método também, porque nenhum resultado desta tabela é
    /// invocável.
    ///
    /// # Erros
    ///
    /// Aridade incompatível, argumento de tipo incompatível, tear-off de método e
    /// chamada de getter. `num.toString()` tem erro próprio: ver
    /// [`Validator::nucleo_num_to_string`].
    pub(super) fn nucleo_membro(
        &self,
        receptor: Type,
        nome: &str,
        argumentos: Option<&[Expr<'a>]>,
        span: Span,
    ) -> Option<Result<Type, Diagnostic>> {
        let tabela = tabela(receptor)?;
        // O apagamento para Number do JavaScript não distingue `1` de `1.0`, e
        // `num` é exatamente o tipo estático que não sabe qual dos dois é.
        if receptor == Type::Num && nome == "toString" {
            return Some(Err(self.nucleo_num_to_string(span)));
        }
        let membro = membro(tabela, nome)?;
        Some(self.nucleo_aplica(receptor, membro, argumentos, span))
    }

    /// Confere aridade e argumentos de um membro já encontrado na tabela.
    fn nucleo_aplica(
        &self,
        receptor: Type,
        membro: &'static Membro,
        argumentos: Option<&[Expr<'a>]>,
        span: Span,
    ) -> Result<Type, Diagnostic> {
        let nome = membro.nome;
        let tipo = self.nome_do_tipo(receptor);
        let Some(argumentos) = argumentos else {
            if !membro.getter {
                return Err(Diagnostic::new(
                    format!(
                        "'{tipo}.{nome}' is a method: a library member tear-off is unsupported in this subset; call it as '{nome}(...)' or wrap it in a closure"
                    ),
                    span,
                ));
            }
            return self.nucleo_resultado(membro, span);
        };
        if membro.getter {
            return Err(Diagnostic::new(
                format!("'{tipo}.{nome}' is a getter and produces a value that cannot be called"),
                span,
            ));
        }
        if argumentos.len() < membro.obrigatorios || argumentos.len() > membro.parametros.len() {
            return Err(Diagnostic::new(
                format!(
                    "'{tipo}.{nome}' takes {} to {} positional arguments, and {} were written",
                    membro.obrigatorios,
                    membro.parametros.len(),
                    argumentos.len()
                ),
                span,
            ));
        }
        for (argumento, esperado) in argumentos.iter().zip(membro.parametros) {
            let real = self.value_expected(argumento, Some(*esperado))?;
            self.require_type(real, *esperado, argumento.span)?;
        }
        self.nucleo_resultado(membro, span)
    }

    /// Materializa o resultado, internando `List<int>` e `List<String>`.
    fn nucleo_resultado(&self, membro: &'static Membro, span: Span) -> Result<Type, Diagnostic> {
        let _ = span;
        Ok(match membro.resultado {
            Resultado::Escalar(ty) => ty,
            Resultado::ListaDeInt => self.intern(TypeShape::List(Type::Int)),
            Resultado::ListaDeString => self.intern(TypeShape::List(Type::String)),
        })
    }

    /// Diagnóstico de `num.toString()`, que o alvo web não consegue honrar.
    ///
    /// No oráculo, `(1 as num).toString()` é `"1"` e `(1.0 as num).toString()` é
    /// `"1.0"`. As duas são o mesmo `Number` no JavaScript, então nenhuma
    /// emissão acerta as duas. Recusar é a única resposta que não inventa texto.
    pub(super) fn nucleo_num_to_string(&self, span: Span) -> Diagnostic {
        Diagnostic::new(
            "'num.toString()' is unsupported: the JavaScript Number erasure cannot tell 1 from 1.0, and the oracle prints '1' for the int and '1.0' for the double; call 'toInt().toString()' or 'toDouble().toString()' to state which text you mean",
            span,
        )
    }

    /// Nome Dart de um tipo escalar, para compor diagnósticos.
    pub(super) fn nome_do_tipo(&self, ty: Type) -> &'static str {
        match ty {
            Type::String => "String",
            Type::Int => "int",
            Type::Double => "double",
            Type::Num => "num",
            Type::Bool => "bool",
            Type::Object => "Object",
            Type::NullableObject => "Object?",
            _ => "value",
        }
    }
}

impl<'a> Validator<'a> {
    /// `Iterable<T>` com o elemento dado.
    fn iteravel_de(&self, elemento: Type) -> Type {
        self.intern(TypeShape::Iterable(elemento))
    }

    /// `List<T>` com o elemento dado.
    fn lista_de(&self, elemento: Type) -> Type {
        self.intern(TypeShape::List(elemento))
    }

    /// Diz se o tipo tem ordenação total utilizável por `sort` sem comparador.
    ///
    /// São os escalares ordenáveis do subconjunto e qualquer classe que declare
    /// `int compareTo(P)` com `P` capaz de receber a própria classe — que é o que
    /// `implements Comparable<Self>` exige. A busca sobe por `extends` e por
    /// `implements`, como qualquer outra resolução de membro.
    pub(super) fn nucleo_comparavel(&self, ty: Type) -> bool {
        match self.upper_bound(ty) {
            Type::Int | Type::Double | Type::Num | Type::String => true,
            Type::Class(id) => self.method(id, "compareTo").is_some_and(|assinatura| {
                !assinatura.is_getter
                    && assinatura.result == Type::Int
                    && assinatura.required_positional == 1
                    && assinatura.parameters.len() == 1
                    && self
                        .require_type(Type::Class(id), assinatura.parameters[0], Span {
                            start: 0,
                            end: 0,
                        })
                        .is_ok()
            }),
            _ => false,
        }
    }

    /// Nome legível de um tipo para diagnóstico, inclusive coleções e classes.
    fn nucleo_rotulo(&self, ty: Type) -> String {
        match self.upper_bound(ty) {
            Type::Class(id) | Type::NullableClass(id) => self
                .classes
                .get(&id)
                .map_or_else(|| "class".to_string(), |info| info.name.to_string()),
            outro => match self.shape(outro) {
                Some(TypeShape::List(t)) => format!("List<{}>", self.nucleo_rotulo(t)),
                Some(TypeShape::Set(t)) => format!("Set<{}>", self.nucleo_rotulo(t)),
                Some(TypeShape::Iterable(t)) => format!("Iterable<{}>", self.nucleo_rotulo(t)),
                Some(TypeShape::Map { key, value }) => format!(
                    "Map<{}, {}>",
                    self.nucleo_rotulo(key),
                    self.nucleo_rotulo(value)
                ),
                _ => self.nome_do_tipo(outro).to_string(),
            },
        }
    }

    /// Leituras de membro de `List`, `Set` e `Iterable` acrescentadas aqui.
    ///
    /// Devolve `None` quando o nome não é um destes: quem chama mantém o
    /// diagnóstico que já emitia.
    pub(super) fn nucleo_iteravel_membro(
        &self,
        receptor: Type,
        elemento: Type,
        nome: &str,
        span: Span,
    ) -> Option<Result<Type, Diagnostic>> {
        let lista = matches!(self.shape(receptor), Some(TypeShape::List(_)));
        Some(match nome {
            // `single` lança quando há zero ou mais de um elemento; o tipo
            // estático é o do elemento, como em `first`.
            "single" => Ok(elemento),
            "hashCode" => Err(self.nucleo_hash_de_colecao(receptor, span)),
            // `reversed` é de `List`: `Iterable` e `Set` não o declaram, e
            // aceitá-lo neles aceitaria código que o Dart recusa.
            "reversed" if lista => Ok(self.iteravel_de(elemento)),
            "reversed" => Err(Diagnostic::new(
                format!(
                    "'{}' declares no 'reversed': only List does; call '.toList().reversed'",
                    self.nucleo_rotulo(receptor)
                ),
                span,
            )),
            "iterator" => Err(Diagnostic::new(
                "reading '.iterator' of a built-in collection is unsupported in this subset: use 'for (var x in ...)', which lowers to the JavaScript iteration protocol; a user class can still declare 'Iterator<T> get iterator' and implement Iterator itself",
                span,
            )),
            "entries" | "runes" | "indices" => Err(self.nucleo_sem_tipo(nome, span)),
            _ => return None,
        })
    }

    /// Leituras de membro de `Map` acrescentadas aqui.
    pub(super) fn nucleo_mapa_membro(
        &self,
        chave: Type,
        valor: Type,
        nome: &str,
        span: Span,
    ) -> Option<Result<Type, Diagnostic>> {
        Some(match nome {
            "isEmpty" => Ok(Type::Bool),
            "isNotEmpty" => Ok(Type::Bool),
            // A ordem de `keys` e `values` é a de inserção, como no
            // `LinkedHashMap` que o Dart usa por padrão para `{}`.
            "keys" => Ok(self.iteravel_de(chave)),
            "values" => Ok(self.iteravel_de(valor)),
            "hashCode" => Err(self.nucleo_hash_de_colecao(
                self.intern(TypeShape::Map { key: chave, value: valor }),
                span,
            )),
            "entries" => Err(self.nucleo_sem_tipo("entries", span)),
            _ => return None,
        })
    }

    /// Diagnóstico de `hashCode` sobre coleção.
    ///
    /// `List`, `Set` e `Map` herdam o `hashCode` de `Object`, que é **identidade**:
    /// duas listas com os mesmos elementos têm hashes diferentes. Reproduzir isso
    /// no JavaScript exige uma tabela lateral de identidades que só existiria por
    /// causa deste membro, e devolver um hash **estrutural** daria a resposta
    /// errada com aparência de certa. A recusa é a política.
    fn nucleo_hash_de_colecao(&self, receptor: Type, span: Span) -> Diagnostic {
        Diagnostic::new(
            format!(
                "'hashCode' on '{}' is unsupported: collections inherit the identity hash of Object — two lists with the same elements have different hashes — and reproducing that would need an identity table kept only for this member; compare with 'identical', or hash the elements you care about",
                self.nucleo_rotulo(receptor)
            ),
            span,
        )
    }

    /// Diagnóstico de membro cujo tipo de retorno não existe no subconjunto.
    fn nucleo_sem_tipo(&self, nome: &str, span: Span) -> Diagnostic {
        let (tipo, alternativa) = match nome {
            "entries" => (
                "MapEntry<K, V>",
                "iterate 'keys' and index the map, or call 'forEach((k, v) { ... })'",
            ),
            "runes" => ("Runes", "use 'codeUnits' for UTF-16 code units"),
            _ => ("a type this subset does not model", "see docs/NUCLEO.md"),
        };
        Diagnostic::new(
            format!(
                "'{nome}' is unsupported because its result type is {tipo}, which this subset does not model; {alternativa}"
            ),
            span,
        )
    }

    /// Métodos de `List`, `Set` e `Iterable` acrescentados aqui.
    ///
    /// Devolve `None` quando o nome não pertence a este conjunto, para que o
    /// caminho anterior — `toList`, `add`, `contains`, `where`, `map`, `any`,
    /// `forEach` — continue respondendo por ele.
    pub(super) fn nucleo_iteravel_metodo(
        &self,
        receptor: Type,
        elemento: Type,
        nome: &str,
        args: &[Expr<'a>],
        span: Span,
    ) -> Option<Result<Type, Diagnostic>> {
        let forma = self.shape(receptor);
        let lista = matches!(forma, Some(TypeShape::List(_)));
        let conjunto = matches!(forma, Some(TypeShape::Set(_)));
        Some(match nome {
            "toString" => self.nucleo_texto_de_colecao(receptor, args, span),
            "join" => self.nucleo_join(receptor, args, span),
            "elementAt" => self.nucleo_um_int(elemento, args, span),
            "skip" | "take" => self
                .nucleo_um_int(Type::Void, args, span)
                .map(|_| self.iteravel_de(elemento)),
            "toSet" => self
                .nucleo_sem_argumentos(nome, args, span)
                .map(|()| self.intern(TypeShape::Set(elemento))),
            "every" => self.nucleo_teste(elemento, args, span).map(|()| Type::Bool),
            "firstWhere" => self.nucleo_teste(elemento, args, span).map(|()| elemento),
            "reduce" => self.nucleo_reduce(elemento, args, span),
            "fold" => self.nucleo_fold(elemento, args, span),
            "expand" => self.nucleo_expand(elemento, args, span),
            "sort" if lista => self.nucleo_sort(elemento, args, span),
            "sort" => Err(Diagnostic::new(
                format!(
                    "'{}' declares no 'sort': only List is ordered in place; call '.toList()' first",
                    self.nucleo_rotulo(receptor)
                ),
                span,
            )),
            "indexOf" if lista => self.nucleo_index_of(elemento, args, span),
            "sublist" if lista => self.nucleo_sublist(elemento, args, span),
            "insert" if lista => self.nucleo_insert(elemento, args, span),
            "removeAt" if lista => self.nucleo_um_int(elemento, args, span),
            "addAll" if lista || conjunto => self.nucleo_add_all(elemento, args, span),
            "remove" if lista || conjunto => {
                self.nucleo_um_objeto(args, span).map(|()| Type::Bool)
            }
            "clear" if lista || conjunto => {
                self.nucleo_sem_argumentos(nome, args, span).map(|()| Type::Void)
            }
            // `Iterable` é preguiçoso e imutável: recusar aqui evita prometer
            // mutação sobre uma sequência que não tem armazenamento próprio.
            "addAll" | "remove" | "clear" | "insert" | "removeAt" | "sublist" | "indexOf" => {
                Err(Diagnostic::new(
                    format!(
                        "'{}' is a lazy sequence and declares no '{nome}': materialize it with '.toList()' first",
                        self.nucleo_rotulo(receptor)
                    ),
                    span,
                ))
            }
            "cast" | "whereType" | "asMap" | "followedBy" | "map" if false => unreachable!(),
            "cast" | "whereType" => Err(Diagnostic::new(
                format!(
                    "'{nome}' is unsupported: it needs a written type argument on a method, and generic methods are outside this subset; declare the target collection with its element type and copy into it"
                ),
                span,
            )),
            _ => return None,
        })
    }

    /// Métodos de `Map` acrescentados aqui.
    pub(super) fn nucleo_mapa_metodo(
        &self,
        receptor: Type,
        chave: Type,
        valor: Type,
        nome: &str,
        args: &[Expr<'a>],
        span: Span,
    ) -> Option<Result<Type, Diagnostic>> {
        Some(match nome {
            "toString" => self.nucleo_texto_de_colecao(receptor, args, span),
            "containsKey" | "containsValue" => {
                self.nucleo_um_objeto(args, span).map(|()| Type::Bool)
            }
            // `Map.remove` devolve o valor removido, ou null quando a chave não
            // existia: o resultado é anulável mesmo com valor não anulável.
            "remove" => self
                .nucleo_um_objeto(args, span)
                .map(|()| self.nullable(valor)),
            "clear" => self
                .nucleo_sem_argumentos(nome, args, span)
                .map(|()| Type::Void),
            "putIfAbsent" => self.nucleo_put_if_absent(chave, valor, args, span),
            "addAll" => self.nucleo_map_add_all(receptor, args, span),
            "forEach" => self.nucleo_map_for_each(chave, valor, args, span),
            "entries" => Err(self.nucleo_sem_tipo("entries", span)),
            _ => return None,
        })
    }

    /// Exige lista de argumentos vazia.
    fn nucleo_sem_argumentos(
        &self,
        nome: &str,
        args: &[Expr<'a>],
        span: Span,
    ) -> Result<(), Diagnostic> {
        if args.is_empty() {
            Ok(())
        } else {
            Err(Diagnostic::new(
                format!("'{nome}' takes no arguments in this subset"),
                span,
            ))
        }
    }

    /// Exige exatamente um `int` e devolve o tipo informado.
    fn nucleo_um_int(
        &self,
        resultado: Type,
        args: &[Expr<'a>],
        span: Span,
    ) -> Result<Type, Diagnostic> {
        if args.len() != 1 {
            return Err(Diagnostic::new("expected exactly one int argument", span));
        }
        let real = self.value_expected(&args[0], Some(Type::Int))?;
        self.require_type(real, Type::Int, args[0].span)?;
        Ok(resultado)
    }

    /// Exige exatamente um argumento compatível com `Object?`.
    fn nucleo_um_objeto(&self, args: &[Expr<'a>], span: Span) -> Result<(), Diagnostic> {
        if args.len() != 1 {
            return Err(Diagnostic::new("expected exactly one argument", span));
        }
        self.value_expected(&args[0], Some(Type::NullableObject))?;
        Ok(())
    }

    /// Exige um `bool Function(E)`.
    fn nucleo_teste(
        &self,
        elemento: Type,
        args: &[Expr<'a>],
        span: Span,
    ) -> Result<(), Diagnostic> {
        if args.len() != 1 {
            return Err(Diagnostic::new("expected exactly one test callback", span));
        }
        let esperado = self.intern(TypeShape::Function {
            result: Type::Bool,
            parameters: vec![elemento],
        });
        let real = self.value_expected(&args[0], Some(esperado))?;
        self.require_type(real, esperado, args[0].span)
    }

    /// `E reduce(E Function(E, E) combine)`.
    fn nucleo_reduce(
        &self,
        elemento: Type,
        args: &[Expr<'a>],
        span: Span,
    ) -> Result<Type, Diagnostic> {
        if args.len() != 1 {
            return Err(Diagnostic::new("'reduce' expects one combine callback", span));
        }
        let esperado = self.intern(TypeShape::Function {
            result: elemento,
            parameters: vec![elemento, elemento],
        });
        let real = self.value_expected(&args[0], Some(esperado))?;
        self.require_type(real, esperado, args[0].span)?;
        Ok(elemento)
    }

    /// `T fold<T>(T inicial, T Function(T, E) combine)` com `T` vindo do inicial.
    ///
    /// O Dart infere `T` do argumento de tipo escrito ou do valor inicial; este
    /// subconjunto não tem métodos genéricos, então **o valor inicial decide** e
    /// o acumulador precisa ter o mesmo tipo do resultado. `fold<T>(...)` com
    /// argumento escrito é recusado antes, pelo parser.
    fn nucleo_fold(
        &self,
        elemento: Type,
        args: &[Expr<'a>],
        span: Span,
    ) -> Result<Type, Diagnostic> {
        if args.len() != 2 {
            return Err(Diagnostic::new(
                "'fold' expects an initial value and a combine callback",
                span,
            ));
        }
        let acumulador = self.value_expected(&args[0], None)?;
        if matches!(acumulador, Type::Void | Type::Inferred | Type::Null) {
            return Err(Diagnostic::new(
                "'fold' takes the accumulator type from its initial value, so the initial value needs a type of its own; annotate it or use a typed local",
                args[0].span,
            ));
        }
        let esperado = self.intern(TypeShape::Function {
            result: acumulador,
            parameters: vec![acumulador, elemento],
        });
        let real = self.value_expected(&args[1], Some(esperado))?;
        self.require_type(real, esperado, args[1].span)?;
        Ok(acumulador)
    }

    /// `Iterable<R> expand(Iterable<R> Function(E) f)`, com `R` vindo do retorno.
    fn nucleo_expand(
        &self,
        elemento: Type,
        args: &[Expr<'a>],
        span: Span,
    ) -> Result<Type, Diagnostic> {
        if args.len() != 1 {
            return Err(Diagnostic::new("'expand' expects one callback", span));
        }
        let contexto = self.intern(TypeShape::Function {
            result: self.iteravel_de(elemento),
            parameters: vec![elemento],
        });
        let real = self.value_expected(&args[0], Some(contexto))?;
        let Some(TypeShape::Function { result, parameters }) = self.shape(real) else {
            return Err(Diagnostic::new("'expand' expects one callback", args[0].span));
        };
        if parameters.len() != 1 {
            return Err(Diagnostic::new(
                "the 'expand' callback takes exactly one element",
                args[0].span,
            ));
        }
        self.require_type(elemento, parameters[0], args[0].span)?;
        let Some(interno) = self.element(result) else {
            return Err(Diagnostic::new(
                "the 'expand' callback must return a List, Set or Iterable",
                args[0].span,
            ));
        };
        Ok(self.iteravel_de(interno))
    }

    /// `void sort([int Function(E, E) compare])`.
    ///
    /// Sem comparador o Dart usa `Comparable.compare`, que **lança em execução**
    /// quando o elemento não é `Comparable` — `dart analyze` aceita
    /// `List<NaoComparavel>.sort()` sem reclamar. Este subconjunto resolve tudo
    /// estaticamente e recusa em compilação: a mensagem nomeia o tipo e oferece a
    /// forma com comparador, que sempre funciona.
    fn nucleo_sort(
        &self,
        elemento: Type,
        args: &[Expr<'a>],
        span: Span,
    ) -> Result<Type, Diagnostic> {
        if args.is_empty() {
            if !self.nucleo_comparavel(elemento) {
                return Err(Diagnostic::new(
                    format!(
                        "'sort()' without a comparator orders by 'compareTo', and '{}' declares none: declare 'int compareTo({} other)' and 'implements Comparable<{}>', or call 'sort((a, b) => ...)'",
                        self.nucleo_rotulo(elemento),
                        self.nucleo_rotulo(elemento),
                        self.nucleo_rotulo(elemento)
                    ),
                    span,
                ));
            }
            return Ok(Type::Void);
        }
        if args.len() != 1 {
            return Err(Diagnostic::new(
                "'sort' takes at most one comparator",
                span,
            ));
        }
        let esperado = self.intern(TypeShape::Function {
            result: Type::Int,
            parameters: vec![elemento, elemento],
        });
        let real = self.value_expected(&args[0], Some(esperado))?;
        self.require_type(real, esperado, args[0].span)?;
        Ok(Type::Void)
    }

    /// `int indexOf(E elemento, [int inicio])`.
    fn nucleo_index_of(
        &self,
        elemento: Type,
        args: &[Expr<'a>],
        span: Span,
    ) -> Result<Type, Diagnostic> {
        if args.is_empty() || args.len() > 2 {
            return Err(Diagnostic::new(
                "'indexOf' takes an element and an optional start index",
                span,
            ));
        }
        self.value_expected(&args[0], Some(self.nullable(elemento)))?;
        if let Some(inicio) = args.get(1) {
            let real = self.value_expected(inicio, Some(Type::Int))?;
            self.require_type(real, Type::Int, inicio.span)?;
        }
        Ok(Type::Int)
    }

    /// `List<E> sublist(int inicio, [int? fim])`.
    fn nucleo_sublist(
        &self,
        elemento: Type,
        args: &[Expr<'a>],
        span: Span,
    ) -> Result<Type, Diagnostic> {
        if args.is_empty() || args.len() > 2 {
            return Err(Diagnostic::new(
                "'sublist' takes a start index and an optional end index",
                span,
            ));
        }
        let inicio = self.value_expected(&args[0], Some(Type::Int))?;
        self.require_type(inicio, Type::Int, args[0].span)?;
        if let Some(fim) = args.get(1) {
            let real = self.value_expected(fim, Some(Type::NullableInt))?;
            self.require_type(real, Type::NullableInt, fim.span)?;
        }
        Ok(self.lista_de(elemento))
    }

    /// `void insert(int indice, E elemento)`.
    fn nucleo_insert(
        &self,
        elemento: Type,
        args: &[Expr<'a>],
        span: Span,
    ) -> Result<Type, Diagnostic> {
        if args.len() != 2 {
            return Err(Diagnostic::new("'insert' takes an index and an element", span));
        }
        let indice = self.value_expected(&args[0], Some(Type::Int))?;
        self.require_type(indice, Type::Int, args[0].span)?;
        let real = self.value_expected(&args[1], Some(elemento))?;
        self.require_type(real, elemento, args[1].span)?;
        Ok(Type::Void)
    }

    /// `void addAll(Iterable<E> outros)`.
    fn nucleo_add_all(
        &self,
        elemento: Type,
        args: &[Expr<'a>],
        span: Span,
    ) -> Result<Type, Diagnostic> {
        if args.len() != 1 {
            return Err(Diagnostic::new("'addAll' takes one iterable", span));
        }
        let esperado = self.iteravel_de(elemento);
        let real = self.value_expected(&args[0], Some(esperado))?;
        self.require_type(real, esperado, args[0].span)?;
        Ok(Type::Void)
    }

    /// `void addAll(Map<K, V> outro)`, com o mesmo par de tipos do receptor.
    fn nucleo_map_add_all(
        &self,
        receptor: Type,
        args: &[Expr<'a>],
        span: Span,
    ) -> Result<Type, Diagnostic> {
        if args.len() != 1 {
            return Err(Diagnostic::new("'addAll' takes one map", span));
        }
        let real = self.value_expected(&args[0], Some(receptor))?;
        self.require_type(real, receptor, args[0].span)?;
        Ok(Type::Void)
    }

    /// `V putIfAbsent(K chave, V Function() fabrica)`.
    fn nucleo_put_if_absent(
        &self,
        chave: Type,
        valor: Type,
        args: &[Expr<'a>],
        span: Span,
    ) -> Result<Type, Diagnostic> {
        if args.len() != 2 {
            return Err(Diagnostic::new(
                "'putIfAbsent' takes a key and a zero-argument factory",
                span,
            ));
        }
        let real = self.value_expected(&args[0], Some(chave))?;
        self.require_type(real, chave, args[0].span)?;
        let esperado = self.intern(TypeShape::Function {
            result: valor,
            parameters: vec![],
        });
        let fabrica = self.value_expected(&args[1], Some(esperado))?;
        self.require_type(fabrica, esperado, args[1].span)?;
        Ok(valor)
    }

    /// `void forEach(void Function(K, V) acao)`.
    fn nucleo_map_for_each(
        &self,
        chave: Type,
        valor: Type,
        args: &[Expr<'a>],
        span: Span,
    ) -> Result<Type, Diagnostic> {
        if args.len() != 1 {
            return Err(Diagnostic::new(
                "'forEach' takes one callback of two arguments",
                span,
            ));
        }
        let esperado = self.intern(TypeShape::Function {
            result: Type::Void,
            parameters: vec![chave, valor],
        });
        let real = self.value_expected(&args[0], Some(esperado))?;
        self.require_type(real, esperado, args[0].span)?;
        Ok(Type::Void)
    }

    /// `String join([String separador])`, com o contrato textual do subconjunto.
    ///
    /// `join` escreve cada elemento pelo `toString` dele, então vale a mesma regra
    /// de `print` e da interpolação: uma instância cuja classe não declara
    /// `String toString()` é **recusada em compilação** em vez de produzir
    /// `Instance of 'Nome'` — ou, pior, o `[object Object]` do JavaScript.
    fn nucleo_join(
        &self,
        receptor: Type,
        args: &[Expr<'a>],
        span: Span,
    ) -> Result<Type, Diagnostic> {
        if args.len() > 1 {
            return Err(Diagnostic::new("'join' takes at most one separator", span));
        }
        if let Some(separador) = args.first() {
            let real = self.value_expected(separador, Some(Type::String))?;
            self.require_type(real, Type::String, separador.span)?;
        }
        self.nucleo_exige_texto(receptor, "join", span)?;
        Ok(Type::String)
    }

    /// `String toString()` de coleção, com o mesmo contrato textual de `join`.
    fn nucleo_texto_de_colecao(
        &self,
        receptor: Type,
        args: &[Expr<'a>],
        span: Span,
    ) -> Result<Type, Diagnostic> {
        self.nucleo_sem_argumentos("toString", args, span)?;
        self.nucleo_exige_texto(receptor, "toString", span)?;
        Ok(Type::String)
    }

    /// Recusa quando algum componente do tipo não tem texto definido.
    fn nucleo_exige_texto(
        &self,
        receptor: Type,
        membro: &str,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if self.printable_type(receptor) {
            return Ok(());
        }
        Err(Diagnostic::new(
            format!(
                "'{membro}' on '{}' has no defined text in this subset: some element type declares no 'String toString()', and emitting the JavaScript \"[object Object]\" — or Dart's \"Instance of 'Name'\" — would be a plausible wrong answer; declare 'String toString()' on it, or map the elements to String first",
                self.nucleo_rotulo(receptor)
            ),
            span,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A busca binária exige ordem: fora dela ela erraria sem falhar.
    #[test]
    fn tabelas_ordenadas_e_sem_duplicatas() {
        for (nome, tabela) in [
            ("String", STRING),
            ("int", INT),
            ("double", DOUBLE),
            ("num", NUM),
            ("bool", BOOL),
            ("StringBuffer", STRING_BUFFER),
        ] {
            for par in tabela.windows(2) {
                assert!(
                    par[0].nome < par[1].nome,
                    "tabela de {nome} fora de ordem entre {} e {}",
                    par[0].nome,
                    par[1].nome
                );
            }
        }
    }

    /// Um getter não tem parâmetros e todo opcional vem depois do obrigatório.
    #[test]
    fn assinaturas_coerentes() {
        for tabela in [STRING, INT, DOUBLE, NUM, BOOL, STRING_BUFFER] {
            for membro in tabela {
                assert!(
                    !membro.getter || membro.parametros.is_empty(),
                    "getter {} não pode declarar parâmetros",
                    membro.nome
                );
                assert!(
                    membro.obrigatorios <= membro.parametros.len(),
                    "{} exige mais posicionais do que declara",
                    membro.nome
                );
            }
        }
    }

    /// A consulta encontra o que existe e não inventa o que não existe.
    #[test]
    fn busca_por_nome_encontra_e_recusa() {
        assert_eq!(membro(STRING, "substring").map(|m| m.obrigatorios), Some(1));
        assert!(membro(STRING, "runtimeType").is_none());
        assert_eq!(
            membro(INT, "sign").map(|m| m.resultado),
            Some(Resultado::Escalar(Type::Int))
        );
        assert_eq!(
            membro(DOUBLE, "sign").map(|m| m.resultado),
            Some(Resultado::Escalar(Type::Double))
        );
        // `num.toString` não existe na tabela: o apagamento para Number não
        // distingue `1` de `1.0`, e a análise recusa com mensagem própria.
        assert!(membro(NUM, "toString").is_none());
        assert!(tabela(Type::Void).is_none());
    }
}
