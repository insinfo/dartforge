// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j90_leitura_de_provedor.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j90_leitura_de_provedor.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/devtools.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$J90LeituraDeProvedor = const [];

class ViewJ90LeituraDeProvedor0 extends import0.ComponentView<import1.J90LeituraDeProvedor> {
  late import1.J90Outro _J90Outro_1_7 = import1.J90Outro(this._J90Servico_1_6);
  late import1.J90Outro _J90Outro_2_7 = import1.J90Outro(this._J90Servico_2_6);
  late final import1.J90Marca _J90Marca_0_5;
  late final import1.J90Servico _J90Servico_0_6;
  late final import1.J90Outro _J90Outro_0_7;
  late final import1.J90Marca _J90Marca_1_5;
  late final import1.J90Servico _J90Servico_1_6;
  late final import1.J90Marca _J90Marca_2_5;
  late final import1.J90Servico _J90Servico_2_6;
  static import2.ComponentStyles? _componentStyles;
  ViewJ90LeituraDeProvedor0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j90-leitura-de-provedor'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j90_leitura_de_provedor.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendSpan(doc, parentRenderNode);
    import7.setAttribute(_el_0, 'j90-marca', '');
    this._J90Marca_0_5 = import1.J90Marca();
    this._J90Servico_0_6 = import1.J90Servico();
    this._J90Outro_0_7 = import1.J90Outro(this._J90Servico_0_6);
    if (import8.isDevToolsEnabled) {
      import8.Inspector.instance.registerDirective(_el_0, this._J90Marca_0_5);
    }
    final _el_1 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'b');
    import7.setAttribute(_el_1, 'j90-marca', '');
    this._J90Marca_1_5 = import1.J90Marca();
    this._J90Servico_1_6 = import1.J90Servico();
    if (import8.isDevToolsEnabled) {
      import8.Inspector.instance.registerDirective(_el_1, this._J90Marca_1_5);
    }
    final _el_2 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'i');
    import7.setAttribute(_el_2, 'j90-marca', '');
    this._J90Marca_2_5 = import1.J90Marca();
    this._J90Servico_2_6 = import1.J90Servico();
    if (import8.isDevToolsEnabled) {
      import8.Inspector.instance.registerDirective(_el_2, this._J90Marca_2_5);
    }
    _ctx.outro = this._J90Outro_0_7;
    _ctx.servicos = [this._J90Servico_1_6, this._J90Servico_2_6];
    _ctx.primeira = this._J90Marca_1_5;
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((0 == nodeIndex)) {
      if (identical(token, import1.J90Servico)) {
        return this._J90Servico_0_6;
      }
      if (identical(token, import1.J90Outro)) {
        return this._J90Outro_0_7;
      }
    }
    if ((1 == nodeIndex)) {
      if (identical(token, import1.J90Servico)) {
        return this._J90Servico_1_6;
      }
      if (identical(token, import1.J90Outro)) {
        return this._J90Outro_1_7;
      }
    }
    if ((2 == nodeIndex)) {
      if (identical(token, import1.J90Servico)) {
        return this._J90Servico_2_6;
      }
      if (identical(token, import1.J90Outro)) {
        return this._J90Outro_2_7;
      }
    }
    return notFoundResult;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J90LeituraDeProvedor, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J90LeituraDeProvedorNgFactory = ComponentFactory<import1.J90LeituraDeProvedor>('j90-leitura-de-provedor', viewFactory_J90LeituraDeProvedorHost0);
ComponentFactory<import1.J90LeituraDeProvedor> get J90LeituraDeProvedorNgFactory {
  return _J90LeituraDeProvedorNgFactory;
}

ComponentFactory<import1.J90LeituraDeProvedor> createJ90LeituraDeProvedorFactory() {
  return ComponentFactory('j90-leitura-de-provedor', viewFactory_J90LeituraDeProvedorHost0);
}

final List<Object> styles$J90LeituraDeProvedorHost = const [];

class _ViewJ90LeituraDeProvedorHost0 extends import10.HostView<import1.J90LeituraDeProvedor> {
  @override
  void build() {
    this.componentView = ViewJ90LeituraDeProvedor0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J90LeituraDeProvedor();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.J90LeituraDeProvedor> viewFactory_J90LeituraDeProvedorHost0() {
  return _ViewJ90LeituraDeProvedorHost0();
}
