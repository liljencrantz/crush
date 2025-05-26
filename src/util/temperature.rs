

#[derive(Copy, Clone)]
pub enum Temperature {
    Celsius,
    Kelvin,
    Fahrenheit,
}

impl Temperature {

    pub fn format(&self, kelvin: f64) -> f64{
        match self {
            Temperature::Celsius => kelvin - 273.15,
            Temperature::Kelvin => kelvin,
            Temperature::Fahrenheit => ((kelvin - 273.15) * (9.0 / 5.0)) + 32.0,
        }
    }

    pub fn unit(&self) -> &str{
        match self {
            Temperature::Celsius => "°C",
            Temperature::Kelvin => "K", // Kelvin is an absolute unit, so no ° symbol
            Temperature::Fahrenheit => "°F",
        }
    }

}
