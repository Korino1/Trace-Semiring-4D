//! Модульная версия TS4 (коэффициенты по модулю m).
//!
//! Быстрое использование:
//! ```
//! use ts4::{Block, Trace, TS4Mod};
//! let t = Trace::new(vec![Block::new(1,0,0)]);
//! let a = TS4Mod::from_trace(t, 3, 5);
//! ```

use crate::divisibility::left_gcd_trace;
use crate::trace::Trace;
use std::collections::BTreeMap;

#[inline]
fn assert_valid_modulus(modulus: u32, context: &str) {
    assert!(modulus >= 1, "{context} requires modulus >= 1");
}

#[inline]
fn assert_matching_modulus(lhs: u32, rhs: u32, context: &str) {
    assert_valid_modulus(lhs, context);
    assert_valid_modulus(rhs, context);
    assert!(lhs == rhs, "{context} requires identical moduli");
}

#[inline]
fn reduce_mod_u64(value: u64, modulus: u32) -> u32 {
    (value % modulus as u64) as u32
}

/// Модульная версия TS4 с коэффициентами mod m.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TS4Mod {
    pub(crate) modulus: u32,
    pub(crate) terms: BTreeMap<Trace, u32>,
}

impl TS4Mod {
    #[inline]
    pub fn new(modulus: u32) -> Self {
        assert_valid_modulus(modulus, "TS4Mod::new");
        Self {
            modulus,
            terms: BTreeMap::new(),
        }
    }

    #[inline]
    pub fn from_trace(t: Trace, coeff: u32, modulus: u32) -> Self {
        assert_valid_modulus(modulus, "TS4Mod::from_trace");
        let mut terms = BTreeMap::new();
        let c = reduce_mod_u64(coeff as u64, modulus);
        if c != 0 {
            terms.insert(t, c);
        }
        Self { modulus, terms }
    }

    /// Модуль коэффициентов.
    #[inline]
    pub fn modulus(&self) -> u32 {
        self.modulus
    }

    /// Число ненулевых членов.
    #[inline]
    pub fn term_count(&self) -> usize {
        self.terms.values().filter(|&&c| c != 0).count()
    }

    /// Сумма ненулевых коэффициентов в текущем представлении mod m.
    #[inline]
    pub fn coeff_sum(&self) -> u32 {
        self.terms
            .values()
            .filter(|&&c| c != 0)
            .fold(0u32, |acc, &c| reduce_mod_u64(acc as u64 + c as u64, self.modulus))
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
    pub(crate) fn from_raw_terms_unchecked(modulus: u32, terms: BTreeMap<Trace, u32>) -> Self {
        assert_valid_modulus(modulus, "TS4Mod::from_raw_terms_unchecked");
        Self { modulus, terms }
    }

    #[inline]
    pub fn add(&self, other: &TS4Mod) -> TS4Mod {
        assert_matching_modulus(self.modulus, other.modulus, "TS4Mod::add");
        let mut out = self.terms.clone();
        out.retain(|_, v| *v != 0);
        for (t, c) in other.terms.iter().filter(|(_, c)| **c != 0) {
            let next = reduce_mod_u64(
                out.get(t).copied().unwrap_or(0) as u64 + *c as u64,
                self.modulus,
            );
            if next == 0 {
                out.remove(t);
            } else {
                out.insert(t.clone(), next);
            }
        }
        TS4Mod {
            modulus: self.modulus,
            terms: out,
        }
    }

    #[inline]
    pub fn compose(&self, other: &TS4Mod) -> TS4Mod {
        assert_matching_modulus(self.modulus, other.modulus, "TS4Mod::compose");
        let self_nonzero = self.terms.values().filter(|&&c| c != 0).count();
        let other_nonzero = other.terms.values().filter(|&&c| c != 0).count();
        if self_nonzero == 0 || other_nonzero == 0 {
            return TS4Mod::new(self.modulus);
        }
        let mut out = BTreeMap::new();
        for (t1, c1) in self.terms.iter().filter(|(_, c)| **c != 0) {
            for (t2, c2) in other.terms.iter().filter(|(_, c)| **c != 0) {
                let t = t1.compose(t2);
                let prod = (*c1 as u64 * *c2 as u64) % self.modulus as u64;
                let next = reduce_mod_u64(
                    out.get(&t).copied().unwrap_or(0) as u64 + prod,
                    self.modulus,
                );
                if next == 0 {
                    out.remove(&t);
                } else {
                    out.insert(t, next);
                }
            }
        }
        TS4Mod {
            modulus: self.modulus,
            terms: out,
        }
    }
}

/// НОД коэффициентов (целочисленный).
pub fn gcd_u32(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a
}

/// НОД мономов (левый НОД по трассам + gcd коэффициентов).
pub fn gcd_monomial(a_coeff: u32, a: &Trace, b_coeff: u32, b: &Trace) -> (u32, Trace) {
    let g = gcd_u32(a_coeff, b_coeff);
    let t = left_gcd_trace(a, b);
    (g, t)
}
