# Referências para DartForge → LLVM → Wasm MVP

Pesquisa e downloads: **20/10/2026**, horário de São Paulo.
Diretório solicitado: `/Volumes/ssd_dock/references`.

Foram baixados **onze clones novos**, aproximadamente **700 MiB em disco**, e
catalogados três clones existentes sem atualizá-los. Todos são referências de
estudo; nenhum projeto foi incorporado ao build do DartForge. Não foram
executados instaladores, builds ou scripts dos projetos baixados.

Este guia complementa o [plano Wasm MVP](PLANO-WASM-MVP.md). As revisões completas,
URLs, datas, licenças e hashes dos arquivos de entrada estão no
[manifesto dedicado](wasm-mvp-references-manifest.json), também disponível em
`/Volumes/ssd_dock/references/wasm-mvp-references-manifest.json`.
Os recibos/logs de download estão em `wasm-mvp-download-2026-10-20/` nesse diretório.

## 1. Seleção e prioridade

| Projeto / pasta local | Situação | Principal utilidade | Fases |
|---|---|---|---|
| [LLVM](https://github.com/llvm/llvm-project), `llvm-wasm-22.1.8` | Novo, sparse, tag `llvmorg-22.1.8` | Backend WebAssembly, LLD, builtins e testes da versão da toolchain | W0–W1, W7 |
| [TinyGo](https://github.com/tinygo-org/tinygo), `tinygo-wasm` | Novo, limpo | Linguagem gerenciada → LLVM → Wasm; raízes precisas, runtime e scheduler | W1–W4, W6 |
| [AssemblyScript](https://github.com/AssemblyScript/assemblyscript), `assemblyscript` | Novo | GC em memória linear, shadow stack, allocator e referências retidas pelo host | W2–W3 |
| [Emscripten](https://github.com/emscripten-core/emscripten), `emscripten` | Novo | Pipeline LLVM/Wasm, suporte JS, crescimento de memória e event loop | W0–W2, W5–W7 |
| [wasm-tools](https://github.com/bytecodealliance/wasm-tools), `wasm-tools` | Novo | Parser/validator Rust e seleção explícita de features MVP | Todas |
| [WABT](https://github.com/WebAssembly/wabt), `wabt` | Novo | Validator/interpreter independentes, inspeção binária e corpus | W0, W3–W7 |
| [Wasmtime](https://github.com/bytecodealliance/wasmtime), `wasmtime` | Novo | Host independente, imports, memória, limites e erros | W0, W5–W7 |
| [Wasmi](https://github.com/wasmi-labs/wasmi), `wasmi` | Novo | Segundo executor, interpretado, para testar ABI e callbacks | W0, W5–W7 |
| [wasm-bindgen](https://github.com/wasm-bindgen/wasm-bindgen), `wasm-bindgen` | Novo | Conversões JS/Wasm, buffers, closures e Promise/Future | W5–W6 |
| [Binaryen](https://github.com/WebAssembly/binaryen), `binaryen` | Novo | Otimização pós-link e verificação de efeitos no tamanho/features | Depois de W3–W5 |
| [Especificação Core 1.0](https://github.com/WebAssembly/spec/tree/wg-1.0), `wasm-spec-core1` | Novo, tag `wg-1.0` | Especificação, interpreter e testes para classificar o perfil permitido | W0 e corpus |
| [SwiftWasm](https://github.com/swiftwasm/swift), `swiftwasm-swift` | Sparse existente preservado | Runtime gerenciado via LLVM, ARC e metadados de imagem Wasm | Complementar |
| [Dart SDK](https://github.com/dart-lang/sdk), `dart-sdk` | Existente preservado | Semântica Dart e comparação com o backend oficial WasmGC | W4, complementar |
| TinyGo anterior, `tinygo` | Existente preservado | Comparar a base da pesquisa anterior com o clone limpo novo | Histórico |

**Começar por LLVM + TinyGo + AssemblyScript + wasmparser/WABT.** Esse conjunto
cobre as principais lacunas locais: emissão, runtime gerenciado em memória
linear, raízes explícitas e validação binária. Depois estudar host/loader com
Wasmtime/Wasmi e Emscripten. Binaryen entra após a correção inicial.

As versões atuais desses projetos não são automaticamente configurações MVP.
Avaliar os mecanismos úteis e confrontá-los com o contrato estrito do plano.

## 2. O que ler em cada referência

Os caminhos desta seção são relativos à pasta do projeto dentro de
`/Volumes/ssd_dock/references`. Os pontos de entrada foram conferidos no disco;
arquivos individuais selecionados têm SHA-256 registrado no manifesto.

### LLVM 22.1.8: geração e ligação

Revisão `ca7933e47d3a3451d81e72ac174dcb5aa28b59d1`, a mesma versão da
instalação LLVM usada pelo projeto. Clone separado porque o checkout existente
`llvm-project-22` não disponibilizava os diretórios WebAssembly no sparse atual.
O clone antigo não foi alterado.

Ler `llvm/lib/Target/WebAssembly`, `llvm/test/CodeGen/WebAssembly`, `lld/wasm`,
`lld/test/wasm` e `compiler-rt/lib/builtins`. Estudar definição de features,
lowering de chamadas/tabelas, relocação, import/export e limites do linker.
O checkout inclui também documentação LLVM e trechos de Clang pertinentes.
Não contém todo o monorepo nem garante um build LLVM completo sem expandir o sparse.

**Resultado esperado do estudo:** definir quais opções e testes fixam o perfil
MVP no gerador embutido, no Clang alternativo e no módulo ligado.

### TinyGo: referência mais próxima do pipeline gerenciado

Clone limpo `tinygo-wasm`, revisão
`4b74579deb287cbc5607426ff40f7eae2e67c5a0`. O clone anterior `tinygo`, base
`f6d269f74ab45bfa53a37c5708c6661ec33531ae`, tem alterações locais preservadas.
Ler `transform/gc.go`, `src/runtime/gc_precise.go`,
`src/runtime/runtime_wasm_js.go`, `targets/wasm.json`,
`targets/wasm-unknown.json` e `targets/wasm_exec.js`.

Estudar a construção das raízes globais/locais no compilador, o coletor, o contexto
de execução e a fronteira com JS. Confrontar os arquivos de target com o runtime
e os imports de fato emitidos.

**Limite observado:** nesse snapshot, `wasm.json` usa triple WASI, GC preciso e
suporte JS, enquanto `wasm-unknown.json` seleciona `gc: leaking`. Portanto, este
último não demonstra GC funcional do perfil sem WASI, e o nome `wasm` não basta
para inferir nosso contrato portátil/MVP. Semântica Go não substitui semântica Dart.
O target `wasm-unknown` declara CPU `mvp`, mas acrescenta `nontrapping-fptoint`
e `sign-ext`: esse arquivo também demonstra por que CPU nominal não prova
conformidade MVP estrita do conjunto final de features.

### AssemblyScript: GC próprio sem depender do GC do engine

Ler `std/assembly/rt/README.md`, `itcms.ts`, `tcms.ts`, `tlsf.ts` e
`index-incremental.ts` nessa mesma pasta.
O runtime descreve mark-and-sweep incremental, shadow stack e allocator TLSF;
também expõe retenção/liberação externa por pin/unpin.

**Resultado esperado:** comparar desenho de raízes, coleta durante execução,
barreiras e referências retidas pelo host com os contratos do runtime DartForge.
É uma referência de runtime/GC, não de LLVM: o compilador AssemblyScript tem
outra arquitetura e outro modelo de tipos/objetos.

### Emscripten: loader e limites entre Wasm e JavaScript

Ler `site/source/docs/compiling/WebAssembly.rst`, `src/preamble.js`,
`src/runtime_init_memory.js`, `src/lib/libcore.js` e `src/lib/libeventloop.js`.

Focar em inicialização, views da memória após crescimento, passagem de buffers,
exports, timers e diferenças entre módulo com suporte JS e standalone.
Isso ajuda a especificar a ABI de host do DartForge e seus adaptadores.

As APIs C/C++ e convenções do runtime Emscripten não precisam virar dependências
do nosso runtime. Recursos modernos e transformações assíncronas exigem
classificação; a saída padrão não é evidência de MVP estrito ou ausência de WASI.

### wasm-tools/wasmparser: gate MVP explícito em Rust

Ler `crates/wasmparser/src/features.rs`, `validator.rs`,
`crates/wasm-encoder` e `tests`.

**Achado concreto:** o snapshot define `WasmFeatures::MVP` separadamente de
`WasmFeatures::WASM1`; este último acrescenta `MUTABLE_GLOBAL`. O nome interno
`GC_TYPES` presente na máscara MVP não significa habilitar a proposta WasmGC:
o comentário do código explica que é uma chave interna de representação.

**Resultado esperado:** um validator do artefato final com perfil explícito e
controles negativos. Nem `default()` nem desativar indiscriminadamente todas
as flags equivale automaticamente ao conjunto MVP com tipos numéricos válidos.
Versão e configuração do parser precisam entrar na evidência de validação.

### WABT: segunda verificação independente

Ler `include/wabt/feature.def`, `src/tools/wasm-validate.cc`,
`src/tools/wasm-interp.cc` e `test`.
Usar como referência para inspeção, execução de fixtures e comparação com o
validator Rust. Em `feature.def`, import/export de globais mutáveis tem flag
própria e está habilitado por padrão nesse snapshot: configurar o perfil.

Os fontes e testes foram baixados; as ferramentas não foram construídas ou
instaladas nesta pesquisa. A presença do repositório não significa gate já ativo.

### Wasmtime e Wasmi: hosts independentes

No Wasmtime, ler `examples/hello.rs`, `examples/memory.c`,
`crates/wasmtime/src/runtime/linker.rs`, `memory.rs` nessa pasta e
`crates/wasmtime/src/config.rs`.
No Wasmi, ler `crates/wasmi/src/linker.rs`, `engine/config.rs` nessa pasta e
`crates/wasmi/tests/integration/{host_call_instantiation,reextract_memory}.rs`.

Comparar ligação de imports, acesso à memória, erros, limites e chamadas do host.
São candidatos a hospedar nossa ABI comum sem tornar WASI obrigatório.
Wasmtime é referência de engine/embedding, não do nosso frontend LLVM; Wasmi
oferece uma execução interpretada independente para detectar dependência acidental
de comportamento específico de um engine. Fixar features e limites em ambos.

### wasm-bindgen: desenho do adaptador JS

Ler `crates/cli-support/src/js/mod.rs`, `binding.rs` na mesma pasta,
`crates/futures/src/lib.rs` e `tests`.

Estudar conversão/cópia de strings e buffers, lifetime, retenção de objetos do
host, closures e agendamento Promise/Future. Trata-se de referência para a
fronteira, sem obrigação de adotar o formato de metadados do wasm-bindgen.
Os modos atuais podem usar referência JS/externref e outros recursos além do
perfil: o adaptador MVP próprio deve preservar buffers/handles portáteis.

### Binaryen: otimização posterior

Ler `src/wasm/wasm-validator.cpp`, `src/passes` e `test`.
Avaliar redução de código e transformações somente após ter corpus correto e
baseline de tempo/tamanho. Uma transformação pode mudar imports, custo de
execução ou requisitos de features; revalidar o módulo final e a semântica.
Não tornar Binaryen requisito inicial para esconder um problema do lowering LLVM.

### Especificação e testes Core 1.0

Pasta `wasm-spec-core1`, tag `wg-1.0`, revisão
`977f97014c962f7bd1291fcc6d28b41a924882bf`. O `document/core/conf.py` declara
versão **1.0**. Ler `document/core`, `interpreter` e `test/core`.

Durante a seleção, a tag `v1.0.0` foi inspecionada e seu conteúdo declarou Core
2.0/Spectec. Ela não foi mantida como a referência Core 1.0; os recibos preservam
a seleção inicial e sua correção. O commit escolhido contém manutenção posterior
das ferramentas da especificação; sua data não representa a data original do MVP.

Core 1.0 e MVP estrito ainda precisam de distinção explícita, como no caso de
import/export de globais mutáveis. Classificar cada fixture; não adotar a suíte
inteira como se todos os recursos estivessem permitidos pelo perfil desejado.

### Complementos existentes: SwiftWasm e Dart SDK

- SwiftWasm: `stdlib/public/runtime/ImageInspectionWasm.cpp`, `HeapObject.cpp`
  e `RefCount.cpp` na mesma pasta. Útil para organização de runtime, ARC e
  metadados em imagem Wasm. Checkout sparse preservado, não um build SwiftWasm
  completo nem um coletor tracing pronto para Dart.
- Dart SDK: `pkg/dart2wasm/lib/{translator,target,closures}.dart`. Útil para
  representar semântica Dart e investigar diferenças. A revisão local usa
  instruções como `struct.new`/`ref.cast`; não é um pipeline LLVM/MVP e não deve
  ser transplantada como implementação do plano. Tampouco é automaticamente
  o oráculo fixado de Dart 3.6.2.

## 3. Revisões, integridade e limites do download

- Clones novos são shallow (`--depth 1`) com `--filter=blob:none`; o checkout
  dos fontes selecionados está disponível localmente. Expandir histórico ou
  sparse deliberadamente caso uma investigação posterior precise deles.
- LLVM usa sparse checkout. Os outros dez clones novos têm checkout normal.
- Submódulos não foram inicializados. O manifesto registra seus estados; builds
  futuros podem exigir downloads adicionais de ferramentas/dependências.
- TinyGo, SwiftWasm e Dart SDK existentes foram consultados sem fetch/reset ou
  mudança de branch. O manifesto registra revisão, limpeza dos arquivos rastreados
  e hashes dos arquivos selecionados, inclusive quando houver alterações locais.
  Os três apresentam diferenças locais: HEAD identifica sua base, não todos os
  bytes examinados. O TinyGo principal deste guia é o clone limpo `tinygo-wasm`.
- Licenças/NOTICE foram preservados e seus hashes registrados. Resumos no
  manifesto são por projeto; componentes vendorizados podem ter outros termos.
  As referências não passam a ter licença MIT por serem estudadas pelo DartForge.
- Nenhum benchmark de desempenho, build desses projetos ou execução Dart→Wasm
  foi realizado nesta tarefa. Apenas pesquisa, download e conferência dos fontes.

Os manifestos gerais antigos de referências foram preservados. Este recorte tem
manifesto próprio, para não alterar revisões históricas nem confundir downloads
de hoje com snapshots utilizados em validações anteriores.

## 4. Próximos estudos ligados ao plano

1. **W0:** confrontar os testes LLVM 22.1.8, o perfil `WasmFeatures::MVP` e as
   flags do WABT; selecionar controles positivos/negativos de conformidade.
2. **W1:** comparar ABI/layout e ligação LLVM/TinyGo com `Ref`, ponteiros e
   tabelas do DartForge; especificar as diferenças antes de alterar o emissor.
3. **W2–W3:** comparar raízes do TinyGo e shadow stack/allocator do AssemblyScript
   com o heap/GC existente, incluindo referências mantidas pelo host.
4. **W5:** especificar imports comuns e consultar Emscripten/wasm-bindgen para
   implementar o adaptador JS e Wasmtime/Wasmi para o host independente.
5. **W6–W7:** testar agendamento/erros e só então estudar Binaryen e distribuição.

Esses passos continuam planejados; o download das referências não implementa
nenhuma fase do backend.
