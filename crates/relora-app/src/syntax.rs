#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlTokenKind {
    Keyword,
    Identifier,
    String,
    Number,
    Symbol,
    Whitespace,
    Comment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlToken {
    pub kind: SqlTokenKind,
    pub text: String,
}

pub fn highlight_sql_line(line: &str) -> Vec<SqlToken> {
    let chars = line.chars().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut index = 0;

    while index < chars.len() {
        let ch = chars[index];

        if ch.is_whitespace() {
            let start = index;
            while index < chars.len() && chars[index].is_whitespace() {
                index += 1;
            }
            tokens.push(token(SqlTokenKind::Whitespace, &chars[start..index]));
            continue;
        }

        if ch == '-' && chars.get(index + 1) == Some(&'-') {
            tokens.push(token(SqlTokenKind::Comment, &chars[index..]));
            break;
        }

        if ch == '\'' {
            let start = index;
            index += 1;
            while index < chars.len() {
                if chars[index] == '\'' {
                    index += 1;
                    if chars.get(index) == Some(&'\'') {
                        index += 1;
                        continue;
                    }
                    break;
                }
                index += 1;
            }
            tokens.push(token(SqlTokenKind::String, &chars[start..index]));
            continue;
        }

        if ch == '"' {
            let start = index;
            index += 1;
            while index < chars.len() {
                if chars[index] == '"' {
                    index += 1;
                    if chars.get(index) == Some(&'"') {
                        index += 1;
                        continue;
                    }
                    break;
                }
                index += 1;
            }
            tokens.push(token(SqlTokenKind::Identifier, &chars[start..index]));
            continue;
        }

        if ch.is_ascii_digit() {
            let start = index;
            index += 1;
            while index < chars.len()
                && (chars[index].is_ascii_digit() || chars[index] == '.' || chars[index] == '_')
            {
                index += 1;
            }
            tokens.push(token(SqlTokenKind::Number, &chars[start..index]));
            continue;
        }

        if is_identifier_start(ch) {
            let start = index;
            index += 1;
            while index < chars.len() && is_identifier_continue(chars[index]) {
                index += 1;
            }
            let text = chars[start..index].iter().collect::<String>();
            let kind = if is_keyword(&text) {
                SqlTokenKind::Keyword
            } else {
                SqlTokenKind::Identifier
            };
            tokens.push(SqlToken { kind, text });
            continue;
        }

        let start = index;
        index += 1;
        if matches!(ch, '<' | '>' | '!' | ':' | '=')
            && chars.get(index).is_some_and(|next| {
                matches!(
                    (ch, *next),
                    ('<', '=') | ('>', '=') | ('!', '=') | ('<', '>') | (':', ':') | ('=', '>')
                )
            })
        {
            index += 1;
        }
        tokens.push(token(SqlTokenKind::Symbol, &chars[start..index]));
    }

    tokens
}

fn token(kind: SqlTokenKind, chars: &[char]) -> SqlToken {
    SqlToken {
        kind,
        text: chars.iter().collect(),
    }
}

fn is_identifier_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_identifier_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '$'
}

fn is_keyword(value: &str) -> bool {
    matches!(
        value.to_ascii_uppercase().as_str(),
        "SELECT"
            | "FROM"
            | "WHERE"
            | "INSERT"
            | "INTO"
            | "VALUES"
            | "UPDATE"
            | "SET"
            | "DELETE"
            | "CREATE"
            | "ALTER"
            | "DROP"
            | "TABLE"
            | "VIEW"
            | "INDEX"
            | "JOIN"
            | "LEFT"
            | "RIGHT"
            | "FULL"
            | "INNER"
            | "OUTER"
            | "ON"
            | "GROUP"
            | "BY"
            | "ORDER"
            | "LIMIT"
            | "OFFSET"
            | "RETURNING"
            | "AS"
            | "AND"
            | "OR"
            | "NOT"
            | "NULL"
            | "TRUE"
            | "FALSE"
            | "IS"
            | "IN"
            | "LIKE"
            | "BETWEEN"
            | "CASE"
            | "WHEN"
            | "THEN"
            | "ELSE"
            | "END"
            | "DISTINCT"
            | "HAVING"
            | "UNION"
            | "ALL"
            | "WITH"
            | "EXPLAIN"
            | "ANALYZE"
    )
}
