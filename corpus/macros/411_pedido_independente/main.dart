// experimentos: macros
import 'package:caso_pedido/pedido.dart';

void main() {
  final pedido = Pedido.fromJson({'codigo': 'P7', 'total': 12.5});
  print(pedido.codigo);
  print(pedido.toJson());
}
