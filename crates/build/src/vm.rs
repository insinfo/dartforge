//! O executor de builders pela **VM Dart** (`docs/BUILD-PROTOCOLO.md` §3):
//! um processo persistente que fala `dfexec/1` e atende o serviço `build.*`
//! com o lado Dart de `pacotes/build_executor`. É o caminho provisório da
//! Fase 4 (BUILD-RUST.md): o mesmo protocolo que o executor auto-hospedado
//! vai falar, com a VM oficial no lugar do nosso runtime. Por isso é opcional
//! e explícito (`dartforge build --dart <exe>`, ou `DARTFORGE_BUILD_DART`):
//! sem ele o motor continua com o executor [`crate::executor::Indisponivel`].
//!
//! O hospedeiro gera, em `.dart_tool/dartforge/build/` do projeto:
//!
//! * o pacote `dartforge_build_executor` (as fontes vêm embutidas no binário);
//! * um `package_config.json` que é o do projeto mais esse pacote — o projeto
//!   usa `build_runner`, então `build`, `build_resolvers` e as dependências
//!   deles já estão lá, nas versões do lock;
//! * o *bootstrap* (`bootstrap.dart`): o equivalente ao
//!   `.dart_tool/build/entrypoint/build.dart` do oficial, com os mesmos
//!   imports das fábricas e um mapa chave → fábrica, no mesmo nível de
//!   diretório (os imports relativos do plano valem sem reescrita);
//! * um *kernel* compilado uma vez (`dart compile kernel`), reaproveitado
//!   entre sessões enquanto nenhum arquivo do `--depfile` mudar.
use crate::cliente::ClienteBuild;
use crate::executor::{
    Disponibilidade, ErroExecutor, ExecutorDart, ExtensoesDeExecucao, PedidoAcao, PedidoExtensoes,
    ResultadoAcao, ScriptDeBuilders, ServicoBuildStep,
};
use dartforge_dfexec::CanalDeProcesso;
use serde_json::{Value, json};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// Nome do pacote Dart do executor no `package_config.json` gerado.
pub const PACOTE: &str = "dartforge_build_executor";

/// Fontes do lado Dart, embutidas: (caminho em `lib/`, texto).
const FONTES: &[(&str, &str)] = &[
    (
        "executor.dart",
        include_str!("../../../pacotes/build_executor/lib/executor.dart"),
    ),
    (
        "src/canal.dart",
        include_str!("../../../pacotes/build_executor/lib/src/canal.dart"),
    ),
    (
        "src/servico.dart",
        include_str!("../../../pacotes/build_executor/lib/src/servico.dart"),
    ),
];

/// Variável de ambiente que liga o executor pela VM fora do `dartforge build`.
pub const VARIAVEL: &str = "DARTFORGE_BUILD_DART";

/// O executável `dart` pedido pelo ambiente ([`VARIAVEL`]), se houver.
pub fn dart_do_ambiente() -> Option<PathBuf> {
    std::env::var_os(VARIAVEL)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Onde estão a VM e o projeto.
#[derive(Debug, Clone)]
pub struct ConfigDaVm {
    /// O executável `dart`.
    pub dart: PathBuf,
    /// Diretório do pacote raiz (o processo roda nele, como o `build_runner`:
    /// o `build_resolvers` guarda o resumo do SDK em `.dart_tool/`).
    pub raiz: PathBuf,
    /// O `package_config.json` do projeto.
    pub package_config: PathBuf,
    /// Onde gravar bootstrap, pacote do executor e kernel.
    pub trabalho: PathBuf,
}

impl ConfigDaVm {
    /// A configuração padrão de um projeto: `.dart_tool/package_config.json`
    /// e trabalho em `.dart_tool/dartforge/build` (mesma profundidade do
    /// `.dart_tool/build/entrypoint` do oficial).
    ///
    /// ```
    /// use dartforge_build::vm::ConfigDaVm;
    /// use std::path::Path;
    /// let c = ConfigDaVm::do_projeto("dart".into(), Path::new("/p"));
    /// assert!(c.trabalho.ends_with(".dart_tool/dartforge/build"));
    /// ```
    pub fn do_projeto(dart: PathBuf, raiz: &Path) -> ConfigDaVm {
        ConfigDaVm {
            dart,
            raiz: raiz.to_path_buf(),
            package_config: raiz.join(".dart_tool").join("package_config.json"),
            trabalho: raiz.join(".dart_tool").join("dartforge").join("build"),
        }
    }
}

/// Literal de texto Dart entre aspas simples.
fn literal(s: &str) -> String {
    let mut r = String::with_capacity(s.len() + 2);
    r.push('\'');
    for c in s.chars() {
        match c {
            '\\' => r.push_str("\\\\"),
            '\'' => r.push_str("\\'"),
            '$' => r.push_str("\\$"),
            '\n' => r.push_str("\\n"),
            '\r' => r.push_str("\\r"),
            c => r.push(c),
        }
    }
    r.push('\'');
    r
}

/// O texto do bootstrap: imports das fábricas (como o `build.dart` do
/// oficial os escreve) e o mapa chave → fábrica.
///
/// ```
/// use dartforge_build::executor::ScriptDeBuilders;
/// use std::path::Path;
/// let s = ScriptDeBuilders {
///     aplicacoes: vec![("p:b".into(), "package:p/builder.dart".into(), vec!["b".into()])],
///     chave_de_cache: "x".into(),
/// };
/// let texto = dartforge_build::vm::bootstrap(&s, Path::new("/p/.dart_tool/package_config.json"));
/// assert!(texto.contains("import 'package:p/builder.dart' as _i0;"));
/// assert!(texto.contains("'p:b': {'b': _i0.b}"));
/// ```
pub fn bootstrap(script: &ScriptDeBuilders, package_config: &Path) -> String {
    let mut imports: Vec<&str> = Vec::new();
    for (_, import, _) in &script.aplicacoes {
        if !imports.contains(&import.as_str()) {
            imports.push(import);
        }
    }
    let mut s = String::from(
        "// Gerado pelo DartForge: bootstrap do executor de builders (dfexec/1, serviço build.*).\n\
         // ignore_for_file: directives_ordering, no_leading_underscores_for_library_prefixes\n\
         import 'package:dartforge_build_executor/executor.dart' as _df;\n",
    );
    for (i, imp) in imports.iter().enumerate() {
        s.push_str(&format!("import {} as _i{i};\n", literal(imp)));
    }
    s.push_str("\nFuture<void> main() => _df.servir({\n");
    for (chave, import, fabricas) in &script.aplicacoes {
        let i = imports.iter().position(|x| x == import).unwrap_or(0);
        let mapa: Vec<String> = fabricas
            .iter()
            .map(|f| format!("{}: _i{i}.{f}", literal(f)))
            .collect();
        s.push_str(&format!("  {}: {{{}}},\n", literal(chave), mapa.join(", ")));
    }
    s.push_str(&format!(
        "}}, packageConfig: {});\n",
        literal(&package_config.to_string_lossy())
    ));
    s
}

/// URI `file:` de um diretório (termina em `/`), absoluto.
fn uri_de_diretorio(p: &Path) -> String {
    let abs = std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
    url::Url::from_directory_path(&abs)
        .map(|u| u.to_string())
        .unwrap_or_else(|_| {
            format!(
                "file:///{}/",
                abs.to_string_lossy()
                    .replace('\\', "/")
                    .trim_start_matches('/')
            )
        })
}

/// O `package_config.json` do projeto (URIs relativas viram absolutas, pois
/// o arquivo novo mora em outro diretório) com o pacote do executor.
///
/// # Erros
///
/// O arquivo do projeto não pode ser lido ou não é um `package_config`.
pub fn package_config(do_projeto: &Path, executor: &Path) -> Result<Value, String> {
    let texto = std::fs::read_to_string(do_projeto)
        .map_err(|e| format!("{}: {e}", do_projeto.display()))?;
    let mut v: Value =
        serde_json::from_str(&texto).map_err(|e| format!("{}: {e}", do_projeto.display()))?;
    let base = url::Url::from_directory_path(
        std::path::absolute(do_projeto.parent().unwrap_or(Path::new(".")))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|_| format!("{}: caminho sem URI", do_projeto.display()))?;
    let ps = v
        .get_mut("packages")
        .and_then(Value::as_array_mut)
        .ok_or("package_config sem 'packages'")?;
    for p in ps.iter_mut() {
        if let Some(r) = p.get("rootUri").and_then(Value::as_str)
            && let Ok(abs) = base.join(r)
        {
            p["rootUri"] = json!(abs.to_string());
        }
    }
    ps.retain(|p| p["name"] != PACOTE);
    ps.push(json!({"name": PACOTE, "rootUri": uri_de_diretorio(executor), "packageUri": "lib/", "languageVersion": "3.6"}));
    Ok(v)
}

/// Normaliza `.`/`..` sem tocar no disco.
fn normalizar(p: &Path) -> PathBuf {
    let mut r = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                r.pop();
            }
            Component::CurDir => {}
            c => r.push(c),
        }
    }
    r
}

/// Arquivos de um depfile do Ninja (`saida: dep1 dep2`, espaço escapado com
/// `\`).
fn deps_do_depfile(texto: &str) -> Vec<PathBuf> {
    let Some((_, resto)) = texto.split_once(": ") else {
        return Vec::new();
    };
    let mut v = Vec::new();
    let mut atual = String::new();
    let mut escapado = false;
    for c in resto.chars() {
        match c {
            _ if escapado => {
                atual.push(c);
                escapado = false;
            }
            '\\' => escapado = true,
            ' ' | '\n' | '\r' | '\t' => {
                if !atual.is_empty() {
                    v.push(PathBuf::from(std::mem::take(&mut atual)));
                }
            }
            c => atual.push(c),
        }
    }
    if !atual.is_empty() {
        v.push(PathBuf::from(atual));
    }
    v
}

fn mtime(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

/// O kernel ainda vale? Existe, e nenhum arquivo que a compilação leu é
/// mais novo que ele (nem sumiu).
fn kernel_valido(dill: &Path, depfile: &Path) -> bool {
    let (Some(t), Ok(texto)) = (mtime(dill), std::fs::read_to_string(depfile)) else {
        return false;
    };
    let deps = deps_do_depfile(&texto);
    !deps.is_empty() && deps.iter().all(|d| mtime(d).is_some_and(|m| m <= t))
}

/// O que o executor mediu (para o relatório e os testes).
#[derive(Debug, Clone, Default)]
pub struct MedicaoVm {
    /// Tempo de `dart compile kernel` (zero quando o kernel foi reaproveitado).
    pub compilacao: Duration,
    /// Do início do processo ao `build.carregado`.
    pub inicio: Duration,
    pub kernel_reaproveitado: bool,
}

/// O executor pela VM: cria o processo na primeira ação Dart da sessão e o
/// mantém quente até [`ExecutorDart::encerrar`].
pub struct ExecutorVm {
    cfg: ConfigDaVm,
    cliente: Option<ClienteBuild<CanalDeProcesso>>,
    /// Chave do kernel que o processo corrente carregou.
    chave: Option<String>,
    falha: Option<String>,
    pub medicao: MedicaoVm,
}

impl ExecutorVm {
    pub fn novo(cfg: ConfigDaVm) -> ExecutorVm {
        ExecutorVm {
            cfg,
            cliente: None,
            chave: None,
            falha: None,
            medicao: MedicaoVm::default(),
        }
    }

    /// Grava os arquivos, compila (ou reaproveita) o kernel e inicia o
    /// processo. Devolve a chave do kernel.
    fn iniciar(&mut self, script: &ScriptDeBuilders) -> Result<String, String> {
        let t = &self.cfg.trabalho;
        let erro = |p: &Path, e: std::io::Error| format!("{}: {e}", p.display());
        let pacote = t.join(PACOTE);
        let mut h = blake3::Hasher::new();
        h.update(script.chave_de_cache.as_bytes());
        for (rel, texto) in FONTES {
            let destino = pacote.join("lib").join(rel);
            if let Some(pai) = destino.parent() {
                std::fs::create_dir_all(pai).map_err(|e| erro(pai, e))?;
            }
            if std::fs::read_to_string(&destino).ok().as_deref() != Some(*texto) {
                std::fs::write(&destino, texto).map_err(|e| erro(&destino, e))?;
            }
            h.update(rel.as_bytes());
            h.update(texto.as_bytes());
        }
        let pc = t.join("package_config.json");
        let json =
            serde_json::to_string_pretty(&package_config(&self.cfg.package_config, &pacote)?)
                .unwrap_or_default();
        if std::fs::read_to_string(&pc).ok().as_deref() != Some(json.as_str()) {
            std::fs::write(&pc, &json).map_err(|e| erro(&pc, e))?;
        }
        let principal = t.join("bootstrap.dart");
        let texto = bootstrap(
            script,
            &normalizar(&std::path::absolute(&self.cfg.package_config).unwrap_or_default()),
        );
        if std::fs::read_to_string(&principal).ok().as_deref() != Some(texto.as_str()) {
            std::fs::write(&principal, &texto).map_err(|e| erro(&principal, e))?;
        }
        h.update(texto.as_bytes());
        h.update(json.as_bytes());
        h.update(self.cfg.dart.to_string_lossy().as_bytes());
        let chave = h.finalize().to_hex()[..16].to_string();
        let dill = t.join(format!("bootstrap-{chave}.dill"));
        let depfile = t.join(format!("bootstrap-{chave}.d"));
        self.medicao = MedicaoVm::default();
        if kernel_valido(&dill, &depfile) {
            self.medicao.kernel_reaproveitado = true;
        } else {
            let t0 = Instant::now();
            let provisorio = t.join(format!("bootstrap-{chave}.dill.tmp"));
            let saida = std::process::Command::new(&self.cfg.dart)
                .current_dir(&self.cfg.raiz)
                .arg("compile")
                .arg("kernel")
                .arg("--verbosity=error")
                .arg(format!("--packages={}", pc.display()))
                .arg(format!("--depfile={}", depfile.display()))
                .arg("-o")
                .arg(&provisorio)
                .arg(&principal)
                .output()
                .map_err(|e| {
                    format!("não foi possível executar {}: {e}", self.cfg.dart.display())
                })?;
            if !saida.status.success() {
                return Err(format!(
                    "a compilação do bootstrap de builders falhou:\n{}{}",
                    String::from_utf8_lossy(&saida.stdout),
                    String::from_utf8_lossy(&saida.stderr)
                ));
            }
            std::fs::rename(&provisorio, &dill).map_err(|e| erro(&dill, e))?;
            // Kernels de chaves antigas não servem mais.
            if let Ok(ls) = std::fs::read_dir(t) {
                for e in ls.flatten() {
                    let n = e.file_name().to_string_lossy().to_string();
                    if n.starts_with("bootstrap-") && !n.contains(&chave) {
                        let _ = std::fs::remove_file(e.path());
                    }
                }
            }
            self.medicao.compilacao = t0.elapsed();
        }
        let mut cmd = std::process::Command::new(&self.cfg.dart);
        cmd.current_dir(&self.cfg.raiz)
            .arg(format!("--packages={}", pc.display()))
            .arg(&dill);
        self.cliente = Some(ClienteBuild::iniciar(cmd)?);
        Ok(chave)
    }
}

impl ExecutorDart for ExecutorVm {
    fn disponibilidade(&self) -> Disponibilidade {
        match &self.falha {
            Some(m) => Disponibilidade::Indisponivel(m.clone()),
            None => Disponibilidade::Disponivel,
        }
    }

    fn preparar(&mut self, script: &ScriptDeBuilders) -> Result<(), ErroExecutor> {
        if let Some(m) = &self.falha {
            return Err(ErroExecutor(m.clone()));
        }
        // Um script novo (a configuração mudou) exige outro processo.
        if self.chave.is_some() {
            self.encerrar();
        }
        let t0 = Instant::now();
        let r = self.iniciar(script).and_then(|chave| {
            let cliente = self.cliente.as_mut().ok_or("executor sem processo")?;
            cliente.preparar(script).map_err(|e| e.0)?;
            self.chave = Some(chave);
            Ok(())
        });
        self.medicao.inicio = t0.elapsed().saturating_sub(self.medicao.compilacao);
        r.map_err(|m| {
            self.cliente = None;
            self.falha = Some(m.clone());
            ErroExecutor(m)
        })
    }

    fn nova_rodada(&mut self) {
        if let Some(c) = self.cliente.as_mut() {
            c.nova_rodada();
        }
    }

    fn extensoes(
        &mut self,
        pedido: &PedidoExtensoes,
    ) -> Result<Option<ExtensoesDeExecucao>, ErroExecutor> {
        match self.cliente.as_mut() {
            Some(c) => c.extensoes(pedido),
            None => Ok(None),
        }
    }

    fn executar(
        &mut self,
        pedido: &PedidoAcao,
        servico: &mut dyn ServicoBuildStep,
    ) -> Result<ResultadoAcao, ErroExecutor> {
        let cliente = self
            .cliente
            .as_mut()
            .ok_or_else(|| ErroExecutor("build.executar antes de build.carregar".into()))?;
        let r = cliente.executar(pedido, servico);
        if let Err(e) = &r {
            // Canal quebrado (processo morreu, protocolo violado): a sessão
            // não usa mais este processo.
            self.falha = Some(format!("executor de builders pela VM: {}", e.0));
            self.cliente = None;
            self.chave = None;
        }
        r
    }

    fn encerrar(&mut self) {
        if let Some(mut c) = self.cliente.take() {
            c.encerrar();
        }
        self.chave = None;
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn depfile_com_espacos_escapados() {
        let d = deps_do_depfile("/t/x.dill: /a/b.dart /c/com\\ espaco.dart\n");
        assert_eq!(
            d,
            vec![
                PathBuf::from("/a/b.dart"),
                PathBuf::from("/c/com espaco.dart")
            ]
        );
    }

    #[test]
    fn package_config_absoluto_com_o_executor() {
        let dir = tempfile::tempdir().unwrap();
        let dt = dir.path().join(".dart_tool");
        std::fs::create_dir_all(&dt).unwrap();
        std::fs::write(
            dt.join("package_config.json"),
            r#"{"configVersion":2,"packages":[
            {"name":"p","rootUri":"../","packageUri":"lib/","languageVersion":"3.6"},
            {"name":"q","rootUri":"file:///c/q/","packageUri":"lib/"}]}"#,
        )
        .unwrap();
        let v = package_config(&dt.join("package_config.json"), &dir.path().join("exec")).unwrap();
        let ps = v["packages"].as_array().unwrap();
        assert_eq!(
            ps[0]["rootUri"],
            json!(
                url::Url::from_directory_path(std::path::absolute(dir.path()).unwrap())
                    .unwrap()
                    .to_string()
            )
        );
        assert_eq!(ps[1]["rootUri"], "file:///c/q/");
        assert_eq!(ps[2]["name"], PACOTE);
    }

    #[test]
    fn literal_dart_escapa_interpolacao() {
        assert_eq!(literal("a'b$c\\"), "'a\\'b\\$c\\\\'");
    }
}
