//! `sass_builder:sass_builder` pelo Sass do `gerador_ng`
//! (`sass::compilar_ativo`, API pública), que é o dart-sass 1.102.0 byte a
//! byte (o crate `dartforge-sass`).
//!
//! Os dois estilos (`compressed` e `expanded`, o padrão do `sass_builder`)
//! e o `.css.map` quando `sourceMaps` está ligado (o `dev` o liga por
//! padrão), com o comentário `sourceMappingURL` no `.css`, como o
//! `SassBuilder` os escreve. As folhas de `@use`/`@forward`/`@import`
//! resolvem como no `BuildImporter` (relativas ao arquivo e `package:` pelas
//! raízes dos pacotes); cada arquivo lido ou procurado é dependência da ação.
use crate::consulta::Consulta;
use crate::executor::{CtxGerador, GeradorNativo, PedidoNativo, SaidaNativa};
use crate::valor::Valor;
use dartforge_gerador_ng::sass::{Estilo, Leitor, compilar_ativo};
use std::path::Path;
use std::sync::Arc;

/// O `BuildStep` para o Sass: o que o motor tem na memória (saídas de
/// builders anteriores) ou o disco.
struct LeitorDoPasso<'a> {
    memoria: &'a dyn Fn(&Path) -> Option<Arc<[u8]>>,
}

impl Leitor for LeitorDoPasso<'_> {
    fn existe(&self, caminho: &Path) -> bool {
        (self.memoria)(&dartforge_elements::gerado::chave(caminho)).is_some() || caminho.is_file()
    }

    fn ler(&self, caminho: &Path) -> std::io::Result<String> {
        match (self.memoria)(&dartforge_elements::gerado::chave(caminho)) {
            Some(b) => String::from_utf8(b.to_vec())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
            None => std::fs::read_to_string(caminho),
        }
    }
}

pub struct SassNativo;

impl GeradorNativo for SassNativo {
    fn chave(&self) -> &'static str {
        "sass_builder:sass_builder"
    }

    fn cobre(&self, _fabrica: &str) -> bool {
        true
    }

    fn por_pacote(&self) -> bool {
        false
    }

    fn verificado(&self) -> bool {
        true
    }

    fn gerar(
        &self,
        ctx: &mut CtxGerador<'_>,
        pedido: &PedidoNativo,
    ) -> Result<SaidaNativa, String> {
        let mut s = SaidaNativa::default();
        for a in &pedido.acoes {
            let nome = a.entrada.caminho.rsplit('/').next().unwrap_or_default();
            if nome.starts_with('_') {
                continue; // parcial: o oficial não gera nada (`sass_builder.dart:43`)
            }
            // `outputStyle` ausente é o `expanded` do `sass_builder`; valor
            // desconhecido ele avisa e usa o `expanded` — o aviso não temos.
            let estilo = match a.opcoes.obter("outputStyle") {
                None | Some(Valor::Nulo) => Estilo::Expandido,
                Some(Valor::Texto(v)) if v == "expanded" => Estilo::Expandido,
                Some(Valor::Texto(v)) if v == "compressed" => Estilo::Comprimido,
                Some(_) => {
                    s.recusas.insert(
                        a.entrada_natural.clone(),
                        "sass: outputStyle desconhecido (o oficial avisa)".into(),
                    );
                    continue;
                }
            };
            let Some(fonte) = ctx.ler(&a.entrada_natural) else {
                s.recusas
                    .insert(a.entrada_natural.clone(), "sass: entrada ilegível".into());
                continue;
            };
            let texto = String::from_utf8_lossy(&fonte);
            let mapa = a.opcoes.obter("sourceMaps") == Some(&Valor::Bool(true));
            let leitor = LeitorDoPasso {
                memoria: ctx.memoria,
            };
            let folha = match compilar_ativo(
                &texto,
                &a.entrada.pacote,
                &a.entrada.caminho,
                &pedido.raizes,
                &leitor,
                estilo,
                mapa,
            ) {
                Ok(f) => f,
                Err(m) => {
                    let primeira = m.lines().next().unwrap_or_default().to_owned();
                    s.recusas.insert(
                        a.entrada_natural.clone(),
                        format!("sass: o dart-sass recusa: {primeira}"),
                    );
                    continue;
                }
            };
            for arquivo in folha.lidos.iter().chain(&folha.sondados) {
                ctx.registrar(Consulta::Arquivo(dartforge_elements::gerado::chave(
                    arquivo,
                )));
            }
            if let Some((_, n)) = a.saidas.iter().find(|(id, _)| id.caminho.ends_with(".css")) {
                s.saidas.insert(n.clone(), folha.css.into_bytes());
            }
            if let Some(m) = folha.mapa
                && let Some((_, n)) = a
                    .saidas
                    .iter()
                    .find(|(id, _)| id.caminho.ends_with(".css.map"))
            {
                s.saidas.insert(n.clone(), m.into_bytes());
            }
        }
        Ok(s)
    }
}
