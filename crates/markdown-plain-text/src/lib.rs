//! A small, dependency-free Markdown-to-plain-text renderer.
//!
//! This implements the Markdown subset commonly used by chat and notification
//! channels. It is intentionally not a complete CommonMark implementation.

use std::collections::HashMap;

type References = HashMap<String, String>;

/// Converts Markdown input to readable plain text.
///
/// Formatting syntax is removed while textual content is retained. Link
/// destinations are retained when they differ from their labels, and code
/// blocks are copied verbatim after fence markers are removed.
pub fn markdown_to_plain_text(markdown: &str) -> String {
    let lines: Vec<&str> = markdown.lines().collect();
    let (references, lines) = remove_reference_definitions(&lines);
    render_blocks(&lines, &references)
        .join("\n")
        .trim()
        .to_string()
}

struct Protected {
    start: usize,
    end: usize,
    text: String,
}

fn remove_reference_definitions<'a>(lines: &[&'a str]) -> (References, Vec<&'a str>) {
    let mut references = References::new();
    let mut kept = Vec::with_capacity(lines.len());

    for line in lines {
        if let Some((label, destination)) = reference_definition(line) {
            references.insert(label.to_lowercase(), destination.to_string());
        } else {
            kept.push(*line);
        }
    }

    (references, kept)
}

fn reference_definition(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    let rest = line.strip_prefix('[')?;
    let label_end = find_closing_bracket(rest, 0)?;
    let label = &rest[..label_end];
    if label.is_empty() || label.contains('\\') {
        return None;
    }

    let after_label = &rest[label_end + 1..];
    let after_colon = after_label.strip_prefix(':')?.trim_start();
    if after_colon.is_empty() {
        return None;
    }

    let destination = if let Some(rest) = after_colon.strip_prefix('<') {
        let end = rest.find('>')?;
        &rest[..end]
    } else {
        after_colon.split_whitespace().next()?
    };

    if destination.is_empty() || destination.starts_with('[') {
        return None;
    }
    Some((label, destination))
}

fn render_blocks(lines: &[&str], references: &References) -> Vec<String> {
    let mut output = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];
        if line.trim().is_empty() {
            push_blank(&mut output);
            index += 1;
            continue;
        }

        if let Some((fence_char, fence_len, _)) = parse_fence(line) {
            index += 1;
            let mut code = Vec::new();
            while index < lines.len() {
                if let Some((close_char, close_len, _)) = parse_fence(lines[index])
                    && close_char == fence_char
                    && close_len >= fence_len
                {
                    index += 1;
                    break;
                }
                code.push(lines[index]);
                index += 1;
            }
            push_lines(&mut output, &code);
            continue;
        }

        if let Some(heading) = parse_heading(line) {
            push_block(&mut output, render_inline(heading, references));
            index += 1;
            continue;
        }

        if is_thematic_break(line) {
            push_blank(&mut output);
            index += 1;
            continue;
        }

        if indentation(line) >= 4 {
            let mut code = Vec::new();
            while index < lines.len()
                && (lines[index].trim().is_empty() || indentation(lines[index]) >= 4)
            {
                code.push(remove_code_indent(lines[index]));
                index += 1;
            }
            while code.last().is_some_and(|line| line.is_empty()) {
                code.pop();
            }
            push_lines(&mut output, &code);
            continue;
        }

        if line.trim_start().starts_with('>') {
            let mut quoted = Vec::new();
            while index < lines.len() && lines[index].trim_start().starts_with('>') {
                quoted.push(strip_block_quote(lines[index]));
                index += 1;
            }
            let rendered = render_blocks(&quoted, references).join("\n");
            push_block(&mut output, rendered);
            continue;
        }

        if let Some((marker, ordered, number)) = parse_list_item(line) {
            index = render_list(
                lines,
                index,
                marker.indent,
                ordered,
                number,
                references,
                &mut output,
            );
            continue;
        }

        if is_table_start(lines, index) {
            let header = table_cells(line);
            index += 2;
            let mut rows = vec![table_row(&header, references)];
            while index < lines.len()
                && !lines[index].trim().is_empty()
                && lines[index].contains('|')
            {
                rows.push(table_row(&table_cells(lines[index]), references));
                index += 1;
            }
            push_lines(
                &mut output,
                &rows.iter().map(String::as_str).collect::<Vec<_>>(),
            );
            continue;
        }

        let mut paragraph = Vec::new();
        while index < lines.len()
            && !lines[index].trim().is_empty()
            && parse_heading(lines[index]).is_none()
            && parse_fence(lines[index]).is_none()
            && !is_thematic_break(lines[index])
            && !lines[index].trim_start().starts_with('>')
            && parse_list_item(lines[index]).is_none()
        {
            paragraph.push(lines[index].trim());
            index += 1;

            if index < lines.len() && is_setext_underline(lines[index]) {
                if paragraph.len() > 1 {
                    let body = paragraph[..paragraph.len() - 1].join(" ");
                    push_block(&mut output, render_inline(&body, references));
                }
                let title = paragraph.pop().expect("paragraph is not empty");
                push_block(&mut output, render_inline(title, references));
                index += 1;
                break;
            }
        }

        if !paragraph.is_empty() {
            push_block(&mut output, render_inline(&paragraph.join(" "), references));
        }
    }

    while output.last().is_some_and(String::is_empty) {
        output.pop();
    }
    output
}

#[allow(clippy::too_many_arguments)]
fn render_list(
    lines: &[&str],
    start: usize,
    base_indent: usize,
    ordered: bool,
    first_number: String,
    references: &References,
    output: &mut Vec<String>,
) -> usize {
    let mut index = start;
    let mut item: Option<ListItem> = None;
    let mut number = first_number;

    fn flush(
        item: &mut Option<ListItem>,
        ordered: bool,
        number: &str,
        references: &References,
        output: &mut Vec<String>,
    ) {
        let Some(current) = item.take() else {
            return;
        };
        let mut content = current.content;
        if let Some((done, rest)) = parse_task(&content[0]) {
            content[0] = format!("{}: {rest}", if done { "DONE" } else { "TODO" });
        }

        let normalized: Vec<String> = content
            .iter()
            .enumerate()
            .map(|(line_index, line)| {
                if line.trim().is_empty() {
                    String::new()
                } else if line_index == 0 {
                    line.to_string()
                } else {
                    remove_indent(line, current.marker_width)
                }
            })
            .collect();
        let content_lines: Vec<&str> = normalized.iter().map(String::as_str).collect();
        let rendered = render_blocks(&content_lines, references).join("\n");
        let mut item_lines = rendered.lines();
        let prefix = if ordered {
            format!("{number}. ")
        } else {
            "• ".to_string()
        };

        if let Some(first) = item_lines.next() {
            output.push(format!("{prefix}{first}"));
        }
        for line in item_lines {
            output.push(format!("  {line}"));
        }
    }

    while index < lines.len() {
        let line = lines[index];
        if line.trim().is_empty() {
            let list_continues = lines.get(index + 1).is_some_and(|next| {
                parse_list_item(next).is_some_and(|marker| marker.0.indent >= base_indent)
            });
            if list_continues && item.is_some() {
                if let Some(current) = item.as_mut() {
                    current.content.push(String::new());
                }
                index += 1;
                continue;
            }
            break;
        }

        if let Some((marker, item_ordered, item_number)) = parse_list_item(line)
            && marker.indent == base_indent
        {
            if item_ordered != ordered {
                break;
            }
            flush(&mut item, ordered, &number, references, output);
            item = Some(ListItem {
                marker_width: marker.indent + marker.width,
                content: vec![marker.content.to_string()],
            });
            if item_ordered {
                number = item_number;
            }
            index += 1;
            continue;
        }

        if item.is_some() {
            if let Some(current) = item.as_mut() {
                current.content.push(line.to_string());
            }
            index += 1;
        } else {
            break;
        }
    }

    flush(&mut item, ordered, &number, references, output);
    index
}

struct ListItem {
    marker_width: usize,
    content: Vec<String>,
}

struct Marker<'a> {
    indent: usize,
    width: usize,
    content: &'a str,
}

fn parse_list_item(line: &str) -> Option<(Marker<'_>, bool, String)> {
    let indent = indentation(line);
    if indent >= 4 {
        return None;
    }

    let rest = line.trim_start();
    let (marker, marker_width, ordered, number) = if let Some(after) = rest
        .strip_prefix('-')
        .or_else(|| rest.strip_prefix('*'))
        .or_else(|| rest.strip_prefix('+'))
    {
        if !after.starts_with(' ') && !after.starts_with('\t') {
            return None;
        }
        ("-", 2usize, false, String::new())
    } else {
        let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits == 0 || digits > 9 {
            return None;
        }
        let after_digits = &rest[digits..];
        let delimiter = after_digits.chars().next()?;
        if delimiter != '.' && delimiter != ')' {
            return None;
        }
        let after = &after_digits[delimiter.len_utf8()..];
        if !after.starts_with(' ') && !after.starts_with('\t') {
            return None;
        }
        (".", digits + 2, true, rest[..digits].to_string())
    };

    let marker_at = rest.find(marker).expect("marker exists");
    let after_marker = &rest[marker_at + marker.len()..];
    let content = after_marker
        .strip_prefix([' ', '\t'])
        .unwrap_or(after_marker);
    Some((
        Marker {
            indent,
            width: marker_width,
            content,
        },
        ordered,
        number,
    ))
}

fn parse_task(content: &str) -> Option<(bool, &str)> {
    let rest = content.strip_prefix('[')?;
    let (done, rest) = rest
        .strip_prefix(' ')
        .map(|rest| (false, rest))
        .or_else(|| rest.strip_prefix('x').map(|rest| (true, rest)))
        .or_else(|| rest.strip_prefix('X').map(|rest| (true, rest)))?;
    let rest = rest.strip_prefix(']')?.strip_prefix(' ')?;
    Some((done, rest))
}

fn parse_fence(line: &str) -> Option<(char, usize, bool)> {
    if indentation(line) >= 4 {
        return None;
    }
    let rest = line.trim_start();
    let fence_char = rest.chars().next()?;
    if fence_char != '`' && fence_char != '~' {
        return None;
    }
    let len = rest.chars().take_while(|&c| c == fence_char).count();
    if len < 3 {
        return None;
    }
    let info = rest[len..].trim();
    let is_open = if fence_char == '`' {
        !info.contains('`')
    } else {
        true
    };
    Some((fence_char, len, is_open))
}

fn parse_heading(line: &str) -> Option<&str> {
    if indentation(line) >= 4 {
        return None;
    }
    let rest = line.trim_start();
    let hashes = rest.chars().take_while(|&c| c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let heading = rest[hashes..]
        .strip_prefix([' ', '\t'])
        .unwrap_or(&rest[hashes..]);
    if heading.trim_end().ends_with('#') {
        return Some(heading.trim_end().trim_end_matches('#').trim());
    }
    Some(heading.trim())
}

fn is_setext_underline(line: &str) -> bool {
    if indentation(line) >= 4 {
        return false;
    }
    let rest = line.trim();
    !rest.is_empty() && (rest.chars().all(|c| c == '=') || rest.chars().all(|c| c == '-'))
}

fn is_thematic_break(line: &str) -> bool {
    if indentation(line) >= 4 {
        return false;
    }
    let rest = line.trim();
    if rest.len() < 3 {
        return false;
    }
    let first = rest.chars().next().expect("nonempty");
    if first != '-' && first != '*' && first != '_' {
        return false;
    }
    rest.chars().all(|c| c == first || c.is_whitespace())
        && rest.chars().filter(|&c| c == first).count() >= 3
}

fn strip_block_quote(line: &str) -> &str {
    let rest = line.trim_start();
    let rest = rest.strip_prefix('>').unwrap_or(rest);
    rest.strip_prefix([' ', '\t']).unwrap_or(rest)
}

fn indentation(line: &str) -> usize {
    let mut width = 0;
    for char in line.chars() {
        match char {
            ' ' => width += 1,
            '\t' => width += 4 - (width % 4),
            _ => break,
        }
    }
    width
}

fn remove_code_indent(line: &str) -> &str {
    let mut spaces = 0;
    for (offset, char) in line.char_indices() {
        match char {
            ' ' if spaces < 3 => spaces += 1,
            ' ' | '\t' => return &line[offset + char.len_utf8()..],
            _ => return line,
        }
    }
    ""
}

fn remove_indent(line: &str, mut amount: usize) -> String {
    let result = String::new();
    for (offset, char) in line.char_indices() {
        if amount == 0 {
            return format!("{result}{}", &line[offset..]);
        }
        let width = match char {
            ' ' => 1,
            '\t' => 4,
            _ => 0,
        };
        if width == 0 || width > amount {
            return format!("{result}{}", &line[offset..]);
        }
        amount -= width;
    }
    result
}

fn is_table_start(lines: &[&str], index: usize) -> bool {
    lines.get(index).is_some_and(|header| header.contains('|'))
        && lines
            .get(index + 1)
            .is_some_and(|delimiter| is_table_delimiter(delimiter))
}

fn is_table_delimiter(line: &str) -> bool {
    let cells = split_table_row(line);
    !cells.is_empty()
        && cells.iter().all(|cell| {
            let cell = cell.trim();
            !cell.is_empty()
                && cell.chars().all(|c| c == '-' || c == ':' || c == ' ')
                && cell.chars().any(|c| c == '-')
        })
}

fn table_cells(line: &str) -> Vec<String> {
    split_table_row(line)
        .into_iter()
        .map(|cell| cell.trim().to_string())
        .collect()
}

fn split_table_row(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    let trimmed = trimmed.strip_prefix('|').unwrap_or(trimmed).trim_end();
    let trimmed = trimmed.strip_suffix('|').unwrap_or(trimmed);

    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut escaped = false;
    for char in trimmed.chars() {
        if escaped {
            if char != '|' {
                cell.push('\\');
            }
            cell.push(char);
            escaped = false;
        } else if char == '\\' {
            escaped = true;
        } else if char == '|' {
            cells.push(cell.clone());
            cell.clear();
        } else {
            cell.push(char);
        }
    }
    if escaped {
        cell.push('\\');
    }
    cells.push(cell);
    cells
}

fn table_row(cells: &[String], references: &References) -> String {
    cells
        .iter()
        .map(|cell| render_inline(cell, references))
        .collect::<Vec<_>>()
        .join("\t")
}

fn push_blank(output: &mut Vec<String>) {
    if !output.is_empty() && !output.last().is_some_and(String::is_empty) {
        output.push(String::new());
    }
}

fn push_block(output: &mut Vec<String>, block: String) {
    for line in block.lines() {
        output.push(line.to_string());
    }
}

fn push_lines(output: &mut Vec<String>, lines: &[&str]) {
    for line in lines {
        output.push((*line).to_string());
    }
}

fn render_inline(input: &str, references: &References) -> String {
    let protected = scan_protected(input, references);
    let masked = mask_protected(input, &protected);
    let mut rendered = render_emphasis(&masked);
    for (index, item) in protected.iter().enumerate() {
        let placeholder = placeholder(index);
        rendered = rendered.replace(placeholder, &item.text);
    }
    rendered
}

fn scan_protected(input: &str, references: &References) -> Vec<Protected> {
    let mut protected = Vec::new();
    let mut index = 0;

    while index < input.len() {
        if let Some(&Protected { end, .. }) = protected.last()
            && index < end
        {
            index = end;
            continue;
        }

        if input.as_bytes()[index] == b'\\'
            && input[index + 1..]
                .chars()
                .next()
                .is_some_and(|char| char.is_ascii_punctuation())
        {
            let next_len = next_char_len(input, index + 1);
            protected.push(Protected {
                start: index,
                end: index + 1 + next_len,
                text: input[index + 1..index + 1 + next_len].to_string(),
            });
            index += 1 + next_len;
            continue;
        }

        if input.as_bytes()[index] == b'`'
            && let Some((end, text)) = parse_code_span(input, index)
        {
            protected.push(Protected {
                start: index,
                end,
                text,
            });
            index = end;
            continue;
        }

        if input.as_bytes()[index] == b'&'
            && let Some((end, text)) = parse_entity(input, index)
        {
            protected.push(Protected {
                start: index,
                end,
                text,
            });
            index = end;
            continue;
        }

        if input.as_bytes()[index] == b'<' {
            if let Some((end, text)) = parse_autolink(input, index) {
                protected.push(Protected {
                    start: index,
                    end,
                    text,
                });
                index = end;
                continue;
            }
            if let Some(end) = parse_html(input, index) {
                protected.push(Protected {
                    start: index,
                    end,
                    text: String::new(),
                });
                index = end;
                continue;
            }
        }

        let is_link_start = input.as_bytes()[index] == b'!' || input.as_bytes()[index] == b'[';
        if is_link_start && let Some((end, text)) = parse_link(input, index, references) {
            protected.push(Protected {
                start: index,
                end,
                text,
            });
            index = end;
            continue;
        }

        index += next_char_len(input, index);
    }

    protected
}

fn next_char_len(input: &str, index: usize) -> usize {
    input[index..].chars().next().map_or(1, char::len_utf8)
}

fn parse_code_span(input: &str, start: usize) -> Option<(usize, String)> {
    let opening = input[start..].chars().take_while(|&c| c == '`').count();
    let mut index = start + opening;
    while index < input.len() {
        if input.as_bytes()[index] != b'`' {
            index += next_char_len(input, index);
            continue;
        }
        let closing = input[index..].chars().take_while(|&c| c == '`').count();
        if closing == opening {
            let mut text = input[start + opening..index].replace('\n', " ");
            if text.starts_with(' ') && text.ends_with(' ') && !text.trim().is_empty() {
                text = text[1..text.len() - 1].to_string();
            }
            return Some((index + closing, text));
        }
        index += closing;
    }
    None
}

fn parse_entity(input: &str, start: usize) -> Option<(usize, String)> {
    let rest = &input[start + 1..];
    let end = rest.find(';')?;
    if end > 32 {
        return None;
    }
    let entity = &rest[..end];
    let decoded = if let Some(number) = entity
        .strip_prefix("#X")
        .or_else(|| entity.strip_prefix("#x"))
    {
        char::from_u32(u32::from_str_radix(number, 16).ok()?)?
    } else if let Some(number) = entity.strip_prefix('#') {
        char::from_u32(number.parse::<u32>().ok()?)?
    } else {
        match entity {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            "nbsp" => '\u{a0}',
            _ => return None,
        }
    };
    Some((start + 1 + end + 1, decoded.to_string()))
}

fn parse_autolink(input: &str, start: usize) -> Option<(usize, String)> {
    let rest = &input[start + 1..];
    let end = rest.find('>')?;
    let content = &rest[..end];
    if content.is_empty() || content.chars().any(char::is_whitespace) || content.contains('<') {
        return None;
    }

    let valid_uri = content.split_once(':').is_some_and(|(scheme, _)| {
        scheme
            .chars()
            .next()
            .is_some_and(|char| char.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '.' || c == '-')
    });
    let valid_email = content.split('@').filter(|part| !part.is_empty()).count() == 2;
    (valid_uri || valid_email).then(|| (start + 1 + end + 1, content.to_string()))
}

fn parse_html(input: &str, start: usize) -> Option<usize> {
    let rest = &input[start + 1..];
    if let Some(comment) = rest.strip_prefix("!--") {
        let end = comment.find("-->")?;
        return Some(start + 1 + 3 + end + 3);
    }
    if let Some(processing) = rest.strip_prefix('?') {
        let end = processing.find("?>")?;
        return Some(start + 1 + 1 + end + 2);
    }
    if rest.starts_with("![CDATA[") {
        let end = rest.find("]]>")?;
        return Some(start + 1 + end + 3);
    }
    if let Some(declaration) = rest.strip_prefix('!') {
        let end = declaration.find('>')?;
        return Some(start + 1 + 1 + end + 1);
    }

    let after_slash = rest.strip_prefix('/').unwrap_or(rest);
    if !after_slash
        .chars()
        .next()
        .is_some_and(|char| char.is_ascii_alphabetic())
    {
        return None;
    }
    let end = rest.find('>')?;
    Some(start + 1 + end + 1)
}

fn parse_link(input: &str, start: usize, references: &References) -> Option<(usize, String)> {
    let is_image = input.as_bytes().get(start) == Some(&b'!');
    let bracket_start = if is_image { start + 1 } else { start };
    if input.as_bytes().get(bracket_start) != Some(&b'[') {
        return None;
    }
    let label_rest = &input[bracket_start + 1..];
    let label_end = find_closing_bracket(label_rest, 0)?;
    let label = &label_rest[..label_end];
    let after_label = bracket_start + 1 + label_end + 1;

    let (destination, preserve_label, end) = if input.as_bytes().get(after_label) == Some(&b'(') {
        let paren_rest = &input[after_label + 1..];
        let mut depth = 1;
        let mut escaped = false;
        let mut offset = 0;
        for char in paren_rest.chars() {
            if escaped {
                escaped = false;
            } else if char == '\\' {
                escaped = true;
            } else if char == '(' {
                depth += 1;
            } else if char == ')' {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            offset += char.len_utf8();
        }
        if depth != 0 {
            return None;
        }
        let inside = paren_rest[..offset].trim();
        let (destination, _title) = split_link_destination(inside)?;
        (destination.to_string(), true, after_label + 1 + offset + 1)
    } else if input.as_bytes().get(after_label) == Some(&b'[') {
        let reference_rest = &input[after_label + 1..];
        let reference_end = find_closing_bracket(reference_rest, 0)?;
        let reference = &reference_rest[..reference_end];
        let key = if reference.is_empty() {
            label
        } else {
            reference
        };
        let destination = references.get(&key.to_lowercase())?;
        (destination.clone(), true, after_label + reference_end + 2)
    } else {
        let destination = references.get(&label.to_lowercase())?;
        (destination.clone(), false, after_label)
    };

    let rendered_label = render_inline(label, references);
    if !preserve_label || rendered_label == destination || rendered_label.is_empty() {
        Some((end, destination))
    } else {
        Some((end, format!("{rendered_label} ({destination})")))
    }
}

fn split_link_destination(input: &str) -> Option<(&str, Option<&str>)> {
    if input.is_empty() {
        return None;
    }
    if let Some(rest) = input.strip_prefix('<') {
        let end = rest.find('>')?;
        return Some((&rest[..end], None));
    }

    let mut escaped = false;
    for (offset, char) in input.char_indices() {
        if escaped {
            escaped = false;
        } else if char == '\\' {
            escaped = true;
        } else if char.is_whitespace() {
            let title = input[offset..].trim();
            return Some((&input[..offset], (!title.is_empty()).then_some(title)));
        }
    }
    Some((input, None))
}

fn find_closing_bracket(input: &str, start: usize) -> Option<usize> {
    let mut depth = 1;
    let mut escaped = false;
    for (offset, char) in input[start..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match char {
            '\\' => escaped = true,
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(start + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn mask_protected(input: &str, protected: &[Protected]) -> String {
    let mut masked = String::with_capacity(input.len());
    let mut index = 0;
    for (item_index, item) in protected.iter().enumerate() {
        if item.start < index {
            continue;
        }
        masked.push_str(&input[index..item.start]);
        masked.push(placeholder(item_index));
        index = item.end;
    }
    masked.push_str(&input[index.min(input.len())..]);
    masked
}

fn placeholder(index: usize) -> char {
    char::from_u32(u32::from(u16::try_from(index).expect("protected span count fits u16")) + 0xe000)
        .expect("private use range")
}

fn render_emphasis(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;

    while cursor < input.len() {
        let char = input[cursor..].chars().next().expect("valid cursor");
        if char != '*' && char != '_' && char != '~' {
            output.push(char);
            cursor += char.len_utf8();
            continue;
        }

        let run_len = input[cursor..]
            .chars()
            .take_while(|&run_char| run_char == char)
            .count();
        let run_end = cursor + run_len * char.len_utf8();
        let before = input[..cursor].chars().next_back();
        let after = input[run_end..].chars().next();

        if can_open(before, after, char, run_len)
            && let Some((close_start, close_end)) =
                find_matching_close(input, run_end, char, run_len)
        {
            output.push_str(&render_emphasis(&input[run_end..close_start]));
            cursor = close_end;
            continue;
        }

        for _ in 0..run_len {
            output.push(char);
        }
        cursor = run_end;
    }

    output
}

fn find_matching_close(
    input: &str,
    start: usize,
    delimiter: char,
    opening_len: usize,
) -> Option<(usize, usize)> {
    let mut cursor = start;
    while cursor < input.len() {
        let char = input[cursor..].chars().next()?;
        if char != delimiter {
            cursor += char.len_utf8();
            continue;
        }

        let run_len = input[cursor..]
            .chars()
            .take_while(|&run_char| run_char == delimiter)
            .count();
        let run_end = cursor + run_len * delimiter.len_utf8();
        let before = input[..cursor].chars().next_back();
        let after = input[run_end..].chars().next();
        if run_len >= opening_len && can_close(before, after, delimiter, run_len) {
            return Some((cursor, run_end));
        }
        cursor = run_end;
    }
    None
}

fn can_open(before: Option<char>, after: Option<char>, delimiter: char, len: usize) -> bool {
    let Some(after) = after.filter(|value| !value.is_whitespace()) else {
        return false;
    };
    if delimiter == '~' {
        return len >= 2 && before.is_none_or(char::is_whitespace);
    }
    if delimiter == '_' {
        let before_is_boundary =
            before.is_none_or(|value| value.is_whitespace() || value.is_ascii_punctuation());
        return before_is_boundary || !after.is_ascii_punctuation();
    }
    true
}

fn can_close(before: Option<char>, after: Option<char>, delimiter: char, len: usize) -> bool {
    let Some(before) = before.filter(|value| !value.is_whitespace()) else {
        return false;
    };
    if delimiter == '~' {
        return len >= 2 && after.is_none_or(char::is_whitespace);
    }
    if delimiter == '_' {
        let after_is_boundary =
            after.is_none_or(|value| value.is_whitespace() || value.is_ascii_punctuation());
        return after_is_boundary || !before.is_ascii_punctuation();
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_common_inline_formatting() {
        assert_eq!(
            markdown_to_plain_text(
                "# Alert\n\nVisit **the site**, read *carefully*, or ~~skip~~ wait for `code`."
            ),
            "Alert\n\nVisit the site, read carefully, or skip wait for code."
        );
    }

    #[test]
    fn preserves_literal_escaped_and_intraword_markers() {
        assert_eq!(
            markdown_to_plain_text(r"snake_case stays literal and \*this\* is literal"),
            "snake_case stays literal and *this* is literal"
        );
    }

    #[test]
    fn links_keep_destination_when_it_is_not_the_label() {
        assert_eq!(
            markdown_to_plain_text("[Open the guide](https://example.com/a?x=1_y)"),
            "Open the guide (https://example.com/a?x=1_y)"
        );
        assert_eq!(
            markdown_to_plain_text("<https://example.com>"),
            "https://example.com"
        );
    }

    #[test]
    fn renders_lists_and_task_states() {
        assert_eq!(
            markdown_to_plain_text("- one\n- two\n  - nested\n1. first\n2. second"),
            "• one\n• two\n  • nested\n1. first\n2. second"
        );
        assert_eq!(
            markdown_to_plain_text("- [ ] todo\n- [x] done"),
            "• TODO: todo\n• DONE: done"
        );
    }

    #[test]
    fn renders_structural_blocks_without_markdown_markers() {
        assert_eq!(
            markdown_to_plain_text("> quoted **text**\n\n---\n\nSetext\n======"),
            "quoted text\n\nSetext"
        );
    }

    #[test]
    fn renders_code_blocks_verbatim() {
        assert_eq!(
            markdown_to_plain_text("```rust\nlet x = *value;\n```\n\n    indented `code`"),
            "let x = *value;\n\nindented `code`"
        );
    }

    #[test]
    fn renders_tables_as_tab_separated_rows() {
        assert_eq!(
            markdown_to_plain_text("| Name | Status |\n| --- | --- |\n| **A** | OK |"),
            "Name\tStatus\nA\tOK"
        );
    }

    #[test]
    fn removes_html_and_reference_definitions() {
        assert_eq!(
            markdown_to_plain_text("<b>bold</b>\n\n[ref]: https://example.com\nSee [ref]"),
            "bold\n\nSee https://example.com"
        );
    }
}
