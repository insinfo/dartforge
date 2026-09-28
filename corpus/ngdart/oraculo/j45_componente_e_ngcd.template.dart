// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j45_componente_e_ngcd.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j45_componente_e_ngcd.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/devtools.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;
import 'package:ngdart/src/core/change_detection/directive_change_detector.dart' as import11;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import12;
import 'package:ngdart/src/runtime/check_binding.dart' as import13;

final List<Object> styles$J45ComponenteENgcd = const [];

class ViewJ45ComponenteENgcd0 extends import0.ComponentView<import1.J45ComponenteENgcd> {
  late final J45AtivoNgCd _J45Ativo_0_5;
  late final import2.HtmlElement _el_0;
  static import3.ComponentStyles? _componentStyles;
  ViewJ45ComponenteENgcd0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('j45-componente-e-ngcd'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j45_componente_e_ngcd.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import2.document;
    this._el_0 = import7.appendElement<import2.HtmlElement>(doc, parentRenderNode, 'b');
    import7.setAttribute(this._el_0, 'j45-ativo', '');
    this._J45Ativo_0_5 = J45AtivoNgCd(import1.J45Ativo());
    if (import8.isDevToolsEnabled) {
      import8.Inspector.instance.registerDirective(this._el_0, this._J45Ativo_0_5.instance);
    }
    final _text_1 = import7.appendText(this._el_0, 'x');
  }

  @override
  void detectChangesInternal() {
    this._J45Ativo_0_5.detectHostChanges(this, this._el_0);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J45ComponenteENgcd, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J45ComponenteENgcdNgFactory = ComponentFactory<import1.J45ComponenteENgcd>('j45-componente-e-ngcd', viewFactory_J45ComponenteENgcdHost0);
ComponentFactory<import1.J45ComponenteENgcd> get J45ComponenteENgcdNgFactory {
  return _J45ComponenteENgcdNgFactory;
}

ComponentFactory<import1.J45ComponenteENgcd> createJ45ComponenteENgcdFactory() {
  return ComponentFactory('j45-componente-e-ngcd', viewFactory_J45ComponenteENgcdHost0);
}

final List<Object> styles$J45ComponenteENgcdHost = const [];

class _ViewJ45ComponenteENgcdHost0 extends import10.HostView<import1.J45ComponenteENgcd> {
  @override
  void build() {
    this.componentView = ViewJ45ComponenteENgcd0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J45ComponenteENgcd();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.J45ComponenteENgcd> viewFactory_J45ComponenteENgcdHost0() {
  return _ViewJ45ComponenteENgcdHost0();
}

class J45AtivoNgCd extends import11.DirectiveChangeDetector {
  final import1.J45Ativo instance;
  Object? _expr_0;
  Object? _expr_1;
  J45AtivoNgCd(this.instance);
  void detectHostChanges(import12.RenderView view, import2.Element el) {
    final currVal_0 = this.instance.selecionado;
    if (import13.checkBinding(this._expr_0, currVal_0, null, null)) {
      import7.updateAttribute(el, 'aria-selected', currVal_0);
      this._expr_0 = currVal_0;
    }
    final currVal_1 = this.instance.ativo;
    if (import13.checkBinding(this._expr_1, currVal_1, null, null)) {
      import7.updateClassBindingNonHtml(el, 'ativo', currVal_1);
      this._expr_1 = currVal_1;
    }
  }
}

class J45DepoisNgCd extends import11.DirectiveChangeDetector {
  final import1.J45Depois instance;
  Object? _expr_0;
  J45DepoisNgCd(this.instance);
  void detectHostChanges(import12.RenderView view, import2.Element el) {
    final currVal_0 = this.instance.depois;
    if (import13.checkBinding(this._expr_0, currVal_0, null, null)) {
      import7.updateClassBindingNonHtml(el, 'depois', currVal_0);
      this._expr_0 = currVal_0;
    }
  }
}
