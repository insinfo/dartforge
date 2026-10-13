// Copyright (c) 2015, the Dart project authors.  Please see the AUTHORS file
// for details. All rights reserved. Use of this source code is governed by a
// BSD-style license that can be found in the LICENSE file.
library dart._compact_hash;

import "dart:_internal" as internal;

import "dart:_internal" show patch, IterableElementError, ClassID, TypeTest;

import "dart:collection" show MapMixin, SetMixin, LinkedHashMap, LinkedHashSet;

import "dart:math" show max;

import "dart:typed_data" show Uint32List;

mixin _UnmodifiableMapMixin<K, V> implements LinkedHashMap<K, V> {
  /// This operation is not supported by an unmodifiable map.
  void operator []=(K key, V value) {
    throw UnsupportedError("Cannot modify unmodifiable map");
  }

  /// This operation is not supported by an unmodifiable map.
  void addAll(Map<K, V> other) {
    throw UnsupportedError("Cannot modify unmodifiable map");
  }

  /// This operation is not supported by an unmodifiable map.
  void addEntries(Iterable<MapEntry<K, V>> entries) {
    throw UnsupportedError("Cannot modify unmodifiable map");
  }

  /// This operation is not supported by an unmodifiable map.
  void clear() {
    throw UnsupportedError("Cannot modify unmodifiable map");
  }

  /// This operation is not supported by an unmodifiable map.
  V? remove(Object? key) {
    throw UnsupportedError("Cannot modify unmodifiable map");
  }

  /// This operation is not supported by an unmodifiable map.
  void removeWhere(bool test(K key, V value)) {
    throw UnsupportedError("Cannot modify unmodifiable map");
  }

  /// This operation is not supported by an unmodifiable map.
  V putIfAbsent(K key, V ifAbsent()) {
    throw UnsupportedError("Cannot modify unmodifiable map");
  }

  /// This operation is not supported by an unmodifiable map.
  V update(K key, V update(V value), {V Function()? ifAbsent}) {
    throw UnsupportedError("Cannot modify unmodifiable map");
  }

  /// This operation is not supported by an unmodifiable map.
  void updateAll(V update(K key, V value)) {
    throw UnsupportedError("Cannot modify unmodifiable map");
  }
}

mixin _UnmodifiableSetMixin<E> implements LinkedHashSet<E> {
  static Never _throwUnmodifiable() {
    throw UnsupportedError("Cannot change an unmodifiable set");
  }

  /// This operation is not supported by an unmodifiable set.
  bool add(E value) => _throwUnmodifiable();

  /// This operation is not supported by an unmodifiable set.
  void clear() => _throwUnmodifiable();

  /// This operation is not supported by an unmodifiable set.
  void addAll(Iterable<E> elements) => _throwUnmodifiable();

  /// This operation is not supported by an unmodifiable set.
  void removeAll(Iterable<Object?> elements) => _throwUnmodifiable();

  /// This operation is not supported by an unmodifiable set.
  void retainAll(Iterable<Object?> elements) => _throwUnmodifiable();

  /// This operation is not supported by an unmodifiable set.
  void removeWhere(bool test(E element)) => _throwUnmodifiable();

  /// This operation is not supported by an unmodifiable set.
  void retainWhere(bool test(E element)) => _throwUnmodifiable();

  /// This operation is not supported by an unmodifiable set.
  bool remove(Object? value) => _throwUnmodifiable();
}

// Hash table with open addressing that separates the index from keys/values.

// This function takes care of rehashing of the linked hashmaps in [objects]. We
// do this eagerly after snapshot deserialization.
@pragma("vm:entry-point", "call")
void _rehashObjects(List objects) {
  final int length = objects.length;
  for (int i = 0; i < length; ++i) {
    internal.unsafeCast<_HashBase>(objects[i])._regenerateIndex();
  }
}

// DartForge: a `_Map`/`_Set` copied between isolates
// (`crates/runtime/src/ports.rs`, `redo_copied_indices`) arrives with the
// origin's `_index`. Like the VM (`object_graph_copy.cc`,
// `CopyLinkedHashBase` with `MightNeedReHashing`, and `_rehashObjects` above),
// the index is rebuilt when some key may have another hash at the destination
// — strings, numbers, bool and null have a structural hash. Returns -1 for an
// object that is not a `_Map`/`_Set`, 1 when it rebuilt and 0 when it did not need to.
@pragma("vm:entry-point")
int _dartforgeRebuildCopiedIndex(Object object) {
  final int step;
  if (object is _Map) {
    step = 2;
  } else if (object is _Set) {
    step = 1;
  } else {
    return -1;
  }
  final h = internal.unsafeCast<_HashVMBase>(object);
  final data = h._data;
  final used = h._usedData;
  for (int i = 0; i < used; i += step) {
    final keyArg = data[i];
    if (_HashBase._isDeleted(data, keyArg) ||
        keyArg == null ||
        keyArg is String ||
        keyArg is num ||
        keyArg is bool) {
      continue;
    }
    h._hashMask = _HashBase._UNINITIALIZED_HASH_MASK;
    h._deletedKeys = 0;
    internal.unsafeCast<_HashBase>(object)._regenerateIndex();
    return 1;
  }
  return 0;
}

// Common interface for [_HashFieldBase] and [_HashVMBase].
abstract class _HashAbstractBase {
  abstract Uint32List _index;

  abstract int _hashMask;

  abstract List<Object?> _data;

  abstract int _usedData;

  abstract int _deletedKeys;
}

abstract class _HashAbstractImmutableBase extends _HashAbstractBase {
  Uint32List? get _indexNullable;
}

abstract class _HashFieldBase implements _HashAbstractImmutableBase {
  // Each occupied entry in _index is a fixed-size integer that encodes a pair:
  //   [ hash pattern for key | index of entry in _data ]
  // The hash pattern is based on hashCode, but is guaranteed to be non-zero.
  // The length of _index is always a power of two, and there is always at
  // least one unoccupied entry.
  // NOTE: When maps are deserialized, their _index and _hashMask is regenerated
  // eagerly by _regenerateIndex.
  Uint32List? _indexNullable = _uninitializedIndex;

  @pragma("vm:exact-result-type", "dart:typed_data#_Uint32List")
  @pragma("vm:prefer-inline")
  @pragma("wasm:prefer-inline")
  Uint32List get _index => _indexNullable!;

  @pragma("vm:prefer-inline")
  @pragma("wasm:prefer-inline")
  void set _index(Uint32List value) => _indexNullable = value;

  // Cached in-place mask for the hash pattern component.
  int _hashMask = _HashBase._UNINITIALIZED_HASH_MASK;

  // Fixed-length list of keys (set) or key/value at even/odd indices (map).
  //
  // Can be either a mutable or immutable list.
  List<Object?> _data = _uninitializedData;

  // Length of _data that is used (i.e., keys + values for a map).
  int _usedData = 0;

  // Number of deleted keys.
  int _deletedKeys = 0;

  // Note: All fields are initialized in a single constructor so that the VM
  // recognizes they cannot hold null values. This makes a big (20%) performance
  // difference on some operations.
  _HashFieldBase();
}

// DartForge (overlay `native_sdk/`, P5c/P5d): on the VM, the fields of
// `_HashVMBase` are injected into the class and read by recognized accessors
// (`vm:recognized`, no native). Here they are real fields; the rest of this
// file is SDK 3.6.2's `compact_hash.dart` unchanged.
abstract class _HashVMBase implements _HashAbstractBase {
  Uint32List? _indexDf;
  Uint32List get _index => _indexDf!;
  void set _index(Uint32List value) {
    _indexDf = value;
  }

  int _hashMask = 0;

  List<Object?> _data = const <Object?>[];

  int _usedData = 0;

  int _deletedKeys = 0;
}

// Base class for immutable VM-internal classes.
abstract class _HashVMImmutableBase extends _HashVMBase
    implements _HashAbstractImmutableBase {
  // The index is nullable rather than not nullable.
  Uint32List? get _indexNullable => _indexDf;
  Uint32List get _index => _indexNullable!;
}

// This mixin can be applied to _HashFieldBase or _HashVMBase (for
// normal and VM-internalized classes, respectively), which provide the
// actual fields/accessors that this mixin assumes.
mixin _HashBase on _HashAbstractBase {
  // The number of bits used for each component is determined by table size.
  // If initialized, the length of _index is (at least) twice the number of
  // entries in _data, and both are doubled when _data is full. Thus, _index
  // will have a max load factor of 1/2, which enables one more bit to be used
  // for the hash.
  // TODO(koda): Consider growing _data by factor sqrt(2), twice as often.
  static const int _INITIAL_INDEX_BITS = 2;
  static const int _INITIAL_INDEX_SIZE = 1 << (_INITIAL_INDEX_BITS + 1);
  static const int _UNINITIALIZED_INDEX_SIZE = 1;
  static const int _UNINITIALIZED_HASH_MASK = 0;

  // Unused and deleted entries are marked by 0 and 1, respectively.
  static const int _UNUSED_PAIR = 0;
  static const int _DELETED_PAIR = 1;

  // On 32-bit, the top bits are wasted to avoid Mint allocation.
  // TODO(koda): Reclaim the bits by making the compiler treat hash patterns
  // as unsigned words.
  // Keep consistent with IndexSizeToHashMask in runtime/vm/object.h.
  static int _indexSizeToHashMask(int indexSize) {
    assert(indexSize >= _INITIAL_INDEX_SIZE ||
        indexSize == _UNINITIALIZED_INDEX_SIZE);
    if (indexSize == _UNINITIALIZED_INDEX_SIZE) {
      return _UNINITIALIZED_HASH_MASK;
    }
    int indexBits = indexSize.bitLength - 2;
    return internal.has63BitSmis
        ? (1 << (32 - indexBits)) - 1
        : (1 << (30 - indexBits)) - 1;
  }

  static int _hashPattern(int fullHash, int hashMask, int size) {
    final int maskedHash = fullHash & hashMask;
    // TODO(koda): Consider keeping bit length and use left shift.
    return (maskedHash == 0) ? (size >> 1) : maskedHash * (size >> 1);
  }

  // Linear probing.
  static int _firstProbe(int fullHash, int sizeMask) {
    final int i = fullHash & sizeMask;
    // Light, fast shuffle to mitigate bad hashCode (e.g., sequential).
    return ((i << 1) + i) & sizeMask;
  }

  static int _nextProbe(int i, int sizeMask) => (i + 1) & sizeMask;

  // A self-loop is used to mark a deleted key or value.
  static bool _isDeleted(List<Object?> data, Object? keyOrValue) =>
      identical(keyOrValue, data);
  static void _setDeletedAt(List<Object?> data, int d) {
    data[d] = data;
  }

  // Concurrent modification detection relies on this checksum monotonically
  // increasing between reallocations of _data.
  int get _checkSum => _usedData + _deletedKeys;
  bool _isModifiedSince(List<Object?> oldData, int oldCheckSum) =>
      !identical(_data, oldData) || (_checkSum != oldCheckSum);

  int get length;

  // If this collection has never had an insertion, and [other] is the same
  // representation and has had no deletions, then adding the entries of [other]
  // will end up building the same [_data] and [_index]. We can do this more
  // efficiently by copying the Lists directly.
  //
  // Precondition: [this] and [other] must use the same hashcode and equality.
  bool _quickCopy(_HashBase other) {
    if (!identical(_index, _uninitializedIndex)) return false;
    if (other._usedData == 0) return true; // [other] is empty, nothing to copy.
    if (other._deletedKeys != 0) return false;

    assert(!identical(other._index, _uninitializedIndex));
    assert(!identical(other._data, _uninitializedData));
    _index = Uint32List.fromList(other._index);
    _hashMask = other._hashMask;
    _data = List<Object?>.of(other._data, growable: false);
    _usedData = other._usedData;
    _deletedKeys = other._deletedKeys;
    return true;
  }

  // This method is called by [_rehashObjects] (see above).
  void _regenerateIndex();
}

abstract class _EqualsAndHashCode {
  int _hashCode(Object? e);
  bool _equals(Object? e1, Object? e2);
}

mixin _OperatorEqualsAndHashCode implements _EqualsAndHashCode {
  int _hashCode(Object? e) => e.hashCode;
  bool _equals(Object? e1, Object? e2) => e1 == e2;
}

mixin _IdenticalAndIdentityHashCode implements _EqualsAndHashCode {
  int _hashCode(Object? e) => identityHashCode(e);
  bool _equals(Object? e1, Object? e2) => identical(e1, e2);
}

// DartForge: a large map literal of constant scalars arrives as the list
// `[keyArg, val, …]` read from a table (`lower/literals.rs`), and not as a
// `[]=` call per entry in the code.
@pragma("vm:entry-point")
void _dartforgeFillMap(Map map, List keysAndValues) {
  for (int i = 0; i < keysAndValues.length; i += 2) {
    map[keysAndValues[i]] = keysAndValues[i + 1];
  }
}

@pragma("vm:entry-point")
void _dartforgeFillList(List list, List vals) {
  for (int i = 0; i < vals.length; i++) {
    list.add(vals[i]);
  }
}

@pragma("vm:entry-point")
void _dartforgeFillSet(Set setArg, List vals) {
  for (int i = 0; i < vals.length; i++) {
    setArg.add(vals[i]);
  }
}

mixin _OperatorEqualsAndCanonicalHashCode implements _EqualsAndHashCode {
  static final int cidSymbol = ClassID.getID(#a);

  int _hashCode(Object? e) {
    final int cid = ClassID.getID(e);
    // DartForge: a record has the id of its SHAPE (above the predefined ones);
    // on the VM it is `_Record`, predefined — the hash is the structural one.
    if (cid < ClassID.numPredefinedCids || cid == cidSymbol || e is Record) {
      return e.hashCode;
    }
    return identityHashCode(e);
  }

  bool _equals(Object? e1, Object? e2) => e1 == e2;
}

mixin _CustomEqualsAndHashCode<K> implements _EqualsAndHashCode {
  int Function(K) get _hasher;
  bool Function(K, K) get _equality;

  // For backwards compatibility, we must allow any key here that is accepted
  // dynamically by the [_hasher] and [_equality] functions.
  int _hashCode(Object? e) => (_hasher as Function)(e);
  bool _equals(Object? e1, Object? e2) => (_equality as Function)(e1, e2);
}

// DartForge: the fast path of `_Map` and `_Set` for `int` and `String` keys
// (`crates/runtime/src/natives_hash.rs`). On the VM the compiler specializes
// `_hashCode`/`_equals` for those types and inlines the probe; here the
// runtime walks the SAME table (`_index`, `_data`) with the same arithmetic
// and stores what Dart would store. When it does not know (another key, an
// `int` facing a `double`, a full table), it changes nothing and returns the
// signal; the SDK path does the operation.

/// The new `_usedData` after `map[keyArg] = val`, or -1.
@pragma("vm:external-name", "DartForge_hash_map_store")
external int _dfMapStore(Uint32List idx, List<Object?> dataArg, int mask,
    int used, Object? keyArg, Object? val);

/// The key's value, `dataArg` if it is missing, or `idx` if Dart must look it up.
@pragma("vm:external-name", "DartForge_hash_map_lookup")
external Object? _dfMapLookup(
    Uint32List idx, List<Object?> dataArg, int mask, Object? keyArg);

/// The new `_usedData` after `setArg.add(keyArg)` (the same if it was
/// already there), or -1.
@pragma("vm:external-name", "DartForge_hash_set_add")
external int _dfSetAdd(Uint32List idx, List<Object?> dataArg,
    int mask, int used, Object? keyArg);

/// The stored key, `dataArg` if it is missing, or `idx` if Dart must look it up.
@pragma("vm:external-name", "DartForge_hash_set_lookup")
external Object? _dfSetLookup(
    Uint32List idx, List<Object?> dataArg, int mask, Object? keyArg);

// DartForge: the batches (docs/NATIVO-PLANO.md §13.2, items 2 and 3). The
// runtime stores at once the `int`/`String`-keyed entries of `pairs` (from
// `from` to `until`), in order, and stops at the first one it cannot store.
// The result joins the position where it stopped (high bits) and the new
// `_usedData` (the low 32 bits); Dart stores the stopping entry through the SDK path and calls again.

/// `this[pairs[i]] = pairs[i + 1]` for `i` from `from` to `until`, in steps of
/// 2; with `skipDeleted`, without the entries deleted from the old `_data`.
@pragma("vm:external-name", "DartForge_hash_map_fill")
external int _dfMapFill(Uint32List idx, List<Object?> dataArg,
    int mask, int used, List pairs, int from, int until, bool skipDeleted);

/// DartForge: the new `_data` of a table, `new List.filled(n, null)` without
/// evaluating the reified type: the list is internal (it never leaves the
/// `_HashBase`) and is only compared by identity and read by the table's own methods.
@pragma("vm:external-name", "DartForge_hash_new_data")
external List<Object?> _dfNewData(int n);

/// `add(old[i])` for `i` from `from` to `until`, without the deleted ones.
@pragma("vm:external-name", "DartForge_hash_set_fill")
external int _dfSetFill(Uint32List idx, List<Object?> dataArg,
    int mask, int used, List old, int from, int until);

final _uninitializedIndex = new Uint32List(_HashBase._UNINITIALIZED_INDEX_SIZE);
// Note: not const. Const arrays are made immutable by having a different class
// than regular arrays that throws on element assignment. We want the data field
// in maps and sets to be monomorphic.
final _uninitializedData = new List.filled(0, null);

// VM-internalized implementation of a default-constructed LinkedHashMap. Map
// literals also create instances of this class.
@pragma("vm:entry-point")
base class _Map<K, V> extends _HashVMBase
    with
        MapMixin<K, V>,
        _HashBase,
        _OperatorEqualsAndHashCode,
        _LinkedHashMapMixin<K, V>
    implements LinkedHashMap<K, V> {
  _Map() {
    _index = _uninitializedIndex;
    _hashMask = _HashBase._UNINITIALIZED_HASH_MASK;
    _data = _uninitializedData;
    _usedData = 0;
    _deletedKeys = 0;
  }

  // DartForge: `[]=`, `[]` and `containsKey` go through the runtime first
  // (`_dfMapStore`/`_dfMapLookup`); the rest is the mixin's.
  void operator []=(K key, V value) {
    final int used =
        _dfMapStore(_index, _data, _hashMask, _usedData, key, value);
    if (used >= 0) {
      _usedData = used;
      return;
    }
    _set(key, value, _hashCode(key));
  }

  V? operator [](Object? key) {
    final Uint32List idx = _index;
    Object? v = _dfMapLookup(idx, _data, _hashMask, key);
    if (identical(v, idx)) v = _getValueOrData(key);
    return identical(_data, v) ? null : internal.unsafeCast<V>(v);
  }

  bool containsKey(Object? key) {
    final Uint32List idx = _index;
    Object? v = _dfMapLookup(idx, _data, _hashMask, key);
    if (identical(v, idx)) v = _getValueOrData(key);
    return !identical(_data, v);
  }

  // DartForge: the mixin's `_init` (the `_rehash`) with the batch reinsertion
  // (`_dfMapFill`): the same order, and each entry the runtime does not know
  // through the usual `this[key] = …`.
  void _init(int size, int hashMask, List? oldData, int oldUsed) {
    if (size < _HashBase._INITIAL_INDEX_SIZE) {
      size = _HashBase._INITIAL_INDEX_SIZE;
      hashMask = _HashBase._indexSizeToHashMask(size);
    }
    assert(size & (size - 1) == 0);
    assert(_HashBase._UNUSED_PAIR == 0);
    _index = new Uint32List(size);
    _hashMask = hashMask;
    _data = _dfNewData(size);
    _usedData = 0;
    _deletedKeys = 0;
    if (oldData != null) {
      int i = 0;
      while (i < oldUsed) {
        final int r = _dfMapFill(
            _index, _data, _hashMask, _usedData, oldData, i, oldUsed, true);
        _usedData = r & 0xFFFFFFFF;
        i = r >> 32;
        if (i < oldUsed) {
          var key = oldData[i];
          if (!_HashBase._isDeleted(oldData, key)) {
            this[key] = oldData[i + 1];
          }
          i += 2;
        }
      }
    }
  }

  // DartForge: the mixin's `_populateUnsafe` through the batch.
  void _populateUnsafe(List<Object?> keyValuePairs) {
    _dfFillFromRange(keyValuePairs, 0, keyValuePairs.length);
  }

  /// DartForge: the `_populateUnsafe` of the pairs `pairs[from..until)` (the map
  /// of a JSON object, from the listener's stack), without checking types: the
  /// same capacity as `_populateUnsafe` and the stores of `_set`, in order.
  void _dfFillFromRange(List<Object?> pairs, int from, int until) {
    assert((until - from).isEven);
    int size = _roundUpToPowerOfTwo(until - from);
    if (size < _HashBase._INITIAL_INDEX_SIZE) {
      size = _HashBase._INITIAL_INDEX_SIZE;
    }
    int hashMask = _HashBase._indexSizeToHashMask(size);

    assert(size & (size - 1) == 0);
    assert(_HashBase._UNUSED_PAIR == 0);
    _index = new Uint32List(size);
    _hashMask = hashMask;
    _data = _dfNewData(size);
    _usedData = 0;
    _deletedKeys = 0;
    int i = from;
    while (i < until) {
      final int r = _dfMapFill(
          _index, _data, _hashMask, _usedData, pairs, i, until, false);
      _usedData = r & 0xFFFFFFFF;
      i = r >> 32;
      if (i < until) {
        final key = internal.unsafeCast<K>(pairs[i]);
        final value = internal.unsafeCast<V>(pairs[i + 1]);
        _set(key, value, _hashCode(key));
        i += 2;
      }
    }
  }

  void addAll(Map<K, V> other) {
    if (other case final _Map otherBase) {
      // If this map is empty we might be able to block-copy from [other].
      if (isEmpty && _quickCopy(otherBase)) return;
      // TODO(48143): Pre-grow capacity if it will reduce rehashing.
    }
    super.addAll(other);
  }
}

// This is essentially the same class as _Map, but it does
// not permit any modification of map entries from Dart code. We use
// this class for maps constructed from Dart constant maps.
@pragma("vm:entry-point")
base class _ConstMap<K, V> extends _HashVMImmutableBase
    with
        MapMixin<K, V>,
        _HashBase,
        _OperatorEqualsAndCanonicalHashCode,
        _LinkedHashMapMixin<K, V>,
        _UnmodifiableMapMixin<K, V>,
        _ImmutableLinkedHashMapMixin<K, V>
    implements LinkedHashMap<K, V> {
  factory _ConstMap._uninstantiable() {
    throw new UnsupportedError("_ConstMap can only be allocated by the VM");
  }

  // DartForge: the constant `const {…}` is the `_Map` of the literal adopted here
  // (the VM allocates the `_ConstMap` straight from the kernel). The index is
  // created on the first lookup, as in the VM's.
  @pragma("vm:entry-point")
  _ConstMap._fromSource(_HashVMBase source) {
    // Like the VM's: the exact (immutable) data and the hash mask that the lazy
    // `_createIndex` checks.
    _data = new List<Object?>.unmodifiable(source._data.take(source._usedData));
    _usedData = source._usedData;
    _deletedKeys = source._deletedKeys;
    _hashMask = _HashBase._indexSizeToHashMask(
        _roundUpToPowerOfTwo(max(_data.length, _HashBase._INITIAL_INDEX_SIZE)));
  }
}

mixin _ImmutableLinkedHashMapMixin<K, V>
    on _LinkedHashMapMixin<K, V>, _HashAbstractImmutableBase {
  bool containsKey(Object? key) {
    if (_indexNullable == null) {
      _createIndex();
    }
    return super.containsKey(key);
  }

  V? operator [](Object? key) {
    if (_indexNullable == null) {
      _createIndex();
    }
    return super[key];
  }

  void _createIndex() {
    final size =
        _roundUpToPowerOfTwo(max(_data.length, _HashBase._INITIAL_INDEX_SIZE));
    final newIndex = new Uint32List(size);
    final hashMask = _HashBase._indexSizeToHashMask(size);
    assert(_hashMask == hashMask);

    for (int j = 0; j < _usedData; j += 2) {
      final key = _data[j] as K;

      final fullHash = _hashCode(key);
      final hashPattern = _HashBase._hashPattern(fullHash, hashMask, size);
      final d =
          _findValueOrInsertPoint(key, fullHash, hashPattern, size, newIndex);
      // We just allocated the index, so we should not find this key in it yet.
      assert(d <= 0);

      final i = -d;

      assert(1 <= hashPattern && hashPattern < (1 << 32));
      final index = j >> 1;
      assert((index & hashPattern) == 0);
      newIndex[i] = hashPattern | index;
    }

    // Publish new index, uses store release semantics.
    _index = newIndex;
  }

  Iterable<K> get keys =>
      _CompactIterableImmutable<K>(this, _data, _usedData, -2, 2);
  Iterable<V> get values =>
      _CompactIterableImmutable<V>(this, _data, _usedData, -1, 2);
}

// Implementation is from "Hacker's Delight" by Henry S. Warren, Jr.,
// figure 3-3, page 48, where the function is called clp2.
int _roundUpToPowerOfTwo(int x) {
  x = x - 1;
  x = x | (x >> 1);
  x = x | (x >> 2);
  x = x | (x >> 4);
  x = x | (x >> 8);
  x = x | (x >> 16);
  x = x | (x >> 32);
  return x + 1;
}

mixin _LinkedHashMapMixin<K, V> on _HashBase, _EqualsAndHashCode {
  int get length => (_usedData >> 1) - _deletedKeys;
  bool get isEmpty => length == 0;
  bool get isNotEmpty => !isEmpty;

  void _rehash() {
    if ((_deletedKeys << 2) > _usedData) {
      // TODO(koda): Consider shrinking.
      // TODO(koda): Consider in-place compaction and more costly CME check.
      _init(_index.length, _hashMask, _data, _usedData);
    } else {
      // TODO(koda): Support 32->64 bit transition (and adjust _hashMask).
      _init(_index.length << 1, _hashMask >> 1, _data, _usedData);
    }
  }

  void clear() {
    if (!isEmpty) {
      _index = _uninitializedIndex;
      _hashMask = _HashBase._UNINITIALIZED_HASH_MASK;
      _data = _uninitializedData;
      _usedData = 0;
      _deletedKeys = 0;
    }
  }

  // Allocate _index and _data, and optionally copy existing contents.
  void _init(int size, int hashMask, List? oldData, int oldUsed) {
    if (size < _HashBase._INITIAL_INDEX_SIZE) {
      size = _HashBase._INITIAL_INDEX_SIZE;
      hashMask = _HashBase._indexSizeToHashMask(size);
    }
    assert(size & (size - 1) == 0);
    assert(_HashBase._UNUSED_PAIR == 0);
    _index = new Uint32List(size);
    _hashMask = hashMask;
    _data = new List.filled(size, null);
    _usedData = 0;
    _deletedKeys = 0;
    if (oldData != null) {
      for (int i = 0; i < oldUsed; i += 2) {
        var key = oldData[i];
        if (!_HashBase._isDeleted(oldData, key)) {
          // TODO(koda): While there are enough hash bits, avoid hashCode calls.
          this[key] = oldData[i + 1];
        }
      }
    }
  }

  /// Populate this hash table from the given list of key-value pairs.
  ///
  /// This function is unsafe: it does not perform any type checking on
  /// keys and values assuming that caller has ensured that types are
  /// correct.
  void _populateUnsafe(List<Object?> keyValuePairs) {
    assert(keyValuePairs.length.isEven);
    int size = _roundUpToPowerOfTwo(keyValuePairs.length);
    if (size < _HashBase._INITIAL_INDEX_SIZE) {
      size = _HashBase._INITIAL_INDEX_SIZE;
    }
    int hashMask = _HashBase._indexSizeToHashMask(size);

    assert(size & (size - 1) == 0);
    assert(_HashBase._UNUSED_PAIR == 0);
    _index = new Uint32List(size);
    _hashMask = hashMask;
    _data = new List.filled(size, null);
    _usedData = 0;
    _deletedKeys = 0;
    for (int i = 0; i < keyValuePairs.length; i += 2) {
      final key = internal.unsafeCast<K>(keyValuePairs[i]);
      final value = internal.unsafeCast<V>(keyValuePairs[i + 1]);
      _set(key, value, _hashCode(key));
    }
  }

  void _regenerateIndex() {
    _index =
        _data.length == 0 ? _uninitializedIndex : new Uint32List(_data.length);
    assert(_hashMask == _HashBase._UNINITIALIZED_HASH_MASK);
    _hashMask = _HashBase._indexSizeToHashMask(_index.length);
    final int tmpUsed = _usedData;
    _usedData = 0;
    _deletedKeys = 0;
    for (int i = 0; i < tmpUsed; i += 2) {
      final key = _data[i];
      if (!_HashBase._isDeleted(_data, key)) {
        this[key as K] = _data[i + 1] as V;
      }
    }
  }

  void _insert(K key, V value, int fullHash, int hashPattern, int i) {
    if (_usedData == _data.length) {
      _rehash();
      _set(key, value, fullHash);
    } else {
      assert(1 <= hashPattern && hashPattern < (1 << 32));
      final int index = _usedData >> 1;
      assert((index & hashPattern) == 0);
      _index[i] = hashPattern | index;
      _data[_usedData++] = key;
      _data[_usedData++] = value;
    }
  }

  // If key is present, returns the index of the value in _data, else returns
  // the negated insertion point in index.
  int _findValueOrInsertPoint(
      K key, int fullHash, int hashPattern, int size, Uint32List index) {
    final int sizeMask = size - 1;
    final int maxEntries = size >> 1;
    int i = _HashBase._firstProbe(fullHash, sizeMask);
    int firstDeleted = -1;
    int pair = index[i];
    while (pair != _HashBase._UNUSED_PAIR) {
      if (pair == _HashBase._DELETED_PAIR) {
        if (firstDeleted < 0) {
          firstDeleted = i;
        }
      } else {
        final int entry = hashPattern ^ pair;
        if (entry < maxEntries) {
          final int d = entry << 1;
          if (_equals(key, _data[d])) {
            return d + 1;
          }
        }
      }
      i = _HashBase._nextProbe(i, sizeMask);
      pair = index[i];
    }
    return firstDeleted >= 0 ? -firstDeleted : -i;
  }

  void operator []=(K key, V value) {
    final int fullHash = _hashCode(key);
    _set(key, value, fullHash);
  }

  void _set(K key, V value, int fullHash) {
    final int size = _index.length;
    final int hashPattern = _HashBase._hashPattern(fullHash, _hashMask, size);
    final int d =
        _findValueOrInsertPoint(key, fullHash, hashPattern, size, _index);
    if (d > 0) {
      _data[d] = value;
    } else {
      final int i = -d;
      _insert(key, value, fullHash, hashPattern, i);
    }
  }

  V putIfAbsent(K key, V ifAbsent()) {
    final int size = _index.length;
    final int fullHash = _hashCode(key);
    final int hashPattern = _HashBase._hashPattern(fullHash, _hashMask, size);
    final int d =
        _findValueOrInsertPoint(key, fullHash, hashPattern, size, _index);
    if (d > 0) {
      return _data[d] as V;
    }
    // 'ifAbsent' is allowed to modify the map.
    List oldData = _data;
    int oldCheckSum = _checkSum;
    V value = ifAbsent();
    if (_isModifiedSince(oldData, oldCheckSum)) {
      this[key] = value;
    } else {
      final int i = -d;
      _insert(key, value, fullHash, hashPattern, i);
    }
    return value;
  }

  V? remove(Object? key) {
    final int size = _index.length;
    final int sizeMask = size - 1;
    final int maxEntries = size >> 1;
    final int fullHash = _hashCode(key);
    final int hashPattern = _HashBase._hashPattern(fullHash, _hashMask, size);
    int i = _HashBase._firstProbe(fullHash, sizeMask);
    int pair = _index[i];
    while (pair != _HashBase._UNUSED_PAIR) {
      if (pair != _HashBase._DELETED_PAIR) {
        final int entry = hashPattern ^ pair;
        if (entry < maxEntries) {
          final int d = entry << 1;
          if (_equals(key, _data[d])) {
            _index[i] = _HashBase._DELETED_PAIR;
            _HashBase._setDeletedAt(_data, d);
            V value = _data[d + 1] as V;
            _HashBase._setDeletedAt(_data, d + 1);
            ++_deletedKeys;
            return value;
          }
        }
      }
      i = _HashBase._nextProbe(i, sizeMask);
      pair = _index[i];
    }
    return null;
  }

  // If key is absent, return _data (which is never a value).
  Object? _getValueOrData(Object? key) {
    final int size = _index.length;
    final int sizeMask = size - 1;
    final int maxEntries = size >> 1;
    final int fullHash = _hashCode(key);
    final int hashPattern = _HashBase._hashPattern(fullHash, _hashMask, size);
    int i = _HashBase._firstProbe(fullHash, sizeMask);
    int pair = _index[i];
    while (pair != _HashBase._UNUSED_PAIR) {
      if (pair != _HashBase._DELETED_PAIR) {
        final int entry = hashPattern ^ pair;
        if (entry < maxEntries) {
          final int d = entry << 1;
          if (_equals(key, _data[d])) {
            return _data[d + 1];
          }
        }
      }
      i = _HashBase._nextProbe(i, sizeMask);
      pair = _index[i];
    }
    return _data;
  }

  bool containsKey(Object? key) => !identical(_data, _getValueOrData(key));

  V? operator [](Object? key) {
    var v = _getValueOrData(key);
    return identical(_data, v) ? null : internal.unsafeCast<V>(v);
  }

  bool containsValue(Object? value) {
    for (var v in values) {
      // Spec. says this should always use "==", also for identity maps, etc.
      if (v == value) {
        return true;
      }
    }
    return false;
  }

  void forEach(void action(K key, V value)) {
    final data = _data;
    final checkSum = _checkSum;
    final len = _usedData;
    for (int offset = 0; offset < len; offset += 2) {
      final current = data[offset];
      if (_HashBase._isDeleted(data, current)) continue;
      final key = internal.unsafeCast<K>(current);
      final value = internal.unsafeCast<V>(data[offset + 1]);
      action(key, value);
      if (_isModifiedSince(data, checkSum)) {
        throw ConcurrentModificationError(this);
      }
    }
  }

  Iterable<K> get keys => _CompactIterable<K>(this, -2, 2);
  Iterable<V> get values => _CompactIterable<V>(this, -1, 2);
}

base class CompactLinkedIdentityHashMap<K, V> extends _HashFieldBase
    with
        MapMixin<K, V>,
        _HashBase,
        _IdenticalAndIdentityHashCode,
        _LinkedHashMapMixin<K, V>
    implements LinkedHashMap<K, V> {
  void addAll(Map<K, V> other) {
    if (other case final CompactLinkedIdentityHashMap otherBase) {
      // If this map is empty we might be able to block-copy from [other].
      if (isEmpty && _quickCopy(otherBase)) return;
      // TODO(48143): Pre-grow capacity if it will reduce rehashing.
    }
    super.addAll(other);
  }
}

base class CompactLinkedCustomHashMap<K, V> extends _HashFieldBase
    with
        MapMixin<K, V>,
        _HashBase,
        _CustomEqualsAndHashCode<K>,
        _LinkedHashMapMixin<K, V>
    implements LinkedHashMap<K, V> {
  final bool Function(K, K) _equality;
  final int Function(K) _hasher;
  final bool Function(Object?) _validKey;

  bool containsKey(Object? o) => _validKey(o) ? super.containsKey(o) : false;
  V? operator [](Object? o) => _validKey(o) ? super[o] : null;
  V? remove(Object? o) => _validKey(o) ? super.remove(o) : null;

  CompactLinkedCustomHashMap(
      this._equality, this._hasher, bool Function(Object?)? validKey)
      : _validKey = validKey ?? TypeTest<K>().test;
}

// Iterates through _data[_offset + _step], _data[_offset + 2*_step], ...
// and checks for concurrent modification.
class _CompactIterable<E> extends Iterable<E> {
  final _HashBase _table;
  final int _offset;
  final int _step;

  _CompactIterable(this._table, this._offset, this._step);

  Iterator<E> get iterator => _CompactIterator<E>(
      _table, _table._data, _table._usedData, _offset, _step);

  int get length => _table.length;
  bool get isEmpty => length == 0;
  bool get isNotEmpty => !isEmpty;
}

class _CompactIterator<E> implements Iterator<E> {
  final _HashBase _table;
  // dart:core#_List (sdk/lib/_internal/vm/lib/array.dart).
  final List _data;
  final int _len;
  int _offset;
  final int _step;
  final int _checkSum;
  E? _current;

  _CompactIterator(this._table, this._data, this._len, this._offset, this._step)
      : _checkSum = _table._checkSum;

  bool moveNext() {
    if (_table._isModifiedSince(_data, _checkSum)) {
      throw new ConcurrentModificationError(_table);
    }
    do {
      _offset += _step;
    } while (_offset < _len && _HashBase._isDeleted(_data, _data[_offset]));
    if (_offset < _len) {
      _current = internal.unsafeCast<E>(_data[_offset]);
      return true;
    } else {
      _current = null;
      return false;
    }
  }

  E get current => _current as E;
}

// Iterates through _data[_offset + _step], _data[_offset + 2*_step], ...
//
// Does not check for concurrent modification since the table
// is known to be immutable.
class _CompactIterableImmutable<E> extends Iterable<E> {
  // _HashBase with _HashVMImmutableBase.
  final _HashBase _table;
  // dart:core#_ImmutableList (sdk/lib/_internal/vm/lib/array.dart).
  final List _data;
  final int _len;
  final int _offset;
  final int _step;

  _CompactIterableImmutable(
      this._table, this._data, this._len, this._offset, this._step);

  Iterator<E> get iterator =>
      _CompactIteratorImmutable<E>(_table, _data, _len, _offset, _step);

  int get length => _table.length;
  bool get isEmpty => length == 0;
  bool get isNotEmpty => !isEmpty;
}

class _CompactIteratorImmutable<E> implements Iterator<E> {
  // _HashBase with _HashVMImmutableBase.
  final _HashBase _table;
  // dart:core#_ImmutableList (sdk/lib/_internal/vm/lib/array.dart).
  final List _data;
  final int _len;
  int _offset;
  final int _step;
  E? _current;

  _CompactIteratorImmutable(
      this._table, this._data, this._len, this._offset, this._step);

  bool moveNext() {
    _offset += _step;
    if (_offset < _len) {
      _current = internal.unsafeCast<E>(_data[_offset]);
      return true;
    } else {
      _current = null;
      return false;
    }
  }

  E get current => _current as E;
}

mixin _LinkedHashSetMixin<E> on _HashBase, _EqualsAndHashCode {
  bool get isEmpty => length == 0;
  bool get isNotEmpty => !isEmpty;
  int get length => _usedData - _deletedKeys;

  E get first {
    for (int offset = 0; offset < _usedData; offset++) {
      Object? current = _data[offset];
      if (!_HashBase._isDeleted(_data, current)) {
        return current as E;
      }
    }
    throw IterableElementError.noElement();
  }

  E get last {
    for (int offset = _usedData - 1; offset >= 0; offset--) {
      Object? current = _data[offset];
      if (!_HashBase._isDeleted(_data, current)) {
        return current as E;
      }
    }
    throw IterableElementError.noElement();
  }

  void _rehash() {
    if ((_deletedKeys << 1) > _usedData) {
      _init(_index.length, _hashMask, _data, _usedData);
    } else {
      _init(_index.length << 1, _hashMask >> 1, _data, _usedData);
    }
  }

  void clear() {
    if (!isEmpty) {
      _index = _uninitializedIndex;
      _hashMask = _HashBase._UNINITIALIZED_HASH_MASK;
      _data = _uninitializedData;
      _usedData = 0;
      _deletedKeys = 0;
    }
  }

  void _init(int size, int hashMask, List? oldData, int oldUsed) {
    if (size < _HashBase._INITIAL_INDEX_SIZE) {
      size = _HashBase._INITIAL_INDEX_SIZE;
      hashMask = _HashBase._indexSizeToHashMask(size);
    }
    _index = new Uint32List(size);
    _hashMask = hashMask;
    _data = new List.filled(size >> 1, null);
    _usedData = 0;
    _deletedKeys = 0;
    if (oldData != null) {
      for (int i = 0; i < oldUsed; i += 1) {
        var key = oldData[i];
        if (!_HashBase._isDeleted(oldData, key)) {
          add(key);
        }
      }
    }
  }

  bool add(E key) {
    final int fullHash = _hashCode(key);
    return _add(key, fullHash);
  }

  bool _add(E key, int fullHash) {
    final int size = _index.length;
    final int sizeMask = size - 1;
    final int maxEntries = size >> 1;
    final int hashPattern = _HashBase._hashPattern(fullHash, _hashMask, size);
    int i = _HashBase._firstProbe(fullHash, sizeMask);
    int firstDeleted = -1;
    int pair = _index[i];
    while (pair != _HashBase._UNUSED_PAIR) {
      if (pair == _HashBase._DELETED_PAIR) {
        if (firstDeleted < 0) {
          firstDeleted = i;
        }
      } else {
        final int d = hashPattern ^ pair;
        if (d < maxEntries && _equals(key, _data[d])) {
          return false;
        }
      }
      i = _HashBase._nextProbe(i, sizeMask);
      pair = _index[i];
    }
    if (_usedData == _data.length) {
      _rehash();
      _add(key, fullHash);
    } else {
      final int insertionPoint = (firstDeleted >= 0) ? firstDeleted : i;
      assert(1 <= hashPattern && hashPattern < (1 << 32));
      assert((hashPattern & _usedData) == 0);
      _index[insertionPoint] = hashPattern | _usedData;
      _data[_usedData++] = key;
    }
    return true;
  }

  // If key is absent, return _data (which is never a value).
  Object? _getKeyOrData(Object? key) {
    final int size = _index.length;
    final int sizeMask = size - 1;
    final int maxEntries = size >> 1;
    final int fullHash = _hashCode(key);
    final int hashPattern = _HashBase._hashPattern(fullHash, _hashMask, size);
    int i = _HashBase._firstProbe(fullHash, sizeMask);
    int pair = _index[i];
    while (pair != _HashBase._UNUSED_PAIR) {
      if (pair != _HashBase._DELETED_PAIR) {
        final int d = hashPattern ^ pair;
        if (d < maxEntries && _equals(key, _data[d])) {
          return _data[d]; // Note: Must return the existing key.
        }
      }
      i = _HashBase._nextProbe(i, sizeMask);
      pair = _index[i];
    }
    return _data;
  }

  E? lookup(Object? key) {
    var k = _getKeyOrData(key);
    return identical(_data, k) ? null : internal.unsafeCast<E>(k);
  }

  bool contains(Object? key) => !identical(_data, _getKeyOrData(key));

  bool remove(Object? key) {
    final int size = _index.length;
    final int sizeMask = size - 1;
    final int maxEntries = size >> 1;
    final int fullHash = _hashCode(key);
    final int hashPattern = _HashBase._hashPattern(fullHash, _hashMask, size);
    int i = _HashBase._firstProbe(fullHash, sizeMask);
    int pair = _index[i];
    while (pair != _HashBase._UNUSED_PAIR) {
      if (pair != _HashBase._DELETED_PAIR) {
        final int d = hashPattern ^ pair;
        if (d < maxEntries && _equals(key, _data[d])) {
          _index[i] = _HashBase._DELETED_PAIR;
          _HashBase._setDeletedAt(_data, d);
          ++_deletedKeys;
          return true;
        }
      }
      i = _HashBase._nextProbe(i, sizeMask);
      pair = _index[i];
    }

    return false;
  }

  Iterator<E> get iterator =>
      _CompactIterator<E>(this, _data, _usedData, -1, 1);

  void _regenerateIndex() {
    final size =
        _roundUpToPowerOfTwo(max(_data.length, _HashBase._INITIAL_INDEX_SIZE));
    _index = _data.length == 0 ? _uninitializedIndex : new Uint32List(size);
    assert(_hashMask == _HashBase._UNINITIALIZED_HASH_MASK);
    _hashMask = _HashBase._indexSizeToHashMask(_index.length);
    _rehash();
  }
}

// Set implementation, analogous to _Map. Set literals create instances of this
// class.
@pragma("vm:entry-point")
base class _Set<E> extends _HashVMBase
    with
        SetMixin<E>,
        _HashBase,
        _OperatorEqualsAndHashCode,
        _LinkedHashSetMixin<E>
    implements LinkedHashSet<E> {
  _Set() {
    _index = _uninitializedIndex;
    _hashMask = _HashBase._UNINITIALIZED_HASH_MASK;
    _data = _uninitializedData;
    _usedData = 0;
    _deletedKeys = 0;
  }

  // DartForge: `add`, `contains` and `lookup` go through the runtime first
  // (`_dfSetAdd`/`_dfSetLookup`); the rest is the mixin's.
  bool add(E key) {
    final int before = _usedData;
    final int used =
        _dfSetAdd(_index, _data, _hashMask, before, key);
    if (used >= 0) {
      if (used == before) return false;
      _usedData = used;
      return true;
    }
    return _add(key, _hashCode(key));
  }

  bool contains(Object? key) {
    final Uint32List idx = _index;
    Object? k = _dfSetLookup(idx, _data, _hashMask, key);
    if (identical(k, idx)) k = _getKeyOrData(key);
    return !identical(_data, k);
  }

  E? lookup(Object? key) {
    final Uint32List idx = _index;
    Object? k = _dfSetLookup(idx, _data, _hashMask, key);
    if (identical(k, idx)) k = _getKeyOrData(key);
    return identical(_data, k) ? null : internal.unsafeCast<E>(k);
  }

  // DartForge: the mixin's `_init` (the `_rehash`) with the batch reinsertion
  // (`_dfSetFill`); each key the runtime does not know through the usual
  // `add`.
  void _init(int size, int hashMask, List? oldData, int oldUsed) {
    if (size < _HashBase._INITIAL_INDEX_SIZE) {
      size = _HashBase._INITIAL_INDEX_SIZE;
      hashMask = _HashBase._indexSizeToHashMask(size);
    }
    _index = new Uint32List(size);
    _hashMask = hashMask;
    _data = _dfNewData(size >> 1);
    _usedData = 0;
    _deletedKeys = 0;
    if (oldData != null) {
      int i = 0;
      while (i < oldUsed) {
        final int r = _dfSetFill(
            _index, _data, _hashMask, _usedData, oldData, i, oldUsed);
        _usedData = r & 0xFFFFFFFF;
        i = r >> 32;
        if (i < oldUsed) {
          var key = oldData[i];
          if (!_HashBase._isDeleted(oldData, key)) {
            add(key);
          }
          i += 1;
        }
      }
    }
  }

  void addAll(Iterable<E> other) {
    if (other case final _Set otherBase) {
      // If this set is empty we might be able to block-copy from [other].
      if (isEmpty && _quickCopy(otherBase)) return;
      // TODO(48143): Pre-grow capacity if it will reduce rehashing.
    }
    super.addAll(other);
  }

  Set<R> cast<R>() => Set.castFrom<E, R>(this, newSet: _newEmpty);

  static Set<R> _newEmpty<R>() => _Set<R>();

  Set<E> toSet() => _Set<E>()..addAll(this);
}

@pragma("vm:entry-point")
base class _ConstSet<E> extends _HashVMImmutableBase
    with
        SetMixin<E>,
        _HashBase,
        _OperatorEqualsAndCanonicalHashCode,
        _LinkedHashSetMixin<E>,
        _UnmodifiableSetMixin<E>,
        _ImmutableLinkedHashSetMixin<E>
    implements LinkedHashSet<E> {
  factory _ConstSet._uninstantiable() {
    throw new UnsupportedError("_ConstSet can only be allocated by the VM");
  }

  // DartForge: like `_ConstMap._fromSource`, for `const {a, b}`.
  @pragma("vm:entry-point")
  _ConstSet._fromSource(_HashVMBase source) {
    // Like the VM's: the exact (immutable) data and the hash mask that the lazy
    // `_createIndex` checks.
    _data = new List<Object?>.unmodifiable(source._data.take(source._usedData));
    _usedData = source._usedData;
    _deletedKeys = source._deletedKeys;
    // The index of a set has twice the entries of `_data` (one key per entry; in
    // the map there are two): the same size as `_createIndex`'s.
    _hashMask = _HashBase._indexSizeToHashMask(
        _roundUpToPowerOfTwo(max(_data.length * 2, _HashBase._INITIAL_INDEX_SIZE)));
  }

  Set<R> cast<R>() => Set.castFrom<E, R>(this, newSet: _newEmpty);

  static Set<R> _newEmpty<R>() => _Set<R>();

  // Returns a mutable set.
  Set<E> toSet() => _Set<E>()..addAll(this);
}

mixin _ImmutableLinkedHashSetMixin<E>
    on Set<E>, _LinkedHashSetMixin<E>, _HashAbstractImmutableBase {
  E? lookup(Object? key) {
    if (_indexNullable == null) {
      _createIndex();
    }
    return super.lookup(key);
  }

  bool contains(Object? key) {
    if (_indexNullable == null) {
      _createIndex();
    }
    return super.contains(key);
  }

  void _createIndex() {
    final size = _roundUpToPowerOfTwo(
        max(_data.length * 2, _HashBase._INITIAL_INDEX_SIZE));
    final index = new Uint32List(size);
    final hashMask = _HashBase._indexSizeToHashMask(size);
    assert(_hashMask == hashMask);

    final sizeMask = size - 1;
    final maxEntries = size >> 1;

    for (int j = 0; j < _usedData; j++) {
      final key = _data[j];

      final fullHash = _hashCode(key);
      final hashPattern = _HashBase._hashPattern(fullHash, hashMask, size);

      int i = _HashBase._firstProbe(fullHash, sizeMask);
      int pair = index[i];
      while (pair != _HashBase._UNUSED_PAIR) {
        assert(pair != _HashBase._DELETED_PAIR);

        final int d = hashPattern ^ pair;
        if (d < maxEntries) {
          // We should not already find an entry in the index.
          assert(!_equals(key, _data[d]));
        }

        i = _HashBase._nextProbe(i, sizeMask);
        pair = index[i];
      }

      final int insertionPoint = i;
      assert(1 <= hashPattern && hashPattern < (1 << 32));
      assert((hashPattern & j) == 0);
      index[insertionPoint] = hashPattern | j;
    }

    // Publish new index, uses store release semantics.
    _index = index;
  }

  Iterator<E> get iterator =>
      _CompactIteratorImmutable<E>(this, _data, _usedData, -1, 1);
}

base class CompactLinkedIdentityHashSet<E> extends _HashFieldBase
    with
        SetMixin<E>,
        _HashBase,
        _IdenticalAndIdentityHashCode,
        _LinkedHashSetMixin<E>
    implements LinkedHashSet<E> {
  void addAll(Iterable<E> other) {
    if (other is CompactLinkedIdentityHashSet<E>) {
      // If this set is empty we might be able to block-copy from [other].
      if (isEmpty && _quickCopy(other)) return;
      // TODO(48143): Pre-grow capacity if it will reduce rehashing.
    }
    super.addAll(other);
  }

  Set<E> toSet() => CompactLinkedIdentityHashSet<E>()..addAll(this);

  static Set<R> _newEmpty<R>() => CompactLinkedIdentityHashSet<R>();

  Set<R> cast<R>() => Set.castFrom<E, R>(this, newSet: _newEmpty);
}

base class CompactLinkedCustomHashSet<E> extends _HashFieldBase
    with
        SetMixin<E>,
        _HashBase,
        _CustomEqualsAndHashCode<E>,
        _LinkedHashSetMixin<E>
    implements LinkedHashSet<E> {
  final bool Function(E, E) _equality;
  final int Function(E) _hasher;
  final bool Function(Object?) _validKey;

  bool contains(Object? o) => _validKey(o) ? super.contains(o) : false;
  E? lookup(Object? o) => _validKey(o) ? super.lookup(o) : null;
  bool remove(Object? o) => _validKey(o) ? super.remove(o) : false;

  CompactLinkedCustomHashSet(
      this._equality, this._hasher, bool Function(Object?)? validKey)
      : _validKey = validKey ?? TypeTest<E>().test;

  Set<R> cast<R>() => Set.castFrom<E, R>(this);
  Set<E> toSet() => CompactLinkedCustomHashSet<E>(_equality, _hasher, _validKey)
    ..addAll(this);
}

/// Expose [_Map] as [DefaultMap] and [_Set] as [DefaultSet] so that
/// `dart:collection` could use these classes. We maintain original private
/// names unchanged to avoid breaking user code which incorrectly relies
/// on stability of [Type.toString].
typedef DefaultMap<K, V> = _Map<K, V>;
typedef DefaultSet<E> = _Set<E>;

@pragma('vm:prefer-inline')
Map<K, V> createMapFromKeyValueListUnsafe<K, V>(List<Object?> keyValuePairs) =>
    DefaultMap<K, V>().._populateUnsafe(keyValuePairs);

/// DartForge: the map of the pairs `keyValuePairs[start..]` (up to the end of
/// the list), like [createMapFromKeyValueListUnsafe] of the sublist, without
/// copying it: the JSON listener builds each object from the range of the
/// value stack (`convert_patch.dart`, docs/NATIVO-PLANO.md §13.2, item 4).
Map<K, V> createMapFromKeyValueRangeUnsafe<K, V>(
        List<Object?> keyValuePairs, int start) =>
    DefaultMap<K, V>()
      .._dfFillFromRange(keyValuePairs, start, keyValuePairs.length);
