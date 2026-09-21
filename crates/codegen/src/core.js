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
  $df_contains(value) { for (const x of this) if (x === value) return true; return false; }
  // Membros do núcleo de dart:core. Ficam aqui, na classe base, porque List e
  // Set herdam de Iterable e a semântica Dart é a mesma nas três; as
  // especializações que dependem do armazenamento estão nas subclasses.
  get $df_single() {
    const it = this[Symbol.iterator]();
    const first = it.next();
    if (first.done) throw new Error('Bad state: No element');
    if (!it.next().done) throw new Error('Bad state: Too many elements');
    return first.value;
  }
  $df_toString() { return $dartforgeFormat(this); }
  // join escreve cada elemento pelo toString dele; a análise já recusou
  // elemento sem representação textual, então $dartforgeString nunca cai no
  // "[object Object]" do JavaScript.
  $df_join(separator = '') {
    let texto = '', primeiro = true;
    for (const x of this) { if (!primeiro) texto += separator; texto += $dartforgeString(x); primeiro = false; }
    return texto;
  }
  $df_elementAt(index) {
    if (!Number.isInteger(index) || index < 0) throw new RangeError('RangeError (index): Invalid value: Not in inclusive range 0..: ' + index);
    let i = 0;
    for (const x of this) { if (i === index) return x; i++; }
    throw new RangeError('RangeError (length): Invalid value: Not in inclusive range 0..' + (i - 1) + ': ' + index);
  }
  // skip e take são preguiçosos, como no SDK: nada é materializado até iterar,
  // e pedir mais do que existe não é erro (devolve o que houver).
  $df_skip(count) { const source = this; return new $dartforgeIterable(function* () { let i = 0; for (const x of source) { if (i++ >= count) yield x; } }, null, this.elementType); }
  $df_take(count) { const source = this; return new $dartforgeIterable(function* () { let i = 0; for (const x of source) { if (i++ >= count) return; yield x; } }, null, this.elementType); }
  $df_toSet() { return new $dartforgeSet([...this], this.elementType); }
  $df_every(test) { for (const x of this) if (!test(x)) return false; return true; }
  $df_firstWhere(test) { for (const x of this) if (test(x)) return x; throw new Error('Bad state: No element'); }
  $df_reduce(combine) {
    const it = this[Symbol.iterator]();
    const first = it.next();
    if (first.done) throw new Error('Bad state: No element');
    let acc = first.value;
    for (let item = it.next(); !item.done; item = it.next()) acc = combine(acc, item.value);
    return acc;
  }
  $df_fold(initial, combine) { let acc = initial; for (const x of this) acc = combine(acc, x); return acc; }
  $df_expand(convert, resultType) {
    const source = this;
    return new $dartforgeIterable(function* () { for (const x of source) yield* convert(x); }, null, resultType);
  }
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
  // List tem armazenamento indexado, então estes membros não precisam iterar.
  get $df_reversed() { const values = this.values; return new $dartforgeIterable(function* () { for (let i = values.length - 1; i >= 0; i--) yield values[i]; }, () => values.length, this.elementType); }
  $df_elementAt(index) { return $dartforgeIndex(this, index); }
  $df_addAll(other) { for (const x of other) { $dartforgeCast(x, this.elementType); this.values.push(x); } }
  // Identidade, não `==`: é a mesma política já documentada para chaves de Map
  // e elementos de Set neste subconjunto (docs/OBJETO.md).
  $df_indexOf(value, start = 0) { return this.values.indexOf(value, start); }
  $df_remove(value) { const at = this.values.indexOf(value); if (at < 0) return false; this.values.splice(at, 1); return true; }
  $df_removeAt(index) { if (!Number.isInteger(index) || index < 0 || index >= this.values.length) throw new RangeError('RangeError (index): Invalid value: Not in inclusive range 0..' + (this.values.length - 1) + ': ' + index); return this.values.splice(index, 1)[0]; }
  $df_insert(index, value) { if (!Number.isInteger(index) || index < 0 || index > this.values.length) throw new RangeError('RangeError (index): Invalid value: Not in inclusive range 0..' + this.values.length + ': ' + index); $dartforgeCast(value, this.elementType); this.values.splice(index, 0, value); }
  $df_clear() { this.values.length = 0; }
  $df_sublist(start, end = null) {
    const fim = end === null ? this.values.length : end;
    if (!Number.isInteger(start) || start < 0 || start > fim) throw new RangeError('RangeError (start): Invalid value: Not in inclusive range 0..' + fim + ': ' + start);
    if (!Number.isInteger(fim) || fim > this.values.length) throw new RangeError('RangeError (end): Invalid value: Not in inclusive range ' + start + '..' + this.values.length + ': ' + fim);
    return new $dartforgeList(this.values.slice(start, fim), this.elementType);
  }
  // Ordena no lugar, como o SDK. Sem comparador a ordem é a de `compareTo`,
  // que a análise semântica já provou existir para o tipo do elemento.
  // O `sort` do JavaScript é estável desde ES2019; o do Dart não promete
  // estabilidade, então esta implementação é mais forte, nunca mais fraca.
  $df_sort(compare) { this.values.sort(compare === undefined ? $dartforgeDefaultCompare : compare); }
}
// `Comparable.compare` do SDK: delega ao compareTo do próprio valor. Números e
// cadeias usam a ordem do alvo web já conferida contra o oráculo; uma instância
// usa o `int compareTo(...)` que ela declara.
function $dartforgeDefaultCompare(a, b) {
  if (typeof a === 'string') return a < b ? -1 : (a > b ? 1 : 0);
  if (typeof a === 'number') { if (Number.isNaN(a)) return Number.isNaN(b) ? 0 : 1; if (Number.isNaN(b)) return -1; return a < b ? -1 : (a > b ? 1 : 0); }
  return a.$df_compareTo(b);
}
// Set conserva ordem de inserção, como o LinkedHashSet padrão do Dart, e
// compara elementos por identidade — a mesma política já adotada por Map.
class $dartforgeSet extends $dartforgeIterable {
  constructor(values, elementType = ['nullable',['object']]) {
    const store = new Set(values);
    super(function* () {
      const size = store.size;
      for (const x of store) {
        if (store.size !== size) throw new Error('Concurrent modification during iteration');
        yield x;
      }
      if (store.size !== size) throw new Error('Concurrent modification during iteration');
    }, () => store.size, elementType);
    this.store = store;
    $dartforgeTyped(this, ['set',elementType]);
  }
  // Dart devolve false quando o elemento já pertencia ao conjunto.
  $df_add(value) { $dartforgeCast(value, this.elementType); const had = this.store.has(value); this.store.add(value); return !had; }
  $df_contains(value) { return this.store.has(value); }
  $df_toList() { return new $dartforgeList([...this.store], this.elementType); }
  $df_addAll(other) { for (const x of other) { $dartforgeCast(x, this.elementType); this.store.add(x); } }
  $df_remove(value) { return this.store.delete(value); }
  $df_clear() { this.store.clear(); }
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
  get $df_isEmpty() { return this.values.size === 0; }
  get $df_isNotEmpty() { return this.values.size !== 0; }
  // A ordem de keys e values é a de inserção: o Map do JavaScript a preserva e
  // é a mesma do LinkedHashMap que o Dart usa para `{}`.
  get $df_keys() { const store = this.values; return new $dartforgeIterable(function* () { yield* store.keys(); }, () => store.size, this.keyType); }
  get $df_values() { const store = this.values; return new $dartforgeIterable(function* () { yield* store.values(); }, () => store.size, this.valueType); }
  $df_toString() { return $dartforgeFormat(this); }
  $df_containsKey(key) { return this.values.has(key); }
  $df_containsValue(value) { for (const v of this.values.values()) if (v === value) return true; return false; }
  // Devolve o valor removido, ou null quando a chave não existia.
  $df_remove(key) { if (!this.values.has(key)) return null; const found = this.values.get(key); this.values.delete(key); return found; }
  $df_clear() { this.values.clear(); }
  // A fábrica só é chamada quando a chave falta, e a inserção mantém a ordem.
  $df_putIfAbsent(key, factory) {
    if (this.values.has(key)) return this.values.get(key);
    const value = factory();
    $dartforgeCast(key, this.keyType);
    $dartforgeCast(value, this.valueType);
    this.values.set(key, value);
    return value;
  }
  $df_addAll(other) { for (const [k, v] of other.values) { $dartforgeCast(k, this.keyType); $dartforgeCast(v, this.valueType); this.values.set(k, v); } }
  $df_forEach(action) { for (const [k, v] of [...this.values]) action(k, v); }
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
// Uma instância responde pelo toString declarado, escolhido pelo próprio objeto
// e não pelo tipo estático: uma derivada com toString próprio aparece mesmo por
// uma referência da base. Instância sem toString declarado não chega aqui — a
// análise semântica recusa imprimi-la em vez de inventar "Instance of 'Nome'".
function $dartforgeString(value) {
  if (value === null || value === undefined) return 'null';
  if (typeof value === 'string') return value;
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  if (typeof value === 'number') return (value + 0).toString(10);
  if (typeof value.$df_toString === 'function') return value.$df_toString();
  // Só coleções e records chegam aqui, e eles já exigem o restante deste
  // arquivo: quando o programa interpola apenas escalares ou instâncias, o
  // emissor recorta esta função entre os marcadores e a linha abaixo não roda.
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
  // Elemento de coleção que é instância usa o toString declarado da classe.
  if (value !== null && typeof value === 'object' && typeof value.$df_toString === 'function') return value.$df_toString();
  if (!(value instanceof $dartforgeIterable)) return String(value);
  // SDK 3.6.2: List e Set imprimem o texto integral (iterableToFullString);
  // só o Iterable preguiçoso usa a abreviação de iterableToShortString.
  const set = value instanceof $dartforgeSet;
  const full = set || value instanceof $dartforgeList;
  const open = set ? '{' : (full ? '[' : '('), close = set ? '}' : (full ? ']' : ')');
  if (active.has(value)) return open + '...' + close;
  active.add(value);
  try {
    const parts = [];
    if (full) {
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
