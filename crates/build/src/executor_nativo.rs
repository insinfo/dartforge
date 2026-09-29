//! O executor de builders **nativo** (B01, `docs/BUILD-RUST.md` Fase 4): o
//! mesmo *bootstrap* e o mesmo `package_config.json` do executor pela VM
//! ([`crate::vm`]), compilados pelo backend nativo do próprio DartForge
//! (`dartforge compile-native`, o `emit_native`) num executável que fala
//! `dfexec/1` por stdio e atende o serviço `build.*` com o lado Dart de
//! `pacotes/build_executor`. Nenhuma VM Dart, nenhum `dart` e nenhum
//! `build_runner` entram no caminho: o processo nasce na primeira ação Dart da
//! sessão e fica quente até [`ExecutorDart::encerrar`], como o da VM.
//!
//! O executável fica em cache em `.dart_tool/dartforge/build/` (o mesmo
//! diretório do bootstrap da VM: os imports relativos do plano valem sem
//! reescrita), como `executor-<chave>`, pela
//! chave do script, das fontes do executor, do `package_config.json` e do
//! compilador (caminho, tamanho e data). Ao lado dele, um *depfile* com cada
//! arquivo fora do SDK que o programa do bootstrap carrega (o equivalente ao
//! `--depfile` do `dart compile kernel`): um deles mais novo que o executável,
//! ou sumido, compila de novo. É também o código dos builders que o motor
//! registra ([`ExecutorDart::codigo`]).
//!
//! Antes de compilar, o hospedeiro confere que o `package_config.json` do
//! projeto resolve os pacotes de que o executor precisa
//! ([`DEPENDENCIAS`]); os que faltam (o projeto não precisa usar
//! `build_runner` nem `build_resolvers`, DF-BUILD-004) vêm do cache do pub
//! ([`crate::dependencias_executor`]), e o que o cache não tem é
//! diagnosticado pelo nome.
use crate::cliente::ClienteBuild;
use crate::dependencias_executor::DEPENDENCIAS;
use crate::executor::{
    Disponibilidade, ErroExecutor, ExecutorDart, ExtensoesDeExecucao, PedidoAcao, PedidoExtensoes,
    ResultadoAcao, ScriptDeBuilders, ServicoBuildStep,
};
use crate::vm::{Gravados, deps_do_depfile, gravar_bootstrap};
use dartforge_dfexec::CanalDeProcesso;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// Variável de ambiente do executor nativo: `0` o desliga onde ele seria o
/// padrão (`dartforge build`); outro valor o liga também nos compiladores.
pub const VARIAVEL: &str = "DARTFORGE_BUILD_NATIVO";

/// Quem compila e onde.
#[derive(Debug, Clone)]
pub struct ConfigNativa {
    /// O `dartforge` com o backend nativo (a feature `nativo` da CLI).
    pub compilador: PathBuf,
    /// O `lib/` do SDK para o `compile-native` (`--sdk`); `None` usa o que
    /// o compilador descobre.
    pub sdk_lib: Option<PathBuf>,
    /// Diretório do pacote raiz: o processo roda nele, como o `build_runner`.
    pub raiz: PathBuf,
    /// O `package_config.json` do projeto.
    pub package_config: PathBuf,
    /// Onde gravar bootstrap, pacote do executor e executável.
    pub trabalho: PathBuf,
}

impl ConfigNativa {
    /// A configuração padrão de um projeto: `.dart_tool/package_config.json`
    /// e trabalho em `.dart_tool/dartforge/build` (a mesma profundidade do
    /// `.dart_tool/build/entrypoint` do oficial, como a VM).
    ///
    /// ```
    /// use dartforge_build::executor_nativo::ConfigNativa;
    /// use std::path::Path;
    /// let c = ConfigNativa::do_projeto("dartforge".into(), None, Path::new("/p"));
    /// assert!(c.trabalho.ends_with(".dart_tool/dartforge/build"));
    /// ```
    pub fn do_projeto(compilador: PathBuf, sdk_lib: Option<PathBuf>, raiz: &Path) -> ConfigNativa {
        ConfigNativa {
            compilador,
            sdk_lib,
            raiz: raiz.to_path_buf(),
            package_config: raiz.join(".dart_tool").join("package_config.json"),
            trabalho: raiz.join(".dart_tool").join("dartforge").join("build"),
        }
    }
}

/// Os pacotes de [`DEPENDENCIAS`] que o `package_config.json` não resolve.
///
/// # Erros
///
/// O arquivo não pode ser lido ou não é um `package_config`.
pub fn dependencias_ausentes(package_config: &Path) -> Result<Vec<&'static str>, String> {
    let texto = std::fs::read_to_string(package_config)
        .map_err(|e| format!("{}: {e}", package_config.display()))?;
    let v: serde_json::Value =
        serde_json::from_str(&texto).map_err(|e| format!("{}: {e}", package_config.display()))?;
    let nomes: Vec<&str> = v["packages"]
        .as_array()
        .ok_or_else(|| format!("{}: sem 'packages'", package_config.display()))?
        .iter()
        .filter_map(|p| p["name"].as_str())
        .collect();
    Ok(DEPENDENCIAS
        .iter()
        .map(|(n, _)| *n)
        .filter(|d| !nomes.contains(d))
        .collect())
}

/// Data de modificação, se o arquivo existe.
fn mtime(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

/// O executável ainda vale? Existe, e nenhum arquivo que o programa do
/// bootstrap carrega é mais novo que ele (nem sumiu).
fn executavel_valido(exe: &Path, depfile: &Path) -> bool {
    let (Some(t), Ok(texto)) = (mtime(exe), std::fs::read_to_string(depfile)) else {
        return false;
    };
    let deps = deps_do_depfile(&texto);
    !deps.is_empty() && deps.iter().all(|d| mtime(d).is_some_and(|m| m <= t))
}

/// Escreve um depfile do Ninja (`saida: dep…`, espaço escapado com `\`).
fn depfile(saida: &Path, deps: &[PathBuf]) -> String {
    let esc = |p: &Path| {
        p.to_string_lossy()
            .replace('\\', "\\\\")
            .replace(' ', "\\ ")
    };
    let mut s = format!("{}:", esc(saida));
    for d in deps {
        s.push(' ');
        s.push_str(&esc(d));
    }
    s.push('\n');
    s
}

/// Os arquivos fora do SDK que o programa do bootstrap carrega (a mesma
/// carga do `compile-native`).
fn arquivos_do_programa(
    principal: &Path,
    pc: &Path,
    sdk_lib: Option<&Path>,
) -> Result<Vec<PathBuf>, String> {
    let dir = sdk_lib
        .map(Path::to_path_buf)
        .or_else(dartforge_elements::sdk::SdkLayout::discover)
        .ok_or(
            "SDK do Dart não encontrado: defina DARTFORGE_SDK_LIB (o lib/ do SDK) ou DART_SDK",
        )?;
    let sdk = dartforge_elements::sdk::SdkLayout::load(&dir, "vm")?;
    let mut nomes = dartforge_intern::Interner::new();
    let (programa, _) =
        dartforge_elements::load::load_lenient(principal, &sdk, Some(pc), &mut nomes);
    let mut v: Vec<PathBuf> = programa
        .units
        .iter()
        .filter(|u| !programa.library(u.library).is_sdk)
        .filter_map(|u| u.path.clone())
        .collect();
    v.push(pc.to_path_buf());
    v.sort();
    v.dedup();
    Ok(v)
}

/// O que o executor nativo mediu (para o relatório e os testes).
#[derive(Debug, Clone, Default)]
pub struct MedicaoNativa {
    /// Tempo do `compile-native` (zero quando o executável foi reaproveitado).
    pub compilacao: Duration,
    /// Do início do processo ao `build.carregado`.
    pub inicio: Duration,
    pub executavel_reaproveitado: bool,
}

/// O executor nativo: compila (ou reaproveita) o executável na primeira
/// ação Dart da sessão, cria o processo e o mantém quente.
pub struct ExecutorNativo {
    cfg: ConfigNativa,
    cliente: Option<ClienteBuild<CanalDeProcesso>>,
    /// Chave do executável que o processo corrente executa.
    chave: Option<String>,
    falha: Option<String>,
    pub medicao: MedicaoNativa,
}

impl ExecutorNativo {
    pub fn novo(cfg: ConfigNativa) -> ExecutorNativo {
        ExecutorNativo {
            cfg,
            cliente: None,
            chave: None,
            falha: None,
            medicao: MedicaoNativa::default(),
        }
    }

    /// O executável do bootstrap de `script`, compilado agora ou do cache,
    /// e a chave dele.
    ///
    /// # Erros
    ///
    /// Dependências do executor que o projeto não resolve, escrita no
    /// diretório de trabalho ou a compilação (a mensagem do compilador vai
    /// no erro).
    pub fn compilar(&mut self, script: &ScriptDeBuilders) -> Result<(PathBuf, String), String> {
        let t = &self.cfg.trabalho;
        std::fs::create_dir_all(t).map_err(|e| format!("{}: {e}", t.display()))?;
        let do_projeto = self.package_config_completo()?;
        let Gravados {
            principal,
            package_config: pc,
            mut hasher,
        } = gravar_bootstrap(t, &do_projeto, script)?;
        let meta = std::fs::metadata(&self.cfg.compilador)
            .map_err(|e| format!("{}: {e}", self.cfg.compilador.display()))?;
        let data = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos());
        hasher.update(
            format!("{}|{}|{data}\n", self.cfg.compilador.display(), meta.len()).as_bytes(),
        );
        if let Some(s) = &self.cfg.sdk_lib {
            hasher.update(s.to_string_lossy().as_bytes());
        }
        let chave = hasher.finalize().to_hex()[..16].to_string();
        let exe = t.join(format!("executor-{chave}{}", std::env::consts::EXE_SUFFIX));
        let dep = t.join(format!("executor-{chave}.d"));
        self.medicao = MedicaoNativa::default();
        if executavel_valido(&exe, &dep) {
            self.medicao.executavel_reaproveitado = true;
            return Ok((exe, chave));
        }
        let t0 = Instant::now();
        let provisorio = t.join(format!(
            "executor-{chave}.tmp{}",
            std::env::consts::EXE_SUFFIX
        ));
        let mut cmd = std::process::Command::new(&self.cfg.compilador);
        // O bootstrap mora no projeto que usa builders: a compilação dele não
        // passa pelo motor (que compilaria este executor de novo).
        cmd.env("DARTFORGE_BUILD_COMPILANDO_EXECUTOR", "1");
        cmd.arg("compile-native")
            .arg(&principal)
            .arg("-o")
            .arg(&provisorio)
            .arg("--packages")
            .arg(&pc);
        if let Some(s) = &self.cfg.sdk_lib {
            cmd.arg("--sdk").arg(s);
        }
        let saida = cmd.output().map_err(|e| {
            format!(
                "não foi possível executar {}: {e}",
                self.cfg.compilador.display()
            )
        })?;
        if !saida.status.success() || !provisorio.is_file() {
            return Err(format!(
                "a compilação nativa do executor de builders falhou:\n{}{}",
                String::from_utf8_lossy(&saida.stdout),
                String::from_utf8_lossy(&saida.stderr)
            ));
        }
        std::fs::rename(&provisorio, &exe).map_err(|e| format!("{}: {e}", exe.display()))?;
        let deps = arquivos_do_programa(&principal, &pc, self.cfg.sdk_lib.as_deref())?;
        std::fs::write(&dep, depfile(&exe, &deps))
            .map_err(|e| format!("{}: {e}", dep.display()))?;
        // Executáveis de chaves antigas não servem mais.
        if let Ok(ls) = std::fs::read_dir(t) {
            for e in ls.flatten() {
                let n = e.file_name().to_string_lossy().to_string();
                if n.starts_with("executor-") && !n.contains(&chave) {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
        self.medicao.compilacao = t0.elapsed();
        Ok((exe, chave))
    }

    /// O `package_config.json` que o executor usa: o do projeto quando ele
    /// resolve [`DEPENDENCIAS`]; senão uma cópia (com URIs absolutas, em
    /// `package_config.projeto.json` no diretório de trabalho) completada
    /// pelo cache do pub (DF-BUILD-004).
    ///
    /// # Erros
    ///
    /// O arquivo do projeto não pode ser lido, ou falta algum pacote que o
    /// cache do pub não tem (a mensagem diz quais).
    pub fn package_config_completo(&self) -> Result<PathBuf, String> {
        if dependencias_ausentes(&self.cfg.package_config)?.is_empty() {
            return Ok(self.cfg.package_config.clone());
        }
        let cache = crate::dependencias_executor::cache_do_pub()
            .ok_or("o cache do pub não foi encontrado (defina PUB_CACHE)")?;
        // As URIs relativas do projeto viram absolutas (o arquivo novo mora
        // em outro diretório); o pacote do executor sai de novo no bootstrap.
        let absoluto = crate::vm::package_config(&self.cfg.package_config, &self.cfg.trabalho)?;
        let (v, novos) = crate::dependencias_executor::completar(&absoluto, &cache)?;
        eprintln!(
            "executor de builders: {} vêm do cache do pub ({})",
            novos.join(", "),
            cache.display()
        );
        let destino = self.cfg.trabalho.join("package_config.projeto.json");
        let json = serde_json::to_string_pretty(&v).unwrap_or_default();
        if std::fs::read_to_string(&destino).ok().as_deref() != Some(json.as_str()) {
            std::fs::write(&destino, &json).map_err(|e| format!("{}: {e}", destino.display()))?;
        }
        Ok(destino)
    }

    /// Compila (ou reaproveita) e inicia o processo; devolve a chave.
    fn iniciar(&mut self, script: &ScriptDeBuilders) -> Result<String, String> {
        let (exe, chave) = self.compilar(script)?;
        let mut cmd = std::process::Command::new(&exe);
        cmd.current_dir(&self.cfg.raiz);
        self.cliente = Some(ClienteBuild::iniciar(cmd)?);
        Ok(chave)
    }

    /// Uma ação (de build ou de pós-processamento) pelo processo.
    fn acao(
        &mut self,
        pedido: &PedidoAcao,
        servico: &mut dyn ServicoBuildStep,
        pos: bool,
    ) -> Result<ResultadoAcao, ErroExecutor> {
        let cliente = self
            .cliente
            .as_mut()
            .ok_or_else(|| ErroExecutor("build.executar antes de build.carregar".into()))?;
        let r = if pos {
            cliente.pos_processar(pedido, servico)
        } else {
            cliente.executar(pedido, servico)
        };
        if let Err(e) = &r {
            // Canal quebrado (processo morreu, protocolo violado): a sessão
            // não usa mais este processo.
            self.falha = Some(format!("executor nativo de builders: {}", e.0));
            self.cliente = None;
            self.chave = None;
        }
        r
    }
}

impl ExecutorDart for ExecutorNativo {
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
        self.acao(pedido, servico, false)
    }

    fn codigo(&self) -> Option<Vec<PathBuf>> {
        let chave = self.chave.as_ref()?;
        let texto =
            std::fs::read_to_string(self.cfg.trabalho.join(format!("executor-{chave}.d"))).ok()?;
        let deps = deps_do_depfile(&texto);
        (!deps.is_empty()).then_some(deps)
    }

    fn entradas_pos(
        &mut self,
        pedido: &PedidoExtensoes,
    ) -> Result<Option<Vec<String>>, ErroExecutor> {
        match self.cliente.as_mut() {
            Some(c) => c.entradas_pos(pedido),
            None => Ok(None),
        }
    }

    fn pos_processar(
        &mut self,
        pedido: &PedidoAcao,
        servico: &mut dyn ServicoBuildStep,
    ) -> Result<ResultadoAcao, ErroExecutor> {
        self.acao(pedido, servico, true)
    }

    fn encerrar(&mut self) {
        if let Some(mut c) = self.cliente.take() {
            c.encerrar();
        }
        self.chave = None;
    }
}

/// Liga o executor nativo no `motor`, com o `compilador` dado (o próprio
/// `dartforge` com a feature `nativo`): o processo só nasce na primeira ação
/// Dart da sessão.
pub fn ligar(
    motor: &mut crate::Motor,
    compilador: PathBuf,
    sdk_lib: Option<PathBuf>,
    package_config: &Path,
) {
    let raiz = motor.grafo_pacotes.dir_raiz.clone();
    let cfg = ConfigNativa {
        package_config: package_config.to_path_buf(),
        ..ConfigNativa::do_projeto(compilador, sdk_lib, &raiz)
    };
    motor.definir_executor_dart(Box::new(ExecutorNativo::novo(cfg)));
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn depfile_ida_e_volta_com_espacos() {
        let deps = vec![
            PathBuf::from("/a/b.dart"),
            PathBuf::from("/c/com espaco.dart"),
        ];
        let texto = depfile(Path::new("/t/executor"), &deps);
        assert_eq!(deps_do_depfile(&texto), deps);
    }

    #[test]
    fn dependencias_que_faltam() {
        let dir = tempfile::tempdir().unwrap();
        let pc = dir.path().join("package_config.json");
        std::fs::write(
            &pc,
            r#"{"configVersion":2,"packages":[{"name":"build","rootUri":"file:///b/","packageUri":"lib/"},
            {"name":"glob","rootUri":"file:///g/","packageUri":"lib/"}]}"#,
        )
        .unwrap();
        assert_eq!(
            dependencias_ausentes(&pc).unwrap(),
            vec!["build_resolvers", "logging", "package_config"]
        );
    }
}
