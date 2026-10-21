//! 保守的逐行三方合并。
//!
//! 只有 base→local 与 base→remote 的修改区间不重叠时才自动合并；重叠区间一律
//! 返回冲突数，交给用户选择，避免把两边语义不同的改写拼成错误句子。

#[derive(Debug, Clone, PartialEq, Eq)]
struct Change {
    start: usize,
    end: usize,
    replacement: Vec<String>,
}

pub fn merge_lines(base: &str, local: &str, remote: &str) -> (Option<String>, usize) {
    if local == remote {
        return (Some(local.to_string()), 0);
    }
    let base_lines = lines(base);
    let local_lines = lines(local);
    let remote_lines = lines(remote);
    if base_lines.len() > 2_000 || local_lines.len() > 2_000 || remote_lines.len() > 2_000 {
        return (None, 1);
    }
    let mut local_changes = changes(&base_lines, &local_lines);
    let remote_changes = changes(&base_lines, &remote_lines);
    if local_changes.is_empty() {
        return (Some(remote.to_string()), 0);
    }
    if remote_changes.is_empty() {
        return (Some(local.to_string()), 0);
    }
    let conflict_count = local_changes
        .iter()
        .flat_map(|left| remote_changes.iter().map(move |right| (left, right)))
        .filter(|(left, right)| overlaps(left, right))
        .count();
    if conflict_count > 0 {
        return (None, conflict_count);
    }
    local_changes.extend(remote_changes);
    local_changes.sort_by_key(|change| (change.start, change.end));
    let mut output = Vec::new();
    let mut cursor = 0usize;
    for change in local_changes {
        if change.start < cursor {
            return (None, 1);
        }
        output.extend_from_slice(&base_lines[cursor..change.start]);
        output.extend(change.replacement);
        cursor = change.end;
    }
    output.extend_from_slice(&base_lines[cursor..]);
    (Some(output.concat()), 0)
}

fn lines(value: &str) -> Vec<String> {
    if value.is_empty() {
        return Vec::new();
    }
    value.split_inclusive('\n').map(str::to_string).collect()
}

fn changes(base: &[String], other: &[String]) -> Vec<Change> {
    let rows = base.len() + 1;
    let cols = other.len() + 1;
    let mut lcs = vec![0u32; rows * cols];
    for left in (0..base.len()).rev() {
        for right in (0..other.len()).rev() {
            lcs[left * cols + right] = if base[left] == other[right] {
                1 + lcs[(left + 1) * cols + right + 1]
            } else {
                lcs[(left + 1) * cols + right].max(lcs[left * cols + right + 1])
            };
        }
    }
    let mut matches = Vec::new();
    let (mut left, mut right) = (0usize, 0usize);
    while left < base.len() && right < other.len() {
        if base[left] == other[right] {
            matches.push((left, right));
            left += 1;
            right += 1;
        } else if lcs[(left + 1) * cols + right] >= lcs[left * cols + right + 1] {
            left += 1;
        } else {
            right += 1;
        }
    }

    let mut output = Vec::new();
    let (mut base_cursor, mut other_cursor) = (0usize, 0usize);
    for (base_match, other_match) in matches
        .into_iter()
        .chain(std::iter::once((base.len(), other.len())))
    {
        if base_match > base_cursor || other_match > other_cursor {
            output.push(Change {
                start: base_cursor,
                end: base_match,
                replacement: other[other_cursor..other_match].to_vec(),
            });
        }
        base_cursor = base_match + 1;
        other_cursor = other_match + 1;
    }
    output
}

fn overlaps(left: &Change, right: &Change) -> bool {
    let left_insert = left.start == left.end;
    let right_insert = right.start == right.end;
    match (left_insert, right_insert) {
        (true, true) => left.start == right.start && left.replacement != right.replacement,
        (true, false) => left.start > right.start && left.start < right.end,
        (false, true) => right.start > left.start && right.start < left.end,
        (false, false) => left.start < right.end && right.start < left.end,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_non_overlapping_line_changes() {
        let base = "A\nB\nC\nD\n";
        let local = "A\nB local\nC\nD\n";
        let remote = "A\nB\nC\nD remote\n";
        assert_eq!(
            merge_lines(base, local, remote),
            (Some("A\nB local\nC\nD remote\n".to_string()), 0)
        );
    }

    #[test]
    fn overlapping_changes_require_manual_resolution() {
        let base = "A\nB\n";
        let local = "A\nlocal\n";
        let remote = "A\nremote\n";
        let (merged, conflicts) = merge_lines(base, local, remote);
        assert!(merged.is_none());
        assert!(conflicts > 0);
    }

    #[test]
    fn one_sided_change_wins_without_manual_work() {
        assert_eq!(
            merge_lines("A\nB\n", "A\nB\n", "A\nremote\n"),
            (Some("A\nremote\n".to_string()), 0)
        );
    }
}
