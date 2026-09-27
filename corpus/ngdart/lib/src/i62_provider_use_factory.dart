import 'package:ngdart/angular.dart';

class I62Dep {}

class I62Servico {
  final String nome;
  I62Servico(this.nome);
}

class I62Outro {
  final I62Dep dep;
  I62Outro(this.dep);
}

I62Servico criarI62Servico() => I62Servico('a');

I62Outro criarI62Outro(I62Dep dep) => I62Outro(dep);

String criarI62Rotulo(I62Dep dep) => 'rotulo';

const i62Rotulo = OpaqueToken<String>('i62Rotulo');

/// Sonda: `FactoryProvider`, `FactoryProvider.forToken` e
/// `Provider(.., useFactory:, deps:)`.
@Component(
  selector: 'i62-provider-use-factory',
  template: '<p>x</p>',
  providers: [
    ClassProvider(I62Dep),
    FactoryProvider(I62Servico, criarI62Servico),
    FactoryProvider.forToken(i62Rotulo, criarI62Rotulo),
    Provider(I62Outro, useFactory: criarI62Outro, deps: [I62Dep]),
  ],
)
class I62ProviderUseFactory {}
