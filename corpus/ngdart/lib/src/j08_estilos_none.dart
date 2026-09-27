import 'package:ngdart/angular.dart';

/// Sonda: `styles: [..]` com `ViewEncapsulation.none`.
@Component(
  selector: 'j08-estilos-none',
  templateUrl: 'j08_estilos_none.html',
  styles: ['.a { color: red; }'],
  encapsulation: ViewEncapsulation.none,
)
class J08EstilosNone {}
