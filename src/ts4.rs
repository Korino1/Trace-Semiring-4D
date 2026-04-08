//! TS4: формальные суммы трасс с коэффициентами.
//!
//! Быстрое использование:
//! ```
//! use ts4::{Block, Trace, TS4};
//! let t1 = Trace::new(vec![Block::new(1,0,0)]);
//! let t2 = Trace::new(vec![Block::new(0,1,0)]);
//! let a = TS4::from_trace(t1, 3);
//! let b = TS4::from_trace(t2, 2);
//! let c = a.compose(&b);
//! assert_eq!(c.coeff_sum(), 6);
//! ```

use crate::trace::Trace;
use std::collections::BTreeMap;

#[inline]
fn checked_add_u32(lhs: u32, rhs: u32, context: &str) -> u32 {
    match lhs.checked_add(rhs) {
        Some(value) => value,
        None => panic!("{context} coefficient overflow"),
    }
}

#[inline]
fn checked_mul_u32(lhs: u32, rhs: u32, context: &str) -> u32 {
    match lhs.checked_mul(rhs) {
        Some(value) => value,
        None => panic!("{context} coefficient overflow"),
    }
}

/// TS4: конечные формальные суммы трасс с коэффициентами в ℕ.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TS4 {
    /// Map from trace to coefficient (ℕ).
    pub(crate) terms: BTreeMap<Trace, u32>,
}

impl TS4 {
    /// Нулевой элемент (пустая сумма).
    #[inline]
    pub fn zero() -> Self {
        Self {
            terms: BTreeMap::new(),
        }
    }

    /// Единица (пустая трасса с коэффициентом 1).
    #[inline]
    pub fn one() -> Self {
        let mut terms = BTreeMap::new();
        terms.insert(Trace::empty(), 1);
        Self { terms }
    }

    /// Создать TS4 из одной трассы с коэффициентом.
    #[inline]
    pub fn from_trace(t: Trace, coeff: u32) -> Self {
        let mut terms = BTreeMap::new();
        if coeff != 0 {
            terms.insert(t, coeff);
        }
        Self { terms }
    }

    /// Число ненулевых членов.
    #[inline]
    pub fn term_count(&self) -> usize {
        self.terms.values().filter(|&&c| c != 0).count()
    }

    /// Сумма ненулевых коэффициентов.
    #[inline]
    pub fn coeff_sum(&self) -> u32 {
        self.terms
            .values()
            .filter(|&&c| c != 0)
            .fold(0u32, |acc, &c| checked_add_u32(acc, c, "TS4::coeff_sum"))
    }

    /// Коэффициент при заданной трассе, если он ненулевой.
    #[inline]
    pub fn get_coeff(&self, trace: &Trace) -> Option<u32> {
        self.terms.get(trace).copied().filter(|&c| c != 0)
    }

    /// Итератор по ненулевым членам.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&Trace, u32)> + '_ {
        self.terms
            .iter()
            .filter_map(|(trace, coeff)| (*coeff != 0).then_some((trace, *coeff)))
    }

    #[cfg(test)]
    #[inline]
    pub(crate) fn from_raw_terms_unchecked(terms: BTreeMap<Trace, u32>) -> Self {
        Self { terms }
    }

    #[inline]
    pub(crate) fn from_raw_terms_unchecked_for_theory(terms: BTreeMap<Trace, u32>) -> Self {
        Self { terms }
    }

    /// Мультисет-сумма (покомпонентное сложение коэффициентов).
    #[inline]
    pub fn add(&self, other: &TS4) -> TS4 {
        let mut out = self.terms.clone();
        out.retain(|_, v| *v != 0);
        for (t, c) in other.terms.iter().filter(|(_, c)| **c != 0) {
            let e = out.entry(t.clone()).or_insert(0);
            *e = checked_add_u32(*e, *c, "TS4::add");
            if *e == 0 {
                out.remove(t);
            }
        }
        TS4 { terms: out }
    }

    /// Каузальная композиция (свёртка по конкатенации трасс).
    ///
    /// Пример:
    /// ```
    /// use ts4::{Block, Trace, TS4};
    /// let t1 = Trace::new(vec![Block::new(1,0,0)]);
    /// let t2 = Trace::new(vec![Block::new(0,1,0)]);
    /// let a = TS4::from_trace(t1, 3);
    /// let b = TS4::from_trace(t2, 2);
    /// let c = a.compose(&b);
    /// assert_eq!(c.coeff_sum(), 6);
    /// ```
    #[inline]
    pub fn compose(&self, other: &TS4) -> TS4 {
        let left_terms: Vec<_> = self.terms.iter().filter(|(_, c)| **c != 0).collect();
        let right_terms: Vec<_> = other.terms.iter().filter(|(_, c)| **c != 0).collect();
        if left_terms.is_empty() || right_terms.is_empty() {
            return TS4::zero();
        }
        let pair_capacity = left_terms
            .len()
            .checked_mul(right_terms.len())
            .expect("TS4::compose pair capacity overflow");
        if pair_capacity <= 32 {
            let mut reduced = Vec::with_capacity(pair_capacity);
            for (t1, c1) in left_terms.iter() {
                for (t2, c2) in right_terms.iter() {
                    let product = checked_mul_u32(**c1, **c2, "TS4::compose");
                    let composed = t1.compose(t2);
                    if let Some((_, coeff)) =
                        reduced.iter_mut().find(|(trace, _)| *trace == composed)
                    {
                        *coeff = checked_add_u32(*coeff, product, "TS4::compose");
                    } else {
                        reduced.push((composed, product));
                    }
                }
            }
            reduced.sort_unstable_by(|lhs, rhs| lhs.0.cmp(&rhs.0));
            return TS4 {
                terms: reduced.into_iter().collect(),
            };
        }

        let mut out = BTreeMap::new();
        for (t1, c1) in left_terms.iter() {
            for (t2, c2) in right_terms.iter() {
                let t = t1.compose(t2);
                let e = out.entry(t).or_insert(0u32);
                let product = checked_mul_u32(**c1, **c2, "TS4::compose");
                *e = checked_add_u32(*e, product, "TS4::compose");
            }
        }
        TS4 { terms: out }
    }

    /// Нормализовать: удалить нулевые коэффициенты.
    pub fn normalize(&mut self) {
        self.terms.retain(|_, v| *v != 0);
    }
}
