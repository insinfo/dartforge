import 'package:dartforge/dartforge.dart';
import 'package:dartforge/annotations.dart' as metadata;
import 'package:dartforge/dartforge.dart' as df;
import 'annotations_reexport.dart' as forwarded;

const preference = df.PreferRegion();

@PreferAcyclic()
final class Sample {
  @PreferAcyclic()
  const Sample(this.value);
  final double value;
}

@preference
double read(@NoEscape() Sample sample) => sample.value;

@forwarded.Realtime()
double gain(double value, double factor) => value * factor;

final class Processor {
  @forwarded.PreferRegion()
  double read(@df.NoEscape() Sample sample) => sample.value;

  @df.Realtime()
  double gain(double value, double factor) => value * factor;
}

void main() {
  @PreferAcyclic()
  const sample = Sample(2);
  final processor = Processor();
  if (dartForgeAnnotationsSchema != 1 ||
      forwarded.dartForgeAnnotationsSchema != 1 ||
      !identical(preference, const PreferRegion()) ||
      !identical(const metadata.Realtime(), const df.Realtime()) ||
      metadata.dartForgeAnnotationsSchema != dartForgeAnnotationsSchema ||
      !identical(const df.PreferAcyclic(), const forwarded.PreferAcyclic()) ||
      !identical(const df.NoEscape(), const NoEscape()) ||
      !identical(const df.Realtime(), const Realtime()) ||
      gain(read(sample), 3) != 6 ||
      processor.gain(processor.read(sample), 3) != 6) {
    throw StateError('annotation portability or constant identity');
  }
  print('annotation portability and schema validated');
}
