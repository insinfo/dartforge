# HTTP, JSON e alocações no DartForge AOT

Análise de 2026-10-13 dos quatro textos anexados, do vídeo
[I Asked Expert Programmers to Max Out a $12 Server](https://www.youtube.com/watch?v=QGWKCaglSBI)
e do [repositório do desafio](https://github.com/arjaythedev/twelve-dollar-server-challenge).
Referência consultada: `main`, revisão `026bdb68d94d0d932bb5981321ad82c70618d752`.
O vídeo foi analisado pela transcrição fornecida; o acesso direto ao YouTube falhou.
Os números do vídeo são resultados relatados pelo autor, não medidas do DartForge.

## 1. O que serve para o nosso compilador

A principal lição é reduzir trabalho por requisição: menos cópias, menos
alocações intermediárias e menos chamadas dinâmicas. AOT sozinho não elimina
a árvore de mapas do JSON nem o custo de `Stream`, `Future` e do protocolo HTTP.

| técnica | situação no DartForge | próxima ação |
| --- | --- | --- |
| enviar bytes sem uma cópia temporária completa | implementado nesta rodada em `Socket_WriteList` | medir CPU por byte e por requisição em respostas pequenas e grandes |
| JSON direto em UTF-8 | `JsonUtf8Encoder` e escrita ASCII no buffer já existem no overlay | comparar com `utf8.encode(jsonEncode(value))` no caminho HTTP |
| mapas especializados por chave | caminho rápido para `int`/`String` já existe em `natives_hash.rs` | perfilar crescimento, criação e iteração, além de lookup |
| hash guardado na string | já existe e é usado pelo caminho rápido | não implementar uma segunda tabela de hashes |
| menos objetos temporários | TLAB, poda de raízes mortas e envelhecimento do GC já existem | medir alocações por resposta e eliminar objetos que comprovadamente não escapam |
| reduzir custo das chamadas | exceções por tabelas já existem (`--exceptions=tables`) | usar a mesma configuração nos pares de benchmark e perfilar SDK HTTP |
| menos syscalls e eventos agrupados | `epoll`/`kqueue`/IOCP já existem | medir lotes do handler e despertares antes de experimentar `io_uring` |

Não há evidência nesta rodada para trocar o tracing por ARC: as medidas locais
anteriores de ARC no HTTP eram piores. Adiar a coleta indefinidamente também
não substitui reduzir alocação: um servidor é uma carga contínua e precisa
recuperar memória com limites previsíveis.

## 2. Alteração concreta: envio de sockets

Antes, `dartforge_native_Socket_WriteList` chamava `typed_list_bytes`, que copia
todos os bytes da lista/view para um `Vec<u8>`, e só então selecionava
`offset..offset+count`. Uma escrita parcial de um buffer grande podia repetir
a cópia de todo o buffer em cada tentativa, até quando o socket retornasse 0.

Agora, a função toma emprestada a fatia de `Heap::typed_bytes` durante
`write_at`. O empréstimo impede coleta no intervalo e termina antes da
construção de uma exceção. No Unix a escrita consome a fatia na chamada;
no Windows o backend mantém a cópia da fatia em sua operação overlapped.
Nenhum ponteiro para o heap passa a sobreviver à chamada nativa.

Preservados: offsets relativos à view, número de bytes efetivamente enviados,
tratamento de escrita parcial, ausência de espaço no socket e erro do SO.
Não há nova API Dart nem mudança de ABI. O cálculo de fim usa soma saturada.

O benefício demonstrado é **uma alocação temporária e uma cópia integral a
menos por chamada de envio**. Não atribuir uma porcentagem de ganho no HTTP
sem pares controlados. A cópia removida era no heap Rust, portanto não se deve
contá-la como um objeto Dart a menos no GC.

O teste `tests_socket_write_slices` usa sockets reais no Unix e cobre lista
interna, memória externa, view somente leitura com offset, contagens grandes,
envios vazios e erro do SO com construção de exceção. O Windows foi auditado
pelo código de `write_to_handle`, mas não executado neste Mac.

## 3. Dicas para a aplicação HTTP e o driver de banco

Estas decisões pertencem à aplicação/driver, não devem ser aplicadas
silenciosamente pelo compilador:

1. **Reutilizar statements preparados e conexões.** Evita preparar a mesma
   consulta por requisição. Uma conexão/escritor pode fazer sentido no teste
   com uma CPU; a escolha para mais núcleos depende da carga e do banco.
2. **Agrupar escritas com limite de tamanho e de espera.** Cada resposta deve
   esperar o commit. Uma requisição isolada precisa progredir sem esperar que
   o lote encha; rollback e cancelamento precisam de tratamento explícito.
3. **Gerar JSON no SQLite quando a resposta vem direto da consulta.**
   `json_object` e `json_group_array` podem evitar construir mapas por linha.
   Conferir ordem, escapes, tipos, `null` e resultados vazios. JSONB do SQLite
   é formato interno, não corpo JSON HTTP. Ver
   [funções JSON do SQLite](https://www.sqlite.org/json1.html).
4. **Enviar UTF-8 diretamente.** Para objetos Dart, experimentar
   `response.add(encoder.convert(value))`, com `final encoder = JsonUtf8Encoder()`
   criado uma vez, em vez de construir
   uma String JSON e convertê-la depois. Isso ainda cria mapas e bytes; o
   ganho deve ser medido. Saída em chunks pode evitar o buffer final agregado.
5. **Pools de buffers com ownership e teto.** Só devolver um buffer ao pool
   quando o consumidor terminar; `add`/um callback de chunk não autoriza
   sobrescrevê-lo imediatamente. Não reter um buffer máximo em toda conexão
   keep-alive ociosa. Medir memória por conexão além da memória por requisição.
6. **Fila limitada e backpressure.** Evitar filas sem limite e não usar
   fechamento/reconexão frequente como mecanismo normal de controle de carga.
7. **Checkpoints e `mmap` mediante perfil.** Ajustar cache do SQLite e cache do
   SO é trabalho do banco. Não copiar valores do vídeo sem medir. Com WAL,
   `synchronous=NORMAL` pode perder transações numa queda de energia; escolher
   a garantia necessária. Ver [synchronous](https://www.sqlite.org/pragma.html#pragma_synchronous)
   e [I/O mapeado](https://www.sqlite.org/mmap.html).

No teste do desafio são permitidos statements preparados, cache de páginas do
SQLite, `mmap` e group commit, mas não cache de respostas, de resultados ou de
verificações JWT entre requisições. É preciso verificar assinatura e validade
a cada chamada autenticada. Ver [regras](https://github.com/arjaythedev/twelve-dollar-server-challenge#rules)
e [contrato HTTP/JSON](https://github.com/arjaythedev/twelve-dollar-server-challenge/blob/main/SPEC.md).

## 4. O que priorizar a seguir

| prioridade | mudança candidata | evidência para decidir |
| --- | --- | --- |
| 1 | UTF-8/chunks no servidor; medir a remoção da cópia de socket | CPU por mil respostas, bytes copiados, alocações, p95/p99 |
| 2 | leitura do socket diretamente no destino Dart, implementada no §9 | cópias e tempo de `Socket_Read`; GC stress e leituras parciais |
| 3 | map literal/`toJson` temporário sem materialização, se escape e efeitos permitirem | IR e alocações; ordem de avaliação e efeitos de `toEncodable` preservados |
| 4 | crescimento/inserção/iteração de mapas JSON | perfil separado de decode e encode; chaves duplicadas, ordem e mutação |
| 5 | menos objetos e despacho no parsing HTTP, streams e futures | perfil de um servidor real, não só `GET /` |
| 6 | PGO/ThinLTO e backend opcional `io_uring` no Linux | treino representativo, latência e memória; comparação com `epoll` |

**Defeito encontrado e corrigido na continuação (§8):** a sonda
`101_json_pilha_e_mapas.dart` abortava no tratamento de
erros com `_Unwind_RaiseException returned 5`, com e sem GC stress. O mesmo
defeito foi reproduzido com o compilador anterior em `target/final/release`,
sem a alteração de sockets. Com `--exceptions=check`, a sonda produziu saída
idêntica ao Dart nos dois casos. O envio de JSON no servidor testado passou
com tabelas, mas isso não validava os caminhos de erro de decode.

A eliminação de mapas exige análise do programa: `jsonEncode` pode executar
`toJson`/`toEncodable`, detectar ciclos e produzir erros com resultado parcial.
O `writeMap` do SDK também captura chaves e valores antes de codificar. Remover
essa captura sem provar equivalência pode mudar o resultado quando um callback
muta o mapa. Um encoder Rust genérico não é substituição automática segura.

## 5. Como medir sem se enganar

Separar `GET /`, JSON pequeno, feed de 20 registros, JSON com Unicode/escapes,
decode e consultas/escritas SQLite. Comparar binários do mesmo commit com e
sem a otimização, mesmos flags, configuração de GC e conjunto de requisições.
Registrar saída correta, taxa de erros, req/s, p50/p95/p99, CPU por mil
requisições, memória por conexão e pausas do GC. Aquecer e alternar os pares.
Gerador de carga em outra máquina para a medida final; no Mac, registrar a
disputa de CPU como limitação. Testar carga baixa e alta, conexões ociosas,
cliente lento e escrita parcial.

Os 64 mil usuários do vídeo incluem pausas entre ações: **não são 64 mil req/s**.
A transcrição relata um limite do rastreamento de conexões do kernel; empatar
nesse teto não prova desempenho igual. No nosso comparativo, Go/Swift com
vários núcleos e Dart com um isolado também não são comparações por núcleo.

## 6. SSD e ambiente

A migração de Dart/Rust para `/Volumes/ssd_dock/tools` foi concluída e testada
pelo proprietário, com **6,26 GB** removidos do disco interno. Nesta rodada
foram conferidos o link `~/tools`, os executáveis no PATH e `tools/env.zsh`.
O arquivo exporta `CARGO_HOME`, `RUSTUP_HOME` e `PUB_CACHE`; shells não
interativos precisam carregá-lo explicitamente quando essas variáveis não
forem herdadas. O LLVM usado está em
`/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64`.

Builds, caches e temporários desta rodada ficam no SSD, em `target/p4`,
`target/native_cache` e `target/tmp-http-json`. Definir `TMPDIR` antes de
Cargo/compilador evita temporários grandes no disco interno. Não mover ou
apagar outros caches do usuário sem inventário e autorização específica.

## 7. Validação desta rodada

- `cargo test --locked -p dartforge-runtime -- --include-ignored`: **344 testes
  passaram** (235 unitários, 24 de integração e 85 doctests).
- `04_soquetes_http.dart`: saída idêntica à VM em tracing/ARC, com e sem
  `DARTFORGE_GC_STRESS=1`, sob `DARTFORGE_EFFECTS=check`.
- Servidor de `bench/comparison/server.dart`, AOT otimizado com tabelas:
  **1.000 respostas JSON corretas na mesma conexão keep-alive**, texto e
  encerramento também corretos. Esse smoke test não mede throughput.
- Sonda JSON/mapas `101`: paridade com a VM em `--exceptions=check`, com e
  sem stress; aborto em tabelas, também reproduzido no compilador anterior.
- `cargo doc --locked --workspace --no-deps --document-private-items`: gerou
  a documentação, com avisos existentes de links e de `unused_mut`.
- `cargo fmt --all -- --check`: falhou em formatação de arquivos existentes
  fora da alteração; o módulo de testes novo foi formatado e `git diff
  --check` passou.
- Clippy estrito do workspace: bloqueado por nove diagnósticos no frontend.
  Clippy estrito do runtime: sete diagnósticos existentes em `heap`, `space`,
  `text` e `layout`; nenhum no arquivo alterado.
- `cargo test --locked --workspace -- --include-ignored`: bloqueado na
  ligação do JIT. O ligador Apple/LLVM 21 não lê os objetos bitcode da
  distribuição LLVM 22.1.8; não é uma falha de teste de socket.

Evidência compacta em `bench/results/2026-10-13-http-json-aot/validation.json`.
Não foi feita validação Windows/Linux nem a carga SQLite completa do desafio.

## 8. Continuação: unwind do JSON na LTO do macOS arm64

O caso mínimo foi `try { jsonDecode('[1,'); } on FormatException catch ...`:
funcionava em desenvolvimento e abortava em produção. Um `throw
FormatException` direto era capturado nos dois perfis. Desligar a LTO apenas
do programa, reduzir seu nível a zero ou pedir frame pointers no módulo
`convert` não corrigia o caso. Esses controles não viraram alterações do produto.

O LLDB mostrou a passagem por `_ChunkedJsonParser.fail`, `close` e
`JsonCodec.decode`. A personalidade não era chamada. No Mach-O final,
`JsonCodec.decode` tinha encoding zero no `__unwind_info` e não tinha FDE
correspondente no `__eh_frame`. Já o objeto nativo gerado pela própria LTO
continha o FDE. Religar os mesmos objetos nativos otimizados corrigiu a
execução. A evidência local aponta para perda da associação de unwind no
caminho LTO do `ld64.lld` 22.1.8, e não para o parser JSON ou o `catch` Dart.
O mecanismo de associação pode ser consultado no
[código do LLD 22.1.8](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.8/lld/MachO/InputFiles.cpp)
e na [geração de unwind_info](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.8/lld/MachO/UnwindInfoSection.cpp).

**Correção:** para LTO no macOS arm64 com tabelas ou mapas de raízes, o
ligador retém os objetos por `-object_path_lto` e faz a ligação final a partir
dessas saídas nativas. A LTO continua fazendo a otimização entre módulos;
a segunda ligação preserva o código já gerado e reconstrói as associações
de unwind. Os outros modos mantêm a ligação usual. A saída intermediária
fica num diretório temporário ao lado do executável, e `TempDir` remove
objetos/intermediários em sucesso e em erro. Há custo de uma ligação adicional,
não uma alegação de ganho de throughput.

Adicionada `corpus/native/138_json_error_unwind.dart`: JSON inválido em String
e UTF-8, número não encodável, `catch`/`finally` e um decode válido após os erros.

Validação da correção:

- Emissor: **478 testes passaram**, 7 ignorados, com `DARTFORGE_CLANG` no LLVM
  do SSD. A primeira tentativa sem essa configuração teve 5 falhas; todas
  passaram ao configurar o compilador, sem alteração adicional de código.
- Regressão nova: tracing e ARC, produção/tabelas, com e sem GC stress,
  efeitos conferidos e duas compilações de cada modo: saída idêntica ao Dart.
- ARC preparado: regressão idêntica ao Dart com e sem GC stress, efeitos
  conferidos e `DARTFORGE_ARC_CHECK=1`.
- Sonda `101` e TCP/HTTP `04`: tabelas/produção/tracing, com e sem stress e
  efeitos conferidos, saída idêntica ao Dart em duas compilações.
- Debug: regressão nova idêntica ao Dart. Recompilação aquecida sem debug:
  binário idêntico byte a byte.
- Servidor AOT com a correção: 1.000 respostas JSON corretas por keep-alive.
- Falha injetada na segunda ligação: erro propagado e nenhum diretório
  `.df-lto-unwind-*` deixado para trás.
- As recompilações aquecidas das sondas levaram 0,3–0,4 s nesta máquina.
  Não é comparação com o desempenho de execução anterior.

O ramo de mapas de raízes e o alvo macOS x86-64 não foram executados nesta
rodada; a correção é limitada ao caminho arm64 reproduzido. Os limites da
verificação geral do workspace descritos no §7 continuam registrados.

Evidência da continuação em
`bench/results/2026-10-13-http-json-aot/unwind-validation.json`.

## 9. Continuação: menos cópias na leitura e resultado das dicas

O baseline de `Socket_Read` criava um `Vec` com o tamanho solicitado,
chamava o SO e copiava o conteúdo para uma nova lista Dart. Agora aloca a
lista Dart antes da leitura e entrega sua fatia ao SO. Quando a leitura é
completa, o resultado já está pronto; quando é curta, uma nova lista tem
exatamente o tamanho recebido. Esse é também o padrão do
[SDK 3.6.2](https://github.com/dart-lang/sdk/blob/3.6.2/runtime/bin/socket.cc#L497).

O receiver é protegido contra coleta/finalização durante a primeira
alocação, e o buffer inicial durante a alocação do resultado curto. As
fatias só vivem enquanto o heap está emprestado; erros são convertidos em
exceções após soltar o empréstimo. O destino de uma leitura curta não é
uma view que retenha a capacidade original. No Windows o buffer da operação
overlapped continua pertencendo ao backend; a cópia para o destino Dart
substitui a passagem pelo `Vec` temporário. Só o macOS foi executado aqui.

**Correção:** 345 testes do runtime passaram. O teste nativo com sockets reais
cobre contagens zero/negativas, EAGAIN/EOF, leitura completa e leitura de
2 bytes solicitada como 64 KiB sob GC stress; confere o tamanho físico do
resultado. As sondas TCP/HTTP `04` e `139` passam em tracing, ARC e ARC
preparado, com e sem stress e com efeitos conferidos. A `139` retém chunks de
um payload de 65.573 bytes, faz outras alocações e relê o conteúdo; também
passa com coleta agendada `42,3` nos três modos.

**CPU de HTTP:** o medidor usa binários já compilados, a mesma configuração,
warmup, ordem alternada e número fixo de requisições. Cada resposta é
validada; CPU inclui todas as threads do processo. Mediana de cinco rodadas:

| carga | quantidade por rodada | antes, ms/1.000 | depois, ms/1.000 |
| --- | ---: | ---: | ---: |
| GET, texto | 50.000 | 35,0 | 34,4 |
| GET, JSON pequeno | 50.000 | 37,2 | 37,2 |
| POST 64 KiB, texto | 20.000 | 54,5 | 52,0 |
| POST 64 KiB, JSON pequeno | 20.000 | 59,0 | 57,0 |

A diferença no GET é pequena, com sobreposição das faixas; não demonstrou
ganho relevante de CPU. Nos POSTs de 64 KiB, a redução foi de **4,6%** e
**3,4%**, respectivamente, com todas as rodadas depois abaixo das anteriores.
O servidor drena o corpo HTTP e produz a resposta pequena: esta carga mede
leitura de rede, não decoding de JSON de entrada. O perfil `sample` do GET
mostrou predominância do syscall `read` dentro da leitura; a cópia quase não
aparecia. Isso é consistente com a pouca diferença observada no GET.

**JSON direto em UTF-8:** `server_utf8.dart` reutiliza um `JsonUtf8Encoder`
criado antes do laço e envia `encoder.convert(...)` por `response.add`.
O formato e o contador são iguais aos do servidor original. São programas
separados, para manter o baseline sem novos branches e variáveis. Com o
runtime corrigido, cinco pares de 50.000 GETs JSON deram **38,0 → 36,0 ms**
de CPU por mil respostas (**5,3% menos**); faixas 37,6–38,2 e 36,0–36,2 ms.
O exemplo compila no Dart oficial; 110 respostas corretas passaram no Dart
AOT e no DartForge com GC stress. Esse ganho de encoder não é somável ao
ganho da leitura, que veio de outra carga.

### Procedimento e limites

```sh
source /Volumes/ssd_dock/tools/env.zsh
mkdir -p target/tmp-http-read
export TMPDIR="$PWD/target/tmp-http-read"
export DARTFORGE_CLANG=/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang
target/p4/release/dartforge compile-native bench/comparison/server_utf8.dart \
  --optimize --exceptions=tables -o target/tmp-http-read/server-bytes
python3 scripts/measure-http-pairs.py target/http-json-evidence/read/server-before \
  target/http-json-evidence/read/server-after \
  --rounds 5 --requests 20000 --request-bytes 65536 --output target/measure-upload.json
python3 scripts/measure-http-pairs.py target/http-json-evidence/read/server-after \
  target/tmp-http-read/server-bytes \
  --rounds 5 --requests 50000 --routes /json --output target/measure-utf8.json
```

Cliente local Python, uma conexão keep-alive e contador CPU `ps` com resolução
de 0,01 s no macOS. Throughput de parede é limitado pelo cliente; não é uma
medida da capacidade máxima nem do droplet Linux do vídeo. Para evitar misturar
estresse de compilação com execução, o par definitivo foi medido após as
compilações. O ganho ainda precisa de validação em vários clientes, p95/p99,
payloads JSON realistas e outros sistemas.

O medidor inicialmente usava a variável `body` para entrada e resposta,
passando a enviar a resposta na próxima iteração. Foram descartadas as
medições de upload/UTF-8 afetadas e repetidas com `request_body` e
`response_body` separados. Um controle de três requisições confirmou que
GET continua sem corpo e que todos os POSTs continuam enviando 65.536 bytes.
As rodadas GET anteriores à introdução desse parâmetro continuam válidas.

O oráculo também rejeitou o exemplo inicialmente escrito com
`const JsonUtf8Encoder()`, embora o DartForge o compilasse. O construtor não
é const; o exemplo e a recomendação foram corrigidos para usar um encoder
válido criado uma vez. Medidas da versão inválida foram descartadas. A
aceitação dessa sintaxe inválida no `compile-native` foi corrigida (§10).
Os números finais de UTF-8 usam o exemplo válido.

Dados e validação em `bench/results/2026-10-13-http-json-aot/read-validation.json`;
os resultados brutos válidos estão junto desse arquivo. Binários e perfil
locais ficam em `target/http-json-evidence/read/`, no SSD. Temporários e
bytecode Python produzidos nesta rodada foram removidos.

## 10. Diagnóstico de constantes no compile-native

A comparação com o Dart oficial revelou um problema de correção:
`const JsonUtf8Encoder()` era aceito no AOT, embora seu construtor não seja
const. O emissor agora chama `const_eval::verify`, o mesmo verificador do
analisador, antes do lowering. Não há uma exceção específica para esse encoder.
A verificação inclui bibliotecas importadas de usuário e funções não
alcançáveis; erros de constantes impedem a emissão. A inferência com vínculos
locais mantém casos válidos como `const n = 4; const c = C(n);` funcionando.
As declarações públicas do SDK preservam a distinção entre constructors const
e não const, inclusive factories cujos corpos têm patches internos.

**Validação:** 480 testes do emissor passaram, sete ignorados. Dois testes novos
cobrem chamadas explícitas/implícitas, named/factory, listas, records, aliases,
imports, encaminhamento por mixin, código não alcançável e constantes válidas
locais/do SDK. Dez exemplos inválidos também foram rejeitados pelo CLI nativo
e por `dart compile exe`, sem arquivo IR produzido pelo nativo.

Os 128 arquivos `.dart` na raiz de `corpus/native/` emitiram IR otimizado;
esta checagem não executa todos os programas nem inclui as entradas em
subdiretórios. As sondas `53_constantes_genericas_distintas`,
`120_constantes_entre_bibliotecas/main` e `131_const_no_argumento_de_fabrica`
foram executadas com tracing, ARC e ARC preparado, com e sem GC stress:
18 resultados iguais ao Dart, com efeitos/RC conferidos.

`server.dart` e `server_utf8.dart` recompilaram e validaram 110 respostas JSON
cada (100 medidas de funcionamento e dez de aquecimento). Compilações
repetidas levaram 0,369 s e 0,448 s; as primeiras, 16,689 s e 2,638 s, com
estados de cache diferentes. Esses tempos não medem o impacto isolado do
verificador. O teste curto HTTP valida correção, sem substituir as rodadas
de desempenho da seção anterior.

Evidências em `bench/results/2026-10-13-http-json-aot/const-validation.json`;
logs/fontes de controles comprimidos em `target/http-json-evidence/const/`,
no SSD. Temporários da rodada removidos. As limitações gerais de
fmt/Clippy/JIT registradas no estado do projeto permanecem. A próxima etapa
HTTP/JSON é medir criação de objetos/mapas e Stream/Future com cargas
representativas, para escolher uma otimização sustentada por perfil.
