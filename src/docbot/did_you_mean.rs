use std::collections::BinaryHeap;

#[derive(Debug, Clone, Copy, PartialEq)]
struct DidYouMean<S: AsRef<str>>(f64, S);

use std::cmp::Ordering;

impl<S: Eq + AsRef<str>> Eq for DidYouMean<S> {}
impl<S: PartialOrd + AsRef<str>> PartialOrd for DidYouMean<S> {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        self.0
            .partial_cmp(&rhs.0)
            .map(|o| o.then_with(|| rhs.1.partial_cmp(&self.1).unwrap_or(Ordering::Equal)))
    }
}

impl<S: Ord + AsRef<str>> Ord for DidYouMean<S> {
    fn cmp(&self, rhs: &Self) -> Ordering { self.partial_cmp(rhs).unwrap() }
}

/// Rank a list of options by their similarity to the given input.  Contains
/// some basic heuristics tailored towards the [`Docbot`](crate::Docbot) parser.
pub fn did_you_mean<S: Ord + AsRef<str>>(
    given: impl AsRef<str>,
    options: impl IntoIterator<Item = S>,
) -> impl Iterator<Item = S> {
    let given = given.as_ref();

    let mut heap = options
        .into_iter()
        .map(|opt| {
            let opt_str = opt.as_ref();
            DidYouMean(
                strsim::normalized_damerau_levenshtein(
                    given,
                    &opt_str[0..opt_str.len().min(given.len() + 1)],
                ),
                opt,
            )
        })
        .collect::<BinaryHeap<_>>();

    std::iter::from_fn(move || heap.pop())
        .take_while(|DidYouMean(s, _)| *s >= 0.3)
        .map(|DidYouMean(_, o)| o)
}
