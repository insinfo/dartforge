// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i88_encapsulation_none_estilo.dart';
import 'package:corpus_ngdart/src/i88_encapsulation_none_estilo.css.dart' as import0;
import 'package:ngdart/src/core/linker/views/component_view.dart' as import1;
import 'i88_encapsulation_none_estilo.dart' as import2;
import 'package:ngdart/src/runtime/text_binding.dart' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;

final List<Object> styles$I88EncapsulationNoneEstilo = [import0.styles];

class ViewI88EncapsulationNoneEstilo0 extends import1.ComponentView<import2.I88EncapsulationNoneEstilo> {
  final import3.TextBinding _textBinding_2 = import3.TextBinding();
  static import4.ComponentStyles? _componentStyles;
  ViewI88EncapsulationNoneEstilo0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('i88-encapsulation-none-estilo'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i88_encapsulation_none_estilo.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import8.document;
    final _el_0 = import9.appendDiv(doc, parentRenderNode);
    this.updateChildClass(_el_0, 'a');
    final _el_1 = import9.appendSpan(doc, _el_0);
    _el_1.append(this._textBinding_2.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_2.updateTextWithPrimitive(_ctx.n) /* REF:package:corpus_ngdart/src/i88_encapsulation_none_estilo.html:21:26 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I88EncapsulationNoneEstilo, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I88EncapsulationNoneEstiloNgFactory = ComponentFactory<import2.I88EncapsulationNoneEstilo>('i88-encapsulation-none-estilo', viewFactory_I88EncapsulationNoneEstiloHost0);
ComponentFactory<import2.I88EncapsulationNoneEstilo> get I88EncapsulationNoneEstiloNgFactory {
  return _I88EncapsulationNoneEstiloNgFactory;
}

ComponentFactory<import2.I88EncapsulationNoneEstilo> createI88EncapsulationNoneEstiloFactory() {
  return ComponentFactory('i88-encapsulation-none-estilo', viewFactory_I88EncapsulationNoneEstiloHost0);
}

final List<Object> styles$I88EncapsulationNoneEstiloHost = const [];

class _ViewI88EncapsulationNoneEstiloHost0 extends import11.HostView<import2.I88EncapsulationNoneEstilo> {
  @override
  void build() {
    this.componentView = ViewI88EncapsulationNoneEstilo0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import2.I88EncapsulationNoneEstilo();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import2.I88EncapsulationNoneEstilo> viewFactory_I88EncapsulationNoneEstiloHost0() {
  return _ViewI88EncapsulationNoneEstiloHost0();
}
