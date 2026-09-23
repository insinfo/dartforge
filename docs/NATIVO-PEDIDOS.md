# Pedidos entre as frentes do nativo

Um pedido é algo que uma frente precisa de um arquivo que é de outra
(`docs/NATIVO-PLANO.md` §7.3). Quem pede não edita o arquivo do outro: escreve
aqui o quê, o porquê e o que muda quando for atendido. Quem atende risca a
linha e aponta o commit.

## De α (P1–P4) para a inferência (`crates/types`)

1. **Inferir os corpos que hoje ficam sem tipo nem resolução:** expressões de
   função (closures), funções locais (`StmtKind::Function`), `switch` como
   comando e como expressão (casos, guardas, corpos), declaração por padrão
   (`var (a, b) = …`), `if-case`, e as seções de uma cascata com o tipo do
   alvo (hoje o `CascadeTarget` é `dynamic`).
   *Por quê:* no nativo, tudo nesses corpos é `dynamic` — os membros vão pelo
   despacho dinâmico por nome (`lower/despacho.rs`), os operadores pelo
   `dartforge_dyn_op` (caixas), os nomes pelo escopo léxico refeito no
   lowering (`resolver_por_nome`). O resultado está certo, mas é lento e
   perde os erros de tipo; e um membro de classe do SDK cujo nome também é
   membro de uma classe do programa cai num `NoSuchMethodError` quando o
   receptor não é do programa.
   *O que muda quando atendido:* nada no contrato; o lowering passa a achar
   o tipo estático e a resolução e usa os caminhos diretos que já existem.
2. **Gravar o tipo das variáveis de padrão** em `tipos_de_locais` (pelo
   offset do nome, como as outras locais), para o local de padrão ter a
   representação do tipo (R6) e não `Ref`.

## De α para δ (runtime, P5)

1. **Records com campo nomeado.** O `Value::Record` do runtime é só
   posicional e o `render` de `saida.rs` não tem os nomes. O nativo recusa
   (diagnóstico "record com campo nomeado") o literal com campo nomeado e o
   padrão de record com campo nomeado. Com o SDK da fonte (P5) o `_Record` da
   VM traz a forma; até lá, se valer a pena: uma forma (`n_posicionais`,
   nomes) no `Value::Record`, e `==`/`hashCode` estruturais.
2. **Ids 1000–1012.** O lowering agora numera as classes do programa por
   caminho estável (P2) e pula essa faixa, que continua sendo a das classes
   de erro que o runtime cria (`excecoes.rs`). Em P5d elas vêm da fonte e a
   faixa sai dos dois lados.
