use std::cmp::Ordering;

/// Compare two version strings numerically, so 1.10 is newer than 1.9.
/// Missing trailing components count as zero. Numerically equal versions
/// compare equal; callers can use the tag string as a tie-breaker.
pub fn compare(a: &str, b: &str) -> Ordering {
    let left = numeric_parts(a);
    let right = numeric_parts(b);
    let len = left.len().max(right.len());
    for index in 0..len {
        let left = left.get(index).copied().unwrap_or(0);
        let right = right.get(index).copied().unwrap_or(0);
        match left.cmp(&right) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
    }
    Ordering::Equal
}

fn numeric_parts(value: &str) -> Vec<u64> {
    let mut parts = Vec::new();
    let mut current = 0u64;
    let mut in_number = false;
    for character in value.chars() {
        if let Some(digit) = character.to_digit(10) {
            current = current * 10 + u64::from(digit);
            in_number = true;
        } else if in_number {
            parts.push(current);
            current = 0;
            in_number = false;
        }
    }
    if in_number {
        parts.push(current);
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::compare;
    use std::cmp::Ordering;

    #[test]
    fn numeric_ordering() {
        assert_eq!(compare("1.10", "1.9"), Ordering::Greater);
        assert_eq!(compare("3.22.35", "3.22.9"), Ordering::Greater);
        assert_eq!(compare("2023.03.0", "2023.3.0"), Ordering::Equal);
        assert_eq!(compare("160.0.1-1", "160.0.1"), Ordering::Greater);
        assert_eq!(compare("v1.3.1", "1.3.1"), Ordering::Equal);
        assert_eq!(compare("2.0", "10.0"), Ordering::Less);
    }
}
