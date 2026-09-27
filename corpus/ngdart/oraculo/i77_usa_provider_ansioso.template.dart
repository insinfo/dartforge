// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i77_usa_provider_ansioso.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i77_usa_provider_ansioso.dart' as import1;
import 'i66_provider_dependencias.dart' as import2;
import 'i66_provider_dependencias.template.dart' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$I77UsaProviderAnsioso = const [];

class ViewI77UsaProviderAnsioso0 extends import0.ComponentView<import1.I77UsaProviderAnsioso> {
  late import2.I66Cache _I66Cache_0_8 = import2.I66Cache(this._I66Repo_0_6);
  late import2.I66Solto _I66Solto_0_9 = import2.I66Solto();
  late final import3.ViewI66ProviderDependencias0 _compView_0;
  late final import2.I66Api _I66Api_0_5;
  late final import2.I66Repo _I66Repo_0_6;
  late final import2.I66ProviderDependencias _I66ProviderDependencias_0_7;
  static import4.ComponentStyles? _componentStyles;
  ViewI77UsaProviderAnsioso0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('i77-usa-provider-ansioso'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i77_usa_provider_ansioso.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import3.ViewI66ProviderDependencias0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._I66Api_0_5 = import2.I66Api();
    this._I66Repo_0_6 = import2.I66Repo(this._I66Api_0_5);
    this._I66ProviderDependencias_0_7 = import2.I66ProviderDependencias(this._I66Repo_0_6);
    this._compView_0.create(this._I66ProviderDependencias_0_7);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((0 == nodeIndex)) {
      if (identical(token, import2.I66Api)) {
        return this._I66Api_0_5;
      }
      if (identical(token, import2.I66Repo)) {
        return this._I66Repo_0_6;
      }
      if (identical(token, import2.I66Cache)) {
        return this._I66Cache_0_8;
      }
      if (identical(token, import2.I66Solto)) {
        return this._I66Solto_0_9;
      }
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
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
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I77UsaProviderAnsioso, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I77UsaProviderAnsiosoNgFactory = ComponentFactory<import1.I77UsaProviderAnsioso>('i77-usa-provider-ansioso', viewFactory_I77UsaProviderAnsiosoHost0);
ComponentFactory<import1.I77UsaProviderAnsioso> get I77UsaProviderAnsiosoNgFactory {
  return _I77UsaProviderAnsiosoNgFactory;
}

ComponentFactory<import1.I77UsaProviderAnsioso> createI77UsaProviderAnsiosoFactory() {
  return ComponentFactory('i77-usa-provider-ansioso', viewFactory_I77UsaProviderAnsiosoHost0);
}

final List<Object> styles$I77UsaProviderAnsiosoHost = const [];

class _ViewI77UsaProviderAnsiosoHost0 extends import10.HostView<import1.I77UsaProviderAnsioso> {
  @override
  void build() {
    this.componentView = ViewI77UsaProviderAnsioso0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I77UsaProviderAnsioso();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.I77UsaProviderAnsioso> viewFactory_I77UsaProviderAnsiosoHost0() {
  return _ViewI77UsaProviderAnsiosoHost0();
}
