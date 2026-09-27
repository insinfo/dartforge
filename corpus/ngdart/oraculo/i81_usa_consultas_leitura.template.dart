// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i81_usa_consultas_leitura.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i81_usa_consultas_leitura.dart' as import1;
import 'i80_consultas_leitura.template.dart' as import2;
import 'i80_consultas_leitura.dart' as import3;
import 'i68_marca.dart' as import4;
import 'i80_rotulo.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import7;
import 'package:ngdart/src/core/linker/views/view.dart' as import8;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import9;
import 'package:ngdart/src/utilities.dart' as import10;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import11;
import 'package:ngdart/src/devtools.dart' as import12;
import 'package:ngdart/src/runtime/check_binding.dart' as import13;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import15;

final List<Object> styles$I81UsaConsultasLeitura = const [];

class ViewI81UsaConsultasLeitura0 extends import0.ComponentView<import1.I81UsaConsultasLeitura> {
  late final import2.ViewI80ConsultasLeitura0 _compView_0;
  late final import3.I80ConsultasLeitura _I80ConsultasLeitura_0_5;
  late final import4.I68Marca _I68Marca_1_5;
  late final import5.I80Rotulo _I80Rotulo_1_6;
  late final import4.I68Marca _I68Marca_2_5;
  late final import5.I80Rotulo _I80Rotulo_2_6;
  late final import5.I80Rotulo _I80Rotulo_3_5;
  Object? _expr_0;
  late final import6.DivElement _el_1;
  static import7.ComponentStyles? _componentStyles;
  ViewI81UsaConsultasLeitura0(import8.View parentView, int parentIndex) : super(parentView, parentIndex, import9.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import10.unsafeCast(import6.document.createElement('i81-usa-consultas-leitura'));
  }
  static String? get _debugComponentUrl {
    return (import10.isDevMode ? 'asset:corpus_ngdart/lib/src/i81_usa_consultas_leitura.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewI80ConsultasLeitura0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._I80ConsultasLeitura_0_5 = import3.I80ConsultasLeitura();
    final doc = import6.document;
    this._el_1 = import10.unsafeCast(doc.createElement('div'));
    import11.setAttribute(this._el_1, 'i68-marca', '');
    import11.setAttribute(this._el_1, 'i80-rotulo', '');
    this._I68Marca_1_5 = import4.I68Marca();
    this._I80Rotulo_1_6 = import5.I80Rotulo();
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(this._el_1, this._I68Marca_1_5);
      import12.Inspector.instance.registerDirective(this._el_1, this._I80Rotulo_1_6);
    }
    final _el_2 = import10.unsafeCast(doc.createElement('span'));
    import11.setAttribute(_el_2, 'i68-marca', '');
    import11.setAttribute(_el_2, 'i80-rotulo', '');
    this._I68Marca_2_5 = import4.I68Marca();
    this._I80Rotulo_2_6 = import5.I80Rotulo();
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_el_2, this._I68Marca_2_5);
      import12.Inspector.instance.registerDirective(_el_2, this._I80Rotulo_2_6);
    }
    final _el_3 = import10.unsafeCast(doc.createElement('p'));
    import11.setAttribute(_el_3, 'i80-rotulo', '');
    this._I80Rotulo_3_5 = import5.I80Rotulo();
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_el_3, this._I80Rotulo_3_5);
    }
    this._I80ConsultasLeitura_0_5.primeira = this._I68Marca_1_5;
    this._I80ConsultasLeitura_0_5.rotulos = [this._I80Rotulo_1_6, this._I80Rotulo_2_6];
    this._I80ConsultasLeitura_0_5.primeiroElemento = this._el_1;
    this._I80ConsultasLeitura_0_5.todos = [this._I80Rotulo_1_6, this._I80Rotulo_2_6, this._I80Rotulo_3_5];
    this._compView_0.createAndProject(this._I80ConsultasLeitura_0_5, [
      <Object>[this._el_1, _el_2, _el_3]
    ]);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.t;
    if (import13.checkBinding(this._expr_0, currVal_0, 't', 'package:corpus_ngdart/src/i81_usa_consultas_leitura.html')) {
      import11.setProperty(this._el_1, 'title', currVal_0) /* REF:package:corpus_ngdart/src/i81_usa_consultas_leitura.html:49:60 */;
      this._expr_0 = currVal_0;
    }
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import7.ComponentStyles.unscoped(styles$I81UsaConsultasLeitura, _debugComponentUrl));
      if (import10.isDevMode) {
        import7.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I81UsaConsultasLeituraNgFactory = ComponentFactory<import1.I81UsaConsultasLeitura>('i81-usa-consultas-leitura', viewFactory_I81UsaConsultasLeituraHost0);
ComponentFactory<import1.I81UsaConsultasLeitura> get I81UsaConsultasLeituraNgFactory {
  return _I81UsaConsultasLeituraNgFactory;
}

ComponentFactory<import1.I81UsaConsultasLeitura> createI81UsaConsultasLeituraFactory() {
  return ComponentFactory('i81-usa-consultas-leitura', viewFactory_I81UsaConsultasLeituraHost0);
}

final List<Object> styles$I81UsaConsultasLeituraHost = const [];

class _ViewI81UsaConsultasLeituraHost0 extends import15.HostView<import1.I81UsaConsultasLeitura> {
  @override
  void build() {
    this.componentView = ViewI81UsaConsultasLeitura0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I81UsaConsultasLeitura();
    this.initRootNode(_el_0);
  }
}

import15.HostView<import1.I81UsaConsultasLeitura> viewFactory_I81UsaConsultasLeituraHost0() {
  return _ViewI81UsaConsultasLeituraHost0();
}
