import 'package:ngdart/angular.dart';

abstract class I63Base {}

class I63Servico implements I63Base {}

abstract class I63Leitor {}

abstract class I63Pai {}

/// Sonda: `ExistingProvider` e `Provider(.., useExisting:)` de provedor do
/// próprio nó (apelido, sem campo), apelido de apelido, e apelido do
/// componente.
@Component(
  selector: 'i63-provider-use-existing',
  template: '<p>x</p>',
  providers: [
    ClassProvider(I63Servico),
    ExistingProvider(I63Base, I63Servico),
    Provider(I63Leitor, useExisting: I63Base),
    ExistingProvider(I63Pai, I63ProviderUseExisting),
  ],
)
class I63ProviderUseExisting implements I63Pai {}
