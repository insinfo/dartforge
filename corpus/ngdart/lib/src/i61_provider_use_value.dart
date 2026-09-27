import 'package:ngdart/angular.dart';

const i61Nome = OpaqueToken<String>('i61Nome');
const i61Limite = OpaqueToken<int>('i61Limite');
const i61Ativo = OpaqueToken<bool>('i61Ativo');

class I61Config {
  final String url;
  const I61Config(this.url);
}

/// Sonda: `ValueProvider`, `ValueProvider.forToken` e `Provider(.., useValue:)`
/// com `OpaqueToken` e com classe.
@Component(
  selector: 'i61-provider-use-value',
  template: '<p>x</p>',
  providers: [
    ValueProvider.forToken(i61Nome, 'sonda'),
    Provider(i61Limite, useValue: 10),
    ValueProvider.forToken(i61Ativo, true),
    ValueProvider(I61Config, I61Config('/api')),
  ],
)
class I61ProviderUseValue {}
