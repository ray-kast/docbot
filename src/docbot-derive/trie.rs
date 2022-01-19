use std::collections::HashMap;

use anyhow::anyhow;
use proc_macro2::{Literal, Span, TokenStream};
use quote::{quote_spanned, ToTokens};

use crate::Result;

pub struct Trie<T> {
    payloads: Box<[(String, T)]>,
    root: TrieNode,
}

pub struct TrieNodeRef<'a, T>(&'a Trie<T>, &'a TrieNode);

struct TrieNode {
    payloads: Box<[usize]>,
    children: HashMap<char, TrieNode>,
}

#[derive(Default)]
struct TrieNodeParts(Option<usize>, HashMap<char, TrieNodeParts>);

impl<T> Trie<T> {
    pub fn new<I: IntoIterator<Item = (S, T)>, S: AsRef<str>>(
        it: I,
    ) -> Result<Self, anyhow::Error> {
        fn insert_payload<I: Iterator<Item = char>>(
            parts: &mut TrieNodeParts,
            payload: usize,
            mut path: I,
            mut breadcrumb: String,
        ) -> Result<(), anyhow::Error> {
            match path.next() {
                None => match parts.0 {
                    None => {
                        parts.0 = Some(payload);
                        Ok(())
                    },
                    Some(_) => Err(anyhow!("multiple entries for identifier {:?}", breadcrumb)),
                },
                Some(c) => {
                    use std::collections::hash_map::Entry::{Occupied, Vacant};

                    let child = match parts.1.entry(c) {
                        Occupied(o) => o.into_mut(),
                        Vacant(v) => v.insert(TrieNodeParts::default()),
                    };

                    breadcrumb.push(c);
                    insert_payload(child, payload, path, breadcrumb)
                },
            }
        }

        let mut payloads = Vec::new();
        let mut root = TrieNodeParts::default();

        for (path, payload) in it {
            let id = payloads.len();
            payloads.push((path.as_ref().into(), payload));

            insert_payload(&mut root, id, path.as_ref().chars(), String::new())?;
        }

        Ok(Self {
            payloads: payloads.into_boxed_slice(),
            root: root.into(),
        })
    }

    pub fn root(&self) -> TrieNodeRef<T> { TrieNodeRef(self, &self.root) }
}

impl<'a, T> TrieNodeRef<'a, T> {
    pub fn payloads(&self) -> impl Iterator<Item = &(String, T)> {
        self.1.payloads.iter().map(move |i| &self.0.payloads[*i])
    }

    pub fn children(&'a self) -> impl ExactSizeIterator<Item = (char, TrieNodeRef<'a, T>)> {
        self.1
            .children
            .iter()
            .map(move |(k, v)| (*k, TrieNodeRef(self.0, v)))
    }

    pub fn to_lexer<
        I: ToTokens + Clone,
        O: Clone + Fn(&T) -> OR,
        OR: ToTokens,
        N: Clone + Fn() -> NR,
        NR: ToTokens,
        A: Clone + Fn(Vec<&str>) -> AR,
        AR: ToTokens,
        R: Clone + Fn(Vec<&(String, T)>) -> Option<&T>,
    >(
        &self,
        span: Span,
        iter_id: I,
        ok: O,
        no_match: N,
        ambiguous: A,
        resolve_ambiguous: R,
    ) -> TokenStream {
        let arms = self.children().map(|(chr, child)| {
            let chr = Literal::character(chr);
            let child = child.to_lexer(
                span,
                iter_id.clone(),
                ok.clone(),
                no_match.clone(),
                ambiguous.clone(),
                resolve_ambiguous.clone(),
            );

            quote_spanned! { span => #chr => #child }
        });

        let mut payloads = self.payloads();
        let eof: Box<dyn ToTokens> = match payloads.next() {
            None => Box::new(no_match()),
            Some((_, p)) => match payloads.next() {
                None => Box::new(ok(p)),
                Some(_) => {
                    let payloads = self.payloads().map(|(s, _)| &**s).collect::<Vec<_>>();

                    resolve_ambiguous(self.payloads().collect())
                        .map_or_else::<Box<dyn ToTokens>, _, _>(
                            || Box::new(ambiguous(payloads)),
                            |r| Box::new(ok(r)),
                        )
                },
            },
        };

        let no_match = no_match();

        let non_eof = if arms.len() == 0 {
            quote_spanned! { span => (_) => #no_match }
        } else {
            quote_spanned! { span =>
                (c) => match c {
                    #(#arms,)*
                    _ => #no_match,
                }
            }
        };

        quote_spanned! { span =>
            match #iter_id.next() {
                None => #eof,
                Some #non_eof,
            }
        }
    }
}

impl From<TrieNodeParts> for TrieNode {
    fn from(TrieNodeParts(payload, children): TrieNodeParts) -> Self {
        let children: HashMap<_, TrieNode> =
            children.into_iter().map(|(k, v)| (k, v.into())).collect();

        // TODO: warn when a strict prefix occurs?
        let payloads = payload.map_or_else(
            || {
                children
                    .values()
                    .flat_map(|v| v.payloads.iter())
                    .copied()
                    .collect::<Vec<_>>()
                    .into_boxed_slice()
            },
            |p| Box::new([p]),
        );

        Self { payloads, children }
    }
}
