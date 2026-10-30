//! elements.rs — Статическая периодическая таблица элементов (Z, атомные массы, заряды)
//! Обеспечивает инициализацию молекулярных структур в чистом Rust без PySCF.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElementInfo {
    pub atomic_number: i32,
    pub symbol: &'static str,
    pub name: &'static str,
    pub atomic_mass: f64,
    pub default_valency: u8,
}

pub static PERIODIC_TABLE: &[ElementInfo] = &[
    ElementInfo { atomic_number: 1, symbol: "H", name: "Hydrogen", atomic_mass: 1.008, default_valency: 1 },
    ElementInfo { atomic_number: 2, symbol: "He", name: "Helium", atomic_mass: 4.0026, default_valency: 0 },
    ElementInfo { atomic_number: 3, symbol: "Li", name: "Lithium", atomic_mass: 6.94, default_valency: 1 },
    ElementInfo { atomic_number: 4, symbol: "Be", name: "Beryllium", atomic_mass: 9.0122, default_valency: 2 },
    ElementInfo { atomic_number: 5, symbol: "B", name: "Boron", atomic_mass: 10.81, default_valency: 3 },
    ElementInfo { atomic_number: 6, symbol: "C", name: "Carbon", atomic_mass: 12.011, default_valency: 4 },
    ElementInfo { atomic_number: 7, symbol: "N", name: "Nitrogen", atomic_mass: 14.007, default_valency: 3 },
    ElementInfo { atomic_number: 8, symbol: "O", name: "Oxygen", atomic_mass: 15.999, default_valency: 2 },
    ElementInfo { atomic_number: 9, symbol: "F", name: "Fluorine", atomic_mass: 18.998, default_valency: 1 },
    ElementInfo { atomic_number: 10, symbol: "Ne", name: "Neon", atomic_mass: 20.180, default_valency: 0 },
    ElementInfo { atomic_number: 11, symbol: "Na", name: "Sodium", atomic_mass: 22.990, default_valency: 1 },
    ElementInfo { atomic_number: 12, symbol: "Mg", name: "Magnesium", atomic_mass: 24.305, default_valency: 2 },
    ElementInfo { atomic_number: 13, symbol: "Al", name: "Aluminium", atomic_mass: 26.982, default_valency: 3 },
    ElementInfo { atomic_number: 14, symbol: "Si", name: "Silicon", atomic_mass: 28.085, default_valency: 4 },
    ElementInfo { atomic_number: 15, symbol: "P", name: "Phosphorus", atomic_mass: 30.974, default_valency: 3 },
    ElementInfo { atomic_number: 16, symbol: "S", name: "Sulfur", atomic_mass: 32.06, default_valency: 2 },
    ElementInfo { atomic_number: 17, symbol: "Cl", name: "Chlorine", atomic_mass: 35.45, default_valency: 1 },
    ElementInfo { atomic_number: 18, symbol: "Ar", name: "Argon", atomic_mass: 39.948, default_valency: 0 },
];

pub fn get_element_by_z(z: i32) -> Option<&'static ElementInfo> {
    PERIODIC_TABLE.iter().find(|e| e.atomic_number == z)
}

pub fn get_element_by_symbol(symbol: &str) -> Option<&'static ElementInfo> {
    PERIODIC_TABLE.iter().find(|e| e.symbol.eq_ignore_ascii_case(symbol))
}
