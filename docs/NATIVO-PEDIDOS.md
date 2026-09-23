# Pedidos entre os agentes do nativo (rodada 2)

Arquivos com dono (docs/NATIVO-PLANO.md §7.3) não são editados por outro
agente: quem precisa de uma mudança num deles a registra aqui, com o porquê e a
forma exata, e o dono a aplica (ou responde aqui por que não).

## δ → α: texto dos literais sem perda (`lower/expressoes.rs`)

**Onde:** `lower/expressoes.rs`, braço `ExprKind::String`, as duas linhas
`String::from_utf8_lossy(text.as_bytes()).to_string()` (o literal constante e
cada `StringPart::Text`).

**Por quê:** o front-end guarda o texto do literal em WTF-8
(`frontend/src/text.rs`): um surrogate solto (`'\uD83D'`) é guardado em três
bytes. O `from_utf8_lossy` troca esses três bytes por três U+FFFD, então
`'\uD83D'.length` dá 3 no nativo e 1 na VM. Desde a decisão 5 o runtime já
aceita WTF-8 em `dartforge_string_new` (o `Texto::de_wtf8` de
`runtime/src/heap.rs`), então o que falta é o texto chegar inteiro ao IR.

**Forma pedida:** `Constant::String` carregar os bytes WTF-8 do literal (por
exemplo `Constant::String(DartStr)` ou um `Vec<u8>`), e o emissor
(`llvm/mod.rs`, `emit_string_constants`) gravar esses bytes como estão — ele já
escapa byte a byte. Nenhuma mudança no runtime.

**Prioridade:** baixa — nenhum programa do corpus tem literal com surrogate
solto (o programa 04 cria os soltos com `String.fromCharCode`, que já está
certo). Morre sozinho se P5d baixar os literais pelo `_OneByteString`/
`_TwoByteString` da fonte.
