// R-CTX-06: em `e1 ?? e2`, e1 recebe o contexto K? e e2 recebe K (ou, sem
// contexto, NonNull(T1)); o resultado é UP(NonNull(T1), T2).
void main(List<String> args) {
  List<num>? a = args.isEmpty ? null : [];
  List<num> b = a ?? /*@*/[1];
  int? i = args.isEmpty ? null : 1;
  var c = /*@*/i ?? 2.0;
  double? d;
  double e = d ?? /*@*/1;
  var f = /*@*/a ?? /*@*/[];
  print([b, c, e, f]);
}
