# Incremento 22 — fases e cache limitado de macros

`MacroSession` executa barreiras de Types, Declarations e Definitions por unidade.
JsonCodable não cria tipos na primeira fase. A segunda valida todos os alvos e
reserva assinaturas; a terceira preenche os corpos. Somente depois do sucesso de
todas as fases as alterações são publicadas. Erros mantêm a AST original intacta.

A transação copia apenas classes aumentadas. Funções e classes sem a anotação não
são clonadas. A tabela de tipos recebe a nova forma de Map somente ao publicar.
Não há cópia de AST nem alocação de origens quando nenhuma macro é aplicada.

O cache guarda planos owned, sem referências à fonte, spans ou IDs da compilação
anterior. Compara nomes, tipos, ordem e finalidade dos campos e política do
construtor. Colisões e validade da classe são verificadas mesmo nos hits. Cada
expansão materializa novos nós e intervalos de origem.

Há limites de entradas e payload, expulsão LRU, desativação por orçamento zero,
estatísticas e limpeza explícita. O payload contabilizado não equivale ao RSS do
processo. A busca atual é linear no conjunto limitado de planos; um hit não é uma
promessa de menor latência para schemas pequenos.

`CompilerSession` mantém esse cache separado do cache de JavaScript completo.
Editar um corpo pode reutilizar o plano sem reutilizar a saída. Alterar campos
invalida o schema. Erros limpam a sessão para não servir uma saída anterior.
`macro-info` informa fases, nós materializados, origens e contadores da expansão.

O benchmark `cargo bench -p dartforge-compiler --bench macros` mede pipeline sem
retenção, plano aquecido, miss com alteração de schema e expansão isolada. Registra
amostras, mediana e p95 de médias por lote; não mede hot reload de uma IDE nem
compara DDC/dart2js. Não pressupõe ganho de desempenho.

A macro ainda é incorporada ao compilador Rust. Execução arbitrária de macros
escritas em Dart, workers isolados e sintaxe geral `augment` permanecem pendentes.
Consulte [a arquitetura](MACROS-ARQUITETURA.md) para os contratos desses próximos
passos. O recurso experimental não depende da reativação das macros upstream.
