//! Делимость трасс по ∘ (левое/правое деление).
//!
//! Быстрое использование:
//! ```
//! use ts4::{Block, Trace, left_divide_trace};
//! let a = Trace::new(vec![Block::new(1,0,0), Block::zero()]);
//! let b = Trace::new(vec![Block::new(1,0,0), Block::new(0,1,0)]);
//! let c = left_divide_trace(&a, &b).unwrap();
//! assert_eq!(c, Trace::new(vec![Block::new(0,1,0)]));
//! ```

use crate::trace::{append_blocks_range, Trace};
use crate::ts4::TS4;
use crate::types::Block;
use std::collections::HashMap;

#[inline]
fn checked_accumulate_coeff(slot: &mut u32, value: u32, context: &str) {
    *slot = match slot.checked_add(value) {
        Some(sum) => sum,
        None => panic!("{context} coefficient overflow"),
    };
}

#[inline]
fn nonzero_term_count(poly: &TS4) -> usize {
    poly.terms.values().filter(|&&c| c != 0).count()
}

/// Левое деление мономов: (a,t) | (b,u) если a|b и t|_L u.
pub fn left_divide_monomial(
    a_coeff: u32,
    a: &Trace,
    b_coeff: u32,
    b: &Trace,
) -> Option<(u32, Trace)> {
    if a_coeff == 0 || b_coeff == 0 || b_coeff % a_coeff != 0 {
        return None;
    }
    let c = left_divide_trace(a, b)?;
    Some((b_coeff / a_coeff, c))
}

/// Правое деление мономов: (a,t) | (b,u) если a|b и t|_R u.
pub fn right_divide_monomial(
    a_coeff: u32,
    a: &Trace,
    b_coeff: u32,
    b: &Trace,
) -> Option<(u32, Trace)> {
    if a_coeff == 0 || b_coeff == 0 || b_coeff % a_coeff != 0 {
        return None;
    }
    let c = right_divide_trace(a, b)?;
    Some((b_coeff / a_coeff, c))
}

/// Левое деление TS4 по мономному делителю (частичный алгоритм).
pub fn left_divide_ts4_monomial(a: &TS4, b: &TS4) -> Option<TS4> {
    if nonzero_term_count(a) != 1 {
        return None;
    }
    let (a_trace, a_coeff) = a.terms.iter().find(|(_, c)| **c != 0).unwrap();
    let mut out = TS4::zero();
    for (b_trace, b_coeff) in b.terms.iter().filter(|(_, c)| **c != 0) {
        let (c_coeff, c_trace) = left_divide_monomial(*a_coeff, a_trace, *b_coeff, b_trace)?;
        let e = out.terms.entry(c_trace).or_insert(0);
        checked_accumulate_coeff(e, c_coeff, "left_divide_ts4_monomial");
    }
    out.normalize();
    Some(out)
}

/// Правое деление TS4 по мономному делителю (частичный алгоритм).
pub fn right_divide_ts4_monomial(a: &TS4, b: &TS4) -> Option<TS4> {
    if nonzero_term_count(a) != 1 {
        return None;
    }
    let (a_trace, a_coeff) = a.terms.iter().find(|(_, c)| **c != 0).unwrap();
    let mut out = TS4::zero();
    for (b_trace, b_coeff) in b.terms.iter().filter(|(_, c)| **c != 0) {
        let (c_coeff, c_trace) = right_divide_monomial(*a_coeff, a_trace, *b_coeff, b_trace)?;
        let e = out.terms.entry(c_trace).or_insert(0);
        checked_accumulate_coeff(e, c_coeff, "right_divide_ts4_monomial");
    }
    out.normalize();
    Some(out)
}

/// Левое деление TS4 при уникальных разложениях (детерминированный режим).
/// Возвращает None, если для некоторой трассы в B есть 0 или >1 разложения через A.
pub fn left_divide_ts4_unique(a: &TS4, b: &TS4) -> Option<TS4> {
    if nonzero_term_count(a) == 0 {
        return None;
    }
    let mut out = TS4::zero();
    for (b_trace, b_coeff) in b.terms.iter().filter(|(_, c)| **c != 0) {
        let mut found: Option<(u32, Trace)> = None;
        for (a_trace, a_coeff) in a.terms.iter().filter(|(_, c)| **c != 0) {
            if let Some(c_trace) = left_divide_trace(a_trace, b_trace) {
                if b_coeff % a_coeff != 0 {
                    return None;
                }
                let c_coeff = b_coeff / a_coeff;
                if found.is_some() {
                    return None;
                }
                found = Some((c_coeff, c_trace));
            }
        }
        let (c_coeff, c_trace) = found?;
        let e = out.terms.entry(c_trace).or_insert(0);
        checked_accumulate_coeff(e, c_coeff, "left_divide_ts4_unique");
    }
    out.normalize();
    Some(out)
}

/// Полное левое деление TS4 через поиск решения системы уравнений.
/// Ограничено по числу переменных и глубине поиска.
pub fn left_divide_ts4_solve(
    a: &TS4,
    b: &TS4,
    max_vars: usize,
    max_solutions: usize,
) -> Option<TS4> {
    let mut normalized_a = a.clone();
    normalized_a.normalize();
    let mut normalized_b = b.clone();
    normalized_b.normalize();

    if nonzero_term_count(&normalized_a) == 0 {
        return None;
    }
    if max_solutions == 0 {
        return None;
    }

    // Build equations per b_trace: sum a_coeff * c_var = b_coeff
    #[derive(Clone)]
    struct Term {
        var: usize,
        coeff: u32,
    }
    #[derive(Clone)]
    struct Eqn {
        terms: Vec<Term>,
        rhs: i64,
    }

    let mut var_index: HashMap<Trace, usize> = HashMap::new();
    let mut vars: Vec<Trace> = Vec::new();
    let mut eqs: Vec<Eqn> = Vec::new();

    for (b_trace, b_coeff) in normalized_b.terms.iter().filter(|(_, c)| **c != 0) {
        let mut terms: Vec<Term> = Vec::new();
        for (a_trace, a_coeff) in normalized_a.terms.iter().filter(|(_, c)| **c != 0) {
            if let Some(c_trace) = left_divide_trace(a_trace, b_trace) {
                let idx = if let Some(&idx) = var_index.get(&c_trace) {
                    idx
                } else {
                    if vars.len() >= max_vars {
                        return None;
                    }
                    let idx = vars.len();
                    vars.push(c_trace);
                    var_index.insert(vars[idx].clone(), idx);
                    idx
                };
                terms.push(Term {
                    var: idx,
                    coeff: *a_coeff,
                });
            }
        }
        if terms.is_empty() {
            return None;
        }
        eqs.push(Eqn {
            terms,
            rhs: *b_coeff as i64,
        });
    }

    if vars.len() > max_vars {
        return None;
    }

    // Build adjacency: var -> list of (eq_idx, coeff)
    let mut adj: Vec<Vec<(usize, u32)>> = vec![Vec::new(); vars.len()];
    for (ei, eq) in eqs.iter().enumerate() {
        for t in eq.terms.iter() {
            adj[t.var].push((ei, t.coeff));
        }
    }

    let mut residuals: Vec<i64> = eqs.iter().map(|e| e.rhs).collect();
    let mut solution: Vec<u32> = vec![0; vars.len()];
    let mut found_solution: Option<Vec<u32>> = None;
    let mut solutions_found: usize = 0;

    // Order variables by degree (most constrained first)
    let mut order: Vec<usize> = (0..vars.len()).collect();
    order.sort_by_key(|&v| usize::MAX - adj[v].len());

    fn dfs(
        idx: usize,
        order: &[usize],
        adj: &[Vec<(usize, u32)>],
        residuals: &mut [i64],
        solution: &mut [u32],
        found_solution: &mut Option<Vec<u32>>,
        solutions_found: &mut usize,
        max_solutions: usize,
    ) -> bool {
        if *solutions_found >= max_solutions {
            return true;
        }

        if idx == order.len() {
            if residuals.iter().all(|&r| r == 0) {
                *solutions_found += 1;
                if found_solution.is_none() {
                    *found_solution = Some(solution.to_vec());
                }
                return *solutions_found >= max_solutions;
            }
            return false;
        }
        let var = order[idx];
        // Compute max feasible value for this var
        let mut max_v: i64 = i64::MAX;
        for (eq_idx, coeff) in adj[var].iter() {
            let r = residuals[*eq_idx];
            if *coeff == 0 {
                continue;
            }
            let mv = r / (*coeff as i64);
            if mv < max_v {
                max_v = mv;
            }
        }
        if max_v < 0 {
            return false;
        }
        let max_v = max_v as u32;
        for v in 0..=max_v {
            // apply
            for (eq_idx, coeff) in adj[var].iter() {
                residuals[*eq_idx] -= (v as i64) * (*coeff as i64);
            }
            solution[var] = v;
            if dfs(
                idx + 1,
                order,
                adj,
                residuals,
                solution,
                found_solution,
                solutions_found,
                max_solutions,
            ) {
                return true;
            }
            // rollback
            for (eq_idx, coeff) in adj[var].iter() {
                residuals[*eq_idx] += (v as i64) * (*coeff as i64);
            }
            solution[var] = 0;
        }
        false
    }

    dfs(
        0,
        &order,
        &adj,
        &mut residuals,
        &mut solution,
        &mut found_solution,
        &mut solutions_found,
        max_solutions,
    );
    let sol = found_solution.as_ref()?;

    let mut out = TS4::zero();
    for (i, coeff) in sol.iter().enumerate() {
        if *coeff == 0 {
            continue;
        }
        let e = out.terms.entry(vars[i].clone()).or_insert(0);
        checked_accumulate_coeff(e, *coeff, "left_divide_ts4_solve");
    }
    out.normalize();
    Some(out)
}

/// Режимы решения делимости TS4.
pub enum SolveMode {
    /// Ограниченный режим: гарантированная остановка по лимитам.
    Bounded {
        max_vars: usize,
        max_solutions: usize,
    },
    /// Неограниченный режим: без лимита на число переменных (может не завершиться).
    Unbounded { max_solutions: usize },
}

/// Гибридный решатель: bounded или unbounded (по выбору).
pub fn left_divide_ts4(a: &TS4, b: &TS4, mode: SolveMode) -> Option<TS4> {
    match mode {
        SolveMode::Bounded {
            max_vars,
            max_solutions,
        } => left_divide_ts4_solve(a, b, max_vars, max_solutions),
        SolveMode::Unbounded { max_solutions } => {
            left_divide_ts4_solve(a, b, usize::MAX, max_solutions)
        }
    }
}

/// Левый НОД трасс: наибольший общий левый делитель.
pub fn left_gcd_trace(a: &Trace, b: &Trace) -> Trace {
    a.assert_canonical("left_gcd_trace");
    b.assert_canonical("left_gcd_trace");
    let a0 = a.first_block();
    let b0 = b.first_block();
    if a0 != b0 {
        let min0 = Block::new(a0.x.min(b0.x), a0.y.min(b0.y), a0.z.min(b0.z));
        return Trace::new(vec![min0]);
    }
    let min_len = a.len_blocks().min(b.len_blocks());
    let prefix_len = a.common_prefix_len(b, min_len);
    let mut out = Vec::with_capacity(prefix_len.saturating_add(1));
    if prefix_len != 0 {
        append_blocks_range(&mut out, a, 0, prefix_len);
    }
    if prefix_len == min_len {
        return Trace::new(out);
    }
    let ai = a.block_at(prefix_len);
    let bi = b.block_at(prefix_len);
    let last = Block::new(ai.x.min(bi.x), ai.y.min(bi.y), ai.z.min(bi.z));
    out.push(last);
    Trace::new(out)
}

/// Левое деление трасс: найти c, такое что b = a ∘ c.
pub fn left_divide_trace(a: &Trace, b: &Trace) -> Option<Trace> {
    a.assert_canonical("left_divide_trace");
    b.assert_canonical("left_divide_trace");
    let p = a.len_blocks();
    let q = b.len_blocks();
    if p > q {
        return None;
    }
    if p == 1 {
        // b = (A0 + C0) | C1...
        let a0 = a.first_block();
        let b0 = b.first_block();
        if b0.x < a0.x || b0.y < a0.y || b0.z < a0.z {
            return None;
        }
        let c0 = b0.sub(a0);
        let mut out = Vec::with_capacity(q);
        out.push(c0);
        if q > 1 {
            append_blocks_range(&mut out, b, 1, q);
        }
        return Some(Trace::new(out));
    }

    // Prefix blocks must match exactly
    if !a.blocks_equal_range(b, 0, 0, p - 1) {
        return None;
    }

    // Boundary block
    let boundary = b.block_at(p - 1);
    let a_last = a.last_block();
    if boundary.x < a_last.x || boundary.y < a_last.y || boundary.z < a_last.z {
        return None;
    }
    let c0 = boundary.sub(a_last);

    let mut out = Vec::with_capacity(q - p + 1);
    out.push(c0);
    if q > p {
        append_blocks_range(&mut out, b, p, q);
    }
    Some(Trace::new(out))
}

/// Правое деление трасс: найти c, такое что b = c ∘ a.
pub fn right_divide_trace(a: &Trace, b: &Trace) -> Option<Trace> {
    a.assert_canonical("right_divide_trace");
    b.assert_canonical("right_divide_trace");
    let p = a.len_blocks();
    let q = b.len_blocks();
    if p > q {
        return None;
    }
    if p == 1 {
        let a0 = a.first_block();
        let b_last = b.last_block();
        if b_last.x < a0.x || b_last.y < a0.y || b_last.z < a0.z {
            return None;
        }
        let c_last = b_last.sub(a0);
        let mut out = Vec::with_capacity(q);
        if q > 1 {
            append_blocks_range(&mut out, b, 0, q - 1);
        }
        out.push(c_last);
        return Some(Trace::new(out));
    }

    // Suffix blocks must match exactly (excluding boundary)
    let r = q - p + 1; // blocks in c
    if !b.blocks_equal_range(a, r, 1, p - 1) {
        return None;
    }

    // Boundary block at r-1
    let boundary = b.block_at(r - 1);
    let a0 = a.first_block();
    if boundary.x < a0.x || boundary.y < a0.y || boundary.z < a0.z {
        return None;
    }
    let c_last = boundary.sub(a0);

    let mut out = Vec::with_capacity(r);
    if r > 1 {
        append_blocks_range(&mut out, b, 0, r - 1);
    }
    out.push(c_last);
    Some(Trace::new(out))
}
