//! Utilities for parsing.

#[cfg(test)]
mod test;

use core::fmt;
use std::fmt::Write;

use crate::diagnostics::{position::Position, span::Span};

pub trait Parse<'a>: Sized {
    fn parse(input: &mut Input<'a>) -> Result<Self, ParseError<'a>>;
}

// todo: convert this into a `Diagnostic` (also create that struct as well)
#[derive(Debug)]
pub enum ParseError<'a> {
    UnexpectedToken {
        token: &'a str,
        explanation: String,
    },
    UnexpectedEndOfInput,
    /// An assumption the parser makes turns out not to be correct.
    InternalError,
    __NonExhaustive,
}

#[derive(Copy, Clone, Debug)]
pub struct Input<'a> {
    inner: &'a str,
    pub(crate) indent: usize,
    position: Position,
}

impl<'a> std::ops::Deref for Input<'a> {
    type Target = &'a str;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a> Input<'a> {
    pub fn new(inner: &'a str) -> Self {
        Self {
            inner,
            indent: 0,
            position: Default::default(),
        }
    }

    pub fn parse_token(&mut self, token: &str) -> Result<&'a str, ParseError<'a>> {
        let peek = self.peek_n(token.len()).ok_or_else(|| {
            println!("failed");
            ParseError::UnexpectedEndOfInput
        })?;
        let ret = if peek == token {
            self.advance_n(token.len())?;
            println!("advanced");
            Ok(peek)
        } else {
            Err(ParseError::UnexpectedToken {
                token: peek,
                explanation: format!(
                    "Expected `{}` in this position, however, instead there was `{}`",
                    token, peek
                ),
            })
        };
        ret
    }

    /// Peek the next character
    pub fn peek_char(&self) -> Option<char> {
        self.inner.chars().next()
    }

    /// Peek `n` characters.
    pub fn peek_n(&self, n: usize) -> Option<&'a str> {
        self.inner
            .char_indices()
            .nth(n - 1)
            .map(|(index, _)| self.inner.get(..=index))
            .flatten()
    }

    /// Attempts to advance the stream by one character.
    pub fn advance_one(&mut self) -> Result<&'a str, ParseError<'a>> {
        self.advance_n(1)
    }

    /// Advance n characters
    pub fn advance_n(&mut self, n: usize) -> Result<&'a str, ParseError<'a>> {
        let n = n - 1;
        if let Some((index, _)) = self.inner.char_indices().nth(n) {
            if let (Some(ret), slice) = (self.inner.get(..=index), self.inner.get(index + 1..)) {
                self.inner = slice.unwrap_or("");
                self.position = ret.chars().fold(self.position, |mut position, char| {
                    if char == '\n' {
                        position.line += 1;
                    } else {
                        position.column += 1;
                    };
                    position
                });
                Ok(ret)
            } else {
                Err(ParseError::UnexpectedEndOfInput)
            }
        } else {
            Err(ParseError::UnexpectedEndOfInput)
        }
    }

    pub fn peek_nth(&self, n: usize) -> Option<char> {
        self.inner.chars().nth(n)
    }

    fn eat_until_inner(
        &mut self,
        stop_eating_if_true: impl Fn(char) -> bool,
        should_error_if_reaches_end: bool,
    ) -> Result<&'a str, ParseError<'a>> {
        let mut n = 0;

        loop {
            let should_stop = if let Some(next) = self.peek_nth(n) {
                (stop_eating_if_true)(next)
            } else if should_error_if_reaches_end {
                return Err(ParseError::UnexpectedEndOfInput);
            } else {
                true
            };

            if should_stop {
                break;
            } else {
                n += 1;
            }
        }

        if n == 0 {
            Ok("")
        } else {
            self.advance_n(n)
        }
    }

    pub fn peek_token(&self, token: char) -> bool {
        self.peek_char().map(|next| next == token).unwrap_or(false)
    }

    /// Eats until the provided funtion `stop_when` is true. If this function reaches the end of
    /// the input, it will return an error.
    pub fn eat_until(
        &mut self,
        stop_when: impl Fn(char) -> bool,
    ) -> Result<&'a str, ParseError<'a>> {
        self.eat_until_inner(stop_when, true)
    }

    /// Eats until the provided function `stop_when` is true. If this function reaches the end of
    /// the input, it will not error.
    pub fn eat_until_or_end(
        &mut self,
        stop_when: impl Fn(char) -> bool,
    ) -> Result<&'a str, ParseError<'a>> {
        self.eat_until_inner(stop_when, false)
    }

    pub fn start_recording(&self) -> IncompleteSpan {
        IncompleteSpan {
            start: self.position,
        }
    }

    pub fn finish_recording(&self, incomplete: IncompleteSpan) -> Span {
        Span::new(incomplete.start, self.position)
    }

    /// Get a reference to the input's position.
    pub fn position(&self) -> &Position {
        &self.position
    }

    pub fn skip_whitespace(&mut self) -> Result<(), ParseError<'a>> {
        self.eat_until_or_end(|input| !input.is_whitespace() || input == '\n')
            .map(drop)
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn peek_line(&self) -> Option<&'a str> {
        let mut pos = 0;
        loop {
            if let Some(character) = self.peek_nth(pos) {
                if character == '\n' {
                    return self.inner.get(..pos);
                } else {
                    pos += 1;
                }
            } else {
                return self.inner.get(..pos);
            }
        }
    }

    /// Set the input's indentation.
    pub fn set_indent(&mut self, indent: usize) {
        self.indent = indent;
    }

    pub fn increment_indent(&mut self, by: usize) {
        self.indent += by;
    }

    pub fn decrement_indent(&mut self, by: usize) {
        self.indent -= by;
    }

    pub fn count_indent(&self) -> Result<usize, ParseError<'a>> {
        let mut iter = self.chars();
        let mut total = 0;

        loop {
            match iter.next() {
                Some(' ' | '\u{C}' | '\u{B}') => total += 1,
                Some('\t') => total += 4,
                _ => break,
            }
        }

        Ok(total)
    }

    pub fn advance_indent(&mut self) -> Result<(), ParseError<'a>> {
        let mut whitespace_units = 0;
        loop {
            match self.inner.chars().next() {
                Some(char) => match char {
                    ' ' | '\u{C}' | '\u{B}' => {
                        whitespace_units += 1;
                        self.advance_one()?;
                    }
                    '\t' => {
                        whitespace_units += 4;
                        self.advance_one()?;
                    }
                    _ => break,
                },
                None => {
                    break;
                }
            }
        }
        if whitespace_units == self.indent {
            Ok(())
        } else {
            return Err(ParseError::__NonExhaustive);
        }
    }
}

pub struct IncompleteSpan {
    start: Position,
}

impl IncompleteSpan {
    pub fn finish_recording(self, input: &Input) -> Span {
        input.finish_recording(self)
    }
}

pub(crate) fn write_indentation(units: usize, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for _ in 0..units {
        f.write_char(' ')?;
    }

    Ok(())
}
