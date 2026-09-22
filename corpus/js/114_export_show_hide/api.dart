// Fachada: reexporta partes de impl.dart e outro.dart, e adiciona membros próprios.
export 'impl.dart' show X, y;
export 'outro.dart' hide Z;

import 'impl.dart' as impl;

String versao() => 'api 1.0';

// api.dart pode usar z e Interno internamente sem exportá-los.
int usaZ(int a) => impl.z(a) + impl.y(a);

String usaInterno() => impl.X(3).interno().toString();

String usaEscondida() => impl.escondida;
