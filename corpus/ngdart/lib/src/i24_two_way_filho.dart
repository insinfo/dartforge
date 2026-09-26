import 'dart:async';

import 'package:ngdart/angular.dart';

@Component(selector: 'i24-contador', template: '<b>{{valor}}</b>')
class I24Contador {
  @Input()
  int valor = 0;
  final _c = StreamController<int>();
  @Output()
  Stream<int> get valorChange => _c.stream;
}

/// Sonda: two-way `[(x)]` em componente filho.
@Component(
  selector: 'i24-two-way-filho',
  templateUrl: 'i24_two_way_filho.html',
  directives: [coreDirectives, I24Contador],
)
class I24TwoWayFilho {
  int n = 0;
}
