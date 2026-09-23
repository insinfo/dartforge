// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'd08_filho_injetado.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'd08_filho_injetado.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/di/errors.dart' as import10;
import 'd08_servico.dart' as import11;

final List<Object> styles$D08FilhoInjetado = const [];

class ViewD08FilhoInjetado0 extends import0.ComponentView<import1.D08FilhoInjetado> {
  static import2.ComponentStyles? _componentStyles;
  ViewD08FilhoInjetado0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('d08-filho-injetado'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/d08_filho_injetado.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'i');
    final _text_1 = import7.appendText(_el_0, 'x');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$D08FilhoInjetado, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _D08FilhoInjetadoNgFactory = ComponentFactory<import1.D08FilhoInjetado>('d08-filho-injetado', viewFactory_D08FilhoInjetadoHost0);
ComponentFactory<import1.D08FilhoInjetado> get D08FilhoInjetadoNgFactory {
  return _D08FilhoInjetadoNgFactory;
}

ComponentFactory<import1.D08FilhoInjetado> createD08FilhoInjetadoFactory() {
  return ComponentFactory('d08-filho-injetado', viewFactory_D08FilhoInjetadoHost0);
}

final List<Object> styles$D08FilhoInjetadoHost = const [];

class _ViewD08FilhoInjetadoHost0 extends import9.HostView<import1.D08FilhoInjetado> {
  @override
  void build() {
    this.componentView = ViewD08FilhoInjetado0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = (import5.isDevMode
        ? import10.debugInjectorWrap(import1.D08FilhoInjetado, () {
            return import1.D08FilhoInjetado(this.injectorGet(import11.D08Servico, this.parentIndex), this.injectorGetOptional(import11.D08Opcional, this.parentIndex), _el_0, this.componentView);
          })
        : import1.D08FilhoInjetado(this.injectorGet(import11.D08Servico, this.parentIndex), this.injectorGetOptional(import11.D08Opcional, this.parentIndex), _el_0, this.componentView));
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.D08FilhoInjetado> viewFactory_D08FilhoInjetadoHost0() {
  return _ViewD08FilhoInjetadoHost0();
}
