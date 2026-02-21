use silex_css::prelude::*;

fn main() {
    let (color_sig, _) = signal(hex("#fff"));
    // 错误：height 的 setter 期望接收维度相关的信号（Px/Rem等），
    // 传入 Signal<Hex> 应当报错
    let _ = Style::new().height(color_sig);
}
