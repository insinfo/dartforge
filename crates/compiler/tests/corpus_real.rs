//! Aferição contra código Dart de produção, não contra exemplos sintéticos.
//!
//! Uma sondagem de construções isoladas diz quais recursos existem; ela não diz
//! se o compilador aguenta um arquivo de verdade, com dezenas de recursos
//! combinados. Este teste roda o front-end sobre pacotes reais do pub.dev e
//! relata o número **por pacote**: a média de um corpus de um só domínio não
//! descreve nenhum pacote.
//!
//! # O alvo é o projeto que consome a biblioteca, não a biblioteca
//!
//! Uma biblioteca Dart **não declara `main`** — nem o SDK do Dart compila uma
//! biblioteca isolada, por esse mesmo motivo. Compilar `lib/**` como se fosse
//! programa pede algo que não existe, e a rodada anterior contornava isso
//! contando `entrada exige void main()` como sucesso, o que media a chegada ao
//! ligador e não a compilação de nada.
//!
//! O que se compila de um pacote é o **projeto que o consome**: os arquivos de
//! `example/`, `bin/`, `test/` ou `tool/` que declaram `main` e importam a
//! biblioteca por `package:`. Deles o grafo puxa `lib/` pelos imports, de modo
//! que a biblioteca é exercitada como um consumidor real a exercita e o que não
//! é alcançável não entra na conta. Um arquivo é ponto de entrada porque declara
//! `main`, não porque está numa pasta.
//!
//! # Dois modos, porque um só mentiria
//!
//! * **grafo**: cada ponto de entrada real é compilado pelo front-end completo,
//!   com `references/pub/.dart_tool/package_config.json` resolvendo `package:`.
//!   Ele responde a duas perguntas que são diferentes e por isso saem em números
//!   separados: **quantos projetos compilam de ponta a ponta** e **quantos
//!   arquivos de `lib/` o grafo atravessa sem diagnóstico**. As entradas de
//!   `test/` vão em grupo próprio: elas importam `package:test`, dependência de
//!   desenvolvimento que o corpus não baixa por padrão, e somá-las às de
//!   `example/` misturaria duas medidas.
//! * **unidade**: cada arquivo é analisado isoladamente, só até a sintaxe. Não
//!   afirma nada sobre o programa e serve a uma única pergunta — quanto da
//!   sintaxe de produção o parser cobre. É também o modo que exercita **todo**
//!   arquivo do pacote, inclusive os que nenhum ponto de entrada alcança.
//!
//! `DARTFORGE_CORPUS_MODO` escolhe entre `grafo`, `unidade` e `ambos` (padrão).
//!
//! # Três categorias, porque exigem ações diferentes
//!
//! * **lacuna** — o compilador precisa implementar, inclusive nomes de
//!   `dart:core`, que é importada implicitamente em todo arquivo Dart;
//! * **externa** — biblioteca fora do disco: `package:` que o corpus não baixou
//!   ou `dart:` que o carregador ainda não implementa;
//! * **artefato** — consequência do recorte da medição, não do código medido.
//!
//! A categoria vem do **span**: o texto exato que o diagnóstico aponta, e onde
//! aquele nome é declarado no corpus. A rodada anterior descontava artefato pela
//! forma da mensagem, e por isso contou 80 ocorrências de `expected an
//! explicitly supported type` como lacuna quando 73 eram tipos declarados em
//! outro arquivo. A mensagem ainda decide se o compilador estava **procurando um
//! nome** — só ela sabe disso —, mas nunca decide a categoria.
//!
//! `main` ausente é a única exceção que a mensagem resolve sozinha, e resolve
//! para **artefato**: nenhum recurso de linguagem falta ali, o que falhou foi a
//! identificação do ponto de entrada.
//!
//! O corpus não é versionado: é grande e tem licença própria. Baixe com
//! `scripts/corpus.sh --com-dependencias`, que também escreve o
//! `package_config.json`. Sem o corpus, o teste é ignorado em vez de falhar,
//! porque a ausência do corpus não é um defeito do compilador.
use dartforge_compiler::CompileOptions;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Raiz do corpus baixado, se existir com algum pacote dentro.
fn corpus_raiz() -> Option<PathBuf> {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/pub");
    raiz.is_dir().then_some(raiz)
}

/// Coleta recursivamente os arquivos `.dart` de um diretório, em ordem estável.
fn arquivos_dart(raiz: &Path, encontrados: &mut Vec<PathBuf>) {
    let Ok(entradas) = std::fs::read_dir(raiz) else {
        return;
    };
    for entrada in entradas.flatten() {
        let caminho = entrada.path();
        if caminho.is_dir() {
            arquivos_dart(&caminho, encontrados);
        } else if caminho.extension().is_some_and(|e| e == "dart") {
            encontrados.push(caminho);
        }
    }
    encontrados.sort();
}

/// Um pacote baixado do pub.dev, com os arquivos que ele traz.
struct Pacote {
    /// Nome do pacote, como `package:` o nomeia.
    nome: String,
    /// Arquivos `.dart` do pacote, inclusive `test/` e `example/`.
    arquivos: Vec<PathBuf>,
    /// Arquivos de `lib/`: a biblioteca que um consumidor importa.
    ///
    /// Medida à parte das entradas porque a pergunta é outra — cobertura de
    /// linguagem, não "o projeto compila" — e porque nenhum desses arquivos é
    /// compilável isoladamente: biblioteca não tem `main`.
    biblioteca: Vec<PathBuf>,
    /// Pontos de entrada reais, agrupados pela pasta que os traz.
    ///
    /// Vazio num pacote que não traga programa algum; o relatório registra o
    /// caso e mede aquele pacote só em modo unidade, dizendo qual modo usou.
    entradas: Vec<Entradas>,
    /// Entra no relatório por pacote.
    ///
    /// As dependências transitivas precisam estar no disco para que `package:`
    /// resolva, mas relatar os milhares de arquivos delas junto esconderia o
    /// número pedido. Elas ficam presentes e não medidas.
    medido: bool,
}

/// Pontos de entrada de uma pasta do pacote, medidos e relatados juntos.
///
/// A pasta não define o que é entrada — `main` define —, mas define o que o
/// número significa: `example/` é o consumidor típico, `test/` arrasta
/// `package:test` junto. Somar os dois daria uma média que não descreve nenhum.
struct Entradas {
    /// Pasta de origem, que decide o que o número significa.
    pasta: &'static str,
    /// Arquivos daquela pasta que declaram `main` de topo.
    arquivos: Vec<PathBuf>,
}

/// O arquivo declara um `main` de topo?
///
/// Reaproveita o índice de declarações de topo: `void main()`,
/// `Future<void> main(List<String> args) async` e `main()` declaram todas o nome
/// `main` na coluna zero, e uma chamada `main();` dentro de outra função está
/// indentada, logo não conta.
fn declara_main(fonte: &str) -> bool {
    nomes_declarados(fonte).iter().any(|nome| nome == "main")
}

/// Pontos de entrada do pacote, por pasta, na ordem em que o relatório os lê.
///
/// `example/` e `bin/` são programas que consomem a biblioteca como qualquer
/// usuário a consome. `test/` e `tool/` também têm `main`, mas importam
/// `package:test`, dependência de desenvolvimento que `scripts/corpus.sh` só
/// baixa com `--com-dev-dependencias`: por isso vão em grupos separados, nunca
/// somados aos de `example/`.
fn entradas_do_pacote(raiz: &Path) -> Vec<Entradas> {
    const PASTAS: [&str; 4] = ["example", "bin", "test", "tool"];
    let mut grupos = Vec::new();
    for pasta in PASTAS {
        let diretorio = raiz.join(pasta);
        if !diretorio.is_dir() {
            continue;
        }
        let mut candidatos = Vec::new();
        arquivos_dart(&diretorio, &mut candidatos);
        let arquivos: Vec<PathBuf> = candidatos
            .into_iter()
            .filter(|arquivo| {
                std::fs::read_to_string(arquivo).is_ok_and(|fonte| declara_main(&fonte))
            })
            .collect();
        if !arquivos.is_empty() {
            grupos.push(Entradas { pasta, arquivos });
        }
    }
    grupos
}

/// Caminho canônico, ou o original quando o disco não resolve.
///
/// O carregador devolve caminhos canônicos e a varredura de diretórios não; sem
/// normalizar, a interseção entre o que o grafo atravessou e o que está em `lib/`
/// sairia vazia e a cobertura seria relatada como zero.
fn canonico(caminho: &Path) -> PathBuf {
    std::fs::canonicalize(caminho).unwrap_or_else(|_| caminho.to_owned())
}

/// Separa o nome do pacote da versão em `nome-1.2.3`.
///
/// Nomes do pub não têm hífen, então o primeiro hífen seguido de dígito começa
/// a versão — inclusive em pré-lançamentos como `1.0.0-beta.1`.
fn nome_do_diretorio(diretorio: &str) -> String {
    let bytes = diretorio.as_bytes();
    for (indice, byte) in bytes.iter().enumerate() {
        if *byte == b'-' && bytes.get(indice + 1).is_some_and(u8::is_ascii_digit) {
            return diretorio[..indice].to_owned();
        }
    }
    diretorio.to_owned()
}

/// Descobre os pacotes do corpus, marcando quais foram pedidos para medição.
fn pacotes(raiz: &Path) -> Vec<Pacote> {
    // `scripts/corpus.sh` grava os pacotes pedidos na linha de comando; as
    // dependências que ele baixou por arrasto não estão nessa lista. Sem o
    // arquivo — corpus antigo — mede-se tudo, que é o comportamento anterior.
    let medidos: Option<BTreeSet<String>> =
        std::fs::read_to_string(raiz.join(".dart_tool/medidos.txt"))
            .ok()
            .map(|texto| texto.split_whitespace().map(str::to_owned).collect())
            .filter(|nomes: &BTreeSet<String>| !nomes.is_empty());
    let Ok(entradas) = std::fs::read_dir(raiz) else {
        return Vec::new();
    };
    let mut encontrados = Vec::new();
    for entrada in entradas.flatten() {
        let caminho = entrada.path();
        let Some(diretorio) = caminho.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !caminho.is_dir() || diretorio.starts_with('.') {
            continue;
        }
        let nome = nome_do_diretorio(diretorio);
        let mut arquivos = Vec::new();
        arquivos_dart(&caminho, &mut arquivos);
        if arquivos.is_empty() {
            continue;
        }
        let mut biblioteca = Vec::new();
        arquivos_dart(&caminho.join("lib"), &mut biblioteca);
        let entradas = entradas_do_pacote(&caminho);
        let medido = medidos.as_ref().is_none_or(|nomes| nomes.contains(&nome));
        encontrados.push(Pacote {
            nome,
            arquivos,
            biblioteca,
            entradas,
            medido,
        });
    }
    encontrados.sort_by(|a, b| a.nome.cmp(&b.nome));
    encontrados
}

/// Reduz um diagnóstico à sua forma, descartando nomes e números concretos.
///
/// Sem isso, `Unknown identifier 'a'` e `Unknown identifier 'b'` contariam como
/// lacunas diferentes e o ranking perderia o sentido.
fn forma(mensagem: &str) -> String {
    let mut texto = String::with_capacity(mensagem.len());
    let mut em_aspas = false;
    for caractere in mensagem.chars() {
        match caractere {
            '\'' | '"' | '`' => {
                if !em_aspas {
                    texto.push_str("'…'");
                }
                em_aspas = !em_aspas;
            }
            _ if em_aspas => {}
            _ if caractere.is_ascii_digit() => texto.push('#'),
            _ => texto.push(caractere),
        }
    }
    // Mensagens longas de explicação viram uma linha de tabela ilegível.
    let mut recortado: String = texto.trim().chars().take(72).collect();
    if texto.trim().chars().count() > 72 {
        recortado.push('…');
    }
    recortado
}

/// Categoria de um diagnóstico: é ela que decide de quem é o trabalho.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Categoria {
    /// O compilador precisa implementar, inclusive nomes de `dart:core`.
    Lacuna,
    /// Biblioteca fora do disco: `package:` não baixado ou `dart:` ausente.
    Externa,
    /// Consequência do recorte da medição, não do código medido.
    Artefato,
}

impl Categoria {
    /// Rótulo curto usado no relatório.
    fn rotulo(self) -> &'static str {
        match self {
            Categoria::Lacuna => "lacuna",
            Categoria::Externa => "externa",
            Categoria::Artefato => "artefato",
        }
    }
}

/// Onde um nome de topo do corpus é declarado.
struct Origem {
    /// Pacote que declara o nome.
    pacote: String,
    /// Arquivo que declara o nome.
    arquivo: PathBuf,
}

/// Índice de declarações de topo de todo o corpus, por nome.
///
/// É deliberadamente aproximado: existe para **classificar** um diagnóstico, não
/// para resolver nomes. Um nome que ele não indexe cai em "não declarado no
/// corpus", e a evidência impressa no relatório permite conferir caso a caso.
type Indice = BTreeMap<String, Vec<Origem>>;

/// Nomes de topo declarados por um arquivo Dart.
///
/// Declarações de topo em Dart começam na coluna zero; qualquer indentação é
/// membro de classe, que não entra no índice porque não é alcançável por nome
/// simples de outro arquivo.
fn nomes_declarados(fonte: &str) -> Vec<String> {
    const TIPOS: [&str; 5] = ["class", "mixin", "enum", "typedef", "extension"];
    const MODIFICADORES: [&str; 8] = [
        "abstract",
        "base",
        "sealed",
        "interface",
        "external",
        "final",
        "const",
        "late",
    ];
    let mut nomes = Vec::new();
    for linha in fonte.lines() {
        if linha.starts_with([' ', '\t', '/', '*', '}', ')', ']', '@', '#']) || linha.is_empty() {
            continue;
        }
        let mut palavras = linha
            .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
            .filter(|palavra| !palavra.is_empty());
        let mut palavra = palavras.next();
        while palavra.is_some_and(|p| MODIFICADORES.contains(&p)) {
            palavra = palavras.next();
        }
        let Some(mut cabeca) = palavra else { continue };
        // `abstract mixin class BaseClient implements Client` declara a classe
        // `BaseClient`: nessa forma `mixin` é modificador e quem nomeia a
        // declaração é `class`. Sem este passo o índice registrava `class` como
        // nome e `BaseClient` aparecia como nome inexistente no corpus — seis
        // diagnósticos contados como lacuna que são nome de outro arquivo.
        if cabeca == "mixin" && palavras.clone().next() == Some("class") {
            palavras.next();
            cabeca = "class";
        }
        if matches!(cabeca, "import" | "export" | "library" | "part" | "return") {
            continue;
        }
        if TIPOS.contains(&cabeca) {
            // `extension type Nome(...)` e `extension Nome on T`; a extensão
            // anônima (`extension on T`) não declara nome algum.
            let mut nome = palavras.next();
            if cabeca == "extension" && nome == Some("type") {
                nome = palavras.next();
            }
            if let Some(nome) = nome.filter(|n| *n != "on") {
                nomes.push(nome.to_owned());
            }
            if cabeca != "typedef" {
                continue;
            }
            // `typedef int Comparador(int a)`: o nome vem antes do parêntese, e
            // não logo após a palavra-chave.
        }
        // Função ou variável de topo: o nome é o último identificador antes do
        // primeiro `(`, `=` ou `;`, que é o que separa cabeçalho de corpo.
        let fim = linha
            .find(['(', '=', ';', '{'])
            .unwrap_or_else(|| linha.trim_end().len());
        if let Some(nome) = linha[..fim]
            .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
            .rfind(|palavra| !palavra.is_empty())
        {
            nomes.push(nome.to_owned());
        }
    }
    nomes
}

/// Indexa as declarações de topo de todos os pacotes presentes no corpus.
fn indexar(pacotes: &[Pacote]) -> Indice {
    let mut indice: Indice = BTreeMap::new();
    for pacote in pacotes {
        for arquivo in &pacote.arquivos {
            let Ok(fonte) = std::fs::read_to_string(arquivo) else {
                continue;
            };
            for nome in nomes_declarados(&fonte) {
                let origens = indice.entry(nome).or_default();
                if origens.len() < 8 {
                    origens.push(Origem {
                        pacote: pacote.nome.clone(),
                        arquivo: arquivo.clone(),
                    });
                }
            }
        }
    }
    indice
}

/// Nomes que `dart:core` declara e o subconjunto ainda não modela.
///
/// `dart:core` é importada implicitamente em todo arquivo Dart: um nome desta
/// lista que não resolve é trabalho do compilador, não dependência ausente.
const CORE: &[&str] = &[
    "BigInt",
    "Comparable",
    "DateTime",
    "Deprecated",
    "Error",
    "Exception",
    "Expando",
    "Function",
    "IndexError",
    "Invocation",
    "Iterator",
    "MapEntry",
    "Match",
    "Pattern",
    "RangeError",
    "Record",
    "RegExp",
    "RegExpMatch",
    "Runes",
    "StackTrace",
    "StateError",
    "Stopwatch",
    "StringBuffer",
    "StringSink",
    "Symbol",
    "Type",
    "Uri",
    "UriData",
    "identical",
    "identityHashCode",
];

/// Nomes de `dart:async` que o subconjunto ainda não modela.
///
/// `dart:async` já existe para o carregador, ao contrário de `dart:io` ou
/// `dart:convert`: um nome dela que não resolve é membro que falta numa
/// biblioteca presente, não biblioteca a escrever do zero. A categoria é lacuna,
/// e a distinção muda a estimativa de custo, não a categoria.
const ASYNC: &[&str] = &[
    "Completer",
    "EventSink",
    "FutureOr",
    "StreamConsumer",
    "StreamController",
    "StreamIterator",
    "StreamSink",
    "StreamSubscription",
    "StreamTransformer",
    "StreamView",
    "Zone",
    "scheduleMicrotask",
    "unawaited",
];

/// Bibliotecas `dart:` que o carregador ainda não implementa, e o que declaram.
///
/// A tabela não é exaustiva: cobre o que código de produção usa com frequência.
/// Um nome dela que não resolve é dependência ausente, não lacuna de linguagem —
/// e a distinção importa porque implementar `dart:typed_data` é outro trabalho,
/// com outro custo, que não sai da lista de lacunas de sintaxe.
const SDK: &[(&str, &[&str])] = &[
    (
        "dart:typed_data",
        &[
            "ByteBuffer",
            "ByteData",
            "Endian",
            "Float32List",
            "Float64List",
            "Int16List",
            "Int32List",
            "Int64List",
            "Int8List",
            "TypedData",
            "Uint16List",
            "Uint32List",
            "Uint64List",
            "Uint8ClampedList",
            "Uint8List",
        ],
    ),
    (
        "dart:math",
        &[
            "Point",
            "Random",
            "Rectangle",
            "acos",
            "asin",
            "atan",
            "atan2",
            "cos",
            "e",
            "exp",
            "log",
            "max",
            "min",
            "pi",
            "pow",
            "sin",
            "sqrt",
            "tan",
        ],
    ),
    (
        "dart:convert",
        &[
            "AsciiCodec",
            "Base64Codec",
            "Codec",
            "Converter",
            "Encoding",
            "HtmlEscape",
            "JsonCodec",
            "JsonDecoder",
            "JsonEncoder",
            "LineSplitter",
            "Utf8Codec",
            "Utf8Decoder",
            "Utf8Encoder",
            "ascii",
            "base64",
            "base64Decode",
            "base64Encode",
            "base64Url",
            "json",
            "jsonDecode",
            "jsonEncode",
            "latin1",
            "utf8",
        ],
    ),
    (
        "dart:io",
        &[
            "Directory",
            "File",
            "FileMode",
            "FileSystemEntity",
            "HttpClient",
            "HttpServer",
            "IOSink",
            "InternetAddress",
            "Platform",
            "Process",
            "SecurityContext",
            "Socket",
            "stderr",
            "stdin",
            "stdout",
        ],
    ),
    (
        "dart:collection",
        &[
            "DoubleLinkedQueue",
            "DoubleLinkedQueueEntry",
            "HashMap",
            "HashSet",
            "IterableBase",
            "IterableMixin",
            "LinkedHashMap",
            "LinkedHashSet",
            "LinkedList",
            "LinkedListEntry",
            "ListBase",
            "ListMixin",
            "ListQueue",
            "MapBase",
            "MapMixin",
            "MapView",
            "Queue",
            "SetBase",
            "SetMixin",
            "SplayTreeMap",
            "SplayTreeSet",
            "UnmodifiableListView",
            "UnmodifiableMapBase",
            "UnmodifiableMapView",
            "UnmodifiableSetView",
        ],
    ),
    (
        "dart:isolate",
        &[
            "Capability",
            "Isolate",
            "RawReceivePort",
            "ReceivePort",
            "SendPort",
        ],
    ),
    (
        "dart:ffi",
        &[
            "Allocator",
            "Finalizable",
            "NativeType",
            "Pointer",
            "Struct",
        ],
    ),
    (
        "dart:js_interop",
        &[
            "JSAny",
            "JSArray",
            "JSFunction",
            "JSNumber",
            "JSObject",
            "JSPromise",
            "JSString",
        ],
    ),
];

/// Biblioteca `dart:` ausente que declara este nome, quando é o caso.
fn biblioteca_sdk(nome: &str) -> Option<&'static str> {
    SDK.iter()
        .find(|(_, nomes)| nomes.contains(&nome))
        .map(|(biblioteca, _)| *biblioteca)
}

/// Classificação de um diagnóstico, com a evidência que a sustenta.
struct Classe {
    /// Categoria que decide a ação.
    categoria: Categoria,
    /// Motivo sem nomes concretos, para agrupar ocorrências.
    motivo: &'static str,
    /// Texto do span e onde o nome foi achado, para conferência manual.
    evidencia: String,
}

/// Primeiro identificador Dart do trecho apontado pelo span.
fn identificador(trecho: &str) -> Option<&str> {
    let inicio = trecho.find(|c: char| c.is_alphabetic() || c == '_' || c == '$')?;
    let resto = &trecho[inicio..];
    let fim = resto
        .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
        .unwrap_or(resto.len());
    Some(&resto[..fim])
}

/// Um caractere pode fazer parte de um identificador Dart?
fn parte_de_nome(caractere: char) -> bool {
    caractere.is_alphanumeric() || caractere == '_' || caractere == '$'
}

/// Palavras que aparecem **entre** o nome procurado e o span que o diagnóstico
/// aponta, e que portanto não podem ser confundidas com o nome.
const RESERVADAS: &[&str] = &[
    "abstract",
    "base",
    "class",
    "const",
    "covariant",
    "enum",
    "extends",
    "extension",
    "external",
    "factory",
    "final",
    "get",
    "implements",
    "interface",
    "late",
    "mixin",
    "on",
    "operator",
    "required",
    "sealed",
    "set",
    "static",
    "typedef",
    "with",
];

/// Último identificador não reservado antes de um ponto da fonte.
///
/// O span de `unknown superclass` aponta o delimitador do corpo da classe, não o
/// nome: `class A extends B {` marca `{`, e `class A extends B<T> {` marca `<`.
/// Sem esta busca para trás, o nome procurado nunca seria examinado, e foi
/// justamente aí que a rodada anterior contou 375 superclasses de outros
/// arquivos como lacuna de linguagem.
fn identificador_anterior(fonte: &str, limite: usize) -> Option<String> {
    let mut fim = limite.min(fonte.len());
    while fim > 0 && !fonte.is_char_boundary(fim) {
        fim -= 1;
    }
    let mut prefixo = &fonte[..fim];
    // Oito recuos cobrem `class A extends B with M1, M2 implements I {`; mais do
    // que isso significa que o span não tem relação com o nome procurado.
    for _ in 0..8 {
        let cortado = prefixo.trim_end_matches(|c: char| !parte_de_nome(c));
        let inicio = cortado
            .char_indices()
            .rev()
            .take_while(|(_, caractere)| parte_de_nome(*caractere))
            .last()
            .map(|(indice, _)| indice)?;
        let nome = &cortado[inicio..];
        if !RESERVADAS.contains(&nome) {
            return Some(nome.to_owned());
        }
        prefixo = &cortado[..inicio];
    }
    None
}

/// Uma falha a classificar: onde ela aconteceu, com a fonte que o span indexa.
struct Falha<'a> {
    /// Mensagem do diagnóstico, que diz o que o compilador procurava.
    mensagem: &'a str,
    /// Fonte do arquivo a que o span se refere.
    fonte: &'a str,
    /// Primeiro byte do span.
    inicio: usize,
    /// Primeiro byte depois do span.
    fim: usize,
    /// Pacote do arquivo apontado.
    pacote: &'a str,
    /// Arquivo apontado, que pode não ser o ponto de entrada.
    arquivo: &'a Path,
}

impl Falha<'_> {
    /// Texto exato que o diagnóstico aponta.
    fn trecho(&self) -> String {
        trecho_do_span(self.fonte, self.inicio, self.fim)
    }

    /// Nome que o compilador procurava, dentro do span ou imediatamente antes.
    fn nome_procurado(&self) -> Option<String> {
        let trecho = self.trecho();
        if let Some(nome) =
            identificador(&trecho).filter(|nome| !RESERVADAS.contains(nome) && *nome != "dynamic")
        {
            return Some(nome.to_owned());
        }
        identificador_anterior(self.fonte, self.inicio)
    }

    /// Biblioteca ausente que o próprio arquivo importa, se houver.
    ///
    /// Um nome que não existe em arquivo nenhum do disco, num arquivo que importa
    /// biblioteca ausente, é quase certamente declarado nela — `StreamChannel`
    /// num arquivo que importa `package:stream_channel` é o caso típico. É a
    /// única inferência do classificador que não enxerga o nome declarado, e ela
    /// se justifica por vir das **diretivas do próprio arquivo**: o relatório
    /// imprime qual biblioteca sustentou a decisão, para conferência.
    fn biblioteca_ausente(&self, presentes: &BTreeSet<String>) -> Option<String> {
        for linha in self.fonte.lines() {
            if !linha.starts_with("import ") && !linha.starts_with("export ") {
                continue;
            }
            let Some(uri) = uri_no_span(linha) else {
                continue;
            };
            if let Some(biblioteca) = uri.strip_prefix("dart:") {
                if !matches!(biblioteca, "core" | "async" | "ffi") {
                    return Some(format!("dart:{biblioteca}"));
                }
            } else if let Some(resto) = uri.strip_prefix("package:") {
                let pacote = resto.split('/').next().unwrap_or(resto);
                if !presentes.contains(pacote) {
                    return Some(format!("package:{pacote}"));
                }
            }
        }
        None
    }

    /// O arquivo declara este nome sem anotação de tipo?
    ///
    /// `final _nodes = <XmlNode>[];` produz `expected an explicitly supported
    /// type` apontando o nome, e é lacuna do compilador — declaração sem tipo —,
    /// não nome de outro arquivo. Exigir `=`, `;` ou `,` depois do nome separa
    /// esse caso de `final Uint8List buffer;`, onde o nome apontado é o tipo.
    fn declaracao_sem_tipo(&self, nome: &str) -> bool {
        const MODIFICADORES: [&str; 6] =
            ["static ", "external ", "late ", "final ", "const ", "var "];
        self.fonte.lines().any(|linha| {
            let mut resto = linha.trim_start();
            let mut houve = false;
            while let Some(proximo) = MODIFICADORES
                .iter()
                .find_map(|modificador| resto.strip_prefix(modificador))
            {
                resto = proximo.trim_start();
                houve = true;
            }
            houve
                && resto.strip_prefix(nome).is_some_and(|depois| {
                    !depois.starts_with(parte_de_nome)
                        && depois.trim_start().starts_with(['=', ';', ','])
                })
        })
    }
}

/// URI de biblioteca dentro do span, quando o diagnóstico é de uma diretiva.
///
/// Só um texto entre aspas que **pareça** URI de biblioteca conta: a mensagem de
/// uma anotação rejeitada também traz aspas, e confundir as duas classificaria
/// `@Deprecated('…')` como dependência ausente.
fn uri_no_span(trecho: &str) -> Option<&str> {
    let mut resto = trecho;
    while let Some(abre) = resto.find(['\'', '"']) {
        let delimitador = resto.as_bytes()[abre];
        let conteudo = &resto[abre + 1..];
        let fecha = conteudo.find(delimitador as char)?;
        let candidato = &conteudo[..fecha];
        if candidato.starts_with("dart:")
            || candidato.starts_with("package:")
            || candidato.ends_with(".dart")
        {
            return Some(candidato);
        }
        resto = &conteudo[fecha + 1..];
    }
    None
}

/// A mensagem indica que o compilador estava procurando um nome?
///
/// A família vem da mensagem porque só ela sabe o que o compilador procurava; a
/// **categoria** vem do span. `expected an explicitly supported type` entra aqui
/// de propósito: era a linha mais alta da tabela anterior e 73 das 80
/// ocorrências eram nome declarado em outro arquivo.
fn falha_de_nome(mensagem: &str) -> bool {
    mensagem.starts_with("unknown ")
        || mensagem.starts_with("Unknown ")
        || mensagem.contains("expected an explicitly supported type")
        || mensagem.contains("declaração oculta um nome nativo")
}

/// Classifica um diagnóstico pelo que está no span, não pela forma da mensagem.
///
/// A ordem é deliberada: uma URI no span identifica a biblioteca ausente sem
/// ambiguidade; depois vem o nome, decidido por **onde ele é declarado**; o que
/// não é nem URI nem nome procurado é lacuna do compilador.
fn classificar(falha: &Falha<'_>, indice: &Indice, presentes: &BTreeSet<String>) -> Classe {
    // `main` ausente vem antes de tudo, e por dois motivos. Nenhum recurso de
    // linguagem falta ali: o que falhou foi a identificação do ponto de entrada,
    // e uma biblioteca nunca declara `main`. E o span desse diagnóstico é 0..0,
    // de modo que o trecho lido seria o início do arquivo — muitas vezes um
    // `import 'package:…'` —, o que classificaria a falha como dependência
    // ausente se a regra da URI viesse primeiro.
    if falha.mensagem.starts_with("entrada exige void main()")
        || falha.mensagem.contains("expected void main() entrypoint")
    {
        return Classe {
            categoria: Categoria::Artefato,
            motivo: "arquivo sem main de topo tomado por ponto de entrada",
            evidencia: curto(falha.arquivo),
        };
    }
    let trecho = falha.trecho();
    if let Some(uri) = uri_no_span(&trecho) {
        return classificar_uri(uri, presentes);
    }
    if !falha_de_nome(falha.mensagem) {
        return Classe {
            categoria: Categoria::Lacuna,
            motivo: "construção fora do subconjunto",
            evidencia: recorte(&trecho),
        };
    }
    let Some(nome) = falha.nome_procurado() else {
        return Classe {
            categoria: Categoria::Lacuna,
            motivo: "nome procurado sem identificador no span",
            evidencia: recorte(&trecho),
        };
    };
    let nome = nome.as_str();
    if falha.declaracao_sem_tipo(nome) {
        return Classe {
            categoria: Categoria::Lacuna,
            motivo: "declaração sem anotação de tipo no próprio arquivo",
            evidencia: format!("{nome} (sem tipo)"),
        };
    }
    if let Some(origens) = indice.get(nome) {
        if origens.iter().any(|origem| origem.arquivo == falha.arquivo) {
            return Classe {
                categoria: Categoria::Lacuna,
                motivo: "nome declarado no próprio arquivo",
                evidencia: format!("{nome} (declarado aqui)"),
            };
        }
        if let Some(origem) = origens.iter().find(|origem| origem.pacote == falha.pacote) {
            return Classe {
                categoria: Categoria::Artefato,
                motivo: "nome declarado em outro arquivo do mesmo pacote",
                evidencia: format!("{nome} (em {})", curto(&origem.arquivo)),
            };
        }
        let origem = &origens[0];
        return Classe {
            categoria: Categoria::Artefato,
            motivo: "nome declarado em outro pacote do corpus",
            evidencia: format!("{nome} (em package:{})", origem.pacote),
        };
    }
    if CORE.contains(&nome) {
        return Classe {
            categoria: Categoria::Lacuna,
            motivo: "nome de dart:core que o subconjunto não modela",
            evidencia: format!("{nome} (dart:core)"),
        };
    }
    if ASYNC.contains(&nome) {
        return Classe {
            categoria: Categoria::Lacuna,
            motivo: "nome de dart:async que o subconjunto não modela",
            evidencia: format!("{nome} (dart:async)"),
        };
    }
    if let Some(biblioteca) = biblioteca_sdk(nome) {
        return Classe {
            categoria: Categoria::Externa,
            motivo: "nome de biblioteca dart: não implementada",
            evidencia: format!("{nome} ({biblioteca})"),
        };
    }
    if let Some(biblioteca) = falha.biblioteca_ausente(presentes) {
        return Classe {
            categoria: Categoria::Externa,
            motivo: "nome ausente do disco num arquivo que importa biblioteca ausente",
            evidencia: format!("{nome} (o arquivo importa {biblioteca})"),
        };
    }
    Classe {
        categoria: Categoria::Lacuna,
        motivo: "nome não declarado em lugar algum do corpus",
        evidencia: format!("{nome} ({})", recorte(&trecho)),
    }
}

/// Classifica por URI de diretiva: é o que diz qual biblioteca falta.
fn classificar_uri(uri: &str, presentes: &BTreeSet<String>) -> Classe {
    if let Some(biblioteca) = uri.strip_prefix("dart:") {
        // core, async e ffi são as únicas que o carregador conhece hoje; uma
        // falha nelas é lacuna, não dependência ausente.
        let implementada = matches!(biblioteca, "core" | "async" | "ffi");
        return Classe {
            categoria: if implementada {
                Categoria::Lacuna
            } else {
                Categoria::Externa
            },
            motivo: if implementada {
                "diretiva de biblioteca dart: implementada"
            } else {
                "biblioteca dart: não implementada"
            },
            evidencia: format!("dart:{biblioteca}"),
        };
    }
    if let Some(resto) = uri.strip_prefix("package:") {
        let nome = resto.split('/').next().unwrap_or(resto);
        return if presentes.contains(nome) {
            Classe {
                categoria: Categoria::Artefato,
                motivo: "package: presente no corpus que não resolveu",
                evidencia: format!("package:{nome}"),
            }
        } else {
            Classe {
                categoria: Categoria::Externa,
                motivo: "package: que o corpus não baixou",
                evidencia: format!("package:{nome}"),
            }
        };
    }
    Classe {
        categoria: Categoria::Artefato,
        motivo: "URI relativa que não resolveu no disco",
        evidencia: recorte(uri),
    }
}

/// Recorta o trecho do span para uma linha de relatório.
fn recorte(trecho: &str) -> String {
    let limpo: String = trecho
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect();
    let limpo = limpo.trim();
    let mut curto: String = limpo.chars().take(40).collect();
    if limpo.chars().count() > 40 {
        curto.push('…');
    }
    curto
}

/// Nome do arquivo sem o caminho, para caber na linha do relatório.
fn curto(arquivo: &Path) -> String {
    arquivo
        .file_name()
        .map(|nome| nome.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Trecho de fonte apontado por um span, tolerante a limites inválidos.
///
/// Um span vazio — fim de arquivo, por exemplo — não mostraria nada, e a
/// classificação precisa ler algo: nesse caso o trecho segue até o fim do texto
/// que vem depois, o suficiente para reconhecer o token.
fn trecho_do_span(fonte: &str, inicio: usize, fim: usize) -> String {
    let limite = fonte.len();
    let mut inicio = inicio.min(limite);
    let mut fim = fim.clamp(inicio, limite);
    if fim == inicio {
        fim = (inicio + 32).min(limite);
    }
    while inicio < limite && !fonte.is_char_boundary(inicio) {
        inicio += 1;
    }
    while fim < limite && !fonte.is_char_boundary(fim) {
        fim += 1;
    }
    fonte[inicio..fim.max(inicio)].to_owned()
}

/// Pacote a que um arquivo do corpus pertence, pelo diretório `nome-versão`.
fn pacote_do_caminho(caminho: &Path, presentes: &BTreeSet<String>) -> Option<String> {
    caminho.components().find_map(|componente| {
        let texto = componente.as_os_str().to_str()?;
        let nome = nome_do_diretorio(texto);
        (nome != texto && presentes.contains(&nome)).then_some(nome)
    })
}

/// Contagem de um pacote em um modo, com as falhas agrupadas e auditáveis.
#[derive(Default)]
struct Contagem {
    /// Unidades medidas que chegaram ao fim sem diagnóstico.
    ///
    /// No modo grafo a unidade é o ponto de entrada; no modo unidade, o arquivo.
    aceitos: usize,
    /// Arquivos que não são texto UTF-8; nem sucesso nem diagnóstico.
    ilegiveis: usize,
    /// `(categoria, forma, motivo)` → ocorrências, evidência e arquivo exemplo.
    falhas: BTreeMap<(Categoria, String, &'static str), (usize, String, String)>,
    /// Entradas que falharam já na carga do grafo, antes do front-end.
    ///
    /// A distinção decide de quem é o trabalho. Uma entrada que morre na carga
    /// não exercitou análise alguma: o carregador recusou uma diretiva — `dart:`
    /// que não existe, `package:` que não está no disco — e nenhum arquivo de
    /// `lib/` foi atravessado. Somá-la às falhas de front-end sugeriria que o
    /// obstáculo é de linguagem quando é de biblioteca.
    falhas_na_carga: usize,
    /// Arquivos de `lib/` que algum fechamento transitivo alcançou.
    alcancados: BTreeSet<PathBuf>,
    /// Arquivos de `lib/` que algum diagnóstico apontou.
    ///
    /// O front-end para no primeiro erro de cada compilação, então "não
    /// apontado" não é prova de correção; é o que o conjunto de entradas
    /// atravessou sem que nada reprovasse. O relatório diz isso com essas
    /// palavras, para que o número não seja lido como mais forte do que é.
    apontados: BTreeSet<PathBuf>,
}

impl Contagem {
    /// Registra uma falha classificada, guardando o primeiro exemplo.
    fn registrar(&mut self, classe: Classe, mensagem: &str, arquivo: &Path) {
        let chave = (classe.categoria, forma(mensagem), classe.motivo);
        let entrada = self
            .falhas
            .entry(chave)
            .or_insert_with(|| (0, classe.evidencia, curto(arquivo)));
        entrada.0 += 1;
    }

    /// Total de arquivos que terminaram com diagnóstico.
    fn falhas_totais(&self) -> usize {
        self.falhas.values().map(|(quantas, _, _)| quantas).sum()
    }

    /// Total de diagnósticos de uma categoria.
    fn na_categoria(&self, categoria: Categoria) -> usize {
        self.falhas
            .iter()
            .filter(|((cat, _, _), _)| *cat == categoria)
            .map(|(_, (quantas, _, _))| quantas)
            .sum()
    }

    /// Soma outra contagem, para o agregado do corpus.
    fn absorver(&mut self, outra: &Contagem) {
        self.aceitos += outra.aceitos;
        self.ilegiveis += outra.ilegiveis;
        self.falhas_na_carga += outra.falhas_na_carga;
        self.alcancados.extend(outra.alcancados.iter().cloned());
        self.apontados.extend(outra.apontados.iter().cloned());
        for (chave, (quantas, evidencia, exemplo)) in &outra.falhas {
            let entrada = self
                .falhas
                .entry(chave.clone())
                .or_insert_with(|| (0, evidencia.clone(), exemplo.clone()));
            entrada.0 += quantas;
        }
    }

    /// Falhas de uma categoria em ordem decrescente de frequência.
    fn ordenadas(&self, categoria: Categoria) -> Vec<(&str, &'static str, usize, &str, &str)> {
        let mut linhas: Vec<_> = self
            .falhas
            .iter()
            .filter(|((cat, _, _), _)| *cat == categoria)
            .map(|((_, forma, motivo), (quantas, evidencia, exemplo))| {
                (
                    forma.as_str(),
                    *motivo,
                    *quantas,
                    evidencia.as_str(),
                    exemplo.as_str(),
                )
            })
            .collect();
        linhas.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(b.0)).then(a.1.cmp(b.1)));
        linhas
    }
}

/// Mede um grupo de pontos de entrada pelo grafo, com `package:` resolvido.
///
/// `package_config.json` na raiz do corpus é descoberto pela subida de
/// diretórios, então `package:pdf/pdf.dart` resolve a partir de qualquer arquivo.
/// A entrada é um programa que **tem** `main`, e o grafo puxa `lib/` pelos
/// imports: é assim que se verifica se um pacote compila, e é o contrário de
/// tratar cada arquivo da biblioteca como programa.
///
/// Além de contar entradas, registra quais arquivos de `lib/` o fechamento
/// transitivo alcançou e quais algum diagnóstico apontou — os dois lados da
/// cobertura de linguagem.
fn medir_grafo(
    pacote: &Pacote,
    entradas: &[PathBuf],
    biblioteca: &BTreeSet<PathBuf>,
    indice: &Indice,
    presentes: &BTreeSet<String>,
) -> Contagem {
    let mut contagem = Contagem::default();
    for entrada in entradas {
        let (unidades, resultado) =
            dartforge_compiler::compile_path_with_units(entrada, CompileOptions::default());
        // Lista vazia significa que a carga do grafo falhou: nenhuma unidade foi
        // aceita, nem a própria entrada.
        if unidades.is_empty() && resultado.is_err() {
            contagem.falhas_na_carga += 1;
        }
        for unidade in &unidades {
            let unidade = canonico(unidade);
            if biblioteca.contains(&unidade) {
                contagem.alcancados.insert(unidade);
            }
        }
        match resultado {
            Ok(_) => contagem.aceitos += 1,
            Err(erro) => {
                // O diagnóstico pode apontar outro arquivo do fechamento
                // transitivo, inclusive de outro pacote: quem classifica é o
                // arquivo apontado, não o ponto de entrada.
                let fonte = std::fs::read_to_string(&erro.path).unwrap_or_default();
                let (inicio, fim) = erro.span.map_or((0, 0), |span| (span.start, span.end));
                let dono =
                    pacote_do_caminho(&erro.path, presentes).unwrap_or_else(|| pacote.nome.clone());
                let falha = Falha {
                    mensagem: &erro.message,
                    fonte: &fonte,
                    inicio,
                    fim,
                    pacote: &dono,
                    arquivo: &erro.path,
                };
                let classe = classificar(&falha, indice, presentes);
                contagem.registrar(classe, &erro.message, &erro.path);
                let apontado = canonico(&erro.path);
                if biblioteca.contains(&apontado) {
                    contagem.apontados.insert(apontado);
                }
            }
        }
    }
    contagem
}

/// Mede uma lista de arquivos por unidade isolada: só sintaxe, sem resolver nada.
fn medir_unidade(
    pacote: &Pacote,
    arquivos: &[PathBuf],
    indice: &Indice,
    presentes: &BTreeSet<String>,
) -> Contagem {
    let mut contagem = Contagem::default();
    for arquivo in arquivos {
        let Ok(fonte) = std::fs::read_to_string(arquivo) else {
            contagem.ilegiveis += 1;
            continue;
        };
        match dartforge_compiler::compile_unit_diagnostics(&fonte) {
            Ok(()) => contagem.aceitos += 1,
            Err(erro) => {
                let falha = Falha {
                    mensagem: &erro.message,
                    fonte: &fonte,
                    inicio: erro.span.start,
                    fim: erro.span.end,
                    pacote: &pacote.nome,
                    arquivo,
                };
                let classe = classificar(&falha, indice, presentes);
                contagem.registrar(classe, &erro.message, arquivo);
            }
        }
    }
    contagem
}

/// Modo de aferição: o que cada número significa depende dele.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Modo {
    /// Compilação pelo grafo, com `package:` resolvendo.
    Grafo,
    /// Análise sintática de cada arquivo isolado.
    Unidade,
}

impl Modo {
    /// Rótulo do modo no relatório.
    fn rotulo(self) -> &'static str {
        match self {
            Modo::Grafo => "grafo (pontos de entrada com main, package: resolvido)",
            Modo::Unidade => "unidade isolada (só sintaxe, todo arquivo do pacote)",
        }
    }
}

/// Modos pedidos por `DARTFORGE_CORPUS_MODO`.
///
/// # Panics
/// Falha com valor desconhecido, em vez de medir silenciosamente outra coisa.
fn modos() -> Vec<Modo> {
    match std::env::var("DARTFORGE_CORPUS_MODO")
        .unwrap_or_default()
        .as_str()
    {
        "grafo" => vec![Modo::Grafo],
        "unidade" => vec![Modo::Unidade],
        "" | "ambos" => vec![Modo::Grafo, Modo::Unidade],
        outro => panic!("DARTFORGE_CORPUS_MODO desconhecido: {outro}"),
    }
}

/// Imprime as falhas de uma categoria em ordem de frequência.
fn imprimir(contagem: &Contagem, categoria: Categoria, limite: usize) {
    let linhas = contagem.ordenadas(categoria);
    if linhas.is_empty() {
        println!("  {}: nenhuma", categoria.rotulo());
        return;
    }
    println!(
        "  {} — {} diagnósticos em {} formas:",
        categoria.rotulo(),
        contagem.na_categoria(categoria),
        linhas.len()
    );
    for (forma, motivo, quantas, evidencia, exemplo) in linhas.iter().take(limite) {
        println!("    {quantas:4}x {forma}");
        println!("          {motivo}: {evidencia}  [{exemplo}]");
    }
    if linhas.len() > limite {
        println!(
            "    … {} formas com menos ocorrências",
            linhas.len() - limite
        );
    }
}

/// Relata o modo grafo: entradas reais por pasta e cobertura de `lib/`.
///
/// São duas medidas, e elas saem separadas de propósito. "Projetos que compilam
/// de ponta a ponta" responde se o compilador compila código de produção;
/// "arquivos de `lib/` atravessados sem diagnóstico" mostra quanto de linguagem
/// o front-end já cobre. Somar as duas produziria um número que não responde a
/// nenhuma das perguntas.
fn relatar_grafo(medidos: &[&Pacote], indice: &Indice, presentes: &BTreeSet<String>) {
    let mut por_pasta: BTreeMap<&'static str, Contagem> = BTreeMap::new();
    let mut sem_entrada: Vec<&str> = Vec::new();
    let mut lib_total = 0usize;
    let mut lib_alcancados = 0usize;
    let mut lib_atravessados = 0usize;
    for pacote in medidos {
        println!(
            "\npackage:{} — {} arquivos em lib/",
            pacote.nome,
            pacote.biblioteca.len()
        );
        if pacote.entradas.is_empty() {
            // Sem programa no pacote não há o que compilar de ponta a ponta. O
            // relatório registra o caso em vez de inventar uma entrada, e aquele
            // pacote é medido pelo modo unidade, que diz o que mede.
            println!(
                "  sem arquivo com main em example/, bin/, test/ ou tool/: \
                 medido apenas no modo unidade"
            );
            sem_entrada.push(&pacote.nome);
            continue;
        }
        let biblioteca: BTreeSet<PathBuf> = pacote.biblioteca.iter().map(|a| canonico(a)).collect();
        let mut acumulado = Contagem::default();
        for grupo in &pacote.entradas {
            let contagem = medir_grafo(pacote, &grupo.arquivos, &biblioteca, indice, presentes);
            // A propriedade que pode regredir sem ninguém notar: toda entrada
            // termina. Um pânico ou um laço infinito nunca chega aqui, e um
            // desfecho não contabilizado quebra esta igualdade.
            assert_eq!(
                contagem.aceitos + contagem.falhas_totais(),
                grupo.arquivos.len(),
                "pacote {} em {}/: toda entrada precisa terminar com sucesso ou diagnóstico",
                pacote.nome,
                grupo.pasta
            );
            println!(
                "  {}/: {} entradas, {} compiladas de ponta a ponta ({:.1}%), \
                 {} interrompidas na carga do grafo e {} no front-end, \
                 lacuna {} / externa {} / artefato {}",
                grupo.pasta,
                grupo.arquivos.len(),
                contagem.aceitos,
                100.0 * contagem.aceitos as f64 / grupo.arquivos.len() as f64,
                contagem.falhas_na_carga,
                contagem.falhas_totais() - contagem.falhas_na_carga,
                contagem.na_categoria(Categoria::Lacuna),
                contagem.na_categoria(Categoria::Externa),
                contagem.na_categoria(Categoria::Artefato),
            );
            imprimir(&contagem, Categoria::Lacuna, 4);
            por_pasta
                .entry(grupo.pasta)
                .or_default()
                .absorver(&contagem);
            acumulado.absorver(&contagem);
        }
        let atravessados = acumulado
            .alcancados
            .difference(&acumulado.apontados)
            .count();
        println!(
            "  cobertura de lib/: {} de {} arquivos alcançados pelo grafo, \
             {} apontados por diagnóstico, {atravessados} atravessados sem diagnóstico",
            acumulado.alcancados.len(),
            pacote.biblioteca.len(),
            acumulado.apontados.len(),
        );
        if acumulado.alcancados.is_empty() && acumulado.falhas_na_carga > 0 {
            // Sem carga não há travessia, e um zero aqui não mede linguagem
            // alguma: mede que o carregador parou antes. Dizer isso na própria
            // linha evita que o zero seja lido como cobertura nula.
            println!(
                "    nenhuma entrada passou da carga do grafo: este zero mede o \
                 carregador, não a cobertura de linguagem"
            );
        }
        lib_total += pacote.biblioteca.len();
        lib_alcancados += acumulado.alcancados.len();
        lib_atravessados += atravessados;
    }
    for (pasta, contagem) in &por_pasta {
        let entradas = contagem.aceitos + contagem.falhas_totais();
        println!(
            "\ntotal de entradas em {pasta}/: {entradas}, {} compiladas de ponta a ponta ({:.1}%), \
             {} interrompidas na carga do grafo",
            contagem.aceitos,
            100.0 * contagem.aceitos as f64 / entradas as f64,
            contagem.falhas_na_carga
        );
        imprimir(contagem, Categoria::Lacuna, 20);
        imprimir(contagem, Categoria::Externa, 10);
        imprimir(contagem, Categoria::Artefato, 10);
    }
    println!(
        "\ncobertura agregada de lib/: {lib_atravessados} de {lib_total} arquivos atravessados \
         sem diagnóstico, {lib_alcancados} alcançados pelo grafo"
    );
    if !sem_entrada.is_empty() {
        println!("pacotes sem ponto de entrada: {}", sem_entrada.join(", "));
    }
}

/// Relata o modo unidade: todo arquivo do pacote, só até a sintaxe.
fn relatar_unidade(medidos: &[&Pacote], indice: &Indice, presentes: &BTreeSet<String>) {
    let mut agregado = Contagem::default();
    let mut arquivos_totais = 0usize;
    for pacote in medidos {
        let contagem = medir_unidade(pacote, &pacote.arquivos, indice, presentes);
        // A mesma igualdade do modo grafo, aqui sobre **todo** arquivo do
        // pacote: é este modo que cobre os que nenhuma entrada alcança.
        assert_eq!(
            contagem.aceitos + contagem.falhas_totais() + contagem.ilegiveis,
            pacote.arquivos.len(),
            "pacote {}: todo arquivo precisa terminar com sucesso ou diagnóstico",
            pacote.nome
        );
        println!(
            "\npackage:{} — {} arquivos, {} aceitos ({:.1}%), \
             lacuna {} / externa {} / artefato {}",
            pacote.nome,
            pacote.arquivos.len(),
            contagem.aceitos,
            100.0 * contagem.aceitos as f64 / pacote.arquivos.len() as f64,
            contagem.na_categoria(Categoria::Lacuna),
            contagem.na_categoria(Categoria::Externa),
            contagem.na_categoria(Categoria::Artefato),
        );
        imprimir(&contagem, Categoria::Lacuna, 6);
        agregado.absorver(&contagem);
        arquivos_totais += pacote.arquivos.len();
    }
    println!(
        "\ntotal do modo unidade: {arquivos_totais} arquivos, {} aceitos ({:.1}%)",
        agregado.aceitos,
        100.0 * agregado.aceitos as f64 / arquivos_totais as f64
    );
    println!("lacunas reais por frequência, agregadas:");
    imprimir(&agregado, Categoria::Lacuna, 30);
    imprimir(&agregado, Categoria::Externa, 12);
    imprimir(&agregado, Categoria::Artefato, 12);
    if agregado.ilegiveis > 0 {
        println!("  {} arquivos não são texto UTF-8", agregado.ilegiveis);
    }
}

/// Roda o front-end sobre o corpus e resume o resultado por pacote e categoria.
///
/// Não exige sucesso: exige que o resultado seja **conhecido**. Um compilador em
/// construção falha em código de produção; o que não pode acontecer é falhar sem
/// que se saiba onde, nem entrar em pânico, nem travar. A asserção é que cada
/// unidade medida termina — com sucesso, com diagnóstico classificado, ou
/// reconhecida como texto ilegível — e que a soma fecha com o número de unidades:
/// por ponto de entrada no modo grafo, e por arquivo no modo unidade, que é o que
/// cobre **todo** arquivo do pacote, inclusive os que nenhuma entrada alcança.
#[test]
#[ignore = "requer o corpus em references/pub; use scripts/corpus.sh"]
fn pacotes_reais_terminam_e_as_lacunas_saem_classificadas() {
    let Some(raiz) = corpus_raiz() else {
        panic!("corpus ausente: rode scripts/corpus.sh --com-dependencias");
    };
    let pacotes = pacotes(&raiz);
    assert!(
        !pacotes.is_empty(),
        "corpus sem pacotes: rode scripts/corpus.sh --com-dependencias"
    );
    let presentes: BTreeSet<String> = pacotes.iter().map(|p| p.nome.clone()).collect();
    let indice = indexar(&pacotes);
    let medidos: Vec<&Pacote> = pacotes.iter().filter(|p| p.medido).collect();
    assert!(
        !medidos.is_empty(),
        "nenhum pacote marcado para medição em .dart_tool/medidos.txt"
    );
    let apenas_resolucao = pacotes.len() - medidos.len();
    let configuracao = raiz.join(".dart_tool/package_config.json");
    println!(
        "corpus: {} pacotes medidos, {apenas_resolucao} presentes só para resolver package:, \
         {} nomes de topo indexados",
        medidos.len(),
        indice.len()
    );

    for modo in modos() {
        if modo == Modo::Grafo {
            assert!(
                configuracao.is_file(),
                "aferição por grafo exige {}; rode scripts/corpus.sh",
                configuracao.display()
            );
        }
        println!("\n=== modo {} ===", modo.rotulo());
        match modo {
            Modo::Grafo => relatar_grafo(&medidos, &indice, &presentes),
            Modo::Unidade => relatar_unidade(&medidos, &indice, &presentes),
        }
    }
}
