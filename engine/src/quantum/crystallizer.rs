//! # R1CS Circuit & Safetensors Weight Crystallizer
//!
//! Воплощает принцип POLER-ERI v3.2.0 (CircuitBuilder -> R1CS Circuit -> VrrCrystallizer):
//! Превращает плотные матрицы весов стандартных моделей (.safetensors)
//! в плоские R1CS-вентили (Zero-Alloc, No-Mul, Single-Pass), выполняемые в L1/L3 кэше.

use serde::Deserialize;
use std::collections::BTreeMap;

/// Метаданные тензора из safetensors JSON заголовка.
#[derive(Debug, Clone, Deserialize)]
pub struct TensorInfo {
    pub dtype: String,
    pub shape: Vec<usize>,
    pub data_offsets: [usize; 2],
}

/// Заголовок safetensors файла.
pub struct SafetensorsHeader {
    pub metadata: BTreeMap<String, String>,
    pub tensors: BTreeMap<String, TensorInfo>,
    pub header_size: usize,
}

impl SafetensorsHeader {
    /// Парсит заголовок safetensors напрямую из mmap или буфера байт.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 8 {
            return Err("Файл слишком мал для safetensors (меньше 8 байт)".to_string());
        }
        let header_len = u64::from_le_bytes(bytes[0..8].try_into().unwrap()) as usize;
        if bytes.len() < 8 + header_len {
            return Err(format!(
                "Размер файла ({}) меньше заявленного заголовка (8 + {})",
                bytes.len(),
                header_len
            ));
        }

        let header_json = std::str::from_utf8(&bytes[8..8 + header_len])
            .map_err(|e| format!("Некорректный UTF-8 в заголовке safetensors: {e}"))?;

        let raw_map: BTreeMap<String, serde_json::Value> = serde_json::from_str(header_json)
            .map_err(|e| format!("Ошибка парсинга JSON заголовка: {e}"))?;

        let mut metadata = BTreeMap::new();
        let mut tensors = BTreeMap::new();

        for (k, v) in raw_map {
            if k == "__metadata__" {
                if let serde_json::Value::Object(map) = v {
                    for (mk, mv) in map {
                        if let serde_json::Value::String(s) = mv {
                            metadata.insert(mk, s);
                        }
                    }
                }
            } else {
                let info: TensorInfo = serde_json::from_value(v)
                    .map_err(|e| format!("Ошибка разбора метаданных тензора '{k}': {e}"))?;
                tensors.insert(k, info);
            }
        }

        Ok(Self {
            metadata,
            tensors,
            header_size: 8 + header_len,
        })
    }
}

/// Коэффициент входа R1CS вентиля: c ∈ {-1, 0, +1} или скалярное масштабирование.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GateCoeff {
    Zero,
    One,
    NegOne,
    Scalar(f32),
}

/// R1CS Вентиль: `operands[result] = coeff_left * operands[left] + coeff_right * operands[right]`
#[derive(Debug, Clone, PartialEq)]
pub struct R1CSGate {
    pub result: usize,
    pub left: usize,
    pub right: usize,
    pub coeff_left: GateCoeff,
    pub coeff_right: GateCoeff,
    pub label: String,
}

/// Строитель схемы R1CS цепей для линейных трансформаций весов.
#[derive(Default)]
pub struct WeightCircuitBuilder {
    pub gates: Vec<R1CSGate>,
    pub n_operands: usize,
}

impl WeightCircuitBuilder {
    pub fn new(input_dim: usize) -> Self {
        Self {
            gates: Vec::new(),
            n_operands: input_dim,
        }
    }

    /// Добавляет R1CS вентиль сложения/вычитания двух промежуточных операндов.
    pub fn add_gate(
        &mut self,
        left: usize,
        c_left: GateCoeff,
        right: usize,
        c_right: GateCoeff,
        label: impl Into<String>,
    ) -> usize {
        let result = self.n_operands;
        self.n_operands += 1;
        self.gates.push(R1CSGate {
            result,
            left,
            right,
            coeff_left: c_left,
            coeff_right: c_right,
            label: label.into(),
        });
        result
    }

    /// Преобразует матрицу проекции весов (напр. тритернарный ротор J или квантованные веса)
    /// в плоский конвейер R1CS вентилей.
    pub fn compile_linear_layer(
        &mut self,
        in_dim: usize,
        out_dim: usize,
        matrix: &[i8], // ternary: -1, 0, 1
    ) -> Vec<usize> {
        let mut output_indices = Vec::with_capacity(out_dim);

        for row in 0..out_dim {
            let row_offset = row * in_dim;
            let mut accum_idx: Option<usize> = None;

            for col in 0..in_dim {
                let val = matrix[row_offset + col];
                if val == 0 {
                    continue;
                }

                let coeff = if val > 0 { GateCoeff::One } else { GateCoeff::NegOne };

                if let Some(prev) = accum_idx {
                    accum_idx = Some(self.add_gate(
                        prev,
                        GateCoeff::One,
                        col,
                        coeff,
                        format!("acc_r{row}_c{col}"),
                    ));
                } else {
                    // Первый ненулевой элемент
                    accum_idx = Some(self.add_gate(
                        col,
                        coeff,
                        0,
                        GateCoeff::Zero,
                        format!("init_r{row}_c{col}"),
                    ));
                }
            }

            output_indices.push(accum_idx.unwrap_or(0));
        }

        output_indices
    }
}

/// Кристаллизатор R1CS схем в плоский, без-аллокационный Rust-код.
pub struct WeightCrystallizer;

impl WeightCrystallizer {
    /// Генерирует плоский Rust-код функции выполнения для прямого прохода.
    pub fn crystallize_to_rust(fn_name: &str, builder: &WeightCircuitBuilder) -> String {
        let mut code = String::new();
        code.push_str("/// Автоматически сгенерированное ядро R1CS без умножений (Zero-Alloc / No-Mul)\n");
        code.push_str(&format!("pub const N_OPERANDS: usize = {};\n", builder.n_operands));
        code.push_str("#[inline(always)]\n");
        code.push_str(&format!(
            "pub fn {fn_name}(operands: &mut [f32]) {{\n"
        ));
        code.push_str("    debug_assert!(operands.len() >= N_OPERANDS);\n");

        for (i, gate) in builder.gates.iter().enumerate() {
            let left_expr = match gate.coeff_left {
                GateCoeff::Zero => String::new(),
                GateCoeff::One => format!("operands[{}]", gate.left),
                GateCoeff::NegOne => format!("-operands[{}]", gate.left),
                GateCoeff::Scalar(s) => format!("{} * operands[{}]", s, gate.left),
            };

            let right_expr = match gate.coeff_right {
                GateCoeff::Zero => String::new(),
                GateCoeff::One => format!("operands[{}]", gate.right),
                GateCoeff::NegOne => format!("-operands[{}]", gate.right),
                GateCoeff::Scalar(s) => format!("{} * operands[{}]", s, gate.right),
            };

            let stmt = if left_expr.is_empty() && right_expr.is_empty() {
                format!("    operands[{}] = 0.0;", gate.result)
            } else if right_expr.is_empty() {
                format!("    operands[{}] = {};", gate.result, left_expr)
            } else if left_expr.is_empty() {
                format!("    operands[{}] = {};", gate.result, right_expr)
            } else if right_expr.starts_with('-') {
                format!(
                    "    operands[{}] = {} - {};",
                    gate.result,
                    left_expr,
                    &right_expr[1..]
                )
            } else {
                format!(
                    "    operands[{}] = {} + {};",
                    gate.result, left_expr, right_expr
                )
            };

            code.push_str(&format!("    // Gate {i}: {}\n{stmt}\n", gate.label));
        }

        code.push_str("}\n");
        code
    }
}

/// Исполняет R1CS схему напрямую в памяти без повторной компиляции.
#[inline(always)]
pub fn execute_circuit_direct(builder: &WeightCircuitBuilder, operands: &mut [f32]) {
    for gate in &builder.gates {
        let left_val = match gate.coeff_left {
            GateCoeff::Zero => 0.0,
            GateCoeff::One => operands[gate.left],
            GateCoeff::NegOne => -operands[gate.left],
            GateCoeff::Scalar(s) => s * operands[gate.left],
        };
        let right_val = match gate.coeff_right {
            GateCoeff::Zero => 0.0,
            GateCoeff::One => operands[gate.right],
            GateCoeff::NegOne => -operands[gate.right],
            GateCoeff::Scalar(s) => s * operands[gate.right],
        };
        operands[gate.result] = left_val + right_val;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_r1cs_circuit_crystallization() {
        let mut builder = WeightCircuitBuilder::new(4);
        let ternary_weights: [i8; 8] = [
            1, 0, -1, 0,  // row 0: x0 - x2
            0, 1,  0, 1,  // row 1: x1 + x3
        ];
        let out_indices = builder.compile_linear_layer(4, 2, &ternary_weights);

        assert_eq!(out_indices.len(), 2);
        assert!(builder.gates.len() >= 2);

        let mut operands = vec![0.0f32; builder.n_operands];
        operands[0] = 10.0;
        operands[1] = 5.0;
        operands[2] = 3.0;
        operands[3] = 2.0;

        execute_circuit_direct(&builder, &mut operands);

        let y0 = operands[out_indices[0]];
        let y1 = operands[out_indices[1]];

        assert_eq!(y0, 7.0);
        assert_eq!(y1, 7.0);

        let rust_code = WeightCrystallizer::crystallize_to_rust("test_forward", &builder);
        assert!(rust_code.contains("pub fn test_forward"));
        assert!(rust_code.contains("operands["));
    }
}
