// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j55_entidades.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j55_entidades.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/runtime/interpolate.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;

final List<Object> styles$J55Entidades = const [];

class ViewJ55Entidades0 extends import0.ComponentView<import1.J55Entidades> {
  final import2.TextBinding _textBinding_5 = import2.TextBinding();
  static import3.ComponentStyles? _componentStyles;
  ViewJ55Entidades0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j55-entidades'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j55_entidades.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendElement<import7.AnchorElement>(doc, parentRenderNode, 'a');
    final _text_1 = import8.appendText(_el_0, '← voltar →');
    final _el_2 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'p');
    final _text_3 = import8.appendText(_el_2, '© © © ©A&#12345; inexistente &amp; <b>  x y');
    final _el_4 = import8.appendSpan(doc, parentRenderNode);
    import8.setAttribute(_el_4, 'title', '&larr; &amp;');
    _el_4.append(this._textBinding_5.element);
    final _text_6 = import8.appendText(_el_4, ' … Ω——');
    final _text_7 = import8.appendText(parentRenderNode, '\n');
    final _el_8 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'i');
    final _text_9 = import8.appendText(_el_8, '&#9;tab');
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_5.updateText(import9.interpolateString0(_ctx.nome)) /* REF:package:corpus_ngdart/src/j55_entidades.html:152:160 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J55Entidades, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J55EntidadesNgFactory = ComponentFactory<import1.J55Entidades>('j55-entidades', viewFactory_J55EntidadesHost0);
ComponentFactory<import1.J55Entidades> get J55EntidadesNgFactory {
  return _J55EntidadesNgFactory;
}

ComponentFactory<import1.J55Entidades> createJ55EntidadesFactory() {
  return ComponentFactory('j55-entidades', viewFactory_J55EntidadesHost0);
}

final List<Object> styles$J55EntidadesHost = const [];

class _ViewJ55EntidadesHost0 extends import11.HostView<import1.J55Entidades> {
  @override
  void build() {
    this.componentView = ViewJ55Entidades0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J55Entidades();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.J55Entidades> viewFactory_J55EntidadesHost0() {
  return _ViewJ55EntidadesHost0();
}
