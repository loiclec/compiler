use std::{
    fmt::{self, Write},
    hash,
};

use crate::diagnostics::span::{HasSpan, IndexOnlySpan, Span};

use super::utils::{Parse, ParseError};

pub const KEYWORDS: &[&'static str] = &[
    &"for",
    &"if",
    &"while",
    &"function",
    &"return",
    &"endfunction",
    &"endwhile",
    &"endif",
];

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Ident<'a> {
    inner: &'a str,
    span: Span,
}

impl HasSpan for Ident<'_> {
    fn span(&self) -> Span {
        self.span
    }
}

impl<'a> hash::Hash for Ident<'a> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}

impl<'a> std::ops::Deref for Ident<'a> {
    type Target = &'a str;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a> Parse<'a> for Ident<'a> {
    fn parse(input: &mut super::utils::Input<'a>) -> Result<Self, super::utils::ParseError> {
        input.skip_whitespace()?;
        let rec = input.start_recording();
        input
            .eat_until_or_end(|char| !char.is_alphanumeric() && char != '_')
            .and_then(|inner| {
                if inner.is_empty() {
                    Err(ParseError::UnexpectedEndOfInput)
                } else if !inner.chars().next().unwrap().is_alphabetic() {
                    Err(ParseError::InvalidIdent {
                        span: input.finish_recording(rec).into(),
                        explanation: format!(
                            "The identifier you have provided is not valid,
                            because it starts with `{}` instead of a letter!",
                            inner.chars().next().unwrap()
                        ),
                    })
                } else if KEYWORDS.contains(&inner) {
                    Err(ParseError::UnexpectedToken {
                        explanation: format!(
                            "Expected an identifier here, but `{}` is a keyword.",
                            inner
                        ),
                        span: { IndexOnlySpan::from(rec.finish_recording(input)) },
                    })
                } else {
                    Ok(Self {
                        inner,
                        span: rec.finish_recording(input),
                    })
                }
            })
    }
}

impl fmt::Display for Ident<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)?;
        f.write_char(' ')
    }
}

#[cfg(test)]
mod test_parse_valid_idents {
    use crate::parse::{
        ident::Ident,
        utils::{Input, Parse},
    };

    fn test_inner(string: &str) {
        let mut input = Input::new(string);
        let ident = Ident::parse(&mut input).expect("failed to parse");
        assert!(input.is_empty());
        assert_eq!(string, ident.inner);
    }

    #[test]
    fn regressions() {
        test_inner("x");
        test_inner("function_call");
    }

    #[test]
    fn test_invalid_idents_do_not_parse() {
        Ident::parse(&mut Input::new("////")).expect_err("this identifier should not be valid");
    }
}
