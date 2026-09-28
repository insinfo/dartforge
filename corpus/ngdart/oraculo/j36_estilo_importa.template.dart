// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j36_estilo_importa.dart';
import 'package:corpus_ngdart/src/j36_estilo_importa.css.shim.dart' as import0;
import 'package:ngdart/src/core/linker/views/component_view.dart' as import1;
import 'j36_estilo_importa.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$J36EstiloImporta = [import0.styles];

class ViewJ36EstiloImporta0 extends import1.ComponentView<import2.J36EstiloImporta> {
  static import3.ComponentStyles? _componentStyles;
  ViewJ36EstiloImporta0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j36-estilo-importa'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j36_estilo_importa.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'i');
    this.updateChildClass(_el_0, 'a');
    this.addShimC(_el_0);
    final _text_1 = import8.appendText(_el_0, 'x');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.scoped(styles$J36EstiloImporta, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J36EstiloImportaNgFactory = ComponentFactory<import2.J36EstiloImporta>('j36-estilo-importa', viewFactory_J36EstiloImportaHost0);
ComponentFactory<import2.J36EstiloImporta> get J36EstiloImportaNgFactory {
  return _J36EstiloImportaNgFactory;
}

ComponentFactory<import2.J36EstiloImporta> createJ36EstiloImportaFactory() {
  return ComponentFactory('j36-estilo-importa', viewFactory_J36EstiloImportaHost0);
}

final List<Object> styles$J36EstiloImportaHost = const [];

class _ViewJ36EstiloImportaHost0 extends import10.HostView<import2.J36EstiloImporta> {
  @override
  void build() {
    this.componentView = ViewJ36EstiloImporta0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import2.J36EstiloImporta();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import2.J36EstiloImporta> viewFactory_J36EstiloImportaHost0() {
  return _ViewJ36EstiloImportaHost0();
}
