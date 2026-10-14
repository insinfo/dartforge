# Plano de redução de tamanho e custo do AOT nativo

Investigação iniciada em 2026-10-14 por solicitação do proprietário. Este plano
acompanha implementação e evidências; não declara paridade ou ganhos ainda não medidos.

## Base reproduzível

- File Explorer, mesmo conjunto de 11.652 arquivos: DartForge tracing otimizado
  **28.475.840 bytes**, upstream Dart 3.6.2 **11.682.880 bytes**: 2,44×.
- Produtor DartForge congelado: SHA-256
  `539086b6334ac61ddbaea956ddf9218aca83d0435eb54633119f9318a88987d7`.
- Binário DartForge: `d40a2312da012fcc34e8e409567e4828ce3bbf6321474985c5ffdeca5b3ce740`.
- Binário upstream: `184bb06ef7e6710d57b5c2fa58df89c18e4efed53a85911cd98087aa8409fc17`.
- A repetição com relatório de retenção produziu exatamente o mesmo SHA do
  binário DartForge. Instrumentação da compilação não alterou esse artefato.
- REA 6.5.0 instalado localmente em `/Volumes/ssd_dock/tools/rea`, com Node,
  JDK e Ghidra no SSD. Análise local, sem enviar executáveis a terceiros.
- O upstream anexa um snapshot ELF ao launcher Mach-O. Comparar somente
  `__TEXT,__text` do launcher com todo o código do DartForge seria incorreto.
  A análise REA deve incluir o ELF extraído, conservando offset e hashes.

Evidências: `bench/results/2026-10-14-rea-native-investigation/` e
`bench/results/2026-10-14-native-platform-specialization/sizes/comparison.json`.

## 1. Fechar a atribuição dos bytes e das funções

A atribuição por intervalos disjuntos do mapa, sem somar aliases duas vezes,
fecha os **25.273.996 bytes** de código do DartForge: `dart_ui` 15.160.360;
runtime/suporte nativo 3.430.736; `j2k` 1.647.284; suporte gerado 1.144.816;
File Explorer 804.008; restante em bibliotecas SDK.

Comparar no REA funções correspondentes nos dois binários: conversão de cor,
construtor e percurso de árvores, serialização JSON e registro de classes.
Guardar assembly, pseudocódigo, endereços, tamanhos e limites da recuperação.
Pseudocódigo C do Ghidra não é especificação da ABI privada do snapshot Dart.
Contar blocos, chamadas diretas/indiretas, verificações de exceção, manipulações
de raízes e boxing; confirmar cada categoria no emissor e no runtime.

Primeiros tamanhos por função, identificados nos símbolos reais do snapshot e
no mapa DartForge de uma recompilação com SHA idêntico ao benchmark congelado:

| Função | DartForge (bytes) | upstream (bytes) |
| --- | ---: | ---: |
| kernel `json` | 8.612 | 1.644 |
| kernel `tree` | 436 | 108 |
| `Node.count` | 688 | 220 |
| `_JsonStringifier.writeStringContent` | 13.684 | 1.360 |

Essas diferenças são expansão estática, não razões de tempo. Funções podem ter
especializações ou código inlined em outros sítios; comparar o caminho completo.

**Saída verificável:** tabela por função e categoria com intervalos reais,
sem atribuir custo de execução apenas ao tamanho estático.

## 2. Podar ramos comprovadamente mortos antes do resumo de alcance

Hoje o resumo HIR é podado antes das simplificações de constantes. Experimentar
somente dobramento literal e remoção de blocos inalcançáveis nessa fronteira,
antes de inlining, ARC e materialização de exceções. Manter todos os efeitos de
blocos alcançáveis e corrigir entradas phi removidas.

**Validação:** condição com efeitos anteriores, ramo vivo que lança, construtor
e assinatura FFI no ramo morto, loops e merges. Comparação do mesmo produtor
com/sem o passe; contar funções, seletores, trampolins e bytes realmente removidos.
Não excluir backends por nome: o dart_ui aceita seleção explícita de plataforma.

## 3. Restringir chamadas e tabelas usando prova do receptor físico

O relatório real mantém `PixelUint8.convert$tc` pelo seletor `tc:convert`
chamado por `AsciiCodec.encode$c`, que alcança a conversão de cores. Isso
evidencia fanout conservador entre domínios distintos; um único caminho no
relatório não prova ausência de outros usos legítimos.

Usar conjuntos completos de CIDs físicos para converter chamada por seletor
em chamada direta somente quando todos os CIDs comprovados escolhem o mesmo
alvo com ABI compatível. Executar antes do inlining e da análise ARC.
Conservar chamadas desconhecidas, nulidade, overrides, `noSuchMethod`,
adaptadores tipados, portas de closures/FFI e caminhos excepcionais.

**Validação:** diferenças com SDK oficial para overrides, receptores anuláveis,
interfaces, tear-offs, argumentos nomeados e callbacks. Demonstrar menos
dispatch e pares de tabela sem apagar entradas necessárias a chamadas externas.

## 4. Reduzir expansão dentro das funções

Após fechar as descompilações, atacar os padrões dominantes: boxing/conversões,
vetores de argumentos, dispatch, raízes e checks repetidos. Preferir eliminar
operações com provas HIR a trocar indiscriminadamente `-O2` por `-Oz`.

Revisar a política de inlining com custos reais do código gerado. Comparar
outlining e especialização em perfis separados; nunca deslocar chamadas cuja
posição é necessária a unwind, mapas de pilha ou rastro simbólico.

O IR preservado de `convertFormatValue` já contém **208 sítios de lookup por
seletor e 208 chamadas indiretas**, além de 174 sítios `df.box_int` e 92
`dartforge_exception_clear`, antes do LTO final. São contagens estáticas de
instruções, não contagens de execução. Revisar particularmente testes de padrões
constantes de enum: hoje passam pelo `==` geral; uma redução exige provar a
identidade canônica e a implementação efetiva da igualdade, sem retirar efeitos
de constantes comuns ou operadores redefinidos.

Na assembly real REA de `convertFormatValue`, os 43.376 bytes correspondem a
10.844 instruções ARM64, com 3.265 `BL` e 5 `BLR`. Muitas `BL` chamam helpers
extraídos pelo machine outliner, portanto não equivalem às chamadas semânticas
do IR. Um helper de contexto aparece em 150 sítios. Avaliar conjuntamente
expansão original, economia do outliner e custo dessas chamadas; a função
upstream ocupa 8.452 bytes, mas usa ABI e helpers próprios.

`Node.count` também mostra busca TLS, quadro de raízes, checks de exceção e
boxing/unboxing dos resultados de `?.`/`??`. Investigar fusão do resultado
numérico anulável com o fallback escalar e reutilização do contexto já conhecido
nas chamadas Dart internas. Qualquer alteração de passagem do contexto precisa
de ABI versionada, entradas FFI/isolados preservadas e medição de pressão de
registradores; não reservar um registrador global sem esse contrato.

**Validação:** saída/exceções idênticas, mesmo modelo de memória, tamanho e
latência medidos juntos. Um binário menor que regrede os kernels não é ganho
de desempenho; registrar explicitamente o compromisso.

## 5. Corrigir o custo de alocação e propriedade

Tracing e ARC são linhas separadas. A poda de candidatos ARC já diminuiu RSS,
mas as medições anteriores não comprovaram aceleração e mostram regressão em
árvores. O novo protótipo reutiliza a consulta de metadados existente e evita
escritas/consultas de fila para objetos sem candidato; precisa de novo produtor
e medições pareadas antes de alegar ganho.

Para ARC, refinar barreiras de empréstimo apenas com resumos completos dos
callee conhecidos: sem mutação, decremento, observação de RC, alocação ou chamada
desconhecida. Para números, especialização deve concordar em toda a família de
dispatch e manter adaptadores boxed nas fronteiras dinâmicas. Não assumir
conteúdo homogêneo de JSON/maps nem eliminar `hashCode`/`==` com efeitos.

**Validação:** stress de coleta, contagens ARC, ciclos, aliases mutáveis,
reentrância, exceções, limites de inteiros e doubles especiais; depois árvores,
strings, JSON e HTTP, incluindo RSS e contagem de alocações.

## 6. Aplicar práticas das referências com contratos equivalentes

Referências locais auditadas:

- Dart SDK `6c4009687d2c881f880127fc12b9ad7c72111d33`: TFA
  `transformer.dart` 364–399 e 692–696 para chamadas monomórficas e seletores;
  `unboxing_info.dart` para representação por família de dispatch.
- Swift `6e75592c4025239130e370877ab0fab4006b675b`:
  `ARCCodeMotion.cpp` 340–350 e 701–723 para observação de RC e interferência.
- TinyGo `f6d269f74ab45bfa53a37c5708c6661ec33531ae`:
  `interface-lowering.go` 359–377 para consumidores de metadados.
- LLVM `ca7933e47d3a3451d81e72ac174dcb5aa28b59d1`:
  `GlobalDCE.cpp` 117–180 e `WholeProgramDevirt.cpp` 9–33.

LLVM não recebe automaticamente prova de dispatch do nosso protocolo customizado.
O trabalho é fornecer fatos corretos ou transformar o HIR; copiar otimizações
de mapas do TinyGo sem preservar efeitos dos métodos Dart seria incorreto.

## 7. Aceitar cada correção por resultado reproduzível

1. Congelar produtor, fontes, SDK, flags, runtime e hashes dos artefatos.
2. Passar regressões semânticas focadas e os gates pertinentes ao código alterado.
3. Compilar os mesmos programas upstream e DartForge; preservar outputs e erros.
4. Medir tamanho por seções/intervalos e execução em janela quieta, com ordem
   alternada, aquecimento explícito e múltiplos processos.
5. Revalidar File Explorer real: renderização, navegação, retorno e fechamento.
6. Publicar ganhos e regressões por perfil, sem misturar tracing/ARC, produção/JIT
   ou SDKs diferentes. Commit e push das correções somente com evidência pertinente.

Ordem de implementação: poda antecipada → desvirtualização → redução dos padrões
de expansão confirmados no REA → propriedade/alocação → política fina de tamanho.
Investigação binária e testes focados correm em paralelo; medições de desempenho
ficam serializadas. A investigação permanece aberta até medir as correções.

## Primeiro checkpoint implementado

- Seleção explícita de SDK propagada do frontend até poda/linkagem. O CLI
  preservou uma alteração observável em SDK copiado, com ambiente conflitante,
  tanto em cache frio quanto quente; o SDK instalado permaneceu intacto.
- Poda antecipada de literais/CFG implementada: File Explorer OFF/ON do mesmo
  produtor passou de **28.475.840 para 28.459.152 bytes**, economia de **16.688
  bytes (0,059%)**. Mesmo runtime e mesmas 11.652 entradas. É ganho pequeno;
  não resolve a diferença de 2,44× para o upstream.
- `bench.dart` OFF/ON produziu binários idênticos. Não houve medição redundante
  nem alegação de aceleração desse benchmark pela poda antecipada.
- Novo File Explorer verificado visualmente: renderização, navegação, retorno
  e fechamento da janela, sem processos próprios sobreviventes. O launcher
  desacoplado não preservou o código de saída; essa limitação consta na evidência.
- Protótipo de getters passou em oito testes, com dois casos reais elegíveis
  e dois casos de factories recusados. Mantém o wrapper e ABI original; ARC
  fica excluído. **Sem integração de produção**, sem ganho de tamanho/tempo
  atribuído a esse protótipo.
- Gate completo do emissor nativo: **658 testes passaram**, incluindo ignorados;
  1.170 arquivos do escopo de código permaneceram iguais durante o gate.
- Ajuste adicional ARC remove consulta redundante de metadados na morte de
  objetos; 313 testes do runtime passaram. Velocidade desse ajuste ainda não medida.

Artefatos: `bench/results/2026-10-14-native-sdk-pre-prune/summary.json`,
`bench/results/2026-10-14-physical-getter-prototype/validation.json` e
`bench/results/2026-10-14-rea-native-investigation/rea-validation.json`.
