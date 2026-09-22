// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'b06_encapsulation.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'b06_encapsulation.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$B06Encapsulation = const [];

class ViewB06Encapsulation0 extends import0.ComponentView<import1.B06Encapsulation> {
  static import2.ComponentStyles? _componentStyles;
  ViewB06Encapsulation0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('b06-encapsulation'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/b06_encapsulation.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    final _text_1 = import7.appendText(_el_0, 'oi');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$B06Encapsulation, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _B06EncapsulationNgFactory = ComponentFactory<import1.B06Encapsulation>('b06-encapsulation', viewFactory_B06EncapsulationHost0);
ComponentFactory<import1.B06Encapsulation> get B06EncapsulationNgFactory {
  return _B06EncapsulationNgFactory;
}

ComponentFactory<import1.B06Encapsulation> createB06EncapsulationFactory() {
  return ComponentFactory('b06-encapsulation', viewFactory_B06EncapsulationHost0);
}

final List<Object> styles$B06EncapsulationHost = const [];

class _ViewB06EncapsulationHost0 extends import9.HostView<import1.B06Encapsulation> {
  @override
  void build() {
    this.componentView = ViewB06Encapsulation0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.B06Encapsulation();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.B06Encapsulation> viewFactory_B06EncapsulationHost0() {
  return _ViewB06EncapsulationHost0();
}
