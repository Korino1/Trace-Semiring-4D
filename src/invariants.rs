//! Инварианты физических трасс (масса, время, оценки).

use crate::trace::Trace;

/// Суммарная пространственная масса (L1) по всем блокам.
pub fn mass_l1(trace: &Trace) -> u64 {
    trace.mass_l1()
}

/// Число блоков (слоёв времени).
pub fn layers(trace: &Trace) -> usize {
    trace.len_blocks()
}

/// Число τ (len_blocks - 1).
pub fn tau_count(trace: &Trace) -> usize {
    trace.tau_count()
}

/// Нижняя граница числа блоков при лимите κ (ceil(mass/κ)).
pub fn min_layers_for_mass(mass: u64, kappa: u32) -> usize {
    assert!(kappa >= 1, "min_layers_for_mass requires kappa >= 1");
    let k = kappa as u64;
    ((mass + k - 1) / k) as usize
}

/// Нижняя граница числа τ при лимите κ (min_layers - 1).
pub fn min_tau_for_mass(mass: u64, kappa: u32) -> usize {
    assert!(kappa >= 1, "min_tau_for_mass requires kappa >= 1");
    let layers = min_layers_for_mass(mass, kappa);
    layers.saturating_sub(1)
}
