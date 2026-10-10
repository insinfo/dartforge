//! HIR própria do DartForge Nativo (docs/NATIVO.md e PLANO.md § Incremento 28).
//!
//! A HIR é uma representação em CFG (Control Flow Graph) SSA onde:
//! - Chamadas estão resolvidas (estática, seletor de interface, ou dinâmica).
//! - Casts implícitos foram tornados explícitos.
//! - Açúcares sintáticos foram desugarados.
//! - O backend LLVM (e futuramente Cranelift) consome diretamente esta estrutura.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueId(pub u32);

/// Tipos primitivos e gerenciados da HIR nativa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Type {
    /// Inteiro assinado de 64 bits com estouro modular (Dart `int`).
    I64,
    /// Byte de 8 bits (para booleanos no runtime Rust).
    I8,
    /// Ponto flutuante IEEE 754 de 64 bits (Dart `double`).
    F64,
    /// Booleano de 1 bit (Dart `bool`).
    I1,
    /// Handle de objeto gerenciado no heap (rastreado pelo GC, 0 = null).
    Ref,
    /// Retorno vazio.
    Void,
    /// Endereço de um `alloca` (local em memória, R6). Nunca é valor Dart.
    Ptr,
    /// `Float32x4` sem caixa: `<4 x float>` (`docs/SIMD-NATIVO.md`). Só em
    /// locais e temporários de funções síncronas; nas fronteiras (campos,
    /// parâmetros, retornos, coleções) o valor é a caixa `Ref`.
    V4F32,
    /// `Int32x4` sem caixa: `<4 x i32>`.
    V4I32,
    /// `Float64x2` sem caixa: `<2 x double>`.
    V2F64,
}

impl Type {
    /// Tipo LLVM IR textual correspondente.
    pub fn llvm_ir(self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::I8 => "i8",
            Self::F64 => "double",
            Self::I1 => "i1",
            Self::Ref => "i64",
            Self::Void => "void",
            Self::Ptr => "ptr",
            Self::V4F32 => "<4 x float>",
            Self::V4I32 => "<4 x i32>",
            Self::V2F64 => "<2 x double>",
        }
    }

    /// Um dos vetores SIMD sem caixa?
    pub fn e_vetor(self) -> bool {
        matches!(self, Self::V4F32 | Self::V4I32 | Self::V2F64)
    }
}

/// Uma operação SIMD sobre vetores sem caixa, com a semântica pista a pista
/// da VM (a mesma de `runtime/src/simd.rs`). O tipo do vetor é o da
/// instrução ou o do primeiro operando.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpSimd {
    /// `a op b` pista a pista (`+ - * /` nos de ponto flutuante; `+ -` com
    /// volta no estouro e `& | ^` no `Int32x4`).
    Add,
    Sub,
    Mul,
    Div,
    And,
    Or,
    Xor,
    /// `a < b ? a : b` e `a > b ? a : b` (o `Utils::Minimum`/`Maximum` da
    /// VM: com NaN, o segundo operando).
    Min,
    Max,
    /// `max(min(a, hi), lo)`.
    Clamp,
    Neg,
    Abs,
    Sqrt,
    /// `1 / x` e `sqrt(1 / x)`.
    Recip,
    RecipSqrt,
    /// `v * s`, com `s` `double` (arredondado a `float` no `Float32x4`).
    Escala,
    /// Comparação de `Float32x4` que dá a máscara `Int32x4` (-1/0).
    Cmp(FCmpOp),
    /// A pista `i` como `double` (`int` no `Int32x4`).
    Pista(u8),
    /// O vetor com a pista `i` trocada.
    ComPista(u8),
    /// A pista `i` do `Int32x4` como `bool` (≠ 0).
    Flag(u8),
    /// O `Int32x4` com a pista `i` em -1/0 pelo `bool`.
    ComFlag(u8),
    /// O vetor das pistas dadas (`double`, `int` ou `bool`).
    Monta,
    /// `Int32x4.bool(x, y, z, w)`: -1/0 por pista.
    MontaFlags,
    Splat,
    Zero,
    /// Os bits de sinal das pistas, a pista 0 no bit 0.
    SinalMask,
    /// `shuffle(m)` / `shuffleMix(b, m)` com a máscara constante `m`.
    Shuffle(u8),
    ShuffleMix(u8),
    /// `Int32x4.select(t, f)`: bit a bit, `t` onde a máscara tem 1.
    Select,
    /// Os mesmos bits noutro tipo (`fromInt32x4Bits`, `fromFloat32x4Bits`).
    Bits,
    /// `Float64x2.fromFloat32x4` (pistas 0 e 1) e `Float32x4.fromFloat64x2`
    /// (`[x, y, 0, 0]`).
    Converte,
    /// O vetor no endereço `args[0]` + 16 × `args[1]` (lista SIMD).
    Carrega,
    /// Grava `args[2]` no endereço `args[0]` + 16 × `args[1]`.
    Grava,
    // API de `Int32x4` do Dart 3.14 (`docs/SIMD-NATIVO.md` §6), pela
    // extensão marcada `@pragma('dartforge:simd-api', '3.14')`. `Mul`,
    // `Min`, `Max`, `Neg` e `Abs` servem também ao `Int32x4` (inteiro, com
    // volta no estouro; `abs(-2^31)` é `-2^31`).
    /// Comparação com sinal pista a pista de `Int32x4`: máscara -1/0.
    CmpInt(ICmpOp),
    /// `a & ~b`.
    AndNot,
    /// `~a`.
    Not,
    /// `a << (s & 31)` (`false`) ou `a >> (s & 31)` aritmético (`true`),
    /// com `s` `int`.
    Desloca(bool),
    /// Alguma pista ≠ 0 (`anyTrue`) / todas ≠ 0 (`allTrue`): `bool`.
    Algum,
    Todos,
    /// `Float32x4(v.x.toDouble(), v.y.toDouble(), v.z.toDouble(),
    /// v.w.toDouble())` de um `Int32x4` `v` (`lower/simd.rs`): `sitofp`
    /// pista a pista — o mesmo valor, pois o `int` de 32 bits vira `double`
    /// sem perda e o `double` vira `float` com um arredondamento só.
    IntParaFloat,
    /// `fptosi` pista a pista de um `Float32x4` cujas pistas estão todas em
    /// `[-2^31, 2^31)` (conferido antes por [`OpSimd::NaFaixaInt32`]): o
    /// `toInt()` de cada pista, que aí não lança nem satura.
    FloatParaInt,
    /// Todas as pistas do `Float32x4` em `[-2^31, 2^31)` (NaN não está):
    /// `bool`.
    NaFaixaInt32,
}

/// Constantes suportadas na HIR.
#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    Int(i64),
    Double(f64),
    Bool(bool),
    String(String),
    /// Bytes WTF-8 de um literal Dart; preserva surrogates isolados.
    StringWtf8(Vec<u8>),
    Null,
    /// O endereço da função `símbolo` como `i64` (`ptrtoint`): a mesma em
    /// todos os isolados do processo.
    Funcao(String),
}

/// Operando de instrução: uma constante direta ou um resultado SSA anterior.
#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    Constant(Constant),
    Val(ValueId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ICmpOp {
    Eq,
    Ne,
    Slt,
    Sle,
    Sgt,
    Sge,
    /// Menor sem sinal: `i u< n` testa `0 <= i < n` de uma vez.
    Ult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FCmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Slot proprietário classificado; não aceita ponteiro nativo arbitrário.
///
/// Quadros são IDs escalares da ABI ARC, com slots Ref inicialmente nulos.
/// O runtime confere identidade, propriedade e limite do índice.
/// Slots de heap/nativos ainda exigem seus próprios descritores.
///
/// ```
/// use dartforge_emit_native::hir::*;
/// let slot = SlotForte::Quadro { quadro: Operand::Val(ValueId(0)), indice: 2 };
/// assert!(matches!(slot, SlotForte::Quadro { indice: 2, .. }));
/// ```
#[derive(Debug, Clone)]
pub enum SlotForte {
    /// Slot Ref num quadro proprietário aberto pela ABI, identificado por SSA I64.
    Quadro { quadro: Operand, indice: u32 },
    /// Armazenamento global Ref declarado exatamente uma vez no módulo.
    /// Inicialização lazy continua sendo responsabilidade do lowering.
    ///
    /// ```
    /// use dartforge_emit_native::hir::*;
    /// let slot = SlotForte::Global { simbolo: "dfg.recurso".into() };
    /// assert!(matches!(slot, SlotForte::Global { .. }));
    /// ```
    Global { simbolo: String },
}

/// Como publicar uma referência no slot proprietário.
///
/// ```
/// use dartforge_emit_native::hir::ModoStoreForte;
/// assert_ne!(ModoStoreForte::Copy, ModoStoreForte::Move);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModoStoreForte {
    /// Retém uma ocorrência independente; não consome o token SSA.
    Copy,
    /// Consome o token SSA no sucesso, transferindo sua ocorrência.
    Move,
}

/// Instruções que produzem um valor (ou efetuam efeito de escrita).
#[derive(Debug, Clone)]
pub enum Instruction {
    /// Carrega uma constante em um identificador SSA.
    Const(Constant),

    /// Copia uma referência já avaliada e produz token owned independente.
    /// Retém sem coletar. O operando é SSA Ref ou null; não cria caixas.
    ///
    /// ```
    /// use dartforge_emit_native::hir::*;
    /// let copia = Instruction::ArcCopy { value: Operand::Val(ValueId(0)) };
    /// assert!(matches!(copia, Instruction::ArcCopy { .. }));
    /// ```
    ArcCopy { value: Operand },
    /// Consome um token owned, sem coletar ou executar Dart.
    /// O verificador de tokens deverá provar disponibilidade e consumo único.
    ///
    /// ```
    /// use dartforge_emit_native::hir::*;
    /// let drop = Instruction::ArcDrop { value: Operand::Val(ValueId(1)) };
    /// assert!(matches!(drop, Instruction::ArcDrop { .. }));
    /// ```
    ArcDrop { value: Operand },
    /// Transfere o token para o resultado Ref sem alterar RC físico.
    /// Consome a origem lógica; preservar a operação até verificar ownership.
    ///
    /// ```
    /// use dartforge_emit_native::hir::*;
    /// let movimento = Instruction::ArcMove { value: Operand::Val(ValueId(1)) };
    /// assert!(matches!(movimento, Instruction::ArcMove { .. }));
    /// ```
    ArcMove { value: Operand },

    /// Carrega slot classificado e produz token owned independente do slot.
    ///
    /// ```
    /// use dartforge_emit_native::hir::*;
    /// let slot = SlotForte::Quadro { quadro: Operand::Val(ValueId(0)), indice: 0 };
    /// let carga = Instruction::ArcLoadStrong { slot };
    /// assert!(matches!(carga, Instruction::ArcLoadStrong { .. }));
    /// ```
    ArcLoadStrong { slot: SlotForte },
    /// Publica a referência já avaliada antes de liberar o conteúdo antigo.
    ///
    /// ```
    /// use dartforge_emit_native::hir::*;
    /// let slot = SlotForte::Quadro { quadro: Operand::Val(ValueId(0)), indice: 0 };
    /// let store = Instruction::ArcStoreStrong {
    ///     slot, value: Operand::Constant(Constant::Null), modo: ModoStoreForte::Copy,
    /// };
    /// assert!(matches!(store, Instruction::ArcStoreStrong { .. }));
    /// ```
    ArcStoreStrong { slot: SlotForte, value: Operand, modo: ModoStoreForte },

    // Aritmética e lógica inteira
    Add(Operand, Operand),
    Sub(Operand, Operand),
    Mul(Operand, Operand),
    SDiv(Operand, Operand), // ~/
    SRem(Operand, Operand), // %
    Shl(Operand, Operand),
    AShr(Operand, Operand),
    /// `>>>`: deslocamento lógico.
    LShr(Operand, Operand),
    And(Operand, Operand),
    Or(Operand, Operand),
    Xor(Operand, Operand),
    Neg(Operand),
    Not(Operand), // ~

    // Aritmética de ponto flutuante
    FAdd(Operand, Operand),
    FSub(Operand, Operand),
    FMul(Operand, Operand),
    FDiv(Operand, Operand), // /
    FNeg(Operand),

    // Comparações
    ICmp(ICmpOp, Operand, Operand),
    FCmp(FCmpOp, Operand, Operand),
    LNot(Operand), // ! (booleano)

    // Conversões primitivas
    IntToDouble(Operand),
    DoubleToInt(Operand),
    ZExt { op: Operand, from: Type, to: Type },
    Trunc { op: Operand, from: Type, to: Type },
    /// Reinterpreta os bits entre `i64` e `double` (valor lido do heap ou
    /// gravado nele). Não é conversão numérica: essa é `IntToDouble`.
    Bitcast { op: Operand, to: Type },
    /// Escalar (`I64`/`F64`/`I1`) numa posição `Ref`: caixa no heap (R3).
    Box { op: Operand, from: Type },
    /// Caixa de volta ao escalar; null ou outro tipo lança `TypeError`
    /// (exceção pendente — quem emite verifica, como numa chamada).
    Unbox { op: Operand, to: Type },

    // Alocações de heap
    AllocObject {
        class_id: u32,
        fields: Vec<Operand>,
    },
    AllocList {
        elements: Vec<(Operand, u8)>,
    },
    AllocRecord {
        elements: Vec<(Operand, u8)>,
    },
    AllocCell {
        value: Operand,
    },
    AllocEnv {
        values: Vec<Operand>,
    },
    /// A interpolação `'a$b c'`: as partes numa só string, com uma alocação
    /// (`dartforge_string_juntar_tipado`), não uma por concatenação. Cada
    /// parte é um texto (`Ref`) ou um `int` sem caixa (`I64`), escrito em
    /// decimal direto no resultado.
    JuntarTextos {
        partes: Vec<Operand>,
    },
    AllocClosure {
        code_symbol: String,
        env: Operand,
    },
    /// Closure cujo corpo tem ABI tipada (`lower/closures.rs`): além da
    /// entrada uniforme `code_symbol`, o corpo `tipado`
    /// `(env, p0…) -> r` nas representações da HIR, com o código `abi`
    /// delas, que a chamada tipada confere.
    AllocClosureTipada {
        code_symbol: String,
        env: Operand,
        tipado: String,
        abi: i64,
        /// `env` é o único valor capturado, guardado no lugar do ambiente
        /// (`lower/closures.rs`, ambiente direto): não é um ambiente.
        direto: bool,
    },
    /// Chamada do corpo tipado de uma closure: `alvo` é o endereço (`I64`,
    /// de `dartforge_closure_tipada`), os argumentos vão nas representações
    /// da HIR (o primeiro é o ambiente) e o resultado volta em `ret`. A
    /// exceção sai pela pendência, como numa chamada Dart.
    ChamadaTipada {
        alvo: Operand,
        args: Vec<(Operand, Type)>,
        ret: Type,
    },

    // Memória local e ponteiros de pilha
    Alloca(Type),
    Load {
        ptr: Operand,
        ty: Type,
    },
    Store {
        ptr: Operand,
        val: Operand,
    },

    // Acesso a propriedades e posições
    GetField {
        object: Operand,
        index: usize,
    },
    SetField {
        object: Operand,
        index: usize,
        value: Operand,
    },
    GetListElement {
        list: Operand,
        index: Operand,
    },
    SetListElement {
        list: Operand,
        index: Operand,
        value: Operand,
    },
    CellGet {
        cell: Operand,
    },
    CellSet {
        cell: Operand,
        value: Operand,
    },
    EnvGet {
        env: Operand,
        index: usize,
    },

    // Chamadas
    CallStatic {
        symbol: String,
        args: Vec<Operand>,
        ret_ty: Type,
    },
    CallInterface {
        receiver: Operand,
        selector_id: u32,
        selector_name: String,
        args: Vec<Operand>,
        ret_ty: Type,
    },
    CallDynamic {
        receiver: Operand,
        selector_name: String,
        args: Vec<Operand>,
        ret_ty: Type,
    },
    /// Chamada de um valor função (closure, tear-off) pela convenção
    /// uniforme: todos os argumentos `Ref`, os posicionais primeiro e depois
    /// os nomeados na ordem de `nomes` (ordenados); o resultado é `Ref`. O
    /// emissor monta o vetor de argumentos e o descritor
    /// (`[n_posicionais, n_nomeados, hash(nome)…]`) e chama a entrada
    /// uniforme da closure pelo endereço guardado nela (`@df_clo_invalido`
    /// quando o valor não é closure). Como na chamada por seletor, o vetor
    /// tem um slot a mais, depois dos argumentos, com a tupla RTI dos
    /// argumentos de tipo (`tupla_tipos`, `I64`; `0` quando a chamada não
    /// passa nenhum): é dela que uma closure genérica lê os seus.
    CallClosure {
        closure: Operand,
        args: Vec<Operand>,
        nomes: Vec<String>,
        ret_ty: Type,
        tupla_tipos: Operand,
    },
    CallRuntime {
        name: String,
        args: Vec<(Operand, Type)>,
        ret_ty: Type,
    },

    // Verificação e cast de tipo
    CheckNotNull(Operand),
    IsClass {
        object: Operand,
        class_id: u32,
    },

    // Globais do módulo (variáveis de topo e campos estáticos — N6)
    LoadGlobal {
        simbolo: String,
        ty: Type,
    },
    /// `raiz`: id do global `Ref`, que o runtime mantém como raiz
    /// permanente (N6/G6); `None` para a bandeira e para escalares.
    StoreGlobal {
        simbolo: String,
        val: Operand,
        ty: Type,
        raiz: Option<u32>,
    },

    /// Operação SIMD sem caixa (`OpSimd`); o tipo do resultado é o da
    /// instrução.
    Simd {
        op: OpSimd,
        args: Vec<Operand>,
    },

    // Phi node
    Phi {
        incoming: Vec<(BlockId, Operand)>,
        ty: Type,
    },

    // --- Closures (P1, docs/NATIVO-PLANO.md §7.4) -----------------------
    /// O tear-off canônico da função cuja entrada uniforme é `code_symbol`
    /// (o mesmo handle sempre: `identical(f, f)`).
    TearOff {
        code_symbol: String,
    },
    /// `base[index]` de um vetor de `i64` (argumentos ou descritor da
    /// convenção uniforme). O tipo registrado diz a representação lida
    /// (`Ref` para um argumento, `I64` para um campo do descritor).
    LoadIndexed {
        base: Operand,
        index: Operand,
    },
    /// Endereço (`Ptr`) de um vetor constante de `i64`, global do módulo
    /// (a assinatura de uma entrada uniforme para a checagem de aridade).
    ConstArray(Vec<i64>),
    /// Endereço (`Ptr`) de um vetor constante com o endereço de cada função
    /// (`@df.fns.<k>`, global do módulo): os getters das constantes de um
    /// literal grande que vira tabela (`lower/literais.rs`,
    /// `preencher_de_tabela`), que o runtime chama.
    TabelaDeFuncoes(Vec<String>),

    // --- P5c (SDK da fonte, δ) ------------------------------------------
    /// Chamada de membro de instância pelo **seletor** (`c:m`, `g:x`,
    /// `s:x`, `lower/sdk_fonte.rs`), no mundo aberto: a implementação sai da
    /// tabela de métodos da classe dinâmica do receptor (registrada no
    /// runtime por classe), com um cache por ponto de chamada. A convenção é
    /// a uniforme das closures: todos os argumentos `Ref`, posicionais e
    /// depois nomeados na ordem de `nomes`; o resultado é `Ref`. Receptor sem
    /// o membro: `NoSuchMethodError` pendente.
    CallSeletor {
        seletor: String,
        recv: Operand,
        args: Vec<Operand>,
        nomes: Vec<String>,
        /// Tupla RTI do método genérico, no slot oculto após os argumentos.
        tupla_tipos: Operand,
    },
    /// Chama o valor função `closure` repassando o vetor de argumentos e o
    /// descritor já montados (os parâmetros de uma entrada uniforme): o
    /// adaptador `c:x` de um getter ou campo chama o valor lido.
    CallClosureRepasse {
        closure: Operand,
        args: Operand,
        desc: Operand,
    },
    /// `recv.<seletor>` repassando o vetor de argumentos e o descritor já
    /// montados (os de uma entrada uniforme, com a tupla de tipos no slot
    /// depois dos argumentos): a entrada do tear-off de um método chama a
    /// implementação do receptor, que confere os argumentos e completa os
    /// opcionais com os padrões dela. Resultado `Ref`.
    CallSeletorRepasse {
        seletor: String,
        recv: Operand,
        args: Operand,
        desc: Operand,
    },
    /// Chamada a uma função nativa (C) no endereço `alvo` (`i64`), com a
    /// ABI C do alvo (`dart:ffi`, `lower/ffi.rs`). Os argumentos vêm na
    /// representação da HIR — inteiros e ponteiros `I64`, ponto flutuante
    /// `F64`, `bool` `I1` — e são convertidos ao tipo C; o resultado volta
    /// do mesmo jeito (inteiros estendidos a 64 bits pelo sinal do tipo C).
    ChamadaNativa {
        alvo: Operand,
        args: Vec<(Operand, TipoC)>,
        ret: TipoC,
    },
    /// O elemento `indice` (tipo C `tipo`) da memória em `endereco` (`I64`):
    /// o caminho rápido de uma lista tipada, com os limites já conferidos.
    /// O resultado vem na representação Dart do tipo (`I64`, `F64`).
    CargaNativa {
        endereco: Operand,
        indice: Operand,
        tipo: TipoC,
    },
    /// Grava `valor` (representação Dart) como o elemento `indice` do tipo
    /// C `tipo` em `endereco`.
    GravacaoNativa {
        endereco: Operand,
        indice: Operand,
        tipo: TipoC,
        valor: Operand,
    },
    /// Uma chamada nativa com structs ou unions por valor, ou variádica: o operando de um
    /// composto é o endereço (`I64`) dos bytes dele, e um retorno composto é
    /// gravado em `destino` (o resultado da instrução é então `Void`). O
    /// emissor aplica a ABI C do alvo (`llvm/abi_c.rs`).
    ChamadaNativaComposta {
        alvo: Operand,
        args: Vec<(Operand, TipoNativo)>,
        ret: TipoNativo,
        destino: Option<Operand>,
        /// Função variádica (`VarArgs`): quantos parâmetros são fixos; os
        /// seguintes vão com as promoções de argumento do C.
        variadica: Option<usize>,
    },
}

/// Um tipo na fronteira nativa: primitivo ou composto por valor.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TipoNativo {
    Prim(TipoC),
    Composto(LayoutC),
}

/// O layout C de uma struct ou union: tamanho, alinhamento e as folhas
/// primitivas (deslocamento, tipo), com structs aninhadas e arrays
/// achatados — o que a classificação da ABI examina.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LayoutC {
    pub tamanho: usize,
    pub alinhamento: usize,
    pub folhas: Vec<(usize, TipoC)>,
}

impl LayoutC {
    /// Alguma folha fora do alinhamento natural (`@Packed`).
    pub fn desalinhado(&self) -> bool {
        self.folhas.iter().any(|(o, t)| {
            let n = t.tamanho_c();
            n > 1 && o % n != 0
        })
    }
}

impl TipoNativo {
    /// A representação na HIR do valor Dart correspondente (um composto
    /// chega como endereço).
    pub fn tipo_hir(&self) -> Type {
        match self {
            TipoNativo::Prim(t) => t.tipo_hir(),
            TipoNativo::Composto(_) => Type::I64,
        }
    }
}

/// Um tipo C na fronteira de uma chamada nativa (os primitivos do
/// `dart:ffi`, com os inteiros específicos da ABI já resolvidos).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TipoC {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
    Bool,
    Ptr,
    /// `Handle`: um objeto Dart na fronteira nativa, como o ponteiro opaco
    /// de uma célula do runtime (o `Dart_Handle` da VM).
    Handle,
    Void,
}

impl TipoC {
    /// O tipo LLVM do valor C.
    pub fn llvm(self) -> &'static str {
        match self {
            TipoC::I8 | TipoC::U8 => "i8",
            TipoC::I16 | TipoC::U16 => "i16",
            TipoC::I32 | TipoC::U32 => "i32",
            TipoC::I64 | TipoC::U64 => "i64",
            TipoC::F32 => "float",
            TipoC::F64 => "double",
            TipoC::Bool => "i1",
            TipoC::Ptr | TipoC::Handle => "ptr",
            TipoC::Void => "void",
        }
    }

    /// A representação na HIR do valor Dart correspondente.
    pub fn tipo_hir(self) -> Type {
        match self {
            TipoC::F32 | TipoC::F64 => Type::F64,
            TipoC::Bool => Type::I1,
            TipoC::Void => Type::Void,
            _ => Type::I64,
        }
    }

    /// O atributo de extensão de um parâmetro estreito (quem chama estende:
    /// exigido pela ABI da Apple em arm64 e inócuo nas outras).
    pub fn extensao(self) -> &'static str {
        match self {
            TipoC::I8 | TipoC::I16 => "signext ",
            TipoC::U8 | TipoC::U16 | TipoC::Bool => "zeroext ",
            _ => "",
        }
    }

    /// Bytes do tipo C (0 para `void`).
    pub fn tamanho_c(self) -> usize {
        match self {
            TipoC::I8 | TipoC::U8 | TipoC::Bool => 1,
            TipoC::I16 | TipoC::U16 => 2,
            TipoC::I32 | TipoC::U32 | TipoC::F32 => 4,
            TipoC::I64 | TipoC::U64 | TipoC::F64 | TipoC::Ptr | TipoC::Handle => 8,
            TipoC::Void => 0,
        }
    }

    /// A letra do tipo na chave de uma assinatura nativa.
    pub fn letra(self) -> char {
        match self {
            TipoC::I8 => 'a',
            TipoC::U8 => 'h',
            TipoC::I16 => 's',
            TipoC::U16 => 't',
            TipoC::I32 => 'i',
            TipoC::U32 => 'j',
            TipoC::I64 => 'l',
            TipoC::U64 => 'm',
            TipoC::F32 => 'f',
            TipoC::F64 => 'd',
            TipoC::Bool => 'b',
            TipoC::Ptr => 'p',
            TipoC::Handle => 'H',
            TipoC::Void => 'v',
        }
    }
}

/// Terminador de controle de fluxo de um bloco básico.
#[derive(Debug, Clone)]
pub enum Terminator {
    Return(Option<Operand>),
    Branch(BlockId),
    CondBranch {
        cond: Operand,
        then_block: BlockId,
        else_block: BlockId,
    },
    Switch {
        val: Operand,
        default: BlockId,
        cases: Vec<(i64, BlockId)>,
    },
    Throw(Operand),
    Unreachable,
}

/// Bloco básico em SSA.
#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub id: BlockId,
    pub instructions: Vec<(ValueId, Instruction, Type)>,
    pub terminator: Terminator,
}

/// Função compilada na HIR.
#[derive(Debug, Clone)]
pub struct Function {
    pub symbol: String,
    pub name: String,
    pub params: Vec<(ValueId, String, Type)>,
    pub return_ty: Type,
    pub blocks: Vec<BasicBlock>,
    /// As posições na fonte, quando a compilação pede informação de
    /// depuração (`CompileOptions::depuracao`, J05).
    pub depuracao: Option<Box<DepuracaoDaFuncao>>,
}

/// A fonte de uma função e a posição de cada instrução (a do comando que a
/// gerou), para as tabelas de linha do depurador (`llvm/depuracao.rs`).
/// Instrução sem posição (criada por um passo de otimização) herda a da
/// anterior no bloco.
#[derive(Debug, Clone, Default)]
pub struct DepuracaoDaFuncao {
    /// O caminho absoluto do arquivo `.dart` (ou o URI, sem arquivo).
    pub arquivo: String,
    /// A url do script no rastro da VM (§13.14, `Context::url_do_rastro`).
    pub url: String,
    /// A linha do primeiro comando com posição (a do `DISubprogram`).
    pub linha: u32,
    /// `(linha, coluna)` de cada instrução, a partir de 1.
    pub posicoes: std::collections::HashMap<ValueId, (u32, u32)>,
    /// A do terminador de cada bloco (o `return` e o `break` não emitem
    /// instrução).
    pub saidas: std::collections::HashMap<BlockId, (u32, u32)>,
    // --- o rastro no formato da VM (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.14)
    /// A posição do token da função (o `Function.token_pos` da VM: o nome,
    /// ou o começo de uma closure): a do quadro de uma closure que escuta um
    /// `Future` (`kFutureListenerPcOffset`).
    pub token: (u32, u32),
    /// As funções copiadas nesta pelo inlining da HIR (`otimizar::inline`),
    /// para o rastro mostrar o quadro de cada uma como a VM mostra os quadros
    /// embutidos (`GetInlinedFunctionsAtReturnAddress`).
    pub embutidas: Vec<Embutida>,
    /// `(linha, coluna, contexto)` das instruções copiadas pelo inlining: a
    /// posição na função copiada e o índice dela em `embutidas`. Fica fora de
    /// `posicoes`, que é das tabelas de linha (o arquivo é o desta função).
    pub posicoes_embutidas: std::collections::HashMap<ValueId, (u32, u32, u32)>,
    /// As marcas do rastro ([`marcas_do_rastro`]).
    pub marcas: u8,
    /// Corpo `async` (a máquina de estados): a entrada das closures dele e a
    /// posição de cada `await`.
    pub corpo_async: Option<CorpoAsyncDoRastro>,
    /// A entrada uniforme (`$ent`), quando esta função é o corpo de uma
    /// closure: o código que a closure guarda, pelo qual o runtime acha a
    /// função de uma closure que escuta um `Future`.
    pub entrada_de_closure: Option<String>,
    /// A captura marcada `@pragma('vm:awaiter-link')` de uma closure: o elo
    /// com quem espera (`ClosureData::awaiter_link` da VM).
    pub elo: Option<EloDeEspera>,
}

impl Function {
    /// O nome da função no rastro no formato da VM (o
    /// `QualifiedUserVisibleName`): o qualificado do Dart, tirado do símbolo
    /// estável `df.<biblioteca>.<classe>.<membro>`, com as partes que o
    /// lowering acrescenta trocadas como a VM as escreve: `$clo<k>` vira
    /// `.<anonymous closure>`, `$<nome>` de função local vira `.<nome>`, e as
    /// impressões digitais e os sufixos de corpo (`$e…`, `$q…`, `$async`,
    /// `$ent`, `$novo`, a repetição `$<k>`) saem.
    pub fn nome_do_rastro(&self) -> String {
        let base = match self.symbol.strip_prefix("dart_main") {
            Some(resto) => format!("main{resto}"),
            None => self
                .symbol
                .strip_prefix("df.")
                .and_then(|resto| resto.split_once('.'))
                .map(|(_, nome)| nome.trim_start_matches('.').to_string())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| self.name.clone()),
        };
        let impressao = |s: &str, p: char| s.len() == 9 && s.starts_with(p) && s[1..].bytes().all(|b| b.is_ascii_hexdigit());
        let mut partes = base.split('$');
        let mut nome = partes.next().unwrap_or_default().to_string();
        for p in partes {
            if p.is_empty() || p == "async" || p == "ent" || p == "novo" || impressao(p, 'e') || impressao(p, 'q') || p.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            if let Some(k) = p.strip_prefix("clo")
                && !k.is_empty()
                && k.bytes().all(|b| b.is_ascii_digit())
            {
                nome.push_str(".<anonymous closure>");
                continue;
            }
            nome.push('.');
            nome.push_str(p);
        }
        nome
    }
}

/// Uma função copiada pelo inlining da HIR.
#[derive(Debug, Clone)]
pub struct Embutida {
    /// O nome da função copiada, como o rastro o escreve.
    pub nome: String,
    /// A url do script dela.
    pub url: String,
    /// A posição da chamada que a copiou, na função de fora (o contexto
    /// `pai`, ou a própria função).
    pub chamada: (u32, u32),
    /// O contexto de fora (índice em `embutidas`); `None`: a própria função.
    pub pai: Option<u32>,
    /// As marcas do rastro da função copiada.
    pub marcas: u8,
}

/// A parte do rastro de um corpo `async`.
#[derive(Debug, Clone, Default)]
pub struct CorpoAsyncDoRastro {
    /// A entrada uniforme do corpo (o código da closure do corpo, cujo
    /// ambiente guarda o quadro na posição 0).
    pub entrada: String,
    /// A posição do `await` de cada estado: o índice `k − 1` para o estado `k`.
    pub esperas: Vec<(u32, u32)>,
}

/// Onde mora a captura que é o elo com quem espera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EloDeEspera {
    /// A posição no ambiente (sem `direto`).
    pub indice: usize,
    /// O ambiente é o próprio valor capturado (`lower/closures.rs`).
    pub direto: bool,
    /// A captura mora numa célula.
    pub celula: bool,
}

/// As marcas do rastro de uma função ([`DepuracaoDaFuncao::marcas`]).
pub mod marcas_do_rastro {
    /// O corpo de uma função `async` (a máquina de estados): o quadro dele,
    /// retomado, começa a cadeia de quem espera.
    pub const CORPO_ASYNC: u8 = 1;
    /// Fora do rastro: o stub de uma função `async` (a VM tem um quadro só
    /// para ela) e os apoios do `async_patch` (`@pragma("dartforge:rastro-oculto")`).
    pub const OCULTA: u8 = 2;
    /// `_FutureListener.handleValue`: o ouvinte dela começa a cadeia
    /// (`MethodRecognizer::kFutureListenerHandleValue`).
    pub const ESCUTA: u8 = 4;
    /// Com [`CORPO_ASYNC`], o corpo de um gerador `async*`: o quadro dele na
    /// pilha conta sempre como já suspenso (`WasPreviouslySuspended`), e o
    /// próximo da cadeia é o controlador.
    pub const GERADOR_ASYNC: u8 = 8;
}

/// Definição de classe na HIR.
#[derive(Debug, Clone)]
pub struct ClassDef {
    pub id: u32,
    pub name: String,
    pub field_count: usize,
    /// Mapa de selector_id -> símbolo da função implementada
    pub vtable: Vec<(u32, String)>,
    pub to_string_symbol: Option<String>,
}

/// Seletor de chamada registrado globalmente.
#[derive(Debug, Clone)]
pub struct SelectorDef {
    pub id: u32,
    pub name: String,
    pub arity: usize,
}

/// Uma struct/union do programa para o runtime: a classe (RTI e heap), onde
/// ficam `_typedDataBase`/`_offsetInBytes` no objeto, e a medida na ABI.
#[derive(Debug, Clone)]
pub struct FfiComposto {
    pub rti: i64,
    pub classe: i64,
    pub campos: i64,
    pub indice_base: i64,
    pub indice_deslocamento: i64,
    pub tamanho: i64,
    pub alinhamento: i64,
}

/// Um callback nativo de uma assinatura: a chave, o corpo HIR (contexto e
/// argumentos na representação Dart → retorno Dart; um composto vai e volta
/// como o endereço dos bytes) e os tipos nativos.
#[derive(Debug, Clone)]
pub struct FfiCallback {
    pub chave: String,
    pub corpo: String,
    pub ret: TipoNativo,
    pub params: Vec<TipoNativo>,
}

/// Um campo do layout de uma classe do programa (J03).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CampoDoLayout {
    pub nome: String,
    /// `null` é um valor do tipo declarado (`T?`, `dynamic`, `Object?`…).
    pub anulavel: bool,
    pub late: bool,
    /// O tipo declarado, como texto (uma mudança dele não migra).
    pub tipo: String,
}

/// Como um `Return` sai quando a exceção pode estar pendente, nas exceções por
/// tabelas (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13): uma função Dart
/// nunca volta a quem a chamou com a exceção pendente — ela desenrola.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaidaPorExcecao {
    /// A exceção está pendente em todo caminho até este `Return`: desenrola
    /// (`@df.lancar`) em vez de retornar.
    Lanca,
    /// Retoma o objeto nativo recebido neste pouso de cleanup Itanium.
    /// Só admite drops ARC e fechamento LIFO de quadros locais antes do Return.
    /// Não coleta, chama Dart nem publica uma nova exceção.
    Retoma,
    /// Pode estar pendente: confere a pendência e desenrola ou retorna.
    Guarda,
}

/// O que o passe das exceções por tabelas (`otimizar/tabelas.rs`) decidiu
/// para uma função: quem é `invoke`, onde pousa e como cada `Return` sai. O
/// emissor (`llvm/mod.rs`) só lê.
#[derive(Debug, Clone, Default)]
pub struct TabelasDaFuncao {
    /// A instrução (chamada Dart) que vira `invoke`, e o bloco de pouso dela.
    pub invocacoes: std::collections::HashMap<ValueId, BlockId>,
    /// Os blocos de pouso: começam com o `landingpad` e a restauração do
    /// topo da pilha-sombra; só o `invoke` chega a eles.
    pub pousos: std::collections::HashSet<BlockId>,
    /// Os blocos que terminam em `Return` com a exceção (talvez) pendente.
    pub saidas: std::collections::HashMap<BlockId, SaidaPorExcecao>,
    /// A função chama código Dart: confere a pilha no prólogo
    /// (`StackOverflowError`). No modelo de conferência a leitura da
    /// pendência depois de cada chamada já dava à função o contexto em que a
    /// conferência da pilha mora; aqui a leitura some, e sem esta marca
    /// `f() => f()` estouraria a pilha do sistema.
    pub confere_pilha: bool,
    /// Inventário certificado de owners para cleanup estrangeiro; recalcular após mutações.
    pub cleanup_estrangeiro: Option<crate::otimizar::arc::CleanupEstrangeiro>,
}

/// Módulo HIR completo representando um programa Dart compilável.
#[derive(Debug, Default)]
pub struct Module {
    /// J05: as posições das funções viram tabelas de linha (`--depuracao`).
    /// Sem isto, as posições que houver são só do rastro simbólico (§13.14).
    pub dwarf: bool,
    /// As exceções do módulo são por tabelas (`--excecoes=tabelas`): o passe
    /// `otimizar::tabelas` rodou e [`Module::tabelas`] vale. Falso: o modelo
    /// de sempre (pendência conferida depois de cada chamada).
    pub excecoes_por_tabelas: bool,
    /// A memória do módulo é ARC (`--memoria=arc`, `alvo::memoria_arc`): a
    /// entrada liga o ARC no runtime e as gravações de referência em objeto
    /// que pode ser velho passam pelo runtime (docs/ARC-IMPLEMENTACAO.md).
    pub memoria_arc: bool,
    /// Com [`Module::excecoes_por_tabelas`], a decisão do passe para cada
    /// função, na ordem de [`Module::functions`].
    pub tabelas: Vec<TabelasDaFuncao>,
    /// `dart:ffi`: (id RTI da classe, letra do tipo C) de cada tipo nativo
    /// e (chave da assinatura, símbolo do trampolim) (`lower/ffi.rs`).
    pub ffi_tipos: Vec<(i64, char)>,
    pub ffi_trampolins: Vec<(String, String)>,
    /// `dart:ffi`: as entradas C dos callbacks nativos, uma por assinatura
    /// (`lower/ffi.rs`; o emissor gera a entrada com a ABI C).
    pub ffi_callbacks: Vec<FfiCallback>,
    /// `dart:ffi`: as structs e unions do programa (`lower/ffi.rs`).
    pub ffi_compostos: Vec<FfiComposto>,
    pub functions: Vec<Function>,
    /// Parâmetros escalares declarados na fonte, por símbolo/ID SSA.
    /// O lowering registra somente int/double/bool sem caixa. Parâmetros
    /// ocultos/nativos i64 não são classificados pela largura. Estes fatos
    /// ainda precisam alimentar os planos ARC e sobreviver aos passes.
    pub parametros_escalares_dart: std::collections::HashMap<String, std::collections::HashSet<ValueId>>,
    /// IDs nativos do universo canônico de RTI, produzidos explicitamente.
    /// Não são Ref nem escalares Dart; vida do universo/pins exige outro protocolo.
    pub parametros_rti_dart: std::collections::HashMap<String, std::collections::HashSet<ValueId>>,
    /// Representações declaradas dos campos por classe/posição, somente no ARC.
    /// Inclui herança, mixins e prefixo de enum; late com inicializador fica Ref.
    /// Não certifica o tipo do receiver, forma física do heap ou versão/pins de recarga.
    pub layouts_campos_arc: std::collections::HashMap<u32, Vec<Type>>,
    pub classes: Vec<ClassDef>,
    pub selectors: Vec<SelectorDef>,
    pub subtyping_edges: Vec<(u32, u32)>,
    pub entry_symbol: Option<String>,
    /// Quantos parâmetros o `main` declara: com um, recebe os argumentos da
    /// linha de comando (`List<String>`); o segundo (`message`, do
    /// `Isolate.spawnUri`) é `null`.
    pub entry_params: usize,
    /// Globais do usuário: (id da variável, representação). Cada um vira
    /// Globais do usuário: (id da raiz no runtime, representação, símbolo
    /// estável do valor `dfg.<caminho>`); a bandeira de inicialização é
    /// `<símbolo>$ok`.
    pub globais: Vec<(u32, Type, String)>,
    /// Construtos que o lowering não sabe baixar (N1). Não vazio = o
    /// programa não compila; o emissor produz só a mensagem.
    pub erros: Vec<String>,
    // --- P5c (SDK da fonte, δ) ---
    /// O módulo é parte de um programa com o SDK compilado da fonte: o
    /// código compartilhado entre módulos (entradas de tear-off, constantes
    /// canônicas) sai em `comdat`, e símbolos de outros módulos são
    /// declarados.
    pub modo_sdk: bool,
    /// O módulo é uma biblioteca do SDK (sem `dartforge_entry` nem o
    /// despacho de `toString`, que são do programa).
    pub biblioteca_sdk: bool,
    /// Tabelas de métodos das classes deste módulo, registradas no runtime
    /// na partida (P5c): (id da classe, [(seletor, símbolo do adaptador)]).
    pub tabelas_de_metodos: Vec<(u32, String, Vec<(String, String)>)>,
    /// A função que devolve a tabela de métodos de cada classe concreta
    /// compilada (de qualquer módulo), pelo id: a alocação de um objeto da
    /// classe registra a tabela (`dartforge_object_new_t`).
    pub funcoes_de_tabela: std::collections::HashMap<u32, String>,
    /// Biblioteca do SDK: o nome da função que registra as classes dela
    /// (nomes, subtipos, tabelas de métodos) no runtime.
    pub registro: Option<String>,
    /// Programa com o SDK da fonte: as funções de registro das bibliotecas
    /// do SDK, chamadas por `dartforge_entry` antes das do programa.
    pub registros_do_sdk: Vec<String>,
    /// Os campos do `dart:async` que o rastro percorre para achar quem espera
    /// (§13.14): `(id da classe, "Classe.campo", posição)`, registrados na
    /// partida com o rastro simbólico.
    pub campos_do_rastro: Vec<(u32, String, usize)>,
    /// As entradas de tear-off que o rastro reconhece nos ramos de stream
    /// (§13.14): `(símbolo da entrada, espécie)`, a espécie 1 para o
    /// `_StreamIterator._onData` e 2 para o `_StreamController._add`.
    pub tearoffs_do_rastro: Vec<(String, i64)>,
    /// Os ids das classes do programa, `(id, biblioteca, classe)`: escritos
    /// no IR (`; df.classe …`) para que a geração seguinte de uma recarga do
    /// JIT dê o mesmo id à mesma classe (J03, `Context::com_ids_anteriores`).
    pub ids_do_programa: Vec<(u32, String, String)>,
    /// O layout dos objetos de cada classe do programa, para a migração das
    /// instâncias vivas numa recarga do JIT (J03): `(id, campos antes dos
    /// declarados — os de enum —, campos)`, escrito no IR como
    /// `; df.campos …`.
    pub campos_do_programa: Vec<(u32, usize, Vec<CampoDoLayout>)>,
    /// Programa com o SDK da fonte: os ids de classe dos valores que o
    /// runtime representa (`Null`, `_Smi`, `_Mint`, `_Double`, `bool`,
    /// `_OneByteString`, `_TwoByteString`, `_GrowableList`, `_List`,
    /// `_ImmutableList`, `_Closure`, `_Record`), na ordem de
    /// `runtime/src/seletores.rs`.
    pub cids_do_runtime: Vec<i64>,
    /// Programa com o SDK da fonte: a versão do SDK Dart compilado (o
    /// arquivo `version` dele), que `Platform.version` informa.
    pub versao_do_sdk: Option<String>,
    /// Membros do SDK da fonte recusados neste módulo: (símbolo, motivo).
    /// Cada um virou uma função que avisa em tempo de execução.
    pub recusados: Vec<(String, String)>,
    /// Funções Dart que o runtime chama pelo nome (`_dartforge*` da
    /// sobreposição): (nome, símbolo), registradas pelo registro do módulo.
    pub ajudantes: Vec<(String, String)>,
    // --- P6 (bibliotecas da fonte, `fonte.rs`) ---------------------------
    /// Diagnósticos das funções de bibliotecas do SDK compiladas da fonte,
    /// pelo símbolo da função que os produziu: só viram `erros` se a poda
    /// (`fonte::podar`) mantiver a função.
    pub erros_da_fonte: Vec<(String, Vec<String>)>,
    /// Funções da fonte que o runtime chama sem que o programa as referencie
    /// (raízes da poda, além das funções do programa).
    pub raizes_da_fonte: Vec<String>,
    /// A função que o laço de eventos do runtime usa para chamar uma
    /// closure sem argumentos (`dartforge_laco_de_eventos`, depois do
    /// `main`). `None`: o programa não usa `dart:async` e não há laço.
    pub chamar_dart: Option<String>,
    /// RTI: a função que registra o universo de tipos antes do `main`
    /// (`lower::rti::registrar_universo`); `None` sem receitas.
    pub iniciar_rti: Option<String>,
}

impl Module {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra ou localiza um selector_id global para um nome e aridade.
    pub fn get_or_register_selector(&mut self, name: &str, arity: usize) -> u32 {
        if let Some(pos) = self.selectors.iter().position(|s| s.name == name && s.arity == arity) {
            return pos as u32;
        }
        let id = self.selectors.len() as u32;
        self.selectors.push(SelectorDef {
            id,
            name: name.to_string(),
            arity,
        });
        id
    }
}
