use silex_core::prelude::*;
use silex_css::prelude::*;

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (color_sig, _) = scope.signal(hex("#fff"));
        // 错误：border_top_width 的 setter 期望接收维度相关的信号（Px/Rem等），
        // 传入 Signal<Hex> 应当报错
        let _ = Style::new().border_top_width(color_sig);
    });
}
