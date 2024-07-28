
pub fn to_real(value: f64) -> Result<currency_rs::Currency, currency_rs::CurrencyErr> {
  let otp = currency_rs::CurrencyOpts::new()
      .set_separator(".")
      .set_decimal(",")
      .set_symbol("R$ ");

  Ok(currency_rs::Currency::new_float(value, Some(otp)))
}
