// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'b19_pipes_sem_uso.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'b19_pipes_sem_uso.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/check_binding.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$B19PipesSemUso = const [];

class ViewB19PipesSemUso0 extends import0.ComponentView<import1.B19PipesSemUso> {
  Object? _expr_0;
  late final import2.DivElement _el_0;
  static import3.ComponentStyles? _componentStyles;
  ViewB19PipesSemUso0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('b19-pipes-sem-uso'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/b19_pipes_sem_uso.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import2.document;
    this._el_0 = import7.appendDiv(doc, parentRenderNode);
    final _text_1 = import7.appendText(this._el_0, 'oi');
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = (_ctx.a || _ctx.b);
    if (import8.checkBinding(this._expr_0, currVal_0, 'a || b', 'package:corpus_ngdart/src/b19_pipes_sem_uso.html')) {
      import7.setProperty(this._el_0, 'hidden', currVal_0) /* REF:package:corpus_ngdart/src/b19_pipes_sem_uso.html:5:22 */;
      this._expr_0 = currVal_0;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$B19PipesSemUso, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _B19PipesSemUsoNgFactory = ComponentFactory<import1.B19PipesSemUso>('b19-pipes-sem-uso', viewFactory_B19PipesSemUsoHost0);
ComponentFactory<import1.B19PipesSemUso> get B19PipesSemUsoNgFactory {
  return _B19PipesSemUsoNgFactory;
}

ComponentFactory<import1.B19PipesSemUso> createB19PipesSemUsoFactory() {
  return ComponentFactory('b19-pipes-sem-uso', viewFactory_B19PipesSemUsoHost0);
}

final List<Object> styles$B19PipesSemUsoHost = const [];

class _ViewB19PipesSemUsoHost0 extends import10.HostView<import1.B19PipesSemUso> {
  @override
  void build() {
    this.componentView = ViewB19PipesSemUso0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.B19PipesSemUso();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.B19PipesSemUso> viewFactory_B19PipesSemUsoHost0() {
  return _ViewB19PipesSemUsoHost0();
}
