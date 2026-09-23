//! O executor de macros visto do hospedeiro: o trait [`ExecutorMacros`] e as
//! implementações.
//!
//! * [`Indisponivel`] — a de hoje no produto. O executor real é o **nativo**
//!   (D4): a macro compilada pelo `emit_native` e executada pelo nosso
//!   executor num processo separado. Ele entra quando o nativo compilar o
//!   que a API usa (`async`/`Future`, coleções, `dart:convert`); o ponto de
//!   encaixe é [`ExecutorDfexec`] com o canal do processo nativo.
//! * [`ExecutorDfexec`] — o cliente do serviço `macro.*` do `dfexec/1` sobre
//!   qualquer [`Canal`]: um processo ([`crate::protocolo::CanalDeProcesso`]),
//!   ou, nos testes, uma sessão gravada ([`CanalGravado`]). É o mesmo cliente
//!   para o executor nativo e para o builder de materialização que roda a
//!   mesma macro numa VM (docs/MACROS-COMPATIBILIDADE.md).
use crate::montagem::{Resultado, resultado_de_json};
use crate::protocolo::{Apresentacao, Canal, conferir_ola, ola_do_hospedeiro};
use serde_json::{Value, json};

/// As três fases (spec, "Phases").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Fase {
    Tipos,
    Declaracoes,
    Definicoes,
}

impl Fase {
    pub const TODAS: [Fase; 3] = [Fase::Tipos, Fase::Declaracoes, Fase::Definicoes];

    /// O nome no protocolo.
    pub fn nome(self) -> &'static str {
        match self {
            Fase::Tipos => "tipos",
            Fase::Declaracoes => "declaracoes",
            Fase::Definicoes => "definicoes",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Disponibilidade {
    Disponivel,
    Indisponivel(String),
}

/// Erro de uma consulta, como a macro o vê (`MacroException`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroDeConsulta {
    /// `implementacao` (uso errado da API), `ciclo` (ciclo de introspecção
    /// na fase 2) ou `inesperado`.
    pub tipo: &'static str,
    pub mensagem: String,
}

impl ErroDeConsulta {
    pub fn implementacao(m: impl Into<String>) -> Self {
        ErroDeConsulta { tipo: "implementacao", mensagem: m.into() }
    }
    pub fn inesperado(m: impl Into<String>) -> Self {
        ErroDeConsulta { tipo: "inesperado", mensagem: m.into() }
    }
}

/// Quem responde às consultas de uma execução (o hospedeiro, sobre o
/// `elements`). Cada resposta é registrada por quem a serve (cache por
/// digest, docs/MACROS-PROTOCOLO.md §6).
pub trait ServicoDeConsultas {
    fn consultar(&mut self, tipo: &str, args: &Value) -> Result<Value, ErroDeConsulta>;
}

/// Uma execução: a instância, a fase, o alvo e o modelo da pré-busca.
#[derive(Debug, Clone)]
pub struct PedidoDeExecucao {
    pub instancia: u64,
    pub fase: Fase,
    pub alvo: Value,
    pub modelo: Value,
}

/// O executor, do ponto de vista do hospedeiro. Um processo por sessão,
/// persistente (regra governante, item 3).
pub trait ExecutorMacros: Send {
    fn disponibilidade(&self) -> Disponibilidade;
    /// Handshake; devolve as macros do bootstrap.
    fn iniciar(&mut self) -> Result<Apresentacao, String>;
    /// `macro` é `uri#Classe`; `argumentos` é `{posicionais, nomeados}`.
    /// Devolve a instância e os nomes das interfaces de macro que ela
    /// implementa (`ClassDeclarationsMacro`…), que decidem as fases.
    fn instanciar(&mut self, macro_: &str, construtor: &str, argumentos: &Value) -> Result<(u64, Vec<String>), String>;
    fn executar(&mut self, pedido: &PedidoDeExecucao, consultas: &mut dyn ServicoDeConsultas) -> Result<Resultado, String>;
    fn encerrar(&mut self);
}

/// A implementação do produto hoje: recusa com o motivo.
pub struct Indisponivel(pub String);

impl Default for Indisponivel {
    fn default() -> Self {
        Indisponivel(
            "o DartForge ainda não executa macros: o executor é o backend nativo (docs/MACROS-PROTOCOLO.md §3), que \
             ainda não compila o que a API de macros usa; para materializar a augmentation, use `dartforge macros \
             --materializar --dart <executável dart>`"
                .into(),
        )
    }
}

impl ExecutorMacros for Indisponivel {
    fn disponibilidade(&self) -> Disponibilidade {
        Disponibilidade::Indisponivel(self.0.clone())
    }
    fn iniciar(&mut self) -> Result<Apresentacao, String> {
        Err(self.0.clone())
    }
    fn instanciar(&mut self, _: &str, _: &str, _: &Value) -> Result<(u64, Vec<String>), String> {
        Err(self.0.clone())
    }
    fn executar(&mut self, _: &PedidoDeExecucao, _: &mut dyn ServicoDeConsultas) -> Result<Resultado, String> {
        Err(self.0.clone())
    }
    fn encerrar(&mut self) {}
}

/// O cliente `macro.*` sobre um canal. Pedidos numerados pelo hospedeiro;
/// durante uma execução, responde às `macro.consulta` do executor.
pub struct ExecutorDfexec<C: Canal> {
    canal: C,
    proximo: u64,
    iniciado: bool,
}

impl<C: Canal> ExecutorDfexec<C> {
    pub fn novo(canal: C) -> Self {
        ExecutorDfexec { canal, proximo: 1, iniciado: false }
    }

    /// O canal (para os testes lerem a gravação).
    pub fn canal(&self) -> &C {
        &self.canal
    }

    /// Devolve o canal (para embrulhá-lo, por exemplo num gravador).
    pub fn into_canal(self) -> C {
        self.canal
    }

    fn id(&mut self) -> u64 {
        let n = self.proximo;
        self.proximo += 1;
        n
    }

    /// Espera a resposta `t` ao pedido `id`, respondendo às consultas no
    /// caminho.
    fn esperar(&mut self, id: u64, t: &str, consultas: &mut dyn ServicoDeConsultas) -> Result<Value, String> {
        loop {
            let m = self.canal.receber()?;
            match m.get("t").and_then(Value::as_str) {
                Some("macro.consulta") => {
                    let cid = m.get("id").cloned().unwrap_or(Value::Null);
                    let tipo = m.get("tipo").and_then(Value::as_str).unwrap_or("");
                    let args = m.get("args").cloned().unwrap_or(Value::Null);
                    let resposta = match consultas.consultar(tipo, &args) {
                        Ok(valor) => json!({"t": "macro.resposta", "id": cid, "valor": valor}),
                        Err(e) => json!({"t": "macro.resposta", "id": cid, "erro": {"tipo": e.tipo, "mensagem": e.mensagem}}),
                    };
                    self.canal.enviar(&resposta)?;
                }
                Some("erro") => {
                    let msg = m.get("mensagem").and_then(Value::as_str).unwrap_or("");
                    return Err(format!("executor: {msg}"));
                }
                Some(x) if x == t && m.get("id").and_then(Value::as_u64) == Some(id) => return Ok(m),
                _ => return Err(format!("mensagem inesperada do executor (esperava {t} {id}): {m}")),
            }
        }
    }
}

struct SemConsultas;
impl ServicoDeConsultas for SemConsultas {
    fn consultar(&mut self, tipo: &str, _: &Value) -> Result<Value, ErroDeConsulta> {
        Err(ErroDeConsulta::implementacao(format!("consulta {tipo} fora de uma execução")))
    }
}

impl<C: Canal> ExecutorMacros for ExecutorDfexec<C> {
    fn disponibilidade(&self) -> Disponibilidade {
        Disponibilidade::Disponivel
    }

    fn iniciar(&mut self) -> Result<Apresentacao, String> {
        self.canal.enviar(&ola_do_hospedeiro())?;
        let r = conferir_ola(&self.canal.receber()?)?;
        self.iniciado = true;
        Ok(r)
    }

    fn instanciar(&mut self, macro_: &str, construtor: &str, argumentos: &Value) -> Result<(u64, Vec<String>), String> {
        let id = self.id();
        self.canal.enviar(&json!({"t": "macro.instanciar", "id": id, "macro": macro_, "construtor": construtor, "argumentos": argumentos}))?;
        let r = self.esperar(id, "macro.instancia", &mut SemConsultas)?;
        let instancia = r.get("instancia").and_then(Value::as_u64).ok_or_else(|| format!("resposta sem instância: {r}"))?;
        let interfaces = r
            .get("interfaces")
            .and_then(Value::as_array)
            .map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default();
        Ok((instancia, interfaces))
    }

    fn executar(&mut self, p: &PedidoDeExecucao, consultas: &mut dyn ServicoDeConsultas) -> Result<Resultado, String> {
        let id = self.id();
        self.canal.enviar(&json!({"t": "macro.executar", "id": id, "instancia": p.instancia, "fase": p.fase.nome(),
            "alvo": p.alvo, "modelo": p.modelo}))?;
        let r = self.esperar(id, "macro.resultado", consultas)?;
        resultado_de_json(r.get("resultado").ok_or("resultado ausente")?)
    }

    fn encerrar(&mut self) {
        if self.iniciado {
            let _ = self.canal.enviar(&json!({"t": "fim"}));
            let _ = self.canal.receber();
            self.iniciado = false;
        }
    }
}

// ------------------------------------------------------------ gravação

/// Um canal que grava tudo o que passa (`>` saiu do hospedeiro, `<` chegou
/// do executor), para reproduzir a sessão sem o executor.
pub struct CanalGravador<C: Canal> {
    pub interno: C,
    pub sessao: Vec<(char, Value)>,
}

impl<C: Canal> Canal for CanalGravador<C> {
    fn enviar(&mut self, m: &Value) -> Result<(), String> {
        self.sessao.push(('>', m.clone()));
        self.interno.enviar(m)
    }
    fn receber(&mut self) -> Result<Value, String> {
        let m = self.interno.receber()?;
        self.sessao.push(('<', m.clone()));
        Ok(m)
    }
}

/// O executor falso: reproduz uma sessão gravada. Confere que cada
/// mensagem do hospedeiro é **igual** à gravada — o modelo, os pedidos e as
/// respostas às consultas —, e devolve as do executor na ordem. É o teste
/// de protocolo sem executor nenhum.
pub struct CanalGravado {
    sessao: std::collections::VecDeque<(char, Value)>,
}

impl CanalGravado {
    pub fn novo(sessao: Vec<(char, Value)>) -> Self {
        CanalGravado { sessao: sessao.into() }
    }

    /// Uma linha por mensagem: `> {json}` ou `< {json}`.
    pub fn de_texto(texto: &str) -> Result<Self, String> {
        let mut v = Vec::new();
        for (n, l) in texto.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
            let (dir, resto) = l.split_at(1);
            let dir = dir.chars().next().unwrap_or(' ');
            if dir != '>' && dir != '<' {
                return Err(format!("linha {}: direção inválida", n + 1));
            }
            v.push((dir, serde_json::from_str(resto.trim()).map_err(|e| format!("linha {}: {e}", n + 1))?));
        }
        Ok(CanalGravado::novo(v))
    }

    /// O que sobrou sem ser consumido.
    pub fn restante(&self) -> usize {
        self.sessao.len()
    }
}

/// A gravação em texto (o formato de [`CanalGravado::de_texto`]).
pub fn sessao_em_texto(sessao: &[(char, Value)]) -> String {
    let mut s = String::new();
    for (d, m) in sessao {
        s.push(*d);
        s.push(' ');
        s.push_str(&m.to_string());
        s.push('\n');
    }
    s
}

impl Canal for CanalGravado {
    fn enviar(&mut self, m: &Value) -> Result<(), String> {
        match self.sessao.pop_front() {
            Some(('>', esperado)) if &esperado == m => Ok(()),
            Some(('>', esperado)) => Err(format!("o hospedeiro mandou\n  {m}\nmas a sessão gravada tem\n  {esperado}")),
            Some(('<', r)) => Err(format!("o hospedeiro mandou {m} onde a sessão gravada espera receber {r}")),
            _ => Err(format!("o hospedeiro mandou {m} depois do fim da sessão gravada")),
        }
    }
    fn receber(&mut self) -> Result<Value, String> {
        match self.sessao.pop_front() {
            Some(('<', m)) => Ok(m),
            Some((_, m)) => Err(format!("o hospedeiro esperava receber, mas a sessão gravada manda {m}")),
            None => Err("fim da sessão gravada".into()),
        }
    }
}
