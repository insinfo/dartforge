import 'package:ngdart/angular.dart';

class I66Api {}

class I66Repo {
  final I66Api api;
  I66Repo(this.api);
}

class I66Cache {
  final I66Repo repo;
  I66Cache(this.repo);
}

class I66Solto {}

/// Sonda: o componente depende de um provedor que depende de outro; os dois
/// saem antes do componente, e o que ninguém pede fica preguiçoso.
@Component(
  selector: 'i66-provider-dependencias',
  template: '<p>x</p>',
  providers: [
    ClassProvider(I66Cache),
    ClassProvider(I66Solto),
    ClassProvider(I66Repo),
    ClassProvider(I66Api),
  ],
)
class I66ProviderDependencias {
  final I66Repo repo;
  I66ProviderDependencias(this.repo);
}
