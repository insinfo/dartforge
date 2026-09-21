// Records guardam valores imutáveis; a forma canônica não reordena avaliação de expressões.
// Contrato consultado: SDK 3.6.2 tests/language/records/simple/equals_and_hashcode_test.dart.
// Não implementa operator== customizado nem hashCode; listas contidas preservam identidade.
function $dartforgeRecord(fields) {
  const positional = [], named = [], value = Object.create(null);
  for (const [name, field] of fields) {
    if (name === null) {
      positional.push(field);
      value['$df_$' + positional.length] = field;
    } else {
      named.push([name, field]);
      value['$df_' + name] = field;
    }
  }
  named.sort((a,b) => a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0);
  $dartforgeRecordData.set(value, [positional, named]);
  $dartforgeTyped(value, ['record', positional.map($dartforgeTypeOf), named.map(([name, field]) => [name, $dartforgeTypeOf(field)])]);
  return Object.freeze(value);
}
function $dartforgeEqual(left, right) {
  if (left === null || right === null || typeof left !== 'object' || typeof right !== 'object') return left === right;
  if ($dartforgeTypeTags.get(left)?.[0] === 'duration' && $dartforgeTypeTags.get(right)?.[0] === 'duration') return left.microseconds === right.microseconds;
  const a = $dartforgeRecordData.get(left), b = $dartforgeRecordData.get(right);
  if (!a || !b) return left === right;
  return a[0].length === b[0].length && a[1].length === b[1].length &&
    a[0].every((value, i) => $dartforgeEqual(value, b[0][i])) &&
    a[1].every(([name, value], i) => name === b[1][i][0] && $dartforgeEqual(value, b[1][i][1]));
}
