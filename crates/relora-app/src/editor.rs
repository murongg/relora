#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlEditorBuffer {
    lines: Vec<String>,
    cursor_row: usize,
    cursor_col: usize,
}

impl SqlEditorBuffer {
    pub fn from_sql(sql: &str) -> Self {
        let lines = split_lines(sql);
        let cursor_row = lines.len().saturating_sub(1);
        let cursor_col = lines.last().map(|line| line.chars().count()).unwrap_or(0);
        Self {
            lines,
            cursor_row,
            cursor_col,
        }
    }

    pub fn sql(&self) -> String {
        self.lines.join("\n")
    }

    pub fn current_statement(&self) -> String {
        let sql = self.sql();
        let cursor_offset = self.cursor_byte_offset(&sql);
        extract_statement_at(&sql, cursor_offset)
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    pub fn cursor(&self) -> (usize, usize) {
        (self.cursor_row, self.cursor_col)
    }

    pub fn completion_prefix(&self) -> Option<String> {
        let (start, end) = self.current_word_bounds()?;
        if start == end {
            return None;
        }
        Some(
            self.lines[self.cursor_row]
                .chars()
                .skip(start)
                .take(end - start)
                .collect(),
        )
    }

    pub fn replace_sql(&mut self, sql: &str) {
        *self = Self::from_sql(sql);
    }

    pub fn apply_completion(&mut self, replacement: &str) -> bool {
        let Some((start, end)) = self.current_word_bounds() else {
            return false;
        };
        let line = self.lines.get(self.cursor_row).cloned().unwrap_or_default();
        let start_byte = char_to_byte_index(&line, start);
        let end_byte = char_to_byte_index(&line, end);
        let mut updated = line;
        updated.replace_range(start_byte..end_byte, replacement);
        self.lines[self.cursor_row] = updated;
        self.cursor_col = start + replacement.chars().count();
        true
    }

    pub fn insert_char(&mut self, ch: char) {
        let row = self.cursor_row.min(self.lines.len().saturating_sub(1));
        let col = self.cursor_col;
        let line = &self.lines[row];
        let byte_index = char_to_byte_index(line, col);
        let mut updated = line.clone();
        updated.insert(byte_index, ch);
        self.lines[row] = updated;
        self.cursor_col += 1;
    }

    pub fn insert_str(&mut self, value: &str) {
        for ch in value.chars() {
            self.insert_char(ch);
        }
    }

    pub fn backspace(&mut self) {
        if self.cursor_row >= self.lines.len() {
            return;
        }

        if self.cursor_col > 0 {
            let line = &self.lines[self.cursor_row];
            let end = char_to_byte_index(line, self.cursor_col);
            let start = char_to_byte_index(line, self.cursor_col - 1);
            let mut updated = line.clone();
            updated.replace_range(start..end, "");
            self.lines[self.cursor_row] = updated;
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            let current = self.lines.remove(self.cursor_row);
            self.cursor_row -= 1;
            let previous_len = self.lines[self.cursor_row].chars().count();
            self.lines[self.cursor_row].push_str(&current);
            self.cursor_col = previous_len;
        }
    }

    pub fn new_line(&mut self) {
        if self.cursor_row >= self.lines.len() {
            self.lines.push(String::new());
            self.cursor_row = self.lines.len() - 1;
            self.cursor_col = 0;
            return;
        }

        let line = self.lines[self.cursor_row].clone();
        let split_at = char_to_byte_index(&line, self.cursor_col);
        let (left, right) = line.split_at(split_at);
        self.lines[self.cursor_row] = left.to_string();
        self.lines.insert(self.cursor_row + 1, right.to_string());
        self.cursor_row += 1;
        self.cursor_col = 0;
    }

    pub fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].chars().count();
        }
    }

    pub fn move_right(&mut self) {
        let line_len = self.lines[self.cursor_row].chars().count();
        if self.cursor_col < line_len {
            self.cursor_col += 1;
        } else if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col = 0;
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self
                .cursor_col
                .min(self.lines[self.cursor_row].chars().count());
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col = self
                .cursor_col
                .min(self.lines[self.cursor_row].chars().count());
        }
    }

    fn cursor_byte_offset(&self, sql: &str) -> usize {
        let mut offset = 0;
        for (index, line) in self.lines.iter().enumerate() {
            if index == self.cursor_row {
                return offset + char_to_byte_index(line, self.cursor_col);
            }
            offset += line.len() + 1;
        }
        sql.len()
    }

    fn current_word_bounds(&self) -> Option<(usize, usize)> {
        let line = self.lines.get(self.cursor_row)?;
        let chars = line.chars().collect::<Vec<_>>();
        let mut start = self.cursor_col.min(chars.len());
        while start > 0 && is_completion_char(chars[start - 1]) {
            start -= 1;
        }

        let mut end = self.cursor_col.min(chars.len());
        while end < chars.len() && is_completion_char(chars[end]) {
            end += 1;
        }

        (start != end).then_some((start, end))
    }
}

fn split_lines(value: &str) -> Vec<String> {
    let mut lines = value.lines().map(str::to_string).collect::<Vec<_>>();
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn char_to_byte_index(value: &str, char_index: usize) -> usize {
    value
        .char_indices()
        .map(|(index, _)| index)
        .nth(char_index)
        .unwrap_or(value.len())
}

fn extract_statement_at(sql: &str, cursor_offset: usize) -> String {
    let ranges = statement_ranges(sql);
    if ranges.is_empty() {
        return sql.trim().to_string();
    }

    let cursor_offset = cursor_offset.min(sql.len());
    let selected = ranges
        .iter()
        .find(|(start, end)| cursor_offset >= *start && cursor_offset <= *end)
        .or_else(|| ranges.iter().find(|(start, _)| cursor_offset < *start))
        .or_else(|| ranges.last())
        .copied()
        .unwrap_or((0, sql.len()));

    sql[selected.0..selected.1].trim().to_string()
}

fn statement_ranges(sql: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start = 0usize;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut previous = '\0';
    let mut chars = sql.char_indices().peekable();

    while let Some((index, ch)) = chars.next() {
        let next = chars.peek().map(|(_, value)| *value);

        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
            }
            previous = ch;
            continue;
        }
        if in_block_comment {
            if previous == '*' && ch == '/' {
                in_block_comment = false;
            }
            previous = ch;
            continue;
        }
        if in_single_quote {
            if ch == '\'' && next == Some('\'') {
                let _ = chars.next();
                previous = '\0';
                continue;
            }
            if ch == '\'' {
                in_single_quote = false;
            }
            previous = ch;
            continue;
        }
        if in_double_quote {
            if ch == '"' && next == Some('"') {
                let _ = chars.next();
                previous = '\0';
                continue;
            }
            if ch == '"' {
                in_double_quote = false;
            }
            previous = ch;
            continue;
        }

        match (ch, next) {
            ('-', Some('-')) => {
                in_line_comment = true;
                let _ = chars.next();
            }
            ('/', Some('*')) => {
                in_block_comment = true;
                let _ = chars.next();
            }
            ('\'', _) => in_single_quote = true,
            ('"', _) => in_double_quote = true,
            (';', _) => {
                let end = index + ch.len_utf8();
                if !sql[start..end].trim().is_empty() {
                    ranges.push((trim_start_offset(sql, start, end), end));
                }
                start = end;
            }
            _ => {}
        }
        previous = ch;
    }

    if !sql[start..].trim().is_empty() {
        ranges.push((trim_start_offset(sql, start, sql.len()), sql.len()));
    }

    ranges
}

fn trim_start_offset(sql: &str, start: usize, end: usize) -> usize {
    let leading = sql[start..end]
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .map(char::len_utf8)
        .sum::<usize>();
    start + leading
}

fn is_completion_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

#[cfg(test)]
mod tests {
    use super::SqlEditorBuffer;

    #[test]
    fn current_statement_uses_cursor_position_and_ignores_semicolons_in_strings() {
        let mut buffer = SqlEditorBuffer::from_sql("select ';';\nselect 2;\nselect 3;");
        buffer.move_up();

        assert_eq!(buffer.current_statement(), "select 2;");
    }
}
