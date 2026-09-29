//! O perfil de versão: quais regras do `build_config` e do `build_runner`
//! valem para o projeto, pela versão que o `pubspec.lock` resolveu.
//!
//! O motor imitava só o `build_runner` 2.4.15 com o `build_config` 1.1.2. As
//! versões novas mudam a configuração aceita e o que roda:
//!
//! | versão | mudança | onde |
//! |---|---|---|
//! | `build_config` 1.2.0 | chave de topo `triggers` | `build_config.g.dart` |
//! | `build_config` 1.3.0 | `build_to` em `post_process_builders` | `builder_definition.g.dart` |
//! | `build_config` 1.3.2 | chave de builder ou alvo de outro pacote é erro | `key_normalization.dart` |
//! | `build_runner` 2.7.0 | `run_only_if_triggered` + `triggers` | `build/build.dart`, `_allowedByTriggers` |
//! | `build_runner` 2.10.1 | trigger procurado pela chave completa | CHANGELOG 2.10.1 |
//! | `build_runner` 2.14.0 | `--build-filter asset:<pacote>/<caminho>` | `build_plan/build_filter.dart` |
//! | `build_runner` 2.15.3 | chave de builder duplicada é erro | `builder_ordering.dart` |
//!
//! Sem `build_config` no lock (nenhum `build_runner`: o DF-BUILD-001 achou um
//! `build.yaml` com builders) vale o perfil 1.1.2. Sem `build_runner`, quem
//! decide se os triggers valem é o `build_config` (a chave só existe para o
//! `build_runner` a ler).
use std::cmp::Ordering;

/// Versão semântica `maior.menor.correção` (o sufixo `-dev`/`+build` é
/// ignorado na comparação: o motor só distingue versões publicadas).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Versao(pub u32, pub u32, pub u32);

impl Versao {
    /// Lê `1.2.3`, `1.2.3-dev.1` ou `1.2.3+4`.
    ///
    /// ```
    /// use dartforge_build::perfil::Versao;
    /// assert_eq!(Versao::ler("2.16.1"), Some(Versao(2, 16, 1)));
    /// assert_eq!(Versao::ler("2.6.0-dev.2"), Some(Versao(2, 6, 0)));
    /// assert_eq!(Versao::ler("x"), None);
    /// ```
    pub fn ler(texto: &str) -> Option<Versao> {
        let nucleo = texto.split(['-', '+']).next()?;
        let mut partes = nucleo.split('.').map(|p| p.parse::<u32>().ok());
        let v = Versao(partes.next()??, partes.next()??, partes.next()??);
        partes.next().is_none().then_some(v)
    }
}

impl std::fmt::Display for Versao {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// As regras que valem para um projeto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Perfil {
    /// Versão do `build_config` do lock (`None`: ausente, vale a 1.1.2).
    pub build_config: Option<Versao>,
    /// Versão do `build_runner` do lock.
    pub build_runner: Option<Versao>,
}

impl Default for Perfil {
    /// O perfil histórico do motor: `build_runner` 2.4.15, `build_config` 1.1.2.
    fn default() -> Self {
        Perfil {
            build_config: Some(Versao(1, 1, 2)),
            build_runner: Some(Versao(2, 4, 15)),
        }
    }
}

fn pelo_menos(v: Option<Versao>, minima: Versao) -> bool {
    v.is_some_and(|v| v.cmp(&minima) != Ordering::Less)
}

impl Perfil {
    /// O perfil das versões do lock.
    ///
    /// ```
    /// use dartforge_build::perfil::Perfil;
    /// let p = Perfil::das_versoes(Some("1.3.3"), Some("2.16.1"));
    /// assert!(p.aceita_triggers() && p.dispara() && p.filtro_asset());
    /// let antigo = Perfil::das_versoes(Some("1.1.2"), Some("2.4.15"));
    /// assert!(!antigo.aceita_triggers() && !antigo.dispara());
    /// ```
    pub fn das_versoes(build_config: Option<&str>, build_runner: Option<&str>) -> Perfil {
        Perfil {
            build_config: build_config.and_then(Versao::ler),
            build_runner: build_runner.and_then(Versao::ler),
        }
    }

    /// `build_config` ≥ 1.2.0: a chave de topo `triggers` existe (senão o
    /// `$checkKeys` a recusa como qualquer chave desconhecida).
    pub fn aceita_triggers(&self) -> bool {
        pelo_menos(self.build_config, Versao(1, 2, 0))
    }

    /// `build_config` ≥ 1.3.0: `post_process_builders.<x>.build_to`.
    pub fn aceita_build_to_pos(&self) -> bool {
        pelo_menos(self.build_config, Versao(1, 3, 0))
    }

    /// `build_config` ≥ 1.3.2: a chave de definição de builder ou de alvo
    /// escrita com outro pacote (`outro:x` no `build.yaml` de `p`) é erro.
    pub fn exige_chave_do_pacote(&self) -> bool {
        pelo_menos(self.build_config, Versao(1, 3, 2))
    }

    /// Os triggers decidem se um passo com `run_only_if_triggered: true`
    /// roda: `build_runner` ≥ 2.7.0 ou, sem `build_runner`, `build_config`
    /// ≥ 1.2.0.
    pub fn dispara(&self) -> bool {
        match self.build_runner {
            Some(v) => v >= Versao(2, 7, 0),
            None => self.aceita_triggers(),
        }
    }

    /// Entre o 2.7.0 e o 2.10.0 o `build_runner` procurava os triggers de um
    /// builder `p:p` pelo nome abreviado `p` (corrigido no 2.10.1).
    pub fn trigger_pelo_nome_abreviado(&self) -> bool {
        self.build_runner
            .is_some_and(|v| v >= Versao(2, 7, 0) && v < Versao(2, 10, 1))
    }

    /// `build_runner` ≥ 2.14.0: `--build-filter asset:<pacote>/<caminho>`.
    pub fn filtro_asset(&self) -> bool {
        pelo_menos(self.build_runner, Versao(2, 14, 0))
    }

    /// `build_runner` ≥ 2.15.3: `findBuilderOrder` recusa chave duplicada.
    pub fn recusa_chave_duplicada(&self) -> bool {
        pelo_menos(self.build_runner, Versao(2, 15, 3))
    }

    /// Texto curto para relatórios (`build_runner 2.16.1, build_config 1.3.3`).
    pub fn texto(&self) -> String {
        let v = |x: Option<Versao>| x.map_or_else(|| "ausente".to_string(), |v| v.to_string());
        format!(
            "build_runner {}, build_config {}",
            v(self.build_runner),
            v(self.build_config)
        )
    }
}
