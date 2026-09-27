import 'package:ngdart/angular.dart';

const i64Nomes = MultiToken<String>('i64Nomes');
const i64Validadores = MultiToken<Object>('i64Validadores');

class I64Validador {}

/// Sonda: `MultiToken` com vários provedores (valor, classe e o próprio
/// componente) — o campo é a lista.
@Component(
  selector: 'i64-provider-multi',
  template: '<p>x</p>',
  providers: [
    ValueProvider.forToken(i64Nomes, 'a'),
    ValueProvider.forToken(i64Nomes, 'b'),
    ClassProvider.forToken(i64Validadores, useClass: I64Validador),
    ExistingProvider.forToken(i64Validadores, I64ProviderMulti),
  ],
)
class I64ProviderMulti {}
