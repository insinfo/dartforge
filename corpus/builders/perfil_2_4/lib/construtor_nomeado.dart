import 'anotacoes.dart' as an;

// `@an.Gerar.nomeado()`: o nome é `an.Gerar.nomeado` e, sem o primeiro
// trecho, `Gerar.nomeado` — nenhum dos dois é `Gerar`: não dispara.
@an.Gerar.nomeado()
class ConstrutorNomeado {}
