// ignore_for_file: uri_has_not_been_generated

import 'package:ngdart/angular.dart';

import 'j88_injetor_e_componente.template.dart' as self;

class J88Servico {}

@Component(
  selector: 'j88-raiz',
  template: '<p>{{ texto }}</p>',
)
class J88Raiz {
  String texto = 'oi';
}

@GenerateInjector([ClassProvider(J88Servico)])
final InjectorFactory injetor = self.injetor$Injector;
