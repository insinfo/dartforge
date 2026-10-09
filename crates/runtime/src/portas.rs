// Runtime nativo: portas (`RawReceivePort`/`SendPort`) e a fila de mensagens
// do isolado — o `PortMap`/`MessageHandler` da VM (`runtime/vm/port.cc`,
// `message_handler.cc`).
//
// * Uma porta é um id global (nunca 0). O registro global diz a quem ela
//   pertence: a fila de um isolado (uma thread) ou um serviço do runtime
//   (as portas nativas, como a do IOService), que recebe a mensagem na
//   thread de quem enviou.
// * Enviar copia a mensagem para um grafo portátil (`Grafo`), que o destino
//   materializa no próprio heap. É a semântica da VM
//   (`object_graph_copy.cc`): o que é mutável é copiado; o que é
//   profundamente imutável e permanente passa pela identidade — os objetos
//   estáticos da imagem (as caixas de `bool`, os literais de string do AOT)
//   em qualquer isolado do processo; constantes, enums, tear-offs de topo e
//   strings do heap quando a mensagem fica no mesmo isolado.
// * O grafo copia blocos do espaço de objetos pelo formato do corpo
//   (docs/NATIVO-ESPACO-UNIFICADO.md §2.12): `INSTANCIA` campo a campo com o
//   bit de referência; `REFS` pela palavra 0 e as referências; `BRUTO` cru,
//   menos o que aponta para dentro de si ou para fora do heap (lista tipada,
//   visão, anexo), que o destino refaz.
// * A fila do isolado guarda (porta, grafo, chegada). O laço de eventos
//   (`eventos.rs`) atende timers e mensagens pela ordem de chegada — na VM o
//   timer também é uma mensagem —, e esvazia as microtarefas depois de
//   cada uma.
// * O despacho chama o `_RawReceivePort._despachar` da sobreposição (o
//   `_handleMessage` da VM), que lê a porta e a mensagem correntes.
// * O isolado vive enquanto tiver porta aberta com `keepIsolateAlive`.

/// Uma palavra dentro de um grafo portátil (uma referência ou os bits de um
/// campo escalar, no destino).
#[derive(Clone, Copy, Debug)]
enum ValG {
    /// Uma palavra que não aponta para o heap: `null`, um `Smi` numa posição
    /// `Ref`, ou os bits de um campo escalar.
    Palavra(i64),
    /// O nó `i` do grafo.
    No(usize),
    /// Um objeto estático da imagem (`PERMANENTE`): o mesmo endereço em todo
    /// isolado do processo, nunca coletado nem gravado.
    Estatico(i64),
    /// Um objeto permanente do heap de origem, passado pela identidade (só
    /// quando origem e destino são o mesmo isolado).
    Mesmo(i64),
    /// Uma constante canônica, pelo getter gerado que a produz: o destino
    /// recebe a instância canônica dele (o grupo de isolados da VM
    /// compartilha as constantes; aqui cada isolado tem a sua, e `identical`
    /// continua valendo entre eles).
    Constante(usize),
    /// O tear-off canônico de uma função de topo, pelo código, com o corpo
    /// tipado, a ABI e o metadado RTI (o tipo da função) da closure de
    /// origem: o destino que ainda não tem o tear-off o cria igual — antes
    /// nascia sem tipo (`runtimeType` `Function`) e o `as Stream
    /// Function(Stream, dynamic)` do `StreamIsolate` do new_sali/backend
    /// falhava no isolado novo (docs/NATIVO-PROJETOS-REAIS.md, C11). Os ids
    /// de tipo são do processo (`tipos.rs`).
    TearOff { codigo: i64, tipado: i64, abi: i64, metadado: u32 },
}

/// Um nó do grafo: um bloco do espaço de objetos copiado pelo formato, com as
/// referências trocadas por `ValG`.
#[derive(Clone, Debug)]
enum NoG {
    /// Um objeto `INSTANCIA` (do programa, ou do runtime: `_GrowableList`,
    /// `_Closure`, `_Contexto`, `_Celula`, `_SendPort`…): a classe e os campos,
    /// com o bit de referência de cada um.
    Instancia { cid: i32, campos: Vec<(ValG, bool)> },
    /// Um corpo `REFS` (`_List`/`_ImmutableList` geral, `_Record`): a palavra 0
    /// (o comprimento ou a forma) e as referências.
    Refs { cid: i32, palavra0: i64, refs: Vec<ValG> },
    /// Uma `_List`/`_ImmutableList` compacta (`BRUTO` com `ELEMENTO`): os
    /// elementos crus (`i64`, bits de `f64` ou 0/1).
    Compacta { cid: i32, forma: crate::listas::Elemento, elementos: Vec<i64> },
    /// Um corpo `BRUTO` sem ponteiro para dentro de si nem para fora do heap
    /// (`_Mint`, `_Double`, SIMD): as palavras e os `flags`, copiados crus.
    Bruto { cid: i32, flags: u8, palavras: Box<[i64]> },
    /// Uma string (a forma canônica é refeita no destino).
    Texto(Texto),
    /// O acumulador de um `StringBuffer` (`_AcumuladorDeTexto`, `ANEXO`): as
    /// unidades, que moram fora do heap.
    Acumulador(Vec<u16>),
    /// O programa de um `RegExp` (`_ProgramaDeRegExp`, `ANEXO`): o padrão e as
    /// opções, recompilado no destino (`regexp.rs`).
    ProgramaRe { fonte: Vec<u16>, opcoes: [bool; 4] },
    /// Uma lista tipada interna ou externa (`EXTERNO`): os bytes, copiados
    /// numa interna no destino (a VM copia a externa para memória nova,
    /// `object_graph_copy.cc`).
    Tipada { tipo: u8, bytes: Vec<u8> },
    /// Uma visão: a base (copiada) e o deslocamento; o endereço dos dados é
    /// recalculado no destino.
    Visao { cid: i32, base: ValG, deslocamento: usize, len: usize },
    /// Um objeto do runtime de um campo inteiro vindo de fora do heap
    /// (`SendPort`, `Capability` do C): a classe é a da posição `pos` da
    /// cid fixo (`layout::cid`, `_SendPort`/`_Capability`).
    DoRuntime { cid: i32, id: i64 },
}

/// Uma mensagem copiada: os nós (com o metadado RTI de cada um) e a raiz.
#[derive(Clone)]
pub struct Grafo {
    nos: Vec<(NoG, i64)>,
    raiz: ValG,
    /// O isolado de origem (id da fila): `ValG::Mesmo` só vale nele.
    origem: u64,
    /// Numa mensagem para outro isolado, os tipos (RTI) dos nós: o
    /// metadado de cada nó (`1 + tipo`) é então `1 + índice` nesta tabela
    /// (0 = sem tipo), porque os ids de tipo são por isolado.
    tipos: Option<TiposDaMensagem>,
    /// Numa mensagem para outro isolado, a tabela de métodos de cada classe
    /// dos objetos (registrada no isolado na criação do primeiro objeto da
    /// classe; um objeto que chega numa mensagem não passa por lá).
    tabelas: Vec<(i64, TabelaDeMetodos)>,
}

/// Por que uma mensagem não pode ser enviada.
pub struct MensagemIlegal(pub String);

/// A palavra `h` (numa posição `Ref`) no grafo: imediata, estática, pela
/// identidade, canônica, ou o nó do objeto (reservado agora e preenchido
/// quando `h` sai da pilha).
fn ref_de_handle(heap: &Heap, h: i64, mapa: &mut crate::hash::HashMap<i64, usize>, pilha: &mut Vec<i64>, nos: &mut Vec<(NoG, i64)>, compartilhar: bool) -> ValG {
    if !crate::layout::e_objeto(h) {
        return ValG::Palavra(h);
    }
    if heap.e_estatico(h) {
        return ValG::Estatico(h);
    }
    if compartilhar {
        if heap.e_permanente(h) || heap.e_texto(h) {
            return ValG::Mesmo(h);
        }
    } else if let Some(g) = heap.getter_da_constante(h) {
        return ValG::Constante(g);
    } else if let Some(c) = heap.codigo_do_tearoff(h) {
        let (tipado, abi) = heap.closure(h).map_or((0, 0), |f| (f.tipado, f.abi));
        return ValG::TearOff { codigo: c, tipado, abi, metadado: heap.cabecalho(h).metadado };
    }
    let c = *heap.cabecalho(h);
    // Um anexo nativo que o grafo não sabe copiar (só o acumulador do
    // `StringBuffer` e o programa do `RegExp` têm cópia) chega como `null`.
    if c.flags & crate::layout::flags::ANEXO != 0
        && c.class_id != crate::layout::cid::ACUMULADOR_DE_TEXTO
        && c.class_id != crate::layout::cid::PROGRAMA_DE_REGEXP
    {
        return ValG::Palavra(0);
    }
    if let Some(&i) = mapa.get(&h) {
        return ValG::No(i);
    }
    let i = nos.len();
    mapa.insert(h, i);
    // Reserva o nó; o conteúdo é preenchido quando `h` sai da pilha.
    nos.push((NoG::Acumulador(Vec::new()), 0));
    pilha.push(h);
    ValG::No(i)
}

/// Classes cujo primeiro campo é MOVIDO numa mensagem (o
/// `TransferableTypedData`): o objeto de origem fica com `null`. Os ids de
/// classe são do programa, então o registro vale para todos os isolados.
fn classes_transferiveis() -> std::sync::RwLockReadGuard<'static, crate::hash::HashSet<i64>> {
    transferiveis().read().unwrap_or_else(|e| e.into_inner())
}

fn transferiveis() -> &'static std::sync::RwLock<crate::hash::HashSet<i64>> {
    static T: std::sync::OnceLock<std::sync::RwLock<crate::hash::HashSet<i64>>> = std::sync::OnceLock::new();
    T.get_or_init(Default::default)
}

thread_local! {
    /// Classes cujas instâncias não podem ir numa mensagem (as portas de
    /// recepção; `DartForge_porta_abrir` registra a classe).
    /// Classe → a descrição que a recusa leva (o texto da VM).
    static NAO_ENVIAVEIS: RefCell<crate::hash::HashMap<i64, String>> = RefCell::new(crate::hash::HashMap::default());
    /// A descrição da última mensagem recusada.
    static ULTIMA_RECUSA: RefCell<String> = const { RefCell::new(String::new()) };
}

/// As unidades do acumulador de texto `h` (`_AcumuladorDeTexto`): o anexo em
/// `b+16` é um `Box<Vec<u16>>` (docs/NATIVO-ESPACO-UNIFICADO.md §2.5).
fn unidades_do_acumulador(heap: &Heap, h: i64) -> Vec<u16> {
    let p = heap.anexo(h) as *const Vec<u16>;
    if p.is_null() {
        return Vec::new();
    }
    // SAFETY: o anexo de um `_AcumuladorDeTexto` vivo é o `Vec<u16>` dele,
    // solto só quando o bloco morre; o empréstimo do heap o mantém.
    #[allow(unsafe_code)]
    unsafe {
        (*p).clone()
    }
}

/// Os elementos de uma `_List`/`_ImmutableList` como nó do grafo, pela forma.
fn no_da_lista_fixa(heap: &Heap, h: i64, classe: i32, v: &mut dyn FnMut(i64) -> ValG) -> NoG {
    use crate::listas::{Elemento, ElementosRef};
    match heap.lista_elementos(h) {
        ElementosRef::Geral(e) => NoG::Refs { cid: classe, palavra0: e.len() as i64, refs: e.iter().map(|&x| v(x)).collect() },
        ElementosRef::Int(e) => NoG::Compacta { cid: classe, forma: Elemento::Int, elementos: e.to_vec() },
        ElementosRef::Double(e) => NoG::Compacta { cid: classe, forma: Elemento::Double, elementos: e.iter().map(|d| d.to_bits() as i64).collect() },
        ElementosRef::Bool(e) => NoG::Compacta { cid: classe, forma: Elemento::Bool, elementos: e.to_vec() },
    }
}

/// O nó do bloco `h` (já conferido: nem imediato, nem estático, nem anexo de
/// programa). `v` traduz cada referência encontrada.
fn no_do_bloco(heap: &Heap, h: i64, v: &mut dyn FnMut(i64) -> ValG, transferidos: &mut Vec<i64>) -> Result<NoG, MensagemIlegal> {
    use crate::layout::{cid, flags};
    let c = *heap.cabecalho(h);
    let classe = c.class_id;
    let forma = c.flags & flags::FORMA;
    if forma == flags::INSTANCIA {
        if cid::e_tipada(classe) {
            // Visão (`INSTANCIA`, cids 36–65): a base e o deslocamento.
            let t = heap.tipada(h).expect("visão viva");
            return Ok(NoG::Visao { cid: classe, base: v(t.base.unwrap_or(0)), deslocamento: t.deslocamento, len: t.len });
        }
        let objeto = heap.objeto(h).expect("objeto vivo");
        let class_id = objeto.class_id;
        let campos = objeto.to_vec();
        if let Some(d) = NAO_ENVIAVEIS.with(|n| n.borrow().get(&class_id).cloned()) {
            return Err(MensagemIlegal(d));
        }
        if classes_transferiveis().contains(&class_id) {
            if campos.first().is_none_or(|f| f.0 == 0) {
                return Err(MensagemIlegal("(TransferableTypedData has been transferred already)\n".to_string()));
            }
            transferidos.push(h);
        }
        let campos = campos.iter().map(|&(bits, e_ref)| (if e_ref { v(bits) } else { ValG::Palavra(bits) }, e_ref)).collect();
        return Ok(NoG::Instancia { cid: classe, campos });
    }
    if forma == flags::REFS {
        if cid::e_lista_fixa(classe) {
            return Ok(no_da_lista_fixa(heap, h, classe, v));
        }
        if classe == cid::RECORD {
            let campos = heap.record(h).expect("record vivo");
            return Ok(NoG::Refs { cid: classe, palavra0: campos.len() as i64, refs: campos.iter().map(|&x| v(x)).collect() });
        }
        // Outro `REFS`: a palavra 0 é o número de referências.
        let p = heap.palavras(h);
        let n = usize::try_from(p[0]).unwrap_or(0).min(p.len().saturating_sub(1));
        return Ok(NoG::Refs { cid: classe, palavra0: p[0], refs: p[1..=n].iter().map(|&x| v(x)).collect() });
    }
    // `BRUTO`.
    if c.flags & flags::ANEXO != 0 {
        if classe == cid::PROGRAMA_DE_REGEXP {
            let (fonte, opcoes) = padrao_do_programa_re(heap, h);
            return Ok(NoG::ProgramaRe { fonte, opcoes });
        }
        return Ok(NoG::Acumulador(unidades_do_acumulador(heap, h)));
    }
    if cid::e_texto(classe) {
        return Ok(NoG::Texto(heap.texto(h).expect("string viva").para_texto()));
    }
    if cid::e_tipada(classe) {
        let t = heap.tipada(h).expect("lista tipada viva");
        let bytes = heap.bytes_da_tipada(h).map(<[u8]>::to_vec).unwrap_or_default();
        return Ok(NoG::Tipada { tipo: t.tipo, bytes });
    }
    if cid::e_lista_fixa(classe) {
        // Compacta: `lista_elementos` diz a forma.
        return Ok(no_da_lista_fixa(heap, h, classe, v));
    }
    Ok(NoG::Bruto { cid: classe, flags: c.flags, palavras: heap.palavras(h).into() })
}

/// Copia o valor `raiz` (um valor numa posição `Ref`) para um grafo.
/// `compartilhar`: o destino é este mesmo isolado.
fn copiar_para_grafo(raiz: i64, compartilhar: bool) -> Result<Grafo, MensagemIlegal> {
    let mut mapa = crate::hash::HashMap::default();
    let mut pilha = Vec::new();
    let mut nos: Vec<(NoG, i64)> = Vec::new();
    let mut tipos = (!compartilhar).then(TiposDaMensagem::default);
    let mut transferidos: Vec<i64> = Vec::new();
    let r = HEAP.with(|heap| ref_de_handle(&heap.borrow(), raiz, &mut mapa, &mut pilha, &mut nos, compartilhar));
    while let Some(h) = pilha.pop() {
        let i = mapa[&h];
        let r = HEAP.with(|heap| -> Result<(NoG, i64), MensagemIlegal> {
            let heap = heap.borrow();
            let meta = i64::from(heap.cabecalho(h).metadado);
            let mut v = |x: i64| ref_de_handle(&heap, x, &mut mapa, &mut pilha, &mut nos, compartilhar);
            let no = no_do_bloco(&heap, h, &mut v, &mut transferidos)?;
            Ok((no, meta))
        });
        let (mut no, meta) = r?;
        let meta = match tipos.as_mut() {
            // O metadado é `1 + tipo` (0 = sem tipo, `tipos.rs`).
            Some(t) if meta != 0 => 1 + t.exportar(meta - 1),
            _ => meta,
        };
        // Um objeto `Type` leva o tipo pela tabela da mensagem.
        if let (Some(t), NoG::Instancia { cid, campos }) = (tipos.as_mut(), &mut no)
            && i64::from(*cid) == CLASSE_TIPO
            && let Some((ValG::Palavra(id), _)) = campos.first().copied()
        {
            campos[0] = (ValG::Palavra(t.exportar(id)), false);
        }
        nos[i] = (no, meta);
    }
    // Os `TransferableTypedData` enviados ficam vazios na origem.
    if !transferidos.is_empty() {
        HEAP.with(|heap| {
            let mut heap = heap.borrow_mut();
            for h in transferidos {
                if heap.objeto(h).is_some_and(|o| !o.is_empty()) {
                    heap.definir_campo(h, 0, 0, true);
                }
            }
        });
    }
    let tabelas = if compartilhar {
        Vec::new()
    } else {
        let classes: std::collections::BTreeSet<i64> = nos
            .iter()
            .filter_map(|(no, _)| match no {
                NoG::Instancia { cid, .. } => Some(i64::from(*cid)),
                _ => None,
            })
            .collect();
        METODOS.with(|m| {
            let m = m.borrow();
            classes.into_iter().filter_map(|c| m.get(&c).map(|t| (c, t.clone()))).collect()
        })
    };
    Ok(Grafo { nos, raiz: r, origem: id_do_isolado(), tipos, tabelas })
}

impl Grafo {
    /// Chama `f` para a raiz e para cada aresta do grafo.
    fn visitar_valores(&self, mut f: impl FnMut(&ValG)) {
        f(&self.raiz);
        for (no, _) in &self.nos {
            match no {
                NoG::Instancia { campos, .. } => campos.iter().for_each(|(v, _)| f(v)),
                NoG::Refs { refs, .. } => refs.iter().for_each(&mut f),
                NoG::Visao { base, .. } => f(base),
                NoG::Compacta { .. }
                | NoG::Bruto { .. }
                | NoG::Texto(_)
                | NoG::Acumulador(_)
                | NoG::ProgramaRe { .. }
                | NoG::Tipada { .. }
                | NoG::DoRuntime { .. } => {}
            }
        }
    }

    /// Um grafo de um valor só, sem nó (as respostas do runtime): `null` ou
    /// um `Smi`.
    fn imediato(r: i64) -> Grafo {
        Grafo { nos: Vec::new(), raiz: ValG::Palavra(r), origem: 0, tipos: None, tabelas: Vec::new() }
    }

    /// O grafo de `null`.
    pub fn nulo() -> Grafo {
        Grafo::imediato(0)
    }

    /// O grafo do `int` `v`.
    pub fn de_int(v: i64) -> Grafo {
        Portavel::Int(v).para_grafo()
    }
}

/// Um valor que o runtime lê ou monta fora de um heap (as mensagens dos
/// serviços nativos — o `Dart_CObject` da VM): `null`, `bool`, `int`,
/// `double`, `String`, lista, `Uint8List` e, só na leitura, os campos de um
/// objeto (a `SendPort` de resposta).
#[derive(Clone, Debug)]
pub enum Portavel {
    Nulo,
    Bool(bool),
    Int(i64),
    Double(f64),
    Str(String),
    Lista(Vec<Portavel>),
    /// Os bytes de uma lista tipada (de qualquer tipo de elemento, como o
    /// `CObjectTypedData` da VM).
    Bytes(Vec<u8>),
    /// Uma lista tipada de elementos `tipo` (os `TIPO_*` do runtime).
    Tipada(u8, Vec<u8>),
    /// Um objeto do runtime de um campo inteiro (ver [`NoG::DoRuntime`]).
    DoRuntime { cid: i32, id: i64 },
    Objeto(Vec<Portavel>),
}

impl Portavel {
    pub fn int(&self) -> Option<i64> {
        match self {
            Portavel::Int(x) => Some(*x),
            _ => None,
        }
    }
    pub fn bool(&self) -> Option<bool> {
        match self {
            Portavel::Bool(b) => Some(*b),
            _ => None,
        }
    }
    pub fn str(&self) -> Option<&str> {
        match self {
            Portavel::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn bytes(&self) -> Option<&[u8]> {
        match self {
            Portavel::Bytes(b) => Some(b),
            _ => None,
        }
    }
    pub fn lista(&self) -> Option<&[Portavel]> {
        match self {
            Portavel::Lista(l) => Some(l),
            _ => None,
        }
    }
}

/// Um objeto estático da imagem (`ValG::Estatico`) lido sem heap: as caixas
/// de `bool` e os literais de string (o bloco é constante e vive o processo
/// inteiro; `layout` dá os deslocamentos).
fn portavel_de_estatico(h: i64) -> Portavel {
    use crate::layout::{DESLOCAMENTO_DO_HANDLE, cid, desl};
    let b = (h - DESLOCAMENTO_DO_HANDLE) as *const u8;
    // SAFETY: `h` é o handle de um objeto estático (conferido na cópia), um
    // bloco constante da seção da imagem com o cabeçalho e o corpo inteiros.
    #[allow(unsafe_code)]
    unsafe {
        let c = *(b as *const crate::layout::Cabecalho);
        let palavra = |d: usize| *(b.add(d) as *const i64);
        match c.class_id {
            cid::BOOL => Portavel::Bool(palavra(desl::VALOR) != 0),
            cid::ONE_BYTE_STRING => {
                let n = palavra(desl::COMPRIMENTO) as usize;
                let u = std::slice::from_raw_parts(b.add(desl::UNIDADES), n);
                Portavel::Str(u.iter().map(|&x| char::from(x)).collect())
            }
            cid::TWO_BYTE_STRING => {
                let n = palavra(desl::COMPRIMENTO) as usize;
                let u = std::slice::from_raw_parts(b.add(desl::UNIDADES) as *const u16, n);
                Portavel::Str(String::from_utf16_lossy(u))
            }
            _ => Portavel::Nulo,
        }
    }
}

impl Grafo {
    /// O grafo lido como [`Portavel`] (numa thread sem heap Dart). Um ciclo,
    /// que as mensagens dos serviços nunca têm, vira `Nulo` na volta.
    pub fn para_portavel(&self) -> Portavel {
        /// A palavra `v`; `e_ref` diz se ela está numa posição `Ref` (senão,
        /// os bits de um campo escalar, lidos como `int`).
        fn val(g: &Grafo, v: &ValG, e_ref: bool, visitando: &mut Vec<bool>) -> Portavel {
            match *v {
                ValG::Palavra(bits) if !e_ref => Portavel::Int(bits),
                ValG::Palavra(0) => Portavel::Nulo,
                ValG::Palavra(bits) if smi::e_smi(bits) => Portavel::Int(smi::valor(bits)),
                ValG::Palavra(_) => Portavel::Nulo,
                ValG::Estatico(h) => portavel_de_estatico(h),
                ValG::Mesmo(_) | ValG::Constante(_) | ValG::TearOff { .. } => Portavel::Nulo,
                ValG::No(i) => {
                    if visitando[i] {
                        return Portavel::Nulo;
                    }
                    visitando[i] = true;
                    let r = no(g, &g.nos[i].0, visitando);
                    visitando[i] = false;
                    r
                }
            }
        }
        fn no(g: &Grafo, n: &NoG, visitando: &mut Vec<bool>) -> Portavel {
            use crate::layout::cid;
            use crate::listas::Elemento;
            match n {
                NoG::Texto(t) => Portavel::Str(t.para_string()),
                NoG::Bruto { cid: cid::MINT, palavras, .. } => Portavel::Int(palavras.first().copied().unwrap_or(0)),
                NoG::Bruto { cid: cid::DOUBLE, palavras, .. } => Portavel::Double(f64::from_bits(palavras.first().copied().unwrap_or(0) as u64)),
                NoG::Refs { cid: c, refs, .. } if cid::e_lista_fixa(*c) => Portavel::Lista(refs.iter().map(|x| val(g, x, true, visitando)).collect()),
                NoG::Compacta { forma, elementos, .. } => Portavel::Lista(
                    elementos
                        .iter()
                        .map(|&x| match forma {
                            Elemento::Double => Portavel::Double(f64::from_bits(x as u64)),
                            Elemento::Bool => Portavel::Bool(x != 0),
                            _ => Portavel::Int(x),
                        })
                        .collect(),
                ),
                NoG::Instancia { cid: cid::GROWABLE_LIST, campos } => {
                    // O comprimento (campo 0, bruto) e o armazenamento (campo 1).
                    let n = match campos.first() {
                        Some((ValG::Palavra(n), false)) => usize::try_from(*n).unwrap_or(0),
                        _ => 0,
                    };
                    match campos.get(1).map(|(d, _)| val(g, d, true, visitando)) {
                        Some(Portavel::Lista(mut itens)) => {
                            itens.truncate(n);
                            Portavel::Lista(itens)
                        }
                        _ => Portavel::Lista(Vec::new()),
                    }
                }
                NoG::Instancia { campos, .. } => Portavel::Objeto(campos.iter().map(|(x, e_ref)| val(g, x, *e_ref, visitando)).collect()),
                NoG::Tipada { bytes, .. } => Portavel::Bytes(bytes.clone()),
                NoG::DoRuntime { cid, id } => Portavel::DoRuntime { cid: *cid, id: *id },
                NoG::Visao { cid: c, base, deslocamento, len } => {
                    let tipo = if *c >= cid::BYTE_DATA_VIEW { TIPO_UINT8 } else { ((*c - cid::PRIMEIRA_VISAO) % cid::TIPOS_DE_ELEMENTO) as u8 };
                    let n = len * tamanho_do_elemento(tipo);
                    match val(g, base, true, visitando) {
                        Portavel::Bytes(b) => Portavel::Bytes(b[(*deslocamento).min(b.len())..(deslocamento + n).min(b.len())].to_vec()),
                        _ => Portavel::Nulo,
                    }
                }
                _ => Portavel::Nulo,
            }
        }
        let mut visitando = vec![false; self.nos.len()];
        val(self, &self.raiz, true, &mut visitando)
    }
}

impl Portavel {
    /// O grafo deste valor.
    pub fn para_grafo(&self) -> Grafo {
        fn no(n: NoG, nos: &mut Vec<(NoG, i64)>) -> ValG {
            nos.push((n, 0));
            ValG::No(nos.len() - 1)
        }
        fn val(p: &Portavel, nos: &mut Vec<(NoG, i64)>) -> ValG {
            use crate::layout::{cid, flags};
            match p {
                Portavel::Nulo | Portavel::Objeto(_) => ValG::Palavra(0),
                Portavel::Bool(b) => ValG::Estatico(Heap::caixa_bool(*b)),
                Portavel::Int(x) => match smi::de(*x) {
                    Some(r) => ValG::Palavra(r),
                    None => no(NoG::Bruto { cid: cid::MINT, flags: flags::BRUTO, palavras: Box::new([*x]) }, nos),
                },
                Portavel::Double(x) => no(NoG::Bruto { cid: cid::DOUBLE, flags: flags::BRUTO, palavras: Box::new([x.to_bits() as i64]) }, nos),
                Portavel::Str(s) => no(NoG::Texto(Texto::de_str(s)), nos),
                Portavel::Lista(itens) => {
                    let i = nos.len();
                    nos.push((NoG::Acumulador(Vec::new()), 0));
                    let refs: Vec<ValG> = itens.iter().map(|x| val(x, nos)).collect();
                    nos[i] = (NoG::Refs { cid: cid::LIST, palavra0: refs.len() as i64, refs }, 0);
                    ValG::No(i)
                }
                Portavel::Bytes(b) => no(NoG::Tipada { tipo: TIPO_UINT8, bytes: b.clone() }, nos),
                Portavel::Tipada(tipo, b) => no(NoG::Tipada { tipo: *tipo, bytes: b.clone() }, nos),
                Portavel::DoRuntime { cid, id } => no(NoG::DoRuntime { cid: *cid, id: *id }, nos),
            }
        }
        let mut nos = Vec::new();
        let raiz = val(self, &mut nos);
        Grafo { nos, raiz, origem: 0, tipos: None, tabelas: Vec::new() }
    }
}

/// Aloca, no heap deste isolado, o bloco vazio do nó `no` (as referências
/// ficam para depois: o grafo pode ter ciclos). `None` para a visão, que
/// precisa da base já alocada, e para os anexos (acumulador, programa de
/// `RegExp`), que o runtime cria fora do empréstimo do heap
/// ([`alocar_anexo`]).
fn alocar_no(heap: &mut Heap, no: &NoG) -> Option<i64> {
    use crate::layout::{cid, flags};
    Some(match no {
        NoG::Instancia { cid: c, campos } => {
            let vazios: Vec<crate::heap::Campo> = campos.iter().map(|&(_, e_ref)| (0, e_ref)).collect();
            heap.novo_objeto(i64::from(*c), &vazios)
        }
        NoG::Refs { cid: c, palavra0, refs } => {
            if cid::e_lista_fixa(*c) {
                heap.nova_lista(*c, refs.len(), crate::listas::Elemento::Geral)
            } else if *c == cid::RECORD {
                heap.novo_record(&vec![0; refs.len()])
            } else {
                let h = heap.alocar(*c, 1 + refs.len(), flags::REFS);
                heap.palavras_mut(h)[0] = *palavra0;
                h
            }
        }
        NoG::Compacta { cid: c, forma, elementos } => {
            let h = heap.nova_lista(*c, elementos.len(), *forma);
            // Antes de publicar: sem barreira (e são escalares).
            heap.palavras_mut(h)[1..=elementos.len()].copy_from_slice(elementos);
            h
        }
        NoG::Bruto { cid: c, flags: f, palavras } => {
            let h = heap.alocar(*c, palavras.len(), *f);
            heap.palavras_mut(h).copy_from_slice(palavras);
            h
        }
        NoG::Texto(t) => heap.alocar_texto(t.vista()),
        NoG::Tipada { tipo, bytes } => {
            let tipo = if *tipo == TIPO_BYTE_DATA { TIPO_UINT8 } else { *tipo };
            let h = heap.nova_tipada(tipo, bytes.len() / tamanho_do_elemento(tipo));
            if let Some(b) = heap.bytes_da_tipada_mut(h) {
                let n = b.len().min(bytes.len());
                b[..n].copy_from_slice(&bytes[..n]);
            }
            h
        }
        NoG::DoRuntime { cid, id } => heap.novo_objeto(i64::from(*cid), &[(*id, false)]),
        NoG::Visao { .. } | NoG::Acumulador(_) | NoG::ProgramaRe { .. } => return None,
    })
}

/// O bloco de um nó com anexo nativo (o acumulador do `StringBuffer`, o
/// programa do `RegExp`), criado pelo dono do anexo (`nativos_strings.rs`,
/// `regexp.rs`), que empresta o heap ele mesmo. `None` para os outros nós.
fn alocar_anexo(no: &NoG) -> Option<i64> {
    match no {
        NoG::Acumulador(u) => Some(novo_acumulador_com(u.clone())),
        NoG::ProgramaRe { fonte, opcoes } => Some(programa_re_de_padrao(fonte, *opcoes)),
        _ => None,
    }
}

/// Recria o grafo no heap deste isolado e devolve o valor (numa posição
/// `Ref`). Todos os nós são alocados primeiro, enraizados, e as arestas
/// ligadas depois — o grafo pode ter ciclos.
fn materializar(g: &Grafo) -> i64 {
    let mesmo = g.origem == id_do_isolado();
    // As tabelas de métodos das classes que este isolado ainda não viu.
    if !g.tabelas.is_empty() {
        METODOS.with(|m| {
            let mut m = m.borrow_mut();
            for (c, t) in &g.tabelas {
                m.entry(*c).or_insert_with(|| t.clone());
            }
        });
    }
    // Os tipos da mensagem no universo deste isolado, e os objetos `Type`
    // canônicos dela (fora do empréstimo do heap).
    let ids_de_tipo = g.tipos.as_ref().map(TiposDaMensagem::importar);
    let mut objetos_tipo: crate::hash::HashMap<usize, i64> = crate::hash::HashMap::default();
    if let Some(ids) = &ids_de_tipo {
        for (i, (no, _)) in g.nos.iter().enumerate() {
            if let NoG::Instancia { cid, campos } = no
                && i64::from(*cid) == CLASSE_TIPO
                && let Some((ValG::Palavra(k), _)) = campos.first()
            {
                objetos_tipo.insert(i, dartforge_rti_objeto_tipo(ids[*k as usize]));
            }
        }
    }
    // As constantes canônicas deste isolado, pelos getters (fora do
    // empréstimo do heap: o getter é código gerado, que aloca na primeira
    // vez). São permanentes: não precisam de raiz durante a montagem.
    let mut canonicas: crate::hash::HashMap<usize, i64> = crate::hash::HashMap::default();
    let mut tearoffs: crate::hash::HashMap<i64, i64> = crate::hash::HashMap::default();
    g.visitar_valores(|v| match *v {
        ValG::Constante(getter) => {
            canonicas.entry(getter).or_insert_with(|| {
                // SAFETY: `getter` é o endereço de um getter gerado sem
                // argumentos (registrado por `dartforge_marcar_constante`),
                // o mesmo em todos os isolados do processo.
                let f = { let alvo_dart: usize = getter; move || -> i64 { dart_r0(alvo_dart) } };
                f()
            });
        }
        ValG::TearOff { codigo, tipado, abi, metadado } => {
            tearoffs.entry(codigo).or_insert_with(|| {
                HEAP.with(|h| {
                    let mut heap = h.borrow_mut();
                    if let Some(t) = heap.tearoff_registrado(codigo) {
                        return t;
                    }
                    let t = heap.nova_closure(codigo, (0, false), tipado, abi);
                    if metadado != 0 {
                        heap.set_metadado(t, i64::from(metadado));
                    }
                    heap.registrar_tearoff(codigo, t);
                    t
                })
            });
        }
        _ => {}
    });
    // Cópias proprietárias dos nós, inclusive anexos criados fora do
    // empréstimo do heap, duram até ligar as arestas e refazer os índices.
    let frame = HEAP.with(|heap| heap.borrow_mut().push_frame_proprietario(g.nos.len()));
    let mut handles = vec![0i64; g.nos.len()];
    for (i, (no, _)) in g.nos.iter().enumerate() {
        if let Some(h) = alocar_anexo(no) {
            HEAP.with(|heap| heap.borrow_mut().set_root(frame, i, h));
            handles[i] = h;
        }
    }
    let resultado = HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        for (i, (no, _)) in g.nos.iter().enumerate() {
            if handles[i] != 0 {
                continue;
            }
            let h = match objetos_tipo.get(&i) {
                Some(&h) => h,
                None => match alocar_no(&mut heap, no) {
                    Some(h) => h,
                    None => continue,
                },
            };
            heap.set_root(frame, i, h);
            handles[i] = h;
        }
        let palavra = |v: &ValG, handles: &[i64]| -> i64 {
            match *v {
                ValG::Palavra(bits) => bits,
                ValG::No(i) => handles[i],
                ValG::Estatico(h) => h,
                ValG::Mesmo(h) if mesmo => h,
                ValG::Constante(getter) => canonicas.get(&getter).copied().unwrap_or(0),
                ValG::TearOff { codigo, .. } => tearoffs.get(&codigo).copied().unwrap_or(0),
                // Fora do isolado de origem não há o que compartilhar: o
                // emissor copia (`compartilhar` falso) para outro isolado.
                ValG::Mesmo(_) => 0,
            }
        };
        // As visões, depois das bases (uma visão de visão espera a dela).
        loop {
            let mut pendentes = false;
            let mut progresso = false;
            for (i, (no, _)) in g.nos.iter().enumerate() {
                let NoG::Visao { cid, base, deslocamento, len } = no else { continue };
                if handles[i] != 0 {
                    continue;
                }
                let b = palavra(base, &handles);
                if b == 0 && matches!(base, ValG::No(_)) {
                    pendentes = true;
                    continue;
                }
                let h = heap.nova_visao(*cid, b, *deslocamento, *len);
                heap.set_root(frame, i, h);
                handles[i] = h;
                progresso = true;
            }
            if !pendentes || !progresso {
                break;
            }
        }
        for (i, (no, meta)) in g.nos.iter().enumerate() {
            if objetos_tipo.contains_key(&i) {
                continue;
            }
            let h = handles[i];
            if h == 0 {
                continue;
            }
            match no {
                NoG::Instancia { campos, .. } => {
                    for (k, (v, e_ref)) in campos.iter().enumerate() {
                        let bits = palavra(v, &handles);
                        heap.definir_campo(h, k, bits, *e_ref);
                    }
                }
                NoG::Refs { refs, .. } if !refs.is_empty() => {
                    let novos: Vec<i64> = refs.iter().map(|v| palavra(v, &handles)).collect();
                    heap.gravar_refs(h, 1, &novos);
                }
                _ => {}
            }
            if *meta != 0 {
                let meta = match &ids_de_tipo {
                    Some(ids) => 1 + ids[(*meta - 1) as usize],
                    None => *meta,
                };
                heap.set_metadado(h, meta);
            }
        }
        palavra(&g.raiz, &handles)
    });
    refazer_indices_copiados(g, &handles);
    HEAP.with(|heap| heap.borrow_mut().pop_frame(frame));
    resultado
}

/// Os `_Map`/`_Set` copiados chegam com o `_index` da origem, calculado com
/// os hashes de identidade de lá (aqui o hash é o endereço, `heap.rs`
/// `hash_de_identidade`): como a VM (`runtime/vm/object_graph_copy.cc`,
/// `CopyLinkedHashBase` + `_rehashObjects`), o índice é refeito no destino
/// quando alguma chave pode ter outro hash. A decisão e o refazer são do
/// Dart (`_dartforgeRefazerIndiceCopiado`, `sdk_nativo/collection/
/// compact_hash.dart`): -1 para objeto que não é `_Map`/`_Set` (a classe
/// inteira fica de fora daí em diante). Os nós continuam enraizados no
/// quadro de [`materializar`].
fn refazer_indices_copiados(g: &Grafo, handles: &[i64]) {
    let Some(f) = ajudante("_dartforgeRefazerIndiceCopiado") else { return };
    // SAFETY: registrado pela `dart:_compact_hash` com a assinatura
    // `(Object) -> int`.
    let refazer = { let alvo_dart: usize = f; move |a0: i64| -> i64 { dart_r1(alvo_dart, a0) } };
    let mut outras: crate::hash::HashSet<i32> = crate::hash::HashSet::default();
    for (i, (no, _)) in g.nos.iter().enumerate() {
        let NoG::Instancia { cid, .. } = no else { continue };
        if handles[i] == 0 || outras.contains(cid) {
            continue;
        }
        if refazer(handles[i]) < 0 {
            outras.insert(*cid);
        }
        if dartforge_exception_pending() != 0 {
            return;
        }
    }
}

// ---------------------------------------------------------------------------
// O registro das portas e a fila do isolado.

#[cfg(test)]
mod testes_owners_materializacao {
    use super::*;

    #[test]
    fn fila_encerrada_descarta_grafo_sem_consultar_handle_ja_morto() {
        let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
        let fila = FILA.with(|f| f.clone());
        let (quadro, texto) = HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.ativar_arc();
            let quadro = h.push_frame_proprietario(1);
            let texto = h.alocar_str("não publicar");
            h.set_root(quadro, 0, texto);
            (quadro, texto)
        });
        let grafo = copiar_para_grafo(texto, true).ok().unwrap();
        fechar_portas_do_isolado();
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.pop_frame(quadro);
            h.collect();
            assert!(!h.e_objeto_vivo(texto));
        });
        postar_na_fila(&fila, 123, grafo);
        assert!(fila.mensagens.lock().unwrap().normal.is_empty());
        HEAP.with(|h| { h.replace(anterior); });
    }

    #[test]
    fn remetente_com_clone_antigo_nao_publica_apos_encerramento() {
        let fila = FILA.with(|f| f.clone());
        let barreira = std::sync::Arc::new(std::sync::Barrier::new(2));
        let liberar = barreira.clone();
        let destino = fila.clone();
        let remetente = std::thread::spawn(move || {
            liberar.wait();
            postar_na_fila(&destino, 123, Grafo {
                nos: Vec::new(), raiz: ValG::Palavra(0), origem: 0,
                tipos: None, tabelas: Vec::new(),
            });
        });
        fechar_portas_do_isolado();
        barreira.wait();
        remetente.join().unwrap();
        let m = fila.mensagens.lock().unwrap();
        assert!(m.encerrada && m.normal.is_empty());
    }

    #[test]
    fn encerramento_do_isolado_solta_owners_de_mensagens_pendentes() {
        let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
        let (quadro, texto) = HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.ativar_arc();
            let quadro = h.push_frame_proprietario(1);
            let texto = h.alocar_str("mensagem não atendida");
            h.set_root(quadro, 0, texto);
            (quadro, texto)
        });
        let porta = dartforge_nativo_DartForge_porta_abrir(0);
        assert_eq!(dartforge_nativo_DartForge_porta_enviar(porta, texto), 0);
        assert_eq!(dartforge_nativo_DartForge_porta_enviar(porta, texto), 0);
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.pop_frame(quadro);
            h.collect();
            assert!(h.e_objeto_vivo(texto));
        });
        fechar_portas_do_isolado();
        FILA.with(|f| assert!(f.mensagens.lock().unwrap().normal.is_empty()));
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.collect();
            assert!(!h.e_objeto_vivo(texto));
        });
        HEAP.with(|h| { h.replace(anterior); });
    }

    extern "C" fn verificar_mensagem(_: i64) -> i64 {
        let valor = dartforge_nativo_DartForge_mensagem_atual();
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.collect();
            let texto = h.palavras(valor)[1];
            assert_eq!(h.palavras(valor)[2], texto);
            assert_eq!(h.texto(texto).unwrap().para_string(), "aliases na fila");
        });
        0
    }

    #[test]
    fn aliases_compartilhados_passam_da_fila_ao_despacho_sem_owner_residual() {
        let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
        let (quadro, texto, lista, despachante) = HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.ativar_arc();
            let quadro = h.push_frame_proprietario(1);
            let texto = h.alocar_str("aliases na fila");
            h.set_root(quadro, 0, texto);
            let lista = h.nova_lista(crate::layout::cid::LIST, 2, crate::listas::Elemento::Geral);
            h.gravar_refs(lista, 1, &[texto, texto]);
            h.set_root(quadro, 0, lista);
            let despachante = h.nova_closure(123, (0, false), 0, 0);
            (quadro, texto, lista, despachante)
        });
        dartforge_nativo_DartForge_portas_despachante(despachante);
        let porta = dartforge_nativo_DartForge_porta_abrir(0);
        assert_eq!(dartforge_nativo_DartForge_porta_enviar(porta, lista), 0);
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.pop_frame(quadro);
            h.collect();
            assert!(!h.e_objeto_vivo(lista) && h.e_objeto_vivo(texto));
        });
        assert!(despachar_proxima(verificar_mensagem));
        dartforge_nativo_DartForge_porta_fechar(porta);
        dartforge_nativo_DartForge_portas_despachante(0);
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.collect();
            assert!(!h.e_objeto_vivo(texto));
            assert!(!h.e_objeto_vivo(despachante));
        });
        HEAP.with(|h| { h.replace(anterior); });
    }

    #[test]
    fn texto_compartilhado_na_fila_sobrevive_sem_owner_do_emissor() {
        let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
        let (quadro, texto) = HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.ativar_arc();
            let quadro = h.push_frame_proprietario(1);
            let texto = h.alocar_str("mensagem compartilhada");
            h.set_root(quadro, 0, texto);
            (quadro, texto)
        });
        let porta = dartforge_nativo_DartForge_porta_abrir(0);
        assert_eq!(dartforge_nativo_DartForge_porta_enviar(porta, texto), 0);
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.pop_frame(quadro);
            h.collect();
            assert!(h.e_objeto_vivo(texto), "a fila deve manter o owner compartilhado");
        });
        dartforge_nativo_DartForge_porta_fechar(porta);
        extern "C" fn ignorar(_: i64) -> i64 { 0 }
        assert!(despachar_proxima(ignorar));
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.collect();
            assert!(!h.e_objeto_vivo(texto), "descarte da mensagem deve soltar seu owner");
        });
        HEAP.with(|h| { h.replace(anterior); });
    }

    #[test]
    fn grafo_ciclico_com_aliases_preserva_nos_ate_ligar_arestas() {
        let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
        let quadro = HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.ativar_arc();
            h.push_frame_proprietario(1)
        });
        let grafo = Grafo {
            nos: vec![
                (NoG::Refs { cid: crate::layout::cid::LIST, palavra0: 2,
                    refs: vec![ValG::No(1), ValG::No(1)] }, 0),
                (NoG::Refs { cid: crate::layout::cid::LIST, palavra0: 1,
                    refs: vec![ValG::No(0)] }, 0),
            ],
            raiz: ValG::No(0),
            origem: id_do_isolado(),
            tipos: None,
            tabelas: Vec::new(),
        };
        let raiz = materializar(&grafo);
        HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.set_root(quadro, 0, raiz);
            h.collect();
            let filho = h.palavras(raiz)[1];
            assert_eq!(h.palavras(raiz)[2], filho);
            assert_eq!(h.palavras(filho)[1], raiz);
            h.pop_frame(quadro);
            h.collect();
            assert!(!h.e_objeto_vivo(raiz) && !h.e_objeto_vivo(filho));
        });
        HEAP.with(|h| { h.replace(anterior); });
    }
}

/// Uma mensagem na fila de um isolado.
struct Mensagem {
    porta: i64,
    grafo: Grafo,
    /// Tokens do heap destinatário para cada ocorrência `Mesmo`.
    owners: Vec<u64>,
    chegada: std::time::Instant,
}

/// As mensagens de um isolado: as das portas do Dart e as da porta de
/// controle (as OOB da VM: `pause`, `kill`, `ping`…), que o runtime atende
/// antes das outras.
#[derive(Default)]
struct Filas {
    /// Protegido pelo mesmo mutex da publicação: clones antigos não reabrem a fila.
    encerrada: bool,
    normal: std::collections::VecDeque<Mensagem>,
    controle: std::collections::VecDeque<Grafo>,
    /// Os pedidos para rodar no ponto seguro (a publicação de uma recarga
    /// do JIT), atendidos com o controle, entre um evento e outro.
    pontos_seguros: std::collections::VecDeque<PedidoNoPontoSeguro>,
}

/// Um pedido para rodar no ponto seguro do isolado: a função recebe o dado
/// e `1` quando roda no ponto seguro, ou `0` quando o isolado terminou sem
/// atendê-lo (o pedido é descartado, e quem pediu fica sabendo).
pub type PedidoNoPontoSeguro = (extern "C" fn(usize, i32), usize);

/// A fila de mensagens de um isolado; qualquer thread posta nela.
pub struct FilaDoIsolado {
    id: u64,
    mensagens: std::sync::Mutex<Filas>,
    sinal: std::sync::Condvar,
    /// A espera com prazo no macOS (ver [`Despertador`]).
    despertador: Despertador,
    /// O endereço do pedido de interrupção da thread do isolado
    /// (`Contexto::interrupcao`), ou 0 depois que ela terminou. Uma mensagem
    /// de controle o liga: o código gerado o lê no ponto seguro de cada volta
    /// de laço e chama [`dartforge_ponto_seguro`], que atende o controle sem
    /// esperar o próximo evento — um `kill` imediato ou um `ping` alcançam
    /// um isolado preso num laço sem eventos, como a verificação de pilha da
    /// VM. O mutex garante que ninguém grava depois que a thread o zerou.
    interrupcao: std::sync::Mutex<usize>,
}

impl FilaDoIsolado {
    /// Acorda a thread do isolado: o `Condvar` e, no macOS, o despertador.
    fn avisar(&self) {
        self.sinal.notify_one();
        self.despertador.acordar();
    }
}

/// A espera com prazo do laço de eventos no macOS: um `kqueue` com um
/// `EVFILT_USER` que o aviso aciona e o prazo no `timeout` do `kevent`, como
/// o manipulador de eventos da VM (`eventhandler_macos.cc`, `Poll`). O
/// `pthread_cond_timedwait` do `Condvar` passa do prazo pela folga de
/// coalescência do kernel: 200 timers de 5 ms levavam 5,9 s contra 1,5 s na
/// VM (a sonda `tools/sondas/precisao_de_timers.dart`), e o
/// `corpus/nativo/33_mensagens_de_controle` estourava o tempo. Fora do macOS
/// o `Condvar` já é preciso e o despertador não faz nada.
struct Despertador {
    #[cfg(target_vendor = "apple")]
    kq: i32,
}

#[cfg(target_vendor = "apple")]
#[repr(C)]
struct EventoDoDespertador {
    ident: usize,
    filtro: i16,
    bandeiras: u16,
    fbandeiras: u32,
    dados: isize,
    udata: *mut std::ffi::c_void,
}

#[cfg(target_vendor = "apple")]
#[repr(C)]
struct PrazoDoDespertador {
    segundos: i64,
    nanos: i64,
}

#[cfg(target_vendor = "apple")]
unsafe extern "C" {
    #[link_name = "kqueue"]
    fn kqueue_do_despertador() -> i32;
    #[link_name = "kevent"]
    fn kevent_do_despertador(
        kq: i32,
        mudancas: *const EventoDoDespertador,
        nm: i32,
        eventos: *mut EventoDoDespertador,
        ne: i32,
        espera: *const PrazoDoDespertador,
    ) -> i32;
    #[link_name = "close"]
    fn close_do_despertador(fd: i32) -> i32;
}

#[cfg(target_vendor = "apple")]
impl Despertador {
    const ATIVO: bool = true;
    const EVFILT_USER: i16 = -10;
    const EV_ADD: u16 = 0x1;
    const EV_CLEAR: u16 = 0x20;
    const NOTE_TRIGGER: u32 = 0x0100_0000;

    fn evento(bandeiras: u16, fbandeiras: u32) -> EventoDoDespertador {
        EventoDoDespertador { ident: 1, filtro: Self::EVFILT_USER, bandeiras, fbandeiras, dados: 0, udata: std::ptr::null_mut() }
    }

    fn novo() -> Despertador {
        // SAFETY: cria o kqueue; o evento vive durante a chamada.
        let kq = unsafe { kqueue_do_despertador() };
        if kq < 0 {
            panic!("falha ao criar o kqueue do laço de eventos");
        }
        let ev = Self::evento(Self::EV_ADD | Self::EV_CLEAR, 0);
        // SAFETY: a mudança vive durante a chamada.
        if unsafe { kevent_do_despertador(kq, &ev, 1, std::ptr::null_mut(), 0, std::ptr::null()) } == -1 {
            panic!("falha ao registrar o despertador do laço de eventos");
        }
        Despertador { kq }
    }

    fn acordar(&self) {
        let ev = Self::evento(0, Self::NOTE_TRIGGER);
        // SAFETY: a mudança vive durante a chamada; o kqueue é deste isolado.
        unsafe { kevent_do_despertador(self.kq, &ev, 1, std::ptr::null_mut(), 0, std::ptr::null()) };
    }

    fn esperar(&self, prazo: std::time::Duration) {
        let ts = PrazoDoDespertador { segundos: prazo.as_secs() as i64, nanos: i64::from(prazo.subsec_nanos()) };
        let mut ev = Self::evento(0, 0);
        // SAFETY: um evento de saída; o prazo vive durante a chamada.
        unsafe { kevent_do_despertador(self.kq, std::ptr::null(), 0, &mut ev, 1, &ts) };
    }
}

#[cfg(target_vendor = "apple")]
impl Drop for Despertador {
    fn drop(&mut self) {
        // SAFETY: o kqueue é deste despertador.
        unsafe { close_do_despertador(self.kq) };
    }
}

#[cfg(not(target_vendor = "apple"))]
impl Despertador {
    const ATIVO: bool = false;
    fn novo() -> Despertador {
        Despertador {}
    }
    fn acordar(&self) {}
    fn esperar(&self, _prazo: std::time::Duration) {}
}

/// Zera o endereço do pedido de interrupção quando a thread termina (antes
/// de a memória dos `thread_local` dela ser liberada).
struct GuardaDaInterrupcao(std::sync::Arc<FilaDoIsolado>);

impl Drop for GuardaDaInterrupcao {
    fn drop(&mut self) {
        *self.0.interrupcao.lock().unwrap_or_else(|e| e.into_inner()) = 0;
    }
}

/// Um serviço nativo: recebe (porta, grafo) na thread de quem enviou.
pub type ServicoNativo = std::sync::Arc<dyn Fn(i64, Grafo) + Send + Sync>;

enum Dono {
    Isolado(std::sync::Arc<FilaDoIsolado>),
    /// A porta de controle de um isolado (`Isolate.controlPort`).
    Controle(std::sync::Arc<FilaDoIsolado>),
    Nativo(ServicoNativo),
}

fn registro() -> &'static std::sync::Mutex<crate::hash::HashMap<i64, Dono>> {
    static R: std::sync::OnceLock<std::sync::Mutex<crate::hash::HashMap<i64, Dono>>> = std::sync::OnceLock::new();
    R.get_or_init(Default::default)
}

fn proximo_id_de_porta() -> i64 {
    static PROX: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);
    PROX.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

thread_local! {
    static FILA: std::sync::Arc<FilaDoIsolado> = {
        static PROX: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let alvo = crate::heap::CONTEXTO.with(|c| &c.interrupcao as *const std::sync::atomic::AtomicU8 as usize);
        let fila = std::sync::Arc::new(FilaDoIsolado {
            id: PROX.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            mensagens: std::sync::Mutex::new(Filas::default()),
            sinal: std::sync::Condvar::new(),
            despertador: Despertador::novo(),
            interrupcao: std::sync::Mutex::new(alvo),
        });
        let guarda = GuardaDaInterrupcao(fila.clone());
        GUARDA_DA_INTERRUPCAO.with(|g| *g.borrow_mut() = Some(guarda));
        fila
    };
    /// Zera o endereço do pedido de interrupção no fim da thread.
    static GUARDA_DA_INTERRUPCAO: RefCell<Option<GuardaDaInterrupcao>> = const { RefCell::new(None) };
    /// Portas abertas deste isolado → se mantêm o isolado vivo.
    static PORTAS_ABERTAS: RefCell<crate::hash::HashMap<i64, bool>> = RefCell::new(crate::hash::HashMap::default());
    /// A mensagem em despacho: (porta, valor), o valor enraizado.
    static ATUAL: RefCell<(i64, i64)> = const { RefCell::new((0, 0)) };
    /// O despachante Dart (`_RawReceivePort._despachar`), enraizado.
    static DESPACHANTE: RefCell<i64> = const { RefCell::new(0) };
}

/// Raízes globais do runtime para as portas (ids negativos fora da faixa
/// dos timers de `eventos.rs`).
const RAIZ_DESPACHANTE: i64 = -0x7000_0000_0000;
const RAIZ_MENSAGEM: i64 = -0x7000_0000_0001;

fn id_do_isolado() -> u64 {
    FILA.with(|f| f.id)
}

/// Posta `grafo` para a porta `porta`: na fila do isolado dono, ou no
/// serviço nativo. Porta fechada ou inexistente: a mensagem é descartada,
/// como na VM.
pub fn postar(porta: i64, grafo: Grafo) {
    let dono = {
        let r = registro().lock().unwrap_or_else(|e| e.into_inner());
        match r.get(&porta) {
            Some(Dono::Isolado(f)) => Some(Dono::Isolado(f.clone())),
            Some(Dono::Controle(f)) => Some(Dono::Controle(f.clone())),
            Some(Dono::Nativo(s)) => Some(Dono::Nativo(s.clone())),
            None => None,
        }
    };
    match dono {
        Some(Dono::Isolado(f)) => {
            postar_na_fila(&f, porta, grafo);
        }
        Some(Dono::Controle(f)) => {
            let mut m = f.mensagens.lock().unwrap_or_else(|e| e.into_inner());
            if m.encerrada { return; }
            m.controle.push_back(grafo);
            f.avisar();
        }
        Some(Dono::Nativo(s)) => s(porta, grafo),
        None => {}
    }
}

/// Publica com a referência já obtida do registro; o mutex lineariza o fechamento.
fn postar_na_fila(f: &FilaDoIsolado, porta: i64, grafo: Grafo) {
    let mut m = f.mensagens.lock().unwrap_or_else(|e| e.into_inner());
    if m.encerrada { return; }
    // Retenção não coleta nem chama Dart; o fechamento usa este mesmo mutex.
    let mut owners = Vec::new();
    grafo.visitar_valores(|v| {
        if let ValG::Mesmo(h) = *v {
            assert_eq!(f.id, id_do_isolado(), "owner compartilhado fora do seu isolado");
            assert_eq!(grafo.origem, f.id, "origem do owner compartilhado incorreta");
            owners.push(HEAP.with(|heap| heap.borrow_mut().reter_owner_mensagem(h)));
        }
    });
    m.normal.push_back(Mensagem { porta, grafo, owners, chegada: std::time::Instant::now() });
    f.avisar();
}

/// Posta `grafo` na fila de controle do isolado dono de `porta` (as
/// mensagens OOB: `Isolate_sendOOB` vale para qualquer porta do isolado).
fn postar_controle(porta: i64, grafo: Grafo) {
    let fila = {
        let r = registro().lock().unwrap_or_else(|e| e.into_inner());
        match r.get(&porta) {
            Some(Dono::Isolado(f) | Dono::Controle(f)) => Some(f.clone()),
            _ => None,
        }
    };
    if let Some(f) = fila {
        {
            let mut m = f.mensagens.lock().unwrap_or_else(|e| e.into_inner());
            if m.encerrada { return; }
            m.controle.push_back(grafo);
        }
        let alvo = f.interrupcao.lock().unwrap_or_else(|e| e.into_inner());
        if *alvo != 0 {
            // SAFETY: o contexto da thread do isolado, vivo enquanto o
            // endereço não foi zerado (com este mutex) pela guarda dela.
            unsafe { (*(*alvo as *const std::sync::atomic::AtomicU8)).store(1, std::sync::atomic::Ordering::Release) };
        }
        drop(alvo);
        f.avisar();
    }
}

/// Apaga o pedido de interrupção deste isolado (quem o atende vai esvaziar a
/// fila de controle em seguida; uma mensagem que chegar depois liga de novo).
fn limpar_pedido_de_interrupcao() {
    crate::heap::CONTEXTO.with(|c| c.interrupcao.store(0, std::sync::atomic::Ordering::Relaxed));
}

// ---------------------------------------------------------------------------
// O ponto seguro do isolado principal (a recarga do JIT).
//
// A publicação de uma geração nova (a troca das células das entradas
// estáveis, o registro das tabelas) só pode acontecer quando nenhum quadro
// Dart do isolado está na pilha: entre dois eventos do laço — o ponto em que
// a VM também comita uma recarga (`isolate_reload.cc`). O observador do
// `dartforge reload` compila numa thread própria e entrega a publicação como
// um pedido na fila do isolado principal; o laço o atende antes do próximo
// evento, e um isolado ocioso (um servidor esperando conexão) acorda para ele.

fn isolado_principal() -> &'static std::sync::Mutex<Option<std::sync::Arc<FilaDoIsolado>>> {
    static P: std::sync::OnceLock<std::sync::Mutex<Option<std::sync::Arc<FilaDoIsolado>>>> = std::sync::OnceLock::new();
    P.get_or_init(Default::default)
}

/// Este isolado é o principal: passa a aceitar pedidos no ponto seguro.
pub fn marcar_isolado_principal() {
    let fila = FILA.with(|f| f.clone());
    *isolado_principal().lock().unwrap_or_else(|e| e.into_inner()) = Some(fila);
}

/// O isolado principal terminou: os pedidos pendentes são descartados (quem
/// pediu recebe `0`) e os novos, recusados.
pub fn desmarcar_isolado_principal() {
    retirar_isolado_vivo();
    let Some(fila) = isolado_principal().lock().unwrap_or_else(|e| e.into_inner()).take() else {
        return;
    };
    let pendentes = std::mem::take(&mut fila.mensagens.lock().unwrap_or_else(|e| e.into_inner()).pontos_seguros);
    for (f, dado) in pendentes {
        f(dado, 0);
    }
}

/// Pede que `f(dado, 1)` rode no próximo ponto seguro do isolado principal,
/// na thread dele. Devolve `0` (e não chama `f`) se não há isolado principal
/// rodando o laço de eventos; `1` se o pedido foi aceito — então `f` é
/// chamada exatamente uma vez, com `1` no ponto seguro ou com `0` se o
/// isolado terminar antes.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_pedir_no_ponto_seguro(f: extern "C" fn(usize, i32), dado: usize) -> i32 {
    // O registro fica travado durante a inserção: o descarte do fim do
    // isolado não perde um pedido que chegue ao mesmo tempo.
    let principal = isolado_principal().lock().unwrap_or_else(|e| e.into_inner());
    let Some(fila) = principal.as_ref() else {
        return 0;
    };
    fila.mensagens.lock().unwrap_or_else(|e| e.into_inner()).pontos_seguros.push_back((f, dado));
    fila.avisar();
    1
}

// A parada de todos os isolados (a publicação de uma recarga): o código
// novo só pode entrar com nenhum quadro Dart na pilha de NENHUM isolado,
// porque as células das entradas estáveis são do processo. O isolado
// principal, no ponto seguro dele, pede aos demais que parem no próximo
// ponto seguro deles (`dartforge_parar_isolados`), troca o código, e os
// libera (`dartforge_liberar_isolados`); cada um refaz os próprios registros
// da geração nova (o estado do runtime é por isolado) antes de continuar.

/// Os isolados vivos no laço de eventos (os que atendem pedidos).
fn isolados_vivos() -> &'static std::sync::Mutex<Vec<std::sync::Arc<FilaDoIsolado>>> {
    static V: std::sync::OnceLock<std::sync::Mutex<Vec<std::sync::Arc<FilaDoIsolado>>>> = std::sync::OnceLock::new();
    V.get_or_init(Default::default)
}

thread_local! {
    static VIVO_REGISTRADO: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Este isolado entrou no laço: passa a ser parado nas publicações.
fn registrar_isolado_vivo() {
    if VIVO_REGISTRADO.with(|r| r.replace(true)) {
        return;
    }
    let fila = FILA.with(|f| f.clone());
    isolados_vivos().lock().unwrap_or_else(|e| e.into_inner()).push(fila);
}

/// Este isolado terminou: sai do registro, e os pedidos pendentes dele são
/// respondidos com `0` (sob o mesmo trinco: nenhum pedido chega depois).
fn retirar_isolado_vivo() {
    if !VIVO_REGISTRADO.with(|r| r.replace(false)) {
        return;
    }
    let meu = id_do_isolado();
    let pendentes = {
        let mut vivos = isolados_vivos().lock().unwrap_or_else(|e| e.into_inner());
        vivos.retain(|f| f.id != meu);
        FILA.with(|f| std::mem::take(&mut f.mensagens.lock().unwrap_or_else(|e| e.into_inner()).pontos_seguros))
    };
    for (f, dado) in pendentes {
        f(dado, 0);
    }
}

/// A barreira de uma parada: quantos pararam, e os registros da geração nova
/// quando o principal libera.
#[derive(Default)]
struct Barreira {
    estado: std::sync::Mutex<(usize, Option<[usize; 3]>)>,
    sinal: std::sync::Condvar,
}

/// O pedido de parada, num isolado que não é o principal.
extern "C" fn parar_neste_isolado(dado: usize, executar: i32) {
    // SAFETY: `dado` é um `Arc<Barreira>` entregue por `dartforge_parar_isolados`
    // (uma referência por pedido, devolvida aqui).
    let barreira = unsafe { std::sync::Arc::from_raw(dado as *const Barreira) };
    let mut estado = barreira.estado.lock().unwrap_or_else(|e| e.into_inner());
    estado.0 += 1;
    barreira.sinal.notify_all();
    if executar == 0 {
        return;
    }
    while estado.1.is_none() {
        estado = barreira.sinal.wait(estado).unwrap_or_else(|e| e.into_inner());
    }
    let [area, registrar, rti] = estado.1.expect("liberado");
    drop(estado);
    let como_fn = |p: usize| {
        // SAFETY: 0 ou um trampolim `void ()` da geração publicada.
        (p != 0).then(|| unsafe { std::mem::transmute::<usize, extern "C" fn()>(p) })
    };
    dartforge_publicar_geracao(como_fn(area), como_fn(registrar), como_fn(rti));
}

/// Para todos os isolados vivos além deste no ponto seguro deles e devolve a
/// parada, que [`dartforge_liberar_isolados`] encerra. Chamada no ponto
/// seguro do isolado principal.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_parar_isolados() -> usize {
    let meu = id_do_isolado();
    let barreira = std::sync::Arc::new(Barreira::default());
    let mut n = 0;
    {
        let vivos = isolados_vivos().lock().unwrap_or_else(|e| e.into_inner());
        for f in vivos.iter().filter(|f| f.id != meu) {
            let dado = std::sync::Arc::into_raw(barreira.clone()) as usize;
            f.mensagens.lock().unwrap_or_else(|e| e.into_inner()).pontos_seguros.push_back((parar_neste_isolado, dado));
            f.avisar();
            n += 1;
        }
    }
    let mut estado = barreira.estado.lock().unwrap_or_else(|e| e.into_inner());
    while estado.0 < n {
        estado = barreira.sinal.wait(estado).unwrap_or_else(|e| e.into_inner());
    }
    drop(estado);
    std::sync::Arc::into_raw(barreira) as usize
}

/// Libera os isolados da parada `parada`: cada um refaz os registros da
/// geração nova (`area`, `registrar`, `rti`, trampolins `void ()` ou 0) na
/// thread dele e continua.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_liberar_isolados(parada: usize, area: usize, registrar: usize, rti: usize) {
    // SAFETY: `parada` veio de `dartforge_parar_isolados`, uma vez.
    let barreira = unsafe { std::sync::Arc::from_raw(parada as *const Barreira) };
    barreira.estado.lock().unwrap_or_else(|e| e.into_inner()).1 = Some([area, registrar, rti]);
    barreira.sinal.notify_all();
}

/// Atende os pedidos no ponto seguro deste isolado (o laço de eventos).
fn atender_pontos_seguros() {
    loop {
        let pedido = FILA.with(|f| f.mensagens.lock().unwrap_or_else(|e| e.into_inner()).pontos_seguros.pop_front());
        let Some((f, dado)) = pedido else { return };
        f(dado, 1);
    }
}

/// A próxima mensagem da porta de controle deste isolado.
fn proxima_de_controle() -> Option<Grafo> {
    limpar_pedido_de_interrupcao();
    FILA.with(|f| f.mensagens.lock().unwrap_or_else(|e| e.into_inner()).controle.pop_front())
}

/// Abre a porta de controle deste isolado e devolve o id.
fn abrir_porta_de_controle() -> i64 {
    let id = proximo_id_de_porta();
    let fila = FILA.with(|f| f.clone());
    registro().lock().unwrap_or_else(|e| e.into_inner()).insert(id, Dono::Controle(fila));
    id
}

/// Fecha todas as portas deste isolado (ele terminou): as mensagens que
/// chegarem depois são descartadas, como na VM.
fn fechar_portas_do_isolado() {
    let meu = id_do_isolado();
    registro().lock().unwrap_or_else(|e| e.into_inner()).retain(|_, d| match d {
        Dono::Isolado(f) | Dono::Controle(f) => f.id != meu,
        Dono::Nativo(_) => true,
    });
    PORTAS_ABERTAS.with(|p| p.borrow_mut().clear());
    let pendentes = FILA.with(|f| {
        let mut m = f.mensagens.lock().unwrap_or_else(|e| e.into_inner());
        m.encerrada = true;
        m.controle.clear();
        std::mem::take(&mut m.normal)
    });
    for m in pendentes { soltar_owners_da_mensagem(&m.owners); }
}

/// Fecha uma porta nativa: as mensagens seguintes são descartadas.
pub fn fechar_porta_nativa(id: i64) {
    registro().lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
}

/// Abre uma porta nativa atendida por `servico` e devolve o id.
pub fn abrir_porta_nativa(servico: ServicoNativo) -> i64 {
    let id = proximo_id_de_porta();
    registro().lock().unwrap_or_else(|e| e.into_inner()).insert(id, Dono::Nativo(servico));
    id
}

thread_local! {
    /// `NativeCallable.isolateLocal` abertos com `keepIsolateAlive`
    /// (`ffi_callbacks.rs`): cada um mantém o isolado vivo como uma porta.
    static CALLBACKS_QUE_MANTEM_VIVO: std::cell::Cell<i64> = const { std::cell::Cell::new(0) };
}

/// Soma `delta` aos callbacks locais que mantêm o isolado vivo.
pub fn ajustar_callbacks_que_mantem_vivo(delta: i64) {
    CALLBACKS_QUE_MANTEM_VIVO.with(|c| c.set((c.get() + delta).max(0)));
}

/// Se o isolado tem porta aberta (ou callback local) que o mantém vivo.
fn tem_porta_viva() -> bool {
    CALLBACKS_QUE_MANTEM_VIVO.with(|c| c.get() > 0) || PORTAS_ABERTAS.with(|p| p.borrow().values().any(|&v| v))
}

/// A chegada da mensagem mais antiga da fila, se houver.
fn chegada_da_proxima() -> Option<std::time::Instant> {
    FILA.with(|f| f.mensagens.lock().unwrap_or_else(|e| e.into_inner()).normal.front().map(|m| m.chegada))
}

/// Espera uma mensagem até `prazo` (ou sem prazo). Devolve se chegou. Com
/// `so_controle` (o isolado pausado), só a de controle acorda.
fn esperar_mensagem(prazo: Option<std::time::Instant>, so_controle: bool) -> bool {
    FILA.with(|f| {
        let mut m = f.mensagens.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if !m.controle.is_empty() || !m.pontos_seguros.is_empty() || (!so_controle && !m.normal.is_empty()) {
                return true;
            }
            match prazo {
                None => m = f.sinal.wait(m).unwrap_or_else(|e| e.into_inner()),
                Some(p) => {
                    let agora = std::time::Instant::now();
                    if agora >= p {
                        return false;
                    }
                    if Despertador::ATIVO {
                        // O `kevent` com prazo (ver [`Despertador`]): o aviso
                        // que chega entre soltar o mutex e esperar fica
                        // pendente no `EVFILT_USER` e acorda na hora.
                        drop(m);
                        f.despertador.esperar(p - agora);
                        m = f.mensagens.lock().unwrap_or_else(|e| e.into_inner());
                    } else {
                        m = f.sinal.wait_timeout(m, p - agora).unwrap_or_else(|e| e.into_inner()).0;
                    }
                }
            }
        }
    })
}

/// Solta tokens exclusivamente no heap que os criou (o destinatário).
fn soltar_owners_da_mensagem(owners: &[u64]) {
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        for &id in owners { h.soltar_owner_mensagem(id); }
    });
}

/// Tira a próxima mensagem, materializa e despacha pelo `chamar` do código
/// gerado. Devolve se havia mensagem.
fn despachar_proxima(chamar: extern "C" fn(i64) -> i64) -> bool {
    let Some(m) = FILA.with(|f| f.mensagens.lock().unwrap_or_else(|e| e.into_inner()).normal.pop_front()) else {
        return false;
    };
    // Mensagem para porta já fechada: descartada, como na VM.
    if !PORTAS_ABERTAS.with(|p| p.borrow().contains_key(&m.porta)) {
        soltar_owners_da_mensagem(&m.owners);
        return true;
    }
    let despachante = DESPACHANTE.with(|d| *d.borrow());
    if despachante == 0 {
        soltar_owners_da_mensagem(&m.owners);
        return true;
    }
    let valor = materializar(&m.grafo);
    HEAP.with(|h| h.borrow_mut().set_global_root(RAIZ_MENSAGEM, valor));
    soltar_owners_da_mensagem(&m.owners);
    ATUAL.with(|a| *a.borrow_mut() = (m.porta, valor));
    dart_r1(chamar as usize, despachante);
    ATUAL.with(|a| *a.borrow_mut() = (0, 0));
    HEAP.with(|h| h.borrow_mut().set_global_root(RAIZ_MENSAGEM, 0));
    true
}

// ---------------------------------------------------------------------------
// Natives da sobreposição (`sdk_nativo/isolate/isolate_patch.dart`).

/// `DartForge_porta_abrir(porta)`: abre uma porta deste isolado para o
/// objeto `porta` (um `_RawReceivePort`, cuja classe passa a ser não
/// enviável) e devolve o id.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_porta_abrir(objeto: i64) -> i64 {
    let classe = HEAP.with(|h| h.borrow().objeto(objeto).map(|o| o.class_id));
    if let Some(c) = classe {
        NAO_ENVIAVEIS.with(|n| {
            n.borrow_mut().entry(c).or_insert_with(|| "(object is a ReceivePort)\n".to_string());
        });
    }
    let id = proximo_id_de_porta();
    let fila = FILA.with(|f| f.clone());
    registro().lock().unwrap_or_else(|e| e.into_inner()).insert(id, Dono::Isolado(fila));
    PORTAS_ABERTAS.with(|p| p.borrow_mut().insert(id, true));
    id
}

/// `DartForge_porta_fechar(id)`: fecha a porta; mensagens pendentes para
/// ela são descartadas no despacho.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_porta_fechar(id: i64) {
    registro().lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
    PORTAS_ABERTAS.with(|p| p.borrow_mut().remove(&id));
}

/// `DartForge_porta_manter_vivo(id, manter)`: `keepIsolateAlive`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_porta_manter_vivo(id: i64, manter: u8) {
    PORTAS_ABERTAS.with(|p| {
        if let Some(v) = p.borrow_mut().get_mut(&id) {
            *v = manter != 0;
        }
    });
}

/// `DartForge_porta_mantem_vivo(id)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_porta_mantem_vivo(id: i64) -> u8 {
    PORTAS_ABERTAS.with(|p| u8::from(p.borrow().get(&id).copied().unwrap_or(false)))
}

/// `DartForge_porta_enviar(id, mensagem)`: copia e posta. Devolve 1 se a
/// mensagem tem objeto não enviável (a descrição fica em
/// `DartForge_porta_recusa`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_porta_enviar(id: i64, mensagem: i64) -> u8 {
    let mesmo = {
        let r = registro().lock().unwrap_or_else(|e| e.into_inner());
        let fila = FILA.with(|f| f.id);
        matches!(r.get(&id), Some(Dono::Isolado(f)) if f.id == fila)
    };
    match copiar_para_grafo(mensagem, mesmo) {
        Ok(g) => {
            postar(id, g);
            0
        }
        Err(MensagemIlegal(d)) => {
            ULTIMA_RECUSA.with(|u| *u.borrow_mut() = d);
            1
        }
    }
}

/// `DartForge_porta_recusa()`: por que a última mensagem foi recusada.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_porta_recusa() -> i64 {
    let d = ULTIMA_RECUSA.with(|u| u.borrow().clone());
    HEAP.with(|h| h.borrow_mut().alocar_str(&d))
}

/// `DartForge_portas_despachante(f)`: a closure sem argumentos que o laço
/// chama para cada mensagem (`_RawReceivePort._despachar`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_portas_despachante(f: i64) {
    HEAP.with(|h| h.borrow_mut().set_global_root(RAIZ_DESPACHANTE, f));
    DESPACHANTE.with(|d| *d.borrow_mut() = f);
}

/// `DartForge_porta_atual()`: a porta da mensagem em despacho.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_porta_atual() -> i64 {
    ATUAL.with(|a| a.borrow().0)
}

/// `DartForge_mensagem_atual()`: a mensagem em despacho.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_mensagem_atual() -> i64 {
    ATUAL.with(|a| a.borrow().1)
}

/// `DartForge_classe_nao_enviavel(objeto)`: a classe de `objeto` não vai
/// numa mensagem (`@pragma('vm:isolate-unsendable')` da VM). A descrição é a
/// da VM sem o sufixo de biblioteca privada e sem o caminho de retenção.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_classe_nao_enviavel(objeto: i64) {
    let classe = HEAP.with(|h| h.borrow().objeto(objeto).map(|o| o.class_id));
    let Some(c) = classe else { return };
    let nome = nome_da_classe(c);
    NAO_ENVIAVEIS.with(|n| {
        n.borrow_mut().entry(c).or_insert_with(|| {
            format!(
                "object is unsendable - Library:'dart:isolate' Class: {nome} (see restrictions listed at `SendPort.send()` documentation for more information)\n"
            )
        });
    });
}

/// `DartForge_classe_transferivel(objeto)`: a classe de `objeto` tem o
/// primeiro campo movido numa mensagem (`TransferableTypedData`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_classe_transferivel(objeto: i64) {
    let classe = HEAP.with(|h| h.borrow().objeto(objeto).map(|o| o.class_id));
    if let Some(c) = classe
        && !classes_transferiveis().contains(&c)
    {
        transferiveis().write().unwrap_or_else(|e| e.into_inner()).insert(c);
    }
}

/// `DartForge_capacidade_nova()`: um id de capacidade único no processo.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_capacidade_nova() -> i64 {
    static PROX: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);
    PROX.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// O nome registrado da classe `c` (vazio se não houver).
fn nome_da_classe(c: i64) -> String {
    CLASS_NAMES.with(|m| m.borrow().get(&c).cloned()).unwrap_or_default()
}
