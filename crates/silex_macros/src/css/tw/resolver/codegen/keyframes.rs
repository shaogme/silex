// 自动生成的动画 Keyframes 规则表（供 silex_macros 使用）
// 由 silex_codegen 自动生成，切勿手写修改！

pub struct KeyframeStep {
    pub selector: &'static str,
    pub declarations: &'static [(&'static str, &'static str)],
}

pub struct KeyframeMeta {
    pub name: &'static str,
    pub steps: &'static [KeyframeStep],
}

#[rustfmt::skip]
pub static KEYFRAME_TABLE: &[KeyframeMeta] = &[
    KeyframeMeta {
        name: "bounce",
        steps: &[
            KeyframeStep { selector: "0%, 100%", declarations: &[("transform", "translateY(-25%)"), ("animation-timing-function", "cubic-bezier(0.8, 0, 1, 1)")] },
            KeyframeStep { selector: "50%", declarations: &[("transform", "none"), ("animation-timing-function", "cubic-bezier(0, 0, 0.2, 1)")] },
        ],
    },
    KeyframeMeta {
        name: "ping",
        steps: &[
            KeyframeStep { selector: "75%, 100%", declarations: &[("transform", "scale(2)"), ("opacity", "0")] },
        ],
    },
    KeyframeMeta {
        name: "pulse",
        steps: &[
            KeyframeStep { selector: "50%", declarations: &[("opacity", "0.5")] },
        ],
    },
    KeyframeMeta {
        name: "spin",
        steps: &[
            KeyframeStep { selector: "from", declarations: &[("transform", "rotate(0deg)")] },
            KeyframeStep { selector: "to", declarations: &[("transform", "rotate(360deg)")] },
        ],
    },
];

/// 根据动画 keyframe 名称二分查找关键帧元数据配置
pub fn lookup_keyframe_meta(name: &str) -> Option<&'static KeyframeMeta> {
    let idx = KEYFRAME_TABLE.binary_search_by_key(&name, |k| k.name).ok()?;
    Some(&KEYFRAME_TABLE[idx])
}
