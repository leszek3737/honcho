use std::pin::Pin;

use futures_util::Stream;

use super::runtime::block_on;

pub(crate) struct BlockingIter<S> {
    stream: Pin<Box<S>>,
}

impl<S> BlockingIter<S> {
    pub(crate) fn new(stream: S) -> Self
    where
        S: Unpin,
    {
        Self {
            stream: Box::pin(stream),
        }
    }
}

impl<S> Iterator for BlockingIter<S>
where
    S: Stream + Unpin,
{
    type Item = S::Item;

    fn next(&mut self) -> Option<Self::Item> {
        block_on(futures_util::StreamExt::next(&mut self.stream))
    }
}

pub(crate) async fn collect_all_pages<T: Clone + Send + 'static>(
    first_page: crate::types::pagination::Page<T>,
) -> Vec<T> {
    let mut all = first_page.items();
    let mut current = first_page;
    while let Some(next) = current.next_page().await {
        all.extend(next.items());
        current = next;
    }
    all
}
