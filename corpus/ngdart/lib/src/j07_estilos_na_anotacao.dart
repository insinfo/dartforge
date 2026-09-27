import 'package:ngdart/angular.dart';

/// Sonda: `styles: [..]` escrito na anotação, com encapsulamento emulado.
@Component(
  selector: 'j07-estilos-na-anotacao',
  templateUrl: 'j07_estilos_na_anotacao.html',
  styles: [':host { display: block; } .a { color: red; }'],
)
class J07EstilosNaAnotacao {}
