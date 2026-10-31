//! A set of small numbers, one bit each: the places of an output, or of what some check reads. Sets that are
//! compared are made for the same count of numbers, so they have the same length.

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub(crate) struct Bits(Vec<u64>);

impl Bits {
    /// An empty set for numbers below `n`.
    pub fn new(n: usize) -> Bits {
        Bits(vec![0; n.div_ceil(64)])
    }

    pub fn set(&mut self, i: usize) {
        let w = i / 64;
        if w >= self.0.len() {
            self.0.resize(w + 1, 0);
        }
        self.0[w] |= 1u64 << (i % 64);
    }

    pub fn has(&self, i: usize) -> bool {
        self.0.get(i / 64).is_some_and(|w| *w & (1u64 << (i % 64)) != 0)
    }

    pub fn any(&self) -> bool {
        self.0.iter().any(|w| *w != 0)
    }

    pub fn or(&mut self, o: &Bits) {
        if o.0.len() > self.0.len() {
            self.0.resize(o.0.len(), 0);
        }
        for (a, b) in self.0.iter_mut().zip(&o.0) {
            *a |= *b;
        }
    }

    pub fn and(&mut self, o: &Bits) {
        for (i, a) in self.0.iter_mut().enumerate() {
            *a &= o.0.get(i).copied().unwrap_or(0);
        }
    }

    pub fn and_not(&mut self, o: &Bits) {
        for (a, b) in self.0.iter_mut().zip(&o.0) {
            *a &= !*b;
        }
    }

    pub fn meets(&self, o: &Bits) -> bool {
        self.0.iter().zip(&o.0).any(|(a, b)| *a & *b != 0)
    }

    /// Every number of the set is in `o`.
    pub fn within(&self, o: &Bits) -> bool {
        self.0
            .iter()
            .enumerate()
            .all(|(i, a)| *a & !o.0.get(i).copied().unwrap_or(0) == 0)
    }

    pub fn ones(&self) -> impl Iterator<Item = usize> {
        self.0.iter().enumerate().flat_map(|(w, x)| {
            let x = *x;
            (0..64usize)
                .filter(move |b| x & (1u64 << *b) != 0)
                .map(move |b| w * 64 + b)
        })
    }
}
