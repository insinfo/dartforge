// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j58_atributo_interpolado_no_filho.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j58_atributo_interpolado_no_filho.dart' as import1;
import 'j58_filho_rotulo.template.dart' as import2;
import 'j58_filho_rotulo.dart' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/interpolate.dart' as import9;
import 'package:ngdart/src/runtime/check_binding.dart' as import10;
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import13;

final List<Object> styles$J58AtributoInterpoladoNoFilho = const [];

class ViewJ58AtributoInterpoladoNoFilho0 extends import0.ComponentView<import1.J58AtributoInterpoladoNoFilho> {
  late final import2.ViewJ58FilhoRotulo0 _compView_0;
  late final import3.J58FilhoRotulo _J58FilhoRotulo_0_5;
  late final import2.ViewJ58FilhoRotulo0 _compView_1;
  late final import3.J58FilhoRotulo _J58FilhoRotulo_1_5;
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  static import4.ComponentStyles? _componentStyles;
  ViewJ58AtributoInterpoladoNoFilho0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('j58-atributo-interpolado-no-filho'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j58_atributo_interpolado_no_filho.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewJ58FilhoRotulo0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._J58FilhoRotulo_0_5 = import3.J58FilhoRotulo();
    this._compView_0.create(this._J58FilhoRotulo_0_5);
    this._compView_1 = import2.ViewJ58FilhoRotulo0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    parentRenderNode.append(_el_1);
    this._J58FilhoRotulo_1_5 = import3.J58FilhoRotulo();
    this._compView_1.create(this._J58FilhoRotulo_1_5);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = import9.interpolateString0(_ctx.nome);
    if (import10.checkBinding(this._expr_0, currVal_0, '{{ nome }}', 'package:corpus_ngdart/src/j58_atributo_interpolado_no_filho.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._J58FilhoRotulo_0_5, 'rotulo', currVal_0);
      }
      this._J58FilhoRotulo_0_5.rotulo = currVal_0 /* REF:package:corpus_ngdart/src/j58_atributo_interpolado_no_filho.html:18:37 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = import9.interpolate2('a ', _ctx.nome, ' b ', _ctx.total, '');
    if (import10.checkBinding(this._expr_1, currVal_1, 'a {{nome}} b {{total}}', 'package:corpus_ngdart/src/j58_atributo_interpolado_no_filho.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._J58FilhoRotulo_1_5, 'rotulo', currVal_1);
      }
      this._J58FilhoRotulo_1_5.rotulo = currVal_1 /* REF:package:corpus_ngdart/src/j58_atributo_interpolado_no_filho.html:76:107 */;
      this._expr_1 = currVal_1;
    }
    final currVal_2 = _ctx.total;
    if (import10.checkBinding(this._expr_2, currVal_2, '{{total}}', 'package:corpus_ngdart/src/j58_atributo_interpolado_no_filho.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._J58FilhoRotulo_1_5, 'titulo', import9.interpolate0(currVal_2));
      }
      this._J58FilhoRotulo_1_5.titulo = import9.interpolate0(currVal_2) /* REF:package:corpus_ngdart/src/j58_atributo_interpolado_no_filho.html:108:126 */;
      this._expr_2 = currVal_2;
    }
    this._compView_0.detectChanges();
    this._compView_1.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
    this._compView_1.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J58AtributoInterpoladoNoFilho, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J58AtributoInterpoladoNoFilhoNgFactory = ComponentFactory<import1.J58AtributoInterpoladoNoFilho>('j58-atributo-interpolado-no-filho', viewFactory_J58AtributoInterpoladoNoFilhoHost0);
ComponentFactory<import1.J58AtributoInterpoladoNoFilho> get J58AtributoInterpoladoNoFilhoNgFactory {
  return _J58AtributoInterpoladoNoFilhoNgFactory;
}

ComponentFactory<import1.J58AtributoInterpoladoNoFilho> createJ58AtributoInterpoladoNoFilhoFactory() {
  return ComponentFactory('j58-atributo-interpolado-no-filho', viewFactory_J58AtributoInterpoladoNoFilhoHost0);
}

final List<Object> styles$J58AtributoInterpoladoNoFilhoHost = const [];

class _ViewJ58AtributoInterpoladoNoFilhoHost0 extends import13.HostView<import1.J58AtributoInterpoladoNoFilho> {
  @override
  void build() {
    this.componentView = ViewJ58AtributoInterpoladoNoFilho0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J58AtributoInterpoladoNoFilho();
    this.initRootNode(_el_0);
  }
}

import13.HostView<import1.J58AtributoInterpoladoNoFilho> viewFactory_J58AtributoInterpoladoNoFilhoHost0() {
  return _ViewJ58AtributoInterpoladoNoFilhoHost0();
}
