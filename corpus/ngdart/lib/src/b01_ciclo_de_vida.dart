import 'package:ngdart/angular.dart';

/// `OnInit` e `OnDestroy`: o que o oficial emite na visão por causa deles.
@Component(
  selector: 'b01-ciclo-de-vida',
  templateUrl: 'b01_ciclo_de_vida.html',
)
class B01CicloDeVida implements OnInit, OnDestroy {
  @override
  void ngOnInit() {}

  @override
  void ngOnDestroy() {}
}
