# Invokes ARC automáticos: prova AOT

A variante `automatico` de `arc_retornos_guardados` acrescenta `propagador`
entre o caller e `retorno_guardado`. Não fornece mapas de classes, efeitos,
invokes ou pousos do propagador. A preparação cria sua aresta excepcional,
fecha o escopo léxico 7 e insere os drops nas duas saídas. O propagador
retém um token local independente e chama o callee com esse token emprestado.

```powershell
$env:DARTFORGE_ARC_CONFERIR='1'
$env:DARTFORGE_ARC_BERCARIO='0'
$env:DARTFORGE_GC_STRESS='1'
cargo run --locked -p dartforge-emit-native --example arc_retornos_guardados -- target/prova-invokes.exe arc erro automatico
& target/prova-invokes.exe
```

Quatro combinações ARC/tracing × normal/erro terminaram com código zero.
Normal imprime `9223372036854775807` depois de liberar o resultado e coletar:
o token independente da fábrica sustenta o Mint. Erro não imprime; o callee
publica a pendência e desenrola para o pouso gerado no propagador. Este
libera seu token local e propaga ao tratador externo, que limpa a pendência
e libera o token da fábrica. O conjunto insere uma retenção de retorno e
cinco drops nos caminhos compilados; esse total não é contagem executada
por processo. IR e stdout permanecem congelados; executáveis estão em target.

O tratador externo continua explícito. A propagação usa o protocolo existente
Lanca/df.lancar; isto não certifica `resume` nativo, identidade do objeto de
unwind ou rastro em todas as plataformas, ainda exigidos pela especificação.
Não observa morte final, não prova ausência geral de vazamentos nem ganho
no runtime. A API ainda precisa ser integrada ao pipeline padrão; fechamento
de quadros proprietários, finally, dispatch indireto, cancelamento e suspensão
continuam pendentes. Os testes HIR cobrem chamadas consecutivas, transferência
apenas no sucesso, Phis, eventos em arestas, tratador prévio, IDs esgotados,
contrato ausente e rejeição atômica de conferência sem sítio preparado.
