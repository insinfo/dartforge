// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j101_classe_estatica_mesclada.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j101_classe_estatica_mesclada.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/runtime/interpolate.dart' as import10;
import 'package:ngdart/src/runtime/check_binding.dart' as import11;

final List<Object> styles$J101Alternador = const [];

class ViewJ101Alternador0 extends import0.ComponentView<import1.J101Alternador> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ101Alternador0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j101-alternador'));
    this.updateChildClassNonHtml(this.rootElement, import1.J101Alternador.hostClass);
    import7.updateAttribute(this.rootElement, 'role', import1.J101Alternador.hostRole);
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j101_classe_estatica_mesclada.dart' : null);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J101Alternador, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J101AlternadorNgFactory = ComponentFactory<import1.J101Alternador>('j101-alternador', viewFactory_J101AlternadorHost0);
ComponentFactory<import1.J101Alternador> get J101AlternadorNgFactory {
  return _J101AlternadorNgFactory;
}

ComponentFactory<import1.J101Alternador> createJ101AlternadorFactory() {
  return ComponentFactory('j101-alternador', viewFactory_J101AlternadorHost0);
}

final List<Object> styles$J101AlternadorHost = const [];

class _ViewJ101AlternadorHost0 extends import9.HostView<import1.J101Alternador> {
  @override
  void build() {
    this.componentView = ViewJ101Alternador0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J101Alternador();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J101Alternador> viewFactory_J101AlternadorHost0() {
  return _ViewJ101AlternadorHost0();
}

final List<Object> styles$J101Usa = const [];

class ViewJ101Usa0 extends import0.ComponentView<import1.J101Usa> {
  late final ViewJ101Alternador0 _compView_0;
  late final import1.J101Alternador _J101Alternador_0_5;
  late final ViewJ101Alternador0 _compView_1;
  late final import1.J101Alternador _J101Alternador_1_5;
  late final ViewJ101Alternador0 _compView_2;
  late final import1.J101Alternador _J101Alternador_2_5;
  Object? _expr_0;
  late final import6.HtmlElement _el_2;
  static import2.ComponentStyles? _componentStyles;
  ViewJ101Usa0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j101-usa'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j101_classe_estatica_mesclada.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ101Alternador0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this.updateChildClassNonHtml(_el_0, import10.interpolate2('', 'a b', ' ', import1.J101Alternador.hostClass, ''));
    import7.setAttribute(_el_0, 'role', 'button');
    import7.setAttribute(_el_0, 'title', 't');
    this._J101Alternador_0_5 = import1.J101Alternador();
    this._compView_0.create(this._J101Alternador_0_5);
    this._compView_1 = ViewJ101Alternador0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    parentRenderNode.append(_el_1);
    this._J101Alternador_1_5 = import1.J101Alternador();
    this._compView_1.create(this._J101Alternador_1_5);
    this._compView_2 = ViewJ101Alternador0(this, 2);
    this._el_2 = this._compView_2.rootElement;
    parentRenderNode.append(this._el_2);
    this._J101Alternador_2_5 = import1.J101Alternador();
    this._compView_2.create(this._J101Alternador_2_5);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = import10.interpolateString0(_ctx.extra);
    if (import11.checkBinding(this._expr_0, currVal_0, '{{ extra }}', 'asset:corpus_ngdart/lib/src/j101_classe_estatica_mesclada.dart')) {
      this._compView_2.updateChildClassNonHtml(this._el_2, currVal_0) /* REF:asset:corpus_ngdart/lib/src/j101_classe_estatica_mesclada.dart:716:735 */;
      this._expr_0 = currVal_0;
    }
    this._compView_0.detectChanges();
    this._compView_1.detectChanges();
    this._compView_2.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
    this._compView_1.destroyInternalState();
    this._compView_2.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J101Usa, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J101UsaNgFactory = ComponentFactory<import1.J101Usa>('j101-usa', viewFactory_J101UsaHost0);
ComponentFactory<import1.J101Usa> get J101UsaNgFactory {
  return _J101UsaNgFactory;
}

ComponentFactory<import1.J101Usa> createJ101UsaFactory() {
  return ComponentFactory('j101-usa', viewFactory_J101UsaHost0);
}

final List<Object> styles$J101UsaHost = const [];

class _ViewJ101UsaHost0 extends import9.HostView<import1.J101Usa> {
  @override
  void build() {
    this.componentView = ViewJ101Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J101Usa();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J101Usa> viewFactory_J101UsaHost0() {
  return _ViewJ101UsaHost0();
}
