/// Normalize CNPJ to standard format: XX.XXX.XXX/XXXX-XX
/// Accepts raw digits (14 digits), already-formatted, or partially formatted CNPJ.
pub fn normalize(cnpj: &str) -> String {
    let digits: String = cnpj.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() == 14 {
        format!(
            "{}.{}.{}/{}-{}",
            &digits[0..2],
            &digits[2..5],
            &digits[5..8],
            &digits[8..12],
            &digits[12..14],
        )
    } else {
        digits
    }
}
