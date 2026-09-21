import 'dart:collection';
// Repro mínimo de um bug de geração de código do dart2js.
//
// Um getter `List.unmodifiable(...)` inlinado dentro de um método `async` com
// laços produz uma atribuição `result.$flags = 3` pendurada num caminho de
// junção onde `result` nunca foi atribuído. Roda correto na VM e no DDC.
enum ParagraphType { text, embed, block }

class Line {
  final String data;
  final Map<String, dynamic> attributes;
  Line(this.data, [Map<String, dynamic>? attributes])
      : attributes = attributes ?? <String, dynamic>{};
}

class Paragraph {
  final List<Line> _lines = <Line>[];
  final ParagraphType type;

  Paragraph(this.type, List<Line> lines) {
    _lines.addAll(lines);
  }

  // O ponto do bug: o getter é inlinado como "cópia + $flags = 3".
  List<Line> get lines => UnmodifiableListView<Line>(_lines);

  bool get isEmbed => lines.first.attributes.containsKey('embed');
}

class FakePdfService {
  int processed = 0;

  Future<void> blockGenerators(List<Paragraph> paragraphs) async {
    for (final paragraph in paragraphs) {
      final blockAttributes = <String, dynamic>{};
      if (paragraph.type == ParagraphType.embed && paragraph.isEmbed) {
        await Future<void>.delayed(Duration.zero);
        continue;
      }
      final isHeader = blockAttributes.containsKey('header');
      final isCodeBlock = blockAttributes.containsKey('code-block');
      for (var k = 0; k < paragraph.lines.length; k++) {
        final line = paragraph.lines[k];
        await Future<void>.delayed(Duration.zero);
        if (isHeader || isCodeBlock) continue;
        processed++;
        if (line.data.isEmpty) continue;
      }
    }
  }
}

Future<void> main() async {
  final service = FakePdfService();
  await service.blockGenerators(<Paragraph>[
    Paragraph(ParagraphType.text, <Line>[Line('a'), Line('b')]),
  ]);
  print('processed lines: ${service.processed}');
}
