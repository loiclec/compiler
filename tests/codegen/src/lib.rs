use proc_macro::{TokenStream, TokenTree};

#[proc_macro]
pub fn regressions(input: TokenStream) -> TokenStream {
    let mut iter = input.into_iter();
    let func_ident = iter.next().expect("expected the token `function` to start");

    let func_ident = match func_ident {
        TokenTree::Ident(ident) => ident.to_string(),
        _ => panic!(),
    };

    iter.next().unwrap();

    let mut literals = vec![];

    match iter.next().unwrap() {
        TokenTree::Group(group) => {
            for tree in group.stream() {
                match tree {
                    TokenTree::Punct(p) if p.as_char() == ',' => continue,
                    _ => literals.push(tree),
                }
            }
        }
        _ => panic!("Expected a literal here"),
    };

    literals
        .into_iter()
        .enumerate()
        .map(|(index, lit)| {
            format!(
                "
        #[test]
        fn regression_{}() {{
            {}({});
        }}
        ",
                index, func_ident, lit
            )
        })
        .collect::<Vec<_>>()
        .join("")
        .parse()
        .unwrap()
}
