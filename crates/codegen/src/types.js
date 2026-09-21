// Descritores próprios do subconjunto; não expõem o runtimeType nem a representação do SDK.
const $dartforgeTypeTags = new WeakMap();
const $dartforgeRecordData = new WeakMap();
let $dartforgeNominalMembers = {};
function $dartforgeTyped(value, type) { $dartforgeTypeTags.set(value, type); return value; }
function $dartforgeTypeOf(value) {
  if (value === null) return ['null'];
  if (typeof value === 'number' && Number.isInteger(value)) return ['int'];
  if (typeof value === 'string') return ['string'];
  if (typeof value === 'boolean') return ['bool'];
  if (value && value.$dartforgeEnumTag !== undefined) return ['class', value.$dartforgeEnumTag];
  for (let current = value; current !== null && (typeof current === 'object' || typeof current === 'function'); current = Object.getPrototypeOf(current)) {
    const type = $dartforgeTypeTags.get(current);
    if (type) return type;
  }
  // Uma função sem metadados não pode satisfazer silenciosamente uma assinatura concreta.
  return ['opaque'];
}
function $dartforgeSubtype(actual, expected) {
  // T? deve normalizar Null? e nullable já nullable após substituir T na ativação.
  while (actual[0] === 'nullable' && (actual[1][0] === 'nullable' || actual[1][0] === 'null')) actual = actual[1];
  while (expected[0] === 'nullable' && (expected[1][0] === 'nullable' || expected[1][0] === 'null')) expected = expected[1];
  const a = actual[0], e = expected[0];
  if (e === 'nullable') return a === 'null' || (a === 'nullable' ? $dartforgeSubtype(actual[1], expected[1]) : $dartforgeSubtype(actual, expected[1]));
  if (a === 'nullable' || a === 'null') return a === e;
  if (e === 'object') return a !== 'void';
  if (a === 'class' && e === 'class') return ($dartforgeNominalMembers[expected[1]] || [expected[1]]).includes(actual[1]);
  if ((a === 'list' || a === 'iterable') && (e === a || e === 'iterable')) return $dartforgeSubtype(actual[1], expected[1]);
  if (a === 'future' && e === 'future') return $dartforgeSubtype(actual[1], expected[1]);
  if (a === 'map' && e === 'map') return $dartforgeSubtype(actual[1], expected[1]) && $dartforgeSubtype(actual[2], expected[2]);
  if (a === 'record' && e === 'record') {
    return actual[1].length === expected[1].length && actual[2].length === expected[2].length &&
      actual[1].every((field, i) => $dartforgeSubtype(field, expected[1][i])) &&
      actual[2].every((field, i) => field[0] === expected[2][i][0] && $dartforgeSubtype(field[1], expected[2][i][1]));
  }
  if (a === 'function' && e === 'function') {
    return actual[2].length === expected[2].length &&
      (expected[1][0] === 'void' || $dartforgeSubtype(actual[1], expected[1])) &&
      actual[2].every((parameter, i) => $dartforgeSubtype(expected[2][i], parameter));
  }
  return a === e && a !== 'opaque';
}
function $dartforgeIs(value, type) { return $dartforgeSubtype($dartforgeTypeOf(value), type); }
function $dartforgeCast(value, type) {
  if (!$dartforgeIs(value, type)) throw new TypeError('Value does not match the reified Dart type');
  return value;
}
