import 'package:ngdart/angular.dart';

class Servico {}

/// Sonda: `providers:` com `ClassProvider`.
@Component(
  selector: 'i19-providers-classe',
  templateUrl: 'i19_providers_classe.html',
  directives: [coreDirectives],
  providers: [ClassProvider(Servico)],
)
class I19ProvidersClasse {
  final Servico servico;
  I19ProvidersClasse(this.servico);
}
