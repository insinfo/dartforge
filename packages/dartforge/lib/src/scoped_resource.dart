import 'dart:async';

/// A resource whose owner can explicitly end its lifetime.
///
/// Implementations decide how disposal works, including any thread requirements.
/// Scopes call [dispose] once; bodies must not dispose the resource themselves
/// or retain it beyond completion. These helpers do not prove that references
/// cannot escape.
abstract interface class ScopedResource {
  /// Ends the owned lifetime. An implementation may throw if cleanup fails.
  void dispose();
}

/// Adapts an existing resource without requiring it to implement an interface.
///
/// ```dart
/// import 'package:dartforge/resources.dart';
/// void main() {
///   final handle = ResourceHandle('file', (name) => print('closed $name'));
///   using(handle, (resource) => print(resource.value));
/// }
/// ```
final class ResourceHandle<T> implements ScopedResource {
  ResourceHandle(this.value, this._dispose);
  final T value;
  final void Function(T) _dispose;
  bool _disposed = false;
  bool get isDisposed => _disposed;

  /// Attempts cleanup at most once, including when the cleanup callback throws.
  @override
  void dispose() {
    if (_disposed) return;
    _disposed = true;
    _dispose(value);
  }
}

/// Runs a synchronous body and disposes [resource] in `finally`.
///
/// ```dart
/// import 'package:dartforge/resources.dart';
/// void main() {
///   final handle = ResourceHandle(3, (value) => print('released $value'));
///   print(using(handle, (resource) => resource.value * 2));
/// }
/// ```
///
/// The body MUST finish synchronously and must not retain the resource. Use
/// [usingAsync] for asynchronous work. Dart permits async callbacks where void
/// is expected and permits Future values through broad result types; this API
/// does not detect those contract violations or extend their resource lifetime.
/// No ARC policy, GC behavior or compiler escape proof is implied.
///
/// # Errors
/// Body errors propagate after disposal. If disposal throws, its error replaces
/// any pending body error, following ordinary Dart `try/finally` semantics.
TResult using<TResource extends ScopedResource, TResult>(
    TResource resource, TResult Function(TResource) body) {
  try {
    return body(resource);
  } finally {
    resource.dispose();
  }
}

/// Awaits the body before disposing [resource], including failed futures.
///
/// ```dart
/// import 'package:dartforge/resources.dart';
/// Future<void> main() async {
///   final handle = ResourceHandle(3, (value) => print('released $value'));
///   print(await usingAsync(handle, (resource) async => resource.value * 2));
/// }
/// ```
///
/// The body may return synchronously or asynchronously. It must not retain the
/// resource beyond completion or dispose it itself. Cleanup itself is
/// synchronous; resources requiring asynchronous cleanup need another API.
///
/// # Errors
/// Body errors propagate after disposal. A disposal error takes precedence over
/// a synchronous body error or failed future, following `try/finally` semantics.
Future<TResult> usingAsync<TResource extends ScopedResource, TResult>(
    TResource resource, FutureOr<TResult> Function(TResource) body) async {
  try {
    return await body(resource);
  } finally {
    resource.dispose();
  }
}
