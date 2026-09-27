// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i64_provider_multi.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i64_provider_multi.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'dart:core';
import 'package:ngdart/src/meta/di_tokens.dart' as import11;

final List<Object> styles$I64ProviderMulti = const [];

class ViewI64ProviderMulti0 extends import0.ComponentView<import1.I64ProviderMulti> {
  static import2.ComponentStyles? _componentStyles;
  ViewI64ProviderMulti0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i64-provider-multi'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i64_provider_multi.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'p');
    final _text_1 = import7.appendText(_el_0, 'x');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I64ProviderMulti, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I64ProviderMultiNgFactory = ComponentFactory<import1.I64ProviderMulti>('i64-provider-multi', viewFactory_I64ProviderMultiHost0);
ComponentFactory<import1.I64ProviderMulti> get I64ProviderMultiNgFactory {
  return _I64ProviderMultiNgFactory;
}

ComponentFactory<import1.I64ProviderMulti> createI64ProviderMultiFactory() {
  return ComponentFactory('i64-provider-multi', viewFactory_I64ProviderMultiHost0);
}

final List<Object> styles$I64ProviderMultiHost = const [];

class _ViewI64ProviderMultiHost0 extends import9.HostView<import1.I64ProviderMulti> {
  late List<String> _i64Nomes_0_6 = ['a', 'b'];
  late List<Object> _i64Validadores_0_7 = [import1.I64Validador(), this.component];
  @override
  void build() {
    this.componentView = ViewI64ProviderMulti0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I64ProviderMulti();
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((0 == nodeIndex)) {
      if (identical(token, const import11.MultiToken<String>('i64Nomes'))) {
        return this._i64Nomes_0_6;
      }
      if (identical(token, const import11.MultiToken<Object>('i64Validadores'))) {
        return this._i64Validadores_0_7;
      }
    }
    return notFoundResult;
  }
}

import9.HostView<import1.I64ProviderMulti> viewFactory_I64ProviderMultiHost0() {
  return _ViewI64ProviderMultiHost0();
}
