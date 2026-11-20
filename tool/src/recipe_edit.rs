use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

/// Update the app part's source-tag and the literal top-level version, if
/// present. Only the value text is replaced; comments, quoting style and the
/// rest of the file are left byte-for-byte unchanged.
pub fn apply(
    path: &Path,
    app_part: &str,
    current_tag: &str,
    current_version: Option<&str>,
    new_tag: &str,
    new_version: &str,
) -> Result<()> {
    let text =
        fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    let updated = rewrite(
        &text,
        app_part,
        current_tag,
        current_version,
        new_tag,
        new_version,
    )
    .with_context(|| format!("cannot update {}", path.display()))?;

    if updated != text {
        fs::write(path, updated).with_context(|| format!("cannot write {}", path.display()))?;
    }
    Ok(())
}

fn rewrite(
    text: &str,
    app_part: &str,
    current_tag: &str,
    current_version: Option<&str>,
    new_tag: &str,
    new_version: &str,
) -> Result<String> {
    let lines: Vec<&str> = text
        .split_inclusive('\n')
        .map(|line| split_eol(line).0)
        .collect();

    let source_index = find_source_tag_line(&lines, app_part, current_tag)?;
    let version_index = find_version_line(&lines);

    if let Some(current_version) = current_version {
        let index = version_index.with_context(|| {
            format!("version is {current_version:?} but there is no top-level version:")
        })?;
        let (_, value, _) = key_parts(lines[index], "version")
            .with_context(|| format!("cannot parse version line {:?}", lines[index]))?;
        let actual = unquote(value);
        if actual != current_version {
            bail!("version is {actual:?}, expected {current_version:?}");
        }
    } else if version_index.is_some() {
        bail!("found a top-level version: but the recipe has no version");
    }

    let mut output = String::with_capacity(text.len());
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let (body, eol) = split_eol(line);
        let mut body = body.to_string();
        if index == source_index {
            body = replace_key_value(&body, "source-tag", new_tag)?;
        }
        if Some(index) == version_index {
            body = replace_key_value(&body, "version", new_version)?;
        }
        output.push_str(&body);
        output.push_str(eol);
    }
    Ok(output)
}

fn find_source_tag_line(lines: &[&str], app_part: &str, expected: &str) -> Result<usize> {
    let mut in_parts = false;
    let mut current_part: Option<String> = None;

    for (index, line) in lines.iter().enumerate() {
        let indent = indentation(line);
        let trimmed = line.trim_start();

        if !in_parts {
            if indent == 0 && is_empty_key(trimmed, "parts") {
                in_parts = true;
            }
            continue;
        }

        if indent == 0
            && !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && !is_empty_key(trimmed, "parts")
        {
            break;
        }

        if indent == 2
            && let Some(name) = part_name(trimmed)
        {
            current_part = Some(name);
            continue;
        }

        if current_part.as_deref() == Some(app_part) && indent == 4 && is_key(trimmed, "source-tag")
        {
            let (_, value, _) = key_parts(line, "source-tag")
                .with_context(|| format!("cannot parse source-tag line {:?}", line))?;
            let actual = unquote(value);
            if actual != expected {
                bail!("source-tag is {actual:?}, expected {expected:?}");
            }
            return Ok(index);
        }
    }

    bail!("cannot find source-tag for part {app_part:?}")
}

fn find_version_line(lines: &[&str]) -> Option<usize> {
    lines
        .iter()
        .position(|line| indentation(line) == 0 && is_key(line, "version"))
}

fn part_name(line: &str) -> Option<String> {
    let (key, rest) = line.split_once(':')?;
    let rest = rest.trim();
    if !(rest.is_empty() || rest.starts_with('#')) {
        return None;
    }
    let key = key.trim();
    if key.is_empty() || key.contains(' ') {
        return None;
    }
    Some(
        key.trim_matches(|character| character == '\'' || character == '"')
            .to_string(),
    )
}

/// Return the indentation, value and trailing text of a `key: value` line.
fn key_parts<'a>(line: &'a str, key: &str) -> Option<(&'a str, &'a str, &'a str)> {
    let indent = indentation(line);
    let rest = &line[indent..];
    let after_key = rest.strip_prefix(key)?.strip_prefix(':')?;
    let value_start = after_key.len() - after_key.trim_start_matches([' ', '\t']).len();
    let prefix = &line[..indent + key.len() + 1 + value_start];
    let value_part = &after_key[value_start..];

    let (value, suffix) = if let Some(stripped) = value_part.strip_prefix('\'') {
        let end = stripped.find('\'')? + 2;
        (&value_part[..end], &value_part[end..])
    } else if let Some(stripped) = value_part.strip_prefix('"') {
        let end = stripped.find('"')? + 2;
        (&value_part[..end], &value_part[end..])
    } else {
        let comment = value_part
            .char_indices()
            .find_map(|(index, character)| {
                (character == '#' && (index == 0 || value_part[..index].ends_with([' ', '\t'])))
                    .then_some(index)
            })
            .unwrap_or(value_part.len());
        let value_end = value_part[..comment].trim_end_matches([' ', '\t']).len();
        (&value_part[..value_end], &value_part[value_end..])
    };

    Some((prefix, value, suffix))
}

fn replace_key_value(line: &str, key: &str, new_value: &str) -> Result<String> {
    let (prefix, value, suffix) =
        key_parts(line, key).with_context(|| format!("cannot parse {key} line: {line:?}"))?;
    if value.is_empty() {
        bail!("{key} has an empty value");
    }
    let rendered = render_value(value, new_value);
    Ok(format!("{prefix}{rendered}{suffix}"))
}

fn render_value(existing: &str, new_value: &str) -> String {
    if existing.starts_with('\'') && existing.ends_with('\'') {
        format!("'{}'", new_value.replace('\'', "''"))
    } else if existing.starts_with('"') && existing.ends_with('"') {
        format!(
            "\"{}\"",
            new_value.replace('\\', "\\\\").replace('"', "\\\"")
        )
    } else {
        new_value.to_string()
    }
}

fn unquote(value: &str) -> &str {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        let quoted = (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
            || (bytes[0] == b'"' && bytes[value.len() - 1] == b'"');
        if quoted {
            return &value[1..value.len() - 1];
        }
    }
    value
}

fn is_key(line: &str, key: &str) -> bool {
    let Some(rest) = line.strip_prefix(key) else {
        return false;
    };
    let Some(rest) = rest.strip_prefix(':') else {
        return false;
    };
    rest.is_empty() || rest.starts_with([' ', '\t', '#'])
}

fn is_empty_key(line: &str, key: &str) -> bool {
    let Some(rest) = line.strip_prefix(key) else {
        return false;
    };
    let Some(rest) = rest.strip_prefix(':') else {
        return false;
    };
    let rest = rest.trim();
    rest.is_empty() || rest.starts_with('#')
}

fn indentation(line: &str) -> usize {
    line.chars()
        .take_while(|character| *character == ' ')
        .count()
}

fn split_eol(line: &str) -> (&str, &str) {
    if let Some(body) = line.strip_suffix("\r\n") {
        (body, "\r\n")
    } else if let Some(body) = line.strip_suffix('\n') {
        (body, "\n")
    } else {
        (line, "")
    }
}

#[cfg(test)]
mod tests {
    use super::rewrite;

    const RECIPE: &str = "\
name: example
adopt-info: example
version: '1.2.3'  # release

parts:
  helper:
    source-tag: '0.9.0'  # helper stays put
  example:
    build-packages: [libfoo]
    source-tag: 'v1.2.3'  # current release
    override-build: |
      echo hi  # not a comment
  cleanup:
    source-tag: '9.9.9'
";

    #[test]
    fn updates_only_the_app_part() {
        let output = rewrite(
            RECIPE,
            "example",
            "v1.2.3",
            Some("1.2.3"),
            "v1.3.0",
            "1.3.0",
        )
        .expect("rewrite");
        assert!(output.contains("version: '1.3.0'  # release"));
        assert!(output.contains("source-tag: 'v1.3.0'  # current release"));
        assert!(output.contains("source-tag: '0.9.0'  # helper stays put"));
        assert!(output.contains("source-tag: '9.9.9'"));
        assert!(output.contains("echo hi  # not a comment"));
    }

    #[test]
    fn preserves_bare_values() {
        let recipe = "\
name: example
adopt-info: example
version: 1.2.3
parts:
  example:
    source-tag: v1.2.3
";
        let output = rewrite(
            recipe,
            "example",
            "v1.2.3",
            Some("1.2.3"),
            "v1.3.0",
            "1.3.0",
        )
        .expect("rewrite");
        assert!(output.contains("version: 1.3.0\n"));
        assert!(output.contains("source-tag: v1.3.0\n"));
    }

    #[test]
    fn preserves_double_quotes() {
        let recipe = "\
name: example
adopt-info: example
parts:
  example:
    source-tag: \"v1.2.3\"
";
        let output =
            rewrite(recipe, "example", "v1.2.3", None, "v1.3.0", "1.3.0").expect("rewrite");
        assert!(output.contains("source-tag: \"v1.3.0\""));
    }

    #[test]
    fn updates_recipes_without_a_literal_version() {
        let recipe = "\
name: example
adopt-info: example
parts:
  example:
    source-tag: '51.0'
";
        let output = rewrite(recipe, "example", "51.0", None, "51.1", "51.1").expect("rewrite");
        assert!(output.contains("source-tag: '51.1'"));
        assert!(!output.contains("version:"));
    }

    #[test]
    fn refuses_a_mismatched_tag() {
        let error = rewrite(
            RECIPE,
            "example",
            "v9.9.9",
            Some("1.2.3"),
            "v1.3.0",
            "1.3.0",
        )
        .expect_err("mismatched tag");
        assert!(error.to_string().contains("source-tag is"));
    }

    #[test]
    fn refuses_a_missing_literal_version() {
        let recipe = "\
name: example
adopt-info: example
parts:
  example:
    source-tag: '1.2.3'
";
        let error = rewrite(recipe, "example", "1.2.3", Some("1.2.3"), "1.3.0", "1.3.0")
            .expect_err("missing version");
        assert!(error.to_string().contains("no top-level version"));
    }
}
