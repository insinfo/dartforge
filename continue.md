SDK 1.24.3 clonado (22 MB, tag imutável) e a pesquisa despachada.

O princípio que você formulou virou regra de projeto
Registrei no PLANO.md, porque é a coisa mais valiosa da sua mensagem:

O DartForge pode tornar código Dart padrão mais rápido, mas não pode fazer um programa semanticamente diferente do mesmo código compilado pelo Dart oficial.

E o seu exemplo do contador é a melhor justificativa possível. Se @worker fizesse o DartForge rodar numa thread separada enquanto o dart2js roda na principal, o mesmo programa daria contador == 1 num caso e 0 no outro — memória isolada. Esse é um defeito que só aparece ao trocar de compilador, o pior tipo que existe.

Por isso a separação que você propôs está registrada como obrigatória: a anotação diz apenas "pode ser entrypoint de worker"; quem determina a semântica é WorkerIsolate.run, idêntica em todos os compiladores.

Onde está a vantagem legítima
O ganho é de compilação, não de semântica. O DartForge conhece a AST e o grafo, então gera um chunk com o fecho transitivo da função e o runtime mínimo — em vez do modelo do Dart 1, onde cada Worker carregava o programa inteiro. Esse era exatamente o problema que motivou a remoção em 2018.

A biblioteca portátil roda igual em dart2js e DDC. O que muda é o tamanho do que foi enviado pela rede.

O que pedi de crítico, não só descritivo
Isolate.spawn(funcao): um Worker nasce de URL, não de função. O Dart 1 exigia função de topo identificável pelo nome. Qual a limitação resultante no nosso desenho?
Serialização: dos quatro problemas documentados do Dart 1 — clone 10–20% mais lento, sem transferência de posse, Blob/File/ImageData não transmissíveis, inconsistência dart2js/Dartium — quais persistem hoje?
Mirrors: reflexão em execução é incompatível com tree shaking por construção. Deixei explícito que a recomendação honesta pode ser "não implementar; oferecer reflexão estática gerada em compilação" — e que, se for, precisa dizer o que se perde.
Cinco agentes ativos agora: dart:core, diretivas/prefixos, lacunas reais, aferidor e este desenho.


Sim, é boa solução — mas a proposta de macros tem duas metades, e só uma serve.

O protótipo clonado é o custo, não a solução
references/dart-macros não contém a especificação: contém a hospedagem. _macro_host, _macro_client, _macro_server, handshake.g.dart, message_grouper.dart para enquadrar mensagens, 1.949 linhas de schema JSON de protocolo. Uma macro de usuário é programa Dart completo, rodando em isolate separado, que introspecta o programa por RPC durante a compilação.

Esse transporte é exatamente o custo que o relato de janeiro de 2025 aponta, e ele colide de frente com o objetivo número um daqui. Uma consulta de introspecção que atravessa serialização, socket e desserialização não cabe no orçamento incremental que se quer defender contra o DDC.

Mas nós não precisamos dessa metade. Ninguém precisa escrever a macro de reflexão — ela é sempre a mesma. Então: herdar o contrato de fases, descartar o transporte, gerador em Rust em processo, como a JsonCodable incorporada já é.

A assimetria que torna reflexão mais difícil que serialização
@JsonCodable numa classe gera código para aquela classe. Reflexão recebe perguntas por nome, em execução: invoke('foo'), onde 'foo' pode ser calculado.

O gerador sabe sobre quais declarações emitir descritores. Não sabe quais nomes o programa vai pedir. Daí a consequência que nenhuma API boa esconde: o conjunto reflexível é declarado, nunca inferido. É por isso que package:reflectable exige capacidades explícitas em vez de reflexão total — não é limitação de implementação, é o preço de conviver com tremor de árvore.

Três regras da especificação que são ganho de incrementalidade
Ordem lexicográfica, não de fonte. A especificação diz isso e dá o motivo: reordenar membros não deve obrigar a regerar. É propriedade de invalidação de cache — a nossa preocupação central.
OmittedTypeAnnotation, inferência só na fase 3. final x = f(); não tem tipo escrito. Isso determina onde o texto de tipo do descritor é materializado; antes da fase 3 ele não existe.
Erro sombrear identificador já resolvido — evita re-resolução. Descritores vivem em namespace reservado.
A sua regra de equivalência semântica decide a forma da anotação
Se @Reflectable fosse exclusiva do DartForge, o mesmo fonte deixaria de compilar no dart2js. Isso viola a regra na forma mais dura: não é comportamento diferente, é build quebrado.

A saída é idêntica à dos isolates — a anotação vem de pacote pub que também traz builder build_runner. Sob dart2js/DDC, build_runner gera; sob DartForge, o compilador gera nativamente sem etapa de build. Mesmo programa, um constrói mais rápido. package:reflectable já tem essa forma, então o alvo é a API dele, não uma inventada aqui.

Registrei em docs/MACROS-ARQUITETURA.md e redirecionei o agente de pesquisa para referenciar em vez de duplicar — pedindo que ele me corrija se eu estiver errado sobre o reflectable, que é afirmação verificável.

----------------------

Registrado como meta em PLANO.md. Concordo com a direção — e o rust-analyzer é a prova de que ela é alcançável: LSP de uma linguagem mais difícil que Dart, em Rust, com memória incomparavelmente menor. Clonei em references/rust-analyzer.

Duas precisões que gravei junto, porque sem elas a meta é mal lida depois:

"Sem coletor de lixo" vale para a ferramenta, não para o programa. Dart tem coletor, e a sua regra de equivalência semântica exige que o alvo tenha também. crates/runtime/src/heap.rs:415 já implementa collect, com protocolo de raízes; no alvo JavaScript o coletor é o do motor. Sem isso escrito, alguém "otimiza" removendo o GC do runtime e quebra ciclos e finalizadores.

A extensão do VS Code não pode ser Rust. O host de extensões executa JavaScript. A forma certa — a do rust-analyzer — é cliente TypeScript fino que só inicia o binário, com toda a lógica no servidor Rust.

O corolário que nos obriga
Uma implementação em Rust que refizesse o mesmo grafo de ponteiros com Rc<RefCell<…>> também usaria gigabytes. Não ganhamos por escolher Rust; ganhamos por escolher arena, interning e índices — Rust só torna a escolha disponível.

Onde já estamos bem: o AST empresta &'a str da fonte em 54 pontos, nunca copia string de identificador. Onde não estamos: interning/SymbolId e arena não começaram, bloqueados exatamente nesses pontos.

O estado real das quatro peças
Peça	Estado
Compilador	existe, é o que estamos medindo
Analisador	crates/semantic, funcional
LSP	22 linhas, sem transporte, sem JSON-RPC, sem sincronização
Extensão	não existe
E o LSP é a peça mais difícil por razão estrutural: um compilador em lote monta a arena e joga fora; um LSP mantém modelo vivo e mutável entre teclas, e não se libera uma declaração de dentro de uma arena. Foi essa pressão que produziu o desenho do analyzer do Dart.

Achei um bloqueio concreto antes de qualquer transporte
crates/lsp/src/lib.rs já expõe diagnose() -> Vec<Diagnostic> — mas chama compile, que devolve Result<String, Diagnostic>. Um erro. O vetor nunca pode ter mais de um elemento.

Então recuperação de erro no parser é pré-requisito do LSP, não recurso posterior. E ela destrava uma segunda coisa: o docs/CORPUS-REAL.md registra que o parser para no primeiro erro de cada arquivo, e é por isso que "uma correção real pode não mover o total de aceitos". Com todos os diagnósticos, a priorização fica confiável.

Despachei um agente para isso — é o único item que serve às duas metas ao mesmo tempo. O benchmark de memória ainda está compilando; dou o número quando sair, em vez de teorizar.


PLANO.md atualizado e enviado (bd61cf3). Fundamentei no código real, e a leitura mudou em relação ao que eu esperava.

Já estamos certos em quatro pontos — e isso precisa de teste que trave
CompilerSession retém exatamente um snapshot (session.rs:45, cached: Option<Cached>, 16 MiB). Uma compilação substitui a anterior, então o lote é à prova de crescimento por construção.
O cache de macros já é LRU com limite duplo e evictions observáveis (cache.rs:30). É o molde a copiar.
A expansão de macro é em memória. O ciclo do build_runner — gerar arquivo, reanalisar o gerado — não existe aqui.
O AST não copia string de identificador.
Dois achados concretos
DiskCache não tem limite de entradas (disk.rs:43). Cache sem teto é a causa mecânica mais comum de retenção monotônica.

O desenho de snapshot único não escala para o LSP. Option<Cached> funciona porque há uma compilação por vez. Um LSP tem N arquivos abertos × M versões — e o desenho atual não diz quem descarta a versão N−1. É exatamente aí que os 6 GB entram.

O portão
O teste de platô — N edições sucessivas, live_bytes estabilizando — vem antes de tudo. crates/instrument já mede; falta o teste.

Coloquei como regra: sem ele, nenhum outro item pode ser declarado concluído. Porque "usa menos memória" sem teste é anedota, e a propriedade que trava a sua máquina é precisamente a que só um teste de repetição detecta.

A lista, em ordem
#	Item	Estado
1	Teste de platô	falta
2	Teto e despejo em todo cache	DiskCache sem teto
3	Snapshot por documento no LSP	não desenhado
4	Interning / SymbolId	não começou
5	Recuperação de erro	em andamento
6	Transporte JSON-RPC, didChange incremental	22 linhas de stub
7	Consultas memoizadas + despejo do cache delas	não começou
8	Emissão modular por biblioteca	não começou
9	Extensão VS Code	não existe
10	Medir RSS contra webdev no mesmo projeto	bloqueado
O item 10 continua bloqueado: cargo bench --bench incremental termina com código 101 e sem saída. Nenhum número de memória do DartForge foi medido, e não vou comparar com os 10 GB do webdev até isso funcionar.