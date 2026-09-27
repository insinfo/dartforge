import 'package:ngdart/angular.dart';

class I70Api {
  final NgZone zona;
  I70Api(this.zona);
}

class I70Servico {}

I70Servico criarI70Servico(I70Api api) => I70Servico();

abstract class I70Base {}

class I70Impl implements I70Base {}

/// Sonda: o componente depende de provedores do nó (um com dependência de
/// fora, outro de fábrica, outro por apelido de classe) e de algo do
/// injetor; todos saem antes do componente, no `build()`.
@Component(
  selector: 'i70-provider-ansioso-externo',
  template: '<p>x</p>',
  providers: [
    ClassProvider(I70Api),
    FactoryProvider(I70Servico, criarI70Servico),
    ClassProvider(I70Base, useClass: I70Impl),
  ],
)
class I70ProviderAnsiosoExterno {
  I70ProviderAnsiosoExterno(
      I70Api api, I70Servico servico, I70Base base, NgZone zona);
}
