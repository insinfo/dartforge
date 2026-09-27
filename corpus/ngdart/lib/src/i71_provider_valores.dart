import 'package:ngdart/angular.dart';

const i71Url = OpaqueToken<String>('url-da-api');
const i71Itens = MultiToken<String>('i71Itens');

class I71Externo {}

class I71Config {
  final String nome;
  final int limite;
  final bool ativo;
  const I71Config(this.nome, {this.limite = 1, this.ativo = false});
}

abstract class I71Leitor {}

class I71Servico implements I71Leitor {
  final String url;
  I71Servico(@Inject(i71Url) this.url);
}

/// Sonda: nome de token com caractere fora de identificador, texto com `$`
/// e aspas, `Provider(.., useValue:)` de objeto com argumentos nomeados,
/// multi com apelido de fora do nó, serviço que injeta um `OpaqueToken`, e
/// apelido por `OpaqueToken`.
@Component(
  selector: 'i71-provider-valores',
  template: '<p>x</p>',
  providers: [
    ValueProvider.forToken(i71Url, "/api/\$x'y"),
    Provider(I71Config, useValue: I71Config('a', limite: 2, ativo: true)),
    ValueProvider.forToken(i71Itens, 'um'),
    ExistingProvider.forToken(i71Itens, I71Externo),
    ClassProvider(I71Servico),
    ExistingProvider(I71Leitor, I71Servico),
  ],
)
class I71ProviderValores {}
