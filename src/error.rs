use std::fmt::Display;

use proc_macro2::{Span, TokenStream};
use syn::{Error};

#[derive(Default)]
pub struct ErrorStash(Option<Error>);


impl ErrorStash {
    pub fn extend(&mut self, span: Span, error: impl Display) {
        if let Some(err) = self.0.as_mut() {
            err.combine(Error::new(span, error));
        } else {
            self.0 = Some(Error::new(span, error));
        }
    }

    pub fn map<F>(&self, f: F) -> Option<TokenStream> where F: FnOnce(&Error) -> TokenStream {
        self.0.as_ref().map(f)
    }
}
