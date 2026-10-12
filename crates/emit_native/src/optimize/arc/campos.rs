//! Acessos físicos a campos por origem estática ou guarda de layout da imagem.
//! Alocações HIR completas são as âncoras; cópias/moves/Phis transportam
//! conjuntos de sítios, sem forte atualização por instância. Publicação,
//! uso opaco ou contenção invalidam o sítio inteiro na prova global. IDs
//! recém-produzidos no mesmo bloco mantêm a prova até a primeira exposição.
//! Leituras Ref sem origem estática usam a conferência do mapa real do heap;
//! o plano de tokens protege o empréstimo antes de invalidar a aresta.
//! Não substitui guardas Dart late/tipo, resumos de heap ou versões/pins.

use super::{PlanoFuncaoDart, arestas::proximo_valor};
use crate::{hir::*, optimize::operandos::operandos};
use std::collections::{HashMap, HashSet};

// Transporta elegibilidade para a guarda física, sem provar ownership/layout.
// Cada SCC exige âncora Ref/Null e todas as dependências externas conferidas.
fn propagar_entradas_ref(f: &Function, entradas: &mut HashSet<ValueId>) {
    let nos: Vec<_> = f.blocks.iter().flat_map(|b| &b.instructions).filter_map(|(v, i, t)| {
        if *t != Type::Ref { return None; }
        let ops: Vec<_> = match i {
            Instruction::ArcCopy { value } | Instruction::ArcMove { value } => vec![value],
            Instruction::Phi { incoming, .. } if !incoming.is_empty() => incoming.iter().map(|(_, op)| op).collect(),
            _ => return None,
        };
        Some((*v, ops))
    }).collect();
    let indice: HashMap<_, _> = nos.iter().enumerate().map(|(i, (v, _))| (*v, i)).collect();
    let mut grafo = vec![Vec::new(); nos.len()];
    let mut ancoras = vec![false; nos.len()];
    let mut invalidos = vec![false; nos.len()];
    for (i, (_, ops)) in nos.iter().enumerate() {
        for op in ops {
            match op {
                Operand::Constant(Constant::Null) => ancoras[i] = true,
                Operand::Val(v) if entradas.contains(v) => ancoras[i] = true,
                Operand::Val(v) if indice.contains_key(v) => grafo[i].push(indice[v]),
                _ => invalidos[i] = true,
            }
        }
    }
    let sccs = super::super::scc::componentes(&grafo);
    let mut componente = vec![usize::MAX; nos.len()];
    for (i, scc) in sccs.iter().enumerate() { for &v in scc { componente[v] = i; } }
    let mut validos = vec![false; sccs.len()];
    for (i, scc) in sccs.iter().enumerate() {
        let mut ancora = false; let mut valido = true;
        for &v in scc {
            ancora |= ancoras[v]; valido &= !invalidos[v];
            for &dep in &grafo[v] {
                if componente[dep] != i { valido &= validos[componente[dep]]; ancora |= validos[componente[dep]]; }
            }
        }
        validos[i] = valido && ancora;
        if validos[i] { for &v in scc { entradas.insert(nos[v].0); } }
    }
}

#[derive(Clone, Default, PartialEq, Eq)]
struct Origem {
    sitios: HashSet<ValueId>,
    desconhecido: bool,
}

fn origem(op: &Operand, origens: &HashMap<ValueId, Origem>) -> Origem {
    match op {
        Operand::Val(v) => origens.get(v).cloned().unwrap_or(Origem {
            desconhecido: true,
            ..Default::default()
        }),
        _ => Origem {
            desconhecido: true,
            ..Default::default()
        },
    }
}

fn origens(f: &Function, layouts: &HashMap<u32, Vec<Type>>, receivers: Option<&HashMap<ValueId, u32>>) -> (HashMap<ValueId, Origem>, HashMap<ValueId, u32>) {
    let mut mapa = HashMap::new();
    let mut classes = HashMap::new();
    for (v, _, _) in &f.params {
        mapa.insert(
            *v,
            Origem {
                desconhecido: true,
                ..Default::default()
            },
        );
    }
    if let Some(receivers) = receivers {
        for (v, cid) in receivers {
            classes.insert(*v, *cid);
            mapa.insert(*v, Origem { sitios: HashSet::from([*v]), desconhecido: false });
        }
    }
    for (v, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
        let o = match inst {
            Instruction::AllocObject { class_id, .. } => {
                classes.insert(*v, *class_id);
                Origem {
                    sitios: HashSet::from([*v]),
                    desconhecido: false,
                }
            }
            Instruction::CallRuntime { name, args, ret_ty }
                if matches!(name.as_str(), "dartforge_object_new" | "dartforge_arc_objeto_owned_v1")
                    && *ret_ty == Type::Ref => {
                // A fábrica zerada também fixa a classe física. Não inferir
                // tamanho/layout pela largura nem por parâmetros não constantes.
                let classe = match args.as_slice() {
                    [(Operand::Constant(Constant::Int(cid)), Type::I64),
                     (Operand::Constant(Constant::Int(n)), Type::I64)] => {
                        u32::try_from(*cid).ok().filter(|cid| i32::try_from(*cid).is_ok())
                            .filter(|cid| usize::try_from(*n).ok().is_some_and(|n|
                                u16::try_from(n).is_ok() && layouts.get(cid).is_some_and(|l| l.len() == n)))
                    }
                    _ => None,
                };
                if let Some(cid) = classe {
                    classes.insert(*v, cid);
                    Origem { sitios: HashSet::from([*v]), desconhecido: false }
                } else {
                    Origem { desconhecido: true, ..Default::default() }
                }
            }
            Instruction::ArcCopy { .. } | Instruction::ArcMove { .. } | Instruction::Phi { .. } => {
                Origem::default()
            }
            _ => Origem {
                desconhecido: true,
                ..Default::default()
            },
        };
        mapa.insert(*v, o);
    }
    loop {
        let mut mudou = false;
        for (v, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            let ops = match inst {
                Instruction::ArcCopy { value } | Instruction::ArcMove { value } => vec![value],
                Instruction::Phi { incoming, .. } => incoming.iter().map(|(_, o)| o).collect(),
                _ => continue,
            };
            let mut novo = Origem::default();
            for op in ops {
                let o = origem(op, &mapa);
                novo.sitios.extend(o.sitios);
                novo.desconhecido |= o.desconhecido;
            }
            if mapa[v] != novo {
                mapa.insert(*v, novo);
                mudou = true;
            }
        }
        if !mudou {
            break;
        }
    }
    (mapa, classes)
}

fn representacao(
    receiver: &Operand,
    index: usize,
    origens: &HashMap<ValueId, Origem>,
    classes: &HashMap<ValueId, u32>,
    layouts: &HashMap<u32, Vec<Type>>,
    escapam: &HashSet<ValueId>,
) -> Result<Type, String> {
    let o = origem(receiver, origens);
    if o.desconhecido || o.sitios.is_empty() || !o.sitios.is_disjoint(escapam) {
        return Err("receiver sem origem local não publicada".into());
    }
    representacao_conhecida(&o, index, classes, layouts)
}

// A parte desconhecida da junção exige guarda física. A parte nominal
// continua obrigatória: não esconder um layout contraditório atrás da guarda.
fn representacao_conhecida(
    o: &Origem, index: usize, classes: &HashMap<ValueId, u32>,
    layouts: &HashMap<u32, Vec<Type>>,
) -> Result<Type, String> {
    let mut repr = None;
    for sitio in &o.sitios {
        let t = layouts
            .get(&classes[sitio])
            .and_then(|l| l.get(index))
            .ok_or("classe/layout/índice de campo não certificado")?;
        if repr.is_some_and(|anterior| anterior != *t) {
            return Err("representações diferentes na junção de receivers".into());
        }
        repr = Some(*t);
    }
    repr.ok_or_else(|| "receiver sem origem nominal".into())
}

const GRAVAR_ESCALAR: &str = "dartforge_arc_gravar_campo_escalar_conferido_v1";
const LER_ESCALAR: &str = "dartforge_arc_ler_campo_escalar_conferido_v1";
const GRAVAR_CONFERIDO: &str = "dartforge_arc_gravar_campo_ref_conferido_v1";

fn layouts_do_campo(layouts: &HashMap<u32, Vec<Type>>, indice: usize, repr: Type) -> Vec<i64> {
    let mut pares: Vec<_> = layouts.iter().filter(|(cid, campos)|
        i32::try_from(**cid).is_ok() && u16::try_from(campos.len()).is_ok()
        && campos.get(indice) == Some(&repr)).collect();
    pares.sort_by_key(|(cid, _)| **cid);
    pares.into_iter().flat_map(|(cid, campos)| [i64::from(*cid), campos.len() as i64]).collect()
}

fn codigo_escalar(repr: Type) -> i64 {
    match repr { Type::I64 => 1, Type::F64 => 2, Type::I1 => 3, Type::I8 => 4,
        _ => unreachable!("representação escalar não conferida") }
}

// Um Ptr importado, alias ou tamanho calculado não prova extensão/imutabilidade.
// A imagem certificada também reconfere o vetor contra os layouts atuais.
pub(super) fn conferir_chamada(
    f: &Function, inst: &Instruction, layouts: Option<&HashMap<u32, Vec<Type>>>,
) -> Result<(), String> {
    let Instruction::CallRuntime { name, args, ret_ty } = inst else { return Ok(()); };
    if !matches!(name.as_str(), GRAVAR_CONFERIDO | GRAVAR_ESCALAR | LER_ESCALAR) { return Ok(()); }
    let erro = || format!("campo conferido em {}: vetor de layouts sem proveniência/limites", f.symbol);
    let (indice, vetor, pares, repr) = match (name.as_str(), args.as_slice(), ret_ty) {
        (GRAVAR_CONFERIDO, [(_, Type::Ref), (Operand::Constant(Constant::Int(i)), Type::I64),
            (_, Type::Ref), (Operand::Val(v), Type::Ptr), (Operand::Constant(Constant::Int(n)), Type::I64)], Type::Void) => (*i, *v, *n, Type::Ref),
        (GRAVAR_ESCALAR, [(_, Type::Ref), (Operand::Constant(Constant::Int(i)), Type::I64),
            (_, Type::I64), (Operand::Val(v), Type::Ptr), (Operand::Constant(Constant::Int(n)), Type::I64),
            (Operand::Constant(Constant::Int(r)), Type::I64)], Type::Void)
        | (LER_ESCALAR, [(_, Type::Ref), (Operand::Constant(Constant::Int(i)), Type::I64),
            (Operand::Val(v), Type::Ptr), (Operand::Constant(Constant::Int(n)), Type::I64),
            (Operand::Constant(Constant::Int(r)), Type::I64)], Type::I64) => {
                let repr = match r { 1 => Type::I64, 2 => Type::F64, 3 => Type::I1, 4 => Type::I8, _ => return Err(erro()) };
                (*i, *v, *n, repr)
            }
        _ => return Err(erro()),
    };
    let indice = usize::try_from(indice).map_err(|_| erro())?;
    let Some((_, Instruction::ConstArray(valores), Type::Ptr)) = f.blocks.iter()
        .flat_map(|b| &b.instructions).find(|(v, _, _)| *v == vetor) else { return Err(erro()); };
    if valores.is_empty() || !valores.len().is_multiple_of(2)
        || usize::try_from(pares).ok() != Some(valores.len() / 2)
        || valores.chunks_exact(2).any(|p| i32::try_from(p[0]).is_err() || p[0] < 0
            || u16::try_from(p[1]).is_err() || usize::try_from(p[1]).ok().is_none_or(|n| indice >= n))
        || valores.chunks_exact(2).map(|p| p[0]).collect::<Vec<_>>().windows(2).any(|p| p[0] >= p[1])
    { return Err(erro()); }
    if let Some(layouts) = layouts && *valores != layouts_do_campo(layouts, indice, repr) {
        return Err(format!("campo conferido em {}: layouts obsoletos/divergentes", f.symbol));
    }
    Ok(())
}

pub(super) fn preparar(
    f: &mut Function,
    layouts: &HashMap<u32, Vec<Type>>,
    plano: &mut PlanoFuncaoDart,
    receivers: Option<&HashMap<ValueId, u32>>,
    preservam_campos: &HashSet<String>,
) -> Result<(), String> {
    // Const Null em SSA não tem token. Na entrada de Phi, usa a forma
    // literal para o protocolo nullable, sem confundir Const Int(0) ou
    // Bool(false). A definição permanece e seu contrato ainda é conferido.
    let nulos: HashSet<_> = f.blocks.iter().flat_map(|b| &b.instructions)
        .filter_map(|(v, i, t)| (*t == Type::Ref && matches!(i, Instruction::Const(Constant::Null))).then_some(*v)).collect();
    for (_, i, _) in f.blocks.iter_mut().flat_map(|b| &mut b.instructions) {
        if let Instruction::Phi { incoming, ty: Type::Ref } = i {
            for (_, op) in incoming {
                if matches!(op, Operand::Val(v) if nulos.contains(v)) { *op = Operand::Constant(Constant::Null); }
            }
        }
    }
    for (_, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
        conferir_chamada(f, inst, Some(layouts))?;
    }
    if !f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .any(|(_, i, _)| {
            matches!(
                i,
                Instruction::GetField { .. } | Instruction::SetField { .. }
            )
        })
    {
        return Ok(());
    }
    let (origens, classes) = origens(f, layouts, receivers);
    // Entradas emprestadas já devem ser instâncias válidas pela semântica
    // Dart. Uma fábrica local sem layout pode ainda conter campos zerados
    // sem marca Ref; não a confundir com um receiver de entrada aberto.
    // Estes valores podem ser receivers conferidos, sem alegar classe ou
    // frescor. Cada leitura e chamada ainda precisa passar pela preparação
    // e pelo contrato de ownership; uma falha descarta o módulo inteiro.
    let mut entradas_ref: HashSet<_> = f.params.iter()
        .filter(|(_, _, ty)| *ty == Type::Ref).map(|(v, _, _)| *v).collect();
    for (v, inst, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
        if *ty != Type::Ref { continue; }
        // Null pode acompanhar uma Ref válida na junção, inclusive no
        // caminho pending. A guarda de acesso continua recusando null;
        // isto não atribui classe nem transforma bits escalares em Ref.
        let derivado = match inst {
            Instruction::ArcLoadStrong { .. } => true,
            Instruction::CallRuntime { name, ret_ty: Type::Ref, .. } if matches!(name.as_str(), super::vetores::LER_ARGUMENTO | "dartforge_arc_lista_ref_owned_v1") => true,
            Instruction::AllocObject { .. } => true,
            Instruction::CallRuntime { name, ret_ty: Type::Ref, .. }
                if matches!(name.as_str(), "dartforge_object_new" | "dartforge_arc_objeto_owned_v1") => classes.contains_key(v),
            // O catálogo confere resultado Ref, aridade e tipos declarados.
            // Isto só autoriza a guarda física; a transação ainda reconfere
            // tipos SSA, proveniência nativa, tokens e fronteiras Dart. Uma
            // extern opaca ou um resultado escalar não se tornam referências.
            Instruction::CallRuntime { ret_ty: Type::Ref, .. } =>
                super::contratos::contrato_chamada_runtime(inst).is_ok(),
            Instruction::Box { from: Type::I64 | Type::F64 | Type::I1 | Type::I8, .. }
            | Instruction::GetField { .. }
            | Instruction::EnvGet { .. }
            | Instruction::CellGet { .. }
            | Instruction::Const(Constant::Null)
            | Instruction::CallStatic { ret_ty: Type::Ref, .. }
            | Instruction::CallSeletor { .. }
            | Instruction::CallInterface { ret_ty: Type::Ref, .. }
            | Instruction::CallDynamic { ret_ty: Type::Ref, .. }
            | Instruction::CallClosure { ret_ty: Type::Ref, .. } => true,
            Instruction::ChamadaTipada { ret: Type::Ref, .. } => super::tipadas::conferir(f, inst, *ty)?,
            _ => false,
        };
        if derivado { entradas_ref.insert(*v); }
    }
    propagar_entradas_ref(f, &mut entradas_ref);
    let mut escapam = HashSet::new();
    let mut exposicoes = HashMap::new();
    for (v, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
        let mut expostos = HashSet::new();
        let mut expor = |op: &Operand| {
            expostos.extend(origem(op, &origens).sitios);
        };
        match inst {
            Instruction::GetField { .. }
            | Instruction::ArcCopy { .. }
            | Instruction::ArcMove { .. }
            | Instruction::ArcDrop { .. }
            | Instruction::Phi { .. } => {}
            Instruction::SetField { value, .. } => expor(value),
            Instruction::AllocObject { fields, .. } => fields.iter().for_each(&mut expor),
            Instruction::CallRuntime { name, args, .. } => {
                let seguro = matches!(
                    name.as_str(),
                    "dartforge_arc_retain"
                        | "dartforge_object_new"
                        | "dartforge_arc_release"
                        | "dartforge_arc_objeto_owned_v1"
                        | "dartforge_arc_box_int_owned_v1"
                        | "dartforge_arc_box_double_owned_v1"
                        | "dartforge_exception_pending"
                        | "dartforge_rti_receita"
                        | "dartforge_rti_avaliar"
                );
                let campo = match name.as_str() {
                    "dartforge_arc_ler_campo_ref_v1" => Some(true),
                    "dartforge_arc_ler_campo_escalar_v1" => Some(false),
                    _ => None,
                };
                let mut seguro_campo = false;
                if let Some(refs) = campo
                    && let [receiver, indice] = args.as_slice()
                    && let Operand::Constant(Constant::Int(n)) = indice.0
                    && let Ok(index) = usize::try_from(n)
                    && let Ok(repr) = representacao(
                        &receiver.0,
                        index,
                        &origens,
                        &classes,
                        layouts,
                        &HashSet::new(),
                    )
                {
                    seguro_campo = (repr == Type::Ref) == refs;
                }
                // Definir RTI pode compactar listas. Só a origem física de
                // instâncias não-lista fecha a obrigação de layout aqui.
                let definir_instancia = name == "dartforge_rti_definir" && args.len() == 2 && {
                    let o = origem(&args[0].0, &origens);
                    !o.desconhecido && !o.sitios.is_empty() && o.sitios.iter().all(|s|
                        i32::try_from(classes[s]).ok().is_some_and(|cid|
                            !dartforge_runtime::layout::cid::e_lista(cid)))
                };
                if !seguro && !seguro_campo && !definir_instancia {
                    // Setter de bits nativo não certifica a representação
                    // lógica (bool/byte). Só SetField tipado mantém a prova.
                    args.iter().for_each(|(o, _)| expor(o));
                    // Ausência de Dart no catálogo de ownership não prova
                    // ausência de mutação do layout, por exemplo na recarga.
                    // Só operações enumeradas acima têm contrato fechado aqui.
                    expostos.extend(classes.keys().copied());
                }
            }
            Instruction::CallStatic { symbol, args, .. } if preservam_campos.contains(symbol) && args.is_empty() => {}
            Instruction::CallStatic { .. }
            | Instruction::CallInterface { .. }
            | Instruction::CallClosure { .. }
            | Instruction::CallClosureRepasse { .. }
            | Instruction::ChamadaTipada { .. } => expostos.extend(classes.keys().copied()),
            _ => operandos(inst, &mut expor),
        }
        escapam.extend(&expostos);
        exposicoes.insert(*v, expostos);
    }
    let tipos: HashMap<_, _> = f
        .params
        .iter()
        .map(|(v, _, t)| (*v, *t))
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| b.instructions.iter().map(|(v, _, t)| (*v, *t))),
        )
        .collect();
    let tipo = |op: &Operand| match op {
        Operand::Val(v) => tipos.get(v).copied(),
        Operand::Constant(Constant::Int(_)) => Some(Type::I64),
        Operand::Constant(Constant::Double(_)) => Some(Type::F64),
        Operand::Constant(Constant::Bool(_)) => Some(Type::I1),
        Operand::Constant(Constant::Null) => Some(Type::Ref),
        _ => None,
    };
    let parametros: HashSet<ValueId> = receivers.into_iter().flat_map(|m| m.keys().copied()).collect();
    let entrada = f.blocks.first().map(|b| b.id);
    let entrada_sem_volta = !f.blocks.iter().any(|b| {
        super::super::cfg::sucessores(&b.terminator).into_iter().any(|destino| Some(destino) == entrada)
    });
    // Para parâmetros certificados, usa somente o prefixo de entrada que
    // domina a primeira chamada/publicação. Não atribui frescor ao parâmetro,
    // nem reutiliza essa prova em blocos posteriores ou numa volta de laço.
    let globais_locais: HashSet<_> = escapam.difference(&parametros).copied().collect();
    let mut proximo = proximo_valor(f, plano);
    for b in &mut f.blocks {
        let mut invalidos = if Some(b.id) == entrada && entrada_sem_volta {
            HashSet::new()
        } else { parametros.clone() };
        // IDs efetivamente produzidos neste bloco, não sítios abstratos de
        // Phis que podem representar objetos de uma iteração anterior.
        let mut frescos: HashSet<ValueId> = HashSet::new();
        let mut instructions = Vec::new();
        for (v, inst, ty) in &b.instructions {
            let (object, index, value) = match inst {
                Instruction::GetField { object, index } => (object, *index, None),
                Instruction::SetField {
                    object,
                    index,
                    value,
                } => (object, *index, Some(value)),
                _ => {
                    frescos.retain(|id| origem(&Operand::Val(*id), &origens).sitios.is_disjoint(&exposicoes[v]));
                    if classes.contains_key(v) && !parametros.contains(v) {
                        frescos.insert(*v);
                    } else if let Instruction::ArcCopy { value } | Instruction::ArcMove { value } = inst
                        && matches!(value, Operand::Val(de) if frescos.contains(de)) {
                        frescos.insert(*v);
                    }
                    invalidos.extend(exposicoes[v].intersection(&parametros).copied());
                    instructions.push((*v, inst.clone(), *ty));
                    continue;
                }
            };
            let atuais: HashSet<_> = if matches!(object, Operand::Val(id) if frescos.contains(id)) {
                // Uma publicação futura não pode afetar a escrita anterior.
                // Não reviver a prova após uma exposição já executada.
                HashSet::new()
            } else { globais_locais.union(&invalidos).copied().collect() };
            let o = origem(object, &origens);
            // Sem âncora nominal, o accessor Ref confere receiver, índice e
            // bitmap antes de emprestar. Não reinterpretar bits escalares,
            // ou ignorar contradições de uma origem conhecida. Escritas abertas
            // exigem a conferência adicional dos layouts declarados da imagem.
            let entrada_aberta = o.desconhecido && o.sitios.is_empty()
                && matches!(object, Operand::Val(v) if entradas_ref.contains(v));
            // A exposição perde a prova estática, não o owner. Conferir a
            // representação nominal antes de optar pela guarda física evita
            // aceitar contradições de classe, tamanho ou tipo na própria HIR.
            let nominal_exposto = !o.desconhecido && !o.sitios.is_empty()
                && !o.sitios.is_disjoint(&atuais);
            let nominal_misto = o.desconhecido && !o.sitios.is_empty()
                && matches!(object, Operand::Val(v) if entradas_ref.contains(v));
            let nominal = if nominal_exposto || nominal_misto {
                Some(representacao_conhecida(&o, index, &classes, layouts)
                    .map_err(|e| format!("campo ARC em {} v{}: {e}", f.symbol, v.0))?)
            } else { None };
            let receiver_conferido = entrada_aberta || nominal_exposto || nominal_misto;
            let leitura_ref_conferida = value.is_none() && *ty == Type::Ref && receiver_conferido;
            let gravacao_ref_conferida = value.is_some_and(|valor| tipo(valor) == Some(Type::Ref))
                && *ty == Type::Void && receiver_conferido;
            let escalar = if let Some(value) = value { tipo(value) } else { Some(*ty) };
            let escalar_conferido = receiver_conferido && matches!(escalar, Some(Type::I64 | Type::F64 | Type::I1 | Type::I8));
            let repr = if let Some(nominal) = nominal { nominal }
            else if leitura_ref_conferida || gravacao_ref_conferida {
                Type::Ref
            } else if escalar_conferido {
                escalar.unwrap()
            } else {
                representacao(object, index, &origens, &classes, layouts, &atuais)
                    .map_err(|e| format!("campo ARC em {} v{}: {e}", f.symbol, v.0))?
            };
            let indice = i64::try_from(index)
                .map_err(|_| format!("campo ARC em {} v{}: índice fora da ABI", f.symbol, v.0))?;
            if tipo(object) != Some(Type::Ref)
                || (value.is_none() && *ty != repr)
                || value.is_some_and(|o| tipo(o) != Some(repr) || *ty != Type::Void)
                || plano.tokens.pendencias.contains_key(v)
                || plano.tabelas.invocacoes.contains_key(v)
            {
                return Err(format!(
                    "campo ARC em {} v{}: tipo ou guarda incompatível",
                    f.symbol, v.0
                ));
            }
            frescos.retain(|id| origem(&Operand::Val(*id), &origens).sitios.is_disjoint(&exposicoes[v]));
            invalidos.extend(exposicoes[v].intersection(&parametros).copied());
            let estreito = !matches!(repr, Type::I64 | Type::Ref);
            let auxiliar = if estreito {
                let a = ValueId(
                    u32::try_from(proximo)
                        .map_err(|_| "IDs SSA esgotados ao preparar campos ARC")?,
                );
                proximo += 1;
                if let Some(limites) = plano.escopos.antes.remove(v) {
                    plano.escopos.antes.insert(a, limites);
                }
                Some(a)
            } else {
                None
            };
            let mut args = vec![
                (object.clone(), Type::Ref),
                (Operand::Constant(Constant::Int(indice)), Type::I64),
            ];
            if let Some(value) = value {
                let bits = if let Some(a) = auxiliar {
                    let conversao = if repr == Type::F64 {
                        Instruction::Bitcast {
                            op: value.clone(),
                            to: Type::I64,
                        }
                    } else {
                        Instruction::ZExt {
                            op: value.clone(),
                            from: repr,
                            to: Type::I64,
                        }
                    };
                    instructions.push((a, conversao, Type::I64));
                    Operand::Val(a)
                } else {
                    value.clone()
                };
                args.push((
                    bits,
                    if repr == Type::Ref {
                        Type::Ref
                    } else {
                        Type::I64
                    },
                ));
                if gravacao_ref_conferida || escalar_conferido {
                    let valores = layouts_do_campo(layouts, index, repr);
                    if valores.is_empty() {
                        return Err(format!("campo ARC em {} v{}: nenhum layout correspondente declarado", f.symbol, v.0));
                    }
                    let vetor = ValueId(u32::try_from(proximo).map_err(|_| "IDs SSA esgotados nos layouts Ref")?);
                    proximo += 1;
                    args.push((Operand::Val(vetor), Type::Ptr));
                    args.push((Operand::Constant(Constant::Int((valores.len() / 2) as i64)), Type::I64));
                    if let Some(limites) = plano.escopos.antes.remove(v) {
                        plano.escopos.antes.insert(vetor, limites);
                    }
                    instructions.push((vetor, Instruction::ConstArray(valores), Type::Ptr));
                    if escalar_conferido {
                        args.push((Operand::Constant(Constant::Int(codigo_escalar(repr))), Type::I64));
                    }
                }
                instructions.push((
                    *v,
                    Instruction::CallRuntime {
                        name: if escalar_conferido {
                            GRAVAR_ESCALAR
                        } else if gravacao_ref_conferida {
                            GRAVAR_CONFERIDO
                        } else if repr == Type::Ref {
                            "dartforge_arc_gravar_campo_ref_v1"
                        } else {
                            "dartforge_arc_gravar_campo_escalar_v1"
                        }
                        .into(),
                        args,
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ));
            } else {
                if escalar_conferido {
                    let valores = layouts_do_campo(layouts, index, repr);
                    if valores.is_empty() { return Err(format!("campo ARC em {} v{}: nenhum layout correspondente declarado", f.symbol, v.0)); }
                    let vetor = ValueId(u32::try_from(proximo).map_err(|_| "IDs SSA esgotados nos layouts escalares")?);
                    proximo += 1;
                    args.push((Operand::Val(vetor), Type::Ptr));
                    args.push((Operand::Constant(Constant::Int((valores.len() / 2) as i64)), Type::I64));
                    args.push((Operand::Constant(Constant::Int(codigo_escalar(repr))), Type::I64));
                    instructions.push((vetor, Instruction::ConstArray(valores), Type::Ptr));
                }
                let ret_ty = if repr == Type::Ref {
                    Type::Ref
                } else {
                    Type::I64
                };
                instructions.push((
                    auxiliar.unwrap_or(*v),
                    Instruction::CallRuntime {
                        name: if escalar_conferido {
                            LER_ESCALAR
                        } else if repr == Type::Ref {
                            "dartforge_arc_ler_campo_ref_v1"
                        } else {
                            "dartforge_arc_ler_campo_escalar_v1"
                        }
                        .into(),
                        args,
                        ret_ty,
                    },
                    ret_ty,
                ));
                if let Some(a) = auxiliar {
                    let conversao = if repr == Type::F64 {
                        Instruction::Bitcast {
                            op: Operand::Val(a),
                            to: repr,
                        }
                    } else {
                        Instruction::Trunc {
                            op: Operand::Val(a),
                            from: Type::I64,
                            to: repr,
                        }
                    };
                    instructions.push((*v, conversao, repr));
                }
            }
        }
        b.instructions = instructions;
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use crate::optimize::arc::*;

    #[test]
    fn entradas_ref_em_scc_exigem_ancora_e_todas_as_dependencias_validas() {
        for caso in 0..6 {
            let val = |n| Operand::Val(ValueId(n));
            let ancora = match caso { 1 => Operand::Constant(Constant::Null), 2 => Operand::Constant(Constant::Int(0)),
                3 => val(99), _ => val(0) };
            let mut incoming = vec![(BlockId(0), val(4))];
            if caso != 4 { incoming.push((BlockId(0), ancora)); }
            if caso == 5 { incoming.push((BlockId(0), val(99))); }
            let f = Function { symbol: "grafo".into(), name: "grafo".into(), depuracao: None,
                params: vec![(ValueId(0), "ref".into(), Type::Ref)], return_ty: Type::Void,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                    (ValueId(3), Instruction::Phi { ty: Type::Ref, incoming }, Type::Ref),
                    (ValueId(4), Instruction::Phi { ty: Type::Ref, incoming: vec![(BlockId(0), val(3))] }, Type::Ref),
                    (ValueId(5), Instruction::Phi { ty: Type::Ref, incoming: vec![(BlockId(0), val(6))] }, Type::Ref),
                    (ValueId(6), Instruction::Phi { ty: Type::Ref, incoming: vec![(BlockId(0), val(5)), (BlockId(0), val(3)), (BlockId(0), val(0))] }, Type::Ref),
                    (ValueId(7), Instruction::ArcCopy { value: val(6) }, Type::Ref),
                ], terminator: Terminator::Return(None) }] };
            let mut entradas = HashSet::from([ValueId(0)]);
            super::propagar_entradas_ref(&f, &mut entradas);
            for id in 3..=7 { assert_eq!(entradas.contains(&ValueId(id)), caso < 2, "caso {caso} v{id}"); }
        }
    }

    #[test]
    fn phi_ref_com_null_nao_admite_zero_escalar_como_receiver() {
        for (constante, valido) in [(Constant::Null, true), (Constant::Int(0), false), (Constant::Bool(false), false)] {
            let (mut m, mut planos) = modulo(vec![Type::I1], vec![Operand::Constant(Constant::Bool(true))]);
            let f = &mut m.functions[0];
            f.params = vec![(ValueId(0), "receiver".into(), Type::Ref), (ValueId(7), "cond".into(), Type::I1)];
            f.return_ty = Type::I1;
            f.blocks = vec![
                BasicBlock { id: BlockId(0), instructions: vec![], terminator: Terminator::CondBranch {
                    cond: Operand::Val(ValueId(7)), then_block: BlockId(1), else_block: BlockId(2) } },
                BasicBlock { id: BlockId(1), instructions: vec![
                    (ValueId(8), Instruction::ArcCopy { value: Operand::Val(ValueId(0)) }, Type::Ref),
                ], terminator: Terminator::Branch(BlockId(3)) },
                BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Branch(BlockId(3)) },
                BasicBlock { id: BlockId(3), instructions: vec![
                    (ValueId(9), Instruction::Phi { ty: Type::Ref, incoming: vec![
                        (BlockId(1), Operand::Val(ValueId(8))), (BlockId(2), Operand::Constant(constante.clone())),
                    ] }, Type::Ref),
                    (ValueId(10), Instruction::GetField { object: Operand::Val(ValueId(9)), index: 0 }, Type::I1),
                ], terminator: Terminator::Return(Some(Operand::Val(ValueId(10)))) },
            ];
            // A forma SSA também aparece no lowering de operadores e casts.
            f.blocks[2].instructions.push((ValueId(11), Instruction::Const(constante.clone()),
                if valido { Type::Ref } else { Type::I64 }));
            if let Instruction::Phi { incoming, .. } = &mut f.blocks[3].instructions[0].1 {
                incoming[1].1 = Operand::Val(ValueId(11));
            }
            let antes = format!("{m:?}/{planos:?}");
            let resultado = preparar_arc_modulo_dart(&mut m, &mut planos);
            assert_eq!(resultado.is_ok(), valido, "{resultado:?}");
            if valido {
                assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(), (0, 0));
            } else { assert_eq!(antes, format!("{m:?}/{planos:?}")); }
        }
    }

    fn modulo(
        layout: Vec<Type>,
        fields: Vec<Operand>,
    ) -> (Module, HashMap<String, PlanoFuncaoDart>) {
        let mut m = Module::new();
        m.memoria_arc = true;
        m.classes.push(ClassDef {
            id: 123,
            name: "C".into(),
            field_count: fields.len(),
            vtable: vec![],
            to_string_symbol: None,
        });
        m.layouts_campos_arc.insert(123, layout);
        m.functions.push(Function {
            symbol: "campos".into(),
            name: "campos".into(),
            depuracao: None,
            params: vec![],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(0),
                    Instruction::AllocObject {
                        class_id: 123,
                        fields,
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(None),
            }],
        });
        (
            m,
            HashMap::from([("campos".into(), PlanoFuncaoDart::default())]),
        )
    }

    #[test]
    fn ref_runtime_auditada_confere_campos_sem_promover_extern_opaca_ou_fabrica_sem_layout() {
        for caso in 0..9 {
            let (mut m, mut planos) = modulo(vec![Type::I64], vec![Operand::Constant(Constant::Int(0))]);
            let f = &mut m.functions[0];
            f.params = vec![(ValueId(0), "closure".into(), Type::Ref)];
            let (nome, args) = match caso {
                0 => ("dartforge_arc_contexto_closure_v1", vec![(Operand::Val(ValueId(0)), Type::Ref)]),
                1 => ("dartforge_arc_ler_campo_ref_v1", vec![(Operand::Val(ValueId(0)), Type::Ref), (Operand::Constant(Constant::Int(0)), Type::I64)]),
                2 => ("opaca", vec![(Operand::Val(ValueId(0)), Type::Ref)]),
                3 => ("dartforge_arc_contexto_closure_v1", vec![(Operand::Val(ValueId(0)), Type::I64)]),
                4 => ("dartforge_arc_objeto_owned_v1", vec![(Operand::Constant(Constant::Int(999)), Type::I64), (Operand::Constant(Constant::Int(1)), Type::I64)]),
                5 => ("dartforge_arc_ler_campo_escalar_v1", vec![(Operand::Val(ValueId(0)), Type::Ref), (Operand::Constant(Constant::Int(0)), Type::I64)]),
                6 => ("dartforge_arc_excecao_owned_v1", vec![]),
                8 => ("dartforge_arc_ler_campo_late_ref_v1", vec![(Operand::Val(ValueId(0)), Type::Ref), (Operand::Constant(Constant::Int(0)), Type::I64)]),
                _ => ("dartforge_arc_contexto_closure_v1", vec![(Operand::Val(ValueId(0)), Type::Ref)]),
            };
            if caso == 7 { f.params[0].2 = Type::I64; }
            f.blocks[0].instructions = vec![
                (ValueId(1), Instruction::CallRuntime { name: nome.into(), args, ret_ty: Type::Ref }, Type::Ref),
                (ValueId(2), Instruction::GetField { object: Operand::Val(ValueId(1)), index: 0 }, Type::I64),
            ];
            f.return_ty = Type::I64;
            f.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(2))));
            let antes = format!("{m:?}"); let ps_antes = planos.clone();
            let r = preparar_arc_modulo_dart(&mut m, &mut planos);
            if caso < 2 || caso == 6 || caso == 8 {
                r.unwrap();
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                assert!(ir.contains("call i64 @dartforge_arc_ler_campo_escalar_conferido_v1"));
                assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(), (0, 0));
            } else {
                assert!(r.is_err(), "caso {caso}");
                assert_eq!(format!("{m:?}"), antes); assert_eq!(planos, ps_antes);
            }
        }
    }

    #[test]
    fn retorno_de_getter_tem_layout_conferido_sem_inventar_classe() {
        for destino_valido in [false, true] {
            let (mut m, mut planos) = modulo(vec![Type::Ref], vec![Operand::Constant(Constant::Null)]);
            m.functions[0].params = vec![(ValueId(0), "this".into(), Type::Ref)];
            m.functions[0].blocks[0].instructions = vec![
                (ValueId(1), Instruction::CallStatic { symbol: "getter".into(),
                    args: vec![Operand::Val(ValueId(0))], ret_ty: Type::Ref }, Type::Ref),
                (ValueId(2), Instruction::SetField { object: Operand::Val(ValueId(1)), index: 0,
                    value: Operand::Constant(Constant::Null) }, Type::Void),
            ];
            if destino_valido {
                m.functions.push(Function { symbol: "getter".into(), name: "getter".into(), depuracao: None,
                    params: vec![(ValueId(0), "this".into(), Type::Ref)], return_ty: Type::Ref,
                    blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                        (ValueId(1), Instruction::GetField { object: Operand::Val(ValueId(0)), index: 0 }, Type::Ref),
                    ], terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) }] });
                let mut p = PlanoFuncaoDart::default();
                p.tokens.retorno = RetornoTokens::Owned;
                planos.insert("getter".into(), p);
            }
            let antes = format!("{m:?}/{planos:?}");
            let resultado = preparar_arc_modulo_dart(&mut m, &mut planos);
            if destino_valido {
                resultado.unwrap();
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                assert!(ir.contains("call void @dartforge_arc_gravar_campo_ref_conferido_v1"));
                assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(), (0, 0));
            } else {
                assert!(resultado.is_err());
                assert_eq!(antes, format!("{m:?}/{planos:?}"));
            }
        }
    }

    #[test]
    fn alocacao_no_bloco_prova_prefixo_sem_reviver_alias_antigo_ou_exposto() {
        for caso in 0..5 {
            let (mut m, mut planos) = modulo(vec![Type::I64], vec![Operand::Constant(Constant::Int(0))]);
            let opaco = (ValueId(7), Instruction::CallRuntime { name: "dartforge_arc_collect".into(),
                args: vec![], ret_ty: Type::Void }, Type::Void);
            let b = &mut m.functions[0].blocks[0];
            if caso == 0 { b.instructions.insert(0, opaco.clone()); }
            if caso == 2 { b.instructions.push(opaco.clone()); }
            if caso == 3 {
                b.instructions.push((ValueId(8), Instruction::ArcCopy { value: Operand::Val(ValueId(0)) }, Type::Ref));
                b.instructions.push(opaco.clone());
            }
            b.instructions.push((ValueId(1), Instruction::GetField { object: Operand::Val(ValueId(if caso == 3 { 8 } else { 0 })), index: 0 }, Type::I64));
            if caso == 1 { b.instructions.push(opaco.clone()); }
            if caso == 4 {
                let alocar = b.instructions[0].clone();
                b.instructions = vec![(ValueId(9), alocar.1.clone(), Type::Ref)];
                b.terminator = Terminator::Branch(BlockId(1));
                m.functions[0].blocks.push(BasicBlock { id: BlockId(1), instructions: vec![
                    (ValueId(8), Instruction::Phi { ty: Type::Ref, incoming: vec![
                        (BlockId(0), Operand::Val(ValueId(9))), (BlockId(1), Operand::Val(ValueId(0))),
                    ] }, Type::Ref),
                    opaco, alocar,
                    (ValueId(1), Instruction::GetField { object: Operand::Val(ValueId(8)), index: 0 }, Type::I64),
                ], terminator: Terminator::CondBranch { cond: Operand::Constant(Constant::Bool(true)),
                    then_block: BlockId(1), else_block: BlockId(2) } });
                m.functions[0].blocks.push(BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(None) });
            }
            if caso == 4 {
                // O objeto da volta anterior morre na aresta de volta, antes
                // de o Phi receber o novo: a preparação solta o token ali, e
                // a leitura do campo continua conferida.
                preparar_arc_modulo_dart(&mut m, &mut planos).unwrap();
                let insts: Vec<_> = m.functions[0].blocks.iter().flat_map(|b| &b.instructions).collect();
                assert!(insts.iter().any(|(_, i, _)| matches!(i, Instruction::CallRuntime { name, .. } if name == super::LER_ESCALAR)));
                assert!(insts.iter().any(|(_, i, _)| matches!(i, Instruction::ArcDrop { .. })));
                continue;
            }
            let resultado = preparar_arc_modulo_dart(&mut m, &mut planos);
            resultado.unwrap();
            let conferido = m.functions[0].blocks.iter().flat_map(|b| &b.instructions)
                .any(|(_, i, _)| matches!(i, Instruction::CallRuntime { name, .. } if name == super::LER_ESCALAR));
            assert_eq!(conferido, caso >= 2, "caso {caso}: acesso exposto deve conferir layout");
            assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(), (0, 0));
        }
    }

    #[test]
    fn definir_rti_preserva_instancia_e_exige_guarda_nas_listas() {
        for lista in [false, true] {
            let (mut m, mut planos) = modulo(vec![Type::Ref], vec![Operand::Constant(Constant::Null)]);
            if lista {
                let cid = dartforge_runtime::layout::cid::LIST as u32;
                m.classes[0].id = cid;
                m.layouts_campos_arc = HashMap::from([(cid, vec![Type::Ref])]);
                let Instruction::AllocObject { class_id, .. } = &mut m.functions[0].blocks[0].instructions[0].1 else { panic!(); };
                *class_id = cid;
            }
            m.functions[0].blocks[0].instructions.extend([
                (ValueId(1), Instruction::CallRuntime { name: "dartforge_rti_definir".into(), args: vec![
                    (Operand::Val(ValueId(0)), Type::Ref), (Operand::Constant(Constant::Int(0)), Type::I64),
                ], ret_ty: Type::Void }, Type::Void),
                (ValueId(2), Instruction::SetField { object: Operand::Val(ValueId(0)), index: 0,
                    value: Operand::Constant(Constant::Null) }, Type::Void),
            ]);
            let resultado = preparar_arc_modulo_dart(&mut m, &mut planos);
            resultado.unwrap();
            assert_eq!(m.functions[0].blocks[0].instructions.iter().any(|(_, i, _)|
                matches!(i, Instruction::CallRuntime { name, .. } if name == super::GRAVAR_CONFERIDO)), lista);
            assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(), (0, 0));
        }
    }

    #[test]
    fn campos_escalares_abertos_conferem_layout_sem_confundir_representacoes() {
        for repr in [Type::I64, Type::F64, Type::I1, Type::I8] {
            let (mut m, mut planos) = modulo(vec![repr], vec![Operand::Constant(Constant::Null)]);
            let f = &mut m.functions[0];
            f.params = vec![(ValueId(0), "this".into(), Type::Ref)];
            f.blocks[0].instructions.clear();
            let valor = match repr {
                Type::I64 => Operand::Constant(Constant::Int(42)),
                Type::F64 => Operand::Constant(Constant::Double(-0.0)),
                Type::I1 => Operand::Constant(Constant::Bool(true)),
                Type::I8 => {
                    f.blocks[0].instructions.push((ValueId(1), Instruction::Trunc {
                        op: Operand::Constant(Constant::Int(255)), from: Type::I64, to: Type::I8,
                    }, Type::I8));
                    Operand::Val(ValueId(1))
                }
                _ => unreachable!(),
            };
            f.blocks[0].instructions.extend([
                (ValueId(2), Instruction::SetField { object: Operand::Val(ValueId(0)), index: 0, value: valor }, Type::Void),
                (ValueId(3), Instruction::GetField { object: Operand::Val(ValueId(0)), index: 0 }, repr),
            ]);
            preparar_arc_modulo_dart(&mut m, &mut planos).unwrap();
            let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
            assert!(ir.contains("call void @dartforge_arc_gravar_campo_escalar_conferido_v1"));
            assert!(ir.contains("call i64 @dartforge_arc_ler_campo_escalar_conferido_v1"));
            assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(), (0, 0));
            certificar_portas_arc_aot(&mut m, &planos).unwrap();
            m.layouts_campos_arc.insert(123, vec![Type::Ref]);
            let antes = format!("{m:?}/{planos:?}");
            assert!(preparar_arc_modulo_dart(&mut m, &mut planos).is_err());
            assert_eq!(antes, format!("{m:?}/{planos:?}"));
            assert!(certificar_portas_arc_aot(&mut m, &planos).is_err());
        }
    }

    #[test]
    fn setter_ref_aberto_confere_classe_tamanho_e_tabela_atual() {
        let (mut m, mut planos) = modulo(vec![Type::Ref], vec![Operand::Constant(Constant::Null)]);
        m.layouts_campos_arc.insert(124, vec![Type::Ref, Type::I64]);
        m.layouts_campos_arc.insert(125, vec![Type::I64]);
        for (id, n) in [(124, 2), (125, 1)] {
            m.classes.push(ClassDef { id, name: format!("C{id}"), field_count: n,
                vtable: vec![], to_string_symbol: None });
        }
        m.functions[0].params = vec![(ValueId(0), "this".into(), Type::Ref),
            (ValueId(1), "valor".into(), Type::Ref)];
        m.functions[0].blocks[0].instructions = vec![(ValueId(2), Instruction::SetField {
            object: Operand::Val(ValueId(0)), index: 0, value: Operand::Val(ValueId(1)),
        }, Type::Void)];
        preparar_arc_modulo_dart(&mut m, &mut planos).unwrap();
        assert!(m.functions[0].blocks[0].instructions.iter().any(|(_, i, _)|
            matches!(i, Instruction::ConstArray(v) if v == &vec![123, 1, 124, 2])));
        let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
        assert!(ir.contains("call void @dartforge_arc_gravar_campo_ref_conferido_v1"));
        assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(), (0, 0));
        for caso in 0..4 {
            let mut fs = m.functions.clone();
            let mut ps = planos.clone();
            if caso == 0 {
                let (_, Instruction::ConstArray(v), _) = &mut fs[0].blocks[0].instructions[0] else { panic!(); };
                v[1] = 2;
            } else {
                let (_, Instruction::CallRuntime { args, .. }, _) = &mut fs[0].blocks[0].instructions[1] else { panic!(); };
                if caso == 1 { args[4].0 = Operand::Constant(Constant::Int(1)); }
                if caso == 2 { args[3] = (Operand::Val(ValueId(0)), Type::Ref); }
                if caso == 3 { args[1].0 = Operand::Constant(Constant::Int(-1)); }
            }
            let antes = format!("{fs:?}/{ps:?}");
            let resultado = super::preparar(&mut fs[0], &m.layouts_campos_arc, ps.get_mut("campos").unwrap(), None, &HashSet::new());
            assert!(resultado.is_err(), "caso {caso}: {resultado:?}");
            assert_eq!(antes, format!("{fs:?}/{ps:?}"));
        }
        certificar_portas_arc_aot(&mut m, &planos).unwrap();
        m.layouts_campos_arc.insert(123, vec![Type::I64]);
        assert!(certificar_portas_arc_aot(&mut m, &planos).is_err());
    }

    #[test]
    fn getter_ref_de_receiver_aberto_confere_mapa_e_devolve_owned() {
        let (mut m, _) = modulo(vec![], vec![]);
        m.functions[0].params = vec![(ValueId(0), "this".into(), Type::Ref)];
        m.functions[0].return_ty = Type::Ref;
        m.functions[0].blocks[0].instructions = vec![(ValueId(1), Instruction::GetField {
            object: Operand::Val(ValueId(0)), index: 39,
        }, Type::Ref)];
        m.functions[0].blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
        let mut plano = PlanoFuncaoDart::default();
        plano.tokens.retorno = RetornoTokens::Owned;
        let mut planos = HashMap::from([("campos".into(), plano)]);
        assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(), (1, 0));
        let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
        let ler = ir.find("call i64 @dartforge_arc_ler_campo_ref_v1").unwrap();
        let reter = ir.find("call void @dartforge_arc_retain").unwrap();
        assert!(ler < reter);
        assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(), (0, 0));
    }

    #[test]
    fn receiver_aberto_exige_layout_para_gravacao_ou_escalar_e_limites() {
        for caso in 0..3 {
            let (mut m, mut planos) = modulo(vec![], vec![]);
            m.functions[0].params = vec![(ValueId(0), "this".into(), Type::Ref)];
            m.functions[0].blocks[0].instructions = vec![(ValueId(1), if caso == 0 {
                Instruction::SetField { object: Operand::Val(ValueId(0)), index: 0,
                    value: Operand::Constant(Constant::Null) }
            } else {
                Instruction::GetField { object: Operand::Val(ValueId(0)),
                    index: if caso == 2 { usize::MAX } else { 0 } }
            }, if caso == 0 { Type::Void } else if caso == 1 { Type::I64 } else { Type::Ref })];
            let antes = format!("{m:?}/{planos:?}");
            assert!(preparar_arc_modulo_dart(&mut m, &mut planos).is_err());
            assert_eq!(antes, format!("{m:?}/{planos:?}"));
        }
    }

    #[test]
    fn fabrica_zerada_fixa_origem_apenas_com_classe_e_tamanho_conferidos() {
        for caso in 0..5 {
            let (mut m, mut planos) = modulo(vec![Type::Ref], vec![Operand::Constant(Constant::Null)]);
            m.functions[0].params.push((ValueId(7), "filho".into(), Type::Ref));
            let nome = if caso == 1 { "dartforge_arc_objeto_owned_v1" } else { "dartforge_object_new" };
            m.functions[0].blocks[0].instructions[0].1 = Instruction::CallRuntime {
                name: nome.into(), args: vec![
                    (Operand::Constant(Constant::Int(if caso == 2 { 124 } else { 123 })), Type::I64),
                    (Operand::Constant(Constant::Int(if caso == 3 { 2 } else { 1 })), Type::I64),
                ], ret_ty: Type::Ref,
            };
            if caso == 4 {
                m.functions[0].blocks[0].instructions.push((ValueId(8), Instruction::CallRuntime {
                    name: "dartforge_print_handle".into(), args: vec![(Operand::Val(ValueId(0)), Type::Ref)], ret_ty: Type::Void,
                }, Type::Void));
            }
            m.functions[0].blocks[0].instructions.push((ValueId(1), Instruction::SetField {
                object: Operand::Val(ValueId(0)), index: 0, value: Operand::Val(ValueId(7)),
            }, Type::Void));
            let antes = format!("{m:?}/{planos:?}");
            let resultado = preparar_arc_modulo_dart(&mut m, &mut planos);
            if caso >= 2 {
                assert!(resultado.is_err(), "caso {caso}: {resultado:?}");
                assert_eq!(antes, format!("{m:?}/{planos:?}"));
            } else {
                resultado.unwrap();
                assert!(m.functions[0].blocks[0].instructions.iter().any(|(_, i, _)|
                    matches!(i, Instruction::CallRuntime { name, .. } if name == "dartforge_arc_gravar_campo_ref_v1")));
                assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(), (0, 0));
            }
        }
    }

    #[test]
    fn leitura_ref_retida_antes_da_substituicao_da_aresta() {
        let (mut m, mut planos) = modulo(vec![Type::Ref], vec![Operand::Val(ValueId(10))]);
        let f = &mut m.functions[0];
        f.return_ty = Type::Ref;
        f.blocks[0].instructions.insert(
            0,
            (
                ValueId(10),
                Instruction::Box {
                    op: Operand::Constant(Constant::Int(i64::MAX)),
                    from: Type::I64,
                },
                Type::Ref,
            ),
        );
        f.blocks[0].instructions.extend([
            (
                ValueId(1),
                Instruction::GetField {
                    object: Operand::Val(ValueId(0)),
                    index: 0,
                },
                Type::Ref,
            ),
            (
                ValueId(2),
                Instruction::SetField {
                    object: Operand::Val(ValueId(0)),
                    index: 0,
                    value: Operand::Constant(Constant::Null),
                },
                Type::Void,
            ),
        ]);
        f.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
        planos.get_mut("campos").unwrap().tokens.retorno = RetornoTokens::Owned;
        assert_eq!(
            preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
            (1, 2)
        );
        let ops = &m.functions[0].blocks[0].instructions;
        let copy = ops
            .iter()
            .position(|(v, i, _)| *v == ValueId(1) && matches!(i, Instruction::ArcCopy { .. }))
            .unwrap();
        let set = ops.iter().position(|(v, _, _)| *v == ValueId(2)).unwrap();
        assert!(copy < set);
        assert!(!ops.iter().any(|(_, i, _)| matches!(
            i,
            Instruction::GetField { .. } | Instruction::SetField { .. }
        )));
        crate::llvm::LlvmEmitter::new(&m).emit_all();
        let antes = format!("{m:?}/{planos:?}");
        assert_eq!(
            preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
            (0, 0)
        );
        assert_eq!(format!("{m:?}/{planos:?}"), antes);
    }

    #[test]
    fn campos_estreitos_preservam_bits_e_limite_antes_da_leitura() {
        let (mut m, mut planos) = modulo(
            vec![Type::F64, Type::I1, Type::I8],
            vec![
                Operand::Constant(Constant::Double(-0.0)),
                Operand::Constant(Constant::Bool(true)),
                Operand::Val(ValueId(10)),
            ],
        );
        let f = &mut m.functions[0];
        f.blocks[0].instructions.insert(
            0,
            (
                ValueId(10),
                Instruction::Trunc {
                    op: Operand::Constant(Constant::Int(255)),
                    from: Type::I64,
                    to: Type::I8,
                },
                Type::I8,
            ),
        );
        for (i, repr) in [Type::F64, Type::I1, Type::I8].into_iter().enumerate() {
            f.blocks[0].instructions.push((
                ValueId(20 + i as u32),
                Instruction::GetField {
                    object: Operand::Val(ValueId(0)),
                    index: i,
                },
                repr,
            ));
            f.blocks[0].instructions.push((
                ValueId(30 + i as u32),
                Instruction::SetField {
                    object: Operand::Val(ValueId(0)),
                    index: i,
                    value: Operand::Val(ValueId(20 + i as u32)),
                },
                Type::Void,
            ));
        }
        let p = planos.get_mut("campos").unwrap();
        p.escopos
            .antes
            .insert(ValueId(20), vec![AlteracaoEscopo::Abrir(7)]);
        p.escopos
            .saidas
            .insert(BlockId(0), vec![AlteracaoEscopo::Fechar(7)]);
        assert_eq!(
            preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
            (0, 1)
        );
        let ops = &m.functions[0].blocks[0].instructions;
        let pos = ops.iter().position(|(v, _, _)| *v == ValueId(20)).unwrap();
        assert!(matches!(
            ops[pos].1,
            Instruction::Bitcast { to: Type::F64, .. }
        ));
        assert_eq!(
            planos["campos"].escopos.antes[&ops[pos - 1].0],
            vec![AlteracaoEscopo::Abrir(7)]
        );
        assert!(!planos["campos"].escopos.antes.contains_key(&ValueId(20)));
        for v in [21, 22] {
            assert!(matches!(
                ops.iter().find(|(id, _, _)| id.0 == v).unwrap().1,
                Instruction::Trunc {
                    from: Type::I64,
                    ..
                }
            ));
        }
        crate::llvm::LlvmEmitter::new(&m).emit_all();
    }

    #[test]
    fn aliases_expostos_exigem_guarda_e_contradicoes_rejeitam_atomicamente() {
        for caso in 0..8 {
            let (mut m, mut planos) =
                modulo(vec![Type::I64], vec![Operand::Constant(Constant::Int(85))]);
            let f = &mut m.functions[0];
            let mut receiver = Operand::Val(ValueId(0));
            let mut index = 0;
            let mut ty = Type::I64;
            match caso {
                0 => {
                    f.blocks[0].instructions.extend([
                        (
                            ValueId(1),
                            Instruction::ArcCopy {
                                value: receiver.clone(),
                            },
                            Type::Ref,
                        ),
                        (
                            ValueId(2),
                            Instruction::CallRuntime {
                                name: "dartforge_print_handle".into(),
                                args: vec![(Operand::Val(ValueId(1)), Type::Ref)],
                                ret_ty: Type::Void,
                            },
                            Type::Void,
                        ),
                    ]);
                }
                1 => {
                    f.params.push((ValueId(7), "C".into(), Type::Ref));
                    receiver = Operand::Val(ValueId(7));
                }
                2 => index = 1,
                3 => ty = Type::Ref,
                4 => {
                    f.blocks[0].instructions.push((
                        ValueId(1),
                        Instruction::CallRuntime {
                            name: "dartforge_arc_gravar_campo_ref_v1".into(),
                            args: vec![
                                (receiver.clone(), Type::Ref),
                                (Operand::Constant(Constant::Int(0)), Type::I64),
                                (Operand::Constant(Constant::Null), Type::Ref),
                            ],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    ));
                }
                5 => {
                    m.layouts_campos_arc.insert(123, vec![Type::F64]);
                    ty = Type::F64;
                    planos
                        .get_mut("campos")
                        .unwrap()
                        .classes
                        .insert(ValueId(u32::MAX), Ownership::Trivial);
                }
                6 => {
                    f.blocks[0].instructions.push((
                        ValueId(8),
                        Instruction::SetField {
                            object: receiver.clone(),
                            index: 0,
                            value: Operand::Constant(Constant::Null),
                        },
                        Type::Void,
                    ));
                }
                _ => {
                    f.blocks[0].instructions.push((
                        ValueId(8),
                        Instruction::CallRuntime {
                            name: "extern_sem_resumo".into(),
                            args: vec![],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    ));
                }
            }
            f.blocks[0].instructions.push((
                ValueId(5),
                Instruction::GetField {
                    object: receiver,
                    index,
                },
                ty,
            ));
            let antes = format!("{m:?}/{planos:?}");
            let resultado = preparar_arc_modulo_dart(&mut m, &mut planos);
            if matches!(caso, 1 | 4) {
                resultado.unwrap();
                assert!(m.functions[0].blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)|
                    matches!(i, Instruction::CallRuntime { name, .. } if name == super::LER_ESCALAR)));
                assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(), (0, 0));
            } else {
                let erro = resultado.unwrap_err();
                if caso == 0 { assert!(erro.contains("saídas excepcionais"), "caso {caso}: {erro}"); }
                if caso == 7 { assert!(erro.contains("extern_sem_resumo"), "caso {caso}: {erro}"); }
                assert_eq!(format!("{m:?}/{planos:?}"), antes, "caso {caso}");
            }
            m.memoria_arc = false;
            let antes = format!("{m:?}/{planos:?}");
            assert_eq!(
                preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                (0, 0)
            );
            assert_eq!(format!("{m:?}/{planos:?}"), antes);
        }
    }

    #[test]
    fn guarda_de_layout_com_phi_misto_recebe_copia_na_aresta() {
        let (mut m, mut planos) =
            modulo(vec![Type::I64], vec![Operand::Constant(Constant::Int(85))]);
        let f = &mut m.functions[0];
        f.params = vec![
            (ValueId(7), "outro".into(), Type::Ref),
            (ValueId(8), "cond".into(), Type::I1),
        ];
        f.blocks[0].terminator = Terminator::CondBranch {
            cond: Operand::Val(ValueId(8)),
            then_block: BlockId(1),
            else_block: BlockId(2),
        };
        for b in [1, 2] {
            f.blocks.push(BasicBlock {
                id: BlockId(b),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(3)),
            });
        }
        f.blocks.push(BasicBlock {
            id: BlockId(3),
            instructions: vec![
                (
                    ValueId(9),
                    Instruction::Phi {
                        incoming: vec![
                            (BlockId(1), Operand::Val(ValueId(0))),
                            (BlockId(2), Operand::Val(ValueId(7))),
                        ],
                        ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(10),
                    Instruction::CallRuntime {
                        name: "dartforge_print_handle".into(),
                        args: vec![(Operand::Val(ValueId(9)), Type::Ref)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ),
                (
                    ValueId(11),
                    Instruction::GetField {
                        object: Operand::Val(ValueId(0)),
                        index: 0,
                    },
                    Type::I64,
                ),
            ],
            terminator: Terminator::Return(None),
        });
        // Sem consumidor falível no meio: o empréstimo do parâmetro ganha a
        // cópia na aresta, e a guarda de layout continua em `v11`.
        m.functions[0].blocks.last_mut().unwrap().instructions.remove(1);
        preparar_arc_modulo_dart(&mut m, &mut planos).unwrap();
        assert!(m.functions[0].blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)|
            matches!(i, Instruction::ArcCopy { value: Operand::Val(ValueId(7)) })));
    }

    #[test]
    fn phi_e_copia_conservam_todos_os_layouts_candidatos() {
        for caso in 0..3 {
            let (mut m, mut planos) =
                modulo(vec![Type::I64], vec![Operand::Constant(Constant::Int(85))]);
            m.classes.push(ClassDef {
                id: 124,
                name: "D".into(),
                field_count: 1,
                vtable: vec![],
                to_string_symbol: None,
            });
            let repr = if caso == 1 { Type::Ref } else { Type::I64 };
            m.layouts_campos_arc.insert(124, vec![repr]);
            let outro = if caso == 1 {
                Operand::Constant(Constant::Null)
            } else {
                Operand::Constant(Constant::Int(86))
            };
            let f = &mut m.functions[0];
            f.params = vec![(ValueId(0), "cond".into(), Type::I1)];
            f.return_ty = Type::I64;
            f.blocks = vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(0)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::AllocObject {
                            class_id: 123,
                            fields: vec![Operand::Constant(Constant::Int(85))],
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Branch(BlockId(3)),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![(
                        ValueId(2),
                        Instruction::AllocObject {
                            class_id: 124,
                            fields: vec![outro],
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Branch(BlockId(3)),
                },
                BasicBlock {
                    id: BlockId(3),
                    instructions: vec![
                        (
                            ValueId(3),
                            Instruction::Phi {
                                incoming: vec![
                                    (BlockId(1), Operand::Val(ValueId(1))),
                                    (
                                        BlockId(2),
                                        if caso == 2 {
                                            Operand::Constant(Constant::Null)
                                        } else {
                                            Operand::Val(ValueId(2))
                                        },
                                    ),
                                ],
                                ty: Type::Ref,
                            },
                            Type::Ref,
                        ),
                        (
                            ValueId(4),
                            Instruction::ArcCopy {
                                value: Operand::Val(ValueId(3)),
                            },
                            Type::Ref,
                        ),
                        (
                            ValueId(5),
                            Instruction::GetField {
                                object: Operand::Val(ValueId(4)),
                                index: 0,
                            },
                            Type::I64,
                        ),
                    ],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(5)))),
                },
            ];
            let antes = format!("{m:?}/{planos:?}");
            if caso == 1 {
                assert!(preparar_arc_modulo_dart(&mut m, &mut planos).is_err());
                assert_eq!(format!("{m:?}/{planos:?}"), antes);
            } else {
                assert_eq!(
                    preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                    if caso == 2 { (0, 3) } else { (0, 2) }
                );
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                if caso == 0 {
                    assert!(ir.contains("call i64 @dartforge_arc_ler_campo_escalar_v1(i64 %v4, i64 0)"));
                } else {
                    // O caminho null é recusado pela guarda física, sem
                    // invalidar a representação conhecida do outro caminho.
                    assert!(ir.contains("call i64 @dartforge_arc_ler_campo_escalar_conferido_v1(i64 %v4, i64 0,"));
                }
            }
        }
    }

    #[test]
    fn phi_misto_confere_origem_nominal_sem_ocultar_tipo_ou_owner() {
        for caso in 0..4 {
            let repr = if caso == 1 { Type::Ref } else { Type::I64 };
            let campo = if repr == Type::Ref { Operand::Constant(Constant::Null) }
                else { Operand::Constant(Constant::Int(85)) };
            let (mut m, mut planos) = modulo(vec![repr], vec![campo.clone()]);
            let f = &mut m.functions[0];
            f.params = vec![(ValueId(7), "receiver".into(), Type::Ref),
                (ValueId(8), "cond".into(), Type::I1)];
            f.return_ty = Type::I64;
            f.blocks = vec![
                BasicBlock { id: BlockId(0), instructions: vec![], terminator: Terminator::CondBranch {
                    cond: Operand::Val(ValueId(8)), then_block: BlockId(1), else_block: BlockId(2) } },
                BasicBlock { id: BlockId(1), instructions: vec![
                    (ValueId(0), Instruction::AllocObject { class_id: 123, fields: vec![campo] }, Type::Ref),
                ], terminator: Terminator::Branch(BlockId(3)) },
                BasicBlock { id: BlockId(2), instructions: vec![
                    (ValueId(9), if caso == 2 { Instruction::Const(Constant::Int(0)) }
                        else { Instruction::ArcCopy { value: Operand::Val(ValueId(7)) } },
                        if caso == 2 { Type::I64 } else { Type::Ref }),
                ], terminator: Terminator::Branch(BlockId(3)) },
                BasicBlock { id: BlockId(3), instructions: vec![
                    (ValueId(10), Instruction::Phi { ty: Type::Ref, incoming: vec![
                        (BlockId(1), Operand::Val(ValueId(0))),
                        (BlockId(2), Operand::Val(ValueId(if caso == 3 { 7 } else { 9 }))),
                    ] }, Type::Ref),
                    (ValueId(11), Instruction::SetField { object: Operand::Val(ValueId(10)), index: 0,
                        value: Operand::Constant(Constant::Int(86)) }, Type::Void),
                    (ValueId(12), Instruction::GetField { object: Operand::Val(ValueId(10)), index: 0 }, Type::I64),
                ], terminator: Terminator::Return(Some(Operand::Val(ValueId(12)))) },
            ];
            let antes = format!("{m:?}/{planos:?}");
            let resultado = preparar_arc_modulo_dart(&mut m, &mut planos);
            // Caso 3: o parâmetro emprestado entra no Phi com owner; a
            // preparação copia na aresta (`phis.rs`), como o caso 0 explícito.
            if caso == 3 {
                resultado.unwrap();
                assert!(m.functions[0].blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)|
                    matches!(i, Instruction::ArcCopy { value: Operand::Val(ValueId(7)) })));
                continue;
            }
            if caso == 0 {
                resultado.unwrap();
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                assert!(ir.contains(super::GRAVAR_ESCALAR));
                assert!(ir.contains(super::LER_ESCALAR));
                let preparado = format!("{m:?}/{planos:?}");
                assert_eq!(preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(), (0, 0));
                assert_eq!(preparado, format!("{m:?}/{planos:?}"));
            } else {
                let erro = resultado.unwrap_err();
                if caso == 1 { assert!(erro.contains("tipo ou guarda incompatível"), "{erro}"); }
                assert_eq!(antes, format!("{m:?}/{planos:?}"));
            }
        }
    }

    #[test]
    fn phi_ancorado_em_laco_conserva_origem_sem_presumir_singleton() {
        let (mut m, mut planos) =
            modulo(vec![Type::I64], vec![Operand::Constant(Constant::Int(85))]);
        let f = &mut m.functions[0];
        f.params = vec![(ValueId(1), "repetir".into(), Type::I1)];
        f.return_ty = Type::I64;
        f.blocks[0].terminator = Terminator::Branch(BlockId(1));
        f.blocks.extend([
            BasicBlock {
                id: BlockId(1),
                instructions: vec![
                    (
                        ValueId(3),
                        Instruction::Phi {
                            incoming: vec![
                                (BlockId(0), Operand::Val(ValueId(0))),
                                (BlockId(2), Operand::Val(ValueId(3))),
                            ],
                            ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(5),
                        Instruction::GetField {
                            object: Operand::Val(ValueId(3)),
                            index: 0,
                        },
                        Type::I64,
                    ),
                ],
                terminator: Terminator::CondBranch {
                    cond: Operand::Val(ValueId(1)),
                    then_block: BlockId(2),
                    else_block: BlockId(3),
                },
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(1)),
            },
            BasicBlock {
                id: BlockId(3),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(5)))),
            },
        ]);
        assert_eq!(
            preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
            (0, 1)
        );
        crate::llvm::LlvmEmitter::new(&m).emit_all();
        let antes = format!("{m:?}/{planos:?}");
        assert_eq!(
            preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
            (0, 0)
        );
        assert_eq!(format!("{m:?}/{planos:?}"), antes);
    }
}
