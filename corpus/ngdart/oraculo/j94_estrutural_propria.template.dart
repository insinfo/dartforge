// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j94_estrutural_propria.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j94_estrutural_propria.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import10;
import 'package:ngdart/src/runtime/check_binding.dart' as import11;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import13;
import 'package:ngdart/src/runtime/text_binding.dart' as import14;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import15;
import 'package:ngdart/src/runtime/interpolate.dart' as import16;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import17;

final List<Object> styles$J94EstruturalPropria = const [];

class ViewJ94EstruturalPropria0 extends import0.ComponentView<import1.J94EstruturalPropria> {
  late final ViewContainer _appEl_0;
  late final import1.J94Adiado _J94Adiado_0_9;
  late final ViewContainer _appEl_1;
  late final import1.J94Adiado _J94Adiado_1_9;
  late final ViewContainer _appEl_2;
  late final import1.J94Repetir _J94Repetir_2_9;
  Object? _expr_2;
  Object? _expr_3;
  static import3.ComponentStyles? _componentStyles;
  ViewJ94EstruturalPropria0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j94-estrutural-propria'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j94_estrutural_propria.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import8.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_J94EstruturalPropria1);
    this._J94Adiado_0_9 = import1.J94Adiado(this._appEl_0, _TemplateRef_0_8);
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(_anchor_0, this._J94Adiado_0_9);
    }
    final _anchor_1 = import8.appendAnchor(parentRenderNode);
    this._appEl_1 = ViewContainer(1, null, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J94EstruturalPropria2);
    this._J94Adiado_1_9 = import1.J94Adiado(this._appEl_1, _TemplateRef_1_8);
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(_anchor_1, this._J94Adiado_1_9);
    }
    final _anchor_2 = import8.appendAnchor(parentRenderNode);
    this._appEl_2 = ViewContainer(2, null, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_J94EstruturalPropria3);
    this._J94Repetir_2_9 = import1.J94Repetir(this._appEl_2, _TemplateRef_2_8);
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(_anchor_2, this._J94Repetir_2_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    if (firstCheck) {
      if (import10.isDevToolsEnabled) {
        import10.Inspector.instance.recordInput(this._J94Adiado_0_9, 'j94Adiado', true);
      }
      this._J94Adiado_0_9.preservar = true /* REF:asset:corpus_ngdart/lib/src/j94_estrutural_propria.dart:1022:1032 */;
    }
    if (((!import11.debugThrowIfChanged) && firstCheck)) {
      this._J94Adiado_0_9.ngOnInit();
    }
    if (firstCheck) {
      if (import10.isDevToolsEnabled) {
        import10.Inspector.instance.recordInput(this._J94Adiado_1_9, 'j94Adiado', true);
      }
      this._J94Adiado_1_9.preservar = true /* REF:asset:corpus_ngdart/lib/src/j94_estrutural_propria.dart:1060:1093 */;
    }
    final currVal_2 = _ctx.ligado;
    if (import11.checkBinding(this._expr_2, currVal_2, 'ligado', 'asset:corpus_ngdart/lib/src/j94_estrutural_propria.dart')) {
      if (import10.isDevToolsEnabled) {
        import10.Inspector.instance.recordInput(this._J94Adiado_1_9, 'j94AdiadoForcar', currVal_2);
      }
      this._J94Adiado_1_9.j94AdiadoForcar = currVal_2 /* REF:asset:corpus_ngdart/lib/src/j94_estrutural_propria.dart:1060:1093 */;
      this._expr_2 = currVal_2;
    }
    if (((!import11.debugThrowIfChanged) && firstCheck)) {
      this._J94Adiado_1_9.ngOnInit();
    }
    final currVal_3 = _ctx.itens;
    if (import11.checkBinding(this._expr_3, currVal_3, 'itens', 'asset:corpus_ngdart/lib/src/j94_estrutural_propria.dart')) {
      if (import10.isDevToolsEnabled) {
        import10.Inspector.instance.recordInput(this._J94Repetir_2_9, 'j94RepetirDe', currVal_3);
      }
      this._J94Repetir_2_9.j94RepetirDe = currVal_3 /* REF:asset:corpus_ngdart/lib/src/j94_estrutural_propria.dart:1102:1130 */;
      this._expr_3 = currVal_3;
    }
    this._appEl_0.detectChangesInNestedViews();
    this._appEl_1.detectChangesInNestedViews();
    this._appEl_2.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
    this._appEl_1.destroyNestedViews();
    this._appEl_2.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J94EstruturalPropria, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J94EstruturalPropriaNgFactory = ComponentFactory<import1.J94EstruturalPropria>('j94-estrutural-propria', viewFactory_J94EstruturalPropriaHost0);
ComponentFactory<import1.J94EstruturalPropria> get J94EstruturalPropriaNgFactory {
  return _J94EstruturalPropriaNgFactory;
}

ComponentFactory<import1.J94EstruturalPropria> createJ94EstruturalPropriaFactory() {
  return ComponentFactory('j94-estrutural-propria', viewFactory_J94EstruturalPropriaHost0);
}

class _ViewJ94EstruturalPropria1 extends import13.EmbeddedView<import1.J94EstruturalPropria> {
  final import14.TextBinding _textBinding_2 = import14.TextBinding();
  _ViewJ94EstruturalPropria1(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import7.document;
    final _el_0 = import6.unsafeCast(doc.createElement('div'));
    final _el_1 = import8.appendElement<import7.HtmlElement>(doc, _el_0, 'b');
    _el_1.append(this._textBinding_2.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_2.updateText(import16.interpolateString0(_ctx.texto)) /* REF:asset:corpus_ngdart/lib/src/j94_estrutural_propria.dart:1036:1047 */;
  }
}

import13.EmbeddedView<void> viewFactory_J94EstruturalPropria1(import15.RenderView parentView, int parentIndex) {
  return _ViewJ94EstruturalPropria1(parentView, parentIndex);
}

class _ViewJ94EstruturalPropria2 extends import13.EmbeddedView<import1.J94EstruturalPropria> {
  _ViewJ94EstruturalPropria2(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import7.document;
    final _el_0 = import6.unsafeCast(doc.createElement('p'));
    final _text_1 = import8.appendText(_el_0, 'x');
    this.initRootNode(_el_0);
  }
}

import13.EmbeddedView<void> viewFactory_J94EstruturalPropria2(import15.RenderView parentView, int parentIndex) {
  return _ViewJ94EstruturalPropria2(parentView, parentIndex);
}

class _ViewJ94EstruturalPropria3 extends import13.EmbeddedView<import1.J94EstruturalPropria> {
  final import14.TextBinding _textBinding_1 = import14.TextBinding();
  _ViewJ94EstruturalPropria3(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import7.document;
    final _el_0 = import6.unsafeCast(doc.createElement('i'));
    _el_0.append(this._textBinding_1.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_s = this.locals['\$implicit'];
    this._textBinding_1.updateText(import16.interpolate0(local_s)) /* REF:asset:corpus_ngdart/lib/src/j94_estrutural_propria.dart:1131:1138 */;
  }
}

import13.EmbeddedView<void> viewFactory_J94EstruturalPropria3(import15.RenderView parentView, int parentIndex) {
  return _ViewJ94EstruturalPropria3(parentView, parentIndex);
}

final List<Object> styles$J94EstruturalPropriaHost = const [];

class _ViewJ94EstruturalPropriaHost0 extends import17.HostView<import1.J94EstruturalPropria> {
  @override
  void build() {
    this.componentView = ViewJ94EstruturalPropria0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J94EstruturalPropria();
    this.initRootNode(_el_0);
  }
}

import17.HostView<import1.J94EstruturalPropria> viewFactory_J94EstruturalPropriaHost0() {
  return _ViewJ94EstruturalPropriaHost0();
}
