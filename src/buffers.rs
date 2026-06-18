/// Fixed-capacity char buffer (replaces arrayvec dependency).
#[derive(Clone)]
pub struct CharVec<const N: usize> {
    data: [char; N],
    len: usize,
}

impl<const N: usize> Default for CharVec<N> {
    fn default() -> Self { Self::new() }
}

impl<const N: usize> CharVec<N> {
    #[inline]
    pub const fn new() -> Self {
        Self { data: ['\0'; N], len: 0 }
    }

    #[inline]
    pub fn len(&self) -> usize { self.len }

    #[inline]
    pub fn is_empty(&self) -> bool { self.len == 0 }

    #[inline]
    pub fn is_full(&self) -> bool { self.len >= N }

    #[inline]
    pub fn clear(&mut self) { self.len = 0; }

    #[inline]
    pub fn try_push(&mut self, ch: char) -> bool {
        if self.len < N {
            self.data[self.len] = ch;
            self.len += 1;
            true
        } else {
            false
        }
    }

    #[inline]
    pub fn pop(&mut self) -> Option<char> {
        if self.len > 0 {
            self.len -= 1;
            Some(self.data[self.len])
        } else {
            None
        }
    }

    #[inline]
    pub fn truncate(&mut self, new_len: usize) {
        if new_len < self.len {
            self.len = new_len;
        }
    }

    #[inline]
    pub fn swap(&mut self, a: usize, b: usize) {
        self.data.swap(a, b);
    }

    #[inline]
    pub fn as_slice(&self) -> &[char] {
        &self.data[..self.len]
    }

    #[inline]
    pub fn iter(&self) -> core::slice::Iter<'_, char> {
        self.as_slice().iter()
    }
}

impl<const N: usize> core::ops::Deref for CharVec<N> {
    type Target = [char];
    #[inline]
    fn deref(&self) -> &[char] { self.as_slice() }
}

impl<const N: usize> core::iter::FromIterator<char> for CharVec<N> {
    fn from_iter<I: IntoIterator<Item = char>>(iter: I) -> Self {
        let mut v = Self::new();
        for ch in iter {
            if !v.try_push(ch) { break; }
        }
        v
    }
}

#[cfg(feature = "heapless")]
pub type RawBuffer = heapless::String<32>;

#[cfg(feature = "heapless")]
pub type OutBuffer = heapless::String<128>;

#[cfg(not(feature = "heapless"))]
pub type RawBuffer = String;

#[cfg(not(feature = "heapless"))]
pub type OutBuffer = String;

#[cfg(all(not(feature = "std"), not(feature = "heapless")))]
compile_error!(
    "no_std build requires `heapless` feature (use --no-default-features --features heapless)"
);

#[cfg(feature = "heapless")]
#[inline(always)]
pub fn new_raw_buffer() -> RawBuffer {
    RawBuffer::new()
}

#[cfg(feature = "heapless")]
#[inline(always)]
pub fn new_out_buffer() -> OutBuffer {
    OutBuffer::new()
}

#[cfg(not(feature = "heapless"))]
#[inline(always)]
pub fn new_raw_buffer() -> RawBuffer {
    String::with_capacity(32)
}

#[cfg(not(feature = "heapless"))]
#[inline(always)]
pub fn new_out_buffer() -> OutBuffer {
    String::with_capacity(128)
}
