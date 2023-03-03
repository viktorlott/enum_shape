use proc_macro::TokenStream;
use syn::{parse_macro_input};

use attribute::{EnumShape};
use subject::Subject;
use shape::Shape;

mod attribute;
mod utils;
mod subject;
mod shape;
mod error;


#[proc_macro_attribute]
pub fn shape(attr: TokenStream, input: TokenStream) -> TokenStream {
    let shape = parse_macro_input!(attr as Shape);
    let input =  parse_macro_input!(input as Subject);

    // Loop through enum definition and match each variant with each shape pattern.
    // for each variant => pattern.find(variant)
    EnumShape::new(shape, input).matcher().unwrap_or_error()
}
