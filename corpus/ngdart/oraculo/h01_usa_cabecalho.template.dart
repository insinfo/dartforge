// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'h01_usa_cabecalho.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'h01_usa_cabecalho.dart' as import1;
import 'h01_cabecalho.template.dart' as import2;
import 'h01_cabecalho.dart' as import3;
import 'h01_item.dart' as import4;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import5;
import 'package:ngdart/src/core/linker/views/view.dart' as import6;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import7;
import 'package:ngdart/src/utilities.dart' as import8;
import 'dart:html' as import9;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import10;
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import13;

final List<Object> styles$H01UsaCabecalho = const [];

class ViewH01UsaCabecalho0 extends import0.ComponentView<import1.H01UsaCabecalho> {
  late final import2.ViewH01Cabecalho0 _compView_0;
  late final import3.H01Cabecalho _H01Cabecalho_0_5;
  late final import4.H01ItemDirective _H01ItemDirective_1_5;
  late final import4.H01ItemDirective _H01ItemDirective_2_5;
  late final import4.H01AcoesDirective _H01AcoesDirective_3_5;
  late final import4.H01AcoesDirective _H01AcoesDirective_4_5;
  static import5.ComponentStyles? _componentStyles;
  ViewH01UsaCabecalho0(import6.View parentView, int parentIndex) : super(parentView, parentIndex, import7.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import8.unsafeCast(import9.document.createElement('h01-usa-cabecalho'));
  }
  static String? get _debugComponentUrl {
    return (import8.isDevMode ? 'asset:corpus_ngdart/lib/src/h01_usa_cabecalho.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewH01Cabecalho0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    import10.setAttribute(_el_0, 'titulo', 'Olá');
    this._H01Cabecalho_0_5 = import3.H01Cabecalho();
    final doc = import9.document;
    final _el_1 = import8.unsafeCast(doc.createElement('h01-it'));
    import10.setAttribute(_el_1, 'label', 'Um');
    this._H01ItemDirective_1_5 = import4.H01ItemDirective();
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_el_1, this._H01ItemDirective_1_5);
    }
    final _el_2 = import8.unsafeCast(doc.createElement('h01-it'));
    import10.setAttribute(_el_2, 'label', 'Dois');
    this._H01ItemDirective_2_5 = import4.H01ItemDirective();
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_el_2, this._H01ItemDirective_2_5);
    }
    final _el_3 = import8.unsafeCast(doc.createElement('div'));
    import10.setAttribute(_el_3, 'acoes', '');
    this._H01AcoesDirective_3_5 = import4.H01AcoesDirective();
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_el_3, this._H01AcoesDirective_3_5);
    }
    final _el_4 = import10.appendSpan(doc, _el_3);
    import10.setAttribute(_el_4, 'acoes', '');
    this._H01AcoesDirective_4_5 = import4.H01AcoesDirective();
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_el_4, this._H01AcoesDirective_4_5);
    }
    final _text_5 = import10.appendText(_el_4, 'dentro');
    this._H01Cabecalho_0_5.itens = [this._H01ItemDirective_1_5, this._H01ItemDirective_2_5];
    this._H01Cabecalho_0_5.acoes = [this._H01AcoesDirective_3_5];
    this._compView_0.createAndProject(this._H01Cabecalho_0_5, [
      <Object>[_el_3]
    ]);
    final _el_6 = import10.appendElement<import9.HtmlElement>(doc, parentRenderNode, 'p');
    final _text_7 = import10.appendText(_el_6, 'depois');
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    if (firstCheck) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._H01Cabecalho_0_5, 'titulo', 'Olá');
      }
      this._H01Cabecalho_0_5.titulo = 'Olá' /* REF:package:corpus_ngdart/src/h01_usa_cabecalho.html:9:21 */;
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._H01Cabecalho_0_5, 'mostrar', true);
      }
      this._H01Cabecalho_0_5.mostrar = true /* REF:package:corpus_ngdart/src/h01_usa_cabecalho.html:22:38 */;
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._H01ItemDirective_1_5, 'label', 'Um');
      }
      this._H01ItemDirective_1_5.label = 'Um' /* REF:package:corpus_ngdart/src/h01_usa_cabecalho.html:52:62 */;
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._H01ItemDirective_2_5, 'label', 'Dois');
      }
      this._H01ItemDirective_2_5.label = 'Dois' /* REF:package:corpus_ngdart/src/h01_usa_cabecalho.html:85:97 */;
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._H01ItemDirective_2_5, 'ativo', true);
      }
      this._H01ItemDirective_2_5.ativo = true /* REF:package:corpus_ngdart/src/h01_usa_cabecalho.html:98:112 */;
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
      _componentStyles = (styles = import5.ComponentStyles.unscoped(styles$H01UsaCabecalho, _debugComponentUrl));
      if (import8.isDevMode) {
        import5.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _H01UsaCabecalhoNgFactory = ComponentFactory<import1.H01UsaCabecalho>('h01-usa-cabecalho', viewFactory_H01UsaCabecalhoHost0);
ComponentFactory<import1.H01UsaCabecalho> get H01UsaCabecalhoNgFactory {
  return _H01UsaCabecalhoNgFactory;
}

ComponentFactory<import1.H01UsaCabecalho> createH01UsaCabecalhoFactory() {
  return ComponentFactory('h01-usa-cabecalho', viewFactory_H01UsaCabecalhoHost0);
}

final List<Object> styles$H01UsaCabecalhoHost = const [];

class _ViewH01UsaCabecalhoHost0 extends import13.HostView<import1.H01UsaCabecalho> {
  @override
  void build() {
    this.componentView = ViewH01UsaCabecalho0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.H01UsaCabecalho();
    this.initRootNode(_el_0);
  }
}

import13.HostView<import1.H01UsaCabecalho> viewFactory_H01UsaCabecalhoHost0() {
  return _ViewH01UsaCabecalhoHost0();
}
