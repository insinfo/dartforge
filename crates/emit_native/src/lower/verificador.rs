//! Verificador da HIR, antes da emissão (E3 do contrato,
//! `docs/NATIVO-PLANO.md` §6.3).
//!
//! Recusa o que, emitido, vira `panic` no runtime ou IR aceito calado pelo
//! Clang: gravação no heap com tag incoerente com a representação do valor
//! (um escalar marcado como referência vira "handle inválido" no coletor;
//! uma referência marcada como escalar é coletada viva); constante inteira
//! numa posição `Ref` (a única constante `Ref` é `Null`); instrução que o
//! emissor não sabe baixar (o antigo `; inst pendente`) ou que saiu com o
//! espaço unificado (`AllocMap`, `AllocSet`); record posicional com campo que
//! não é `Ref` (o `_Record` é `REFS`); e captura lida numa representação
//! diferente da gravada (docs/NATIVO-ESPACO-UNIFICADO.md §6, risco 12: o
//! `_Contexto` e a `_Celula` guardam a palavra crua, e só o bit do mapa diz se
//! é referência — ler um `int` gravado cru como `Ref` é um handle falso).
//!
//! Um problema aqui é bug do compilador, não do programa: a mensagem diz a
//! função e a instrução.

use crate::hir::*;
use std::collections::HashMap;

/// Pares (índice do argumento com os bits, índice do argumento com a tag)
/// das externs do runtime que recebem um valor com tag (a ABI plana `(bits,
/// tag)`: 1 `int`, 2 `bool`, 3 `Ref`, 4 `double`).
fn pares_com_tag(nome: &str) -> &'static [(usize, usize)] {
    match nome {
        "dartforge_exception_throw" | "dartforge_tagged_to_string" => &[(0, 1)],
        _ => &[],
    }
}

/// Pares (bits, `is_ref`) das externs que recebem a marca de referência.
fn pares_com_is_ref(nome: &str) -> &'static [(usize, usize)] {
    match nome {
        "dartforge_object_set" => &[(2, 3)],
        "dartforge_assertion_error_new"
        | "dartforge_exception_new"
        | "dartforge_argument_error_value" => &[(0, 1)],
        _ => &[],
    }
}

/// Uma posição de um `_Contexto` (`AllocEnv`): a representação gravada e,
/// se o valor é uma `_Celula` da mesma função, as representações gravadas
/// nela.
#[derive(Clone, PartialEq)]
struct Captura {
    ty: Type,
    celula: Option<Vec<Type>>,
}

/// A representação de `a` e `b` é a mesma palavra no contexto? (`I1` e `I8`
/// são o mesmo `bool` de 0/1.)
fn mesma_representacao(a: Type, b: Type) -> bool {
    a == b || matches!((a, b), (Type::I1 | Type::I8, Type::I1 | Type::I8))
}

struct Contexto<'m> {
    tipos: HashMap<ValueId, Type>,
    alocas: HashMap<ValueId, Type>,
    params: &'m HashMap<&'m str, Vec<Type>>,
    funcao: &'m Function,
    erros: Vec<String>,
    /// As representações gravadas em cada `_Celula` alocada nesta função
    /// (`AllocCell` e `CellSet` sobre o mesmo valor).
    celulas: HashMap<ValueId, Vec<Type>>,
    /// Os valores lidos do ambiente da closure (`EnvGet` sobre o parâmetro
    /// `env`), com a posição lida.
    lidos_do_ambiente: HashMap<ValueId, usize>,
    /// O ambiente desta função (o de toda closure que a usa como corpo),
    /// quando ele é conhecido e único no módulo.
    ambiente: Option<&'m [Captura]>,
}

impl Contexto<'_> {
    fn tipo(&self, op: &Operand) -> Type {
        match op {
            Operand::Val(v) => self.tipos.get(v).copied().unwrap_or(Type::I64),
            Operand::Constant(Constant::Int(_)) => Type::I64,
            Operand::Constant(Constant::Double(_)) => Type::F64,
            Operand::Constant(Constant::Bool(_)) => Type::I1,
            Operand::Constant(Constant::Null | Constant::String(_) | Constant::StringWtf8(_)) => Type::Ref,
            Operand::Constant(Constant::Funcao(_)) => Type::I64,
        }
    }

    fn erro(&mut self, msg: String) {
        self.erros
            .push(format!("verificador da HIR ({}): {msg}", self.funcao.name));
    }

    /// E1: a tag gravada tem de ser a da representação do valor.
    fn checar_tag(&mut self, onde: &str, bits: &Operand, tag: &Operand) {
        let Operand::Constant(Constant::Int(t)) = tag else {
            return;
        };
        let ty = self.tipo(bits);
        let coerente = match t {
            3 => ty == Type::Ref,
            1 | 2 | 4 => ty != Type::Ref,
            _ => true,
        };
        if !coerente {
            self.erro(format!("{onde}: tag {t} para um valor {ty:?}"));
        }
    }

    fn checar_is_ref(&mut self, onde: &str, bits: &Operand, is_ref: &Operand) {
        let Operand::Constant(Constant::Int(r)) = is_ref else {
            return;
        };
        let ty = self.tipo(bits);
        if (*r != 0) != (ty == Type::Ref) {
            self.erro(format!("{onde}: is_ref {r} para um valor {ty:?}"));
        }
    }

    /// E3: constante inteira numa posição `Ref`.
    fn checar_ref(&mut self, onde: &str, op: &Operand, posicao: Type) {
        if posicao == Type::Ref && matches!(op, Operand::Constant(Constant::Int(_))) {
            self.erro(format!("{onde}: constante inteira numa posição Ref"));
        }
    }

    /// O parâmetro `env` desta função (o ambiente do corpo de uma closure).
    fn e_parametro_env(&self, op: &Operand) -> bool {
        let Operand::Val(v) = op else { return false };
        self.funcao.params.iter().any(|(p, nome, _)| p == v && nome == "env")
    }

    /// As representações gravadas na célula `cell`, se conhecidas: uma
    /// `_Celula` desta função, ou uma lida do ambiente da closure.
    fn gravado_na_celula(&self, cell: &Operand) -> Option<Vec<Type>> {
        let Operand::Val(v) = cell else { return None };
        if let Some(t) = self.celulas.get(v) {
            return Some(t.clone());
        }
        let i = *self.lidos_do_ambiente.get(v)?;
        self.ambiente?.get(i)?.celula.clone()
    }

    /// Risco 12: o valor lido de uma captura tem de estar na representação
    /// com que ela foi gravada.
    fn checar_leitura(&mut self, onde: &str, lida: Type, gravadas: &[Type]) {
        if let Some(g) = gravadas.iter().find(|&&g| !mesma_representacao(g, lida)) {
            self.erro(format!("{onde}: lida como {lida:?}, gravada como {g:?}"));
        }
    }
}

/// As capturas de cada `AllocEnv` de `f` (pelo valor dele), com as células
/// alocadas em `f` que ele guarda.
fn ambientes_da_funcao(f: &Function) -> HashMap<ValueId, Vec<Captura>> {
    let mut tipos: HashMap<ValueId, Type> = HashMap::new();
    for (vid, _, ty) in &f.params {
        tipos.insert(*vid, *ty);
    }
    let mut celulas: HashMap<ValueId, Vec<Type>> = HashMap::new();
    for b in &f.blocks {
        for (vid, inst, ty) in &b.instructions {
            tipos.insert(*vid, *ty);
            if let Instruction::AllocCell { value } = inst {
                celulas.insert(*vid, vec![tipo_do_operando(&tipos, value)]);
            }
        }
    }
    for b in &f.blocks {
        for (_, inst, _) in &b.instructions {
            if let Instruction::CellSet { cell: Operand::Val(c), value } = inst
                && let Some(t) = celulas.get_mut(c)
            {
                t.push(tipo_do_operando(&tipos, value));
            }
        }
    }
    let mut saida = HashMap::new();
    for b in &f.blocks {
        for (vid, inst, _) in &b.instructions {
            if let Instruction::AllocEnv { values } = inst {
                let capturas = values
                    .iter()
                    .map(|v| Captura {
                        ty: tipo_do_operando(&tipos, v),
                        celula: match v {
                            Operand::Val(c) => celulas.get(c).cloned(),
                            _ => None,
                        },
                    })
                    .collect();
                saida.insert(*vid, capturas);
            }
        }
    }
    saida
}

/// A representação de `op` pelos tipos registrados.
fn tipo_do_operando(tipos: &HashMap<ValueId, Type>, op: &Operand) -> Type {
    match op {
        Operand::Val(v) => tipos.get(v).copied().unwrap_or(Type::I64),
        Operand::Constant(Constant::Int(_) | Constant::Funcao(_)) => Type::I64,
        Operand::Constant(Constant::Double(_)) => Type::F64,
        Operand::Constant(Constant::Bool(_)) => Type::I1,
        Operand::Constant(Constant::Null | Constant::String(_) | Constant::StringWtf8(_)) => Type::Ref,
    }
}

/// O ambiente de cada corpo de closure do módulo (a entrada uniforme e o corpo
/// tipado): o `AllocEnv` passado a `AllocClosure`/`AllocClosureTipada`. Um
/// corpo com ambientes de formas diferentes fica de fora (`None`).
fn ambientes_dos_corpos(module: &Module) -> HashMap<String, Option<Vec<Captura>>> {
    let mut saida: HashMap<String, Option<Vec<Captura>>> = HashMap::new();
    for f in &module.functions {
        let ambientes = ambientes_da_funcao(f);
        let mut registrar = |simbolo: &str, env: &Operand| {
            let Operand::Val(e) = env else { return };
            let Some(capturas) = ambientes.get(e) else { return };
            match saida.get(simbolo) {
                Some(Some(ja)) if ja != capturas => {
                    saida.insert(simbolo.to_string(), None);
                }
                Some(_) => {}
                None => {
                    saida.insert(simbolo.to_string(), Some(capturas.clone()));
                }
            }
        };
        for b in &f.blocks {
            for (_, inst, _) in &b.instructions {
                match inst {
                    Instruction::AllocClosure { code_symbol, env } => registrar(code_symbol, env),
                    Instruction::AllocClosureTipada { code_symbol, env, tipado, direto: false, .. } => {
                        registrar(code_symbol, env);
                        registrar(tipado, env);
                    }
                    _ => {}
                }
            }
        }
    }
    saida
}

/// Verifica o módulo inteiro; devolve os problemas encontrados.
pub fn verificar(module: &Module) -> Vec<String> {
    let params: HashMap<&str, Vec<Type>> = module
        .functions
        .iter()
        .map(|f| (f.symbol.as_str(), f.params.iter().map(|p| p.2).collect()))
        .collect();
    let corpos = ambientes_dos_corpos(module);
    let mut erros = Vec::new();
    for f in &module.functions {
        let mut c = Contexto {
            tipos: HashMap::new(),
            alocas: HashMap::new(),
            params: &params,
            funcao: f,
            erros: Vec::new(),
            celulas: HashMap::new(),
            lidos_do_ambiente: HashMap::new(),
            ambiente: corpos.get(&f.symbol).and_then(|a| a.as_deref()),
        };
        for (vid, _, ty) in &f.params {
            c.tipos.insert(*vid, *ty);
        }
        for b in &f.blocks {
            for (vid, inst, ty) in &b.instructions {
                c.tipos.insert(*vid, *ty);
                if let Instruction::Alloca(t) = inst {
                    c.alocas.insert(*vid, *t);
                }
            }
        }
        // As células desta função e o que foi gravado nelas, e o que foi
        // lido do ambiente (a posição), antes de conferir as leituras.
        for b in &f.blocks {
            for (vid, inst, _) in &b.instructions {
                match inst {
                    Instruction::AllocCell { value } => {
                        let t = c.tipo(value);
                        c.celulas.insert(*vid, vec![t]);
                    }
                    Instruction::EnvGet { env, index } if c.e_parametro_env(env) => {
                        c.lidos_do_ambiente.insert(*vid, *index);
                    }
                    _ => {}
                }
            }
        }
        for b in &f.blocks {
            for (_, inst, _) in &b.instructions {
                if let Instruction::CellSet { cell: Operand::Val(cel), value } = inst
                    && c.celulas.contains_key(cel)
                {
                    let t = c.tipo(value);
                    c.celulas.get_mut(cel).expect("célula conferida").push(t);
                }
            }
        }
        for b in &f.blocks {
            for (_, inst, ty) in &b.instructions {
                verificar_instrucao(&mut c, inst, *ty);
            }
            if let Terminator::Return(Some(op)) = &b.terminator {
                c.checar_ref("return", op, f.return_ty);
            }
        }
        erros.extend(c.erros);
    }
    erros
}

fn verificar_instrucao(c: &mut Contexto, inst: &Instruction, ty: Type) {
    match inst {
        Instruction::ArcLoadStrong { slot } | Instruction::ArcStoreStrong { slot, .. } => {
            let resultado = if matches!(inst, Instruction::ArcLoadStrong { .. }) { Type::Ref } else { Type::Void };
            if ty != resultado {
                c.erro(format!("slot ARC: resultado {ty:?}, esperado {resultado:?}"));
            }
            let SlotForte::Quadro { quadro, .. } = slot;
            if c.tipo(quadro) != Type::I64 || !matches!(quadro, Operand::Val(_)) {
                c.erro(format!("slot ARC exige ID de quadro SSA I64: {quadro:?}"));
            }
            if let Instruction::ArcStoreStrong { value, .. } = inst {
                if c.tipo(value) != Type::Ref || !matches!(value, Operand::Val(_) | Operand::Constant(Constant::Null)) {
                    c.erro(format!("store ARC exige referência já avaliada: {value:?}"));
                }
            }
        }
        Instruction::ArcCopy { value } | Instruction::ArcDrop { value } | Instruction::ArcMove { value } => {
            let resultado = if matches!(inst, Instruction::ArcDrop { .. }) { Type::Void } else { Type::Ref };
            if ty != resultado {
                c.erro(format!("operação ARC: resultado {ty:?}, esperado {resultado:?}"));
            }
            if c.tipo(value) != Type::Ref
                || !matches!(value, Operand::Val(_) | Operand::Constant(Constant::Null))
            {
                c.erro(format!("operação ARC exige referência já avaliada: {value:?}"));
            }
        }
        Instruction::CallRuntime { name, args, .. } => {
            for &(b, t) in pares_com_tag(name) {
                if let (Some((bits, _)), Some((tag, _))) = (args.get(b), args.get(t)) {
                    c.checar_tag(name, bits, tag);
                }
            }
            for &(b, r) in pares_com_is_ref(name) {
                if let (Some((bits, _)), Some((is_ref, _))) = (args.get(b), args.get(r)) {
                    c.checar_is_ref(name, bits, is_ref);
                }
            }
            for (op, posicao) in args {
                c.checar_ref(name, op, *posicao);
            }
        }
        Instruction::CallStatic { symbol, args, .. } => {
            if let Some(ps) = c.params.get(symbol.as_str()).cloned() {
                for (op, p) in args.iter().zip(ps) {
                    c.checar_ref(symbol, op, p);
                }
            }
        }
        Instruction::AllocList { elements } => {
            for (op, tag) in elements {
                c.checar_tag(
                    "literal de coleção",
                    op,
                    &Operand::Constant(Constant::Int(i64::from(*tag))),
                );
            }
        }
        // O record posicional é `REFS` (§2.5): todo campo é `Ref`.
        Instruction::AllocRecord { elements } => {
            for (op, _) in elements {
                let t = c.tipo(op);
                if t != Type::Ref {
                    c.erro(format!("record posicional: campo {t:?} (tem de ser Ref)"));
                }
                c.checar_ref("record posicional", op, Type::Ref);
            }
        }
        // Risco 12: a captura lida na representação gravada.
        Instruction::EnvGet { env, index } if c.e_parametro_env(env) => {
            if let Some(g) = c.ambiente.and_then(|a| a.get(*index)).map(|x| x.ty) {
                c.checar_leitura(&format!("captura {index} do ambiente"), ty, &[g]);
            }
        }
        Instruction::CellGet { cell } => {
            if let Some(g) = c.gravado_na_celula(cell) {
                c.checar_leitura("célula", ty, &g);
            }
        }
        Instruction::CellSet { cell, value } => {
            // Gravar numa célula que veio do ambiente: na representação dela.
            if let Operand::Val(v) = cell
                && !c.celulas.contains_key(v)
                && let Some(g) = c.gravado_na_celula(cell)
            {
                let t = c.tipo(value);
                c.checar_leitura("gravação na célula", t, &g);
            }
        }
        Instruction::Phi { incoming, ty } => {
            for (_, op) in incoming {
                c.checar_ref("phi", op, *ty);
            }
        }
        Instruction::Store {
            ptr: Operand::Val(p),
            val,
        } => {
            if let Some(&t) = c.alocas.get(p) {
                c.checar_ref("store", val, t);
            }
        }
        Instruction::Const(Constant::Int(_)) if ty == Type::Ref => {
            c.erro("constante inteira registrada como Ref".to_string());
        }
        Instruction::CallClosure { closure, args, tupla_tipos, .. } => {
            c.checar_ref("chamada de closure", closure, Type::Ref);
            if c.tipo(tupla_tipos) != Type::I64 {
                c.erro(format!("tupla de tipos da closure não é I64: {tupla_tipos:?}"));
            }
            for a in args {
                c.checar_ref("chamada de closure", a, Type::Ref);
                if c.tipo(a) != Type::Ref {
                    c.erro(format!("argumento de closure não é Ref: {a:?}"));
                }
            }
        }
        Instruction::CallSeletorRepasse { recv, .. } => {
            c.checar_ref("repasse por seletor", recv, Type::Ref);
        }
        Instruction::CallSeletor { recv, args, tupla_tipos, .. } => {
            c.checar_ref("chamada por seletor", recv, Type::Ref);
            if c.tipo(tupla_tipos) != Type::I64 {
                c.erro(format!("tupla de tipos do seletor não é I64: {tupla_tipos:?}"));
            }
            for a in args {
                if c.tipo(a) != Type::Ref {
                    c.erro(format!("argumento de seletor não é Ref: {a:?}"));
                }
            }
        }
        Instruction::GetListElement { .. }
        | Instruction::SetListElement { .. }
        | Instruction::CallInterface { .. }
        | Instruction::CallDynamic { .. }
        | Instruction::CheckNotNull(_)
        | Instruction::IsClass { .. } => {
            c.erro(format!("instrução sem emissão: {inst:?}"));
        }
        _ => {}
    }
}
