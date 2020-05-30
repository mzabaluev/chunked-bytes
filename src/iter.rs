use bytes::Bytes;

use std::collections::vec_deque;
use std::iter::FusedIterator;

pub struct DrainChunks<'a> {
    inner: vec_deque::Drain<'a, Bytes>,
}

impl<'a> DrainChunks<'a> {
    #[inline]
    pub(crate) fn new(inner: vec_deque::Drain<'a, Bytes>) -> Self {
        DrainChunks { inner }
    }
}

impl<'a> Iterator for DrainChunks<'a> {
    type Item = Bytes;

    #[inline]
    fn next(&mut self) -> Option<Bytes> {
        self.inner.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a> ExactSizeIterator for DrainChunks<'a> {}
impl<'a> FusedIterator for DrainChunks<'a> {}

pub struct IntoChunks {
    inner: vec_deque::IntoIter<Bytes>,
}

impl IntoChunks {
    #[inline]
    pub(crate) fn new(inner: vec_deque::IntoIter<Bytes>) -> Self {
        IntoChunks { inner }
    }
}

impl Iterator for IntoChunks {
    type Item = Bytes;

    #[inline]
    fn next(&mut self) -> Option<Bytes> {
        self.inner.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl ExactSizeIterator for IntoChunks {}
impl FusedIterator for IntoChunks {}
