//! Cliente do serviço `build.*` do `dfexec/1`. O canal e o enquadramento são
//! os mesmos usados pelas macros; o executor real pode atender ambos.
use crate::executor::{Disponibilidade, ErroExecutor, ExecutorDart, Nivel, PedidoAcao, ResultadoAcao, ScriptDeBuilders, ServicoBuildStep};
use crate::grafo::AssetId;
use crate::valor::{Mapa, Valor};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use dartforge_dfexec::{Canal, CanalDeProcesso, PROTOCOLO};
use serde_json::{Value, json};
use std::sync::Arc;

pub struct ClienteBuild<C: Canal> {
    canal: C,
    proximo: u64,
    iniciado: bool,
    carregado: bool,
    falha_de_preparo: Option<String>,
}

impl ClienteBuild<CanalDeProcesso> {
    /// Inicia um processo compatível. O handshake e o carregamento ficam
    /// para a primeira ação Dart; o motor deve construir este cliente sob
    /// demanda para manter o processo quente só quando for necessário.
    pub fn iniciar(comando: std::process::Command) -> Result<Self, String> {
        CanalDeProcesso::iniciar(comando).map(Self::novo)
    }
}

impl<C: Canal> ClienteBuild<C> {
    pub fn novo(canal: C) -> Self {
        Self { canal, proximo: 1, iniciado: false, carregado: false, falha_de_preparo: None }
    }

    pub fn canal(&self) -> &C { &self.canal }

    fn id(&mut self) -> u64 {
        let id = self.proximo;
        self.proximo += 1;
        id
    }

    fn receber(&mut self, id: u64, esperado: &str, servico: &mut dyn ServicoBuildStep) -> Result<Value, ErroExecutor> {
        loop {
            let mensagem = self.canal.receber().map_err(ErroExecutor)?;
            let tipo = mensagem.get("t").and_then(Value::as_str).unwrap_or("");
            if matches!(tipo, "build.ler" | "build.existe" | "build.glob" | "build.digest" |
                "build.escrever" | "build.log" | "build.resolver") || tipo.starts_with("build.resolver.") {
                let resposta = match self.servir(&mensagem, servico) {
                    Ok(resposta) => resposta,
                    Err(erro) => {
                        let pedido = mensagem.get("id").and_then(Value::as_u64)
                            .ok_or_else(|| ErroExecutor(format!("consulta build sem id: {mensagem}")))?;
                        json!({"t":"build.resposta","id":pedido,"erro":erro.0})
                    }
                };
                self.canal.enviar(&resposta).map_err(ErroExecutor)?;
                continue;
            }
            if mensagem.get("id").and_then(Value::as_u64) != Some(id) {
                return Err(ErroExecutor(format!("resposta fora de ordem do executor: {mensagem}")));
            }
            if tipo == "erro" {
                return Err(ErroExecutor(mensagem.get("mensagem").and_then(Value::as_str).unwrap_or("erro sem mensagem").into()));
            }
            if tipo != esperado {
                return Err(ErroExecutor(format!("esperava {esperado}, veio {mensagem}")));
            }
            return Ok(mensagem);
        }
    }

    fn servir(&self, m: &Value, s: &mut dyn ServicoBuildStep) -> Result<Value, ErroExecutor> {
        let tipo = m.get("t").and_then(Value::as_str).unwrap_or("");
        let id = m.get("id").and_then(Value::as_u64).ok_or_else(|| ErroExecutor("consulta build sem id".into()))?;
        let asset = || m.get("asset").and_then(Value::as_str).and_then(AssetId::de_texto)
            .ok_or_else(|| ErroExecutor(format!("{tipo}: AssetId inválido")));
        let mut resposta = json!({"t":"build.resposta","id":id});
        match tipo {
            "build.ler" => {
                let asset = asset()?;
                match s.ler(&asset) {
                    Some(bytes) => resposta["bytes_base64"] = json!(STANDARD.encode(bytes)),
                    None => resposta["erro"] = json!("AssetNotFound"),
                }
            }
            "build.existe" => resposta["sim"] = json!(s.can_read(&asset()?)),
            "build.glob" => {
                let padrao = m.get("padrao").and_then(Value::as_str).ok_or_else(|| ErroExecutor("glob sem padrão".into()))?;
                resposta["assets"] = json!(s.find_assets(padrao).into_iter().map(|a| a.texto()).collect::<Vec<_>>());
            }
            "build.digest" => {
                resposta["hex"] = s.digest(&asset()?).map(|d| d.iter().map(|b| format!("{b:02x}")).collect::<String>()).map_or(Value::Null, Value::String);
            }
            "build.escrever" => {
                let asset = asset()?;
                let bytes = m.get("bytes_base64").and_then(Value::as_str).ok_or_else(|| ErroExecutor("escrever sem bytes".into()))?;
                let bytes: Arc<[u8]> = STANDARD.decode(bytes).map_err(|e| ErroExecutor(format!("base64 inválido: {e}")))?.into();
                if s.escrever(&asset, bytes).is_err() { resposta["erro"] = json!("SaidaNaoPermitida"); }
            }
            "build.log" => {
                let nivel = match m.get("nivel").and_then(Value::as_str).unwrap_or("info") {
                    "fino" => Nivel::Fino, "aviso" => Nivel::Aviso, "severo" => Nivel::Severo, _ => Nivel::Info,
                };
                s.log(nivel, m.get("mensagem").and_then(Value::as_str).unwrap_or(""));
            }
            "build.resolver" => resposta["erro"] = json!("indisponivel"),
            t if t.starts_with("build.resolver.") => resposta["erro"] = json!("indisponivel"),
            _ => return Err(ErroExecutor(format!("consulta desconhecida: {tipo}"))),
        }
        Ok(resposta)
    }

    fn handshake(&mut self) -> Result<(), ErroExecutor> {
        self.canal.enviar(&json!({"t":"ola","protocolo":PROTOCOLO,"servicos":["build"],"motor":crate::VERSAO})).map_err(ErroExecutor)?;
        let resposta = self.canal.receber().map_err(ErroExecutor)?;
        let servicos = resposta.get("servicos").and_then(Value::as_array);
        if resposta.get("t").and_then(Value::as_str) != Some("ola")
            || resposta.get("protocolo").and_then(Value::as_str) != Some(PROTOCOLO)
            || !servicos.is_some_and(|s| s.iter().any(|v| v.as_str() == Some("build"))) {
            return Err(ErroExecutor(format!("handshake build incompatível: {resposta}")));
        }
        self.iniciado = true;
        Ok(())
    }
}

struct SemServico;
impl ServicoBuildStep for SemServico {
    fn can_read(&mut self, _: &AssetId) -> bool { false }
    fn ler(&mut self, _: &AssetId) -> Option<Arc<[u8]>> { None }
    fn find_assets(&mut self, _: &str) -> Vec<AssetId> { Vec::new() }
    fn digest(&mut self, _: &AssetId) -> Option<[u8; 32]> { None }
    fn escrever(&mut self, id: &AssetId, _: Arc<[u8]>) -> Result<(), crate::executor::SaidaNaoPermitida> { Err(crate::executor::SaidaNaoPermitida(id.clone())) }
    fn resolver(&mut self) -> Option<&mut dyn crate::executor::ServicoResolver> { None }
    fn log(&mut self, _: Nivel, _: &str) {}
}

impl<C: Canal> ExecutorDart for ClienteBuild<C> {
    fn disponibilidade(&self) -> Disponibilidade {
        self.falha_de_preparo.as_ref().map_or(Disponibilidade::Disponivel,
            |e| Disponibilidade::Indisponivel(e.clone()))
    }

    fn preparar(&mut self, script: &ScriptDeBuilders) -> Result<(), ErroExecutor> {
        if let Some(erro) = &self.falha_de_preparo { return Err(ErroExecutor(erro.clone())); }
        let resultado: Result<(), ErroExecutor> = (|| {
            if !self.iniciado { self.handshake()?; }
            let id = self.id();
            let aplicacoes: Vec<_> = script.aplicacoes.iter().map(|(chave, import, fabricas)|
                json!({"chave":chave,"import":import,"fabricas":fabricas})).collect();
            self.canal.enviar(&json!({"t":"build.carregar","id":id,"script":{"aplicacoes":aplicacoes,"chave_de_cache":script.chave_de_cache}})).map_err(ErroExecutor)?;
            self.receber(id, "build.carregado", &mut SemServico)?;
            self.carregado = true;
            Ok(())
        })();
        if let Err(e) = &resultado { self.falha_de_preparo = Some(e.0.clone()); }
        resultado
    }

    fn executar(&mut self, p: &PedidoAcao, servico: &mut dyn ServicoBuildStep) -> Result<ResultadoAcao, ErroExecutor> {
        if !self.carregado { return Err(ErroExecutor("build.executar antes de build.carregar".into())); }
        let id = self.id();
        self.canal.enviar(&json!({"t":"build.executar","id":id,"fase":p.fase,"chave":p.chave,
            "fabrica":p.fabrica,"opcoes":mapa_json(&p.opcoes),"isRoot":p.raiz,"entrada":p.entrada.texto(),
            "saidas_permitidas":p.saidas_permitidas.iter().map(AssetId::texto).collect::<Vec<_>>()
        })).map_err(ErroExecutor)?;
        let resposta = self.receber(id, "build.resultado", servico)?;
        let mut saidas = Vec::new();
        let lista = resposta.get("saidas").and_then(Value::as_array)
            .ok_or_else(|| ErroExecutor("build.resultado sem lista de saídas".into()))?;
        for saida in lista {
            let asset = saida.get("asset").and_then(Value::as_str).and_then(AssetId::de_texto)
                .ok_or_else(|| ErroExecutor("resultado com AssetId inválido".into()))?;
            let bytes = saida.get("bytes_base64").and_then(Value::as_str)
                .ok_or_else(|| ErroExecutor("resultado sem bytes".into()))?;
            saidas.push((asset, Arc::from(STANDARD.decode(bytes).map_err(|e| ErroExecutor(e.to_string()))?)));
        }
        let logs = resposta.get("logs").and_then(Value::as_array).into_iter().flatten().map(|log|
            (log.get("nivel").and_then(Value::as_str).unwrap_or("info").to_string(),
             log.get("mensagem").and_then(Value::as_str).unwrap_or("").to_string())).collect();
        let falhou = resposta.get("falhou").and_then(Value::as_bool)
            .ok_or_else(|| ErroExecutor("build.resultado sem indicador falhou".into()))?;
        Ok(ResultadoAcao { saidas, logs, falhou })
    }

    fn encerrar(&mut self) {
        if self.iniciado {
            let _ = self.canal.enviar(&json!({"t":"fim"}));
            let _ = self.canal.receber();
            self.iniciado = false;
            self.carregado = false;
            self.falha_de_preparo = None;
        }
    }
}

fn mapa_json(mapa: &Mapa) -> Value { valor_json(&Valor::Mapa(mapa.clone())) }

fn valor_json(valor: &Valor) -> Value {
    match valor {
        Valor::Nulo => Value::Null,
        Valor::Bool(v) => json!(v),
        Valor::Int(v) => json!(v),
        Valor::Real(v) => serde_json::from_str(v).unwrap_or_else(|_| json!(v)),
        Valor::Texto(v) => json!(v),
        Valor::Lista(v) => Value::Array(v.iter().map(valor_json).collect()),
        Valor::Mapa(v) => Value::Object(v.0.iter().filter_map(|(k, v)|
            k.como_texto().map(|k| (k.to_string(), valor_json(v)))).collect()),
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use dartforge_dfexec::Canal;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct CanalFalso { recebidas: VecDeque<Value>, enviadas: Arc<Mutex<Vec<Value>>> }
    impl Canal for CanalFalso {
        fn enviar(&mut self, m: &Value) -> Result<(), String> {
            self.enviadas.lock().unwrap().push(m.clone());
            Ok(())
        }
        fn receber(&mut self) -> Result<Value, String> {
            self.recebidas.pop_front().ok_or_else(|| "executor falso terminou".into())
        }
    }

    struct ServicoFalso { escritas: Vec<(AssetId, Arc<[u8]>)> }
    impl ServicoBuildStep for ServicoFalso {
        fn can_read(&mut self, id: &AssetId) -> bool { id.texto() == "p|lib/a.dart" }
        fn ler(&mut self, id: &AssetId) -> Option<Arc<[u8]>> {
            self.can_read(id).then(|| Arc::from(&b"fonte"[..]))
        }
        fn find_assets(&mut self, _: &str) -> Vec<AssetId> { vec![AssetId::novo("p", "lib/a.dart")] }
        fn digest(&mut self, _: &AssetId) -> Option<[u8; 32]> { None }
        fn escrever(&mut self, id: &AssetId, bytes: Arc<[u8]>) -> Result<(), crate::executor::SaidaNaoPermitida> {
            if id.texto() != "p|lib/a.g.dart" { return Err(crate::executor::SaidaNaoPermitida(id.clone())); }
            self.escritas.push((id.clone(), bytes));
            Ok(())
        }
        fn resolver(&mut self) -> Option<&mut dyn crate::executor::ServicoResolver> { None }
        fn log(&mut self, _: Nivel, _: &str) {}
    }

    #[test]
    fn handshake_carregar_e_consultas_build_no_mesmo_canal() {
        let enviadas = Arc::new(Mutex::new(Vec::new()));
        let recebidas = [
            json!({"t":"ola","protocolo":"dfexec/1","servicos":["build"],"executor":"teste","abi":"a"}),
            json!({"t":"build.carregado","id":1}),
            json!({"t":"build.ler","id":9,"asset":"p|lib/a.dart"}),
            json!({"t":"build.escrever","id":10,"asset":"p|lib/a.g.dart","bytes_base64":STANDARD.encode(b"gerado")}),
            json!({"t":"build.escrever","id":11,"asset":"p|lib/outro.dart","bytes_base64":STANDARD.encode(b"indevido")}),
            json!({"t":"build.resultado","id":2,"saidas":[],"falhou":false}),
        ].into();
        let mut cliente = ClienteBuild::novo(CanalFalso { recebidas, enviadas: enviadas.clone() });
        cliente.preparar(&ScriptDeBuilders { aplicacoes: vec![("p:b".into(), "package:p/b.dart".into(), vec!["b".into()])], chave_de_cache: "digest".into() }).unwrap();
        let mut servico = ServicoFalso { escritas: Vec::new() };
        let pedido = PedidoAcao { fase: 0, chave: "p:b".into(), fabrica: "b".into(), opcoes: Mapa::default(),
            raiz: true, entrada: AssetId::novo("p", "lib/a.dart"), saidas_permitidas: vec![AssetId::novo("p", "lib/a.g.dart")] };
        let resultado = cliente.executar(&pedido, &mut servico).unwrap();
        assert!(!resultado.falhou);
        assert_eq!(servico.escritas[0].1.as_ref(), b"gerado");
        let enviadas = enviadas.lock().unwrap();
        assert_eq!(enviadas[0]["t"], "ola");
        assert_eq!(enviadas[1]["t"], "build.carregar");
        assert_eq!(enviadas[2]["isRoot"], true);
        assert_eq!(enviadas[3]["bytes_base64"], STANDARD.encode(b"fonte"));
        assert_eq!(enviadas[4], json!({"t":"build.resposta","id":10}));
        assert_eq!(enviadas[5]["erro"], "SaidaNaoPermitida");
    }

    #[test]
    fn handshake_recusa_executores_sem_servico_build() {
        for resposta in [
            json!({"t":"ola","protocolo":"dfexec/2","servicos":["build"]}),
            json!({"t":"ola","protocolo":"dfexec/1","servicos":["macro"]}),
        ] {
            let canal = CanalFalso { recebidas: [resposta].into(), enviadas: Arc::new(Mutex::new(Vec::new())) };
            let mut cliente = ClienteBuild::novo(canal);
            assert!(cliente.preparar(&ScriptDeBuilders { aplicacoes: vec![], chave_de_cache: "x".into() }).is_err());
            assert!(matches!(cliente.disponibilidade(), Disponibilidade::Indisponivel(_)));
        }
    }

    #[test]
    fn resultado_build_incompleto_nao_vira_sucesso() {
        let canal = CanalFalso {
            recebidas: [
                json!({"t":"ola","protocolo":"dfexec/1","servicos":["build"]}),
                json!({"t":"build.carregado","id":1}),
                json!({"t":"build.resultado","id":2,"falhou":false}),
            ].into(),
            enviadas: Arc::new(Mutex::new(Vec::new())),
        };
        let mut cliente = ClienteBuild::novo(canal);
        cliente.preparar(&ScriptDeBuilders { aplicacoes: vec![], chave_de_cache: "x".into() }).unwrap();
        let pedido = PedidoAcao { fase: 0, chave: "p:b".into(), fabrica: "b".into(), opcoes: Mapa::default(),
            raiz: true, entrada: AssetId::novo("p", "lib/a.dart"), saidas_permitidas: Vec::new() };
        let mut servico = ServicoFalso { escritas: Vec::new() };
        assert!(cliente.executar(&pedido, &mut servico).is_err());
    }

    #[test]
    fn consulta_invalida_recebe_erro_sem_encerrar_acao() {
        let enviadas = Arc::new(Mutex::new(Vec::new()));
        let canal = CanalFalso {
            recebidas: [
                json!({"t":"ola","protocolo":"dfexec/1","servicos":["build"]}),
                json!({"t":"build.carregado","id":1}),
                json!({"t":"build.ler","id":9,"asset":"invalido"}),
                json!({"t":"build.escrever","id":10,"asset":"p|lib/a.g.dart","bytes_base64":"?"}),
                json!({"t":"build.existe","id":11,"asset":"p|lib/a.dart"}),
                json!({"t":"build.resultado","id":2,"saidas":[],"falhou":false}),
            ].into(),
            enviadas: enviadas.clone(),
        };
        let mut cliente = ClienteBuild::novo(canal);
        cliente.preparar(&ScriptDeBuilders { aplicacoes: vec![], chave_de_cache: "x".into() }).unwrap();
        let pedido = PedidoAcao { fase: 0, chave: "p:b".into(), fabrica: "b".into(), opcoes: Mapa::default(),
            raiz: true, entrada: AssetId::novo("p", "lib/a.dart"), saidas_permitidas: Vec::new() };
        let mut servico = ServicoFalso { escritas: Vec::new() };
        assert!(!cliente.executar(&pedido, &mut servico).unwrap().falhou);
        let enviadas = enviadas.lock().unwrap();
        assert_eq!(enviadas[3]["id"], 9);
        assert!(enviadas[3]["erro"].as_str().unwrap().contains("AssetId"));
        assert_eq!(enviadas[4]["id"], 10);
        assert!(enviadas[4]["erro"].as_str().unwrap().contains("base64"));
        assert_eq!(enviadas[5], json!({"t":"build.resposta","id":11,"sim":true}));
        assert!(servico.escritas.is_empty());
    }
}
