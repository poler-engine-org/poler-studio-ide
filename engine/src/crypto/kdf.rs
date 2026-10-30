//! Вывод 256-битного ключа Vault из парольной фразы (KDF).
//!
//! Стойкость к перебору паролей обеспечивается итеративной свёрткой
//! через полный шифр PND v8.2 (Feistel ×20 на итерацию) и pndMix/Φ.
//! Соль (8 слов, свежая при каждой печати Vault) разводит одинаковые
//! пароли в разные ключи и делает невозможным радужную переиспользуемость
//! таблиц между контейнерами. Домен «POLER.VAULT.KDF.v1» отделяет поток
//! от хешера и MAC — один и тот же пароль не даёт связанных ключей
//! в разных применениях.
//!
//! Работа фактора: DEFAULT_ITERATIONS подбирается так, чтобы вывод
//! ключа занимал десятки миллисекунд на обычном CPU — мгновенно для
//! владельца и дорого для массового перебора.

use super::pnd::{phi, pnd_mix, PolerCipher, PolerDrbg, KEY_WORDS};

/// Домен KDF (domain separation от хешера/MAC).
pub const DOMAIN: &str = "POLER.VAULT.KDF.v1";

/// Итерации по умолчанию (печать/вскрытие Vault).
pub const DEFAULT_ITERATIONS: u32 = 100_000;

/// Минимум, ниже которого печать отказывается работать (защита от
/// случайного обнуления стойкости через флаг).
pub const MIN_ITERATIONS: u32 = 10_000;

/// Свёртка парольной фразы в 8 слов (Φ-каскад по байтам).
fn fold_passphrase(passphrase: &str) -> [u32; KEY_WORDS] {
    let mut acc = [0u32; KEY_WORDS];
    for (i, b) in passphrase.bytes().enumerate() {
        let j = i % KEY_WORDS;
        acc[j] = phi(acc[j].wrapping_add((b as u32) << ((i % 5) * 6)))
            .rotate_left(((i / KEY_WORDS) % 31) as u32);
    }
    // Пустая фраза не даёт нулевого сидa: доменная константа.
    if passphrase.is_empty() {
        for (j, w) in acc.iter_mut().enumerate() {
            *w = phi((j as u32).wrapping_mul(0x9E3779B9));
        }
    }
    acc
}

/// Вывести 256-битный ключ из парольной фразы и соли.
///
/// `iterations >= MIN_ITERATIONS` (иначе — Err). Детерминизм: одна и
/// та же (фраза, соль, итерации) → один и тот же ключ на любой машине —
/// это контракт переоткрытия Vault на новом железе после git-клона.
pub fn derive_key(
    passphrase: &str,
    salt: &[u32; KEY_WORDS],
    iterations: u32,
) -> Result<[u32; KEY_WORDS], String> {
    if iterations < MIN_ITERATIONS {
        return Err(format!(
            "iterations={iterations} ниже минимума {MIN_ITERATIONS} (защита от слабого вывода ключа)"
        ));
    }

    // Состояние = свёртка фразы ⊕ соль, затем доменная домешка.
    let mut state = fold_passphrase(passphrase);
    for w in 0..KEY_WORDS {
        state[w] ^= salt[w];
    }
    for (i, b) in DOMAIN.bytes().enumerate() {
        let j = i % KEY_WORDS;
        state[j] = phi(state[j] ^ (b as u32));
    }

    // Итеративная прокачка: каждая итерация — полный шифр (Feistel ×20)
    // над левой половиной + pndMix-диффузия правой, полушага со свёрткой.
    let epsilon = phi(state[0] ^ state[4]) | 1;
    let cipher = PolerCipher::new(&state, epsilon).ok_or_else(|| "KDF: выделение шифра".to_string())?;
    let mut counter: u32 = 0;
    for _ in 0..iterations {
        let left = [
            state[0] ^ counter,
            state[1].rotate_left(3),
            state[2] ^ counter.rotate_left(17),
            state[3],
        ];
        let l = cipher.encrypt_block(&left);
        state[4] = pnd_mix(state[4] ^ l[0], l[2], epsilon);
        state[5] = pnd_mix(state[5] ^ l[1], l[3], epsilon);
        state[6] = pnd_mix(state[6] ^ l[2], l[0], epsilon);
        state[7] = pnd_mix(state[7] ^ l[3], l[1], epsilon);
        state[0] = l[0] ^ state[4].rotate_left(9);
        state[1] = l[1] ^ state[5].rotate_left(11);
        state[2] = l[2] ^ state[6].rotate_left(13);
        state[3] = l[3] ^ state[7].rotate_left(15);
        counter = counter.wrapping_add(0x9E3779B9);
    }

    // Финальная DRBG-экстракция: разворачивает свёрнутое состояние
    // в равномерный 256-битный ключ (выравнивание распределения).
    let mut drbg =
        PolerDrbg::new(&state).ok_or_else(|| "KDF: выделение DRBG".to_string())?;
    Ok(core::array::from_fn(|_| drbg.next()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_ITER: u32 = MIN_ITERATIONS; // быстрые тесты

    #[test]
    fn deterministic() {
        let salt = [5u32; KEY_WORDS];
        let a = derive_key("суверенный ключ", &salt, TEST_ITER).unwrap();
        let b = derive_key("суверенный ключ", &salt, TEST_ITER).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn salt_and_passphrase_sensitivity() {
        let s1 = [1u32; KEY_WORDS];
        let s2 = [2u32; KEY_WORDS];
        let k1 = derive_key("alpha", &s1, TEST_ITER).unwrap();
        // Другая соль → другой ключ.
        assert_ne!(k1, derive_key("alpha", &s2, TEST_ITER).unwrap());
        // Другая фраза (даже однобуквенная) → другой ключ.
        assert_ne!(k1, derive_key("alphа", &s1, TEST_ITER).unwrap()); // кириллическая а
        assert_ne!(k1, derive_key("Alpha", &s1, TEST_ITER).unwrap());
        // Однобитовый флип соли.
        let mut s3 = s1;
        s3[7] ^= 1;
        assert_ne!(k1, derive_key("alpha", &s3, TEST_ITER).unwrap());
    }

    #[test]
    fn iterations_guard_and_change() {
        let salt = [9u32; KEY_WORDS];
        assert!(derive_key("k", &salt, MIN_ITERATIONS - 1).is_err());
        let k1 = derive_key("k", &salt, TEST_ITER).unwrap();
        let k2 = derive_key("k", &salt, TEST_ITER + 5_000).unwrap();
        assert_ne!(k1, k2);
    }

    #[test]
    fn empty_passphrase_is_not_degenerate() {
        let salt = [7u32; KEY_WORDS];
        let k = derive_key("", &salt, TEST_ITER).unwrap();
        assert_ne!(k, [0u32; KEY_WORDS]);
        // Не совпадает с односимвольными фразами.
        assert_ne!(k, derive_key("\0", &salt, TEST_ITER).unwrap());
        assert_ne!(k, derive_key(" ", &salt, TEST_ITER).unwrap());
    }

    /// Ключи не вырождаются в слабые паттерны (все слова ненулевые,
    /// попарно различны) на серии фраз.
    #[test]
    fn no_weak_key_patterns() {
        let salt = [0xC0FFEE; KEY_WORDS];
        for phrase in ["a", "password", "12345", "гиппокамп", "x".repeat(1000).as_str()] {
            let k = derive_key(phrase, &salt, TEST_ITER).unwrap();
            assert!(k.iter().all(|&w| w != 0), "нулевое слово для {phrase:?}");
            let uniq: std::collections::HashSet<_> = k.iter().collect();
            assert_eq!(uniq.len(), KEY_WORDS, "дубликаты слов для {phrase:?}");
        }
    }
}
