#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl Diagnostic {
    fn new(message: impl Into<String>, line: usize, column: usize) -> Self {
        Self {
            message: message.into(),
            line,
            column,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub channels: Vec<ChannelDecl>,
    pub tasks: Vec<TaskDecl>,
    pub run_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelDecl {
    pub name: String,
    pub capacity: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskDecl {
    pub name: String,
    pub priority: u8,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    StringLiteral(String),
    Variable(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    Print(Expr),
    Yield,
    Sleep(u64),
    Send { channel: String, expr: Expr },
    Recv { channel: String, var: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TokenKind {
    Ident(String),
    Number(u64),
    StringLiteral(String),
    LBrace,
    RBrace,
    Comma,
    Semicolon,
    KeywordChannel,
    KeywordCapacity,
    KeywordTask,
    KeywordPriority,
    KeywordPrint,
    KeywordYield,
    KeywordSleep,
    KeywordSend,
    KeywordRecv,
    KeywordAs,
    KeywordRun,
    Eof,
}

#[derive(Debug, Clone)]
struct Token {
    kind: TokenKind,
    line: usize,
    column: usize,
}

struct Lexer<'a> {
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    src: &'a str,
    line: usize,
    column: usize,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            chars: src.char_indices().peekable(),
            src,
            line: 1,
            column: 1,
        }
    }

    fn bump_char(&mut self) -> Option<(usize, char)> {
        let next = self.chars.next()?;
        if next.1 == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(next)
    }

    fn peek_char(&mut self) -> Option<char> {
        self.chars.peek().map(|(_, ch)| *ch)
    }

    fn lex_all(mut self) -> Result<Vec<Token>, Diagnostic> {
        let mut tokens = Vec::new();
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() {
                self.bump_char();
                continue;
            }

            let line = self.line;
            let column = self.column;

            match ch {
                '{' => {
                    self.bump_char();
                    tokens.push(Token {
                        kind: TokenKind::LBrace,
                        line,
                        column,
                    });
                }
                '}' => {
                    self.bump_char();
                    tokens.push(Token {
                        kind: TokenKind::RBrace,
                        line,
                        column,
                    });
                }
                ',' => {
                    self.bump_char();
                    tokens.push(Token {
                        kind: TokenKind::Comma,
                        line,
                        column,
                    });
                }
                ';' => {
                    self.bump_char();
                    tokens.push(Token {
                        kind: TokenKind::Semicolon,
                        line,
                        column,
                    });
                }
                '"' => {
                    self.bump_char();
                    let mut s = String::new();
                    let mut terminated = false;
                    while let Some(c) = self.peek_char() {
                        if c == '"' {
                            self.bump_char();
                            terminated = true;
                            break;
                        }
                        if c == '\n' {
                            return Err(Diagnostic::new(
                                "unterminated string literal",
                                line,
                                column,
                            ));
                        }
                        self.bump_char();
                        s.push(c);
                    }
                    if !terminated {
                        return Err(Diagnostic::new("unterminated string literal", line, column));
                    }
                    tokens.push(Token {
                        kind: TokenKind::StringLiteral(s),
                        line,
                        column,
                    });
                }
                '#' => {
                    while let Some(c) = self.peek_char() {
                        self.bump_char();
                        if c == '\n' {
                            break;
                        }
                    }
                }
                c if c.is_ascii_digit() => {
                    let start = self
                        .chars
                        .peek()
                        .map(|(idx, _)| *idx)
                        .expect("digit start exists");
                    while let Some(c2) = self.peek_char() {
                        if c2.is_ascii_digit() {
                            self.bump_char();
                        } else {
                            break;
                        }
                    }
                    let end = self
                        .chars
                        .peek()
                        .map(|(idx, _)| *idx)
                        .unwrap_or(self.src.len());
                    let raw = &self.src[start..end];
                    let value = raw.parse::<u64>().map_err(|_| {
                        Diagnostic::new(format!("invalid number literal '{raw}'"), line, column)
                    })?;
                    tokens.push(Token {
                        kind: TokenKind::Number(value),
                        line,
                        column,
                    });
                }
                c if c.is_ascii_alphabetic() || c == '_' => {
                    let start = self
                        .chars
                        .peek()
                        .map(|(idx, _)| *idx)
                        .expect("identifier start exists");
                    while let Some(c2) = self.peek_char() {
                        if c2.is_ascii_alphanumeric() || c2 == '_' {
                            self.bump_char();
                        } else {
                            break;
                        }
                    }
                    let end = self
                        .chars
                        .peek()
                        .map(|(idx, _)| *idx)
                        .unwrap_or(self.src.len());
                    let raw = &self.src[start..end];
                    let kind = match raw {
                        "channel" => TokenKind::KeywordChannel,
                        "capacity" => TokenKind::KeywordCapacity,
                        "task" => TokenKind::KeywordTask,
                        "priority" => TokenKind::KeywordPriority,
                        "print" => TokenKind::KeywordPrint,
                        "yield" => TokenKind::KeywordYield,
                        "sleep" => TokenKind::KeywordSleep,
                        "send" => TokenKind::KeywordSend,
                        "recv" => TokenKind::KeywordRecv,
                        "as" => TokenKind::KeywordAs,
                        "run" => TokenKind::KeywordRun,
                        _ => TokenKind::Ident(raw.to_owned()),
                    };
                    tokens.push(Token { kind, line, column });
                }
                other => {
                    return Err(Diagnostic::new(
                        format!("unexpected character '{other}'"),
                        line,
                        column,
                    ));
                }
            }
        }
        tokens.push(Token {
            kind: TokenKind::Eof,
            line: self.line,
            column: self.column,
        });
        Ok(tokens)
    }
}

struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, cursor: 0 }
    }

    fn current(&self) -> &Token {
        &self.tokens[self.cursor]
    }

    fn bump(&mut self) {
        if self.cursor + 1 < self.tokens.len() {
            self.cursor += 1;
        }
    }

    fn parse_ident(&mut self, context: &str) -> Result<String, Diagnostic> {
        match &self.current().kind {
            TokenKind::Ident(name) => {
                let out = name.clone();
                self.bump();
                Ok(out)
            }
            _ => Err(Diagnostic::new(
                format!("expected identifier for {context}"),
                self.current().line,
                self.current().column,
            )),
        }
    }

    fn parse_number(&mut self, context: &str) -> Result<u64, Diagnostic> {
        match self.current().kind {
            TokenKind::Number(n) => {
                self.bump();
                Ok(n)
            }
            _ => Err(Diagnostic::new(
                format!("expected number for {context}"),
                self.current().line,
                self.current().column,
            )),
        }
    }

    fn expect(&mut self, expected: TokenKind, msg: &str) -> Result<(), Diagnostic> {
        if std::mem::discriminant(&self.current().kind) == std::mem::discriminant(&expected) {
            self.bump();
            Ok(())
        } else {
            Err(Diagnostic::new(
                msg,
                self.current().line,
                self.current().column,
            ))
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, Diagnostic> {
        match &self.current().kind {
            TokenKind::StringLiteral(v) => {
                let out = Expr::StringLiteral(v.clone());
                self.bump();
                Ok(out)
            }
            TokenKind::Ident(v) => {
                let out = Expr::Variable(v.clone());
                self.bump();
                Ok(out)
            }
            _ => Err(Diagnostic::new(
                "expected expression (string literal or variable)",
                self.current().line,
                self.current().column,
            )),
        }
    }

    fn parse_statement(&mut self) -> Result<Statement, Diagnostic> {
        match self.current().kind.clone() {
            TokenKind::KeywordPrint => {
                self.bump();
                let expr = self.parse_expr()?;
                self.expect(TokenKind::Semicolon, "expected ';' after print statement")?;
                Ok(Statement::Print(expr))
            }
            TokenKind::KeywordYield => {
                self.bump();
                self.expect(TokenKind::Semicolon, "expected ';' after yield")?;
                Ok(Statement::Yield)
            }
            TokenKind::KeywordSleep => {
                self.bump();
                let ticks = self.parse_number("sleep ticks")?;
                self.expect(TokenKind::Semicolon, "expected ';' after sleep")?;
                Ok(Statement::Sleep(ticks))
            }
            TokenKind::KeywordSend => {
                self.bump();
                let channel = self.parse_ident("send channel")?;
                let expr = self.parse_expr()?;
                self.expect(TokenKind::Semicolon, "expected ';' after send")?;
                Ok(Statement::Send { channel, expr })
            }
            TokenKind::KeywordRecv => {
                self.bump();
                let channel = self.parse_ident("recv channel")?;
                self.expect(TokenKind::KeywordAs, "expected 'as' in recv statement")?;
                let var = self.parse_ident("recv variable")?;
                self.expect(TokenKind::Semicolon, "expected ';' after recv")?;
                Ok(Statement::Recv { channel, var })
            }
            _ => Err(Diagnostic::new(
                "unknown statement",
                self.current().line,
                self.current().column,
            )),
        }
    }

    fn parse_channel_decl(&mut self) -> Result<ChannelDecl, Diagnostic> {
        self.expect(TokenKind::KeywordChannel, "expected 'channel'")?;
        let name = self.parse_ident("channel name")?;
        self.expect(
            TokenKind::KeywordCapacity,
            "expected 'capacity' in channel declaration",
        )?;
        let capacity = self.parse_number("channel capacity")? as usize;
        if capacity == 0 {
            return Err(Diagnostic::new(
                "channel capacity must be greater than zero",
                self.current().line,
                self.current().column,
            ));
        }
        self.expect(
            TokenKind::Semicolon,
            "expected ';' after channel declaration",
        )?;
        Ok(ChannelDecl { name, capacity })
    }

    fn parse_task_decl(&mut self) -> Result<TaskDecl, Diagnostic> {
        self.expect(TokenKind::KeywordTask, "expected 'task'")?;
        let name = self.parse_ident("task name")?;
        self.expect(
            TokenKind::KeywordPriority,
            "expected 'priority' in task declaration",
        )?;
        let priority = self.parse_number("task priority")?;
        if priority > u8::MAX as u64 {
            return Err(Diagnostic::new(
                "task priority must be between 0 and 255",
                self.current().line,
                self.current().column,
            ));
        }
        self.expect(TokenKind::LBrace, "expected '{' to start task body")?;
        let mut statements = Vec::new();
        while !matches!(self.current().kind, TokenKind::RBrace | TokenKind::Eof) {
            statements.push(self.parse_statement()?);
        }
        self.expect(TokenKind::RBrace, "expected '}' to close task body")?;
        Ok(TaskDecl {
            name,
            priority: priority as u8,
            statements,
        })
    }

    fn parse_run_decl(&mut self) -> Result<Vec<String>, Diagnostic> {
        self.expect(TokenKind::KeywordRun, "expected 'run' clause")?;
        let mut items = Vec::new();
        items.push(self.parse_ident("run task name")?);
        while matches!(self.current().kind, TokenKind::Comma) {
            self.bump();
            items.push(self.parse_ident("run task name")?);
        }
        self.expect(TokenKind::Semicolon, "expected ';' after run clause")?;
        Ok(items)
    }

    fn parse_program(mut self) -> Result<Program, Diagnostic> {
        let mut channels = Vec::new();
        let mut tasks = Vec::new();

        while matches!(
            self.current().kind,
            TokenKind::KeywordChannel | TokenKind::KeywordTask
        ) {
            if matches!(self.current().kind, TokenKind::KeywordChannel) {
                channels.push(self.parse_channel_decl()?);
            } else {
                tasks.push(self.parse_task_decl()?);
            }
        }

        let run_order = self.parse_run_decl()?;
        if !matches!(self.current().kind, TokenKind::Eof) {
            return Err(Diagnostic::new(
                "unexpected tokens after run clause",
                self.current().line,
                self.current().column,
            ));
        }
        Ok(Program {
            channels,
            tasks,
            run_order,
        })
    }
}

pub fn parse_program(src: &str) -> Result<Program, Vec<Diagnostic>> {
    let tokens = Lexer::new(src).lex_all().map_err(|e| vec![e])?;
    Parser::new(tokens).parse_program().map_err(|e| vec![e])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_program() {
        let src = r#"
channel bus capacity 2;
task a priority 1 {
  print "hi";
  yield;
}
run a;
"#;
        let program = parse_program(src).expect("should parse");
        assert_eq!(program.channels.len(), 1);
        assert_eq!(program.tasks.len(), 1);
        assert_eq!(program.run_order, vec!["a"]);
    }

    #[test]
    fn reports_location_on_error() {
        let src = "task t priority 1 { print \"x\" } run t;";
        let err = parse_program(src).expect_err("should fail");
        assert!(err[0].message.contains("expected ';'"));
        assert!(err[0].line >= 1);
        assert!(err[0].column >= 1);
    }
}
