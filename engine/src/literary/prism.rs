//! Принцип «No Excuses» — Закон Сохранения Смысла (архетип «Prism»).
//!
//! Если прямой семантический путь заблокирован (проектор причинности
//! аннигилирует компоненту обновления, или свободная энергия растёт —
//! семантический тупик), энергия смысла не может исчезнуть: она обязана
//! **преломиться**. Символическое замещение меняет форму, сохраняя
//! энергетическую суть замысла.
//!
//! Математика — точная, без приближений:
//!
//! 1. Спектральный расклад: v = a + b, где a = Πv (разрешённая часть,
//!    null(J_c)), b = v − Πv (заблокированная, range(J_cᵀ)); Π —
//!    ортогональный проектор ⇒ a ⊥ b ⇒ ‖v‖² = ‖a‖² + ‖b‖².
//! 2. Преломление: заблокированная энергия ‖b‖ переизлучается ВНУТРИ
//!    разрешённого подпространства вдоль ближайшего архетипа:
//!    c = ‖b‖·normalize(Π(ĉ)), где ĉ — ось архетипа с максимальным
//!    положительным косинусом к b. Переизлучение ортогонализуется к a.
//! 3. Итог: r = a + c, причём a ⊥ c, ‖c‖ = ‖b‖ ⇒ **‖r‖ = ‖v‖** с
//!    точностью f32-эпсилон, и J_c·r = 0 — причинность не нарушена.
//!
//! Режимы (диагностика для агента):
//! - `Direct` — блокировки нет (‖b‖ ≈ 0), преломление не требуется;
//! - `Refraction` — переизлучение в ближайший разрешённый архетип;
//! - `TotalInternal` — все архетипы отвергли направление (косинус ≤ 0
//!   или Π убивает ось): детерминированная циклическая подстановка
//!   формы (поворот осей), энергия сохранена перестановкой.

use super::linalg::{cosine, normalize, norm2, sub};

/// Режим преломления.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefractionMode {
    /// Путь свободен: блокировки нет.
    Direct,
    /// Преломление в ближайший разрешённый архетип.
    Refraction,
    /// Полное внутреннее отражение: символическая подстановка осей.
    TotalInternal,
}

/// Результат работы призмы.
pub struct Prism {
    /// Преломлённый вектор (энергия сохранена, причинность чиста).
    pub refracted: Vec<f32>,
    /// Заблокированная компонента исходника (что не прошло напрямую).
    pub blocked: Vec<f32>,
    /// Режим преломления.
    pub mode: RefractionMode,
    /// Индекс архетипа-приёмника (None для Direct/TotalInternal).
    pub via_archetype: Option<usize>,
    /// Косинус заблокированного направления с осью приёмника.
    pub alignment: f32,
    /// Безвозвратно потерянная энергия (≠ 0 только в вырожденном
    /// подпространстве; норма 0 — Закон Сохранения выполнен).
    pub energy_loss: f32,
}

/// Порог «блокировки нет»: ‖b‖ ниже — шум f32, не смысл.
const DIRECT_EPS: f32 = 1e-6;
/// Порог «ось выжила после проекции».
const AXIS_EPS: f32 = 1e-6;

/// Преломить вектор сквозь призму.
///
/// - `v` — заблокированный вектор (градиент/обновление);
/// - `project` — ортогональный проектор Π_Λ (замыкание над
///   [`crate::literary::projector::CausalProjector`], чтобы не тащить
///   зависимость модулей);
/// - `allowed_axes` — оси-кандидаты приёма (якоря архетипов; сырые,
///   проекция Π выполняется внутри).
pub fn refract(
    v: &[f32],
    project: impl Fn(&[f32]) -> Vec<f32>,
    allowed_axes: &[Vec<f32>],
) -> Prism {
    let a = project(v);
    let b = sub(v, &a);
    let b_norm = norm2(&b);
    if b_norm <= DIRECT_EPS {
        return Prism {
            refracted: v.to_vec(),
            blocked: b,
            mode: RefractionMode::Direct,
            via_archetype: None,
            alignment: 1.0,
            energy_loss: 0.0,
        };
    }

    // Кандидаты приёма: архетипы с положительным косинусом к b,
    // отсортированные по близости (детерминированно: косинус ↓, индекс ↑).
    let mut candidates: Vec<(usize, f32)> = Vec::with_capacity(allowed_axes.len());
    for (i, axis) in allowed_axes.iter().enumerate() {
        if axis.len() != v.len() {
            continue;
        }
        let c = cosine(&b, axis);
        if c > 1e-6 {
            candidates.push((i, c));
        }
    }
    candidates.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap().then(x.0.cmp(&y.0)));

    // Первая ось, чья проекция в null(J_c) жива.
    for &(idx, align) in &candidates {
        if let Some(c) = reemit(&b, &a, || project(&allowed_axes[idx]), v.len()) {
            return Prism {
                refracted: combine(&a, &c),
                blocked: b,
                mode: RefractionMode::Refraction,
                via_archetype: Some(idx),
                alignment: align,
                energy_loss: 0.0,
            };
        }
    }

    // Полное внутреннее отражение: детерминированный поворот формы.
    // Энергия ‖b‖ переизлучается вдоль Π(rotate(b)) — перестановка осей
    // меняет форму, сохраняя суть.
    let rotated: Vec<f32> = {
        let mut r = b.clone();
        r.rotate_right(1);
        r
    };
    if let Some(c) = reemit(&b, &a, || project(&rotated), v.len()) {
        return Prism {
            refracted: combine(&a, &c),
            blocked: b,
            mode: RefractionMode::TotalInternal,
            via_archetype: None,
            alignment: 0.0,
            energy_loss: 0.0,
        };
    }

    // Вырожденный случай: null(J_c) ортогонален всему подряд — энергия
    // честно фиксируется как потерянная (диагностика, не молчание).
    Prism {
        refracted: a,
        blocked: b,
        mode: RefractionMode::TotalInternal,
        via_archetype: None,
        alignment: 0.0,
        energy_loss: b_norm,
    }
}

/// Переизлучение заблокированной энергии вдоль направления d (сырого):
/// c = ‖b‖·normalize(orth(Π(d), a)). None — направление мертво.
fn reemit(b: &[f32], a: &[f32], d: impl FnOnce() -> Vec<f32>, dims: usize) -> Option<Vec<f32>> {
    let b_norm = norm2(b);
    let mut dir = d();
    if dir.len() != dims || norm2(&dir) < AXIS_EPS {
        return None;
    }
    // Ортогонализация к a (Грам—Шмидт): c ⊥ a.
    let a_hat = normalize(a);
    let proj = super::linalg::dot(&dir, &a_hat);
    if proj.abs() > 1e-12 {
        for (x, &ah) in dir.iter_mut().zip(&a_hat) {
            *x -= proj * ah;
        }
    }
    let n = norm2(&dir);
    if n < AXIS_EPS {
        return None;
    }
    let k = b_norm / n;
    Some(dir.iter().map(|&x| x * k).collect())
}

/// r = a + c (поэлементно; длины совпадают по построению).
fn combine(a: &[f32], c: &[f32]) -> Vec<f32> {
    a.iter().zip(c).map(|(&x, &y)| x + y).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::literary::projector::CausalProjector;

    fn axes() -> Vec<Vec<f32>> {
        vec![
            normalize(&[1.0, 0.0, 0.0, 0.0]),
            normalize(&[0.0, 0.0, 1.0, 0.0]),
            normalize(&[0.0, 1.0, 0.0, 0.0]),
        ]
    }

    #[test]
    fn energy_conserved_and_causality_clean() {
        let p = CausalProjector::canonical_lock(4).unwrap();
        let v = vec![0.8, -0.2, 0.5, 0.3];
        let r = refract(&v, |x| p.project(x), &axes());
        let (nv, nr) = (norm2(&v), norm2(&r.refracted));
        assert!(
            (nv - nr).abs() < 1e-4,
            "Закон Сохранения Смысла: {nv} vs {nr} (mode {:?})",
            r.mode
        );
        assert_eq!(r.mode, RefractionMode::Refraction);
        assert_eq!(r.energy_loss, 0.0);
        // Преломлённый вектор удовлетворяет причинному закону
        assert!(p.residual(&r.refracted) < 1e-4, "причинность после призмы");
    }

    #[test]
    fn direct_when_no_blockage() {
        let p = CausalProjector::new(&[], 4).unwrap(); // Π = I
        let v = vec![0.5, 0.5, 0.7, 0.1];
        let r = refract(&v, |x| p.project(x), &axes());
        assert_eq!(r.mode, RefractionMode::Direct);
        assert_eq!(r.refracted, v);
        assert!(r.blocked.iter().all(|&x| x.abs() < 1e-6));
        assert_eq!(r.energy_loss, 0.0);
    }

    #[test]
    fn total_internal_when_all_axes_reject() {
        let p = CausalProjector::canonical_lock(4).unwrap();
        // Оси-приёмники лежат в плоскостях, ортогональных заблокированной
        // компоненте: косинусы с b = (1,−1,0,0)/√2 равны нулю.
        let axes_reject = vec![
            normalize(&[0.0, 0.0, 1.0, 0.0]),
            normalize(&[0.0, 0.0, 0.0, 1.0]),
        ];
        let v = vec![1.0, -1.0, 0.0, 0.0];
        let r = refract(&v, |x| p.project(x), &axes_reject);
        assert_eq!(r.mode, RefractionMode::TotalInternal, "mode = {:?}", r.mode);
        let (nv, nr) = (norm2(&v), norm2(&r.refracted));
        assert!((nv - nr).abs() < 1e-4, "подстановка сохраняет норму: {nv} vs {nr}");
        // Причинность чиста и в полном отражении
        assert!(p.residual(&r.refracted) < 1e-4);
        assert_eq!(r.energy_loss, 0.0);
    }

    #[test]
    fn prism_picks_nearest_allowed_archetype() {
        let p = CausalProjector::canonical_lock(4).unwrap();
        // b ∝ (1,−1,0,0)/√2: косинус +1/√2 с осью 0 = (1,0,1,0)/√2
        // (близкая), 0 с осью 1, −1/√2 с осью 2 → выбор индекса 0.
        // Третья компонента v даёт проекции осей компоненту, живую после
        // ортогонализации к a (вырожденный случай a ∥ Π(ось) исключён).
        let axes = vec![
            normalize(&[1.0, 0.0, 1.0, 0.0]),
            normalize(&[0.0, 0.0, 1.0, 0.0]),
            normalize(&[0.0, 1.0, 0.0, 0.0]),
        ];
        let v = vec![2.0, 0.0, 0.6, 0.0];
        let r = refract(&v, |x| p.project(x), &axes);
        assert_eq!(r.mode, RefractionMode::Refraction, "mode: {:?}", r.mode);
        assert_eq!(r.via_archetype, Some(0), "alignment = {}", r.alignment);
        assert!(r.alignment > 0.0);
        // Энергия и причинность — по-прежнему святы
        assert!((norm2(&v) - norm2(&r.refracted)).abs() < 1e-4);
        assert!(p.residual(&r.refracted) < 1e-4);
    }
}
