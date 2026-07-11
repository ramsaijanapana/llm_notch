/// Unified diff for display in preview UI.
pub fn unified_diff(old_label: &str, new_label: &str, old_text: &str, new_text: &str) -> String {
    if old_text == new_text {
        return String::new();
    }

    let old_lines: Vec<&str> = old_text.lines().collect();
    let new_lines: Vec<&str> = new_text.lines().collect();
    let mut out = format!("--- {old_label}\n+++ {new_label}\n");

    let max = old_lines.len().max(new_lines.len());
    let mut old_idx = 0usize;
    let mut new_idx = 0usize;

    while old_idx < old_lines.len() || new_idx < new_lines.len() {
        if old_idx < old_lines.len()
            && new_idx < new_lines.len()
            && old_lines[old_idx] == new_lines[new_idx]
        {
            old_idx += 1;
            new_idx += 1;
            continue;
        }

        let hunk_start_old = old_idx.saturating_add(1);
        let hunk_start_new = new_idx.saturating_add(1);
        let mut removed = Vec::new();
        let mut added = Vec::new();

        while old_idx < old_lines.len() || new_idx < new_lines.len() {
            let old_line = old_lines.get(old_idx);
            let new_line = new_lines.get(new_idx);
            match (old_line, new_line) {
                (Some(o), Some(n)) if o == n => break,
                (Some(o), Some(_)) => {
                    removed.push(*o);
                    old_idx += 1;
                }
                (Some(o), None) => {
                    removed.push(*o);
                    old_idx += 1;
                }
                (None, Some(n)) => {
                    added.push(*n);
                    new_idx += 1;
                }
                (None, None) => break,
            }
            if removed.len() + added.len() > max {
                break;
            }
        }

        if removed.is_empty() && added.is_empty() {
            break;
        }

        let old_count = removed.len();
        let new_count = added.len();
        out.push_str(&format!(
            "@@ -{hunk_start_old},{old_count} +{hunk_start_new},{new_count} @@\n"
        ));
        for line in &removed {
            out.push('-');
            out.push_str(line);
            out.push('\n');
        }
        for line in &added {
            out.push('+');
            out.push_str(line);
            out.push('\n');
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_when_identical() {
        assert!(unified_diff("a", "b", "{}", "{}").is_empty());
    }

    #[test]
    fn shows_added_lines() {
        let diff = unified_diff("a", "b", "{}", "{\n  \"x\": 1\n}");
        assert!(diff.contains("+"));
    }
}
