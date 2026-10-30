use std::collections::HashMap;

fn compute(value: i32) -> i32 {
    value * 2
}

pub fn process_data(input: &[i32]) -> Vec<i32> {
    input.iter().map(|x| compute(*x)).collect()
}

fn helper(extra: &HashMap<String, i32>) -> i32 {
    extra.values().sum()
}

fn main() {
    let data = vec![1, 2, 3];
    let result = process_data(&data);
    println!("{:?}", result);
}
