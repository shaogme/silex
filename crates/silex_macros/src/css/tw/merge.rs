//! 跨段层叠顺序的消解（报告 §3.5）
//!
//! `tw!` 的条件分支、`tw_variants!` 的各个变体槽位，都是**各自编译出一个类名**、
//! 运行时用 `cx!` 拼在一起。同为 `@layer utilities` 中的单类选择器，特异性相同，
//! 胜负因此取决于样式表的注入顺序，而注入顺序 = 首次渲染顺序。
//! `tw!("p-4", (big, "p-8"))` 里谁覆盖谁于是取决于哪个组件先挂载——非确定。
//!
//! 解法不是给类加层级，而是把**真正会互相覆盖**的段合并进同一个类，
//! 交给既有的编译期 tw-merge（`deduplicate_utility_rules`）裁决——
//! 产出的语义与把这些词条写在一个字符串里完全一致，包括 `md:` 之类变体
//! 相对于无修饰符词条的优先级（那是按修饰符权重排序决定的，不是按书写顺序）。
//!
//! 关键在于**只对有冲突的段**做笛卡尔展开：互不覆盖的段留在各自的类里，
//! 类的数量与今天一样。实测仓库里 6 个 `tw_variants!` 组件一个簇都不会形成。

use crate::css::tw::{
    ast::UtilityRule, resolver::codegen::property_id::CssPropertyId,
};

/// 一个段写到的属性覆盖面：按 `bitmask` 的组号聚合出的掩码
///
/// 复用 `CssPropertyId::bitmask()` 这套简写/长写覆盖关系的建模——
/// `inset-x-0` 与 `left-4` 属于同一组且掩码相交，正是"会互相覆盖"的定义。
#[derive(Default)]
pub(crate) struct WriteSet {
    /// 按组号升序，同组的掩码已经并起来
    groups: Vec<(u16, u64)>,
}

impl WriteSet {
    pub(crate) fn of(rules: &[UtilityRule]) -> Self {
        let mut groups: Vec<(u16, u64)> = Vec::new();
        let mut add_mask = |group_id: u16, mask: u64| {
            match groups.iter_mut().find(|(g, _)| *g == group_id) {
                Some((_, m)) => *m |= mask,
                None => groups.push((group_id, mask)),
            }
        };

        for rule in rules {
            let bm = rule.css_property.bitmask();
            add_mask(bm.group_id, bm.mask);

            match rule.css_property {
                CssPropertyId::VarTwSpaceXReverse => {
                    let l_bm = CssPropertyId::MarginLeft.bitmask();
                    let r_bm = CssPropertyId::MarginRight.bitmask();
                    add_mask(l_bm.group_id, l_bm.mask | r_bm.mask);
                }
                CssPropertyId::VarTwSpaceYReverse => {
                    let t_bm = CssPropertyId::MarginTop.bitmask();
                    let b_bm = CssPropertyId::MarginBottom.bitmask();
                    add_mask(t_bm.group_id, t_bm.mask | b_bm.mask);
                }
                CssPropertyId::VarTwDivideXReverse => {
                    let l_bm = CssPropertyId::BorderLeftWidth.bitmask();
                    let r_bm = CssPropertyId::BorderRightWidth.bitmask();
                    add_mask(l_bm.group_id, l_bm.mask | r_bm.mask);
                }
                CssPropertyId::VarTwDivideYReverse => {
                    let t_bm = CssPropertyId::BorderTopWidth.bitmask();
                    let b_bm = CssPropertyId::BorderBottomWidth.bitmask();
                    add_mask(t_bm.group_id, t_bm.mask | b_bm.mask);
                }
                _ => {}
            }
        }
        groups.sort_unstable_by_key(|(g, _)| *g);
        Self { groups }
    }

    pub(crate) fn merge_from(&mut self, other: &Self) {
        for (g, m) in &other.groups {
            match self.groups.iter_mut().find(|(sg, _)| sg == g) {
                Some((_, mask)) => *mask |= *m,
                None => self.groups.push((*g, *m)),
            }
        }
        self.groups.sort_unstable_by_key(|(g, _)| *g);
    }

    /// 两个段是否会写到同一个属性槽位
    ///
    /// **刻意不比较修饰符**。`md:p-4` 与 `p-8` 分处两个类时同样是同特异性打架
    /// （媒体查询不贡献特异性），照样由注入顺序决定；而合并进一个类之后，
    /// 修饰符组的排序会给出 Tailwind 的正确答案。至于 `hover:p-4` 与 `p-8`
    /// 这种本来就由特异性分出胜负的组合，合并后结果不变——多合并无害，漏合并才有害。
    pub(crate) fn conflicts_with(&self, other: &Self) -> bool {
        let (mut i, mut j) = (0, 0);
        while i < self.groups.len() && j < other.groups.len() {
            let (ga, ma) = self.groups[i];
            let (gb, mb) = other.groups[j];
            match ga.cmp(&gb) {
                std::cmp::Ordering::Less => i += 1,
                std::cmp::Ordering::Greater => j += 1,
                std::cmp::Ordering::Equal => {
                    if ma & mb != 0 {
                        return true;
                    }
                    i += 1;
                    j += 1;
                }
            }
        }
        false
    }
}

/// 把互相冲突的段并进同一簇。
///
/// 返回的每个簇内部按下标升序（层叠顺序 = 源码顺序），簇之间按各自的最小下标升序，
/// 于是产出的类名顺序与今天一致，`insta` 式的文本比对不会因为重排而全红。
pub(crate) fn cluster(sets: &[WriteSet]) -> Vec<Vec<usize>> {
    let n = sets.len();
    let mut parent: Vec<usize> = (0..n).collect();

    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }

    for a in 0..n {
        for b in (a + 1)..n {
            if sets[a].conflicts_with(&sets[b]) {
                let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
                if ra != rb {
                    parent[ra] = rb;
                }
            }
        }
    }

    // 以「簇的最小下标」为序输出
    let mut clusters: Vec<Vec<usize>> = Vec::new();
    let mut root_to_slot: Vec<(usize, usize)> = Vec::new(); // (root, clusters 中的位置)
    for i in 0..n {
        let root = find(&mut parent, i);
        match root_to_slot.iter().find(|(r, _)| *r == root) {
            Some((_, at)) => clusters[*at].push(i),
            None => {
                root_to_slot.push((root, clusters.len()));
                clusters.push(vec![i]);
            }
        }
    }
    clusters
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css::tw::parser::{TokenAnchor, parse_class_list};
    use proc_macro2::Span;

    fn rules(src: &str) -> Vec<UtilityRule> {
        let mut extra = Vec::new();
        parse_class_list(&TokenAnchor::whole(src, Span::call_site()), &mut extra)
            .unwrap_or_else(|e| panic!("{src}: {e}"))
    }

    fn set(src: &str) -> WriteSet {
        WriteSet::of(&rules(src))
    }

    #[test]
    fn same_property_conflicts_across_shorthand_and_longhand() {
        assert!(set("p-4").conflicts_with(&set("p-8")));
        assert!(set("p-4").conflicts_with(&set("px-8")));
        assert!(set("inset-x-0").conflicts_with(&set("left-4")));
        // 修饰符不参与判定：分处两个类时 `md:p-4` 与 `p-8` 同样由注入顺序决定
        assert!(set("md:p-4").conflicts_with(&set("p-8")));
    }

    #[test]
    fn unrelated_properties_do_not_conflict() {
        assert!(!set("p-4").conflicts_with(&set("text-red-500")));
        assert!(!set("flex gap-2").conflicts_with(&set("rounded-lg")));
        // Button 的 variant 与 size 两槽：一个写颜色/边框，一个写尺寸——不该被合并
        assert!(
            !set("bg-primary text-primary-foreground hover:bg-primary/90")
                .conflicts_with(&set("h-9 px-4 py-2"))
        );
    }

    #[test]
    fn composable_properties_conflict_with_themselves() {
        // transform / filter 分处两个类时是整条覆盖，不会叠加
        assert!(set("translate-x-2").conflicts_with(&set("translate-y-2")));
        assert!(set("blur-sm").conflicts_with(&set("brightness-50")));
        assert!(!set("blur-sm").conflicts_with(&set("translate-x-2")));
    }

    #[test]
    fn clustering_keeps_independent_segments_apart() {
        let sets = [set("p-4"), set("text-red-500"), set("p-8")];
        let clusters = cluster(&sets);
        assert_eq!(clusters, vec![vec![0, 2], vec![1]]);
    }

    #[test]
    fn clustering_is_transitive_through_a_shared_segment() {
        // A 与 B 不直接冲突，但都与 C 冲突 ⇒ 三者必须同簇
        let sets = [set("px-4"), set("py-4"), set("p-8")];
        let clusters = cluster(&sets);
        assert_eq!(clusters, vec![vec![0, 1, 2]]);
    }
}
