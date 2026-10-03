# THIS FILE IS AUTO-GENERATED. DO NOT MODIFY!!

# Copyright 2020-2023 Tauri Programme within The Commons Conservancy
# SPDX-License-Identifier: Apache-2.0
# SPDX-License-Identifier: MIT

-keep class app.velocity.harness.* {
  native <methods>;
}

-keep class app.velocity.harness.WryActivity {
  public <init>(...);

  void setWebView(app.velocity.harness.RustWebView);
  java.lang.Class getAppClass(...);
  int getId();
  java.lang.String getVersion();
  int startActivity(...);
}

-keep class app.velocity.harness.Ipc {
  public <init>(...);

  @android.webkit.JavascriptInterface public <methods>;
}

-keep class app.velocity.harness.RustWebView {
  public <init>(...);

  void loadUrlMainThread(...);
  void loadHTMLMainThread(...);
  void evalScript(...);
}

-keep class app.velocity.harness.RustWebChromeClient,app.velocity.harness.RustWebViewClient {
  public <init>(...);
}
