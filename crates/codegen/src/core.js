/*
Formatação abreviada de Iterable adaptada para JS a partir do algoritmo do SDK Dart 3.6.2,
sdk/lib/core/iterable.dart. Copyright (c) 2011, the Dart project authors. All rights reserved.
Copyright 2012, the Dart project authors.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:
    * Redistributions of source code must retain the above copyright
      notice, this list of conditions and the following disclaimer.
    * Redistributions in binary form must reproduce the above
      copyright notice, this list of conditions and the following
      disclaimer in the documentation and/or other materials provided
      with the distribution.
    * Neither the name of Google LLC nor the names of its
      contributors may be used to endorse or promote products derived
      from this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
"AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
*/
// Subconjunto próprio de dart:core; Iterable preguiçoso e List growable.
class $dartforgeIterable {
  constructor(iterator, length = null, elementType = ['nullable',['object']]) { this.iterator = iterator; this.size = length; this.elementType = elementType; $dartforgeTyped(this, ['iterable',elementType]); }
  [Symbol.iterator]() { return this.iterator(); }
  get $df_length() { if (this.size) return this.size(); let n = 0; for (const x of this) n++; return n; }
  get $df_isEmpty() { return this[Symbol.iterator]().next().done; }
  get $df_isNotEmpty() { return !this.$df_isEmpty; }
  get $df_first() { const v = this[Symbol.iterator]().next(); if (v.done) throw new Error('Bad state: No element'); return v.value; }
  get $df_last() { let found = false, last; for (const x of this) { found = true; last = x; } if (!found) throw new Error('Bad state: No element'); return last; }
  $df_where(test) { const source = this; return new $dartforgeIterable(function* () { for (const x of source) if (test(x)) yield x; }, null, this.elementType); }
  $df_map(convert, resultType) { return new $dartforgeMapped(this, convert, resultType); }
  $df_forEach(action) { for (const x of this) action(x); }
  $df_any(test) { for (const x of this) if (test(x)) return true; return false; }
  $df_toList() { return new $dartforgeList([...this], this.elementType); }
}
// Map não chama o conversor ao consultar length/isEmpty e transforma apenas last ao consultá-lo.
class $dartforgeMapped extends $dartforgeIterable {
  constructor(source, convert, resultType) {
    super(function* () { for (const x of source) yield convert(x); }, () => source.$df_length, resultType);
    this.source = source; this.convert = convert;
  }
  get $df_isEmpty() { return this.source.$df_isEmpty; }
  get $df_first() { return this.convert(this.source.$df_first); }
  get $df_last() { return this.convert(this.source.$df_last); }
}
class $dartforgeList extends $dartforgeIterable {
  constructor(values, elementType = ['nullable',['object']]) {
    super(function* () {
      const length = values.length;
      for (let i = 0; i < length; i++) {
        if (values.length !== length) throw new Error('Concurrent modification during iteration');
        yield values[i];
      }
      if (values.length !== length) throw new Error('Concurrent modification during iteration');
    }, () => values.length, elementType);
    this.values = values;
    $dartforgeTyped(this, ['list',elementType]);
  }
  $df_add(value) { $dartforgeCast(value, this.elementType); this.values.push(value); }
  $df_forEach(action) {
    const n = this.values.length;
    for (let i = 0; i < n; i++) { action(this.values[i]); if (this.values.length !== n) throw new Error('Concurrent modification during iteration'); }
  }
}
// Map conserva ordem de inserção e evita propriedades especiais de objetos JavaScript.
class $dartforgeMap {
  constructor(entries, keyType, valueType) {
    this.values = new Map(entries);
    this.keyType = keyType;
    this.valueType = valueType;
    $dartforgeTyped(this, ['map', keyType, valueType]);
  }
  get $df_length() { return this.values.size; }
}
function $dartforgeIndex(list, index) {
  if (list instanceof $dartforgeMap) return list.values.has(index) ? list.values.get(index) : null;
  if (!Number.isInteger(index) || index < 0 || index >= list.values.length) throw new RangeError('Index out of range');
  return list.values[index];
}
function $dartforgeIndexSet(list, index, value) {
  if (list instanceof $dartforgeMap) {
    $dartforgeCast(index, list.keyType);
    $dartforgeCast(value, list.valueType);
    list.values.set(index, value);
    return;
  }
  $dartforgeCast(value, list.elementType);
  if (!Number.isInteger(index) || index < 0 || index >= list.values.length) throw new RangeError('Index out of range');
  list.values[index] = value;
}
// >>> $dartforgeString
// toString de Dart 3.6.2 para os valores do subconjunto, usado pela interpolação.
// int não passa por String(x) do JavaScript: o inteiro do subconjunto tem 32 bits
// com sinal e o zero negativo do JavaScript precisa ser impresso como "0", que é
// o único texto que Dart produz para zero. Coleções e records vão ao formatador.
function $dartforgeString(value) {
  if (value === null || value === undefined) return 'null';
  if (typeof value === 'string') return value;
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  if (typeof value === 'number') return (value + 0).toString(10);
  // Só coleções e records chegam aqui, e eles já exigem o restante deste
  // arquivo: quando o programa interpola apenas escalares, o emissor recorta
  // esta função entre os marcadores e a linha abaixo nunca executa.
  return $dartforgeFormat(value);
}
// <<< $dartforgeString
function $dartforgeFormat(value, active = new Set()) {
  if (value instanceof $dartforgeMap) {
    if (active.has(value)) return '{...}';
    active.add(value);
    try {
      return '{' + [...value.values].map(([key, field]) => $dartforgeFormat(key, active) + ': ' + $dartforgeFormat(field, active)).join(', ') + '}';
    } finally { active.delete(value); }
  }
  const record = $dartforgeRecordData.get(value);
  if (record) {
    if (active.has(value)) return '(...)';
    active.add(value);
    try {
      const parts = record[0].map(field => $dartforgeFormat(field, active));
      for (const [name, field] of record[1]) parts.push(name + ': ' + $dartforgeFormat(field, active));
      // SDK 3.6.2 imprime o record unário como (valor), sem a vírgula da sintaxe literal.
      return '(' + parts.join(', ') + ')';
    } finally { active.delete(value); }
  }
  if (!(value instanceof $dartforgeIterable)) return String(value);
  const list = value instanceof $dartforgeList, open = list ? '[' : '(', close = list ? ']' : ')';
  if (active.has(value)) return open + '...' + close;
  active.add(value);
  try {
    const parts = [];
    if (list) {
      for (const element of value) parts.push($dartforgeFormat(element, active));
    } else {
      $dartforgeIterableParts(value, active, parts);
    }
    return open + parts.join(', ') + close;
  }
  finally { active.delete(value); }
}

// Contrato observado em Dart SDK 3.6.2 sdk/lib/core/iterable.dart:
// prefixo mínimo de três, cauda de dois, alvo de 80 unidades UTF-16.
// A busca da cauda para ao observar o elemento 101. Somente os valores que
// entram no prefixo/cauda são formatados: mover o iterator pode ter efeitos.
function $dartforgeIterableParts(value, active, parts) {
  const iterator = value[Symbol.iterator]();
  const format = element => $dartforgeFormat(element, active);
  let width = 0, visited = 0, item;
  while (width < 80 || visited < 3) {
    item = iterator.next();
    if (item.done) return;
    const text = format(item.value);
    parts.push(text);
    width += text.length + 2;
    visited++;
  }
  let beforeLast, last;
  item = iterator.next();
  if (item.done) {
    if (visited <= 5) return;
    last = parts.pop();
    beforeLast = parts.pop();
  } else {
    let previousValue = item.value;
    visited++;
    item = iterator.next();
    if (item.done) {
      if (visited <= 4) { parts.push(format(previousValue)); return; }
      last = format(previousValue);
      beforeLast = parts.pop();
      width += last.length + 2;
    } else {
      let lastValue = item.value;
      visited++;
      for (;;) {
        item = iterator.next();
        if (item.done) break;
        previousValue = lastValue;
        lastValue = item.value;
        visited++;
        if (visited > 100) {
          while (width > 75 && visited > 3) {
            width -= parts.pop().length + 2;
            visited--;
          }
          parts.push('...');
          return;
        }
      }
      beforeLast = format(previousValue);
      last = format(lastValue);
      width += beforeLast.length + last.length + 4;
    }
  }
  let omitted = visited > parts.length + 2;
  if (omitted) width += 5;
  while (width > 80 && parts.length > 3) {
    width -= parts.pop().length + 2;
    if (!omitted) { omitted = true; width += 5; }
  }
  if (omitted) parts.push('...');
  parts.push(beforeLast, last);
}
function $dartforgePrint(value) { console.log($dartforgeFormat(value)); }


const $dartforgeConstLists = new Map();
function $dartforgeConstList(key,values,elementType = ['nullable',['object']]) {
  let found=$dartforgeConstLists.get(key);
  if(found===undefined){ found=Object.freeze(new $dartforgeList(Object.freeze(values),elementType)); $dartforgeConstLists.set(key,found); }
  return found;
}
