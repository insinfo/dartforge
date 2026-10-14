/// Optional optimization preferences and verification requests.
///
/// These are ordinary Dart constants. The current package does not prove the
/// requested properties or change runtime behavior. Compiler support requires
/// resolved declaration identity, a compatible schema, and separate proof.
library;

import 'package:meta/meta_meta.dart';

/// Version of the annotation API and its intended verification contracts.
const int dartForgeAnnotationsSchema = 1;

/// Requests region analysis for allocations in a synchronous function body.
///
/// ```dart
/// @PreferRegion()
/// int sum(List<int> values) => values.fold(0, (a, b) => a + b);
/// ```
///
/// A preference does not force stack allocation or waive escape checks. Only
/// synchronous functions or methods with bodies are intended targets; `Target`
/// cannot express the body or synchronicity requirement.
@Target({TargetKind.function, TargetKind.method})
final class PreferRegion {
  const PreferRegion();
}

/// Requests acyclicity analysis for a class, constructor, or local variable.
///
/// ```dart
/// @PreferAcyclic()
/// final class Point {
///   @PreferAcyclic()
///   const Point(this.x);
///   final int x;
/// }
/// ```
///
/// This does not classify subclasses, reachable objects, or later mutations as
/// acyclic. `package:meta` has no local-variable target, so this annotation has
/// no `Target` restriction; its intended targets need compiler validation.
final class PreferAcyclic {
  const PreferAcyclic();
}

/// Requests proof that a call creates no persistent escape of this parameter.
///
/// ```dart
/// int first(@NoEscape() List<int> values) => values[0];
/// ```
///
/// Intended for parameters of functions or methods with available bodies or
/// certified summaries. It does not transfer ownership, erase existing aliases,
/// or promise absence of allocation, mutation, exceptions, or blocking. The
/// current compiler does not certify this contract.
@Target({TargetKind.parameter})
final class NoEscape {
  const NoEscape();
}

/// Requests the proposed synchronous, bounded `dsp-v1` execution contract.
///
/// ```dart
/// @Realtime()
/// double gain(double value, double factor) => value * factor;
/// ```
///
/// An annotation alone proves neither transitive effects nor device entry
/// safety, and provides no operating-system deadline guarantee. `async`,
/// `sync*`, and `async*` bodies are outside the intended contract. The current
/// compiler does not certify this contract.
@Target({TargetKind.function, TargetKind.method})
final class Realtime {
  const Realtime();
}
