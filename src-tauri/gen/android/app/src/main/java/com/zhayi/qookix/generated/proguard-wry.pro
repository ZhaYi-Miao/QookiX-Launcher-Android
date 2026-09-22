# THIS FILE IS AUTO-GENERATED. DO NOT MODIFY!!

# Copyright 2020-2023 Tauri Programme within The Commons Conservancy
# SPDX-License-Identifier: Apache-2.0
# SPDX-License-Identifier: MIT

-keep class com.zhayi.qookix.* {
  native <methods>;
}

-keep class com.zhayi.qookix.WryActivity {
  public <init>(...);

  void setWebView(com.zhayi.qookix.RustWebView);
  java.lang.Class getAppClass(...);
  int getId();
  java.lang.String getVersion();
  int startActivity(...);
}

-keep class com.zhayi.qookix.Ipc {
  public <init>(...);

  @android.webkit.JavascriptInterface public <methods>;
}

-keep class com.zhayi.qookix.RustWebView {
  public <init>(...);

  void loadUrlMainThread(...);
  void loadHTMLMainThread(...);
  void evalScript(...);
}

-keep class com.zhayi.qookix.RustWebChromeClient,com.zhayi.qookix.RustWebViewClient {
  public <init>(...);
}
