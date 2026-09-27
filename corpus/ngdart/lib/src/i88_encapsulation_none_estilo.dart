import 'package:ngdart/angular.dart';

/// Sonda: `encapsulation: ViewEncapsulation.none` com folha de estilo — o
/// `.css.dart` sem shim e `ComponentStyles.unscoped`.
@Component(
  selector: 'i88-encapsulation-none-estilo',
  templateUrl: 'i88_encapsulation_none_estilo.html',
  styleUrls: ['i88_encapsulation_none_estilo.css'],
  encapsulation: ViewEncapsulation.none,
)
class I88EncapsulationNoneEstilo {
  int n = 1;
}
