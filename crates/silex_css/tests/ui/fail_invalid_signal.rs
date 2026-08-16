use silex_core::prelude::*;
use silex_css::prelude::*;

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        let (color_sig, _) = owner
            .signal(hex("#fff"))
            .expect("signal should initialize");
        // 错误：border_top_width 的 setter 期望接收维度相关的信号（Px/Rem等），
        // 传入 Signal<'scope, Hex> 应当报错
        let error_handler = owner
            .error_handler(|_| {})
            .expect("handler should register");
        let _ = Style::new(SilexContext::new(owner, error_handler.view())).border_top_width(color_sig);
    });
}
