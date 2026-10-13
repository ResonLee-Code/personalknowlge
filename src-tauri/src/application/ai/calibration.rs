//! 校准度量与拟合（D4 收口）。
//!
//! 为什么需要：决策模型的价值全在"**它说的概率能不能信**"。公开研究用的口径是
//! **期望校准误差（ECE）≤ 0.05**。这里把它做成可复算的三件事：
//! 1. **ECE / Brier / 分箱表**——把"说 80% 的时候真的对了多少"摆出来；
//! 2. **等渗回归（PAVA）**——把 stated P 拉成单调的"经验正例率"映射；
//! 3. **折外评估**——对半切：一半拟合、另一半算 ECE，两个方向取平均。
//!    只用同一批数据既拟合又评估是循环论证，会得出虚假的好数字；折外才是能拿出去说的数。

/// 一个校准分箱（10 等宽箱之一）。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationBin {
    /// 展示用的区间（例如 `0.7–0.8`）
    pub range: String,
    pub count: usize,
    /// 这一箱里模型平均说了多大的概率
    pub mean_stated: f32,
    /// 这一箱里真实为正的比例（样本太少时参考意义有限）
    pub actual_share: f32,
}

/// 校准评估结果。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Calibration {
    pub samples: usize,
    /// 期望校准误差（越小越好；公开研究用的线是 0.05）
    pub ece: f32,
    /// Brier 分数（越小越好）
    pub brier: f32,
    pub bins: Vec<CalibrationBin>,
}

/// 一条样本：模型说的 P(是) 与真实答案。
pub type Sample = (f32, bool);

const BINS: usize = 10;

/// ECE / Brier / 分箱表。
pub fn evaluate(samples: &[Sample]) -> Calibration {
    let mut buckets: Vec<Vec<&Sample>> = vec![Vec::new(); BINS];
    for sample in samples {
        buckets[bin_index(sample.0)].push(sample);
    }
    let total = samples.len().max(1) as f32;
    let mut ece = 0.0f32;
    let mut table = Vec::new();
    for (index, bucket) in buckets.iter().enumerate() {
        if bucket.is_empty() {
            continue;
        }
        let count = bucket.len() as f32;
        let mean_stated = bucket.iter().map(|(p, _)| *p).sum::<f32>() / count;
        let actual = bucket.iter().filter(|(_, truth)| *truth).count() as f32 / count;
        ece += (count / total) * (mean_stated - actual).abs();
        table.push(CalibrationBin {
            range: format!(
                "{:.1}–{:.1}",
                index as f32 / BINS as f32,
                (index + 1) as f32 / BINS as f32
            ),
            count: bucket.len(),
            mean_stated,
            actual_share: actual,
        });
    }
    let brier = if samples.is_empty() {
        0.0
    } else {
        samples
            .iter()
            .map(|(p, truth)| {
                let target = if *truth { 1.0 } else { 0.0 };
                (p - target) * (p - target)
            })
            .sum::<f32>()
            / samples.len() as f32
    };
    Calibration { samples: samples.len(), ece, brier, bins: table }
}

fn bin_index(p: f32) -> usize {
    let clamped = p.clamp(0.0, 1.0);
    ((clamped * BINS as f32) as usize).min(BINS - 1)
}

/// 等渗回归（Pool Adjacent Violators）：拟合出**单调不减**的校准映射。
///
/// 返回一组断点 `(stated_p, 经验正例率)`。
pub fn fit_isotonic(samples: &[Sample]) -> Vec<(f32, f32)> {
    if samples.is_empty() {
        return Vec::new();
    }
    let mut sorted: Vec<Sample> = samples.to_vec();
    sorted.sort_by(|left, right| left.0.partial_cmp(&right.0).unwrap_or(std::cmp::Ordering::Equal));

    struct Block {
        sum_x: f32,
        sum_y: f32,
        count: f32,
    }
    impl Block {
        fn mean_y(&self) -> f32 {
            self.sum_y / self.count
        }
    }

    let mut blocks: Vec<Block> = Vec::new();
    for (p, truth) in sorted {
        blocks.push(Block { sum_x: p, sum_y: if truth { 1.0 } else { 0.0 }, count: 1.0 });
        while blocks.len() >= 2 {
            let last = blocks.len() - 1;
            if blocks[last - 1].mean_y() <= blocks[last].mean_y() {
                break;
            }
            let right = blocks.pop().expect("刚检查过长度");
            let left = blocks.pop().expect("刚检查过长度");
            blocks.push(Block {
                sum_x: left.sum_x + right.sum_x,
                sum_y: left.sum_y + right.sum_y,
                count: left.count + right.count,
            });
        }
    }
    blocks
        .into_iter()
        .map(|block| (block.sum_x / block.count, block.sum_y / block.count))
        .collect()
}

/// 应用校准映射：取 `<= p` 的最后一个断点（没有则用第一个）。
pub fn apply_mapping(mapping: &[(f32, f32)], p: f32) -> f32 {
    if mapping.is_empty() {
        return p;
    }
    let mut value = mapping[0].1;
    for (x, y) in mapping {
        if *x <= p {
            value = *y;
        } else {
            break;
        }
    }
    value.clamp(0.0, 1.0)
}

/// **折外**校准误差：对半切，一半拟合、另一半评估，两个方向取平均。
pub fn out_of_sample_ece(samples: &[Sample]) -> Option<f32> {
    if samples.len() < 6 {
        return None;
    }
    let mut left: Vec<Sample> = Vec::new();
    let mut right: Vec<Sample> = Vec::new();
    for (index, sample) in samples.iter().enumerate() {
        if index % 2 == 0 {
            left.push(*sample);
        } else {
            right.push(*sample);
        }
    }
    let mut total = 0.0f32;
    let mut folds = 0.0f32;
    for (train, test) in [(&left, &right), (&right, &left)] {
        if train.is_empty() || test.is_empty() {
            continue;
        }
        let mapping = fit_isotonic(train);
        let mapped: Vec<Sample> =
            test.iter().map(|(p, truth)| (apply_mapping(&mapping, *p), *truth)).collect();
        total += evaluate(&mapped).ece;
        folds += 1.0;
    }
    (folds > 0.0).then(|| total / folds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_well_calibrated_set_has_near_zero_ece() {
        let mut samples = Vec::new();
        for bin in 0..10 {
            let stated = bin as f32 / 10.0 + 0.05;
            // 每箱 100 条，正例数正好等于 stated×100 → 完美校准
            let positives = ((stated * 100.0).round() as usize).min(100);
            for index in 0..100 {
                samples.push((stated, index < positives));
            }
        }
        let calibration = evaluate(&samples);
        assert!(calibration.ece < 0.02, "应当接近完美：{}", calibration.ece);
        assert_eq!(calibration.bins.len(), 10);
    }

    #[test]
    fn overconfidence_shows_up_as_large_ece() {
        let samples: Vec<Sample> = (0..20).map(|index| (0.9, index % 2 == 0)).collect();
        let calibration = evaluate(&samples);
        assert!(calibration.ece > 0.3, "过度自信必须被量出来：{}", calibration.ece);
        assert!(calibration.brier > 0.3);
    }

    #[test]
    fn isotonic_is_monotone_and_improves_out_of_sample_ece() {
        // 造一组"系统性过度自信"的数据：真实正例率 ≈ 说出的概率 × 0.6
        let mut samples = Vec::new();
        for index in 0..120 {
            let stated = ((index % 10) as f32) / 10.0 + 0.05;
            let truth = (index as f32 * 0.618).fract() < stated * 0.6;
            samples.push((stated, truth));
        }
        let before = evaluate(&samples).ece;
        let mapping = fit_isotonic(&samples);
        assert!(!mapping.is_empty());
        for pair in mapping.windows(2) {
            assert!(pair[0].1 <= pair[1].1 + 1e-6, "等渗映射必须单调不减：{mapping:?}");
        }
        let after = out_of_sample_ece(&samples).expect("样本足够");
        assert!(before > 0.05, "构造的数据本来就该是不校准的：{before}");
        assert!(after < before, "折外校准应当有改善：{after} vs {before}");
    }

    #[test]
    fn apply_mapping_uses_the_last_breakpoint_at_or_below_the_value() {
        let mapping = vec![(0.2, 0.05), (0.6, 0.4), (0.9, 0.95)];
        assert_eq!(apply_mapping(&mapping, 0.1), 0.05);
        assert_eq!(apply_mapping(&mapping, 0.5), 0.05);
        assert_eq!(apply_mapping(&mapping, 0.7), 0.4);
        assert_eq!(apply_mapping(&mapping, 0.99), 0.95);
        assert_eq!(apply_mapping(&[], 0.42), 0.42, "没有映射就原样返回");
    }
}
