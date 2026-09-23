# Pedidos do motor de build ao `crates/gerador_ng`

O motor (`crates/build`, `docs/BUILD-MOTOR.md`) usa o `gerador_ng` só pela
API pública, pelo adaptador `crates/build/src/nativos/ng.rs` (e `sass.rs`).
Hoje isso é o **estágio A**: uma ação de pacote que chama `gerar_com_apoio`
e reanalisa o pacote inteiro a cada edição relevante. Medido no
`new_sali/frontend` (ESTADO.md, motor de build): ~520 ms de `gerar_com_apoio`
+ ~140 ms de consultas por edição de `.html`/`.scss`/`.dart` do pacote —
acima do orçamento de 500 ms da edição de componente, e é o que torna o
**estágio B** (uma ação por componente, consultas finas) pré-requisito do
aceite. O estágio B precisa dos acréscimos abaixo, **públicos e em arquivos
do `gerador_ng`** (o motor não toca os internos). Cada item diz o que o
motor faz com ele.

## 1. Geração de um arquivo

```rust
/// O que `gerar_arquivo` (hoje privado, lib.rs:517) devolve, mais as
/// consultas que a geração fez.
pub struct SaidaArquivo {
    /// Texto do `<nome>.template.dart`.
    pub template: String,
    /// Arquivos lidos (o `.dart`, o `.html` do `templateUrl`, cada `.css`/
    /// `.scss` do `styleUrls` e os módulos que o Sass abriu).
    pub entradas: Vec<PathBuf>,
    /// Saídas extras (`<nome>.css.shim.dart`), por caminho natural.
    pub extras: Vec<(PathBuf, String)>,
    /// O que a geração perguntou a outras bibliotecas (item 3).
    pub consultas: Vec<ConsultaNg>,
}

pub fn gerar_arquivo(
    pacote: &Pacote,
    fonte: &Path,
    achados: &Achados,
    resolvedor: Option<&dyn resolucao::Resolucao>,
    nomes: &mut Interner,
    indice: &Indice,
) -> Result<SaidaArquivo, Motivo>;

/// `achar` já é público; falta um atalho que leia e analise um arquivo.
pub fn analisar_arquivo(fonte: &Path, texto: &str, nomes: &mut Interner) -> Achados;
```

O motor passa a ter uma ação por `.dart` (a fábrica `templateCompiler`), com
a entrada primária no próprio arquivo, e reexecuta só a do componente
editado.

## 2. Índice de componentes incremental

```rust
impl Indice {
    pub fn novo() -> Indice;
    /// Os achados de um arquivo entram (ou substituem os anteriores).
    pub fn atualizar(&mut self, pacote: &Pacote, arquivo: &Path, achados: &Achados);
    pub fn remover(&mut self, arquivo: &Path);
    /// Hoje privado (lib.rs:600): os filhos que um componente usa.
    pub fn filhos_de(&self, comp: &componente::Componente, fonte: &Path,
                     resolvedor: Option<&dyn resolucao::Resolucao>) -> Vec<visao::Filho>;
    /// Quem declara um seletor (para a consulta `Indice("ng.seletor", s)`).
    pub fn declarante(&self, seletor: &str) -> Option<(String /*biblioteca*/, String /*classe*/)>;
}
```

`Indice::montar` (lib.rs:378) relê tudo; com `atualizar`/`remover` o motor
mantém o índice vivo na sessão e cada `.dart` mudado custa uma análise.
Um seletor que aparece ou some muda o digest do termo e acorda só quem o
consultou (o equivalente do `GlobAssetNode` do `build_runner`).

## 3. Consultas feitas

```rust
pub enum ConsultaNg {
    /// Um filho resolvido no template: seletor → biblioteca e classe.
    Filho { seletor: String, biblioteca: String, classe: String },
    /// Superfície de um tipo lida pelo `Resolucao` (injeção, tipo de membro
    /// para `interpolate*`): biblioteca que declara e nome.
    Tipo { biblioteca: String, nome: String },
    /// Um seletor procurado e não achado (a consulta negativa também conta).
    SeletorAusente(String),
}
```

O motor converte em `Consulta::Indice { espaco: "ng.seletor", .. }` e
`Consulta::Declaracao { .. }`; o digest da declaração (assinatura,
anotações, `@Input`/`@Output`, ciclo de vida, `OnPush`) é calculado pelo
banco da sessão. É o que faz "`@Input` novo num filho" reexecutar só os
pais que o consultaram, e "corpo de método em arquivo sem Angular" não
executar ação nenhuma.

## 4. Sass byte a byte

```rust
pub enum Estilo { Expandido, Comprimido }

/// Compila como o `sass_builder` 2.2.1 (dart-sass do lock): o CSS **byte a
/// byte** do oficial no estilo pedido, e os módulos abertos por `@use`/
/// `@import` (consultas). Recusa (com motivo) o que ainda não sabe igual.
pub fn sass::compilar_com(fonte: &str, dir: Option<&Path>, estilo: Estilo)
    -> Result<(String, Vec<PathBuf>), Motivo>;
```

Hoje o Sass do `gerador_ng` só é verificado depois do shim do ngdart
(`sass.rs:14-17`), então o motor o trata como **não verificado**: publica o
`.css` do apoio e só executa o nativo para medir (`dartforge build
--comparar`). Com `compilar_com` igual ao oficial nos dois estilos (o
`corpus/builders/sass_builder` e `sass_builder_compressed` são o oráculo),
o `.css` servido pelo `dartforge serve` passa a sair do nativo. O `.css.map`
de desenvolvimento (D-B3: o mesmo que o oficial, em memória) precisa também
do mapa de fontes; pode vir num segundo passo.

## O que não é pedido

Nada muda em `gerar_com_apoio`, `gerar_em`, `sass::compilar_em`,
`css::shim` e `Resolvedor`: o estágio A continua funcionando enquanto o B
não existir, e o motor troca de estágio só no adaptador. O teste
`crates/build/tests/ng_transparencia.rs` (no `ci.yml`) acusa na hora uma
mudança de assinatura pública que quebre o adaptador.
