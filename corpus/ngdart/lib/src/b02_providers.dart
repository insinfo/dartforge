import 'package:ngdart/angular.dart';

class Servico {}

/// `providers:` no componente — a injeção do elemento.
@Component(
  selector: 'b02-providers',
  templateUrl: 'b02_providers.html',
  providers: [ClassProvider(Servico)],
)
class B02Providers {}
